//! F2XM1 and FSCALE values, independent of guest state and memory.
use crate::{
    ARITHMETIC_LOCK, Arithmetic, DENORMAL, Ext80, INVALID, NumericResult, OVERFLOW, PRECISION,
    UNDERFLOW, Unsupported, arithmetic,
};

unsafe extern "C" {
    fn vine_exponential_minus_one(
        sig: u64,
        exp: u32,
        rounding: u32,
        unmasked: u32,
        out: *mut u64,
    ) -> u32;
}
fn result(value: Ext80, exceptions: u16) -> NumericResult {
    NumericResult {
        value,
        exceptions,
        ..NumericResult::default()
    }
}
fn infinity(sign: u16) -> Ext80 {
    Ext80 {
        significand: 1 << 63,
        sign_exponent: sign | 0x7fff,
    }
}

/// Full64, RC-selected 2^x-1. All defined PC settings retain full precision.
/// Finite operands outside [-1,1] have undefined architectural results and
/// stop explicitly. This is not an x87 invalid-operation exception.
pub fn exponential_minus_one(x: Ext80, control: u16) -> Result<NumericResult, Unsupported> {
    if control >> 8 & 3 == 1 {
        return Err(Unsupported::ReservedPrecision);
    }
    if x.is_unsupported() {
        return Ok(result(Ext80::INDEFINITE, INVALID));
    }
    if x.is_nan() {
        return arithmetic(Arithmetic::Add, x, x, control | 0x300);
    }
    if x.exponent() == 0x7fff {
        return Ok(result(
            if x.sign_exponent & 0x8000 != 0 {
                Ext80 {
                    sign_exponent: 0xbfff,
                    ..Ext80::ONE
                }
            } else {
                x
            },
            0,
        ));
    }
    if x.exponent() > 0x3fff || x.exponent() == 0x3fff && x.significand != 1 << 63 {
        return Err(Unsupported::TranscendentalDomain);
    }
    if x.significand == 0 {
        return Ok(result(x, 0));
    }
    let denormal = if x.is_denormal() { DENORMAL } else { 0 };
    if denormal & !control != 0 {
        return Ok(result(Ext80::INDEFINITE, denormal));
    }
    let x = x.canonical();
    if x.exponent() == 0x3fff {
        // The exact endpoint satisfies Intel's accuracy bound. PE remains
        // observable on the ordinary nonzero transcendental path.
        return Ok(result(
            if x.sign_exponent & 0x8000 != 0 {
                Ext80 {
                    sign_exponent: 0xbffe,
                    ..Ext80::ONE
                }
            } else {
                Ext80::ONE
            },
            PRECISION,
        ));
    }
    let _guard = ARITHMETIC_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let mut out = [0u64; 4];
    // SAFETY: finite nonzero in-domain value, bounded RC and owned output.
    let certified = unsafe {
        vine_exponential_minus_one(
            x.significand,
            u32::from(x.sign_exponent),
            u32::from(control >> 10 & 3),
            u32::from(control & UNDERFLOW == 0),
            out.as_mut_ptr(),
        )
    };
    if certified == 0 {
        return Err(Unsupported::UncertainRounding);
    }
    Ok(NumericResult {
        value: Ext80 {
            significand: out[0],
            sign_exponent: out[1] as u16,
        },
        exceptions: out[3] as u16 | denormal,
        round_up: out[3] >> 16 != 0,
        bits: 0,
    })
}

/// ST0 * 2^trunc(ST1). The scale conversion never uses RC or raises PE.
/// Integer normalization and final packing retain the full64 significand.
/// Unmasked OE/UE apply the architectural +/-24576 exponent adjustment to
/// the original exact result, before rounding. If the adjusted result itself
/// is out of the normal exponent range, the unsupported boundary is explicit.
pub fn scale(x: Ext80, y: Ext80, control: u16) -> Result<NumericResult, Unsupported> {
    if control >> 8 & 3 == 1 {
        return Err(Unsupported::ReservedPrecision);
    }
    if x.is_unsupported() || y.is_unsupported() {
        return Ok(result(Ext80::INDEFINITE, INVALID));
    }
    if x.is_nan() || y.is_nan() {
        return arithmetic(Arithmetic::Add, x, y, control | 0x300);
    }
    let sign = x.sign_exponent & 0x8000;
    let y_negative = y.sign_exponent & 0x8000 != 0;
    let x_infinite = x.exponent() == 0x7fff;
    let y_infinite = y.exponent() == 0x7fff;
    if y_infinite && (x_infinite && y_negative || x.significand == 0 && !y_negative) {
        return Ok(result(Ext80::INDEFINITE, INVALID));
    }
    let denormal = if x.is_denormal() || y.is_denormal() {
        DENORMAL
    } else {
        0
    };
    if denormal & !control != 0 {
        return Ok(result(Ext80::INDEFINITE, denormal));
    }
    let x = x.canonical();
    let y = y.canonical();
    if x_infinite || x.significand == 0 {
        return Ok(result(x, denormal));
    }
    if y_infinite {
        return Ok(result(
            if y_negative {
                Ext80 {
                    sign_exponent: sign,
                    ..Ext80::ZERO
                }
            } else {
                infinity(sign)
            },
            denormal,
        ));
    }
    let y_exponent = i32::from(y.exponent()) - 16383;
    // Beyond +/-65536 even the most extreme ext80 operand, adjusted for an
    // unmasked range exception, still overflows/underflows. No huge cast or
    // modulo shift is used to manufacture an incorrect finite exponent.
    let magnitude = if y_exponent < 0 {
        0
    } else if y_exponent >= 16 {
        65536
    } else {
        (y.significand >> (63 - y_exponent)) as i32
    };
    let shift = x.significand.leading_zeros();
    let significand = x.significand << shift;
    let mut exponent = i32::from(x.exponent().max(1)) - shift as i32
        + if y_negative { -magnitude } else { magnitude };
    let mut exceptions = denormal;
    if exponent < 1 && control & UNDERFLOW == 0 {
        exceptions |= UNDERFLOW;
        exponent += 24576;
        if exponent < 1 {
            return Err(Unsupported::UnmaskedRangeException);
        }
    } else if exponent >= 0x7fff && control & OVERFLOW == 0 {
        exceptions |= OVERFLOW;
        exponent -= 24576;
        if exponent >= 0x7fff {
            return Err(Unsupported::UnmaskedRangeException);
        }
    }
    let rc = control >> 10 & 3;
    if exponent >= 0x7fff {
        let round_up = rc == 0 || rc == 1 && sign != 0 || rc == 2 && sign == 0;
        return Ok(NumericResult {
            value: if round_up {
                infinity(sign)
            } else {
                Ext80 {
                    significand: u64::MAX,
                    sign_exponent: sign | 0x7ffe,
                }
            },
            exceptions: exceptions | OVERFLOW | PRECISION,
            round_up,
            bits: 0,
        });
    }
    if exponent > 0 {
        return Ok(result(
            Ext80 {
                significand,
                sign_exponent: sign | exponent as u16,
            },
            exceptions,
        ));
    }
    let distance = (1 - exponent) as u32;
    let retained = if distance < 64 {
        significand >> distance
    } else {
        0
    };
    let lost = distance >= 64 || significand & (u64::MAX >> (64 - distance)) != 0;
    let half = distance <= 64 && significand & (1u64 << (distance - 1)) != 0;
    let lower = distance > 1 && distance <= 64 && significand & (u64::MAX >> (65 - distance)) != 0;
    let round_up = lost
        && match rc {
            0 => half && (lower || retained & 1 != 0),
            1 => sign != 0,
            2 => sign == 0,
            _ => false,
        };
    let rounded = retained + u64::from(round_up);
    // Tininess is tested after full64 rounding with an unbounded exponent,
    // before the narrower subnormal encoding can round up to minimum normal.
    if lost {
        exceptions |= PRECISION | UNDERFLOW;
    }
    Ok(NumericResult {
        value: Ext80 {
            significand: rounded,
            sign_exponent: sign | (rounded >> 63) as u16,
        },
        exceptions,
        round_up,
        bits: 0,
    })
}

#[cfg(test)]
#[path = "exponential_tests.rs"]
mod tests;

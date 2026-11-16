//! Combined FYL2X numeric semantics. Classification precedes logarithm and
//! multiplication: neither intermediate IEEE exceptions nor intermediate
//! rounding are guest-visible.
use crate::{
    ARITHMETIC_LOCK, Arithmetic, DENORMAL, Ext80, INVALID, NumericResult, PRECISION, UNDERFLOW,
    Unsupported, ZERO_DIVIDE, arithmetic,
};

unsafe extern "C" {
    fn vine_log2_multiply(
        x_sig: u64,
        x_exp: u32,
        y_sig: u64,
        y_exp: u32,
        rounding: u32,
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
fn infinity(negative: bool) -> Ext80 {
    Ext80 {
        significand: 1 << 63,
        sign_exponent: 0x7fff | (u16::from(negative) << 15),
    }
}

/// ST(1) * log2(ST(0)). PC does not truncate transcendental results; RC still
/// selects final extended rounding. Stack and deferred exception handling are
/// owned by `State` or the compiler's explicit FP IR, outside this pure kernel.
pub fn log2_multiply(x: Ext80, y: Ext80, control: u16) -> Result<NumericResult, Unsupported> {
    // Keep the existing explicit reserved-control diagnostic. Defined PCs all
    // use the full 64-bit significand, including exact power-of-two cases.
    if control >> 8 & 3 == 1 {
        return Err(Unsupported::ReservedPrecision);
    }
    if x.is_unsupported() || y.is_unsupported() {
        return Ok(result(Ext80::INDEFINITE, INVALID));
    }
    if x.is_nan() || y.is_nan() {
        // The pinned 8086 specialization implements x87 payload selection and
        // signaling-NaN invalid priority. No finite arithmetic occurs here.
        return arithmetic(Arithmetic::Add, x, y, control | 0x300);
    }
    let x_zero = x.significand == 0;
    let y_zero = y.significand == 0;
    let x_infinite = x.exponent() == 0x7fff;
    let y_infinite = y.exponent() == 0x7fff;
    let denormal = if x.is_denormal() || y.is_denormal() {
        DENORMAL
    } else {
        0
    };
    let x = x.canonical();
    let y = y.canonical();
    let x_one = x == Ext80::ONE;
    if x.sign_exponent & 0x8000 != 0 && !x_zero
        || y_zero && (x_zero || x_infinite)
        || x_one && y_infinite
    {
        return Ok(result(Ext80::INDEFINITE, INVALID));
    }
    let log_negative = !x_infinite && x.exponent() < 0x3fff;
    let negative = (y.sign_exponent & 0x8000 != 0) ^ log_negative;
    if x_zero {
        return Ok(result(
            infinity(negative),
            if y_infinite { 0 } else { ZERO_DIVIDE },
        ));
    }
    if denormal & !control != 0 {
        // A pre-computation exception suppresses the result. Do not let a
        // later interval-certification failure replace this deferred #D.
        return Ok(result(Ext80::INDEFINITE, denormal));
    }
    if x_infinite || y_infinite {
        return Ok(result(infinity(negative), denormal));
    }
    if x_one || y_zero {
        return Ok(result(
            Ext80 {
                significand: 0,
                sign_exponent: u16::from(negative) << 15,
            },
            denormal,
        ));
    }
    if x.significand.is_power_of_two() {
        // A mathematical identity for every binary exponent, not a value
        // specialization or a dependence on the reached executable.
        let shift = x.significand.leading_zeros();
        let exponent = i32::from(x.exponent().max(1)) - 16383 - shift as i32;
        let integer = crate::load(crate::Format::Integer32, exponent as u32 as u64, 0x37f)?;
        let mut answer = arithmetic(Arithmetic::Multiply, y, integer.value, control | 0x300)?;
        // x87 reports precision for the ordinary transcendental path even
        // when log2(x) and the product happen to be exactly representable.
        // The zero/one identities above are the special no-PE cases. This is
        // independently exercised with PM masked/unmasked, including the full
        // 64-bit result when PC requests 24 or 53 significand bits.
        answer.exceptions |= denormal | PRECISION;
        if answer.value.exponent() == 0 {
            // FYL2X's ordinary path also reports UE for an exactly
            // representable subnormal result, alongside its precision flag.
            answer.exceptions |= UNDERFLOW;
        }
        return Ok(answer);
    }
    let _guard = ARITHMETIC_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let mut out = [0u64; 4];
    // SAFETY: nonzero finite operands have been classified, x is positive and
    // not a power of two, RC is bounded, and four output words are writable.
    let certified = unsafe {
        vine_log2_multiply(
            x.significand,
            u32::from(x.sign_exponent),
            y.significand,
            u32::from(y.sign_exponent),
            u32::from(control >> 10 & 3),
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
        bits: 0,
        exceptions: out[3] as u16 | denormal,
        round_up: out[3] >> 16 != 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_transcendental_path_retains_full_precision_and_reports_precision() {
        let two = Ext80 {
            sign_exponent: 0x4000,
            ..Ext80::ONE
        };
        let low_bit = Ext80 {
            significand: 0x8000_0000_0000_0001,
            ..Ext80::ONE
        };
        for pc in [0, 2, 3] {
            for rc in 0..4 {
                for pm in [0, PRECISION] {
                    let control = (0x7f | pc << 8 | rc << 10) & !pm;
                    let actual = log2_multiply(two, low_bit, control).unwrap();
                    assert_eq!(actual.value, low_bit);
                    assert_eq!(actual.exceptions, PRECISION);
                    assert!(!actual.round_up);
                }
            }
        }
        let tiny = Ext80 {
            significand: 1,
            sign_exponent: 0,
        };
        let actual = log2_multiply(two, tiny, 0x37f).unwrap();
        assert_eq!(actual.value, tiny);
        assert_eq!(actual.exceptions, DENORMAL | UNDERFLOW | PRECISION);
    }

    #[test]
    fn special_cases_follow_combined_exception_priority_before_denormal() {
        let tiny = Ext80 {
            significand: 1,
            sign_exponent: 0,
        };
        let pseudo = Ext80 {
            significand: 1 << 63,
            sign_exponent: 0,
        };
        let negative = Ext80 {
            sign_exponent: 0xbfff,
            ..Ext80::ONE
        };
        for sign in [false, true] {
            let inf = infinity(sign);
            let actual = log2_multiply(Ext80::ZERO, inf, 0x37b).unwrap();
            assert_eq!(actual.value, infinity(!sign));
            assert_eq!(actual.exceptions, 0);
        }
        assert_eq!(
            log2_multiply(Ext80::ZERO, tiny, 0x37f).unwrap().exceptions,
            ZERO_DIVIDE
        );
        assert_eq!(
            log2_multiply(negative, tiny, 0x37f).unwrap().exceptions,
            INVALID
        );
        assert_eq!(
            log2_multiply(negative, Ext80::INDEFINITE, 0x37f)
                .unwrap()
                .exceptions,
            0
        );
        for denormal in [tiny, pseudo] {
            assert_eq!(
                log2_multiply(Ext80::ONE, denormal, 0x37f)
                    .unwrap()
                    .exceptions,
                DENORMAL
            );
            assert_eq!(
                log2_multiply(infinity(false), denormal, 0x37d)
                    .unwrap()
                    .exceptions,
                DENORMAL
            );
            assert_eq!(
                log2_multiply(denormal, Ext80::ONE, 0x37d)
                    .unwrap()
                    .exceptions,
                DENORMAL
            );
        }
    }

    #[test]
    fn adjacent_to_one_is_certified_without_binary64_cancellation() {
        // Independently bounded by exact rational atanh series. The host's
        // approximate FYL2X is one ulp below this RU result on the test machine.
        let above = Ext80 {
            significand: 0x8000_0000_0000_0001,
            ..Ext80::ONE
        };
        let expected = Ext80 {
            significand: 0xb8aa_3b29_5c17_f0bc,
            sign_exponent: 0x3fc0,
        };
        let actual = log2_multiply(above, Ext80::ONE, 0xb7f).unwrap();
        assert_eq!(actual.value, expected);
        assert_eq!(actual.exceptions, PRECISION);
        assert!(actual.round_up);
        for pc in [0, 2, 3] {
            assert_eq!(
                log2_multiply(above, Ext80::ONE, 0x87f | pc << 8).unwrap(),
                actual
            );
        }
    }
}

//! Pure FPATAN numeric semantics: atan2(y, x), with full ext80 output.
use crate::{
    ARITHMETIC_LOCK, Arithmetic, DENORMAL, Ext80, INVALID, NumericResult, Unsupported, arithmetic,
};

unsafe extern "C" {
    fn vine_arctangent(
        x_sig: u64,
        x_exp: u32,
        y_sig: u64,
        y_exp: u32,
        rounding: u32,
        out: *mut u64,
    ) -> u32;
}

/// PC does not truncate the transcendental result. RC controls its rounding.
/// Stack effects, pending exceptions and commit are outside this pure function.
pub fn arctangent(x: Ext80, y: Ext80, control: u16) -> Result<NumericResult, Unsupported> {
    if control >> 8 & 3 == 1 {
        return Err(Unsupported::ReservedPrecision);
    }
    if x.is_unsupported() || y.is_unsupported() {
        return Ok(NumericResult {
            value: Ext80::INDEFINITE,
            exceptions: INVALID,
            ..NumericResult::default()
        });
    }
    if x.is_nan() || y.is_nan() {
        return arithmetic(Arithmetic::Add, x, y, control | 0x300);
    }
    let denormal = if x.is_denormal() || y.is_denormal() {
        DENORMAL
    } else {
        0
    };
    if denormal & !control != 0 {
        return Ok(NumericResult {
            value: Ext80::INDEFINITE,
            exceptions: denormal,
            ..NumericResult::default()
        });
    }
    let x = x.canonical();
    let y = y.canonical();
    if x.sign_exponent & 0x8000 == 0
        && (y.significand == 0 || x.exponent() == 0x7fff && y.exponent() != 0x7fff)
    {
        return Ok(NumericResult {
            value: Ext80 {
                significand: 0,
                sign_exponent: y.sign_exponent & 0x8000,
            },
            exceptions: denormal,
            ..NumericResult::default()
        });
    }
    let _guard = ARITHMETIC_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let mut out = [0u64; 4];
    // SAFETY: NaNs/unsupported and zero-result cases were classified; RC is
    // bounded, and the four-word output is valid and private to this call.
    let certified = unsafe {
        vine_arctangent(
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
        exceptions: out[3] as u16 | denormal,
        round_up: out[3] >> 16 != 0,
        bits: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PRECISION, UNDERFLOW};

    #[test]
    fn equal_inputs_use_full_precision_pi_quarter_and_sign() {
        for pc in [0, 2, 3] {
            for rc in 0..4 {
                for negative in [false, true] {
                    let y = Ext80 {
                        sign_exponent: 0x3fff | if negative { 0x8000 } else { 0 },
                        ..Ext80::ONE
                    };
                    let value = arctangent(Ext80::ONE, y, 0x7f | pc << 8 | rc << 10).unwrap();
                    let increment = rc == 0 || rc == if negative { 1 } else { 2 };
                    assert_eq!(
                        value.value,
                        Ext80 {
                            significand: 0xc90f_daa2_2168_c234 + u64::from(increment),
                            sign_exponent: 0x3ffe | if negative { 0x8000 } else { 0 }
                        }
                    );
                    assert_eq!(value.exceptions, PRECISION);
                    assert_eq!(value.round_up, increment);
                }
            }
        }
    }

    #[test]
    fn tiny_ratio_keeps_exponent_and_strict_rounding_limits() {
        let y = Ext80 {
            significand: 1 << 63,
            sign_exponent: 0x3fff - 1000,
        };
        for rc in 0..4 {
            let value = arctangent(Ext80::ONE, y, 0x37f | rc << 10).unwrap();
            let above = rc == 0 || rc == 2;
            assert_eq!(
                value.value,
                if above {
                    y
                } else {
                    Ext80 {
                        significand: u64::MAX,
                        sign_exponent: y.sign_exponent - 1,
                    }
                }
            );
            assert_eq!(value.exceptions, PRECISION);
            assert_eq!(value.round_up, above);
        }
        let min = Ext80 {
            significand: 1,
            sign_exponent: 0,
        };
        let huge = Ext80 {
            significand: u64::MAX,
            sign_exponent: 0x7ffe,
        };
        let rn = arctangent(huge, min, 0x37f).unwrap();
        let ru = arctangent(huge, min, 0xb7f).unwrap();
        assert_eq!(rn.value, Ext80::ZERO);
        assert_eq!(ru.value, min);
        assert_eq!(rn.exceptions, DENORMAL | UNDERFLOW | PRECISION);
        assert_eq!(ru.exceptions, DENORMAL | UNDERFLOW | PRECISION);
    }

    #[test]
    fn exact_quotient_refines_subnormal_rounding_boundaries() {
        let minimum_normal = Ext80 {
            significand: 1 << 63,
            sign_exponent: 1,
        };
        let above_one = Ext80 {
            significand: (1 << 63) + 1,
            sign_exponent: 0x3fff,
        };
        let below_one = Ext80 {
            significand: u64::MAX,
            sign_exponent: 0x3ffe,
        };
        // Exact rational bounds place the first angle just above the largest
        // subnormal and the second just above a midpoint above min-normal.
        // A binary128 division alone cannot resolve these distances.
        for rc in 0..4 {
            let smaller = arctangent(above_one, minimum_normal, 0x37f | rc << 10).unwrap();
            assert_eq!(
                smaller.value,
                if rc == 2 {
                    minimum_normal
                } else {
                    Ext80 {
                        significand: (1 << 63) - 1,
                        sign_exponent: 0,
                    }
                }
            );
            assert_eq!(
                smaller.exceptions,
                PRECISION | if rc == 2 { 0 } else { UNDERFLOW }
            );
            assert_eq!(smaller.round_up, rc == 2);
            let larger = arctangent(below_one, minimum_normal, 0x37f | rc << 10).unwrap();
            let increment = rc == 0 || rc == 2;
            assert_eq!(
                larger.value,
                Ext80 {
                    significand: (1 << 63) + u64::from(increment),
                    sign_exponent: 1
                }
            );
            assert_eq!(larger.exceptions, PRECISION);
            assert_eq!(larger.round_up, increment);
        }
    }

    #[test]
    fn zero_and_infinity_quadrants_do_not_raise_zero_divide_or_invalid() {
        let positive_infinity = Ext80 {
            significand: 1 << 63,
            sign_exponent: 0x7fff,
        };
        for x in [Ext80::ZERO, Ext80::ONE, positive_infinity] {
            for sign in [0, 0x8000] {
                let y = Ext80 {
                    significand: 0,
                    sign_exponent: sign,
                };
                let value = arctangent(x, y, 0x37f).unwrap();
                assert_eq!(value.value, y);
                assert_eq!(value.exceptions, 0);
            }
        }
        let negative_infinity = Ext80 {
            sign_exponent: 0xffff,
            ..positive_infinity
        };
        assert_eq!(
            arctangent(negative_infinity, positive_infinity, 0x37f)
                .unwrap()
                .exceptions,
            PRECISION
        );
        let tiny = Ext80 {
            significand: 1,
            sign_exponent: 0,
        };
        assert_eq!(
            arctangent(tiny, Ext80::ZERO, 0x37f).unwrap().exceptions,
            DENORMAL
        );
        assert_eq!(
            arctangent(tiny, Ext80::ZERO, 0x37d).unwrap().exceptions,
            DENORMAL
        );
    }
}

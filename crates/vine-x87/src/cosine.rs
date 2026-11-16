//! Intel x87 FCOS numeric contract, including its finite internal pi.
use crate::{
    ARITHMETIC_LOCK, Arithmetic, DENORMAL, Ext80, INVALID, NumericResult, Unsupported, arithmetic,
};

unsafe extern "C" {
    fn vine_cosine(significand: u64, exponent: u32, rounding: u32, out: *mut u64) -> u32;
}

/// ST0 stack effects, pending #MF and C2 publication belong to the caller.
/// `bits == 1` denotes an out-of-range finite input: retain ST0 and set C2.
/// For ordinary inputs the numerical target is cos(x*pi/p), where Intel's
/// fixed p is C90FDAA22168C234C * 2^-66, not mathematical cos(x).
pub fn cosine(x: Ext80, control: u16) -> Result<NumericResult, Unsupported> {
    if control >> 8 & 3 == 1 {
        return Err(Unsupported::ReservedPrecision);
    }
    if x.is_unsupported() || x.exponent() == 0x7fff && !x.is_nan() {
        return Ok(NumericResult {
            value: Ext80::INDEFINITE,
            exceptions: INVALID,
            ..NumericResult::default()
        });
    }
    if x.is_nan() {
        return arithmetic(Arithmetic::Add, x, x, control | 0x300);
    }
    if x.exponent() >= 0x403e {
        return Ok(NumericResult {
            value: x,
            bits: 1,
            ..NumericResult::default()
        });
    }
    if x.significand == 0 {
        return Ok(NumericResult {
            value: Ext80::ONE,
            ..NumericResult::default()
        });
    }
    let denormal = if x.is_denormal() { DENORMAL } else { 0 };
    if denormal & !control != 0 {
        return Ok(NumericResult {
            value: Ext80::INDEFINITE,
            exceptions: denormal,
            ..NumericResult::default()
        });
    }
    let x = x.canonical();
    let _guard = ARITHMETIC_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let mut out = [0u64; 4];
    // SAFETY: finite nonzero input below2^63 and bounded RC were classified;
    // four output words are private and valid for the complete synchronous call.
    let certified = unsafe {
        vine_cosine(
            x.significand,
            u32::from(x.exponent()),
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
    use crate::PRECISION;
    #[test]
    fn zero_tiny_range_and_classification_are_not_host_float() {
        for pc in [0, 2, 3] {
            for rc in 0..4 {
                let control = 0x7f | pc << 8 | rc << 10;
                for sign in [0, 0x8000] {
                    assert_eq!(
                        cosine(
                            Ext80 {
                                sign_exponent: sign,
                                ..Ext80::ZERO
                            },
                            control
                        )
                        .unwrap(),
                        NumericResult {
                            value: Ext80::ONE,
                            ..NumericResult::default()
                        }
                    );
                    let tiny = cosine(
                        Ext80 {
                            significand: 1,
                            sign_exponent: sign,
                        },
                        control,
                    )
                    .unwrap();
                    let up = rc == 0 || rc == 2;
                    assert_eq!(
                        tiny.value,
                        if up {
                            Ext80::ONE
                        } else {
                            Ext80 {
                                significand: u64::MAX,
                                sign_exponent: 0x3ffe,
                            }
                        }
                    );
                    assert_eq!(tiny.exceptions, DENORMAL | PRECISION);
                    assert_eq!(tiny.round_up, up);
                    let large = Ext80 {
                        significand: 1 << 63,
                        sign_exponent: sign | 0x403e,
                    };
                    assert_eq!(
                        cosine(large, control).unwrap(),
                        NumericResult {
                            value: large,
                            bits: 1,
                            ..NumericResult::default()
                        }
                    );
                }
            }
        }
        assert_eq!(
            cosine(
                Ext80 {
                    significand: 1,
                    sign_exponent: 0
                },
                0x37d
            )
            .unwrap()
            .exceptions,
            DENORMAL
        );
        assert_eq!(
            cosine(
                Ext80 {
                    significand: 1 << 63,
                    sign_exponent: 0x7fff
                },
                0x37f
            )
            .unwrap()
            .exceptions,
            INVALID
        );
        assert_eq!(
            cosine(
                Ext80 {
                    significand: 1,
                    sign_exponent: 0x4000
                },
                0x37f
            )
            .unwrap()
            .exceptions,
            INVALID
        );
    }
    #[test]
    fn ordinary_cosine_is_even_full_precision_and_certified() {
        for exponent in [0x3fdf, 0x3ffd, 0x3ffe, 0x3fff, 0x4000, 0x4005, 0x403d] {
            for significand in [1 << 63, 0xc90f_daa2_2168_c235, u64::MAX] {
                for rc in 0..4 {
                    let x = Ext80 {
                        significand,
                        sign_exponent: exponent,
                    };
                    let expected = cosine(x, 0x37f | rc << 10)
                        .unwrap_or_else(|error| panic!("{x:?} rc={rc}: {error:?}"));
                    assert_eq!(expected.bits, 0);
                    assert_eq!(expected.exceptions, PRECISION);
                    for pc in [0, 2, 3] {
                        assert_eq!(cosine(x, 0x7f | pc << 8 | rc << 10).unwrap(), expected);
                        assert_eq!(
                            cosine(
                                Ext80 {
                                    sign_exponent: exponent | 0x8000,
                                    ..x
                                },
                                0x7f | pc << 8 | rc << 10
                            )
                            .unwrap(),
                            expected
                        );
                    }
                }
            }
        }
    }
}

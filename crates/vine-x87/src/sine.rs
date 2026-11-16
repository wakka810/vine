//! Intel's rounded-period FSIN target with full x87 exponent range.
use crate::{
    ARITHMETIC_LOCK, Arithmetic, DENORMAL, Ext80, INVALID, NumericResult, UNDERFLOW, Unsupported,
    arithmetic,
};

unsafe extern "C" {
    fn vine_sine(
        significand: u64,
        sign_exponent: u32,
        rounding: u32,
        underflow_unmasked: u32,
        out: *mut u64,
    ) -> u32;
}

/// Correctly rounded full64 sin(x*pi/p), with p=C90FDAA22168C234C*2^-66.
/// `bits == 1` is finite range failure: the caller retains ST0 and sets C2.
/// Unmasked UE returns the exponent-biased register result, not a suppressed
/// destination. The caller owns stack/flags, location and deferred #MF.
pub fn sine(x: Ext80, control: u16) -> Result<NumericResult, Unsupported> {
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
            value: x,
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
    // SAFETY: Rust classified the finite, nonzero operand and bounded RC;
    // the synchronous call owns all four output words under the SoftFloat lock.
    let certified = unsafe {
        vine_sine(
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
        bits: 0,
        exceptions: out[3] as u16 | denormal,
        round_up: out[3] >> 16 != 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PRECISION;

    #[test]
    fn signed_zero_range_and_exception_priority_are_explicit() {
        for pc in [0, 2, 3] {
            for rc in 0..4 {
                let control = 0x7f | pc << 8 | rc << 10;
                for sign in [0, 0x8000] {
                    let zero = Ext80 {
                        sign_exponent: sign,
                        ..Ext80::ZERO
                    };
                    assert_eq!(
                        sine(zero, control).unwrap(),
                        NumericResult {
                            value: zero,
                            ..NumericResult::default()
                        }
                    );
                    let huge = Ext80 {
                        significand: 1 << 63,
                        sign_exponent: sign | 0x403e,
                    };
                    assert_eq!(
                        sine(huge, control & !0x3f).unwrap(),
                        NumericResult {
                            value: huge,
                            bits: 1,
                            ..NumericResult::default()
                        }
                    );
                }
            }
        }
        let tiny = Ext80 {
            significand: 1,
            sign_exponent: 0,
        };
        assert_eq!(sine(tiny, 0x34d).unwrap().exceptions, DENORMAL);
        assert_eq!(
            sine(
                Ext80 {
                    significand: 1 << 63,
                    sign_exponent: 0xffff
                },
                0x37e
            )
            .unwrap()
            .exceptions,
            INVALID
        );
        let snan = Ext80 {
            significand: 0x8000_0000_0000_1234,
            sign_exponent: 0xffff,
        };
        let quiet = sine(snan, 0x37f).unwrap();
        assert_eq!(
            quiet.value,
            Ext80 {
                significand: snan.significand | 1 << 62,
                ..snan
            }
        );
        assert_eq!(quiet.exceptions, INVALID);
        assert_eq!(sine(Ext80::ONE, 0x17f), Err(Unsupported::ReservedPrecision));
    }

    #[test]
    fn signed_ordinary_rounding_and_scaled_tiny_c1_are_distinct() {
        // Exact-rational interval certificates, not host transcendental values.
        for pc in [0, 2, 3] {
            for rc in 0..4 {
                let control = 0x7f | pc << 8 | rc << 10;
                for sign in [0, 0x8000] {
                    let up = rc == 0 || rc == if sign == 0 { 2 } else { 1 };
                    let r = sine(
                        Ext80 {
                            sign_exponent: 0x3fff | sign,
                            ..Ext80::ONE
                        },
                        control,
                    )
                    .unwrap();
                    assert_eq!(
                        r.value,
                        Ext80 {
                            significand: 0xd76a_a478_4867_7020 + u64::from(up),
                            sign_exponent: 0x3ffe | sign
                        }
                    );
                    assert_eq!(r.round_up, up);
                    assert_eq!(r.exceptions, PRECISION);
                    // At 2^-32 the sine correction exceeds the pi/p increment;
                    // at 2^-64 and below their ordering reverses. Retaining an
                    // exact copy of x in either path would give incorrect C1.
                    for exponent in [0x3fbf, 0x3fbe] {
                        let up = rc == if sign == 0 { 2 } else { 1 };
                        let r = sine(
                            Ext80 {
                                significand: 1 << 63,
                                sign_exponent: exponent | sign,
                            },
                            control,
                        )
                        .unwrap();
                        assert_eq!(
                            r.value,
                            Ext80 {
                                significand: (1 << 63) + u64::from(up),
                                sign_exponent: exponent | sign
                            }
                        );
                        assert_eq!(r.round_up, up);
                    }
                }
                let r = sine(
                    Ext80 {
                        significand: 1 << 63,
                        sign_exponent: 0x3fdf,
                    },
                    control,
                )
                .unwrap();
                let up = rc == 0 || rc == 2;
                assert_eq!(r.round_up, up);
                assert_eq!(
                    r.value,
                    if up {
                        Ext80 {
                            significand: 1 << 63,
                            sign_exponent: 0x3fdf,
                        }
                    } else {
                        Ext80 {
                            significand: u64::MAX,
                            sign_exponent: 0x3fde,
                        }
                    }
                );
            }
        }
    }

    #[test]
    fn tininess_uses_unbounded_rounding_and_unmasked_result_retains_all_bits() {
        for sign in [0, 0x8000] {
            for rc in 0..4 {
                for pc in [0, 2, 3] {
                    let control = 0x7f | pc << 8 | rc << 10;
                    let up = rc == if sign == 0 { 2 } else { 1 };
                    for (sig, exponent, biased_sig, biased_exp) in [
                        (1, 0, (1 << 63) + u64::from(up), 0x5fc2),
                        ((1 << 63) - 1, 0, u64::MAX - u64::from(!up), 0x6000),
                    ] {
                        let input = Ext80 {
                            significand: sig,
                            sign_exponent: exponent | sign,
                        };
                        let masked = sine(input, control).unwrap();
                        assert_eq!(masked.exceptions, DENORMAL | UNDERFLOW | PRECISION);
                        assert_eq!(masked.round_up, up);
                        if sig != 1 && up {
                            assert_eq!(
                                masked.value,
                                Ext80 {
                                    significand: 1 << 63,
                                    sign_exponent: 1 | sign
                                }
                            );
                            assert_ne!(
                                masked.exceptions & UNDERFLOW,
                                0,
                                "normal stored encoding does not imply non-tiny unbounded rounding"
                            );
                        }
                        let unmasked = sine(input, control & !UNDERFLOW).unwrap();
                        assert_eq!(
                            unmasked.value,
                            Ext80 {
                                significand: biased_sig,
                                sign_exponent: biased_exp | sign
                            }
                        );
                        assert_eq!(unmasked.exceptions, masked.exceptions);
                        assert_eq!(unmasked.round_up, up);
                    }
                }
            }
        }
    }
}

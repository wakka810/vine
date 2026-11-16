use super::*;

#[cfg(target_arch = "x86_64")]
unsafe extern "C" {
    fn vine_x87_native(op: u32, a: u64, ae: u32, b: u64, be: u32, cw: u32, out: *mut u64);
}
#[cfg(target_arch = "x86_64")]
fn native(op: u32, a: Ext80, b: Ext80, cw: u16) -> NumericResult {
    let _guard = ARITHMETIC_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let mut out = [0u64; 4];
    // SAFETY: test-only host instruction probe saves/restores host FP state;
    // all operands are raw values, with a live owned four-word output.
    unsafe {
        vine_x87_native(
            op,
            a.significand,
            u32::from(a.sign_exponent),
            b.significand,
            u32::from(b.sign_exponent),
            u32::from(cw),
            out.as_mut_ptr(),
        );
    }
    NumericResult {
        value: Ext80 {
            significand: out[0],
            sign_exponent: out[1] as u16,
        },
        exceptions: out[3] as u16 & 0x3f,
        round_up: out[3] & 0x200 != 0,
        bits: 0,
    }
}
fn raw(sig: u64, exp: u16) -> Ext80 {
    Ext80 {
        significand: sig,
        sign_exponent: exp,
    }
}

#[test]
#[cfg(target_arch = "x86_64")]
fn scale_matches_native_exact_values_rounding_range_and_pre_exception_priority() {
    let mut values = vec![
        Ext80::ZERO,
        Ext80::ONE,
        Ext80::INDEFINITE,
        raw(1, 0),
        raw(1 << 63, 0),
        raw((1 << 63) - 1, 0),
        raw(1 << 63, 1),
        raw((1 << 63) + 1, 1),
        raw(u64::MAX, 1),
        raw(u64::MAX, 0x7ffe),
        raw(1 << 63, 0x7fff),
        raw(0x8000_0000_0000_0123, 0x7fff),
        raw(0x4000_0000_0000_0000, 0x3fff),
    ];
    let mut random = 0x8c16_75be_92d3_1af7u64;
    for exponent in [0, 1, 2, 0x3ffe, 0x3fff, 0x4000, 0x7ffd, 0x7ffe] {
        for _ in 0..8 {
            random ^= random << 13;
            random ^= random >> 7;
            random ^= random << 17;
            values.push(raw(
                if exponent == 0 {
                    random >> 1
                } else {
                    random | 1 << 63
                },
                exponent,
            ));
        }
    }
    let mut scales = vec![
        Ext80::ZERO,
        Ext80::INDEFINITE,
        raw(1, 0),
        raw(1 << 63, 0),
        raw(0x8000_0000_0000_0123, 0x7fff),
        raw(1 << 63, 0x7fff),
        raw(u64::MAX, 0x7ffe),
    ];
    for n in [
        1u32, 2, 63, 64, 65, 16382, 16383, 24576, 32767, 65535, 65536,
    ] {
        let bit = 31 - n.leading_zeros();
        scales.push(raw(u64::from(n) << (63 - bit), (16383 + bit) as u16));
    }
    scales.extend([raw(0xc000_0000_0000_0000, 0x3ffe), raw(u64::MAX, 0x3fff)]);
    for pc in [0, 2, 3] {
        for rc in 0..4 {
            for unmask in [0, 2, 8, 16, 24] {
                let cw = (0x7f | pc << 8 | rc << 10) & !unmask;
                for &a in &values {
                    for &b in &scales {
                        for sign in [0, 0x8000] {
                            let a = raw(a.significand, a.sign_exponent ^ sign);
                            let b = raw(b.significand, b.sign_exponent ^ sign);
                            let computed = scale(a, b, cw);
                            if computed == Err(Unsupported::UnmaskedRangeException) {
                                // This is a documented, pre-publication unsupported boundary,
                                // not an accepted approximation of the native default result.
                                assert_ne!(unmask & (OVERFLOW | UNDERFLOW), 0);
                                assert!(!a.is_nan() && !b.is_nan());
                                continue;
                            }
                            let actual = computed.unwrap();
                            let expected = native(7, a, b, cw);
                            assert_eq!(
                                (actual.exceptions, actual.round_up),
                                (expected.exceptions, expected.round_up),
                                "a={a:?} b={b:?} cw={cw:x} actual={actual:?} expected={expected:?}"
                            );
                            if actual.exceptions & !cw & 7 == 0 {
                                assert_eq!(
                                    actual.value, expected.value,
                                    "a={a:?} b={b:?} cw={cw:x}"
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn exponential_specials_domain_and_full_precision_are_explicit() {
    for pc in [0, 2, 3] {
        for rc in 0..4 {
            let cw = 0x7f | pc << 8 | rc << 10;
            for sign in [0, 0x8000] {
                let zero = raw(0, sign);
                assert_eq!(exponential_minus_one(zero, cw).unwrap(), result(zero, 0));
                assert_eq!(
                    exponential_minus_one(raw(1 << 63, sign | 0x3fff), cw).unwrap(),
                    result(
                        raw(1 << 63, if sign == 0 { 0x3fff } else { 0xbffe }),
                        PRECISION
                    )
                );
                assert_eq!(
                    exponential_minus_one(raw(1 << 63, sign | 0x4000), cw),
                    Err(Unsupported::TranscendentalDomain)
                );
                assert_eq!(
                    exponential_minus_one(raw(1 << 63, sign | 0x7fff), cw).unwrap(),
                    result(raw(1 << 63, if sign == 0 { 0x7fff } else { 0xbfff }), 0)
                );
            }
            assert_eq!(
                exponential_minus_one(raw(1, 0), cw & !2)
                    .unwrap()
                    .exceptions,
                DENORMAL
            );
            assert_eq!(
                exponential_minus_one(raw(1, 0x3fff), cw).unwrap(),
                result(Ext80::INDEFINITE, INVALID)
            );
            assert_eq!(
                exponential_minus_one(raw(0x8000_0000_0000_0123, 0x7fff), cw).unwrap(),
                result(raw(0xc000_0000_0000_0123, 0x7fff), INVALID)
            );
        }
    }
}

#[test]
fn exponential_certified_values_and_tiny_bias_retain_full64() {
    // Independent rational enclosure of ln2 and the exponential series.
    // The actual captured input is deliberately not a product specialization.
    let samples = [
        (
            raw(0x8000_0000_0000_0000, 0x3ffe),
            0x3ffd,
            [
                0xd413_cccf_e779_9211,
                0xd413_cccf_e779_9211,
                0xd413_cccf_e779_9212,
                0xd413_cccf_e779_9211,
            ],
        ),
        (
            raw(0x8000_0000_0000_0000, 0xbffe),
            0xbffd,
            [
                0x95f6_1998_0c43_36f7,
                0x95f6_1998_0c43_36f8,
                0x95f6_1998_0c43_36f7,
                0x95f6_1998_0c43_36f7,
            ],
        ),
    ];
    for pc in [0, 2, 3] {
        for rc in 0..4 {
            let cw = 0x7f | pc << 8 | rc << 10;
            for &(input, exp, sigs) in &samples {
                let actual = exponential_minus_one(input, cw).unwrap();
                assert_eq!(actual.value, raw(sigs[rc as usize], exp));
                assert_eq!(actual.exceptions, PRECISION);
                assert_eq!(actual.round_up, sigs[rc as usize] != sigs[3]);
            }
            for sign in [0, 0x8000] {
                let up = rc == 0 || rc == if sign == 0 { 2 } else { 1 };
                let actual = exponential_minus_one(raw(1, sign), cw).unwrap();
                assert_eq!(actual.value, raw(u64::from(up), sign));
                assert_eq!(actual.exceptions, DENORMAL | UNDERFLOW | PRECISION);
                assert_eq!(actual.round_up, up);
                let biased = exponential_minus_one(raw(1, sign), cw & !UNDERFLOW).unwrap();
                assert_eq!(biased.value.sign_exponent, sign | 0x5fc1);
                assert_eq!(biased.exceptions, DENORMAL | UNDERFLOW | PRECISION);
                assert_eq!(
                    biased.value.significand,
                    if rc == 1 && sign == 0 || rc == 2 && sign != 0 || rc == 3 {
                        0xb172_17f7_d1cf_79ab
                    } else {
                        0xb172_17f7_d1cf_79ac
                    }
                );
            }
        }
    }
}

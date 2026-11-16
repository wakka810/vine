use super::*;

#[test]
fn double_rounding_zero_nan_and_tiny_contracts() {
    let one = 0x3ff0_0000_0000_0000;
    for op in [add_f64, subtract_f64, multiply_f64] {
        assert_eq!(op(0, 0, 0x10000), Err(InvalidMxcsr));
        assert_eq!(
            op(0x7ff8_0000_0000_0123, 0xfff0_0000_0000_0456, 0x1f80).unwrap(),
            Float64Result {
                bits: 0x7ff8_0000_0000_0123,
                exceptions: IE
            }
        );
        assert_eq!(
            op(0, 0xfff0_0000_0000_0123, 0x1f80).unwrap(),
            Float64Result {
                bits: 0xfff8_0000_0000_0123,
                exceptions: IE
            }
        );
    }
    for rc in 0..4 {
        let control = 0x1f80 | rc << 13;
        assert_eq!(
            add_f64(one, 0x3ca0_0000_0000_0000, control).unwrap(),
            Float64Result {
                bits: one + u64::from(rc == 2),
                exceptions: PE
            }
        );
        assert_eq!(
            subtract_f64(one, 1, control).unwrap(),
            Float64Result {
                bits: one - u64::from(rc == 1 || rc == 3),
                exceptions: DE | PE
            }
        );
        assert_eq!(
            subtract_f64(one, one, control).unwrap().bits,
            if rc == 1 { SIGN } else { 0 }
        );
        assert_eq!(add_f64(SIGN, SIGN, control).unwrap().bits, SIGN);
        for sign in [0, SIGN] {
            let a = sign | 0x0010_0000_0000_0001;
            let b = sign | 0x0010_0000_0000_0000;
            assert_eq!(
                subtract_f64(a, b, control).unwrap(),
                Float64Result {
                    bits: sign | 1,
                    exceptions: 0
                }
            );
            assert_eq!(
                subtract_f64(a, b, control | 0x8000).unwrap(),
                Float64Result {
                    bits: sign,
                    exceptions: UE | PE
                }
            );
            assert_eq!(subtract_f64(a, b, control & !0x800).unwrap().exceptions, UE);
        }
        // 53x53-bit multiplication, exact product retained before subnormal rounding.
        assert_eq!(
            multiply_f64(0x0010_0000_0000_0001, 0x3fe0_0000_0000_0000, control).unwrap(),
            Float64Result {
                bits: 0x0008_0000_0000_0000 + u64::from(rc == 2),
                exceptions: UE | PE
            }
        );
        assert_eq!(
            multiply_f64(
                0x0010_0000_0000_0001,
                0x3fe0_0000_0000_0000,
                control & !0x800
            )
            .unwrap()
            .exceptions,
            UE
        );
        for (op, b) in [
            (add_f64 as fn(u64, u64, u32) -> _, 0x7fe0_0000_0000_0000),
            (multiply_f64, 0x4000_0000_0000_0000),
        ] {
            assert_eq!(
                op(0x7fe0_0000_0000_0000, b, control).unwrap().exceptions,
                OE | PE
            );
            assert_eq!(
                op(0x7fe0_0000_0000_0000, b, control & !0x400)
                    .unwrap()
                    .exceptions,
                OE
            );
        }
    }
}

#[cfg(target_arch = "x86_64")]
fn assert_native(a: [u64; 2], b: [u64; 2], control: u32, operation: u32) {
    unsafe extern "C" {
        fn vine_sse_native_double(
            a: *const u64,
            b: *const u64,
            control: u32,
            operation: u32,
            out: *mut u32,
        );
    }
    let _guard = crate::ARITHMETIC_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let op = [add_f64, subtract_f64, multiply_f64][operation as usize];
    let results = [
        op(a[0], b[0], control).unwrap(),
        op(a[1], b[1], control).unwrap(),
    ];
    let mut flags = results[0].exceptions | results[1].exceptions;
    if flags & 7 & !(control >> 7) != 0 {
        flags &= 7;
    }
    let fault = flags & !(control >> 7) & 63 != 0;
    let expected = if fault {
        a
    } else {
        [results[0].bits, results[1].bits]
    };
    let mut out = [0; 6];
    // SAFETY: exact-size raw numeric inputs/output; probe saves/restores host FP.
    unsafe {
        vine_sse_native_double(a.as_ptr(), b.as_ptr(), control, operation, out.as_mut_ptr());
    }
    let actual = [
        u64::from(out[0]) | u64::from(out[1]) << 32,
        u64::from(out[2]) | u64::from(out[3]) << 32,
    ];
    assert_eq!(
        actual, expected,
        "op={operation} a={a:x?} b={b:x?} mxcsr={control:x}"
    );
    assert_eq!(
        out[4],
        control | flags,
        "op={operation} a={a:x?} b={b:x?} mxcsr={control:x}"
    );
    assert_ne!(out[5], u32::MAX, "native exception handler unavailable");
    assert_eq!(out[5] != 0, fault);
}

#[test]
#[cfg(target_arch = "x86_64")]
fn double_masked_random_and_edge_values_match_sse2() {
    let edges = [
        0,
        SIGN,
        1,
        SIGN | 1,
        FRAC,
        1 << 52,
        (1 << 52) + 1,
        0x3ff0_0000_0000_0000,
        0x3ff0_0000_0000_0001,
        0x3ca0_0000_0000_0000,
        0x7fe0_0000_0000_0000,
        INF - 1,
        INF,
        SIGN | INF,
        INF | 1,
        INF | QUIET | 123,
        SIGN | INF | 456,
    ];
    let mut random = 0x8721_3fa1_9dab_2337u64;
    for operation in 0..3 {
        for rc in 0..4 {
            for daz_ftz in [0, 0x40, 0x8000, 0x8040] {
                let control = 0x1f80 | rc << 13 | daz_ftz;
                for &a in &edges {
                    for &b in &edges {
                        assert_native([a, b], [b, a], control, operation);
                    }
                }
                for _ in 0..3000 {
                    let mut next = || {
                        random ^= random << 13;
                        random ^= random >> 7;
                        random ^= random << 17;
                        random
                    };
                    let a = [next(), next()];
                    let b = [next(), next()];
                    assert_native(a, b, control, operation);
                }
            }
        }
    }
}

#[test]
#[cfg(all(target_arch = "x86_64", target_os = "windows"))]
fn double_unmasked_stages_suppress_both_lanes_and_preserve_old_flags() {
    let cases = [
        ([1, INF - 1], [0x3ff0_0000_0000_0000, INF - 1]),
        ([INF | 1, 1 << 52], [INF | QUIET | 12, 1]),
        (
            [0x0010_0000_0000_0001, 0x7fe0_0000_0000_0000],
            [0x3fe0_0000_0000_0000, 0x4000_0000_0000_0000],
        ),
        ([1, 0x0010_0000_0000_0001], [0, 0x0010_0000_0000_0000]),
        (
            [0x3ff0_0000_0000_0000, INF],
            [0x3ca0_0000_0000_0000, SIGN | INF],
        ),
    ];
    for operation in 0..3 {
        for rc in 0..4 {
            for daz_ftz in [0, 0x40, 0x8000, 0x8040] {
                for masks in 0..64 {
                    for (a, b) in cases {
                        for sticky in [0, 63] {
                            assert_native(
                                a,
                                b,
                                masks << 7 | rc << 13 | daz_ftz | sticky,
                                operation,
                            );
                        }
                    }
                }
            }
        }
    }
}

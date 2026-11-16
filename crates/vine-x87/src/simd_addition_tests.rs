use super::*;

#[test]
fn reserved_control_nan_sources_and_invalid_infinities_are_explicit() {
    for operation in [add_f32, subtract_f32] {
        for bit in 16..32 {
            assert_eq!(operation(0, 0, 1 << bit), Err(InvalidMxcsr));
        }
        assert_eq!(operation(0, 0, u32::MAX), Err(InvalidMxcsr));
        for (a, b, bits, exceptions) in [
            (0x7fc1_2345, 0xff85_4321, 0x7fc1_2345, IE),
            (0xff85_4321, 0x7fc1_2345, 0xffc5_4321, IE),
            (0xffc5_4321, 0x7fc1_2345, 0xffc5_4321, 0),
            (0xff81_2345, 0x7f85_4321, 0xffc1_2345, IE),
            (0x7fc1_2345, 1, 0x7fc1_2345, 0),
            (1, 0xffc1_2345, 0xffc1_2345, 0),
            (1, 0xff81_2345, 0xffc1_2345, IE),
        ] {
            assert_eq!(
                operation(a, b, 0x1f80).unwrap(),
                Float32Addition { bits, exceptions }
            );
        }
    }
    assert_eq!(
        add_f32(INFINITY, SIGN | INFINITY, 0x1f80).unwrap(),
        Float32Addition {
            bits: 0xffc0_0000,
            exceptions: IE
        }
    );
    assert_eq!(
        subtract_f32(INFINITY, INFINITY, 0x1f80).unwrap(),
        Float32Addition {
            bits: 0xffc0_0000,
            exceptions: IE
        }
    );
    assert_eq!(
        subtract_f32(1, INFINITY, 0x1f80).unwrap(),
        Float32Addition {
            bits: SIGN | INFINITY,
            exceptions: DE
        }
    );
}

#[test]
fn exact_zero_sign_and_tiny_result_depend_on_rounding_and_masks() {
    for rc in 0..4 {
        let control = 0x1f80 | (rc << 13);
        let cancellation = if rc == 1 { SIGN } else { 0 };
        for (a, b) in [(0, SIGN), (SIGN, 0), (0x3f80_0000, 0xbf80_0000)] {
            assert_eq!(add_f32(a, b, control).unwrap().bits, cancellation);
        }
        for (a, b) in [(0, 0), (SIGN, SIGN), (0x3f80_0000, 0x3f80_0000)] {
            assert_eq!(subtract_f32(a, b, control).unwrap().bits, cancellation);
        }
        assert_eq!(add_f32(SIGN, SIGN, control).unwrap().bits, SIGN);
        assert_eq!(add_f32(0, 0, control).unwrap().bits, 0);
        assert_eq!(subtract_f32(0, SIGN, control).unwrap().bits, 0);
        assert_eq!(subtract_f32(SIGN, 0, control).unwrap().bits, SIGN);
        for sign in [0, SIGN] {
            for tiny in [1, 0x0040_0000, 0x007f_ffff] {
                // Both operands are normal; only cancellation creates tiny.
                let a = (0x0080_0000 + tiny) | sign;
                let b = 0x0080_0000 | sign;
                assert_eq!(
                    subtract_f32(a, b, control).unwrap(),
                    Float32Addition {
                        bits: sign | tiny,
                        exceptions: 0
                    }
                );
                assert_eq!(
                    subtract_f32(a, b, control | 0x8000).unwrap(),
                    Float32Addition {
                        bits: sign,
                        exceptions: UE | PE
                    }
                );
                for ftz in [0, 0x8000] {
                    assert_eq!(
                        subtract_f32(a, b, (control & !0x800) | ftz).unwrap(),
                        Float32Addition {
                            bits: sign | tiny,
                            exceptions: UE
                        }
                    );
                }
            }
        }
        assert_eq!(
            add_f32(0x8000_0001, 0, control | 0x40).unwrap(),
            Float32Addition {
                bits: cancellation,
                exceptions: 0
            }
        );
    }
}

#[test]
fn ties_distant_operands_and_overflow_round_the_exact_sum_once() {
    for rc in 0..4 {
        let control = 0x1f80 | (rc << 13);
        assert_eq!(
            add_f32(0x3f80_0000, 0x3380_0000, control).unwrap(),
            Float32Addition {
                bits: 0x3f80_0000 + u32::from(rc == 2),
                exceptions: PE
            }
        );
        assert_eq!(
            add_f32(0x3f80_0001, 0x3380_0000, control).unwrap(),
            Float32Addition {
                bits: 0x3f80_0001 + u32::from(rc == 0 || rc == 2),
                exceptions: PE
            }
        );
        assert_eq!(
            subtract_f32(0x3f80_0000, 1, control).unwrap(),
            Float32Addition {
                bits: 0x3f80_0000 - u32::from(rc == 1 || rc == 3),
                exceptions: DE | PE
            }
        );
        assert_eq!(
            add_f32(0x7f00_0000, 0x7f00_0000, control)
                .unwrap()
                .exceptions,
            OE | PE
        );
        assert_eq!(
            add_f32(0x7f00_0000, 0x7f00_0000, control & !0x400)
                .unwrap()
                .exceptions,
            OE
        );
        let overflow = rc == 0 || rc == 2;
        assert_eq!(
            add_f32(0x7f7f_ffff, 0x7300_0000, control).unwrap(),
            Float32Addition {
                bits: if overflow { INFINITY } else { INFINITY - 1 },
                exceptions: PE | if overflow { OE } else { 0 }
            }
        );
    }
}

#[cfg(target_arch = "x86_64")]
fn assert_native(a: [u32; 4], b: [u32; 4], control: u32, subtract: bool) {
    unsafe extern "C" {
        fn vine_sse_native_add_sub(
            a: *const u32,
            b: *const u32,
            control: u32,
            subtract: u32,
            out: *mut u32,
        );
    }
    let _guard = crate::ARITHMETIC_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let operation = if subtract { subtract_f32 } else { add_f32 };
    let results = std::array::from_fn::<_, 4, _>(|i| operation(a[i], b[i], control).unwrap());
    let mut flags = results
        .iter()
        .fold(0, |flags, result| flags | result.exceptions);
    if flags & 7 & !(control >> 7) != 0 {
        flags &= 7;
    }
    let fault = flags & !(control >> 7) != 0;
    let mut out = [0; 6];
    // SAFETY: initialized input/output arrays; the C oracle restores host FP
    // state and only captures a fault at its known ADDPS/SUBPS PC. The lock
    // serializes its temporary Windows exception observation pointer.
    unsafe {
        vine_sse_native_add_sub(
            a.as_ptr(),
            b.as_ptr(),
            control,
            u32::from(subtract),
            out.as_mut_ptr(),
        );
    }
    assert_ne!(out[5], u32::MAX, "native VEH installation failed");
    assert_eq!(
        out[5] != 0,
        fault,
        "subtract={subtract}, a={a:x?}, b={b:x?}, mxcsr={control:x}, native={out:x?}"
    );
    assert_eq!(
        out[4],
        control | flags,
        "subtract={subtract}, a={a:x?}, b={b:x?}, mxcsr={control:x}, native={out:x?}"
    );
    let expected = if fault {
        a
    } else {
        results.map(|result| result.bits)
    };
    assert_eq!(
        out[..4],
        expected,
        "subtract={subtract}, a={a:x?}, b={b:x?}, mxcsr={control:x}, native={out:x?}"
    );
}

#[cfg(target_arch = "x86_64")]
#[test]
fn masked_packed_add_and_sub_match_native_classes_exponent_gaps_and_roundings() {
    let classes = [
        0,
        SIGN,
        1,
        SIGN | 1,
        0x007f_ffff,
        0x807f_ffff,
        0x0080_0000,
        0x8080_0000,
        0x0080_0001,
        0x00ff_ffff,
        0x3380_0000,
        0x3f00_0000,
        0x3f7f_ffff,
        0x3f80_0000,
        0x3f80_0001,
        0xbf80_0001,
        0x4000_0000,
        0x7300_0000,
        0x7f00_0000,
        0xff00_0000,
        0x7f7f_ffff,
        0xff7f_ffff,
        INFINITY,
        SIGN | INFINITY,
        0x7fc1_2345,
        0xffc5_4321,
        0x7f81_2345,
        0xff85_4321,
    ];
    let mut pairs = Vec::new();
    for a in classes {
        for b in classes {
            pairs.push((a, b));
        }
    }
    for exponent in 1..255 {
        let value = exponent << 23;
        for fraction in [0, 1, FRACTION] {
            for b in [1, value - 1, value, value | 1, 0x7f7f_ffff] {
                pairs.push((value | fraction, b));
                pairs.push((value | fraction | SIGN, b));
            }
        }
    }
    let mut seed = 0x5b94_28a1_u32;
    for _ in 0..4096 {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let a = seed;
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        pairs.push((a, seed));
    }
    for chunk in pairs.chunks(4) {
        let a = std::array::from_fn(|i| chunk[i % chunk.len()].0);
        let b = std::array::from_fn(|i| chunk[i % chunk.len()].1);
        for subtract in [false, true] {
            for rc in 0..4 {
                for daz_ftz in [0, 0x40, 0x8000, 0x8040] {
                    assert_native(a, b, 0x1f80 | (rc << 13) | daz_ftz, subtract);
                }
            }
        }
    }
}

#[cfg(all(target_arch = "x86_64", target_os = "windows"))]
#[test]
fn unmasked_native_context_observes_packed_stage_gate_exact_tiny_and_overflow() {
    let packs = [
        (
            [0x7f81_2345, 1, 0x7f7f_ffff, 0x3f80_0000],
            [0x3f80_0000, 0x3f80_0000, 0x7f7f_ffff, 0x3380_0000],
        ),
        (
            [0x7fc1_2345, INFINITY, 0x0080_0001, 0x3f80_0000],
            [1, SIGN | INFINITY, 0x8080_0000, 0xb380_0000],
        ),
        (
            [1, 0xffc5_4321, 0x0080_0001, 0x7f00_0000],
            [0xff81_2345, 0x7f81_2345, 0x0080_0000, 0xff00_0000],
        ),
    ];
    for (a, b) in packs {
        for subtract in [false, true] {
            for rc in 0..4 {
                for masks in 0..64 {
                    for daz_ftz in [0, 0x40, 0x8000, 0x8040] {
                        assert_native(a, b, (masks << 7) | (rc << 13) | daz_ftz, subtract);
                    }
                }
            }
        }
    }
    for (a, b) in [
        (0x0080_0001, 0x8080_0000),
        (0x0080_0000, 0x807f_ffff),
        (0x7f00_0000, 0x7f00_0000),
        (0x7f7f_ffff, 0x7300_0000),
        (0x7f7f_ffff, 0x72ff_ffff),
        (0x7f7f_ffff, 0x7300_0001),
    ] {
        for sign in [0, SIGN] {
            for subtract in [false, true] {
                for rc in 0..4 {
                    for masks in 0..8 {
                        for ftz in [0, 0x8000] {
                            assert_native(
                                [a ^ sign, 0x3f80_0000, 0, SIGN],
                                [b ^ sign ^ if subtract { SIGN } else { 0 }, 0, 0, 0],
                                0x380 | (masks << 10) | (rc << 13) | ftz,
                                subtract,
                            );
                        }
                    }
                }
            }
        }
    }
    for subtract in [false, true] {
        for rc in 0..4 {
            assert_native(
                [0x3f80_0000, 0xbf80_0000, 0, SIGN],
                [0; 4],
                0x3f | (rc << 13),
                subtract,
            );
        }
    }
}

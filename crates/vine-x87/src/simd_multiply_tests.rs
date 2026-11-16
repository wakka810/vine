use super::*;

#[test]
fn reserved_mxcsr_is_rejected_before_numeric_work() {
    for bit in 16..32 {
        assert_eq!(multiply_f32(0, 0, 1 << bit), Err(InvalidMxcsr));
    }
    assert_eq!(multiply_f32(0, 0, u32::MAX), Err(InvalidMxcsr));
}

#[test]
fn nan_selection_signed_zero_and_denormal_precedence_are_explicit() {
    for (a, b, bits, exceptions) in [
        (0x7fc1_2345, 0xff85_4321, 0x7fc1_2345, IE),
        (0xff85_4321, 0x7fc1_2345, 0xffc5_4321, IE),
        (0xffc5_4321, 0x7fc1_2345, 0xffc5_4321, 0),
        (0xff81_2345, 0x7f85_4321, 0xffc1_2345, IE),
        (0x7fc1_2345, 1, 0x7fc1_2345, 0),
        (1, 0x7fc1_2345, 0x7fc1_2345, 0),
        (0x7f81_2345, 1, 0x7fc1_2345, IE),
        (0x7f80_0000, 0, 0xffc0_0000, IE),
        (0xff80_0000, 1, 0xff80_0000, DE),
        (0x8000_0000, 0x3f80_0000, 0x8000_0000, 0),
        (0x8000_0000, 0xbf80_0000, 0, 0),
        (0x8000_0001, 0, 0x8000_0000, DE),
    ] {
        assert_eq!(
            multiply_f32(a, b, 0x1f80).unwrap(),
            Float32Multiplication { bits, exceptions }
        );
    }
    assert_eq!(
        multiply_f32(0x7f80_0000, 1, 0x1fc0).unwrap(),
        Float32Multiplication {
            bits: 0xffc0_0000,
            exceptions: IE,
        }
    );
}

#[test]
fn range_rounding_distinguishes_unbounded_and_bounded_precision() {
    for rc in 0..4 {
        for (a, b, bounded, masked_flags, unmasked_flags) in [
            (0x0080_0000, 0x3f00_0000, 0x0040_0000, 0, UE),
            (
                0x0080_0001,
                0x3f00_0000,
                if rc == 2 { 0x0040_0001 } else { 0x0040_0000 },
                UE | PE,
                UE,
            ),
            (
                0x0080_0000,
                0x3f7f_ffff,
                if rc == 0 || rc == 2 {
                    0x0080_0000
                } else {
                    0x007f_ffff
                },
                UE | PE,
                UE,
            ),
            (
                0x0080_0001,
                0x3f7f_fffe,
                if rc == 0 || rc == 2 {
                    0x0080_0000
                } else {
                    0x007f_ffff
                },
                if rc == 0 || rc == 2 { PE } else { UE | PE },
                if rc == 0 || rc == 2 { PE } else { UE | PE },
            ),
        ] {
            let control = 0x1f80 | (rc << 13);
            assert_eq!(
                multiply_f32(a, b, control).unwrap(),
                Float32Multiplication {
                    bits: bounded,
                    exceptions: masked_flags
                }
            );
            assert_eq!(
                multiply_f32(a, b, control & !0x800).unwrap().exceptions,
                unmasked_flags
            );
            let unmasked = multiply_f32(a, b, control & !0x800).unwrap();
            assert_eq!(
                multiply_f32(a, b, (control & !0x800) | 0x8000).unwrap(),
                unmasked,
                "FTZ is ignored with UM=0"
            );
            let tiny = unmasked_flags & UE != 0;
            assert_eq!(
                multiply_f32(a, b, control | 0x8000).unwrap(),
                Float32Multiplication {
                    bits: if tiny { 0 } else { bounded },
                    exceptions: if tiny { UE | PE } else { PE },
                }
            );
        }
        assert_eq!(
            multiply_f32(0x7f00_0000, 0x4000_0000, 0x1f80 | (rc << 13))
                .unwrap()
                .exceptions,
            OE | PE
        );
        assert_eq!(
            multiply_f32(0x7f00_0000, 0x4000_0000, (0x1f80 & !0x400) | (rc << 13))
                .unwrap()
                .exceptions,
            OE
        );
        // The smallest subnormal squared requires a shift far beyond 64.
        assert_eq!(
            multiply_f32(1, 1, 0x1f80 | (rc << 13)).unwrap(),
            Float32Multiplication {
                bits: u32::from(rc == 2),
                exceptions: DE | UE | PE,
            }
        );
        assert_eq!(
            multiply_f32(0x8000_0001, 1, 0x1f80 | (rc << 13)).unwrap(),
            Float32Multiplication {
                bits: SIGN | u32::from(rc == 1),
                exceptions: DE | UE | PE,
            }
        );
    }
}

#[cfg(target_arch = "x86_64")]
fn assert_native(a: [u32; 4], b: [u32; 4], control: u32) {
    unsafe extern "C" {
        fn vine_sse_native_mulps(a: *const u32, b: *const u32, control: u32, out: *mut u32);
    }
    let _guard = crate::ARITHMETIC_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let results =
        std::array::from_fn::<_, 4, _>(|index| multiply_f32(a[index], b[index], control).unwrap());
    let mut flags = results
        .iter()
        .fold(0, |flags, result| flags | result.exceptions);
    if flags & 7 & !(control >> 7) != 0 {
        flags &= 7;
    }
    let fault = flags & !(control >> 7) != 0;
    let mut out = [0; 6];
    // SAFETY: initialized four-lane inputs/six writable outputs. The native
    // oracle saves host FP state and captures Windows faults at one known PC;
    // the shared test lock serializes its temporary VEH observation pointer.
    unsafe {
        vine_sse_native_mulps(a.as_ptr(), b.as_ptr(), control, out.as_mut_ptr());
    }
    assert_ne!(out[5], u32::MAX, "native VEH installation failed");
    assert_eq!(
        out[5] != 0,
        fault,
        "a={a:x?}, b={b:x?}, mxcsr={control:x}, native={out:x?}"
    );
    assert_eq!(
        out[4],
        control | flags,
        "a={a:x?}, b={b:x?}, mxcsr={control:x}, native={out:x?}"
    );
    let expected = if fault {
        a
    } else {
        results.map(|result| result.bits)
    };
    assert_eq!(
        out[..4],
        expected,
        "a={a:x?}, b={b:x?}, mxcsr={control:x}, native={out:x?}"
    );
}

#[cfg(target_arch = "x86_64")]
#[test]
fn masked_packed_results_and_flags_match_native_for_all_roundings() {
    let classes = [
        0,
        0x8000_0000,
        1,
        0x8000_0001,
        0x007f_ffff,
        0x807f_ffff,
        0x0080_0000,
        0x8080_0000,
        0x0080_0001,
        0x3f00_0000,
        0x3f7f_ffff,
        0x3f7f_fffe,
        0x3f80_0000,
        0x3f80_0001,
        0xbf80_0001,
        0x4000_0000,
        0x7f00_0000,
        0xff00_0000,
        0x7f7f_ffff,
        0xff7f_ffff,
        0x7f80_0000,
        0xff80_0000,
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
    let mut seed = 0x8b13_4597_u32;
    for _ in 0..4096 {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let a = seed;
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        pairs.push((a, seed));
    }
    for chunk in pairs.chunks(4) {
        let a = std::array::from_fn(|i| chunk[i % chunk.len()].0);
        let b = std::array::from_fn(|i| chunk[i % chunk.len()].1);
        for rc in 0..4 {
            for daz_ftz in [0, 0x40, 0x8000, 0x8040] {
                assert_native(a, b, 0x1f80 | (rc << 13) | daz_ftz);
            }
        }
    }
}

#[cfg(all(target_arch = "x86_64", target_os = "windows"))]
#[test]
fn native_fault_context_keeps_destination_and_pre_post_priority() {
    let packs = [
        (
            [0x7f80_0000, 1, 0x7f7f_ffff, 0x3f80_0001],
            [0, 0x3f80_0000, 0x4000_0000, 0x3f80_0001],
        ),
        (
            [0x7f81_2345, 1, 0x0080_0000, 0x3f80_0001],
            [0x3f80_0000, 0x3f80_0000, 0x3f00_0000, 0x3f80_0001],
        ),
        (
            [0x7fc1_2345, 0xffc5_4321, 0x3f80_0000, 0x8000_0000],
            [1, 0x7f81_2345, 0x4000_0000, 0xbf80_0000],
        ),
    ];
    for (a, b) in packs {
        for rc in 0..4 {
            for masks in 0..64 {
                for daz_ftz in [0, 0x40, 0x8000, 0x8040] {
                    assert_native(a, b, (masks << 7) | (rc << 13) | daz_ftz);
                }
            }
        }
    }
    for (a, b) in [
        (0x0080_0000, 0x3f00_0000),
        (0x0080_0001, 0x3f00_0000),
        (0x0080_0000, 0x3f7f_ffff),
        (0x0080_0001, 0x3f7f_fffe),
        (0x7f00_0000, 0x4000_0000),
    ] {
        for sign in [0, SIGN] {
            for rc in 0..4 {
                for masks in 0..8 {
                    for ftz in [0, 0x8000] {
                        assert_native(
                            [a | sign, 0x3f80_0000, 0x4000_0000, 0x4040_0000],
                            [b, 0x3f80_0000, 0x3f80_0000, 0x3f80_0000],
                            0x380 | (masks << 10) | (rc << 13) | ftz,
                        );
                    }
                }
            }
        }
    }
    for rc in 0..4 {
        // Old sticky flags, even unmasked, do not themselves retrigger #XM.
        assert_native(
            [0x3f80_0000, 0xbf80_0000, 0, 0x8000_0000],
            [0x4000_0000; 4],
            0x3f | (rc << 13),
        );
    }
}

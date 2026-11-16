//! Numeric SSE scalar conversion. MXCSR masks, sticky flags and CPU state are
//! deliberately outside this layer. No x87 precision/control setting applies.
use crate::ARITHMETIC_LOCK;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IntegerConversion {
    pub value: i32,
    /// Newly raised MXCSR IE/PE only. These instructions never raise DE.
    pub exceptions: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Float32Conversion {
    pub bits: u32,
    /// Newly raised MXCSR PE only. Every i32 is a finite normal f32 or zero.
    pub exceptions: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidRoundingMode;

unsafe extern "C" {
    fn vine_sse_cvtt(wide: u32, bits: u64) -> u64;
    fn vine_sse_i32_to_f32(value: i32, rounding: u32) -> u64;
    fn vine_sse_cvt_f64(bits: u64, rounding: u32) -> u64;
}

/// Convert one CVTDQ2PS lane using MXCSR.RC (0 nearest-even, 1 down, 2 up,
/// 3 toward zero). The caller owns masks, sticky flags and packed commit.
/// DAZ and FTZ cannot affect an integer-to-f32 conversion.
pub fn convert_i32_to_f32(
    value: i32,
    rounding: u8,
) -> Result<Float32Conversion, InvalidRoundingMode> {
    if rounding > 3 {
        return Err(InvalidRoundingMode);
    }
    let _guard = ARITHMETIC_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    // SAFETY: numeric values only, with the C rounding selector checked above.
    let result = unsafe { vine_sse_i32_to_f32(value, u32::from(rounding)) };
    Ok(Float32Conversion {
        bits: result as u32,
        exceptions: (result >> 32) as u32,
    })
}

fn convert(wide: bool, bits: u64) -> IntegerConversion {
    let _guard = ARITHMETIC_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    // SAFETY: both values are numeric bits, not addresses; the selector is
    // bounded and all IEEE encodings are valid inputs to this conversion.
    let result = unsafe { vine_sse_cvtt(u32::from(wide), bits) };
    IntegerConversion {
        value: result as i32,
        exceptions: (result >> 32) as u32,
    }
}

pub fn truncate_f32_to_i32(bits: u32, denormals_are_zero: bool) -> IntegerConversion {
    if denormals_are_zero && bits & 0x7f80_0000 == 0 {
        IntegerConversion {
            value: 0,
            exceptions: 0,
        }
    } else {
        convert(false, u64::from(bits))
    }
}

pub fn truncate_f64_to_i32(bits: u64, denormals_are_zero: bool) -> IntegerConversion {
    if denormals_are_zero && bits & 0x7ff0_0000_0000_0000 == 0 {
        IntegerConversion {
            value: 0,
            exceptions: 0,
        }
    } else {
        convert(true, bits)
    }
}

pub fn round_f64_to_i32(bits: u64, mxcsr: u32) -> Result<IntegerConversion, crate::InvalidMxcsr> {
    if mxcsr & !0xffff != 0 {
        return Err(crate::InvalidMxcsr);
    }
    if mxcsr & 0x40 != 0 && bits & 0x7ff0_0000_0000_0000 == 0 {
        return Ok(IntegerConversion {
            value: 0,
            exceptions: 0,
        });
    }
    let _guard = ARITHMETIC_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    // SAFETY: numeric bits and a bounded rounding selector, no addresses.
    let result = unsafe { vine_sse_cvt_f64(bits, mxcsr >> 13 & 3) };
    Ok(IntegerConversion {
        value: result as i32,
        exceptions: (result >> 32) as u32,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // Independent integer rounding oracle. No native float casts or SoftFloat
    // operations participate in the expected result.
    fn expected_i32_to_f32(value: i32, rounding: u8) -> Float32Conversion {
        let magnitude = value.unsigned_abs();
        if magnitude == 0 {
            return Float32Conversion {
                bits: 0,
                exceptions: 0,
            };
        }
        let sign = u32::from(value < 0) << 31;
        let mut exponent = 31 - magnitude.leading_zeros();
        let (mut significand, discarded, shift) = if exponent > 23 {
            let shift = exponent - 23;
            (magnitude >> shift, magnitude & ((1 << shift) - 1), shift)
        } else {
            (magnitude << (23 - exponent), 0, 0)
        };
        if discarded != 0 {
            let increment = match rounding {
                0 => {
                    let halfway = 1 << (shift - 1);
                    discarded > halfway || (discarded == halfway && significand & 1 != 0)
                }
                1 => value < 0,
                2 => value > 0,
                3 => false,
                _ => unreachable!(),
            };
            significand += u32::from(increment);
            if significand == 1 << 24 {
                significand >>= 1;
                exponent += 1;
            }
        }
        Float32Conversion {
            bits: sign | ((exponent + 127) << 23) | (significand & 0x7f_ffff),
            exceptions: if discarded == 0 { 0 } else { 32 },
        }
    }

    fn integer_inputs() -> Vec<i32> {
        let mut values = vec![0, 1, -1, i32::MIN, i32::MAX];
        // Exact/inexact boundaries, both signs, midpoint parity and carry into
        // the next exponent. The largest f32 spacing here is 128.
        for exponent in 0..=31 {
            for offset in -257_i64..=257 {
                let magnitude = (1_i64 << exponent) + offset;
                for signed in [magnitude, -magnitude] {
                    if let Ok(value) = i32::try_from(signed) {
                        values.push(value);
                    }
                }
            }
        }
        let mut seed = 0x5231_abc9_u32;
        for _ in 0..4096 {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            values.push(seed as i32);
        }
        values.sort_unstable();
        values.dedup();
        values
    }

    #[test]
    fn integer_to_float_rejects_invalid_rounding() {
        for rounding in 4..=u8::MAX {
            assert_eq!(convert_i32_to_f32(1, rounding), Err(InvalidRoundingMode));
        }
    }

    #[test]
    fn integer_to_float_four_roundings_match_exact_integer_oracle() {
        for value in integer_inputs() {
            for rounding in 0..4 {
                assert_eq!(
                    convert_i32_to_f32(value, rounding).unwrap(),
                    expected_i32_to_f32(value, rounding),
                    "value={value}, RC={rounding}"
                );
            }
        }
        // Explicitly pin midpoint-even and directed rounding around 2^24.
        for (value, bits) in [
            (
                16_777_217,
                [0x4b80_0000, 0x4b80_0000, 0x4b80_0001, 0x4b80_0000],
            ),
            (
                16_777_219,
                [0x4b80_0002, 0x4b80_0001, 0x4b80_0002, 0x4b80_0001],
            ),
            (
                -16_777_217,
                [0xcb80_0000, 0xcb80_0001, 0xcb80_0000, 0xcb80_0000],
            ),
            (
                i32::MAX,
                [0x4f00_0000, 0x4eff_ffff, 0x4f00_0000, 0x4eff_ffff],
            ),
        ] {
            for (rounding, bits) in bits.into_iter().enumerate() {
                assert_eq!(
                    convert_i32_to_f32(value, rounding as u8).unwrap(),
                    Float32Conversion {
                        bits,
                        exceptions: 32
                    }
                );
            }
        }
    }

    #[cfg(target_arch = "x86_64")]
    #[test]
    fn integer_to_float_matches_native_packed_cvtdq2ps() {
        unsafe extern "C" {
            fn vine_sse_native_cvtdq2ps(input: *const i32, rounding: u32, out: *mut u32);
        }
        let values = integer_inputs();
        for chunk in values.chunks(4) {
            let lanes = std::array::from_fn::<_, 4, _>(|index| chunk[index % chunk.len()]);
            for rounding in 0..4 {
                let mut native = [0; 5];
                // SAFETY: four initialized inputs and five writable outputs;
                // the bounded selector is used by an x86-64 test-only oracle.
                unsafe {
                    vine_sse_native_cvtdq2ps(
                        lanes.as_ptr(),
                        u32::from(rounding),
                        native.as_mut_ptr(),
                    );
                }
                let mut exceptions = 0;
                for (index, value) in lanes.into_iter().enumerate() {
                    let expected = expected_i32_to_f32(value, rounding);
                    let actual = convert_i32_to_f32(value, rounding).unwrap();
                    assert_eq!(actual, expected, "value={value}, RC={rounding}");
                    assert_eq!(
                        native[index], expected.bits,
                        "lane={index}, value={value}, RC={rounding}"
                    );
                    exceptions |= expected.exceptions;
                }
                assert_eq!(native[4], exceptions, "lanes={lanes:?}, RC={rounding}");
            }
        }
    }

    #[test]
    fn scalar_conversion_preserves_integer_boundary_and_ie_pe_priority() {
        for (bits, value, exceptions) in [
            (0x3ff8_0000_0000_0000, 1, 32),
            (0xbff8_0000_0000_0000, -1, 32),
            (0x41df_ffff_ffff_ffff, i32::MAX, 32),
            (0x41e0_0000_0000_0000, i32::MIN, 1),
            (0xc1e0_0000_001f_ffff, i32::MIN, 32),
            (0xc1e0_0000_0020_0000, i32::MIN, 1),
            (0xc1e0_0000_0000_0000, i32::MIN, 0),
            (0x7ff8_0000_0000_0000, i32::MIN, 1),
            (0xfff0_0000_0000_0001, i32::MIN, 1),
        ] {
            assert_eq!(
                truncate_f64_to_i32(bits, false),
                IntegerConversion { value, exceptions }
            );
        }
        for (bits, value, exceptions) in [
            (0x3fc0_0000, 1, 32),
            (0xbfc0_0000, -1, 32),
            (0x4eff_ffff, 2_147_483_520, 0),
            (0x4f00_0000, i32::MIN, 1),
            (0xcf00_0000, i32::MIN, 0),
            (0xcf00_0001, i32::MIN, 1),
            (0x7fc0_0000, i32::MIN, 1),
            (0xff80_0001, i32::MIN, 1),
        ] {
            assert_eq!(
                truncate_f32_to_i32(bits, false),
                IntegerConversion { value, exceptions }
            );
        }
    }

    #[test]
    fn denormal_conversion_raises_precision_only_unless_daz() {
        for bits in [1, 0x007f_ffff, 0x8000_0001, 0x807f_ffff] {
            assert_eq!(
                truncate_f32_to_i32(bits, false),
                IntegerConversion {
                    value: 0,
                    exceptions: 32
                }
            );
            assert_eq!(
                truncate_f32_to_i32(bits, true),
                IntegerConversion {
                    value: 0,
                    exceptions: 0
                }
            );
        }
        for bits in [
            1,
            0x000f_ffff_ffff_ffff,
            0x8000_0000_0000_0001,
            0x800f_ffff_ffff_ffff,
        ] {
            assert_eq!(
                truncate_f64_to_i32(bits, false),
                IntegerConversion {
                    value: 0,
                    exceptions: 32
                }
            );
            assert_eq!(
                truncate_f64_to_i32(bits, true),
                IntegerConversion {
                    value: 0,
                    exceptions: 0
                }
            );
        }
    }
}

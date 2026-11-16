//! Pure fixed-format SSE numeric exports. Wasm calls these directly with value
//! bits; no CPU handle, registry or guest-memory access exists. Binary64 results
//! use instance-owned scratch (as do x87 kernels), copied before any other call.
const INVALID_INPUT: u64 = 1 << 63;

fn packed(result: vine_x87::IntegerConversion) -> u64 {
    u64::from(result.value as u32) | u64::from(result.exceptions) << 32
}

#[unsafe(no_mangle)]
pub extern "C" fn backend_sse_cvtt_f32(mxcsr: u32, low: u32, _high: u32) -> u64 {
    if mxcsr & !vine_cpu_profile::MXCSR_MASK != 0 {
        return INVALID_INPUT;
    }
    packed(vine_x87::truncate_f32_to_i32(low, mxcsr & 0x40 != 0))
}

#[unsafe(no_mangle)]
pub extern "C" fn backend_sse_cvtt_f64(mxcsr: u32, low: u32, high: u32) -> u64 {
    if mxcsr & !vine_cpu_profile::MXCSR_MASK != 0 {
        return INVALID_INPUT;
    }
    packed(vine_x87::truncate_f64_to_i32(
        u64::from(low) | u64::from(high) << 32,
        mxcsr & 0x40 != 0,
    ))
}

#[unsafe(no_mangle)]
pub extern "C" fn backend_sse_i32_to_f32(mxcsr: u32, low: u32, _high: u32) -> u64 {
    if mxcsr & !vine_cpu_profile::MXCSR_MASK != 0 {
        return INVALID_INPUT;
    }
    // Only RC belongs to the numeric operation. Generated code combines all
    // packed-lane results before publishing sticky flags or changing XMM.
    match vine_x87::convert_i32_to_f32(low as i32, ((mxcsr >> 13) & 3) as u8) {
        Ok(result) => u64::from(result.bits) | u64::from(result.exceptions) << 32,
        Err(_) => INVALID_INPUT,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn backend_sse_mul_f32(mxcsr: u32, left: u32, right: u32) -> u64 {
    if mxcsr & !vine_cpu_profile::MXCSR_MASK != 0 {
        return INVALID_INPUT;
    }
    // Return both exception stages. Generated packed code aggregates all
    // lanes and suppresses new post-computation flags if any pre-computation
    // exception is unmasked; no CPU state belongs to this numeric boundary.
    match vine_x87::multiply_f32(left, right, mxcsr) {
        Ok(result) => u64::from(result.bits) | u64::from(result.exceptions) << 32,
        Err(_) => INVALID_INPUT,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn backend_sse_add_f32(mxcsr: u32, left: u32, right: u32) -> u64 {
    if mxcsr & !vine_cpu_profile::MXCSR_MASK != 0 {
        return INVALID_INPUT;
    }
    match vine_x87::add_f32(left, right, mxcsr) {
        Ok(result) => u64::from(result.bits) | u64::from(result.exceptions) << 32,
        Err(_) => INVALID_INPUT,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn backend_sse_sub_f32(mxcsr: u32, left: u32, right: u32) -> u64 {
    if mxcsr & !vine_cpu_profile::MXCSR_MASK != 0 {
        return INVALID_INPUT;
    }
    match vine_x87::subtract_f32(left, right, mxcsr) {
        Ok(result) => u64::from(result.bits) | u64::from(result.exceptions) << 32,
        Err(_) => INVALID_INPUT,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn backend_sse_cvt_f64(mxcsr: u32, low: u32, high: u32) -> u64 {
    match vine_x87::round_f64_to_i32(u64::from(low) | u64::from(high) << 32, mxcsr) {
        Ok(value) => packed(value),
        Err(_) => INVALID_INPUT,
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
struct DoubleOutput {
    bits: u64,
    exceptions: u32,
}
thread_local! {
    #[cfg_attr(all(target_os = "windows", target_env = "gnu"),
        allow(clippy::missing_const_for_thread_local,
            reason = "https://github.com/rust-lang/rust-clippy/issues/13422"))]
    static DOUBLE: std::cell::Cell<DoubleOutput> = const { std::cell::Cell::new(DoubleOutput { bits: 0, exceptions: 0 }) };
}
macro_rules! double_kernel {
    ($name:ident, $operation:path) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn $name(mxcsr: u32, a: u64, b: u64) -> usize {
            // This synchronous numeric call imports nothing and cannot reenter.
            // On invalid control the previous scratch bytes remain unchanged.
            match $operation(a, b, mxcsr) {
                Ok(value) => DOUBLE.with(|scratch| {
                    scratch.set(DoubleOutput {
                        bits: value.bits,
                        exceptions: value.exceptions,
                    });
                    scratch.as_ptr() as usize
                }),
                Err(_) => 0,
            }
        }
    };
}
double_kernel!(backend_sse_add_f64, vine_x87::add_f64);
double_kernel!(backend_sse_sub_f64, vine_x87::subtract_f64);
double_kernel!(backend_sse_mul_f64, vine_x87::multiply_f64);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cvtt_exports_validate_control_and_ignore_rounding_ftz_and_unused_lane() {
        assert_eq!(backend_sse_cvtt_f32(0x1_1f80, 0, 0), INVALID_INPUT);
        assert_eq!(backend_sse_cvtt_f64(u32::MAX, 0, 0), INVALID_INPUT);
        for rc in 0..4 {
            for ftz in [0, 0x8000] {
                for masks in [0, 0x1f80] {
                    for sticky in [0, 0x3f] {
                        let control = rc << 13 | ftz | masks | sticky;
                        let expected = 32u64 << 32 | 1;
                        assert_eq!(
                            backend_sse_cvtt_f32(control, 0x3fc0_0000, u32::MAX),
                            expected
                        );
                        assert_eq!(backend_sse_cvtt_f64(control, 0, 0x3ff8_0000), expected);
                        assert_eq!(backend_sse_cvtt_f64(control | 0x40, 1, 0), 0);
                    }
                }
            }
        }
    }

    #[test]
    fn i32_to_f32_export_uses_only_rounding_and_returns_new_precision() {
        assert_eq!(backend_sse_i32_to_f32(0x1_1f80, 0, 0), INVALID_INPUT);
        assert_eq!(backend_sse_i32_to_f32(u32::MAX, 0, 0), INVALID_INPUT);
        for rc in 0..4 {
            for daz_ftz in [0, 0x40, 0x8000, 0x8040] {
                for masks in [0, 0x1f80] {
                    for sticky in [0, 0x3f] {
                        let control = rc << 13 | daz_ftz | masks | sticky;
                        for (value, bits) in [
                            (0, 0u32),
                            (1, 0x3f80_0000),
                            (-1, 0xbf80_0000),
                            (i32::MIN, 0xcf00_0000),
                        ] {
                            assert_eq!(
                                backend_sse_i32_to_f32(control, value as u32, u32::MAX),
                                u64::from(bits)
                            );
                        }
                        // 2^24 + 1 is halfway between adjacent binary32 values.
                        let positive = if rc == 2 { 0x4b80_0001 } else { 0x4b80_0000 };
                        let negative = if rc == 1 { 0xcb80_0001 } else { 0xcb80_0000 };
                        assert_eq!(
                            backend_sse_i32_to_f32(control, 16_777_217, 0),
                            (32u64 << 32) | positive
                        );
                        assert_eq!(
                            backend_sse_i32_to_f32(control, (-16_777_217i32) as u32, u32::MAX),
                            (32u64 << 32) | negative
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn multiply_export_preserves_raw_results_and_both_new_exception_stages() {
        assert_eq!(backend_sse_mul_f32(0x1_1f80, 0, 0), INVALID_INPUT);
        assert_eq!(backend_sse_mul_f32(u32::MAX, 0, 0), INVALID_INPUT);
        for sticky in [0, 0x3f] {
            let control = 0x1f80 | sticky;
            for (left, right, bits, exceptions) in [
                (0x3f80_0000, 0x4000_0000, 0x4000_0000u32, 0u32),
                (0x8000_0000, 0x3f80_0000, 0x8000_0000, 0),
                (0x7f80_0123, 0xffc0_0456, 0x7fc0_0123, 1),
                (0, 0x7f80_0000, 0xffc0_0000, 1),
                (1, 0x3f80_0000, 1, 2),
                (0x0080_0001, 0x3f00_0000, 0x0040_0000, 0x30),
                (0x7f00_0000, 0x4000_0000, 0x7f80_0000, 0x28),
            ] {
                assert_eq!(
                    backend_sse_mul_f32(control, left, right),
                    u64::from(bits) | u64::from(exceptions) << 32
                );
            }
            assert_eq!(backend_sse_mul_f32(control | 0x40, 1, 0x3f80_0000), 0);
            assert_eq!(
                backend_sse_mul_f32(control | 0x8000, 1, 0x3f80_0000),
                0x32u64 << 32
            );
            // Unmasked range exceptions report the precision of an unbounded
            // binary32 significand. These exact products do not raise PE.
            assert_eq!(
                backend_sse_mul_f32(control & !0x800, 0x0080_0001, 0x3f00_0000) >> 32,
                0x10
            );
            assert_eq!(
                backend_sse_mul_f32(control & !0x400, 0x7f00_0000, 0x4000_0000) >> 32,
                8
            );
        }
    }

    #[test]
    fn add_sub_exports_keep_nan_sign_zero_rounding_and_new_range_flags() {
        for operation in [backend_sse_add_f32, backend_sse_sub_f32] {
            assert_eq!(operation(0x1_1f80, 0, 0), INVALID_INPUT);
            assert_eq!(operation(u32::MAX, 0, 0), INVALID_INPUT);
            for sticky in [0, 0x3f] {
                let control = 0x1f80 | sticky;
                // SUB must select and quiet the original second NaN before
                // applying subtraction's sign inversion to finite operands.
                assert_eq!(operation(control, 0, 0xff80_0123), 1u64 << 32 | 0xffc0_0123);
                assert_eq!(
                    operation(control, 0x7fc0_0123, 0xff80_0456),
                    1u64 << 32 | 0x7fc0_0123
                );
                assert_eq!(operation(control, 1, 0), 2u64 << 32 | 1);
                assert_eq!(operation(control | 0x40, 1, 0), 0);
                assert_eq!(operation(control | 0x8000, 1, 0), 0x32u64 << 32);
                assert_eq!(operation(control & !0x800, 1, 0) >> 32, 0x12);
                for rc in 0..4 {
                    let control = control | rc << 13;
                    let cancellation = if rc == 1 { 0x8000_0000 } else { 0 };
                    assert_eq!(
                        backend_sse_add_f32(control, 0x3f80_0000, 0xbf80_0000),
                        cancellation
                    );
                    assert_eq!(
                        backend_sse_sub_f32(control, 0x3f80_0000, 0x3f80_0000),
                        cancellation
                    );
                    let positive_tie = if rc == 2 { 0x3f80_0001 } else { 0x3f80_0000 };
                    assert_eq!(
                        backend_sse_add_f32(control, 0x3f80_0000, 0x3380_0000),
                        32u64 << 32 | positive_tie
                    );
                    assert_eq!(
                        backend_sse_sub_f32(control, 0x3f80_0000, 0xb380_0000),
                        32u64 << 32 | positive_tie
                    );
                }
            }
        }
        // The finite unbounded sum 2^128 is exact. Only masked overflow
        // adds PE to OE; the unmasked destination is never committed.
        assert_eq!(
            backend_sse_add_f32(0x1f80, 0x7f00_0000, 0x7f00_0000) >> 32,
            0x28
        );
        assert_eq!(
            backend_sse_add_f32(0x1b80, 0x7f00_0000, 0x7f00_0000) >> 32,
            8
        );
        assert_eq!(
            backend_sse_sub_f32(0x1f80, 0x7f00_0000, 0xff00_0000) >> 32,
            0x28
        );
        assert_eq!(
            backend_sse_sub_f32(0x1b80, 0x7f00_0000, 0xff00_0000) >> 32,
            8
        );
    }
}

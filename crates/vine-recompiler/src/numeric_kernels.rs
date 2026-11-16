//! Fixed-operation, exact numeric kernels. They have no execution authority:
//! no Backend handle, CPU state, guest address, mapping, or output pointer input.
//!
//! Output belongs to this compiler instance (native: this OS thread). Generated
//! callers copy all seven words before another numeric call or any exit. These
//! synchronous functions import nothing and cannot reenter generated code. The
//! Wasm memory is non-shared; native SoftFloat calls retain their arithmetic lock.
use crate::fp_ir::NumericOutput;
use std::cell::Cell;
use vine_x87::{Arithmetic, Ext80, Format, NumericResult, Unsupported};

struct Scratch {
    output: Cell<NumericOutput>,
    error: Cell<u32>,
}
thread_local! {
    // The initializer is const; Windows GNU's macro expansion triggers #13422.
    #[cfg_attr(all(target_os = "windows", target_env = "gnu"),
        allow(clippy::missing_const_for_thread_local,
            reason = "https://github.com/rust-lang/rust-clippy/issues/13422"))]
    static SCRATCH: Scratch = const { Scratch {
        output: Cell::new(NumericOutput {
            value: [0; 3], bits: [0; 2], exceptions: 0, round_up: 0,
        }),
        error: Cell::new(0),
    } };
}

fn publish(result: Result<NumericResult, Unsupported>) -> usize {
    publish_output(result.map(Into::into))
}

fn publish_output(result: Result<NumericOutput, Unsupported>) -> usize {
    SCRATCH.with(|scratch| match result {
        Ok(result) => {
            scratch.output.set(result);
            scratch.error.set(0);
            scratch.output.as_ptr() as usize
        }
        Err(error) => {
            // Never replace even a part of the previous successful output.
            scratch.error.set(match error {
                Unsupported::ReservedPrecision => 1,
                Unsupported::UnmaskedRangeException => 2,
                Unsupported::InvalidRegister => 3,
                Unsupported::InvalidFormat => 4,
                Unsupported::UncertainRounding => 5,
                Unsupported::TranscendentalDomain => 6,
            });
            0
        }
    })
}

/// Generated callers copy this immediately into the unsupported exit record
/// after a zero result, before another kernel/backend can change the diagnostic.
#[unsafe(no_mangle)]
pub extern "C" fn backend_fp_kernel_error() -> u32 {
    SCRATCH.with(|scratch| scratch.error.get())
}

macro_rules! kernel {
    ($name:ident, $operation:expr) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn $name(
            control: u32,
            a0: u32,
            a1: u32,
            a2: u32,
            b0: u32,
            b1: u32,
            b2: u32,
        ) -> usize {
            if control > u32::from(u16::MAX) || a2 > u32::from(u16::MAX) || b2 > u32::from(u16::MAX)
            {
                return publish(Err(Unsupported::InvalidFormat));
            }
            let a = Ext80::from_words([a0, a1, a2]);
            let b = Ext80::from_words([b0, b1, b2]);
            publish($operation(control as u16, a, b))
        }
    };
}

// IDs match the typed IR's existing numeric operations. Each export directly
// selects its operation/format; no runtime opcode or registry lookup is used.
kernel!(backend_fp_kernel_0, |c, a, b| vine_x87::arithmetic(
    Arithmetic::Add,
    a,
    b,
    c
));
kernel!(backend_fp_kernel_1, |c, a, b| vine_x87::arithmetic(
    Arithmetic::Subtract,
    a,
    b,
    c
));
kernel!(backend_fp_kernel_2, |c, a, b| vine_x87::arithmetic(
    Arithmetic::Multiply,
    a,
    b,
    c
));
kernel!(backend_fp_kernel_3, |c, a, b| vine_x87::arithmetic(
    Arithmetic::Divide,
    a,
    b,
    c
));
kernel!(backend_fp_kernel_4, |c, a, b| vine_x87::arithmetic(
    Arithmetic::SquareRoot,
    a,
    b,
    c
));
kernel!(backend_fp_kernel_5, |c, a, b| vine_x87::arithmetic(
    Arithmetic::RoundToInteger,
    a,
    b,
    c
));
kernel!(backend_fp_kernel_6, |c, a: Ext80, _b| vine_x87::load(
    Format::Float32,
    a.significand,
    c
));
kernel!(backend_fp_kernel_7, |c, a: Ext80, _b| vine_x87::load(
    Format::Float64,
    a.significand,
    c
));
kernel!(backend_fp_kernel_8, |c, a: Ext80, _b| vine_x87::load(
    Format::Float80,
    a.significand,
    c
));
kernel!(backend_fp_kernel_9, |c, a: Ext80, _b| vine_x87::load(
    Format::Integer16,
    a.significand,
    c
));
kernel!(backend_fp_kernel_10, |c, a: Ext80, _b| vine_x87::load(
    Format::Integer32,
    a.significand,
    c
));
kernel!(backend_fp_kernel_11, |c, a: Ext80, _b| vine_x87::load(
    Format::Integer64,
    a.significand,
    c
));
kernel!(backend_fp_kernel_12, |c, a, _b| vine_x87::store(
    Format::Float32,
    a,
    c
));
kernel!(backend_fp_kernel_13, |c, a, _b| vine_x87::store(
    Format::Float64,
    a,
    c
));
kernel!(backend_fp_kernel_14, |c, a, _b| vine_x87::store(
    Format::Float80,
    a,
    c
));
kernel!(backend_fp_kernel_15, |c, a, _b| vine_x87::store(
    Format::Integer16,
    a,
    c
));
kernel!(backend_fp_kernel_16, |c, a, _b| vine_x87::store(
    Format::Integer32,
    a,
    c
));
kernel!(backend_fp_kernel_17, |c, a, _b| vine_x87::store(
    Format::Integer64,
    a,
    c
));
kernel!(backend_fp_kernel_18, |_c, a, b| Ok(vine_x87::compare(
    a, b, false
)));
kernel!(backend_fp_kernel_19, |_c, a, b| Ok(vine_x87::compare(
    a, b, true
)));
kernel!(backend_fp_kernel_20, |c, a, b| vine_x87::log2_multiply(
    a, b, c
));
kernel!(backend_fp_kernel_21, |c, a, b| vine_x87::arctangent(
    a, b, c
));
kernel!(backend_fp_kernel_22, |c, a, _b| vine_x87::cosine(a, c));
kernel!(backend_fp_kernel_23, |c, a, _b| vine_x87::sine(a, c));
kernel!(backend_fp_kernel_24, |c, a, _b| {
    vine_x87::exponential_minus_one(a, c)
});
kernel!(backend_fp_kernel_25, |c, a, b| vine_x87::scale(a, b, c));

// The import name fixes numerical policy in generated code/cache content.
// These wrappers have no Backend/CPU pointer and retain the exact commit ABI.
macro_rules! approximate_kernel {
    ($name:ident, $strict:ident, $mode:ident, $operation:literal) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn $name(
            control: u32,
            a0: u32,
            a1: u32,
            a2: u32,
            b0: u32,
            b1: u32,
            b2: u32,
        ) -> usize {
            if control > u32::from(u16::MAX) || a2 > u32::from(u16::MAX) || b2 > u32::from(u16::MAX)
            {
                return publish(Err(Unsupported::InvalidFormat));
            }
            if let Some(value) = crate::numeric_mode::approximate(
                crate::numeric_mode::NumericMode::$mode,
                $operation,
                control as u16,
                Ext80::from_words([a0, a1, a2]),
                Ext80::from_words([b0, b1, b2]),
            ) {
                publish(Ok(value))
            } else {
                $strict(control, a0, a1, a2, b0, b1, b2)
            }
        }
    };
}
approximate_kernel!(backend_fp_balanced_0, backend_fp_kernel_0, Balanced, 0);
approximate_kernel!(backend_fp_balanced_1, backend_fp_kernel_1, Balanced, 1);
approximate_kernel!(backend_fp_balanced_2, backend_fp_kernel_2, Balanced, 2);
approximate_kernel!(backend_fp_balanced_3, backend_fp_kernel_3, Balanced, 3);
approximate_kernel!(backend_fp_balanced_4, backend_fp_kernel_4, Balanced, 4);
approximate_kernel!(backend_fp_balanced_20, backend_fp_kernel_20, Balanced, 20);
approximate_kernel!(backend_fp_balanced_21, backend_fp_kernel_21, Balanced, 21);
approximate_kernel!(backend_fp_balanced_22, backend_fp_kernel_22, Balanced, 22);
approximate_kernel!(backend_fp_balanced_23, backend_fp_kernel_23, Balanced, 23);
approximate_kernel!(backend_fp_balanced_24, backend_fp_kernel_24, Balanced, 24);
approximate_kernel!(backend_fp_fast_0, backend_fp_kernel_0, Fast, 0);
approximate_kernel!(backend_fp_fast_1, backend_fp_kernel_1, Fast, 1);
approximate_kernel!(backend_fp_fast_2, backend_fp_kernel_2, Fast, 2);
approximate_kernel!(backend_fp_fast_3, backend_fp_kernel_3, Fast, 3);
approximate_kernel!(backend_fp_fast_4, backend_fp_kernel_4, Fast, 4);
approximate_kernel!(backend_fp_fast_20, backend_fp_kernel_20, Fast, 20);
approximate_kernel!(backend_fp_fast_21, backend_fp_kernel_21, Fast, 21);
approximate_kernel!(backend_fp_fast_22, backend_fp_kernel_22, Fast, 22);
approximate_kernel!(backend_fp_fast_23, backend_fp_kernel_23, Fast, 23);
approximate_kernel!(backend_fp_fast_24, backend_fp_kernel_24, Fast, 24);

#[cfg(test)]
mod tests {
    use super::*;

    type Kernel = extern "C" fn(u32, u32, u32, u32, u32, u32, u32) -> usize;
    const KERNELS: [Kernel; crate::fp_ir::NUMERIC_KERNEL_COUNT as usize] = [
        backend_fp_kernel_0,
        backend_fp_kernel_1,
        backend_fp_kernel_2,
        backend_fp_kernel_3,
        backend_fp_kernel_4,
        backend_fp_kernel_5,
        backend_fp_kernel_6,
        backend_fp_kernel_7,
        backend_fp_kernel_8,
        backend_fp_kernel_9,
        backend_fp_kernel_10,
        backend_fp_kernel_11,
        backend_fp_kernel_12,
        backend_fp_kernel_13,
        backend_fp_kernel_14,
        backend_fp_kernel_15,
        backend_fp_kernel_16,
        backend_fp_kernel_17,
        backend_fp_kernel_18,
        backend_fp_kernel_19,
        backend_fp_kernel_20,
        backend_fp_kernel_21,
        backend_fp_kernel_22,
        backend_fp_kernel_23,
        backend_fp_kernel_24,
        backend_fp_kernel_25,
    ];

    fn output(pointer: usize) -> NumericOutput {
        assert_ne!(pointer, 0);
        // SAFETY: the pointer is from a successful kernel call on this thread;
        // scratch lives for the thread and no call overlaps this copy.
        unsafe { *(pointer as *const NumericOutput) }
    }

    #[test]
    fn fixed_kernels_preserve_exact_results_and_failures_across_controls() {
        let values = [
            Ext80::ZERO,
            Ext80::ONE,
            Ext80::INDEFINITE,
            Ext80 {
                significand: 1,
                sign_exponent: 0,
            },
            Ext80 {
                significand: 0x8000_0000_0000_0001,
                sign_exponent: 0x7fff,
            },
            Ext80 {
                significand: 0x4000_0000_0000_0000,
                sign_exponent: 0x4000,
            },
            Ext80 {
                significand: 0xffff_ffff_ffff_ffff,
                sign_exponent: 0x7ffe,
            },
        ];
        for (operation, kernel) in KERNELS.into_iter().enumerate() {
            for pc in [0, 1, 2, 3] {
                for rc in 0..4 {
                    let control = 0x7f | pc << 8 | rc << 10;
                    for (index, a) in values.into_iter().enumerate() {
                        let a = a.words();
                        let b = values[(index + 1) % values.len()].words();
                        let expected = crate::fp_ir::numeric(operation as u32, control, a, b);
                        let previous = SCRATCH.with(|scratch| scratch.output.get());
                        let pointer = kernel(control, a[0], a[1], a[2], b[0], b[1], b[2]);
                        match expected {
                            Ok(value) => {
                                assert_eq!(output(pointer), value);
                                assert_eq!(backend_fp_kernel_error(), 0);
                            }
                            Err(_) => {
                                assert_eq!(pointer, 0);
                                assert_ne!(backend_fp_kernel_error(), 0);
                                assert_eq!(SCRATCH.with(|s| s.output.get()), previous);
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn scratch_has_no_backend_lifetime_or_cpu_authority() {
        let first = crate::backend_new(1);
        let second = crate::backend_new(1);
        assert_ne!(first, 0);
        assert_ne!(second, 0);
        let pointer = backend_fp_kernel_0(0x37f, 0, 0x8000_0000, 0x3fff, 0, 0x8000_0000, 0x3fff);
        let two = output(pointer);
        assert_eq!(crate::backend_drop(first), 0);
        assert_eq!(crate::backend_drop(second), 0);
        assert_eq!(output(pointer), two);
        for invalid in [(0x1_037f, 0, 0), (0x37f, 0x1_0000, 0), (0x37f, 0, 0x1_0000)] {
            assert_eq!(
                backend_fp_kernel_0(invalid.0, 0, 0, invalid.1, 0, 0, invalid.2),
                0
            );
            assert_eq!(backend_fp_kernel_error(), 4);
            assert_eq!(output(pointer), two);
        }
        assert_eq!(
            backend_fp_kernel_1(0x37f, 0, 0x8000_0000, 0x3fff, 0, 0x8000_0000, 0x3fff),
            pointer
        );
        assert_eq!(output(pointer).value, Ext80::ZERO.words());
    }

    #[test]
    fn native_threads_have_separate_output_and_error_cells() {
        let pointer = backend_fp_kernel_0(0x37f, 0, 0x8000_0000, 0x3fff, 0, 0x8000_0000, 0x3fff);
        let previous = output(pointer);
        std::thread::spawn(|| {
            let other = backend_fp_kernel_1(0x37f, 0, 0x8000_0000, 0x3fff, 0, 0x8000_0000, 0x3fff);
            assert_eq!(output(other).value, Ext80::ZERO.words());
            assert_eq!(backend_fp_kernel_0(u32::MAX, 0, 0, 0, 0, 0, 0), 0);
            assert_eq!(backend_fp_kernel_error(), 4);
        })
        .join()
        .unwrap();
        assert_eq!(output(pointer), previous);
        assert_eq!(backend_fp_kernel_error(), 0);
    }
}

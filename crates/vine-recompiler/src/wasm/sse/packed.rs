//! Exact packed algorithms on a proven numeric domain; all other inputs use
//! the existing precise kernels. No assumption is learned from a guest PC.
//!
//! Wasm SIMD operations round to nearest, ties to even. For ADD/SUB we admit
//! only that MXCSR rounding mode and zero/normal operands with biased exponents
//! 25..=230. The smallest nonzero difference is 2^-125 and the largest sum is
//! below 2^105: neither range exception nor a subnormal result is possible.
//! With exponent gap <=29, two binary32 significands have an exact binary64
//! sum/difference (at most 53 significant bits). Compare that sum with the
//! promoted binary32 result to determine PE. With a larger gap and two nonzero
//! inputs the exact sum cannot be binary32, so PE is unconditional even if a
//! binary64 sum loses the smaller operand. Signed zeros follow RN arithmetic.
//! DAZ/FTZ have no effect on this admitted domain. No NaN/Inf is admitted.
//!
//! CVTDQ2PS admits only exactly representable integers, hence every MXCSR.RC
//! agrees. CVTTPS2DQ admits zero/normal finite inputs in [-2^31,2^31). Its
//! truncated integer is representable as binary32: a fractional input has
//! magnitude below 2^24; larger binary32 inputs already are integers. Converting
//! it back therefore detects PE exactly, independently of MXCSR.RC.
//!
//! Native results and fresh flags remain private until the caller aggregates
//! exceptions and checks #XM. Memory access, old sticky flags, partial effects
//! and destination suppression use the same contract as the precise path.
//! SIMD encodings: WebAssembly/simd proposals/simd/BinarySIMD.md.

use super::*;

const NUMERIC: u32 = 51;

impl Emitter {
    fn sse_splat_bits(&mut self, bits: u32) {
        self.num(bits);
        self.simd(0x11); // i32x4.splat
    }

    fn sse_absolute_bits(&mut self, vector: u32) {
        self.get(vector);
        self.sse_splat_bits(0x7fff_ffff);
        self.simd(0x4e);
    }

    fn sse_normal_sum_domain(&mut self, vector: u32) {
        self.sse_absolute_bits(vector);
        self.sse_splat_bits(25 << 23);
        self.simd(0x40); // i32x4.ge_u
        self.sse_absolute_bits(vector);
        self.sse_splat_bits((231 << 23) - 1);
        self.simd(0x3e); // i32x4.le_u
        self.simd(0x4e);
        self.sse_absolute_bits(vector);
        self.vector_zero();
        self.simd(0x37); // i32x4.eq: includes both signs of zero
        self.simd(0x50);
        self.simd(0xa3); // i32x4.all_true
    }

    pub(super) fn sse_native_packed_guard(&mut self, opcode: SseOpcode, destination: u8) {
        let sum = matches!(opcode, SseOpcode::Addps | SseOpcode::Subps);
        self.load(0, 396);
        self.num(0xffff_0000 | if sum { 0x6000 } else { 0 });
        self.op(0x71);
        self.op(0x45); // Reserved MXCSR bits still reach precise rejection.
        match opcode {
            SseOpcode::Cvtdq2ps => {
                self.get(VECTOR);
                self.simd(0xa0); // INT_MIN stays 0x80000000, unsigned comparison rejects it.
                self.sse_splat_bits(1 << 24);
                self.simd(0x3e);
                self.simd(0xa3);
            }
            SseOpcode::Cvttps2dq => {
                self.get(VECTOR);
                self.sse_splat_bits(0xcf00_0000); // -2^31 inclusive
                self.simd(0x46);
                self.get(VECTOR);
                self.sse_splat_bits(0x4f00_0000); // +2^31 exclusive; NaNs fail both bounds
                self.simd(0x43);
                self.simd(0x4e);
                self.sse_absolute_bits(VECTOR);
                self.sse_splat_bits(0x0080_0000);
                self.simd(0x40);
                self.sse_absolute_bits(VECTOR);
                self.vector_zero();
                self.simd(0x37);
                self.simd(0x50); // zero or normal; DAZ cases stay on the precise path
                self.simd(0x4e);
                self.simd(0xa3);
            }
            SseOpcode::Addps | SseOpcode::Subps => {
                self.sse_normal_sum_domain(XMM + u32::from(destination));
                self.sse_normal_sum_domain(VECTOR);
                self.op(0x71);
            }
            _ => unreachable!("qualified packed operation"),
        }
        self.op(0x71);
    }

    fn sse_promote_half(&mut self, vector: u32, high: bool) {
        self.get(vector);
        if high {
            self.get(vector);
            self.simd(0x0d);
            self.code
                .extend([8, 9, 10, 11, 12, 13, 14, 15, 0, 1, 2, 3, 4, 5, 6, 7]);
        }
        self.simd(0x5f); // exact f64x2.promote_low_f32x4
    }

    fn sse_exponents(&mut self, vector: u32) {
        self.sse_absolute_bits(vector);
        self.num(23);
        self.simd(0xad);
    }

    pub(super) fn sse_native_packed(&mut self, opcode: SseOpcode, destination: u8) {
        match opcode {
            SseOpcode::Cvtdq2ps => {
                self.get(VECTOR);
                self.simd(0xfa);
                self.set(VECTOR);
                self.num(0);
                self.set(B);
            }
            SseOpcode::Cvttps2dq => {
                self.get(VECTOR);
                self.simd(0xf8); // valid-domain conversion, saturation cannot occur
                self.tee(NUMERIC);
                self.simd(0xfa);
                self.get(VECTOR);
                self.simd(0x42); // f32x4.ne, -0 equals +0
                self.simd(0x53);
                self.num(5);
                self.op(0x74);
                self.set(B);
                self.get(NUMERIC);
                self.set(VECTOR);
            }
            SseOpcode::Addps | SseOpcode::Subps => {
                let left = XMM + u32::from(destination);
                let subtract = opcode == SseOpcode::Subps;
                self.get(left);
                self.get(VECTOR);
                self.simd(if subtract { 0xe5 } else { 0xe4 });
                self.set(NUMERIC);
                for high in [false, true] {
                    self.sse_promote_half(left, high);
                    self.sse_promote_half(VECTOR, high);
                    self.simd(if subtract { 0xf1 } else { 0xf0 });
                    self.sse_promote_half(NUMERIC, high);
                    self.simd(0x48); // f64x2.ne
                }
                self.simd(0x50);
                self.simd(0x53);
                // A large exponent gap can hide a nonzero operand even in f64.
                self.sse_exponents(left);
                self.sse_exponents(VECTOR);
                self.simd(0xb1);
                self.simd(0xa0);
                self.sse_splat_bits(29);
                self.simd(0x3c);
                for vector in [left, VECTOR] {
                    self.sse_absolute_bits(vector);
                    self.vector_zero();
                    self.simd(0x38);
                    self.simd(0x4e);
                }
                self.simd(0x53);
                self.op(0x72);
                self.num(5);
                self.op(0x74);
                self.set(B);
                self.get(NUMERIC);
                self.set(VECTOR);
            }
            _ => unreachable!("qualified packed operation"),
        }
    }
}

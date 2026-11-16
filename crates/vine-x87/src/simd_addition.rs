//! Raw binary32 addition/subtraction for legacy SSE. Every finite input is an
//! integer multiple of 2^-149; five limbs retain the exact signed sum before
//! its single rounding to 24 significand bits. No host FP or x87 state is used.
//! Intel SDM Vol.1 Table 4-7 and sections 11.5.2/11.5.3 define NaN selection
//! and the pre/post exceptions. The CPU owns packed aggregation and commit.

use crate::InvalidMxcsr;

const IE: u32 = 1;
const DE: u32 = 2;
const OE: u32 = 8;
const UE: u32 = 16;
const PE: u32 = 32;
const SIGN: u32 = 0x8000_0000;
const INFINITY: u32 = 0x7f80_0000;
const FRACTION: u32 = 0x007f_ffff;
const QUIET: u32 = 0x0040_0000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Float32Addition {
    pub bits: u32,
    /// Newly raised IE/DE and OE/UE/PE for this lane, before the packed gate.
    pub exceptions: u32,
}

fn nan(bits: u32) -> bool {
    bits & INFINITY == INFINITY && bits & FRACTION != 0
}

fn denormal(bits: u32) -> bool {
    bits & INFINITY == 0 && bits & FRACTION != 0
}

/// Unsigned magnitude in exact units of 2^-149. The highest input bit is 276,
/// and adding two finite inputs needs at most bit 277, below the 320-bit bound.
#[derive(Clone, Copy)]
struct Magnitude([u64; 5]);

impl Magnitude {
    fn from_finite(bits: u32) -> Self {
        let exponent = (bits >> 23) & 255;
        let (significand, shift) = if exponent == 0 {
            (u64::from(bits & FRACTION), 0)
        } else {
            (u64::from((bits & FRACTION) | 0x0080_0000), exponent - 1)
        };
        let word = (shift / 64) as usize;
        let offset = shift % 64;
        let mut result = [0; 5];
        result[word] = significand << offset;
        if offset != 0 && word + 1 < result.len() {
            result[word + 1] = significand >> (64 - offset);
        }
        Self(result)
    }

    fn add(self, other: Self) -> Self {
        let mut result = [0; 5];
        let mut carry = false;
        for (index, word) in result.iter_mut().enumerate() {
            let (sum, first) = self.0[index].overflowing_add(other.0[index]);
            let (sum, second) = sum.overflowing_add(u64::from(carry));
            *word = sum;
            carry = first || second;
        }
        debug_assert!(!carry, "two finite binary32 values fit in 278 bits");
        Self(result)
    }

    /// The caller compares the positive IEEE encodings first, so self >= other.
    fn subtract(self, other: Self) -> Self {
        let mut result = [0; 5];
        let mut borrow = false;
        for (index, word) in result.iter_mut().enumerate() {
            let (difference, first) = self.0[index].overflowing_sub(other.0[index]);
            let (difference, second) = difference.overflowing_sub(u64::from(borrow));
            *word = difference;
            borrow = first || second;
        }
        debug_assert!(!borrow, "magnitude subtraction is nonnegative");
        Self(result)
    }

    fn top(self) -> Option<u32> {
        self.0.iter().enumerate().rev().find_map(|(index, word)| {
            (*word != 0).then(|| index as u32 * 64 + 63 - word.leading_zeros())
        })
    }

    fn bit(self, index: u32) -> bool {
        self.0[(index / 64) as usize] & (1_u64 << (index % 64)) != 0
    }

    fn any_below(self, index: u32) -> bool {
        let word = (index / 64) as usize;
        let offset = index % 64;
        self.0[..word].iter().any(|value| *value != 0)
            || self.0[word] & ((1_u64 << offset) - 1) != 0
    }

    /// The caller keeps the top 24 bits; no higher quotient bits are discarded.
    fn quotient(self, shift: u32) -> u32 {
        let word = (shift / 64) as usize;
        let offset = shift % 64;
        let low = self.0[word] >> offset;
        let high = if offset != 0 && word + 1 < self.0.len() {
            self.0[word + 1] << (64 - offset)
        } else {
            0
        };
        (low | high) as u32
    }
}

/// Add one lane under MXCSR.RC/DAZ/FTZ/OM/UM. Flags are fresh, not sticky;
/// unmasked exceptions are reported without changing any host/guest state.
pub fn add_f32(a: u32, b: u32, mxcsr: u32) -> Result<Float32Addition, InvalidMxcsr> {
    addition(a, b, mxcsr, false)
}

/// Subtract source b from source a. NaN selection uses both original bit
/// patterns, before the arithmetic sign change of a finite/infinite source b.
pub fn subtract_f32(a: u32, b: u32, mxcsr: u32) -> Result<Float32Addition, InvalidMxcsr> {
    addition(a, b, mxcsr, true)
}

fn addition(
    a_bits: u32,
    b_bits: u32,
    mxcsr: u32,
    subtract: bool,
) -> Result<Float32Addition, InvalidMxcsr> {
    if mxcsr & !0xffff != 0 {
        return Err(InvalidMxcsr);
    }
    let result = |bits, exceptions| Ok(Float32Addition { bits, exceptions });
    // SSE selects the first NaN, with its sign/payload intact except quieting.
    // An SNaN in either source raises IE; NaNs suppress same-lane DE.
    if nan(a_bits) || nan(b_bits) {
        let invalid = (nan(a_bits) && a_bits & QUIET == 0) || (nan(b_bits) && b_bits & QUIET == 0);
        return result(
            (if nan(a_bits) { a_bits } else { b_bits }) | QUIET,
            if invalid { IE } else { 0 },
        );
    }
    let daz = mxcsr & 0x40 != 0;
    let a = if daz && denormal(a_bits) {
        a_bits & SIGN
    } else {
        a_bits
    };
    let b = if daz && denormal(b_bits) {
        b_bits & SIGN
    } else {
        b_bits
    } ^ if subtract { SIGN } else { 0 };
    let a_magnitude = a & !SIGN;
    let b_magnitude = b & !SIGN;
    let opposite = (a ^ b) & SIGN != 0;
    if a_magnitude == INFINITY && b_magnitude == INFINITY && opposite {
        return result(0xffc0_0000, IE);
    }
    let pre = if denormal(a) || denormal(b) { DE } else { 0 };
    if a_magnitude == INFINITY {
        return result(a, pre);
    }
    if b_magnitude == INFINITY {
        return result(b, pre);
    }
    let rc = (mxcsr >> 13) & 3;
    let left = Magnitude::from_finite(a);
    let right = Magnitude::from_finite(b);
    let (magnitude, sign) = if !opposite {
        (left.add(right), a & SIGN)
    } else if a_magnitude > b_magnitude {
        (left.subtract(right), a & SIGN)
    } else if b_magnitude > a_magnitude {
        (right.subtract(left), b & SIGN)
    } else {
        // Opposite equal values, including signed zeros, cancel exactly.
        return result(if rc == 1 { SIGN } else { 0 }, pre);
    };
    let Some(mut top) = magnitude.top() else {
        return result(sign, pre); // Two same-sign zeros preserve that sign.
    };
    if top < 23 {
        // The exact sum is an integer multiple of 2^-149. Every tiny nonzero
        // sum therefore fits the subnormal fraction exactly, with no rounding.
        if mxcsr & 0x800 == 0 {
            return result(sign | magnitude.0[0] as u32, pre | UE);
        }
        if mxcsr & 0x8000 != 0 {
            return result(sign, pre | UE | PE);
        }
        return result(sign | magnitude.0[0] as u32, pre);
    }
    let shift = top - 23;
    let mut rounded = magnitude.quotient(shift);
    let inexact = magnitude.any_below(shift);
    let increment = inexact
        && match rc {
            0 => magnitude.bit(shift - 1) && (magnitude.any_below(shift - 1) || rounded & 1 != 0),
            1 => sign != 0,
            2 => sign == 0,
            3 => false,
            _ => unreachable!("MXCSR.RC is two bits"),
        };
    rounded += u32::from(increment);
    if rounded == 1 << 24 {
        rounded >>= 1;
        top += 1;
    }
    // The rounded exponent is top - 149, before the binary32 exponent bound.
    if top > 276 {
        let to_infinity = rc == 0 || (rc == 1 && sign != 0) || (rc == 2 && sign == 0);
        let bits = sign | if to_infinity { INFINITY } else { INFINITY - 1 };
        let precision = mxcsr & 0x400 != 0 || inexact;
        return result(bits, pre | OE | if precision { PE } else { 0 });
    }
    result(
        sign | ((top - 22) << 23) | (rounded & FRACTION),
        pre | if inexact { PE } else { 0 },
    )
}

#[cfg(test)]
#[path = "simd_addition_tests.rs"]
mod tests;

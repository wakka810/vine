//! Raw binary32 multiplication for legacy SSE. Integer products retain all
//! 48 significand bits; no host floating point or x87 arithmetic state is used.
//! The CPU combines packed lanes and suppresses post-computation flags when
//! any lane has an unmasked pre-computation exception, before committing XMM.

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
pub struct Float32Multiplication {
    pub bits: u32,
    /// Newly raised IE/DE and OE/UE/PE for this lane, before the packed gate.
    pub exceptions: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidMxcsr;

fn nan(bits: u32) -> bool {
    bits & INFINITY == INFINITY && bits & FRACTION != 0
}

fn denormal(bits: u32) -> bool {
    bits & INFINITY == 0 && bits & FRACTION != 0
}

fn finite(bits: u32) -> (u64, i32) {
    let exponent = (bits >> 23) & 255;
    if exponent == 0 {
        (u64::from(bits & FRACTION), -149)
    } else {
        (
            u64::from((bits & FRACTION) | 0x0080_0000),
            exponent as i32 - 150,
        )
    }
}

/// Round an exact nonnegative integer after division by 2^shift. A very tiny
/// product may require more than 64 discarded places: it is below halfway,
/// but directed rounding must still retain the nonzero remainder.
fn round_shift(value: u64, shift: u32, negative: bool, rc: u32) -> (u64, bool) {
    if shift == 0 {
        return (value, false);
    }
    let (quotient, remainder, halfway) = if shift < 64 {
        let remainder = value & ((1_u64 << shift) - 1);
        (
            value >> shift,
            remainder,
            remainder.cmp(&(1_u64 << (shift - 1))),
        )
    } else {
        let halfway = if shift == 64 {
            value.cmp(&(1_u64 << 63))
        } else {
            std::cmp::Ordering::Less
        };
        (0, value, halfway)
    };
    let inexact = remainder != 0;
    let increment = inexact
        && match rc {
            0 => halfway.is_gt() || (halfway.is_eq() && quotient & 1 != 0),
            1 => negative,
            2 => !negative,
            3 => false,
            _ => unreachable!("MXCSR.RC is two bits"),
        };
    (quotient + u64::from(increment), inexact)
}

/// Multiply one lane using MXCSR.RC/DAZ/FTZ/OM/UM. No flags are made sticky
/// here, no architectural destination is owned, and unmasked inputs do not
/// cause host exceptions. All other MXCSR fields are ignored after validation.
pub fn multiply_f32(
    a_bits: u32,
    b_bits: u32,
    mxcsr: u32,
) -> Result<Float32Multiplication, InvalidMxcsr> {
    if mxcsr & !0xffff != 0 {
        return Err(InvalidMxcsr);
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
    };
    let result = |bits, exceptions| Ok(Float32Multiplication { bits, exceptions });
    // Intel SDM Vol.1 Table 4-7: with two NaNs SSE selects the first
    // source, unlike x87's NaN selection. NaNs suppress DE in the
    // same lane; an SNaN in either source still raises IE.
    if nan(a) || nan(b) {
        let invalid = (nan(a) && a & QUIET == 0) || (nan(b) && b & QUIET == 0);
        return result(
            (if nan(a) { a } else { b }) | QUIET,
            if invalid { IE } else { 0 },
        );
    }
    let sign = (a ^ b) & SIGN;
    let a_magnitude = a & !SIGN;
    let b_magnitude = b & !SIGN;
    if (a_magnitude == INFINITY && b_magnitude == 0)
        || (b_magnitude == INFINITY && a_magnitude == 0)
    {
        return result(0xffc0_0000, IE);
    }
    let pre = if denormal(a) || denormal(b) { DE } else { 0 };
    if a_magnitude == INFINITY || b_magnitude == INFINITY {
        return result(sign | INFINITY, pre);
    }
    if a_magnitude == 0 || b_magnitude == 0 {
        return result(sign, pre);
    }
    let (a_significand, a_exponent) = finite(a);
    let (b_significand, b_exponent) = finite(b);
    let product = a_significand * b_significand;
    let exponent = a_exponent + b_exponent;
    let top = 63 - product.leading_zeros() as i32;
    let rc = (mxcsr >> 13) & 3;
    let negative = sign != 0;
    let (mut rounded, unbounded_inexact) = if top <= 23 {
        (product << (23 - top), false)
    } else {
        round_shift(product, (top - 23) as u32, negative, rc)
    };
    let mut unbounded_exponent = exponent + top;
    if rounded == 1 << 24 {
        rounded >>= 1;
        unbounded_exponent += 1;
    }
    if unbounded_exponent > 127 {
        let to_infinity = rc == 0 || (rc == 1 && negative) || (rc == 2 && !negative);
        let bits = sign | if to_infinity { INFINITY } else { INFINITY - 1 };
        // Masked overflow is inexact even when the unbounded result is exact.
        // Unmasked overflow reports precision from unbounded rounding only.
        let precision = mxcsr & 0x400 != 0 || unbounded_inexact;
        return result(bits, pre | OE | if precision { PE } else { 0 });
    }
    if unbounded_exponent < -126 {
        let underflow_masked = mxcsr & 0x800 != 0;
        if underflow_masked && mxcsr & 0x8000 != 0 {
            // FTZ acts even on an exact tiny result, and reports both UE/PE.
            return result(sign, pre | UE | PE);
        }
        // Compute the bounded result directly from the exact product, never
        // by rounding the already rounded unbounded significand a second time.
        let (bounded, bounded_inexact) = if exponent >= -149 {
            // Tininess implies top + exponent < -126, so this left shift
            // cannot exceed 22 - top or overflow the exact product.
            (product << (exponent + 149), false)
        } else {
            round_shift(product, (-149 - exponent) as u32, negative, rc)
        };
        let flags = if underflow_masked {
            if bounded_inexact { UE | PE } else { 0 }
        } else {
            UE | if unbounded_inexact { PE } else { 0 }
        };
        // A rounded subnormal may become the minimum normal (0x00800000).
        // Tininess above describes rounding with an unbounded exponent.
        return result(sign | bounded as u32, pre | flags);
    }
    result(
        sign | ((unbounded_exponent + 127) as u32) << 23 | (rounded as u32 & FRACTION),
        pre | if unbounded_inexact { PE } else { 0 },
    )
}

#[cfg(test)]
#[path = "simd_multiply_tests.rs"]
mod tests;

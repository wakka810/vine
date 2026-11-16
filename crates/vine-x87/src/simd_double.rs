//! Legacy SSE2 binary64 arithmetic. Products retain all 106 significand bits.
//! Addition aligns significands with three rounding bits and a jammed sticky
//! bit. Cancellation can expose low bits only for exponent distances <= 1,
//! where alignment is exact. No host floating point or x87 state participates.
//! The CPU aggregates fresh exceptions across lanes before committing results.
use crate::InvalidMxcsr;

const SIGN: u64 = 1 << 63;
const INF: u64 = 0x7ff0_0000_0000_0000;
const FRAC: u64 = 0x000f_ffff_ffff_ffff;
const QUIET: u64 = 1 << 51;
const IE: u32 = 1;
const DE: u32 = 2;
const OE: u32 = 8;
const UE: u32 = 16;
const PE: u32 = 32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Float64Result {
    pub bits: u64,
    pub exceptions: u32,
}

fn result(bits: u64, exceptions: u32) -> Result<Float64Result, InvalidMxcsr> {
    Ok(Float64Result { bits, exceptions })
}
fn nan(bits: u64) -> bool {
    bits & INF == INF && bits & FRAC != 0
}
fn denormal(bits: u64) -> bool {
    bits & INF == 0 && bits & FRAC != 0
}
fn finite(bits: u64) -> (u64, i32) {
    let exponent = ((bits >> 52) & 2047) as i32;
    if exponent == 0 {
        (bits & FRAC, -1074)
    } else {
        ((bits & FRAC) | 1 << 52, exponent - 1075)
    }
}
fn sources(a: u64, b: u64, mxcsr: u32) -> (u64, u64) {
    let daz = |x| {
        if mxcsr & 0x40 != 0 && denormal(x) {
            x & SIGN
        } else {
            x
        }
    };
    (daz(a), daz(b))
}
fn nan_result(a: u64, b: u64) -> Option<Float64Result> {
    (nan(a) || nan(b)).then(|| Float64Result {
        bits: (if nan(a) { a } else { b }) | QUIET,
        exceptions: if (nan(a) && a & QUIET == 0) || (nan(b) && b & QUIET == 0) {
            IE
        } else {
            0
        },
    })
}

fn round_shift(value: u128, shift: u32, negative: bool, rc: u32) -> (u64, bool) {
    if shift == 0 {
        return (value as u64, false);
    }
    let (quotient, remainder, halfway) = if shift < 128 {
        let remainder = value & ((1_u128 << shift) - 1);
        (
            value >> shift,
            remainder,
            remainder.cmp(&(1_u128 << (shift - 1))),
        )
    } else {
        (
            0,
            value,
            if shift == 128 {
                value.cmp(&(1_u128 << 127))
            } else {
                std::cmp::Ordering::Less
            },
        )
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
    (quotient as u64 + u64::from(increment), inexact)
}

/// Round an exact positive integer times 2^exponent once. For addition the low
/// sticky bit summarizes distant operands; only normal rounding can reach it.
fn finish(
    value: u128,
    exponent: i32,
    sign: u64,
    pre: u32,
    mxcsr: u32,
) -> Result<Float64Result, InvalidMxcsr> {
    if value == 0 {
        return result(sign, pre);
    }
    let top = 127 - value.leading_zeros() as i32;
    let rc = (mxcsr >> 13) & 3;
    let (mut rounded, inexact) = if top <= 52 {
        ((value << (52 - top)) as u64, false)
    } else {
        round_shift(value, (top - 52) as u32, sign != 0, rc)
    };
    let mut unbounded_exponent = exponent + top;
    if rounded == 1 << 53 {
        rounded >>= 1;
        unbounded_exponent += 1;
    }
    if unbounded_exponent > 1023 {
        let infinity = rc == 0 || (rc == 1 && sign != 0) || (rc == 2 && sign == 0);
        let precision = mxcsr & 0x400 != 0 || inexact;
        return result(
            sign | if infinity { INF } else { INF - 1 },
            pre | OE | if precision { PE } else { 0 },
        );
    }
    if unbounded_exponent < -1022 {
        let masked = mxcsr & 0x800 != 0;
        if masked && mxcsr & 0x8000 != 0 {
            return result(sign, pre | UE | PE);
        }
        let (bounded, bounded_inexact) = if exponent >= -1074 {
            ((value << (exponent + 1074)) as u64, false)
        } else {
            round_shift(value, (-1074 - exponent) as u32, sign != 0, rc)
        };
        let flags = if masked {
            if bounded_inexact { UE | PE } else { 0 }
        } else {
            UE | if inexact { PE } else { 0 }
        };
        return result(sign | bounded, pre | flags);
    }
    result(
        sign | ((unbounded_exponent + 1023) as u64) << 52 | (rounded & FRAC),
        pre | if inexact { PE } else { 0 },
    )
}

pub fn multiply_f64(a: u64, b: u64, mxcsr: u32) -> Result<Float64Result, InvalidMxcsr> {
    if mxcsr & !0xffff != 0 {
        return Err(InvalidMxcsr);
    }
    if let Some(value) = nan_result(a, b) {
        return Ok(value);
    }
    let (a, b) = sources(a, b, mxcsr);
    let sign = (a ^ b) & SIGN;
    let (am, bm) = (a & !SIGN, b & !SIGN);
    if (am == INF && bm == 0) || (bm == INF && am == 0) {
        return result(SIGN | INF | QUIET, IE);
    }
    let pre = if denormal(a) || denormal(b) { DE } else { 0 };
    if am == INF || bm == INF {
        return result(sign | INF, pre);
    }
    let (av, ae) = finite(a);
    let (bv, be) = finite(b);
    finish(u128::from(av) * u128::from(bv), ae + be, sign, pre, mxcsr)
}

fn shift_jam(value: u64, distance: u32) -> u64 {
    if distance == 0 {
        value
    } else if distance < 64 {
        (value >> distance) | u64::from(value << (64 - distance) != 0)
    } else {
        u64::from(value != 0)
    }
}

fn addition(a: u64, b: u64, mxcsr: u32, subtract: bool) -> Result<Float64Result, InvalidMxcsr> {
    if mxcsr & !0xffff != 0 {
        return Err(InvalidMxcsr);
    }
    // Select the original NaN before subtraction changes the second sign.
    if let Some(value) = nan_result(a, b) {
        return Ok(value);
    }
    let (a, b) = sources(a, b, mxcsr);
    let b = b ^ if subtract { SIGN } else { 0 };
    let (am, bm) = (a & !SIGN, b & !SIGN);
    let opposite = (a ^ b) & SIGN != 0;
    if am == INF && bm == INF && opposite {
        return result(SIGN | INF | QUIET, IE);
    }
    let pre = if denormal(a) || denormal(b) { DE } else { 0 };
    if am == INF {
        return result(a, pre);
    }
    if bm == INF {
        return result(b, pre);
    }
    if opposite && am == bm {
        return result(if mxcsr >> 13 & 3 == 1 { SIGN } else { 0 }, pre);
    }
    let (av, ae) = finite(a);
    let (bv, be) = finite(b);
    let exponent = ae.max(be);
    let av = shift_jam(av << 3, (exponent - ae) as u32);
    let bv = shift_jam(bv << 3, (exponent - be) as u32);
    let (value, sign) = if !opposite {
        (av + bv, a & SIGN)
    } else if am > bm {
        (av - bv, a & SIGN)
    } else {
        (bv - av, b & SIGN)
    };
    finish(u128::from(value), exponent - 3, sign, pre, mxcsr)
}

pub fn add_f64(a: u64, b: u64, mxcsr: u32) -> Result<Float64Result, InvalidMxcsr> {
    addition(a, b, mxcsr, false)
}
pub fn subtract_f64(a: u64, b: u64, mxcsr: u32) -> Result<Float64Result, InvalidMxcsr> {
    addition(a, b, mxcsr, true)
}

#[cfg(test)]
#[path = "simd_double_tests.rs"]
mod tests;

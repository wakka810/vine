//! Exact x87 arithmetic and architectural stack state, independent of any CPU
//! compiler or execution mirror. See README.md for the explicit supported set.

use std::sync::Mutex;

mod constants;
pub use constants::Constant;
mod logarithm;
pub use logarithm::log2_multiply;
mod arctangent;
pub use arctangent::arctangent;
mod cosine;
pub use cosine::cosine;
mod sine;
pub use sine::sine;
mod exponential;
pub use exponential::{exponential_minus_one, scale};
mod simd;
pub use simd::{
    Float32Conversion, IntegerConversion, InvalidRoundingMode, convert_i32_to_f32,
    round_f64_to_i32, truncate_f32_to_i32, truncate_f64_to_i32,
};
mod simd_multiply;
pub use simd_multiply::{Float32Multiplication, InvalidMxcsr, multiply_f32};
mod simd_addition;
pub use simd_addition::{Float32Addition, add_f32, subtract_f32};
mod simd_double;
pub use simd_double::{Float64Result, add_f64, multiply_f64, subtract_f64};
mod state;
pub use state::{Instruction, Location, Operand, Prepared, State, StoreValue, Trap};

pub const INVALID: u16 = 1;
pub const DENORMAL: u16 = 2;
pub const ZERO_DIVIDE: u16 = 4;
pub const OVERFLOW: u16 = 8;
pub const UNDERFLOW: u16 = 16;
pub const PRECISION: u16 = 32;
pub const STACK_FAULT: u16 = 64;
pub const EXCEPTION_SUMMARY: u16 = 128;
pub const C1: u16 = 0x200;
pub const BUSY: u16 = 0x8000;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Ext80 {
    pub significand: u64,
    pub sign_exponent: u16,
}

impl Ext80 {
    pub const INDEFINITE: Self = Self {
        significand: 0xc000_0000_0000_0000,
        sign_exponent: 0xffff,
    };
    pub const ZERO: Self = Self {
        significand: 0,
        sign_exponent: 0,
    };
    pub const ONE: Self = Self {
        significand: 0x8000_0000_0000_0000,
        sign_exponent: 0x3fff,
    };

    pub fn from_words(words: [u32; 3]) -> Self {
        Self {
            significand: u64::from(words[0]) | u64::from(words[1]) << 32,
            sign_exponent: words[2] as u16,
        }
    }
    pub fn words(self) -> [u32; 3] {
        [
            self.significand as u32,
            (self.significand >> 32) as u32,
            u32::from(self.sign_exponent),
        ]
    }
    pub fn exponent(self) -> u16 {
        self.sign_exponent & 0x7fff
    }
    pub fn is_nan(self) -> bool {
        self.exponent() == 0x7fff && self.significand << 1 != 0
    }
    pub fn is_signaling_nan(self) -> bool {
        self.is_nan() && self.significand & (1 << 62) == 0
    }
    pub fn is_denormal(self) -> bool {
        self.exponent() == 0 && self.significand != 0
    }
    pub fn is_unsupported(self) -> bool {
        self.exponent() != 0 && self.significand >> 63 == 0
    }
    fn canonical(self) -> Self {
        if self.exponent() == 0 && self.significand >> 63 != 0 {
            Self {
                sign_exponent: self.sign_exponent | 1,
                ..self
            }
        } else {
            self
        }
    }
    pub fn tag(self) -> u16 {
        if self.exponent() == 0 && self.significand == 0 {
            1
        } else if self.exponent() == 0 || self.exponent() == 0x7fff || self.is_unsupported() {
            2
        } else {
            0
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Arithmetic {
    Add,
    Subtract,
    Multiply,
    Divide,
    SquareRoot,
    RoundToInteger,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Format {
    Float32,
    Float64,
    Float80,
    Integer16,
    Integer32,
    Integer64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Unsupported {
    TranscendentalDomain,
    ReservedPrecision,
    UnmaskedRangeException,
    InvalidRegister,
    InvalidFormat,
    UncertainRounding,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct NumericResult {
    pub value: Ext80,
    pub bits: u64,
    pub exceptions: u16,
    pub round_up: bool,
}

static ARITHMETIC_LOCK: Mutex<()> = Mutex::new(());

unsafe extern "C" {
    fn vine_softfloat(
        op: u32,
        a_sig: u64,
        a_exp: u32,
        b_sig: u64,
        b_exp: u32,
        precision: u32,
        rounding: u32,
        out: *mut u64,
    );
}

fn calculate(op: u32, a: Ext80, b: Ext80, control: u16) -> Result<NumericResult, Unsupported> {
    let precision = match control >> 8 & 3 {
        0 => 32,
        2 => 64,
        3 => 80,
        _ => return Err(Unsupported::ReservedPrecision),
    };
    let _guard = ARITHMETIC_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let mut out = [0u64; 4];
    let a = if op <= 5 || op >= 10 {
        a.canonical()
    } else {
        a
    };
    let b = b.canonical();
    // SAFETY: fixed validated op/rounding/precision, and four writable words.
    unsafe {
        vine_softfloat(
            op,
            a.significand,
            u32::from(a.sign_exponent),
            b.significand,
            u32::from(b.sign_exponent),
            precision,
            u32::from(control >> 10 & 3),
            out.as_mut_ptr(),
        );
    }
    Ok(NumericResult {
        value: Ext80 {
            significand: out[0],
            sign_exponent: out[1] as u16,
        },
        bits: out[2],
        exceptions: out[3] as u16,
        round_up: out[3] >> 16 != 0,
    })
}

pub fn arithmetic(
    op: Arithmetic,
    a: Ext80,
    b: Ext80,
    control: u16,
) -> Result<NumericResult, Unsupported> {
    let binary = !matches!(op, Arithmetic::SquareRoot | Arithmetic::RoundToInteger);
    if a.is_unsupported() || (binary && b.is_unsupported()) {
        return Ok(NumericResult {
            value: Ext80::INDEFINITE,
            exceptions: INVALID,
            ..NumericResult::default()
        });
    }
    let mut result = calculate(op as u32, a, b, control)?;
    // Invalid operation has priority over a denormal operand.
    if result.exceptions & (INVALID | ZERO_DIVIDE) == 0
        && !a.is_nan()
        && !(binary && b.is_nan())
        && (a.is_denormal() || binary && b.is_denormal())
    {
        result.exceptions |= DENORMAL;
    }
    Ok(result)
}

pub fn load(format: Format, bits: u64, control: u16) -> Result<NumericResult, Unsupported> {
    let op = match format {
        Format::Float32 => 6,
        Format::Float64 => 7,
        Format::Integer16 | Format::Integer32 => 8,
        Format::Integer64 => 9,
        Format::Float80 => return Err(Unsupported::InvalidFormat),
    };
    let bits = if format == Format::Integer16 {
        bits as i16 as i32 as u32 as u64
    } else {
        bits
    };
    let mut result = calculate(
        op,
        Ext80 {
            significand: bits,
            sign_exponent: 0,
        },
        Ext80::ZERO,
        control | 0x300,
    )?;
    if format == Format::Float32 && bits as u32 & 0x7f80_0000 == 0 && bits as u32 & 0x007f_ffff != 0
        || format == Format::Float64
            && bits & 0x7ff0_0000_0000_0000 == 0
            && bits & 0x000f_ffff_ffff_ffff != 0
    {
        result.exceptions |= DENORMAL;
    }
    Ok(result)
}

pub fn store(format: Format, value: Ext80, control: u16) -> Result<NumericResult, Unsupported> {
    let op = match format {
        Format::Float32 => 10,
        Format::Float64 => 11,
        Format::Integer16 | Format::Integer32 => 12,
        Format::Integer64 => 13,
        Format::Float80 => {
            return Ok(NumericResult {
                value,
                ..NumericResult::default()
            });
        }
    };
    let mut result = calculate(
        op,
        if value.is_unsupported() {
            Ext80::INDEFINITE
        } else {
            value
        },
        Ext80::ZERO,
        control | 0x300,
    )?;
    if value.is_unsupported() {
        result.exceptions = INVALID;
    }
    if format == Format::Integer16 {
        if (result.bits as u32 as i32) < i32::from(i16::MIN)
            || (result.bits as u32 as i32) > i32::from(i16::MAX)
            || result.exceptions & INVALID != 0
        {
            result.bits = 0x8000;
            result.exceptions = INVALID;
            result.round_up = false;
        } else {
            result.bits &= 0xffff;
        }
    }
    Ok(result)
}

pub fn compare(a: Ext80, b: Ext80, quiet: bool) -> NumericResult {
    if a.is_unsupported() || b.is_unsupported() {
        return NumericResult {
            bits: 3,
            exceptions: INVALID,
            ..NumericResult::default()
        };
    }
    let mut result =
        calculate(if quiet { 14 } else { 15 }, a, b, 0x37f).expect("fixed control precision");
    if result.exceptions & (INVALID | ZERO_DIVIDE) == 0
        && !a.is_nan()
        && !b.is_nan()
        && (a.is_denormal() || b.is_denormal())
    {
        result.exceptions |= DENORMAL;
    }
    result
}

#[cfg(test)]
mod tests;

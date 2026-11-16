//! Typed x87 effects for the independent compiler. Descriptor decoding happens
//! once while compiling; generated execution never interprets a descriptor.
//!
//! Numerical helpers know only raw operands/control. Architectural stack,
//! pending exceptions, memory and commit belong to the generated operation.
use crate::ir::{MemoryOperand, Operand as GuestOperand};
use crate::x87::{Descriptor, OperandKind, Operation};
use vine_x87::{Arithmetic, Ext80, Format, NumericResult, Unsupported};

pub const NUMERIC_KERNEL_COUNT: u32 = 26;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Source {
    Register(u8),
    Memory(Format),
    Constant(vine_x87::Constant),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Destination {
    Register(u8),
    Memory(Format),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Effect {
    Load(Source),
    Store {
        destination: Destination,
        pop: bool,
    },
    Arithmetic {
        operation: Arithmetic,
        left: Source,
        right: Source,
        destination: u8,
        pop: bool,
    },
    Log2Multiply,
    Arctangent,
    Cosine,
    Sine,
    ExponentialMinusOne,
    Scale,
    Compare {
        right: Source,
        quiet: bool,
        pops: u8,
    },
    Exchange(u8),
    ChangeSign,
    Absolute,
    Free(u8),
    LoadControl,
    StoreControl,
    StoreStatus,
    ClearExceptions,
    Initialize,
    Wait,
}

/// A checked memory effect is ordered before arithmetic and after #MF. Store
/// permission checks precede arithmetic; journal reservation occurs only when
/// the arithmetic/exception effects actually permit an output.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MemoryEffect {
    pub operand: MemoryOperand,
    pub segment: u32,
    pub write: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Instruction {
    pub effect: Effect,
    pub memory: Option<MemoryEffect>,
    pub waiting: bool,
    pub record_location: bool,
    pub eip: u32,
    pub fop: u16,
}

fn format(kind: OperandKind) -> Option<Format> {
    Some(match kind {
        OperandKind::Register => return None,
        OperandKind::Float32 => Format::Float32,
        OperandKind::Float64 => Format::Float64,
        OperandKind::Float80 => Format::Float80,
        OperandKind::Integer16 => Format::Integer16,
        OperandKind::Integer32 => Format::Integer32,
        OperandKind::Integer64 => Format::Integer64,
    })
}

pub fn lower(
    descriptor: u32,
    fop: u16,
    segment: u32,
    operand: Option<GuestOperand>,
    eip: u32,
) -> Result<Instruction, String> {
    let d = Descriptor::decode(descriptor)?;
    let source = format(d.operand).map_or(
        Source::Register(if d.destination_register {
            0
        } else {
            d.register
        }),
        Source::Memory,
    );
    let destination = if d.destination_register {
        d.register
    } else {
        0
    };
    use Operation as O;
    let effect = match d.operation {
        O::Load => Effect::Load(source),
        O::Store => Effect::Store {
            destination: format(d.operand)
                .map_or(Destination::Register(d.register), Destination::Memory),
            pop: d.pops == 1,
        },
        O::Add
        | O::Subtract
        | O::Multiply
        | O::Divide
        | O::ReverseSubtract
        | O::ReverseDivide
        | O::SquareRoot
        | O::RoundToInteger => {
            let register = Source::Register(destination);
            let (left, right) = if matches!(d.operation, O::ReverseSubtract | O::ReverseDivide) {
                (source, register)
            } else {
                (register, source)
            };
            Effect::Arithmetic {
                operation: match d.operation {
                    O::Add => Arithmetic::Add,
                    O::Subtract | O::ReverseSubtract => Arithmetic::Subtract,
                    O::Multiply => Arithmetic::Multiply,
                    O::Divide | O::ReverseDivide => Arithmetic::Divide,
                    O::SquareRoot => Arithmetic::SquareRoot,
                    _ => Arithmetic::RoundToInteger,
                },
                left,
                right,
                destination,
                pop: d.pops == 1,
            }
        }
        O::Log2Multiply => Effect::Log2Multiply,
        O::Arctangent => Effect::Arctangent,
        O::Cosine => Effect::Cosine,
        O::Sine => Effect::Sine,
        O::ExponentialMinusOne => Effect::ExponentialMinusOne,
        O::Scale => Effect::Scale,
        O::Compare => Effect::Compare {
            right: source,
            quiet: d.quiet,
            pops: d.pops,
        },
        O::Exchange => Effect::Exchange(d.register),
        O::ChangeSign => Effect::ChangeSign,
        O::Absolute => Effect::Absolute,
        O::Free => Effect::Free(d.register),
        O::LoadConstant => Effect::Load(Source::Constant(
            vine_x87::Constant::from_id(d.register).expect("validated constant"),
        )),
        O::LoadControl => Effect::LoadControl,
        O::StoreControl => Effect::StoreControl,
        O::StoreStatus => Effect::StoreStatus,
        O::ClearExceptions => Effect::ClearExceptions,
        O::Initialize => Effect::Initialize,
        O::Wait => Effect::Wait,
    };
    let memory = if d.operand.bytes().is_some() {
        let Some(GuestOperand::Mem(operand)) = operand else {
            return Err("x87 IR memory operand missing".into());
        };
        if segment >= 6 || operand.width.bytes() as usize != d.operand.bytes().unwrap() {
            return Err("x87 IR memory width/segment mismatch".into());
        }
        Some(MemoryEffect {
            operand,
            segment,
            write: matches!(
                effect,
                Effect::Store { .. } | Effect::StoreControl | Effect::StoreStatus
            ),
        })
    } else {
        if segment != u32::MAX || operand.is_some() {
            return Err("x87 register IR has memory".into());
        }
        None
    };
    Ok(Instruction {
        effect,
        memory,
        waiting: d.waiting(),
        record_location: !matches!(
            effect,
            Effect::LoadControl
                | Effect::ClearExceptions
                | Effect::Initialize
                | Effect::Wait
                | Effect::StoreControl
                | Effect::StoreStatus
        ),
        eip,
        fop,
    })
}

/// This is backend-owned nonarchitectural storage. No pointer supplied by a
/// guest or an external caller is accepted as an output destination.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct NumericOutput {
    pub value: [u32; 3],
    pub bits: [u32; 2],
    pub exceptions: u32,
    pub round_up: u32,
}

impl From<NumericResult> for NumericOutput {
    fn from(r: NumericResult) -> Self {
        Self {
            value: r.value.words(),
            bits: [r.bits as u32, (r.bits >> 32) as u32],
            exceptions: u32::from(r.exceptions),
            round_up: u32::from(r.round_up),
        }
    }
}

pub const fn format_id(format: Format) -> u32 {
    match format {
        Format::Float32 => 0,
        Format::Float64 => 1,
        Format::Float80 => 2,
        Format::Integer16 => 3,
        Format::Integer32 => 4,
        Format::Integer64 => 5,
    }
}

fn numeric_format(value: u32) -> Result<Format, Unsupported> {
    match value {
        0 => Ok(Format::Float32),
        1 => Ok(Format::Float64),
        2 => Ok(Format::Float80),
        3 => Ok(Format::Integer16),
        4 => Ok(Format::Integer32),
        5 => Ok(Format::Integer64),
        _ => Err(Unsupported::InvalidFormat),
    }
}

/// 0..5 arithmetic, 6..11 load, 12..17 store, 18/19 signaling/quiet compare,
/// 20 combined y * log2(x), 21 atan2(y,x), with x in a and y in b;
/// 22 Intel FCOS(a), whose output bits==1 denotes finite range failure.
/// 23 Intel FSIN(a), with the same range sentinel and a pre-adjusted result
/// for an unmasked underflow (the exponent is biased by +24576).
/// 24 full64 F2XM1(a), 25 FSCALE(a,b), with pre-adjusted range results.
/// All validation and computation precede the caller's result publication.
pub fn numeric(
    operation: u32,
    control: u32,
    a: [u32; 3],
    b: [u32; 3],
) -> Result<NumericOutput, Unsupported> {
    numeric_mode(
        crate::numeric_mode::NumericMode::Strict,
        operation,
        control,
        a,
        b,
    )
}

pub fn numeric_mode(
    mode: crate::numeric_mode::NumericMode,
    operation: u32,
    control: u32,
    a: [u32; 3],
    b: [u32; 3],
) -> Result<NumericOutput, Unsupported> {
    if control > u32::from(u16::MAX) || a[2] > u32::from(u16::MAX) || b[2] > u32::from(u16::MAX) {
        return Err(Unsupported::InvalidFormat);
    }
    let a = Ext80::from_words(a);
    let b = Ext80::from_words(b);
    if mode != crate::numeric_mode::NumericMode::Strict
        && let Some(value) = crate::numeric_mode::approximate(mode, operation, control as u16, a, b)
    {
        return Ok(value.into());
    }
    let result = match operation {
        0..=5 => vine_x87::arithmetic(
            match operation {
                0 => Arithmetic::Add,
                1 => Arithmetic::Subtract,
                2 => Arithmetic::Multiply,
                3 => Arithmetic::Divide,
                4 => Arithmetic::SquareRoot,
                _ => Arithmetic::RoundToInteger,
            },
            a,
            b,
            control as u16,
        )?,
        6..=11 => vine_x87::load(
            numeric_format(operation - 6)?,
            a.significand,
            control as u16,
        )?,
        12..=17 => vine_x87::store(numeric_format(operation - 12)?, a, control as u16)?,
        18..=19 => vine_x87::compare(a, b, operation == 19),
        20 => vine_x87::log2_multiply(a, b, control as u16)?,
        21 => vine_x87::arctangent(a, b, control as u16)?,
        22 => vine_x87::cosine(a, control as u16)?,
        23 => vine_x87::sine(a, control as u16)?,
        24 => vine_x87::exponential_minus_one(a, control as u16)?,
        25 => vine_x87::scale(a, b, control as u16)?,
        _ => return Err(Unsupported::InvalidFormat),
    };
    Ok(result.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn numeric_output_is_fixed_and_exact_for_all_control_roundings() {
        assert_eq!(std::mem::size_of::<NumericOutput>(), 28);
        for precision in [0, 2, 3] {
            for rounding in 0..4 {
                let control = 0x7f | precision << 8 | rounding << 10;
                let a = Ext80 {
                    significand: 0x8000_0000_0000_0001,
                    sign_exponent: 0x3fff,
                };
                assert_eq!(
                    numeric(0, control, a.words(), a.words()).unwrap(),
                    NumericOutput::from(
                        vine_x87::arithmetic(Arithmetic::Add, a, a, control as u16).unwrap()
                    )
                );
            }
        }
        assert!(numeric(NUMERIC_KERNEL_COUNT, 0x37f, [0; 3], [0; 3]).is_err());
        assert!(numeric(0, 0x1_037f, [0; 3], [0; 3]).is_err());
        assert!(numeric(0, 0x37f, [0, 0, 0x1_0000], [0; 3]).is_err());
        assert!(numeric(0, 0x17f, [0; 3], [0; 3]).is_err());
        assert!(numeric(8, 0x37f, [0; 3], [0; 3]).is_err());
    }
}

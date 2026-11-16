//! x87 decoding records architectural operands, never a host C calling ABI.
use super::*;
use crate::x87::{Descriptor, OperandKind, Operation};

fn stack_register(register: X86Register) -> Option<u8> {
    (X86Register::ST0..=X86Register::ST7)
        .contains(&register)
        .then_some((register as u32).wrapping_sub(X86Register::ST0 as u32) as u8)
}

pub(super) fn lift(
    decoded: &X86Instruction,
    bytes: &[u8],
    eip: u32,
    len: u8,
) -> Option<Instruction> {
    use Mnemonic::*;
    let operation = match decoded.mnemonic() {
        Fld | Fild => Operation::Load,
        Fst | Fstp | Fist | Fistp => Operation::Store,
        Fadd | Faddp | Fiadd => Operation::Add,
        Fsub | Fsubp | Fisub => Operation::Subtract,
        Fsubr | Fsubrp | Fisubr => Operation::ReverseSubtract,
        Fmul | Fmulp | Fimul => Operation::Multiply,
        Fdiv | Fdivp | Fidiv => Operation::Divide,
        Fdivr | Fdivrp | Fidivr => Operation::ReverseDivide,
        Fcom | Fcomp | Fcompp | Fucom | Fucomp | Fucompp | Ficom | Ficomp => Operation::Compare,
        Fxch => Operation::Exchange,
        Fchs => Operation::ChangeSign,
        Fabs => Operation::Absolute,
        Fldcw => Operation::LoadControl,
        Fnstcw => Operation::StoreControl,
        Fnstsw => Operation::StoreStatus,
        Fnclex => Operation::ClearExceptions,
        Fninit => Operation::Initialize,
        Wait => Operation::Wait,
        Fldz | Fld1 | Fldl2t | Fldl2e | Fldpi | Fldlg2 | Fldln2 => Operation::LoadConstant,
        Fsqrt => Operation::SquareRoot,
        Frndint => Operation::RoundToInteger,
        Ffree => Operation::Free,
        Fyl2x => Operation::Log2Multiply,
        Fpatan => Operation::Arctangent,
        Fcos => Operation::Cosine,
        Fsin => Operation::Sine,
        F2xm1 => Operation::ExponentialMinusOne,
        Fscale => Operation::Scale,
        _ => return None,
    };
    let failed = || Some(unsupported(eip, len, UnsupportedReason::Operand));
    let mut descriptor = Descriptor {
        operation,
        operand: OperandKind::Register,
        register: 0,
        destination_register: false,
        pops: match decoded.mnemonic() {
            Fstp | Fistp | Faddp | Fsubp | Fsubrp | Fmulp | Fdivp | Fdivrp | Fcomp | Fucomp
            | Ficomp => 1,
            Fcompp | Fucompp => 2,
            _ => 0,
        },
        quiet: matches!(decoded.mnemonic(), Fucom | Fucomp | Fucompp),
    };
    let mut operands = Vec::new();
    let mut segment = crate::x87::NO_SEGMENT;
    for index in 0..decoded.op_count() {
        match decoded.op_kind(index) {
            OpKind::Memory => {
                let Some(value @ Operand::Mem(memory)) = operand(decoded, index, Width::W32, false)
                else {
                    return failed();
                };
                let integer = matches!(
                    decoded.mnemonic(),
                    Fild | Fist
                        | Fistp
                        | Fiadd
                        | Fisub
                        | Fisubr
                        | Fimul
                        | Fidiv
                        | Fidivr
                        | Ficom
                        | Ficomp
                        | Fldcw
                        | Fnstcw
                        | Fnstsw
                );
                descriptor.operand = match (integer, memory.width) {
                    (false, Width::W32) => OperandKind::Float32,
                    (false, Width::W64) => OperandKind::Float64,
                    (false, Width::W80) => OperandKind::Float80,
                    (true, Width::W16) => OperandKind::Integer16,
                    (true, Width::W32) => OperandKind::Integer32,
                    (true, Width::W64) => OperandKind::Integer64,
                    _ => return failed(),
                };
                segment = match decoded.memory_segment() {
                    X86Register::ES => 0,
                    X86Register::SS => 2,
                    X86Register::DS => 3,
                    X86Register::FS => 4,
                    _ => return failed(),
                };
                operands.push(value);
            }
            OpKind::Register => {
                let register = decoded.op_register(index);
                if operation == Operation::StoreStatus && register == X86Register::AX {
                    return Some(Instruction {
                        eip,
                        len,
                        opcode: Opcode::X87StatusAx,
                        width: Width::W16,
                        operands: Vec::new(),
                        condition: None,
                        flags_read: 0,
                        flags_written: 0,
                        flags_undefined: 0,
                        unsupported: None,
                    });
                }
                let Some(register) = stack_register(register) else {
                    return failed();
                };
                if decoded.op_count() == 2 && decoded.op_kind(1 - index) == OpKind::Memory {
                    if register != 0 {
                        return failed();
                    }
                    continue;
                }
                if decoded.op_count() == 2 {
                    let Some(other) = stack_register(decoded.op_register(1 - index)) else {
                        return failed();
                    };
                    if register != 0 && other != 0 {
                        return failed();
                    }
                    if index == 0
                        && (register != 0
                            || descriptor.pops == 1
                                && matches!(
                                    operation,
                                    Operation::Add
                                        | Operation::Subtract
                                        | Operation::Multiply
                                        | Operation::Divide
                                        | Operation::ReverseSubtract
                                        | Operation::ReverseDivide
                                ))
                    {
                        descriptor.destination_register = true;
                        descriptor.register = register;
                    } else if index == 1 && !descriptor.destination_register {
                        descriptor.register = register;
                    }
                } else {
                    descriptor.register = register;
                }
            }
            _ => return failed(),
        }
    }
    if matches!(decoded.mnemonic(), Fcompp | Fucompp) {
        descriptor.register = 1;
    }
    if operation == Operation::LoadConstant {
        descriptor.register = match decoded.mnemonic() {
            Fldz => vine_x87::Constant::Zero,
            Fld1 => vine_x87::Constant::One,
            Fldl2t => vine_x87::Constant::Log2Ten,
            Fldl2e => vine_x87::Constant::Log2E,
            Fldpi => vine_x87::Constant::Pi,
            Fldlg2 => vine_x87::Constant::Log10Two,
            Fldln2 => vine_x87::Constant::LnTwo,
            _ => unreachable!(),
        } as u8;
    }
    if matches!(operation, Operation::Log2Multiply | Operation::Arctangent) {
        descriptor.register = 1;
        descriptor.destination_register = true;
        descriptor.pops = 1;
    }
    let Ok(descriptor) = descriptor.encode() else {
        return failed();
    };
    let at = bytes
        .iter()
        .position(|byte| !matches!(byte, 0x66 | 0x26 | 0x2e | 0x36 | 0x3e | 0x64))
        .unwrap_or(0);
    let fop = if operation == Operation::Wait {
        0
    } else {
        u16::from(bytes[at] & 7) << 8 | u16::from(bytes[at + 1])
    };
    let width = operands
        .first()
        .map_or(Width::W32, |operand| match operand {
            Operand::Mem(memory) => memory.width,
            _ => unreachable!(),
        });
    Some(Instruction {
        eip,
        len,
        opcode: Opcode::X87 {
            descriptor,
            fop,
            segment,
        },
        width,
        operands,
        condition: None,
        flags_read: 0,
        flags_written: 0,
        flags_undefined: 0,
        unsupported: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fp_ir::{Effect, Source};
    use vine_x87::{Arithmetic, Format};

    #[test]
    fn reverse_register_arithmetic_preserves_both_sources_destination_and_pop() {
        for index in 0..8 {
            for (first, second, operation, numeric, reverse, pops) in [
                (
                    0xd8,
                    0xe0,
                    Operation::Subtract,
                    Arithmetic::Subtract,
                    false,
                    0,
                ),
                (
                    0xd8,
                    0xe8,
                    Operation::ReverseSubtract,
                    Arithmetic::Subtract,
                    true,
                    0,
                ),
                (0xd8, 0xf0, Operation::Divide, Arithmetic::Divide, false, 0),
                (
                    0xd8,
                    0xf8,
                    Operation::ReverseDivide,
                    Arithmetic::Divide,
                    true,
                    0,
                ),
                (
                    0xdc,
                    0xe8,
                    Operation::Subtract,
                    Arithmetic::Subtract,
                    false,
                    0,
                ),
                (
                    0xdc,
                    0xe0,
                    Operation::ReverseSubtract,
                    Arithmetic::Subtract,
                    true,
                    0,
                ),
                (0xdc, 0xf8, Operation::Divide, Arithmetic::Divide, false, 0),
                (
                    0xdc,
                    0xf0,
                    Operation::ReverseDivide,
                    Arithmetic::Divide,
                    true,
                    0,
                ),
                (
                    0xde,
                    0xe8,
                    Operation::Subtract,
                    Arithmetic::Subtract,
                    false,
                    1,
                ),
                (
                    0xde,
                    0xe0,
                    Operation::ReverseSubtract,
                    Arithmetic::Subtract,
                    true,
                    1,
                ),
                (0xde, 0xf8, Operation::Divide, Arithmetic::Divide, false, 1),
                (
                    0xde,
                    0xf0,
                    Operation::ReverseDivide,
                    Arithmetic::Divide,
                    true,
                    1,
                ),
            ] {
                let bytes = [first, second + index];
                let instruction = decode_one(0x1000, &bytes);
                let Opcode::X87 {
                    descriptor,
                    fop,
                    segment,
                } = instruction.opcode
                else {
                    panic!("{bytes:x?}: {instruction:?}");
                };
                let decoded = Descriptor::decode(descriptor).unwrap();
                assert_eq!(decoded.operation, operation, "{bytes:x?}");
                assert_eq!(decoded.pops, pops);
                let destination = if first == 0xd8 { 0 } else { index };
                let other = if first == 0xd8 { index } else { 0 };
                let (left, right) = if reverse {
                    (other, destination)
                } else {
                    (destination, other)
                };
                let plan = crate::fp_ir::lower(descriptor, fop, segment, None, 0x1000).unwrap();
                assert_eq!(
                    plan.effect,
                    Effect::Arithmetic {
                        operation: numeric,
                        left: Source::Register(left),
                        right: Source::Register(right),
                        destination,
                        pop: pops == 1,
                    },
                    "{bytes:x?}"
                );
                assert!(plan.memory.is_none());
            }
        }
    }

    #[test]
    fn reverse_memory_arithmetic_has_memory_left_and_st0_right() {
        for (first, kind, format, width) in [
            (0xd8, OperandKind::Float32, Format::Float32, Width::W32),
            (0xdc, OperandKind::Float64, Format::Float64, Width::W64),
            (0xda, OperandKind::Integer32, Format::Integer32, Width::W32),
            (0xde, OperandKind::Integer16, Format::Integer16, Width::W16),
        ] {
            for (second, operation, numeric) in [
                (0x28, Operation::ReverseSubtract, Arithmetic::Subtract),
                (0x38, Operation::ReverseDivide, Arithmetic::Divide),
            ] {
                let instruction = decode_one(0x1000, &[0x64, first, second]);
                let Opcode::X87 {
                    descriptor,
                    fop,
                    segment,
                } = instruction.opcode
                else {
                    panic!("{instruction:?}");
                };
                let decoded = Descriptor::decode(descriptor).unwrap();
                assert_eq!(decoded.operation, operation);
                assert_eq!(decoded.operand, kind);
                assert_eq!(decoded.pops, 0);
                let plan = crate::fp_ir::lower(
                    descriptor,
                    fop,
                    segment,
                    instruction.operands.first().copied(),
                    0x1000,
                )
                .unwrap();
                assert_eq!(
                    plan.effect,
                    Effect::Arithmetic {
                        operation: numeric,
                        left: Source::Memory(format),
                        right: Source::Register(0),
                        destination: 0,
                        pop: false,
                    }
                );
                let memory = plan.memory.unwrap();
                assert_eq!(memory.segment, 4);
                assert_eq!(memory.operand.width, width);
                assert!(!memory.write);
            }
        }
    }

    #[test]
    fn cosine_keeps_one_stack_operand_without_pop() {
        let instruction = decode_one(0x1000, &[0xd9, 0xff]);
        let Opcode::X87 {
            descriptor,
            fop,
            segment,
        } = instruction.opcode
        else {
            panic!("{instruction:?}")
        };
        let descriptor = Descriptor::decode(descriptor).unwrap();
        assert_eq!(descriptor.operation, Operation::Cosine);
        assert_eq!(descriptor.register, 0);
        assert_eq!(descriptor.pops, 0);
        assert!(!descriptor.destination_register);
        assert!(descriptor.waiting());
        assert_eq!(fop, 0x1ff);
        assert_eq!(segment, crate::x87::NO_SEGMENT);
        assert!(instruction.operands.is_empty());
        let plan =
            crate::fp_ir::lower(descriptor.encode().unwrap(), fop, segment, None, 0x1000).unwrap();
        assert_eq!(plan.effect, crate::fp_ir::Effect::Cosine);
        assert!(plan.memory.is_none());
    }
    #[test]
    fn sine_keeps_raw_stack_location_and_precise_waiting_effect() {
        let instruction = decode_one(0x1234, &[0xd9, 0xfe]);
        let Opcode::X87 {
            descriptor,
            fop,
            segment,
        } = instruction.opcode
        else {
            panic!("{instruction:?}");
        };
        let descriptor = Descriptor::decode(descriptor).unwrap();
        assert_eq!(descriptor.operation, Operation::Sine);
        assert_eq!(descriptor.register, 0);
        assert_eq!(descriptor.pops, 0);
        assert!(!descriptor.destination_register);
        assert!(descriptor.waiting());
        assert_eq!(fop, 0x1fe);
        assert_eq!(segment, crate::x87::NO_SEGMENT);
        assert!(instruction.operands.is_empty());
        let plan =
            crate::fp_ir::lower(descriptor.encode().unwrap(), fop, segment, None, 0x1234).unwrap();
        assert_eq!(plan.effect, crate::fp_ir::Effect::Sine);
        assert!(plan.memory.is_none());
        assert_eq!(plan.eip, 0x1234);
    }
    #[test]
    fn logarithm_multiply_keeps_implicit_operands_and_single_pop() {
        for (second, operation) in [
            (0xf1, Operation::Log2Multiply),
            (0xf3, Operation::Arctangent),
        ] {
            let instruction = decode_one(0x1000, &[0xd9, second]);
            let Opcode::X87 {
                descriptor,
                fop,
                segment,
            } = instruction.opcode
            else {
                panic!("{instruction:?}");
            };
            let descriptor = Descriptor::decode(descriptor).unwrap();
            assert_eq!(descriptor.operation, operation);
            assert_eq!(descriptor.register, 1);
            assert!(descriptor.destination_register);
            assert_eq!(descriptor.pops, 1);
            assert_eq!(fop, 0x100 | u16::from(second));
            assert_eq!(segment, crate::x87::NO_SEGMENT);
            assert!(instruction.operands.is_empty());
            assert!(descriptor.waiting());
        }
    }

    #[test]
    fn exponential_and_scale_keep_implicit_stack_operands_without_pop() {
        for (second, operation, effect) in [
            (
                0xf0,
                Operation::ExponentialMinusOne,
                crate::fp_ir::Effect::ExponentialMinusOne,
            ),
            (0xfd, Operation::Scale, crate::fp_ir::Effect::Scale),
        ] {
            let instruction = decode_one(0x1234, &[0xd9, second]);
            let Opcode::X87 {
                descriptor,
                fop,
                segment,
            } = instruction.opcode
            else {
                panic!("{instruction:?}")
            };
            let d = Descriptor::decode(descriptor).unwrap();
            assert_eq!(d.operation, operation);
            assert_eq!((d.register, d.pops, d.destination_register), (0, 0, false));
            assert!(d.waiting() && instruction.operands.is_empty());
            assert_eq!(fop, 0x100 | u16::from(second));
            let plan = crate::fp_ir::lower(descriptor, fop, segment, None, 0x1234).unwrap();
            assert_eq!(plan.effect, effect);
            assert!(plan.record_location && plan.memory.is_none());
        }
    }
    #[test]
    fn all_constant_encodings_keep_distinct_ids_and_no_memory_operand() {
        for (second, id) in [
            (0xe8, 1),
            (0xe9, 2),
            (0xea, 3),
            (0xeb, 4),
            (0xec, 5),
            (0xed, 6),
            (0xee, 0),
        ] {
            let instruction = decode_one(0x1000, &[0xd9, second]);
            let Opcode::X87 {
                descriptor,
                fop,
                segment,
            } = instruction.opcode
            else {
                panic!("{instruction:?}");
            };
            let descriptor = Descriptor::decode(descriptor).unwrap();
            assert_eq!(descriptor.operation, Operation::LoadConstant);
            assert_eq!(descriptor.register, id);
            assert_eq!(fop, 0x100 | u16::from(second));
            assert_eq!(segment, crate::x87::NO_SEGMENT);
            assert!(instruction.operands.is_empty());
        }
    }
    #[test]
    fn reached_audio_decodes_memory_and_pop_operands() {
        for bytes in [
            &[0xd8, 0x0e][..],
            &[0xd8, 0x01],
            &[0xde, 0xe9],
            &[0xde, 0xc0],
            &[0xd9, 0x02],
            &[0xd9, 0x5e, 0xfc],
        ] {
            let instruction = decode_one(0x1000, bytes);
            assert!(
                matches!(instruction.opcode, Opcode::X87 { .. }),
                "{bytes:x?}: {instruction:?}"
            );
        }
        let Opcode::X87 { descriptor, .. } = decode_one(0x1000, &[0xde, 0xe9]).opcode else {
            unreachable!()
        };
        let value = Descriptor::decode(descriptor).unwrap();
        assert_eq!(value.operation, Operation::Subtract);
        assert!(value.destination_register);
        assert_eq!(value.register, 1);
        assert_eq!(value.pops, 1);
        assert_eq!(
            decode_one(0x1000, &[0xdf, 0xe0]).opcode,
            Opcode::X87StatusAx
        );
    }

    #[test]
    fn stack_copy_operands_retain_destination_and_source_indices() {
        for index in 0..8 {
            for (first, second, operation, pops) in [
                (0xdd, 0xd0, Operation::Store, 0),
                (0xdd, 0xd8, Operation::Store, 1),
                (0xd9, 0xc0, Operation::Load, 0),
                (0xd9, 0xc8, Operation::Exchange, 0),
            ] {
                let instruction = decode_one(0x1000, &[first, second + index]);
                let Opcode::X87 { descriptor, .. } = instruction.opcode else {
                    panic!("{instruction:?}");
                };
                let descriptor = Descriptor::decode(descriptor).unwrap();
                assert_eq!(descriptor.operation, operation);
                assert_eq!(descriptor.register, index);
                assert_eq!(descriptor.pops, pops);
                assert!(!descriptor.destination_register);
            }
        }
    }
}

//! Conservative flags liveness inside an admitted straight-line block.
//!
//! This pass does not remove instructions, memory operations, or retirement.
//! Code generation may omit a defined flag write only when its returned mask
//! excludes that flag. Use it only when the block's complete instruction budget
//! is available and executable mappings remain valid until a block boundary.
//! Partial budgets preserve every write; only exact recovery recipes may defer it.

use crate::ir::{Block, Instruction, Opcode, Operand, Region, Segment, SseOpcode, Width};
use std::collections::{BTreeMap, BTreeSet};

/// A nonfaulting, invocation-local proof candidate. Endpoints include the base
/// itself, so a successful one-page proof also excludes 32-bit address wrap.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AffineRange {
    pub base: u8,
    pub minimum: i32,
    pub maximum: i32,
    pub permissions: u32,
    pub references: usize,
    pub distinct_offsets: usize,
}

/// Select spatial reuse, without PC, title, profile or concrete guest values.
/// Bounds are deliberately small: two base-only scalar ranges, at least two
/// distinct displacements, and a complete range contained in one guest page.
pub fn affine_ranges(region: &Region) -> Vec<AffineRange> {
    let mut candidates: BTreeMap<u8, (AffineRange, BTreeSet<i32>)> = BTreeMap::new();
    for instruction in region.blocks.values().flat_map(|b| &b.instructions) {
        if matches!(
            instruction.opcode,
            Opcode::Lea
                | Opcode::Nop
                | Opcode::DiagnosticStop
                | Opcode::Unsupported
                | Opcode::Mmx(_)
                | Opcode::Sse(_)
                | Opcode::X87 { .. }
                | Opcode::Pop
                | Opcode::String(_, _)
        ) {
            continue;
        }
        for (index, operand) in instruction.operands.iter().enumerate() {
            let Operand::Mem(m) = operand else { continue };
            let Some(base) = m.base else { continue };
            if m.index.is_some() || m.segment == Segment::Fs || m.width.bytes() > 4 {
                continue;
            }
            let displacement = m.displacement as i32;
            let last = i64::from(displacement) + i64::from(m.width.bytes()) - 1;
            let (range, offsets) = candidates.entry(base).or_insert_with(|| {
                (
                    AffineRange {
                        base,
                        minimum: 0,
                        maximum: 0,
                        permissions: 0,
                        references: 0,
                        distinct_offsets: 0,
                    },
                    BTreeSet::new(),
                )
            });
            range.minimum = range.minimum.min(displacement);
            range.maximum = range.maximum.max(last.min(i64::from(i32::MAX)) as i32);
            let write = index == 0
                && matches!(
                    instruction.opcode,
                    Opcode::Mov
                        | Opcode::Add
                        | Opcode::Adc
                        | Opcode::Sub
                        | Opcode::Sbb
                        | Opcode::And
                        | Opcode::Or
                        | Opcode::Xor
                        | Opcode::Inc
                        | Opcode::Dec
                        | Opcode::Neg
                        | Opcode::Not
                        | Opcode::Shl
                        | Opcode::Shr
                        | Opcode::Sar
                        | Opcode::Shrd
                        | Opcode::Setcc
                );
            let read = !write || !matches!(instruction.opcode, Opcode::Mov | Opcode::Setcc);
            range.permissions |= u32::from(read) | (u32::from(write) << 1);
            range.references += usize::from(read) + usize::from(write);
            offsets.insert(displacement);
        }
    }
    let mut ranges: Vec<_> = candidates
        .into_values()
        .filter_map(|(mut range, offsets)| {
            range.distinct_offsets = offsets.len();
            (offsets.len() >= 2 && i64::from(range.maximum) - i64::from(range.minimum) < 4096)
                .then_some(range)
        })
        .collect();
    ranges.sort_by_key(|r| {
        (
            std::cmp::Reverse(r.distinct_offsets),
            std::cmp::Reverse(r.references),
            r.base,
        )
    });
    ranges.truncate(2);
    ranges
}

/// Conservative architectural GPR writes, including partial/conditional
/// writes and implicit stack/division effects. Invalidating after completion
/// avoids certifying an old source value then retaining it after assignment.
pub fn written_gprs(instruction: &Instruction) -> u8 {
    if instruction.opcode == Opcode::Loop {
        return 1 << 1;
    }
    if instruction.opcode == Opcode::Rdtsc {
        return 0b0101;
    }
    if instruction.opcode == Opcode::Cpuid {
        return 0b1111;
    }
    if instruction.opcode == Opcode::Xchg {
        return instruction.operands.iter().fold(0, |mask, operand| {
            mask | match operand {
                Operand::Reg(register) => 1 << register.index,
                _ => 0,
            }
        });
    }
    if instruction.opcode == Opcode::X87StatusAx {
        return 1;
    }
    if instruction.opcode == Opcode::SignExtendPair {
        return 1 << 2;
    }
    let mut mask = 0;
    if !matches!(
        instruction.opcode,
        Opcode::Cmp
            | Opcode::Test
            | Opcode::Nop
            | Opcode::Ud2
            | Opcode::DiagnosticStop
            | Opcode::Unsupported
            | Opcode::Rdtsc
            | Opcode::Jcc
            | Opcode::Jmp
            | Opcode::Call
            | Opcode::Ret
            | Opcode::Push
            | Opcode::Div
            | Opcode::Idiv
            | Opcode::Mul
            | Opcode::ImulWide
    ) && let Some(Operand::Reg(r)) = instruction.operands.first()
    {
        mask |= 1 << r.index;
    }
    if matches!(
        instruction.opcode,
        Opcode::Push | Opcode::Pop | Opcode::Call | Opcode::Ret | Opcode::Leave
    ) {
        mask |= 1 << 4;
    }
    if instruction.opcode == Opcode::Leave {
        mask |= 1 << 5;
    }
    if matches!(
        instruction.opcode,
        Opcode::Div | Opcode::Idiv | Opcode::Mul | Opcode::ImulWide
    ) {
        mask |= 1;
        if instruction.width != Width::W8 {
            mask |= 1 << 2;
        }
    }
    if let Opcode::String(operation, repeat) = instruction.opcode {
        mask |= 1 << 7;
        if matches!(
            operation,
            crate::ir::StringOpcode::Movs | crate::ir::StringOpcode::Cmps
        ) {
            mask |= 1 << 6;
        }
        if repeat {
            mask |= 1 << 1;
        }
    }
    mask
}

/// Returns the architectural flags that each instruction must define.
/// All state is observable at exits and immediately before a possible fault.
pub fn flags_needed_after(block: &Block) -> Vec<u32> {
    flags_needed_with_live(block, u32::MAX)
}

/// Caller must prove that excluded outgoing flags cannot be observed, including
/// budget/fault exits in successors. Ordinary block-local callers keep all bits.
pub(crate) fn flags_needed_with_live(block: &Block, mut live: u32) -> Vec<u32> {
    let mut needed = vec![0; block.instructions.len()];
    for (index, instruction) in block.instructions.iter().enumerate().rev() {
        // A store can complete and then exit for code invalidation. That exit
        // observes this instruction's new flags as well as its committed bytes.
        needed[index] = if can_fault(instruction) {
            instruction.flags_written
        } else {
            live & instruction.flags_written
        };
        live = (live & !unconditional_flag_writes(instruction)) | instruction.flags_read;
        if can_fault(instruction) {
            live = u32::MAX;
        }
    }
    needed
}

pub(crate) fn unconditional_flag_writes(instruction: &Instruction) -> u32 {
    match instruction.opcode {
        Opcode::Add
        | Opcode::Adc
        | Opcode::Sub
        | Opcode::Sbb
        | Opcode::Cmp
        | Opcode::Test
        | Opcode::And
        | Opcode::Or
        | Opcode::Xor
        | Opcode::Inc
        | Opcode::Dec
        | Opcode::Neg
        | Opcode::Imul
        | Opcode::ImulWide
        | Opcode::Mul
        | Opcode::Sahf => instruction.flags_written,
        // In particular, a zero shift count preserves all prior flags. Do not
        // treat the decoder's may-write mask as an unconditional definition.
        // Undefined flags are also retained: the emitter may preserve them.
        _ => 0,
    }
}

pub(crate) fn can_fault(instruction: &Instruction) -> bool {
    matches!(
        instruction.opcode,
        Opcode::Sse(
            SseOpcode::Cvttss2si
                | SseOpcode::Cvttsd2si
                | SseOpcode::Cvtdq2ps
                | SseOpcode::Cvttps2dq
                | SseOpcode::Mulps
                | SseOpcode::Addps
                | SseOpcode::Subps
                | SseOpcode::Cvtsd2si
                | SseOpcode::Addsd
                | SseOpcode::Subsd
                | SseOpcode::Mulsd
                | SseOpcode::Addpd
                | SseOpcode::Subpd
                | SseOpcode::Mulpd
        ) | Opcode::Push
            | Opcode::Pop
            | Opcode::Leave
            | Opcode::Div
            | Opcode::Idiv
            | Opcode::String(_, _)
            | Opcode::Call
            | Opcode::Ret
            | Opcode::Mmx(_)
            | Opcode::X87 { .. }
            | Opcode::Ud2
            | Opcode::Rdtsc
            | Opcode::DiagnosticStop
            | Opcode::Unsupported
    ) || (!matches!(instruction.opcode, Opcode::Lea | Opcode::Nop)
        && instruction
            .operands
            .iter()
            .any(|operand| matches!(operand, Operand::Mem(_))))
}

#[cfg(test)]
mod tests {
    use crate::decode::decode_one;

    use super::*;

    fn block(bytes: &[&[u8]]) -> Block {
        let mut eip = 0x1000;
        let instructions = bytes
            .iter()
            .map(|bytes| {
                let instruction = decode_one(eip, bytes);
                eip = instruction.next_eip();
                instruction
            })
            .collect();
        Block {
            start: 0x1000,
            instructions,
            successors: vec![eip],
        }
    }

    #[test]
    fn sahf_overwrites_status_flags_without_losing_of_or_fault_visibility() {
        let sahf = decode_one(0x1000, &[0x9e]);
        assert_eq!(written_gprs(&sahf), 0);
        assert_eq!(unconditional_flag_writes(&sahf), 0xd5);
        let before_fault = block(&[&[0x83, 0xc0, 1], &[0x9e], &[0x8b, 0x03]]);
        assert_eq!(flags_needed_after(&before_fault), vec![0x800, 0xd5, 0]);
        let before_x87 = block(&[&[0x83, 0xc0, 1], &[0x9e], &[0xd9, 0xff]]);
        assert_eq!(flags_needed_after(&before_x87), vec![0x800, 0xd5, 0]);
        let overwrite = block(&[&[0x83, 0xc0, 1], &[0x9e], &[0x83, 0xf8, 0]]);
        assert_eq!(flags_needed_after(&overwrite), vec![0, 0, 0x8d5]);
        let consumed = block(&[&[0x9e], &[0x0f, 0x96, 0xc3], &[0x83, 0xc0, 1]]);
        assert_eq!(flags_needed_after(&consumed), vec![0x41, 0, 0x8d5]);
        let branch = block(&[&[0x9e], &[0x72, 0]]);
        assert_eq!(flags_needed_after(&branch), vec![0xd5, 0]);
    }
    #[test]
    fn cpuid_invalidates_all_four_outputs_without_changing_flags() {
        assert_eq!(written_gprs(&decode_one(0x1000, &[0x0f, 0xa2])), 0b1111);
        let block = block(&[&[0x83, 0xc0, 1], &[0x0f, 0xa2], &[0x8b, 0x03]]);
        assert_eq!(flags_needed_after(&block), vec![0x8d5, 0, 0]);
    }

    #[test]
    fn timestamp_handoff_observes_prior_flags_and_writes_full_output_pair() {
        assert_eq!(
            written_gprs(&decode_one(0x1000, &[0x66, 0x0f, 0x31])),
            0b0101
        );
        let block = block(&[&[0x83, 0xc0, 1], &[0x0f, 0x31], &[0x83, 0xe8, 1]]);
        assert_eq!(flags_needed_after(&block), vec![0x8d5, 0, 0x8d5]);
    }

    #[test]
    fn scalar_simd_exception_observes_prior_integer_flags() {
        for conversion in [&[0xf2, 0x0f, 0x2c, 0xc0][..], &[0xf3, 0x0f, 0x2c, 0xc7][..]] {
            let block = block(&[&[0x83, 0xc0, 1], conversion, &[0x83, 0xe8, 2]]);
            assert_eq!(flags_needed_after(&block), vec![0x8d5, 0, 0x8d5]);
        }
    }

    #[test]
    fn packed_simd_register_exception_keeps_flags_before_later_dead_writes() {
        for conversion in [
            &[0x0f, 0x5b, 0xc1][..],
            &[0xf3, 0x0f, 0x5b, 0xc1],
            &[0x0f, 0x59, 0xc1],
            &[0x0f, 0x58, 0xc1],
            &[0x0f, 0x5c, 0xc1],
        ] {
            let block = block(&[
                &[0x83, 0xc0, 1],
                conversion,
                &[0x83, 0xeb, 1],
                &[0x83, 0xfa, 0],
            ]);
            // The SUB flags remain dead, so complete-block code generation
            // still uses the optimized path. #XM must observe the earlier ADD.
            assert_eq!(flags_needed_after(&block), vec![0x8d5, 0, 0, 0x8d5]);
        }
    }

    #[test]
    fn exchange_invalidates_both_full_and_partial_gpr_destinations() {
        for (bytes, expected) in [
            (&[0x94][..], (1 << 0) | (1 << 4)),
            (&[0x66, 0x87, 0xcb][..], (1 << 1) | (1 << 3)),
            (&[0x86, 0xe0][..], 1),
            (&[0x86, 0xcc][..], (1 << 0) | (1 << 1)),
        ] {
            assert_eq!(written_gprs(&decode_one(0x1000, bytes)), expected);
        }
        let block = block(&[&[0x83, 0xc0, 1], &[0x94], &[0x8b, 0x03]]);
        assert_eq!(flags_needed_after(&block), vec![0x8d5, 0, 0]);
    }

    #[test]
    fn removes_only_overwritten_register_arithmetic_flags() {
        let block = block(&[&[0x83, 0xc0, 1], &[0x83, 0xe8, 2]]);
        assert_eq!(flags_needed_after(&block), vec![0, 0x8d5]);
    }

    #[test]
    fn carry_input_and_partial_flag_writes_remain_live() {
        let add_adc = block(&[&[0x83, 0xc0, 1], &[0x83, 0xd0, 2]]);
        assert_eq!(flags_needed_after(&add_adc), vec![1, 0x8d5]);
        let add_inc = block(&[&[0x83, 0xc0, 1], &[0x40]]);
        assert_eq!(flags_needed_after(&add_inc), vec![1, 0x8d4]);
    }

    #[test]
    fn memory_fault_observes_flags_from_preceding_instruction() {
        let block = block(&[&[0x83, 0xc0, 1], &[0x8b, 0x03], &[0x83, 0xe8, 2]]);
        assert_eq!(flags_needed_after(&block), vec![0x8d5, 0, 0x8d5]);
    }

    #[test]
    fn zero_variable_shift_cannot_kill_preceding_flags() {
        let block = block(&[&[0x83, 0xc0, 1], &[0xd3, 0xe0]]);
        assert_eq!(flags_needed_after(&block)[0], 0x8d5);
    }

    #[test]
    fn lea_has_no_memory_fault_barrier() {
        let block = block(&[&[0x83, 0xc0, 1], &[0x8d, 0x03], &[0x83, 0xe8, 2]]);
        assert_eq!(flags_needed_after(&block), vec![0, 0, 0x8d5]);
    }

    #[test]
    fn double_numeric_register_xm_preserves_preceding_flags() {
        for prefix in [0x66, 0xf2] {
            for byte in [0x58, 0x59, 0x5c] {
                let block = block(&[
                    &[0x83, 0xc0, 1],
                    &[prefix, 0x0f, byte, 0xc1],
                    &[0x83, 0xf8, 2],
                ]);
                assert_eq!(flags_needed_after(&block)[0], 0x8d5);
            }
        }
        let block = block(&[
            &[0x83, 0xc0, 1],
            &[0xf2, 0x0f, 0x2d, 0xc1],
            &[0x83, 0xf8, 2],
        ]);
        assert_eq!(flags_needed_after(&block)[0], 0x8d5);
    }

    #[test]
    fn executable_memory_store_may_exit_with_its_new_flags() {
        let block = block(&[&[0x83, 0x03, 1], &[0x83, 0xf8, 2]]);
        assert_eq!(flags_needed_after(&block), vec![0x8d5, 0x8d5]);
    }

    #[test]
    fn double_right_shift_preserves_flags_at_zero_and_has_rmw_memory_effects() {
        let register = block(&[&[0x83, 0xc0, 1], &[0x0f, 0xad, 0xd3], &[0x90]]);
        assert_eq!(flags_needed_after(&register)[0], 0x8d5);
        assert_eq!(written_gprs(&register.instructions[1]), 1 << 3);
        let memory = block(&[
            &[0x83, 0xc0, 1],
            &[0x0f, 0xac, 0x13, 0],
            &[0x83, 0xf8, 2],
            &[0x8b, 0x43, 4],
        ]);
        assert_eq!(flags_needed_after(&memory)[0], 0x8d5);
        assert_eq!(written_gprs(&memory.instructions[1]), 0);
        let region = Region {
            entry: memory.start,
            blocks: [(memory.start, memory)].into(),
            limit_reached: false,
        };
        let facts = affine_ranges(&region);
        assert_eq!(facts.len(), 1);
        assert_eq!(facts[0].permissions, 3);
    }

    #[test]
    fn affine_candidates_have_explicit_permissions_and_bounded_spatial_reuse() {
        let first = block(&[&[0x8b, 0x03], &[0x89, 0x4b, 4], &[0x64, 0x8b, 0x43, 8]]);
        let region = Region {
            entry: first.start,
            blocks: [(first.start, first)].into(),
            limit_reached: false,
        };
        assert_eq!(
            affine_ranges(&region),
            vec![AffineRange {
                base: 3,
                minimum: 0,
                maximum: 7,
                permissions: 3,
                references: 2,
                distinct_offsets: 2,
            }]
        );
        let block = block(&[&[0x8b, 0x03], &[0x8b, 0x83, 0, 0x10, 0, 0]]);
        let region = Region {
            entry: block.start,
            blocks: [(block.start, block)].into(),
            limit_reached: false,
        };
        assert!(affine_ranges(&region).is_empty());
    }

    #[test]
    fn affine_invalidation_accounts_for_partial_conditional_and_implicit_writes() {
        for (bytes, mask) in [
            (&[0x66, 0xbb, 0x34, 0x12][..], 1 << 3),
            (&[0x0f, 0x45, 0xda][..], 1 << 3),
            (&[0x50][..], 1 << 4),
            (&[0xc3][..], 1 << 4),
            (&[0xc9][..], (1 << 4) | (1 << 5)),
            (&[0xf7, 0xf1][..], 1 | (1 << 2)),
            (&[0xf6, 0xf1][..], 1),
            (&[0xf6, 0xec][..], 1),
            (&[0x66, 0xf7, 0xe9][..], 1 | (1 << 2)),
            (&[0xf7, 0xe9][..], 1 | (1 << 2)),
            (&[0xf3, 0xa4][..], (1 << 1) | (1 << 6) | (1 << 7)),
            (&[0xab][..], 1 << 7),
        ] {
            assert_eq!(written_gprs(&decode_one(0x1000, bytes)), mask);
        }
        let before_fault = block(&[&[0x83, 0xc0, 1], &[0xf7, 0xf1], &[0x83, 0xe8, 2]]);
        assert_eq!(flags_needed_after(&before_fault)[0], 0x8d5);
        let before_imul_memory_fault = block(&[&[0x83, 0xc0, 1], &[0xf7, 0x2a], &[0x83, 0xe8, 2]]);
        assert_eq!(flags_needed_after(&before_imul_memory_fault)[0], 0x8d5);
        let before_rep = block(&[&[0x83, 0xc0, 1], &[0xf3, 0xa4], &[0x83, 0xe8, 2]]);
        assert_eq!(flags_needed_after(&before_rep)[0], 0x8d5);
    }
}

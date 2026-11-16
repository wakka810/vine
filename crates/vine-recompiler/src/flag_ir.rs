//! Bounded scalar flag recipes for ordinary instruction lowering.
//!
//! Only values already produced by an instruction may be used at recovery.
//! Memory inputs are saved, never reloaded. Unchanged architectural registers
//! and constants need no saved local; a later overlapping GPR write is a use
//! boundary. Recipes do not cross a block entry or require a runtime opcode.
use crate::ir::{Block, Instruction, Opcode, Operand, Region, Register, Width};
use std::collections::BTreeMap;

const CF: u32 = 1;
const PF: u32 = 4;
const AF: u32 = 16;
const ZF: u32 = 64;
const SF: u32 = 128;
const OF: u32 = 2048;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Kind {
    Add,
    Adc,
    Sub,
    Sbb,
    Logic,
}

impl Kind {
    pub fn of(opcode: Opcode) -> Option<Self> {
        Some(match opcode {
            Opcode::Add | Opcode::Inc => Self::Add,
            Opcode::Adc => Self::Adc,
            Opcode::Sub | Opcode::Cmp | Opcode::Dec | Opcode::Neg => Self::Sub,
            Opcode::Sbb => Self::Sbb,
            Opcode::And | Opcode::Or | Opcode::Xor | Opcode::Test => Self::Logic,
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Input {
    Constant(u32),
    Register {
        index: u8,
        bits: u8,
        high8: bool,
    },
    /// Left, right, truncated result, or carry-in (indices 0..4).
    Saved(u8),
}

impl Input {
    fn register(register: Register) -> Self {
        Self::Register {
            index: register.index,
            bits: register.width.bits() as u8,
            high8: register.high8,
        }
    }

    fn operand(operand: Operand, written: u8, saved: u8, width: Width) -> Self {
        match operand {
            Operand::Reg(register) if written & (1 << register.index) == 0 => {
                Self::register(register)
            }
            Operand::Imm(value) => Self::Constant(value & (u32::MAX >> (32 - width.bits()))),
            _ => Self::Saved(saved),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Recipe {
    pub kind: Kind,
    pub bits: u8,
    pub mask: u32,
    pub inputs: [Input; 4],
}

impl Recipe {
    fn new(instruction: &Instruction, mask: u32) -> Option<Self> {
        let kind = Kind::of(instruction.opcode)?;
        let written = crate::optimize::written_gprs(instruction);
        let source = |operand, saved| Input::operand(operand, written, saved, instruction.width);
        let (left, right) = match instruction.opcode {
            Opcode::Inc | Opcode::Dec => (source(instruction.operands[0], 0), Input::Constant(1)),
            Opcode::Neg => (Input::Constant(0), source(instruction.operands[0], 1)),
            _ => (
                source(instruction.operands[0], 0),
                source(instruction.operands[1], 1),
            ),
        };
        let result = match instruction.operands[0] {
            Operand::Reg(register) if !matches!(instruction.opcode, Opcode::Cmp | Opcode::Test) => {
                Input::register(register)
            }
            _ => Input::Saved(2),
        };
        Some(Self {
            kind,
            bits: instruction.width.bits() as u8,
            mask,
            inputs: [left, right, result, Input::Saved(3)],
        })
    }

    pub fn input_mask(self) -> u8 {
        let mut inputs = if self.mask & (PF | ZF | SF) != 0 {
            4
        } else {
            0
        };
        if self.kind == Kind::Logic {
            return inputs;
        }
        if self.mask & (AF | OF) != 0 {
            inputs |= 7;
        }
        if self.mask & CF != 0 {
            inputs |= match self.kind {
                Kind::Add => 5,
                Kind::Sub => 3,
                Kind::Adc | Kind::Sbb => 11,
                Kind::Logic => unreachable!(),
            };
        }
        inputs
    }

    pub fn saved_inputs(self) -> u8 {
        self.inputs
            .iter()
            .enumerate()
            .fold(0, |mask, (index, input)| {
                mask | match input {
                    Input::Saved(slot) if self.input_mask() & (1 << index) != 0 => 1 << slot,
                    _ => 0,
                }
            })
    }

    pub fn register_inputs(self) -> u8 {
        self.inputs
            .iter()
            .enumerate()
            .fold(0, |mask, (index, input)| {
                mask | match input {
                    Input::Register {
                        index: register, ..
                    } if self.input_mask() & (1 << index) != 0 => 1 << register,
                    _ => 0,
                }
            })
    }
}

fn block_recipes(block: &Block) -> impl Iterator<Item = (u32, Recipe)> + '_ {
    let exact = crate::optimize::flags_needed_after(block);
    let mut hot = vec![0; block.instructions.len()];
    let mut live = u32::MAX;
    for (index, instruction) in block.instructions.iter().enumerate().rev() {
        hot[index] = live & instruction.flags_written;
        live = (live & !crate::optimize::unconditional_flag_writes(instruction))
            | instruction.flags_read;
        if Kind::of(instruction.opcode).is_none()
            && (instruction.flags_written | instruction.flags_undefined) != 0
        {
            live = u32::MAX;
        }
    }
    block
        .instructions
        .iter()
        .enumerate()
        .filter_map(move |(index, instruction)| {
            let mask = exact[index] & !hot[index];
            if mask == 0 {
                return None;
            }
            let mut recipe = Recipe::new(instruction, mask)?;
            // A single producer bank is enough. Keep only bits overwritten before
            // another producer or a GPR conflict would force normal materialization.
            for next in &block.instructions[index + 1..] {
                if Kind::of(next.opcode).is_some() {
                    recipe.mask &= next.flags_written;
                    return (recipe.mask != 0).then_some((instruction.eip, recipe));
                }
                if (next.flags_written | next.flags_undefined) != 0
                    || crate::optimize::written_gprs(next) & recipe.register_inputs() != 0
                {
                    return None;
                }
            }
            None
        })
}

pub(crate) fn discover(region: &Region) -> BTreeMap<u32, Recipe> {
    region.blocks.values().flat_map(block_recipes).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn recipes(bytes: &[&[u8]]) -> Vec<Recipe> {
        let mut pc = 0x1000;
        let block = Block {
            start: pc,
            instructions: bytes
                .iter()
                .map(|bytes| {
                    let instruction = crate::decode::decode_one(pc, bytes);
                    pc = instruction.next_eip();
                    instruction
                })
                .collect(),
            successors: vec![],
        };
        block_recipes(&block).map(|(_, recipe)| recipe).collect()
    }

    #[test]
    fn fault_only_values_survive_without_reloading_memory_or_unchanged_gprs() {
        let add = recipes(&[&[0x83, 0xc0, 1], &[0x8b, 0x16], &[0x31, 0xc9]]);
        assert_eq!(add.len(), 1);
        assert_eq!(add[0].mask, CF | PF | ZF | SF | OF);
        assert_eq!(add[0].saved_inputs(), 1); // pre-add EAX only
        assert_eq!(add[0].register_inputs(), 1); // completed EAX is still live
        assert_eq!(add[0].inputs[1], Input::Constant(1));
        let compare = recipes(&[&[0x39, 0xd8], &[0x8b, 0x0e], &[0x39, 0xfa]]);
        assert_eq!(compare[0].saved_inputs(), 4); // only the comparison result
        assert_eq!(compare[0].register_inputs(), 1 | 8);
        let memory = recipes(&[&[0x83, 0x3e, 1], &[0x8b, 0x16], &[0x83, 0xc1, 1]]);
        assert_eq!(memory[0].saved_inputs(), 1 | 4);
        assert_eq!(memory[0].register_inputs(), 0); // an address is not a loaded value
    }

    #[test]
    fn clobbers_dynamic_flags_and_partial_writes_bound_the_recipe() {
        assert!(recipes(&[&[0x83, 0xc0, 1], &[0x8b, 0x06], &[0x83, 0xc1, 1]]).is_empty());
        assert!(
            recipes(&[
                &[0x83, 0xc0, 1],
                &[0x8b, 0x16],
                &[0xd3, 0xe9],
                &[0x39, 0xfa]
            ])
            .is_empty()
        );
        let carry = recipes(&[&[0x83, 0xc0, 1], &[0x8b, 0x16], &[0x83, 0xd1, 1]]);
        assert_eq!(carry[0].mask, PF | AF | ZF | SF | OF); // ADC reads old CF normally
        let partial = recipes(&[&[0x83, 0xc0, 1], &[0x8b, 0x16], &[0x41], &[0x83, 0xfa, 1]]);
        assert_eq!(partial[0].mask & CF, 0); // the first producer bank replacement is INC
        let neg = recipes(&[&[0xf7, 0xd8], &[0x8b, 0x16], &[0x83, 0xc1, 1]]);
        assert_eq!(neg[0].inputs[0], Input::Constant(0));
        assert_eq!(neg[0].saved_inputs(), 2); // normalized right operand is old EAX
    }
}

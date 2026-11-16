//! Lower pure flag recipes without re-executing an architectural effect.
use super::*;
use crate::flag_ir::{Input, Kind, Recipe};

#[derive(Clone, Copy)]
pub(super) enum Inputs {
    Scratch,
    Recipe([Input; 4]),
}

impl Emitter {
    pub(super) fn flag_input(&mut self, inputs: Inputs, index: usize) {
        let Inputs::Recipe(inputs) = inputs else {
            self.get([A, B, R, CARRY][index]);
            return;
        };
        match inputs[index] {
            Input::Constant(value) => self.num(value),
            Input::Saved(slot) => {
                let local = self.flag_saved[usize::from(slot)];
                assert_ne!(local, u32::MAX, "every demanded input has storage");
                self.get(local);
            }
            Input::Register { index, bits, high8 } => {
                self.get(3 + u32::from(index));
                if high8 {
                    self.num(8);
                    self.op(0x76);
                }
                if bits < 32 {
                    self.num((1u32 << bits) - 1);
                    self.op(0x71);
                }
            }
        }
    }

    pub(super) fn emit_flag_recipe(&mut self, recipe: Recipe) {
        let width = match recipe.bits {
            8 => Width::W8,
            16 => Width::W16,
            32 => Width::W32,
            _ => unreachable!("scalar flag width"),
        };
        let inputs = Inputs::Recipe(recipe.inputs);
        if recipe.kind == Kind::Logic {
            self.get(FLAGS);
            self.num(!recipe.mask);
            self.op(0x71);
            self.set(FLAGS);
            self.common_flags_using(width, recipe.mask, inputs);
        } else {
            let opcode = match recipe.kind {
                Kind::Add => Opcode::Add,
                Kind::Adc => Opcode::Adc,
                Kind::Sub => Opcode::Sub,
                Kind::Sbb => Opcode::Sbb,
                Kind::Logic => unreachable!(),
            };
            self.arithmetic_flags_using(opcode, width, recipe.mask, inputs);
        }
    }

    fn require_flags(&mut self, mask: u32) {
        let Some(mut recipe) = self.retirement.and_then(|position| position.flags) else {
            return;
        };
        let required = recipe.mask & mask;
        if required == 0 {
            return;
        }
        self.emit_flag_recipe(Recipe {
            mask: required,
            ..recipe
        });
        recipe.mask &= !required;
        self.retirement.as_mut().expect("prefix flags").flags =
            (recipe.mask != 0).then_some(recipe);
    }

    pub(super) fn prepare_lazy_flags(&mut self, instruction: &Instruction) {
        self.require_flags(instruction.flags_read);
        let Some(recipe) = self.retirement.and_then(|position| position.flags) else {
            return;
        };
        if Kind::of(instruction.opcode).is_none()
            && ((instruction.flags_written | instruction.flags_undefined) != 0
                || crate::optimize::written_gprs(instruction) & recipe.register_inputs() != 0)
        {
            self.require_flags(u32::MAX);
        }
    }

    pub(super) fn ordinary_flags(&mut self, instruction: &Instruction, mask: u32) {
        // Operands, address/protection and dirty capacity have succeeded; the
        // destination is still unchanged. Preserve only old bits not replaced
        // by this operation before reusing its bounded saved-input bank.
        self.require_flags(!instruction.flags_written);
        if let Some(position) = &mut self.retirement {
            position.flags = None;
        }
        let deferred = self
            .flag_plan
            .get(&instruction.eip)
            .copied()
            .and_then(|mut recipe| {
                recipe.mask &= mask;
                (recipe.mask != 0).then_some(recipe)
            });
        let immediate = mask & !deferred.map_or(0, |recipe| recipe.mask);
        if immediate != 0 || deferred.is_none() {
            if matches!(
                instruction.opcode,
                Opcode::And | Opcode::Or | Opcode::Xor | Opcode::Test
            ) {
                self.get(FLAGS);
                self.num(!immediate);
                self.op(0x71);
                self.set(FLAGS);
                self.common_flags(instruction.width, immediate);
            } else {
                self.arithmetic_flags(instruction.opcode, instruction.width, immediate);
            }
        }
        if let Some(recipe) = deferred {
            for (index, source) in [A, B, R, CARRY].into_iter().enumerate() {
                if recipe.saved_inputs() & (1 << index) != 0 {
                    self.get(source);
                    self.set(self.flag_saved[index]);
                }
            }
            self.retirement
                .as_mut()
                .expect("ordinary prefix owns recipes")
                .flags = Some(recipe);
        }
    }
}

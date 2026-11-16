use super::*;
use ir::{Condition, Instruction, Opcode, Operand, Register, Segment};

struct Builder {
    graph: Graph,
    home: BlockId,
    previous: Option<EffectId>,
}
impl Builder {
    fn value(&mut self, op: ValueOp) -> ValueId {
        let home = if matches!(op, ValueOp::Constant(_)) {
            None
        } else {
            Some(self.home)
        };
        let id = ValueId(self.graph.values.len() as u32);
        self.graph.values.push(Value { home, op });
        id
    }
    fn constant(&mut self, value: u32) -> ValueId {
        self.value(ValueOp::Constant(value))
    }
    fn binary(&mut self, op: BinaryOp, left: ValueId, right: ValueId) -> ValueId {
        self.value(ValueOp::Binary { op, left, right })
    }
    fn immediate(&mut self, op: BinaryOp, left: ValueId, right: u32) -> ValueId {
        let right = self.constant(right);
        self.binary(op, left, right)
    }
    fn select(&mut self, condition: ValueId, yes: ValueId, no: ValueId) -> ValueId {
        self.value(ValueOp::Select { condition, yes, no })
    }
    fn truncate(&mut self, value: ValueId, width: ScalarWidth) -> ValueId {
        if width == ScalarWidth::W32 {
            value
        } else {
            self.immediate(BinaryOp::And, value, width.mask())
        }
    }
    fn sign_extend(&mut self, value: ValueId, width: ScalarWidth) -> ValueId {
        if width == ScalarWidth::W32 {
            return value;
        }
        let left = self.immediate(BinaryOp::Shl, value, 32 - width.bits());
        self.immediate(BinaryOp::ShrS, left, 32 - width.bits())
    }
    fn flag(&mut self, bit: u8, op: FlagOp) -> FlagRef {
        let id = FlagRef(self.graph.flags.len() as u32);
        self.graph.flags.push(Flag {
            home: self.home,
            bit,
            op,
        });
        id
    }
    fn bit(&mut self, state: &CpuValues, bit: usize) -> ValueId {
        self.value(ValueOp::EvaluateFlag(state.flags.arithmetic[bit]))
    }
    fn snapshot(&mut self, cpu: &CpuValues, eip: ValueId, completed_prefix: u32) -> SnapshotId {
        let id = SnapshotId(self.graph.snapshots.len() as u32);
        self.graph.snapshots.push(ExitState {
            block: self.home,
            cpu: cpu.clone(),
            eip,
            completed_prefix,
        });
        id
    }
    fn effect(
        &mut self,
        before: SnapshotId,
        kind: EffectKind,
        guard: Option<ValueId>,
        effects: &mut Vec<EffectId>,
    ) -> EffectId {
        let id = EffectId(self.graph.effects.len() as u32);
        self.graph.effects.push(Effect {
            block: self.home,
            previous: self.previous,
            guard,
            before,
            after: before,
            kind,
        });
        self.previous = Some(id);
        effects.push(id);
        id
    }
    fn output(&mut self, effect: EffectId, output: EffectOutput) -> ValueId {
        self.value(ValueOp::EffectResult { effect, output })
    }
    fn address(&mut self, memory: ir::MemoryOperand, cpu: &CpuValues) -> Result<Address, Error> {
        let mut offset = self.constant(memory.displacement);
        if let Some(index) = memory.base {
            offset = self.binary(BinaryOp::Add, offset, cpu.gpr[index as usize]);
        }
        if let Some(index) = memory.index {
            let scaled = self.immediate(
                BinaryOp::Mul,
                cpu.gpr[index as usize],
                u32::from(memory.scale),
            );
            offset = self.binary(BinaryOp::Add, offset, scaled);
        }
        Ok(Address {
            offset,
            segment: memory.segment,
            width: ScalarWidth::from_arch(memory.width)?,
        })
    }
    fn load(
        &mut self,
        address: Address,
        access: Access,
        before: SnapshotId,
        effects: &mut Vec<EffectId>,
    ) -> (ValueId, EffectId) {
        let id = self.effect(before, EffectKind::Load { address, access }, None, effects);
        (self.output(id, EffectOutput::Loaded), id)
    }
    fn read(
        &mut self,
        operand: Operand,
        cpu: &CpuValues,
        before: SnapshotId,
        effects: &mut Vec<EffectId>,
        access: Access,
    ) -> Result<(ValueId, Option<(Address, EffectId)>), Error> {
        match operand {
            Operand::Imm(value) => Ok((self.constant(value), None)),
            Operand::Reg(register) => {
                let value = cpu.gpr[register.index as usize];
                let value = if register.high8 {
                    self.immediate(BinaryOp::ShrU, value, 8)
                } else {
                    value
                };
                Ok((
                    self.truncate(value, ScalarWidth::from_arch(register.width)?),
                    None,
                ))
            }
            Operand::Mem(memory) => {
                let address = self.address(memory, cpu)?;
                let (value, effect) = self.load(address, access, before, effects);
                Ok((value, Some((address, effect))))
            }
            _ => Err(Error("vector operand reached scalar value lift".into())),
        }
    }
    fn assign_register(
        &mut self,
        register: Register,
        value: ValueId,
        cpu: &mut CpuValues,
    ) -> Result<(), Error> {
        let width = ScalarWidth::from_arch(register.width)?;
        let value = self.truncate(value, width);
        cpu.gpr[register.index as usize] = if width == ScalarWidth::W32 {
            value
        } else {
            let shift = if register.high8 { 8 } else { 0 };
            let retained = self.immediate(
                BinaryOp::And,
                cpu.gpr[register.index as usize],
                !(width.mask() << shift),
            );
            let inserted = if shift != 0 {
                self.immediate(BinaryOp::Shl, value, shift)
            } else {
                value
            };
            self.binary(BinaryOp::Or, retained, inserted)
        };
        Ok(())
    }
    // Keep the optional RMW proof and optional store guard distinct: a zero
    // count suppresses a store, never the earlier permission check.
    #[allow(clippy::too_many_arguments)]
    fn assign(
        &mut self,
        operand: Operand,
        value: ValueId,
        checked: Option<(Address, EffectId)>,
        guard: Option<ValueId>,
        cpu: &mut CpuValues,
        before: SnapshotId,
        effects: &mut Vec<EffectId>,
    ) -> Result<(), Error> {
        match operand {
            Operand::Reg(register) => self.assign_register(register, value, cpu),
            Operand::Mem(memory) => {
                let (address, checked_by) = if let Some((address, id)) = checked {
                    (address, Some(id))
                } else {
                    (self.address(memory, cpu)?, None)
                };
                self.effect(
                    before,
                    EffectKind::Store {
                        address,
                        value,
                        checked_by,
                    },
                    guard,
                    effects,
                );
                Ok(())
            }
            _ => Err(Error("invalid scalar destination".into())),
        }
    }
    fn define_flags(
        &mut self,
        cpu: &mut CpuValues,
        instruction: &Instruction,
        kind: FlagKind,
        width: ScalarWidth,
        args: [ValueId; 4],
    ) {
        for (bit, mask) in FLAG_MASKS.iter().enumerate() {
            if instruction.flags_written & mask != 0 {
                cpu.flags.arithmetic[bit] =
                    self.flag(bit as u8, FlagOp::Leaf { kind, width, args });
            }
        }
        let retained = self.immediate(
            BinaryOp::And,
            cpu.flags.defined,
            !instruction.flags_undefined,
        );
        cpu.flags.defined = self.immediate(BinaryOp::Or, retained, instruction.flags_written);
    }
    fn condition(&mut self, condition: Condition, cpu: &CpuValues) -> ValueId {
        use Condition::*;
        match condition {
            O => self.bit(cpu, 5),
            No => {
                let value = self.bit(cpu, 5);
                self.immediate(BinaryOp::Eq, value, 0)
            }
            B => self.bit(cpu, 0),
            Ae => {
                let value = self.bit(cpu, 0);
                self.immediate(BinaryOp::Eq, value, 0)
            }
            E => self.bit(cpu, 3),
            Ne => {
                let value = self.bit(cpu, 3);
                self.immediate(BinaryOp::Eq, value, 0)
            }
            S => self.bit(cpu, 4),
            Ns => {
                let value = self.bit(cpu, 4);
                self.immediate(BinaryOp::Eq, value, 0)
            }
            P => self.bit(cpu, 1),
            Np => {
                let value = self.bit(cpu, 1);
                self.immediate(BinaryOp::Eq, value, 0)
            }
            Be | A => {
                let cf = self.bit(cpu, 0);
                let zf = self.bit(cpu, 3);
                let value = self.binary(BinaryOp::Or, cf, zf);
                if condition == A {
                    self.immediate(BinaryOp::Eq, value, 0)
                } else {
                    value
                }
            }
            L | Ge | Le | G => {
                let sf = self.bit(cpu, 4);
                let of = self.bit(cpu, 5);
                let mut value = self.binary(BinaryOp::Xor, sf, of);
                if matches!(condition, Le | G) {
                    let zf = self.bit(cpu, 3);
                    value = self.binary(BinaryOp::Or, value, zf);
                }
                if matches!(condition, Ge | G) {
                    self.immediate(BinaryOp::Eq, value, 0)
                } else {
                    value
                }
            }
        }
    }
    fn target(&self, pc: u32) -> Target {
        self.graph
            .entries
            .get(&pc)
            .copied()
            .map_or(Target::External(pc), Target::Block)
    }
    fn opaque(
        &mut self,
        instruction: &Instruction,
        cpu: &mut CpuValues,
        before: SnapshotId,
        effects: &mut Vec<EffectId>,
    ) {
        let contract = opaque_contract(instruction);
        let id = self.effect(
            before,
            EffectKind::Opaque {
                instruction: instruction.clone(),
                inputs: cpu.clone(),
                contract,
            },
            None,
            effects,
        );
        for index in 0..8 {
            if contract.gpr_written & (1 << index) != 0 {
                cpu.gpr[index] = self.output(id, EffectOutput::Gpr(index as u8));
            }
        }
        if contract.flags_written != 0 {
            let packed = self.output(id, EffectOutput::PackedFlags);
            for (bit, mask) in FLAG_MASKS.iter().enumerate() {
                if contract.flags_written & mask != 0 {
                    let shift = self.immediate(BinaryOp::ShrU, packed, mask.trailing_zeros());
                    let value = self.immediate(BinaryOp::And, shift, 1);
                    let zero = self.constant(0);
                    cpu.flags.arithmetic[bit] = self.flag(
                        bit as u8,
                        FlagOp::Leaf {
                            kind: FlagKind::Materialized,
                            width: ScalarWidth::W32,
                            args: [value, zero, zero, zero],
                        },
                    );
                }
            }
            if contract.flags_written & !ARITHMETIC_MASK != 0 {
                let retained =
                    self.immediate(BinaryOp::And, cpu.flags.other, !contract.flags_written);
                let written = self.immediate(
                    BinaryOp::And,
                    packed,
                    contract.flags_written & !ARITHMETIC_MASK,
                );
                cpu.flags.other = self.binary(BinaryOp::Or, retained, written);
            }
        }
        // REP count zero and partial progress have dynamic validity. The
        // adapter reports its exact outcome, rather than applying may-write.
        if matches!(
            instruction.opcode,
            Opcode::String(_, _) | Opcode::Shld | Opcode::Shrd
        ) {
            cpu.flags.defined = self.output(id, EffectOutput::DefinedFlags);
        } else {
            let retained =
                self.immediate(BinaryOp::And, cpu.flags.defined, !contract.flags_undefined);
            cpu.flags.defined = self.immediate(BinaryOp::Or, retained, contract.flags_written);
        }
    }
}

fn opaque_contract(instruction: &Instruction) -> OpaqueContract {
    // These adapters have a runtime zero-count path that preserves all old
    // flags, not merely a statically known subset. Double shifts also define
    // deterministic raw AF/OF bits outside their dynamic defined mask.
    let dynamic_flags = matches!(
        instruction.opcode,
        Opcode::Shld
            | Opcode::Shrd
            | Opcode::String(ir::StringOpcode::Scas | ir::StringOpcode::Cmps, _)
    );
    let vector_memory = instruction
        .operands
        .iter()
        .any(|operand| matches!(operand, Operand::Mem(_)));
    let banks_read = match instruction.opcode {
        Opcode::Mmx(_) => {
            banks::X87
                | banks::MMX
                | if instruction
                    .operands
                    .iter()
                    .any(|operand| matches!(operand, Operand::Xmm(_)))
                {
                    banks::XMM
                } else {
                    0
                }
        }
        Opcode::Sse(_) => banks::XMM | banks::MXCSR,
        Opcode::X87 { .. } | Opcode::X87StatusAx => banks::X87 | banks::MMX,
        _ => 0,
    };
    OpaqueContract {
        // The initial bridge may use implicit registers. A conservative GPR
        // mask costs moves only; it must not force unrelated flag evaluation.
        gpr_read: u8::MAX,
        gpr_written: crate::optimize::written_gprs(instruction),
        flags_read: if matches!(
            instruction.opcode,
            Opcode::String(ir::StringOpcode::Scas | ir::StringOpcode::Cmps, _)
        ) {
            // SCAS/CMPS record the entire entry EFLAGS word for Intel's fault
            // restart rule; DF-only input is insufficient. MOVS/STOS retain
            // the SSA flags, with exit materialization separate from progress.
            u32::MAX
        } else {
            instruction.flags_read | if dynamic_flags { ARITHMETIC_MASK } else { 0 }
        },
        flags_written: if dynamic_flags {
            ARITHMETIC_MASK
        } else {
            instruction.flags_written
        },
        flags_undefined: instruction.flags_undefined,
        banks_read,
        banks_written: banks_read,
        memory: if matches!(instruction.opcode, Opcode::String(_, _)) {
            MemoryEffects::Dynamic
        } else if vector_memory {
            MemoryEffects::ReadWrite
        } else {
            MemoryEffects::None
        },
        may_exit: true,
    }
}

/// Lift all decoded blocks without changing code discovery or admissible PCs.
pub fn lift(region: &ir::Region) -> Result<Graph, Error> {
    if region.blocks.is_empty() {
        return Err(Error("cannot lift an empty region".into()));
    }
    let entries: BTreeMap<_, _> = region
        .blocks
        .keys()
        .enumerate()
        .map(|(index, &pc)| (pc, BlockId(index as u32)))
        .collect();
    let entry = *entries
        .get(&region.entry)
        .ok_or_else(|| Error("region entry is absent".into()))?;
    let mut builder = Builder {
        graph: Graph {
            entry,
            entries,
            blocks: Vec::new(),
            values: Vec::new(),
            flags: Vec::new(),
            effects: Vec::new(),
            snapshots: Vec::new(),
        },
        home: entry,
        previous: None,
    };
    for (&pc, block) in &region.blocks {
        builder.home = builder.graph.entries[&pc];
        builder.previous = None;
        let gpr = std::array::from_fn(|index| {
            builder.value(ValueOp::Parameter {
                block: builder.home,
                field: InputField::Gpr(index as u8),
            })
        });
        let arithmetic = std::array::from_fn(|index| {
            builder.flag(
                index as u8,
                FlagOp::Parameter {
                    block: builder.home,
                    bit: index as u8,
                },
            )
        });
        let other = builder.value(ValueOp::Parameter {
            block: builder.home,
            field: InputField::OtherFlags,
        });
        let defined = builder.value(ValueOp::Parameter {
            block: builder.home,
            field: InputField::DefinedFlags,
        });
        let mut cpu = CpuValues {
            gpr,
            flags: Flags {
                arithmetic,
                other,
                defined,
            },
        };
        let parameters = cpu.clone();
        let mut steps = Vec::new();
        let mut terminator = None;
        for (ordinal, instruction) in block.instructions.iter().enumerate() {
            if terminator.is_some() {
                return Err(Error("instruction follows a block terminator".into()));
            }
            let eip = builder.constant(instruction.eip);
            let before = builder.snapshot(&cpu, eip, ordinal as u32);
            let mut effects = Vec::new();
            let mut next = builder.constant(instruction.next_eip());
            let mut completes = true;
            let operands = &instruction.operands;
            use Opcode::*;
            match instruction.opcode {
                Nop => {}
                Cld | Std => {
                    cpu.flags.other = builder.immediate(
                        if instruction.opcode == Cld {
                            BinaryOp::And
                        } else {
                            BinaryOp::Or
                        },
                        cpu.flags.other,
                        if instruction.opcode == Cld {
                            !0x400
                        } else {
                            0x400
                        },
                    );
                    cpu.flags.defined = builder.immediate(BinaryOp::Or, cpu.flags.defined, 0x400);
                }
                Mov | Movzx | Movsx | Lea => {
                    let value = if instruction.opcode == Lea {
                        let Operand::Mem(memory) = operands[1] else {
                            return Err(Error("LEA source is not an address".into()));
                        };
                        builder.address(memory, &cpu)?.offset
                    } else {
                        let (value, _) =
                            builder.read(operands[1], &cpu, before, &mut effects, Access::Read)?;
                        if instruction.opcode == Movsx {
                            let width = match operands[1] {
                                Operand::Reg(register) => register.width,
                                Operand::Mem(memory) => memory.width,
                                _ => return Err(Error("invalid MOVSX source".into())),
                            };
                            builder.sign_extend(value, ScalarWidth::from_arch(width)?)
                        } else {
                            value
                        }
                    };
                    builder.assign(
                        operands[0],
                        value,
                        None,
                        None,
                        &mut cpu,
                        before,
                        &mut effects,
                    )?;
                }
                Xchg => {
                    let (left, _) =
                        builder.read(operands[0], &cpu, before, &mut effects, Access::Read)?;
                    let (right, _) =
                        builder.read(operands[1], &cpu, before, &mut effects, Access::Read)?;
                    if !operands
                        .iter()
                        .all(|operand| matches!(operand, Operand::Reg(_)))
                    {
                        return Err(Error(
                            "locked memory XCHG is not a scalar value operation".into(),
                        ));
                    }
                    builder.assign(
                        operands[0],
                        right,
                        None,
                        None,
                        &mut cpu,
                        before,
                        &mut effects,
                    )?;
                    builder.assign(
                        operands[1],
                        left,
                        None,
                        None,
                        &mut cpu,
                        before,
                        &mut effects,
                    )?;
                }
                Add | Adc | Sub | Sbb | Cmp | Test | And | Or | Xor | Inc | Dec | Neg | Not => {
                    let width = ScalarWidth::from_arch(instruction.width)?;
                    let readonly = matches!(instruction.opcode, Cmp | Test);
                    let (mut left, checked) = builder.read(
                        operands[0],
                        &cpu,
                        before,
                        &mut effects,
                        if readonly {
                            Access::Read
                        } else {
                            Access::ReadWrite
                        },
                    )?;
                    let mut right = if matches!(instruction.opcode, Inc | Dec) {
                        builder.constant(1)
                    } else if matches!(instruction.opcode, Neg | Not) {
                        builder.constant(0)
                    } else {
                        builder
                            .read(operands[1], &cpu, before, &mut effects, Access::Read)?
                            .0
                    };
                    right = builder.truncate(right, width);
                    if instruction.opcode == Neg {
                        right = left;
                        left = builder.constant(0);
                    }
                    let zero = builder.constant(0);
                    let carry = if matches!(instruction.opcode, Adc | Sbb) {
                        builder.bit(&cpu, 0)
                    } else {
                        zero
                    };
                    let subtract = matches!(instruction.opcode, Sub | Sbb | Cmp | Dec | Neg);
                    let logical = matches!(instruction.opcode, And | Or | Xor | Test);
                    let binary = match instruction.opcode {
                        And | Test => BinaryOp::And,
                        Or => BinaryOp::Or,
                        Xor | Not => BinaryOp::Xor,
                        _ if subtract => BinaryOp::Sub,
                        _ => BinaryOp::Add,
                    };
                    let result = if instruction.opcode == Not {
                        builder.immediate(BinaryOp::Xor, left, u32::MAX)
                    } else {
                        builder.binary(binary, left, right)
                    };
                    let result = if matches!(instruction.opcode, Adc | Sbb) {
                        builder.binary(
                            if subtract {
                                BinaryOp::Sub
                            } else {
                                BinaryOp::Add
                            },
                            result,
                            carry,
                        )
                    } else {
                        result
                    };
                    let result = builder.truncate(result, width);
                    if instruction.opcode != Not {
                        builder.define_flags(
                            &mut cpu,
                            instruction,
                            if logical {
                                FlagKind::Logic
                            } else if subtract {
                                FlagKind::Sub
                            } else {
                                FlagKind::Add
                            },
                            width,
                            if logical {
                                [result, zero, zero, zero]
                            } else {
                                [left, right, carry, zero]
                            },
                        );
                    }
                    if !readonly {
                        builder.assign(
                            operands[0],
                            result,
                            checked,
                            None,
                            &mut cpu,
                            before,
                            &mut effects,
                        )?;
                    }
                }
                Shl | Shr | Sar => {
                    let width = ScalarWidth::from_arch(instruction.width)?;
                    let (left, checked) =
                        builder.read(operands[0], &cpu, before, &mut effects, Access::ReadWrite)?;
                    let count = builder
                        .read(operands[1], &cpu, before, &mut effects, Access::Read)?
                        .0;
                    let count = builder.immediate(BinaryOp::And, count, 31);
                    let nonzero = builder.immediate(BinaryOp::Ne, count, 0);
                    let signed_left = if instruction.opcode == Sar {
                        builder.sign_extend(left, width)
                    } else {
                        left
                    };
                    let shifted = builder.binary(
                        match instruction.opcode {
                            Shl => BinaryOp::Shl,
                            Shr => BinaryOp::ShrU,
                            _ => BinaryOp::ShrS,
                        },
                        signed_left,
                        count,
                    );
                    let shifted = builder.truncate(shifted, width);
                    let result = builder.select(nonzero, shifted, left);
                    let zero = builder.constant(0);
                    for bit in 0..6 {
                        let new = builder.flag(
                            bit as u8,
                            FlagOp::Leaf {
                                kind: match instruction.opcode {
                                    Shl => FlagKind::Shl,
                                    Shr => FlagKind::Shr,
                                    _ => FlagKind::Sar,
                                },
                                width,
                                args: [left, count, zero, zero],
                            },
                        );
                        cpu.flags.arithmetic[bit] = builder.flag(
                            bit as u8,
                            FlagOp::Select {
                                condition: nonzero,
                                yes: new,
                                no: cpu.flags.arithmetic[bit],
                            },
                        );
                    }
                    let mut defined =
                        builder.immediate(BinaryOp::And, cpu.flags.defined, !ARITHMETIC_MASK);
                    defined = builder.immediate(BinaryOp::Or, defined, 0xc4);
                    let cf = if instruction.opcode == Sar {
                        builder.constant(1)
                    } else {
                        builder.immediate(BinaryOp::LtU, count, width.bits())
                    };
                    defined = builder.binary(BinaryOp::Or, defined, cf);
                    let one = builder.immediate(BinaryOp::Eq, count, 1);
                    let of = builder.immediate(BinaryOp::Shl, one, 11);
                    defined = builder.binary(BinaryOp::Or, defined, of);
                    cpu.flags.defined = builder.select(nonzero, defined, cpu.flags.defined);
                    builder.assign(
                        operands[0],
                        result,
                        checked,
                        Some(nonzero),
                        &mut cpu,
                        before,
                        &mut effects,
                    )?;
                }
                Setcc | Cmovcc => {
                    let condition = instruction
                        .condition
                        .ok_or_else(|| Error("conditional operation has no condition".into()))?;
                    let value = if instruction.opcode == Cmovcc {
                        let source = builder
                            .read(operands[1], &cpu, before, &mut effects, Access::Read)?
                            .0;
                        let old = builder
                            .read(operands[0], &cpu, before, &mut effects, Access::Read)?
                            .0;
                        let condition = builder.condition(condition, &cpu);
                        builder.select(condition, source, old)
                    } else {
                        builder.condition(condition, &cpu)
                    };
                    builder.assign(
                        operands[0],
                        value,
                        None,
                        None,
                        &mut cpu,
                        before,
                        &mut effects,
                    )?;
                }
                Push | Call => {
                    let width = if instruction.opcode == Call {
                        ScalarWidth::W32
                    } else {
                        ScalarWidth::from_arch(instruction.width)?
                    };
                    let value = builder
                        .read(operands[0], &cpu, before, &mut effects, Access::Read)?
                        .0;
                    let new_sp = builder.immediate(BinaryOp::Sub, cpu.gpr[4], width.bytes());
                    let stored = if instruction.opcode == Call {
                        next
                    } else {
                        value
                    };
                    builder.effect(
                        before,
                        EffectKind::Store {
                            address: Address {
                                offset: new_sp,
                                segment: Segment::Ss,
                                width,
                            },
                            value: stored,
                            checked_by: None,
                        },
                        None,
                        &mut effects,
                    );
                    cpu.gpr[4] = new_sp;
                    if instruction.opcode == Call {
                        next = value;
                    }
                }
                Pop | Ret | Leave => {
                    let width = if instruction.opcode == Ret {
                        ScalarWidth::W32
                    } else {
                        ScalarWidth::from_arch(instruction.width)?
                    };
                    let sp = if instruction.opcode == Leave {
                        cpu.gpr[5]
                    } else {
                        cpu.gpr[4]
                    };
                    let (value, _) = builder.load(
                        Address {
                            offset: sp,
                            segment: Segment::Ss,
                            width,
                        },
                        Access::Read,
                        before,
                        &mut effects,
                    );
                    let adjustment = if instruction.opcode == Ret {
                        match operands.first() {
                            Some(Operand::Imm(value)) => *value,
                            _ => 0,
                        }
                    } else {
                        0
                    };
                    cpu.gpr[4] = builder.immediate(
                        BinaryOp::Add,
                        sp,
                        width.bytes().wrapping_add(adjustment),
                    );
                    if instruction.opcode == Ret {
                        next = value;
                    } else if instruction.opcode == Leave {
                        builder.assign_register(
                            Register {
                                index: 5,
                                width: instruction.width,
                                high8: false,
                            },
                            value,
                            &mut cpu,
                        )?;
                    } else {
                        builder.assign(
                            operands[0],
                            value,
                            None,
                            None,
                            &mut cpu,
                            before,
                            &mut effects,
                        )?;
                    }
                }
                Jmp => {
                    next = builder
                        .read(operands[0], &cpu, before, &mut effects, Access::Read)?
                        .0;
                }
                Loop => {
                    cpu.gpr[1] = builder.immediate(BinaryOp::Sub, cpu.gpr[1], 1);
                }
                Jcc | Jecxz => {}
                Ud2 | Rdtsc | DiagnosticStop | Unsupported => {
                    builder.opaque(instruction, &mut cpu, before, &mut effects);
                    next = eip;
                    completes = false;
                }
                _ => builder.opaque(instruction, &mut cpu, before, &mut effects),
            }
            let condition = match instruction.opcode {
                Jcc => Some(
                    builder.condition(
                        instruction
                            .condition
                            .ok_or_else(|| Error("Jcc lacks condition".into()))?,
                        &cpu,
                    ),
                ),
                Loop | Jecxz => {
                    let comparison = if instruction.opcode == Jecxz {
                        BinaryOp::Eq
                    } else {
                        BinaryOp::Ne
                    };
                    let count = builder.immediate(comparison, cpu.gpr[1], 0);
                    Some(if let Some(condition) = instruction.condition {
                        let flags = builder.condition(condition, &cpu);
                        builder.binary(BinaryOp::And, count, flags)
                    } else {
                        count
                    })
                }
                _ => None,
            };
            if let Some(condition) = condition {
                let Operand::Imm(target) = operands[0] else {
                    return Err(Error("conditional branch target is not direct".into()));
                };
                let target = builder.constant(target);
                next = builder.select(condition, target, next);
            }
            let after = builder.snapshot(&cpu, next, ordinal as u32 + u32::from(completes));
            for &effect in &effects {
                builder.graph.effects[effect.index()].after = after;
            }
            steps.push(Step {
                source: instruction.clone(),
                before,
                after,
                effects,
            });
            terminator = match instruction.opcode {
                Jcc | Loop | Jecxz => {
                    let Operand::Imm(target) = operands[0] else {
                        unreachable!()
                    };
                    Some(Terminator::Conditional {
                        condition: condition.expect("branch condition"),
                        taken: builder.target(target),
                        fallthrough: builder.target(instruction.next_eip()),
                        state: after,
                    })
                }
                Jmp | Call => Some(if let Operand::Imm(target) = operands[0] {
                    Terminator::Direct {
                        target: builder.target(target),
                        state: after,
                    }
                } else {
                    Terminator::Indirect {
                        target: next,
                        state: after,
                    }
                }),
                Ret => Some(Terminator::Indirect {
                    target: next,
                    state: after,
                }),
                Ud2 | Rdtsc | DiagnosticStop | Unsupported => {
                    Some(Terminator::Stop { state: before })
                }
                Opcode::String(_, true) => Some(Terminator::Direct {
                    target: builder.target(instruction.next_eip()),
                    state: after,
                }),
                _ => None,
            };
        }
        let last = steps
            .last()
            .ok_or_else(|| Error("empty decoded block".into()))?;
        let terminator = terminator.unwrap_or(Terminator::Direct {
            target: builder.target(last.source.next_eip()),
            state: last.after,
        });
        builder.graph.blocks.push(Block {
            id: builder.home,
            start: pc,
            parameters,
            steps,
            terminator,
        });
    }
    super::verify(&builder.graph)?;
    Ok(builder.graph)
}

//! Ordered string element execution with architectural REP restart state.
//! No memcpy lowering: overlapping source/destination and element faults must
//! observe the preceding completed stores, including aliases and code writes.
use super::*;

pub(super) const REP_ELEMENT_BUDGET: u32 = 4096;

impl Emitter {
    pub(super) fn string_instruction(
        &mut self,
        instruction: &Instruction,
        operation: StringOpcode,
        repeat: bool,
    ) {
        if matches!(operation, StringOpcode::Scas | StringOpcode::Cmps) {
            self.comparison_instruction(instruction, operation, repeat);
            return;
        }
        let done = format!("string-done-{}", instruction.eip);
        let body = format!("string-body-{}", instruction.eip);
        let fuel = self.rep_fuel;
        let progress = self.string_scratch;
        if repeat {
            self.open(0x02, done.clone());
            self.get(4); // ECX
            self.op(0x45);
            self.if_();
            self.branch(&done);
            self.end();
            // Fuel is shared by all REP instructions in this invocation. If
            // a prior REP completed at the exact limit, this instruction has
            // not started, so report ordinary budget yield (never partial0).
            self.get(fuel);
            self.op(0x45);
            self.if_();
            self.exit(1);
            self.end();
            self.num(0);
            self.set(progress);
        }
        self.num(instruction.width.bytes());
        self.set(A);
        self.get(FLAGS);
        self.num(0x400);
        self.op(0x71);
        self.if_();
        self.num(0u32.wrapping_sub(instruction.width.bytes()));
        self.set(A);
        self.end();
        if repeat {
            self.open(0x03, body.clone());
        }

        // Bypass affine proofs inside the element loop: its implicit GPR
        // updates happen before the instruction's ordinary invalidation point.
        match instruction.operands[1] {
            Operand::Mem(source) => {
                self.effective(source, true);
                self.set(ADDR);
                self.read_memory(source.width, source.segment == Segment::Ss);
            }
            source => self.operand(source),
        }
        self.set(R);
        let Operand::Mem(destination) = instruction.operands[0] else {
            unreachable!()
        };
        self.effective(destination, true);
        self.set(ADDR);
        self.access(destination.width, true, false);
        self.write_memory(destination.width);

        // Only a fully committed element advances indices/count. A failing
        // destination span or journal preflight retains all three registers.
        if operation == StringOpcode::Movs {
            self.get(9);
            self.get(A);
            self.op(0x6a);
            self.set(9);
        }
        self.get(10);
        self.get(A);
        self.op(0x6a);
        self.set(10);
        if repeat {
            self.get(4);
            self.num(1);
            self.op(0x6b);
            self.set(4);
            self.get(fuel);
            self.num(1);
            self.op(0x6b);
            self.set(fuel);
            self.get(progress);
            self.num(1);
            self.op(0x6a);
            self.set(progress);
            self.get(4);
            self.op(0x45);
            self.if_();
            self.branch(&done);
            self.end();
            self.get(SMC);
            self.if_();
            self.get(progress);
            self.set(ERROR);
            self.exit(13);
            self.end();
            self.get(fuel);
            self.op(0x45);
            self.if_();
            self.get(progress);
            self.set(ERROR);
            self.exit(12);
            self.end();
            self.branch(&body);
            self.end();
            self.end();
        }
    }

    fn comparison_instruction(
        &mut self,
        instruction: &Instruction,
        operation: StringOpcode,
        repeat: bool,
    ) {
        let cmps = operation == StringOpcode::Cmps;
        let done = format!("scas-done-{}", instruction.eip);
        let body = format!("scas-body-{}", instruction.eip);
        if repeat {
            self.open(0x02, done.clone());
            self.get(4);
            self.op(0x45);
            self.if_();
            self.branch(&done);
            self.end();
            self.get(self.rep_fuel);
            self.op(0x45);
            self.if_();
            self.exit(1);
            self.end();
            self.op(0x23);
            uleb(&mut self.code, 0);
            self.unit_identity();
            self.num(instruction.eip);
            for local in [FLAGS, DEFINED, if cmps { 9 } else { 3 }, 4, 10] {
                self.get(local);
            }
            self.num(crate::continuation::descriptor(instruction).expect("repeated comparison"));
            self.op(0x10);
            uleb(&mut self.code, self.scas_import);
            self.if_();
            self.exit(3);
            self.end();
            self.num(0);
            self.set(self.string_scratch);
            self.open(0x03, body.clone());
        }
        if cmps {
            let Operand::Mem(source) = instruction.operands[1] else {
                unreachable!()
            };
            // Source overrides apply only to ESI. Neither progress register nor
            // flags change until both complete element reads have succeeded.
            self.effective(source, true);
            self.set(ADDR);
            self.read_memory(source.width, source.segment == Segment::Ss);
            self.set(A);
        }
        // ES is the fixed destination segment, independently of any override.
        // Admission currently accepts flat, full-width ES only. The full span
        // is checked before comparison, flags or either progress register.
        self.get(10);
        self.set(ADDR);
        self.read_memory(instruction.width, false);
        self.set(B);
        if !cmps {
            self.get(3);
            self.truncate(instruction.width);
            self.set(A);
        }
        self.get(A);
        self.get(B);
        self.op(0x6b);
        self.truncate(instruction.width);
        self.set(R);
        self.arithmetic_flags(Opcode::Cmp, instruction.width, ARITH);
        self.get(DEFINED);
        self.num(ARITH);
        self.op(0x72);
        self.set(DEFINED);
        self.get(FLAGS);
        self.num(0x400);
        self.op(0x71);
        self.if_();
        for local in if cmps { &[9, 10][..] } else { &[10][..] } {
            self.get(*local);
            self.num(instruction.width.bytes());
            self.op(0x6b);
            self.set(*local);
        }
        self.op(0x05);
        for local in if cmps { &[9, 10][..] } else { &[10][..] } {
            self.get(*local);
            self.num(instruction.width.bytes());
            self.op(0x6a);
            self.set(*local);
        }
        self.end();
        if repeat {
            for local in [4, self.rep_fuel] {
                self.get(local);
                self.num(1);
                self.op(0x6b);
                self.set(local);
            }
            self.get(self.string_scratch);
            self.num(1);
            self.op(0x6a);
            self.set(self.string_scratch);
            self.get(4);
            self.op(0x45);
            self.if_();
            self.branch(&done);
            self.end();
            // Initial ZF never gates the first element. Only the just-computed
            // comparison can end REPE/REPNE, before considering synthetic fuel.
            self.condition(instruction.condition.expect("comparison repeat condition"));
            self.op(0x45);
            self.if_();
            self.branch(&done);
            self.end();
            self.get(self.rep_fuel);
            self.op(0x45);
            self.if_();
            self.get(self.string_scratch);
            self.set(ERROR);
            self.exit(12);
            self.end();
            self.branch(&body);
            self.end();
            self.end();
            self.op(0x23);
            uleb(&mut self.code, 0);
            self.op(0x10);
            uleb(&mut self.code, self.scas_import + 1);
            self.if_();
            self.exit(3);
            self.end();
        }
    }
}

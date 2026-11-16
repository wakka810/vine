//! Lower a target-set edge without replacing the original guest memory load.
use super::*;

impl Emitter {
    pub(super) fn table_dispatch(&mut self, i: &Instruction) -> bool {
        let Some(plan) = self.dispatches.get(&i.eip).cloned() else {
            return false;
        };
        // Reject malformed IR rather than accidentally changing a register or
        // call transfer into a switch. This operand has already been loaded.
        assert_eq!(i.opcode, Opcode::Jmp);
        assert_eq!(i.operands, [Operand::Mem(plan.operand)]);
        let targets: BTreeMap<u32, usize> = plan
            .targets
            .iter()
            .flatten()
            .copied()
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .enumerate()
            .map(|(n, pc)| (pc, n))
            .collect();
        if targets.is_empty() {
            return false;
        }
        self.open(0x02, "switch-miss".into());
        for n in (0..targets.len()).rev() {
            self.open(0x02, format!("switch-{n}"));
        }
        self.get(3 + u32::from(plan.operand.index.expect("table index")));
        self.op(0x0e);
        uleb(&mut self.code, plan.targets.len() as u32);
        for target in &plan.targets {
            uleb(
                &mut self.code,
                target.map_or(targets.len(), |pc| targets[&pc]) as u32,
            );
        }
        uleb(&mut self.code, targets.len() as u32);
        for &pc in targets.keys() {
            self.end();
            // A range check in guest code may be bypassed at another entry;
            // the table may change via a writable alias without code SMC.
            self.get(PC);
            self.num(pc);
            self.op(0x46);
            self.if_();
            self.jump(pc);
            self.end();
            self.branch("switch-miss");
        }
        self.end();
        self.indirect();
        true
    }
}

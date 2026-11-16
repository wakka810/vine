//! Sink position and flag-validity accounting to observable exits/block edges.
//!
//! Architectural effects execute in order. LEFT stays at the block-entry budget;
//! its completed prefix is subtracted only at exits. Position/validity needs no
//! extra locals; scalar flag recipes save bounded inputs separately. Recovery
//! never replays effects, including helper faults and partial REP completion.
use super::*;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct Position {
    pub pc: u32,
    pub completed: u32,
    pub call_target: bool,
    pub flags: Option<crate::flag_ir::Recipe>,
    defined_keep: u32,
    defined_set: u32,
}

/// Conservative store effects. Implicit stack/string/FP/SIMD stores cannot
/// be inferred from the first explicit operand alone (e.g. MASKMOVDQU).
pub(super) fn may_invalidate_code(instruction: &Instruction) -> bool {
    matches!(
        instruction.opcode,
        Opcode::Push
            | Opcode::Call
            | Opcode::String(_, _)
            | Opcode::Mmx(_)
            | Opcode::Sse(_)
            | Opcode::X87 { .. }
    ) || (!matches!(instruction.opcode, Opcode::Lea | Opcode::Nop)
        && instruction
            .operands
            .iter()
            .any(|operand| matches!(operand, Operand::Mem(_))))
}

impl Emitter {
    /// All checks for the same prefix share its recovery body. ROUTE is dead
    /// on this terminating path, so outlining needs no extra live local and
    /// cannot alter normal dispatch or recompute any architectural effect.
    pub(super) fn outline_retirement_exit(&mut self) {
        let position = self.retirement.expect("completed prefix on cold exit");
        let next = self.retirement_exits.len() as u32;
        let index = *self.retirement_exits.entry(position).or_insert(next);
        self.num(index);
        self.set(ROUTE);
        self.branch("retirement-cold");
    }

    pub(super) fn emit_retirement_exits(&mut self) {
        assert!(self.retirement.is_none());
        let mut exits: Vec<_> = self
            .retirement_exits
            .iter()
            .map(|(&p, &i)| (i, p))
            .collect();
        exits.sort_unstable_by_key(|&(index, _)| index);
        self.open(0x02, "retirement-invalid".into());
        for index in (0..exits.len()).rev() {
            self.open(0x02, format!("retirement-{index}"));
        }
        self.get(ROUTE);
        self.op(0x0e);
        uleb(&mut self.code, exits.len() as u32);
        for index in 0..=exits.len() {
            uleb(&mut self.code, index as u32);
        }
        for (_, position) in exits {
            self.end();
            self.retirement = Some(position);
            self.commit_retirement();
            self.branch("exit");
        }
        self.end();
        self.op(0x00); // A compiler-owned selector cannot name another state.
    }

    pub(super) fn begin_retirement(&mut self, pc: u32) {
        assert!(self.retirement.is_none());
        if self.coalesce_retirement {
            self.retirement = Some(Position {
                pc,
                completed: 0,
                call_target: false,
                flags: None,
                defined_keep: u32::MAX,
                defined_set: 0,
            });
        }
    }

    /// Emit a copy on this exit edge, retaining the context for other paths.
    /// In particular, a fault excludes the current instruction, whereas SMC
    /// after a successful store includes it and reports its next PC.
    pub(super) fn materialize_retirement(&mut self) {
        let Some(position) = self.retirement else {
            return;
        };
        if let Some(recipe) = position.flags {
            self.emit_flag_recipe(recipe);
        }
        self.materialize_defined_flags();
        if position.call_target {
            self.get(B);
        } else {
            self.num(position.pc);
        }
        self.set(PC);
        if position.completed != 0 {
            self.get(LEFT);
            self.num(position.completed);
            self.op(0x6b);
            self.set(LEFT);
        }
    }

    pub(super) fn commit_retirement(&mut self) {
        self.materialize_retirement();
        self.retirement = None;
    }

    pub(super) fn define_flags(&mut self, undefined: u32, written: u32) {
        if let Some(position) = &mut self.retirement {
            // Compose constant bit effects in instruction order. A later
            // definition also makes an earlier clearing of that bit redundant.
            position.defined_keep = (position.defined_keep & !undefined) | written;
            position.defined_set = (position.defined_set & !undefined) | written;
        } else {
            self.get(DEFINED);
            self.num(!undefined);
            self.op(0x71);
            self.num(written);
            self.op(0x72);
            self.set(DEFINED);
        }
    }

    fn materialize_defined_flags(&mut self) {
        let Some(position) = self.retirement else {
            return;
        };
        if position.defined_keep == u32::MAX && position.defined_set == 0 {
            return;
        }
        self.get(DEFINED);
        if position.defined_keep != u32::MAX {
            self.num(position.defined_keep);
            self.op(0x71);
        }
        if position.defined_set != 0 {
            self.num(position.defined_set);
            self.op(0x72);
        }
        self.set(DEFINED);
    }

    /// Runtime shift counts and REP comparisons read/update the live mask.
    /// Commit only that mask before them; PC/count remain a static prefix.
    pub(super) fn commit_defined_flags(&mut self) {
        self.materialize_defined_flags();
        if let Some(position) = &mut self.retirement {
            position.defined_keep = u32::MAX;
            position.defined_set = 0;
        }
    }
}

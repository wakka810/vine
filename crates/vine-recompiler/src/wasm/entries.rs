//! Bounded bucket dispatch with an exact guest-PC guard in every candidate.
use super::*;

impl Emitter {
    pub(super) fn lookup_buckets(&mut self, entries: &[(u32, usize)]) {
        let plan = crate::entry_ir::Plan::new(entries);
        self.open(0x02, "entry-found".into());
        self.open(0x02, "entry-miss".into());
        for id in (0..plan.buckets.len()).rev() {
            self.open(0x02, format!("entry-bucket{id}"));
        }
        self.get(PC);
        self.num(plan.multiplier);
        self.op(0x6c);
        self.num(plan.shift);
        self.op(0x76);
        self.op(0x0e);
        uleb(&mut self.code, plan.slots.len() as u32);
        for slot in plan.slots {
            uleb(&mut self.code, slot.unwrap_or(plan.buckets.len()) as u32);
        }
        uleb(&mut self.code, plan.buckets.len() as u32);
        for bucket in plan.buckets {
            self.end();
            for (pc, route) in bucket {
                self.get(PC);
                self.num(pc);
                self.op(0x46);
                self.if_();
                self.num(route as u32);
                self.set(ROUTE);
                self.branch("entry-found");
                self.end();
            }
            self.branch("entry-miss");
        }
        self.end(); // miss retains the caller's unbound route sentinel
        self.end();
    }
}

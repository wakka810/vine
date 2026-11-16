//! Emit a component DAG with independent same-function loop scopes.
//! Only multi-block cycles need a local selector; a one-block backedge is a
//! direct Wasm loop branch. Arbitrary PC lookup still admits every block once.
use super::*;

impl Emitter {
    pub(super) fn scope_jump(&mut self, pc: u32) -> bool {
        let Some(plan) = self.scope_plan.clone() else {
            return false;
        };
        let Some(target) = plan.targets.get(&pc) else {
            return false;
        };
        let forward = format!("b{}", target.index);
        if self.labels.contains(&forward) {
            self.branch(&forward);
            return true;
        }
        let cycle = format!("cycle{}", target.group);
        let group = format!("group{}", target.group);
        let destination = if self.labels.contains(&cycle) {
            cycle
        } else if self.labels.contains(&group) {
            group
        } else {
            // An unmodelled edge (including unreachable fallthrough code)
            // retains exact-PC lookup, never an invented direct entry.
            self.branch("lookup");
            return true;
        };
        if plan.groups[target.group].blocks.len() > 1 {
            self.num(target.index as u32);
            self.set(ROUTE);
        }
        self.branch(&destination);
        true
    }

    pub(super) fn emit_scopes(&mut self, region: &Region) {
        let plan = self.scope_plan.clone().expect("scope layout");
        self.sample_location(2, 0);
        self.open(0x02, "scope-invalid".into());
        for id in (0..plan.groups.len()).rev() {
            self.open(0x02, format!("group{id}"));
        }
        self.get(ROUTE);
        self.op(0x0e);
        uleb(&mut self.code, plan.targets.len() as u32);
        for (id, group) in plan.groups.iter().enumerate() {
            for _ in &group.blocks {
                uleb(&mut self.code, id as u32);
            }
        }
        uleb(&mut self.code, plan.groups.len() as u32);
        for (id, group) in plan.groups.iter().enumerate() {
            self.end(); // group entry
            if group.cyclic {
                self.open(0x03, format!("cycle{id}"));
            }
            if group.blocks.len() > 1 {
                self.open(0x02, format!("cycle-invalid{id}"));
                for offset in (0..group.blocks.len()).rev() {
                    self.open(0x02, format!("b{}", group.first + offset));
                }
                self.get(ROUTE);
                self.num(group.first as u32);
                self.op(0x6b);
                self.op(0x0e);
                uleb(&mut self.code, group.blocks.len() as u32);
                for offset in 0..=group.blocks.len() {
                    uleb(&mut self.code, offset as u32);
                }
                for pc in &group.blocks {
                    self.end();
                    self.block_body(&region.blocks[pc]);
                }
                self.end(); // impossible selector
                self.op(0x00);
            } else {
                self.block_body(&region.blocks[&group.blocks[0]]);
            }
            if group.cyclic {
                self.end();
            }
        }
        self.end(); // impossible outer selector
        self.op(0x00);
    }
}

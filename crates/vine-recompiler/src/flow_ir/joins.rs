//! A finite flag-recipe analysis before SSA construction. Internal joins get
//! one body, not one body for every incoming operand/recipe combination.
use super::*;
use crate::continuation_ir::{Signature, Templates};

struct Analysis<'a> {
    source: &'a v::Graph,
    templates: &'a mut Templates,
    flags: Vec<Vec<v::FlagRef>>,
    incoming: Vec<Option<[usize; 6]>>,
    pending: BTreeSet<BlockId>,
}

pub(super) fn signatures(
    source: &v::Graph,
    entries: &BTreeSet<BlockId>,
    templates: &mut Templates,
) -> Result<Vec<Option<Signature>>, v::Error> {
    let mut a = Analysis {
        source,
        templates,
        flags: vec![vec![]; source.blocks.len()],
        incoming: vec![None; source.blocks.len()],
        pending: BTreeSet::new(),
    };
    for index in v::dependency_order(source)? {
        if index >= source.values.len() {
            let index = index - source.values.len();
            a.flags[source.flags[index].home.index()].push(v::FlagRef(index as u32));
        }
    }
    // Architectural ports keep independent raw entry adapters. Their outgoing
    // states participate in the same joins as ordinary internal predecessors.
    for &entry in entries {
        a.propagate(entry, [a.templates.identity; 6]);
    }
    while let Some(block) = a.pending.pop_first() {
        a.propagate(block, a.incoming[block.index()].unwrap());
    }
    Ok(a.incoming
        .into_iter()
        .map(|recipes| recipes.map(|r| a.templates.join_signature(r)))
        .collect())
}

impl Analysis<'_> {
    fn merge(&mut self, block: BlockId, next: [usize; 6]) {
        let old = self.incoming[block.index()];
        let joined = old.map_or(next, |old| {
            std::array::from_fn(|bit| {
                if old[bit] == next[bit] {
                    old[bit]
                } else {
                    // Identity means an already evaluated numeric predicate.
                    // It is the top element, never a guessed runtime recipe.
                    self.templates.identity
                }
            })
        });
        if old != Some(joined) {
            self.incoming[block.index()] = Some(joined);
            self.pending.insert(block);
        }
    }
    fn propagate(&mut self, block: BlockId, initial: [usize; 6]) {
        let (targets, state) = match self.source.blocks[block.index()].terminator {
            v::Terminator::Direct { target, state } => (vec![target], state),
            v::Terminator::Conditional {
                taken,
                fallthrough,
                state,
                ..
            } => (vec![taken, fallthrough], state),
            // Indirect edges can only enter the independently seeded raw ports.
            v::Terminator::Indirect { .. } | v::Terminator::Stop { .. } => return,
        };
        let mut recipes = BTreeMap::new();
        for &id in &self.flags[block.index()] {
            let flag = &self.source.flags[id.index()];
            let recipe = match flag.op {
                v::FlagOp::Parameter { bit, .. } => initial[bit as usize],
                v::FlagOp::Alias(other) => recipes[&other],
                v::FlagOp::Leaf { kind, width, .. } => self.templates.leaf(flag.bit, kind, width),
                v::FlagOp::Select { yes, no, .. } => {
                    if recipes[&yes] == recipes[&no] {
                        recipes[&yes]
                    } else {
                        self.templates.identity
                    }
                }
            };
            recipes.insert(id, recipe);
        }
        let outgoing = self.source.snapshots[state.index()]
            .cpu
            .flags
            .arithmetic
            .map(|id| recipes[&id]);
        for target in targets {
            if let v::Target::Block(block) = target {
                self.merge(block, outgoing);
            }
        }
    }
}

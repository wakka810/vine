//! Whole-unit scalar SSA, including the unrestricted external/indirect entry.
//!
//! Root -> ingress -> exposed decoded entries is part of the graph. Known edges
//! bypass ingress. An indirect edge feeds ingress, rather than manufacturing
//! an all-to-all set of edges or assuming a recoverable C calling convention.
//! Ordered effects and precise recovery states remain in `value_ir`.
use crate::predicate_ir as p;
use crate::value_ir::{self as v, BinaryOp, BlockId, EffectId, EffectOutput};
use std::collections::{BTreeMap, BTreeSet};
mod continuations;
mod dominance;
mod joins;
mod recovery;

pub const FIELDS: usize = 16;
pub const ROOT: NodeId = NodeId(0);
pub const INGRESS: NodeId = NodeId(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeId(pub usize);
impl NodeId {
    pub fn block(id: BlockId) -> Self {
        Self(id.index() + 2)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ValueId(pub usize);

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Op {
    Constant(u32),
    Input(u8),
    Phi {
        incoming: Vec<(NodeId, ValueId)>,
    },
    Binary {
        op: BinaryOp,
        left: ValueId,
        right: ValueId,
    },
    Select {
        condition: ValueId,
        yes: ValueId,
        no: ValueId,
    },
    Popcnt(ValueId),
    EffectResult {
        effect: EffectId,
        output: EffectOutput,
    },
    Alias(ValueId),
}
#[derive(Clone, Debug)]
pub struct Value {
    pub home: NodeId,
    pub op: Op,
}
#[derive(Clone, Debug, Default)]
pub struct Stats {
    pub phis_removed: usize,
    pub folded: usize,
    pub shared: usize,
}
#[derive(Clone, Debug)]
pub struct Graph {
    pub values: Vec<Value>,
    /// Only decoded entry ports admit arbitrary architectural state.
    pub raw_nodes: Vec<Option<NodeId>>,
    pub parameters: Vec<Vec<ValueId>>,
    pub predecessors: Vec<Vec<NodeId>>,
    pub successors: Vec<Vec<NodeId>>,
    /// Raw architectural entry nodes are first; each decoded cell then has at
    /// most one internal body with a statically joined flag-recipe signature.
    pub node_blocks: Vec<Option<BlockId>>,
    pub targets: Vec<BTreeMap<BlockId, NodeId>>,
    pub maps: Vec<NodeMap>,
    constant_map: BTreeMap<v::ValueId, ValueId>,
    dom_ranges: Vec<(usize, usize)>,
    pub canonicalized_edges: usize,
    pub expanded_instructions: usize,
    pub template_folded: usize,
    pub template_shared: usize,
    pub stats: Stats,
}

#[derive(Clone, Debug, Default)]
pub struct NodeMap {
    pub values: BTreeMap<v::ValueId, ValueId>,
    pub payloads: BTreeMap<v::FlagRef, ValueId>,
    pub conditions: BTreeMap<v::FlagRef, ValueId>,
}

pub fn operands(op: &Op) -> Vec<ValueId> {
    match op {
        Op::Binary { left, right, .. } => vec![*left, *right],
        Op::Select { condition, yes, no } => vec![*condition, *yes, *no],
        Op::Popcnt(id) | Op::Alias(id) => vec![*id],
        _ => vec![],
    }
}

/// Iterative Kosaraju. Guest-sized graphs must not consume the Rust call stack.
pub(crate) fn components(edges: &[Vec<usize>]) -> Vec<Vec<usize>> {
    let mut seen = vec![false; edges.len()];
    let mut post = Vec::new();
    let mut reverse = vec![Vec::new(); edges.len()];
    for (a, targets) in edges.iter().enumerate() {
        for &b in targets {
            reverse[b].push(a);
        }
        if seen[a] {
            continue;
        }
        let mut stack = vec![(a, false)];
        while let Some((node, leaving)) = stack.pop() {
            if leaving {
                post.push(node);
                continue;
            }
            if std::mem::replace(&mut seen[node], true) {
                continue;
            }
            stack.push((node, true));
            for &next in edges[node].iter().rev() {
                if !seen[next] {
                    stack.push((next, false));
                }
            }
        }
    }
    seen.fill(false);
    let mut result = Vec::new();
    for node in post.into_iter().rev() {
        if seen[node] {
            continue;
        }
        let mut group = Vec::new();
        let mut stack = vec![node];
        while let Some(node) = stack.pop() {
            if std::mem::replace(&mut seen[node], true) {
                continue;
            }
            group.push(node);
            stack.extend(reverse[node].iter().copied());
        }
        group.sort_unstable();
        result.push(group);
    }
    result
}

impl Graph {
    fn add(&mut self, home: NodeId, op: Op) -> ValueId {
        let id = ValueId(self.values.len());
        self.values.push(Value { home, op });
        id
    }
    pub fn resolve(&self, mut id: ValueId) -> ValueId {
        while let Op::Alias(next) = self.values[id.0].op {
            id = next;
        }
        id
    }
    pub fn dominates(&self, a: NodeId, b: NodeId) -> bool {
        let (a0, a1) = self.dom_ranges[a.0];
        let (b0, b1) = self.dom_ranges[b.0];
        a0 <= b0 && b1 <= a1
    }
    pub fn value_at(&self, node: NodeId, id: v::ValueId) -> ValueId {
        self.maps[node.0]
            .values
            .get(&id)
            .copied()
            .or_else(|| self.constant_map.get(&id).copied())
            .expect("value belongs to the executing source block")
    }
    pub fn payload_at(&self, node: NodeId, id: v::FlagRef) -> ValueId {
        self.maps[node.0].payloads[&id]
    }
    pub fn condition_at(&self, node: NodeId, id: v::FlagRef) -> ValueId {
        self.maps[node.0].conditions[&id]
    }
    pub fn build(source: &v::Graph) -> Result<Self, v::Error> {
        Self::build_entries(source, &source.blocks.iter().map(|b| b.id).collect(), false)
    }
    fn edge(
        &mut self,
        from: NodeId,
        to: NodeId,
        values: impl IntoIterator<Item = ValueId>,
    ) -> Result<(), v::Error> {
        let values: Vec<_> = values.into_iter().collect();
        let bad = || v::Error("invalid continuation edge arguments".into());
        if from.0 >= self.successors.len()
            || to.0 >= self.parameters.len()
            || values.len() != self.parameters[to.0].len()
            || values.iter().any(|v| v.0 >= self.values.len())
        {
            return Err(bad());
        }
        let duplicate = self.successors[from.0].contains(&to);
        for (&phi, &value) in self.parameters[to.0].iter().zip(&values) {
            let Some(Value {
                op: Op::Phi { incoming },
                ..
            }) = self.values.get(phi.0)
            else {
                return Err(bad());
            };
            if duplicate
                && incoming.iter().find(|(n, _)| *n == from).map(|(_, v)| *v) != Some(value)
            {
                return Err(bad());
            }
        }
        if duplicate {
            return Ok(());
        }
        self.successors[from.0].push(to);
        self.predecessors[to.0].push(from);
        for (phi, value) in self.parameters[to.0].clone().into_iter().zip(values) {
            if let Op::Phi { incoming } = &mut self.values[phi.0].op {
                incoming.push((from, value));
            }
        }
        Ok(())
    }
    fn simplify_phis(&mut self) -> usize {
        let phis: Vec<_> = self
            .values
            .iter()
            .enumerate()
            .filter_map(|(i, n)| matches!(n.op, Op::Phi { .. }).then_some(ValueId(i)))
            .collect();
        let indices: BTreeMap<_, _> = phis.iter().enumerate().map(|(i, id)| (*id, i)).collect();
        let edges: Vec<_> = phis
            .iter()
            .map(|id| {
                let Op::Phi { incoming } = &self.values[id.0].op else {
                    unreachable!()
                };
                incoming
                    .iter()
                    .filter_map(|(_, v)| indices.get(&self.resolve(*v)).copied())
                    .collect()
            })
            .collect();
        let mut removed = 0;
        for component in components(&edges) {
            let ids: BTreeSet<_> = component.iter().map(|i| phis[*i]).collect();
            let mut outside = BTreeSet::new();
            for id in &ids {
                let Op::Phi { incoming } = &self.values[id.0].op else {
                    unreachable!()
                };
                for &(_, value) in incoming {
                    let value = self.resolve(value);
                    if !ids.contains(&value) {
                        outside.insert(value);
                    }
                }
            }
            if outside.len() == 1 {
                let value = *outside.first().unwrap();
                for id in ids {
                    self.values[id.0].op = Op::Alias(value);
                    removed += 1;
                }
            }
        }
        self.stats.phis_removed += removed;
        removed
    }
    fn number(&self, id: ValueId) -> Option<u32> {
        if let Op::Constant(n) = self.values[id.0].op {
            Some(n)
        } else {
            None
        }
    }
    fn optimize(&mut self) {
        loop {
            let mut changed = self.simplify_phis();
            let mut table: BTreeMap<Op, Vec<ValueId>> = BTreeMap::new();
            for index in 0..self.values.len() {
                let home = self.values[index].home;
                let mut op = self.values[index].op.clone();
                let mut alias = None;
                let mut constant = None;
                match &mut op {
                    Op::Binary { op, left, right } => {
                        *left = self.resolve(*left);
                        *right = self.resolve(*right);
                        use BinaryOp::*;
                        if matches!(op, Add | Mul | And | Or | Xor | Eq | Ne) && right < left {
                            std::mem::swap(left, right);
                        }
                        let a = self.number(*left);
                        let b = self.number(*right);
                        if let (Some(a), Some(b)) = (a, b) {
                            constant = Some(v::fold_binary(*op, a, b));
                        } else {
                            alias = match op {
                                Add | Or | Xor if a == Some(0) => Some(*right),
                                Add | Sub | Or | Xor | Shl | ShrU | ShrS if b == Some(0) => {
                                    Some(*left)
                                }
                                And if a == Some(u32::MAX) => Some(*right),
                                And if b == Some(u32::MAX) => Some(*left),
                                Mul if a == Some(1) => Some(*right),
                                Mul if b == Some(1) => Some(*left),
                                And | Or if left == right => Some(*left),
                                _ => None,
                            };
                            constant = match op {
                                Mul | And if a == Some(0) || b == Some(0) => Some(0),
                                Or if a == Some(u32::MAX) || b == Some(u32::MAX) => Some(u32::MAX),
                                Sub | Xor | Ne | LtU | LtS if left == right => Some(0),
                                Eq | LeU | LeS if left == right => Some(1),
                                _ => constant,
                            };
                        }
                    }
                    Op::Select { condition, yes, no } => {
                        *condition = self.resolve(*condition);
                        *yes = self.resolve(*yes);
                        *no = self.resolve(*no);
                        if yes == no {
                            alias = Some(*yes);
                        } else if let Some(n) = self.number(*condition) {
                            alias = Some(if n == 0 { *no } else { *yes });
                        }
                    }
                    Op::Popcnt(id) => {
                        *id = self.resolve(*id);
                        constant = self.number(*id).map(u32::count_ones);
                    }
                    Op::Phi { incoming } => {
                        for (_, id) in incoming {
                            *id = self.resolve(*id);
                        }
                    }
                    _ => {}
                }
                if let Some(a) = alias {
                    op = Op::Alias(a);
                    self.stats.folded += 1;
                } else if let Some(n) = constant {
                    op = Op::Constant(n);
                    self.stats.folded += 1;
                }
                if matches!(
                    op,
                    Op::Constant(_)
                        | Op::Binary { .. }
                        | Op::Select { .. }
                        | Op::Popcnt(_)
                        | Op::Phi { .. }
                ) {
                    let entries = table.entry(op.clone()).or_default();
                    if let Some(&old) = entries.iter().find(|id| {
                        let old_home = self.values[id.0].home;
                        if matches!(op, Op::Phi { .. }) {
                            old_home == home
                        } else {
                            self.dominates(old_home, home)
                        }
                    }) {
                        op = Op::Alias(old);
                        self.stats.shared += 1;
                    } else {
                        entries.push(ValueId(index));
                    }
                }
                if self.values[index].op != op {
                    changed += 1;
                    self.values[index].op = op;
                }
            }
            if changed == 0 {
                break;
            }
        }
    }
    pub fn verify(&self, source: &v::Graph) -> Result<(), v::Error> {
        let bad = || v::Error("invalid whole-unit flow SSA".into());
        let count = self.node_blocks.len();
        if self.parameters.len() != count
            || self.predecessors.len() != count
            || self.successors.len() != count
            || self.raw_nodes.len() != source.blocks.len()
            || self.maps.len() != count
            || self.targets.len() != count
            || count < 3
            || self.raw_nodes[source.entry.index()].is_none()
            || self.node_blocks[..2] != [None, None]
            || self.parameters[ROOT.0].len() != FIELDS
            || self.parameters[INGRESS.0].len() != FIELDS
            || !self.targets[ROOT.0].is_empty()
            || !self.targets[INGRESS.0].is_empty()
        {
            return Err(bad());
        }
        let mut expected = vec![BTreeSet::new(); count];
        expected[ROOT.0].insert(INGRESS);
        for block in &source.blocks {
            let Some(raw) = self.raw_nodes[block.id.index()] else {
                continue;
            };
            if raw.0 >= count
                || self.node_blocks[raw.0] != Some(block.id)
                || self.parameters[raw.0].len() != FIELDS
            {
                return Err(bad());
            }
            expected[INGRESS.0].insert(raw);
        }
        for (index, source_block) in self.node_blocks.iter().enumerate().skip(2) {
            let from = NodeId(index);
            let Some(block) = source_block.and_then(|id| source.blocks.get(id.index())) else {
                return Err(bad());
            };
            if !(10..=34).contains(&self.parameters[index].len()) {
                return Err(bad());
            }
            let targets = match block.terminator {
                v::Terminator::Direct { target, .. } => vec![target],
                v::Terminator::Conditional {
                    taken, fallthrough, ..
                } => vec![taken, fallthrough],
                v::Terminator::Indirect { .. } => {
                    expected[from.0].insert(INGRESS);
                    vec![]
                }
                v::Terminator::Stop { .. } => vec![],
            };
            let mut keys = BTreeSet::new();
            for target in targets {
                if let v::Target::Block(id) = target {
                    keys.insert(id);
                    let Some(&to) = self.targets[index].get(&id) else {
                        return Err(bad());
                    };
                    if to.0 >= count || self.node_blocks[to.0] != Some(id) {
                        return Err(bad());
                    }
                    expected[from.0].insert(to);
                }
            }
            if self.targets[index].keys().copied().collect::<BTreeSet<_>>() != keys {
                return Err(bad());
            }
        }
        let mut predecessors = vec![BTreeSet::new(); count];
        for (from, targets) in expected.iter().enumerate() {
            let actual: BTreeSet<_> = self.successors[from].iter().copied().collect();
            if actual != *targets || actual.len() != self.successors[from].len() {
                return Err(bad());
            }
            for to in targets {
                predecessors[to.0].insert(NodeId(from));
            }
        }
        for (node, expected) in predecessors.iter().enumerate() {
            if expected.iter().copied().collect::<Vec<_>>() != self.predecessors[node] {
                return Err(bad());
            }
        }
        if self.dom_ranges != dominance::ranges(&self.successors, &self.predecessors)? {
            return Err(bad());
        }
        // Validate every reference and alias chain before resolving any use.
        // A malformed later node must not turn verification into an unbounded
        // walk or an out-of-range index through an earlier phi's input.
        let mut status = vec![0_u8; self.values.len()];
        let mut resolved: Vec<_> = (0..self.values.len()).map(ValueId).collect();
        for (index, node) in self.values.iter().enumerate() {
            if node.home.0 >= count || operands(&node.op).iter().any(|v| v.0 >= self.values.len()) {
                return Err(bad());
            }
            if let Op::Phi { incoming } = &node.op
                && incoming
                    .iter()
                    .any(|(n, v)| n.0 >= count || v.0 >= self.values.len())
            {
                return Err(bad());
            }
            let mut path = Vec::new();
            let mut at = ValueId(index);
            loop {
                if at.0 >= self.values.len() || status[at.0] == 1 {
                    return Err(bad());
                }
                if status[at.0] == 2 {
                    at = resolved[at.0];
                    break;
                }
                status[at.0] = 1;
                path.push(at);
                if let Op::Alias(next) = self.values[at.0].op {
                    at = next;
                } else {
                    break;
                }
            }
            for id in path {
                resolved[id.0] = at;
                status[id.0] = 2;
            }
        }
        for (index, node) in self.values.iter().enumerate() {
            if node.home.0 >= count {
                return Err(bad());
            }
            let mut references = operands(&node.op);
            if let Op::Phi { incoming } = &node.op {
                if node.home == ROOT
                    || incoming.iter().map(|(n, _)| *n).collect::<Vec<_>>()
                        != self.predecessors[node.home.0]
                {
                    return Err(bad());
                }
                references.extend(incoming.iter().map(|(_, v)| *v));
            }
            if references.iter().any(|v| v.0 >= self.values.len()) {
                return Err(bad());
            }
            if let Op::Phi { incoming } = &node.op {
                for &(pred, value) in incoming {
                    if pred.0 >= count
                        || !self.successors[pred.0].contains(&node.home)
                        || !self.dominates(self.values[resolved[value.0].0].home, pred)
                    {
                        return Err(bad());
                    }
                }
            } else {
                for value in references {
                    let value = resolved[value.0];
                    if !self.dominates(self.values[value.0].home, node.home) {
                        return Err(bad());
                    }
                    // Pure definitions form a DAG within a node; only phi edges
                    // can refer to a definition from a later loop iteration.
                    if self.values[value.0].home == node.home
                        && value.0 >= index
                        && !matches!(
                            self.values[value.0].op,
                            Op::Constant(_)
                                | Op::Phi { .. }
                                | Op::Input(_)
                                | Op::EffectResult { .. }
                        )
                    {
                        return Err(bad());
                    }
                }
            }
            match node.op {
                Op::Input(n) if n as usize >= FIELDS || node.home != ROOT => return Err(bad()),
                Op::EffectResult { effect, .. }
                    if source
                        .effects
                        .get(effect.index())
                        .is_none_or(|e| self.node_blocks[node.home.0] != Some(e.block)) =>
                {
                    return Err(bad());
                }
                _ => {}
            }
        }
        for (node, params) in self.parameters.iter().enumerate() {
            if params.iter().any(|id| {
                id.0 >= self.values.len()
                    || !self.dominates(self.values[self.resolve(*id).0].home, NodeId(node))
            }) {
                return Err(bad());
            }
        }
        let constants = source
            .values
            .iter()
            .enumerate()
            .filter(|(_, v)| v.home.is_none());
        let mut constant_count = 0;
        for (index, _) in constants {
            constant_count += 1;
            let Some(&id) = self.constant_map.get(&v::ValueId(index as u32)) else {
                return Err(bad());
            };
            if id.0 >= self.values.len() || self.values[resolved[id.0].0].home != ROOT {
                return Err(bad());
            }
        }
        if constant_count != self.constant_map.len() {
            return Err(bad());
        }
        let mut value_counts = vec![0; source.blocks.len()];
        let mut flag_counts = value_counts.clone();
        for value in &source.values {
            if let Some(home) = value.home {
                value_counts[home.index()] += 1;
            }
        }
        for flag in &source.flags {
            flag_counts[flag.home.index()] += 1;
        }
        for (node, source_block) in self.node_blocks.iter().enumerate().skip(2) {
            let home = NodeId(node);
            let source_block = source_block.unwrap();
            let map = &self.maps[node];
            for (&source_id, &value) in &map.values {
                if source
                    .values
                    .get(source_id.index())
                    .is_none_or(|v| v.home != Some(source_block))
                    || value.0 >= self.values.len()
                    || !self.dominates(self.values[resolved[value.0].0].home, home)
                {
                    return Err(bad());
                }
            }
            for (&source_id, &value) in map.payloads.iter().chain(&map.conditions) {
                if source
                    .flags
                    .get(source_id.index())
                    .is_none_or(|f| f.home != source_block)
                    || value.0 >= self.values.len()
                    || !self.dominates(self.values[resolved[value.0].0].home, home)
                {
                    return Err(bad());
                }
            }
            if map.values.len() != value_counts[source_block.index()]
                || map.payloads.len() != flag_counts[source_block.index()]
                || map.conditions.len() != map.payloads.len()
            {
                return Err(bad());
            }
        }
        recovery::verify(self, source)
    }
}

#[cfg(test)]
mod tests;

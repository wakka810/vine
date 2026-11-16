//! Logical-point liveness for whole-unit SSA. Recovery uses live atoms at the
//! fault site; it does not extend every block's atoms to a global cold table.
use crate::flow_ir::{self as f, Graph, NodeId, Op, ValueId};
use crate::value_ir::{self as v, EffectKind, Terminator};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Default)]
struct Point {
    uses: BTreeSet<ValueId>,
    defs: BTreeSet<ValueId>,
}
pub(super) struct Allocation {
    pub colors: Vec<Option<u32>>,
    pub count: u32,
    pub peak: usize,
    pub coalesced: usize,
    pub phis: Vec<Vec<ValueId>>,
    pub materialized: Vec<bool>,
    pub definitions: Vec<Vec<ValueId>>,
}

/// Values carried between instruction cells are evaluated once at their
/// defining cell. They remain SSA values, not architectural writeback. Keeping
/// these cut points prevents a later use/recovery from expanding the entire
/// history of scalar arithmetic again.
fn materialized_values(g: &Graph) -> Vec<bool> {
    let mut result = vec![false; g.values.len()];
    let mut retain = |id: ValueId| {
        let id = g.resolve(id);
        if g.values[id.0].home.0 >= 2
            && matches!(
                g.values[id.0].op,
                Op::Binary { .. } | Op::Select { .. } | Op::Popcnt(_)
            )
        {
            result[id.0] = true;
        }
    };
    for &id in g.parameters.iter().flatten() {
        retain(id);
    }
    for value in &g.values {
        if let Op::Phi { incoming } = &value.op {
            for &(_, id) in incoming {
                retain(id);
            }
        }
    }
    result
}

struct Uses<'a> {
    graph: &'a Graph,
    materialized: &'a [bool],
    node: NodeId,
    completed: bool,
}
impl Uses<'_> {
    fn atoms(&self, value: ValueId, into: &mut BTreeSet<ValueId>) {
        let g = self.graph;
        let mut seen = BTreeSet::new();
        let mut stack = vec![value];
        while let Some(id) = stack.pop() {
            let id = g.resolve(id);
            if !seen.insert(id) {
                continue;
            }
            if self.materialized[id.0] && (self.completed || g.values[id.0].home != self.node) {
                into.insert(id);
                continue;
            }
            match &g.values[id.0].op {
                Op::Input(_) | Op::Phi { .. } | Op::EffectResult { .. } => {
                    into.insert(id);
                }
                op => stack.extend(f::operands(op)),
            }
        }
    }
    fn value(&self, id: v::ValueId, into: &mut BTreeSet<ValueId>) {
        self.atoms(self.graph.value_at(self.node, id), into);
    }
    fn flags(&self, flags: &v::Flags, mask: u32, into: &mut BTreeSet<ValueId>) {
        if mask & !v::ARITHMETIC_MASK != 0 {
            self.value(flags.other, into);
        }
        for (bit, id) in flags.arithmetic.iter().enumerate() {
            if mask & v::FLAG_MASKS[bit] != 0 {
                self.atoms(self.graph.condition_at(self.node, *id), into);
            }
        }
    }
    fn snapshot(&self, source: &v::Graph, id: v::SnapshotId, into: &mut BTreeSet<ValueId>) {
        let s = &source.snapshots[id.index()];
        for id in s.cpu.gpr {
            self.value(id, into);
        }
        self.flags(&s.cpu.flags, u32::MAX, into);
        self.value(s.cpu.flags.defined, into);
        self.value(s.eip, into);
    }
}

pub(super) fn allocate(g: &Graph, source: &v::Graph, admission: &BTreeSet<NodeId>) -> Allocation {
    let n = g.successors.len();
    let materialized = materialized_values(g);
    let mut definitions = vec![vec![]; n];
    for (id, &stored) in materialized.iter().enumerate() {
        if stored {
            definitions[g.values[id].home.0].push(ValueId(id));
        }
    }
    let complete = Uses {
        graph: g,
        materialized: &materialized,
        node: f::ROOT,
        completed: true,
    };
    let mut points: Vec<Vec<Point>> = (0..n).map(|_| vec![]).collect();
    let mut scalar_points = vec![0; n];
    let mut root = Point::default();
    root.defs.extend(g.parameters[f::ROOT.0].iter().copied());
    points[f::ROOT.0].push(root);
    let mut miss = Point::default();
    for &id in &g.parameters[f::INGRESS.0] {
        complete.atoms(id, &mut miss.uses);
    }
    points[f::INGRESS.0].push(miss);
    let mut outputs = BTreeMap::<(NodeId, v::EffectId), BTreeSet<ValueId>>::new();
    for (i, node) in g.values.iter().enumerate() {
        if let Op::EffectResult { effect, .. } = node.op {
            outputs
                .entry((node.home, effect))
                .or_default()
                .insert(ValueId(i));
        }
    }
    for (node, source_block) in g.node_blocks.iter().enumerate().skip(2) {
        let node = NodeId(node);
        let mut uses = Uses {
            graph: g,
            materialized: &materialized,
            node,
            completed: false,
        };
        let block = &source.blocks[source_block.expect("continuation source block").index()];
        let list = &mut points[node.0];
        if admission.contains(&node) {
            let mut entry = Point::default();
            uses.snapshot(source, block.steps[0].before, &mut entry.uses);
            list.push(entry);
        }
        for step in &block.steps {
            for &id in &step.effects {
                let effect = &source.effects[id.index()];
                let mut before = Point::default();
                uses.snapshot(source, effect.before, &mut before.uses);
                if let Some(guard) = effect.guard {
                    uses.value(guard, &mut before.uses);
                }
                match &effect.kind {
                    EffectKind::Load { address, .. } => {
                        uses.value(address.offset, &mut before.uses)
                    }
                    EffectKind::Store {
                        address, value: v, ..
                    } => {
                        uses.value(address.offset, &mut before.uses);
                        uses.value(*v, &mut before.uses);
                    }
                    EffectKind::Opaque {
                        inputs,
                        contract,
                        instruction,
                    } => {
                        for (i, id) in inputs.gpr.iter().enumerate() {
                            if contract.gpr_read & (1 << i) != 0 {
                                uses.value(*id, &mut before.uses);
                            }
                        }
                        let scan = matches!(
                            instruction.opcode,
                            crate::ir::Opcode::String(
                                crate::ir::StringOpcode::Scas | crate::ir::StringOpcode::Cmps,
                                _
                            )
                        );
                        uses.flags(
                            &inputs.flags,
                            if scan { u32::MAX } else { contract.flags_read },
                            &mut before.uses,
                        );
                        uses.value(inputs.flags.defined, &mut before.uses);
                    }
                }
                list.push(before);
                list.push(Point {
                    defs: outputs.get(&(node, id)).cloned().unwrap_or_default(),
                    ..Point::default()
                });
            }
            if step.effects.iter().any(|id| {
                matches!(
                    source.effects[id.index()].kind,
                    EffectKind::Store { .. } | EffectKind::Opaque { .. }
                )
            }) {
                let mut post = Point::default();
                uses.snapshot(source, step.after, &mut post.uses);
                list.push(post);
            }
        }
        // Reserve the definition point, but do not treat every potential
        // materialization as a root. Only observable uses may make it live.
        scalar_points[node.0] = list.len();
        list.push(Point::default());
        uses.completed = true;
        let mut end = Point::default();
        match block.terminator {
            Terminator::Conditional {
                condition,
                state,
                taken,
                fallthrough,
            } => {
                uses.value(condition, &mut end.uses);
                if matches!(taken, v::Target::External(_))
                    || matches!(fallthrough, v::Target::External(_))
                {
                    uses.snapshot(source, state, &mut end.uses);
                }
            }
            Terminator::Direct {
                target: v::Target::External(_),
                state,
            }
            | Terminator::Stop { state } => uses.snapshot(source, state, &mut end.uses),
            Terminator::Indirect { target, .. } => uses.value(target, &mut end.uses),
            _ => {}
        }
        list.push(end);
    }
    let mut required = BTreeSet::<ValueId>::new();
    for list in &points {
        for p in list {
            required.extend(&p.uses);
        }
    }
    let mut work: Vec<_> = required.iter().copied().collect();
    while let Some(id) = work.pop() {
        let mut input = BTreeSet::new();
        if let Op::Phi { incoming } = &g.values[id.0].op {
            for (_, v) in incoming {
                complete.atoms(*v, &mut input);
            }
        } else if materialized[id.0] {
            Uses {
                graph: g,
                materialized: &materialized,
                node: g.values[id.0].home,
                completed: false,
            }
            .atoms(id, &mut input);
        }
        for v in input {
            if required.insert(v) {
                work.push(v);
            }
        }
    }
    for node in 2..n {
        definitions[node].retain(|id| required.contains(id));
        let point = &mut points[node][scalar_points[node]];
        let inputs = Uses {
            graph: g,
            materialized: &materialized,
            node: NodeId(node),
            completed: false,
        };
        for &id in &definitions[node] {
            inputs.atoms(id, &mut point.uses);
            point.defs.insert(id);
        }
    }
    let mut phis = vec![vec![]; n];
    let mut edge_uses = BTreeMap::<(NodeId, NodeId), BTreeSet<ValueId>>::new();
    for &id in &required {
        let home = g.values[id.0].home;
        if let Op::Phi { incoming } = &g.values[id.0].op {
            phis[home.0].push(id);
            for &(from, v) in incoming {
                complete.atoms(v, edge_uses.entry((from, home)).or_default());
            }
        }
    }
    let mut uses = vec![BTreeSet::new(); n];
    let mut defs = uses.clone();
    for node in 0..n {
        defs[node].extend(&phis[node]);
        for p in &points[node] {
            uses[node].extend(p.uses.difference(&defs[node]).copied());
            defs[node].extend(p.defs.intersection(&required).copied());
        }
    }
    let mut live_in = vec![BTreeSet::new(); n];
    let mut live_out = live_in.clone();
    loop {
        let mut changed = false;
        for node in (0..n).rev() {
            let mut out = BTreeSet::new();
            for &next in &g.successors[node] {
                out.extend(&live_in[next.0]);
                if let Some(edge) = edge_uses.get(&(NodeId(node), next)) {
                    out.extend(edge);
                }
            }
            let mut input = uses[node].clone();
            input.extend(out.difference(&defs[node]).copied());
            changed |= out != live_out[node] || input != live_in[node];
            live_out[node] = out;
            live_in[node] = input;
        }
        if !changed {
            break;
        }
    }
    let mut interference: BTreeMap<_, BTreeSet<_>> =
        required.iter().map(|id| (*id, BTreeSet::new())).collect();
    let mut peak = 0;
    let mut clique = |live: &BTreeSet<ValueId>| {
        peak = peak.max(live.len());
        for &a in live {
            for &b in live {
                if a != b {
                    interference.get_mut(&a).unwrap().insert(b);
                }
            }
        }
    };
    for node in 0..n {
        let mut live = live_out[node].clone();
        clique(&live);
        for point in points[node].iter().rev() {
            let mut at_def = live.clone();
            at_def.extend(point.defs.intersection(&required).copied());
            clique(&at_def);
            for def in &point.defs {
                live.remove(def);
            }
            live.extend(&point.uses);
            clique(&live);
        }
        live.extend(&phis[node]);
        clique(&live);
    }
    // Phi operands are parallel copies. Coalesce only if their complete groups
    // have no interference; a nontrivial loop permutation remains a real copy.
    let mut parent: Vec<_> = (0..g.values.len()).collect();
    fn find(p: &mut [usize], mut id: usize) -> usize {
        let mut root = id;
        while p[root] != root {
            root = p[root];
        }
        // Keep the chosen representative (and deterministic coloring) intact.
        // Later phi merges can form long chains; flatten visited paths so each
        // interference query need not walk every earlier merge again.
        while p[id] != root {
            let next = p[id];
            p[id] = root;
            id = next;
        }
        root
    }
    let mut members: BTreeMap<_, BTreeSet<_>> = required
        .iter()
        .map(|id| (id.0, BTreeSet::from([*id])))
        .collect();
    let mut coalesced = 0;
    for list in &phis {
        for &phi in list {
            let Op::Phi { incoming } = &g.values[phi.0].op else {
                unreachable!()
            };
            for &(_, v) in incoming {
                let v = g.resolve(v);
                if !required.contains(&v) {
                    continue;
                }
                let a = find(&mut parent, phi.0);
                let b = find(&mut parent, v.0);
                if a == b {
                    continue;
                }
                let conflict = members[&a].iter().any(|id| {
                    interference[id]
                        .iter()
                        .any(|other| find(&mut parent, other.0) == b)
                });
                if !conflict {
                    parent[b] = a;
                    let mut moved = members.remove(&b).unwrap();
                    let into = members.get_mut(&a).unwrap();
                    // The representative stays a, but its storage need not.
                    // Move the smaller set so long phi chains do not rebuild
                    // an increasingly large group at every merge.
                    if into.len() < moved.len() {
                        std::mem::swap(into, &mut moved);
                    }
                    into.extend(moved);
                    coalesced += 1;
                }
            }
        }
    }
    let mut group_edges = BTreeMap::<usize, BTreeSet<usize>>::new();
    for &id in &required {
        let a = find(&mut parent, id.0);
        let into = group_edges.entry(a).or_default();
        for other in &interference[&id] {
            let b = find(&mut parent, other.0);
            if a != b {
                into.insert(b);
            }
        }
    }
    let mut order: Vec<_> = group_edges.keys().copied().collect();
    order.sort_by_key(|id| (std::cmp::Reverse(group_edges[id].len()), *id));
    let mut assigned = BTreeMap::new();
    let mut count = 0;
    for id in order {
        let used: BTreeSet<_> = group_edges[&id]
            .iter()
            .filter_map(|id| assigned.get(id).copied())
            .collect();
        let color = (0..).find(|n| !used.contains(n)).unwrap();
        assigned.insert(id, color);
        count = count.max(color + 1);
    }
    let mut colors = vec![None; g.values.len()];
    for &id in &required {
        colors[id.0] = Some(assigned[&find(&mut parent, id.0)]);
    }
    Allocation {
        colors,
        count,
        peak,
        coalesced,
        phis,
        materialized,
        definitions,
    }
}

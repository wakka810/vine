//! SSA atomic storage with logical-point liveness. Cold recipes only read
//! atomic inputs and rematerialize pure DAGs into a separate temporary bank.
//! A branch to a cold recipe skips every intervening normal definition, so it
//! does not extend unrelated SSA values to the physical end of the function.
use super::control;
use crate::rooted_ir::{Graph, Op, ValueId};
use crate::value_ir::{BlockId, EffectKind, SnapshotId, Terminator};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Default)]
struct Point {
    uses: BTreeSet<ValueId>,
    definitions: BTreeSet<ValueId>,
}

pub(super) struct Allocation {
    pub colors: Vec<Option<u32>>,
    pub count: u32,
    pub atomic_values: usize,
    pub coalesced: usize,
    pub peak_live: usize,
    pub phis: BTreeMap<BlockId, Vec<ValueId>>,
    pub required: BTreeSet<ValueId>,
}

pub(super) fn operands(op: &Op) -> Vec<ValueId> {
    match op {
        Op::Binary { left, right, .. } => vec![*left, *right],
        Op::Select { condition, yes, no } => vec![*condition, *yes, *no],
        Op::Flag { args, .. } => args.to_vec(),
        Op::Alias(value) => vec![*value],
        _ => Vec::new(),
    }
}

pub(super) fn atom_dependencies(graph: &Graph, value: ValueId, into: &mut BTreeSet<ValueId>) {
    fn visit(
        graph: &Graph,
        value: ValueId,
        into: &mut BTreeSet<ValueId>,
        seen: &mut BTreeSet<ValueId>,
    ) {
        let value = graph.resolve(value);
        if !seen.insert(value) {
            return;
        }
        match &graph.values[value.index()].op {
            Op::Input(_) | Op::Phi { .. } | Op::EffectResult { .. } => {
                into.insert(value);
            }
            Op::Constant(_) => {}
            Op::Unreachable => panic!("unreachable source reached atomic demand"),
            op => {
                for input in operands(op) {
                    visit(graph, input, into, seen);
                }
            }
        }
    }
    visit(graph, value, into, &mut BTreeSet::new());
}

fn value_use(graph: &Graph, value: crate::value_ir::ValueId, into: &mut BTreeSet<ValueId>) {
    atom_dependencies(graph, graph.value_map[value.index()], into);
}
fn flags_use(
    graph: &Graph,
    flags: &crate::value_ir::Flags,
    mask: u32,
    into: &mut BTreeSet<ValueId>,
) {
    if mask & !crate::value_ir::ARITHMETIC_MASK != 0 {
        value_use(graph, flags.other, into);
    }
    for (bit, flag) in flags.arithmetic.iter().enumerate() {
        if mask & crate::value_ir::FLAG_MASKS[bit] != 0 {
            atom_dependencies(graph, graph.flag_map[flag.index()], into);
        }
    }
}
fn snapshot_use(graph: &Graph, snapshot: SnapshotId, into: &mut BTreeSet<ValueId>) {
    let snapshot = &graph.source.snapshots[snapshot.index()];
    for value in snapshot.cpu.gpr {
        value_use(graph, value, into);
    }
    flags_use(graph, &snapshot.cpu.flags, u32::MAX, into);
    value_use(graph, snapshot.cpu.flags.defined, into);
    value_use(graph, snapshot.eip, into);
}

pub(super) fn allocate(graph: &Graph) -> Allocation {
    let mut points = BTreeMap::<BlockId, Vec<Point>>::new();
    let mut phis = BTreeMap::<BlockId, Vec<ValueId>>::new();
    let mut required = BTreeSet::new();
    for &block_id in &graph.reachable {
        let block = &graph.source.blocks[block_id.index()];
        let mut list = Vec::new();
        let mut admission = Point::default();
        if let Some(step) = block.steps.first() {
            snapshot_use(graph, step.before, &mut admission.uses);
        }
        list.push(admission);
        for step in &block.steps {
            for &effect_id in &step.effects {
                let effect = &graph.source.effects[effect_id.index()];
                let mut before = Point::default();
                snapshot_use(graph, effect.before, &mut before.uses);
                if let Some(guard) = effect.guard {
                    value_use(graph, guard, &mut before.uses);
                }
                match &effect.kind {
                    EffectKind::Load { address, .. } => {
                        value_use(graph, address.offset, &mut before.uses)
                    }
                    EffectKind::Store { address, value, .. } => {
                        value_use(graph, address.offset, &mut before.uses);
                        value_use(graph, *value, &mut before.uses);
                    }
                    EffectKind::Opaque {
                        inputs,
                        contract,
                        instruction,
                    } => {
                        for (index, value) in inputs.gpr.iter().enumerate() {
                            if contract.gpr_read & (1 << index) != 0 {
                                value_use(graph, *value, &mut before.uses);
                            }
                        }
                        let scan = matches!(
                            instruction.opcode,
                            crate::ir::Opcode::String(
                                crate::ir::StringOpcode::Scas | crate::ir::StringOpcode::Cmps,
                                _
                            )
                        );
                        flags_use(
                            graph,
                            &inputs.flags,
                            if scan { u32::MAX } else { contract.flags_read },
                            &mut before.uses,
                        );
                        value_use(graph, inputs.flags.defined, &mut before.uses);
                    }
                }
                list.push(before);
                let mut after = Point::default();
                for (index, node) in graph.values.iter().enumerate() {
                    if matches!(node.op, Op::EffectResult { effect, .. } if effect == effect_id) {
                        after.definitions.insert(ValueId(index as u32));
                    }
                }
                list.push(after);
            }
            if step.effects.iter().any(|id| {
                matches!(
                    graph.source.effects[id.index()].kind,
                    EffectKind::Store { .. } | EffectKind::Opaque { .. }
                )
            }) {
                let mut after = Point::default();
                snapshot_use(graph, step.after, &mut after.uses);
                list.push(after);
            }
        }
        let mut end = Point::default();
        match block.terminator {
            Terminator::Conditional {
                condition,
                state,
                taken,
                fallthrough,
            } => {
                value_use(graph, condition, &mut end.uses);
                if matches!(taken, crate::value_ir::Target::External(_))
                    || matches!(fallthrough, crate::value_ir::Target::External(_))
                {
                    snapshot_use(graph, state, &mut end.uses);
                }
            }
            Terminator::Direct {
                target: crate::value_ir::Target::External(_),
                state,
            }
            | Terminator::Stop { state } => snapshot_use(graph, state, &mut end.uses),
            Terminator::Indirect { target, state } => {
                snapshot_use(graph, state, &mut end.uses);
                value_use(graph, target, &mut end.uses);
            }
            _ => {}
        }
        list.push(end);
        for point in &list {
            required.extend(point.uses.iter().copied());
        }
        points.insert(block_id, list);
        phis.insert(block_id, Vec::new());
    }
    // Edge recipes can introduce additional atom demands, including cold-only
    // flags. Iterate to closure; a loop phi is a stopping point, not recursion.
    loop {
        let old = required.len();
        for value in required.clone() {
            if let Op::Phi { incoming, .. } = &graph.values[value.index()].op {
                for (_, input) in incoming {
                    atom_dependencies(graph, *input, &mut required);
                }
            }
        }
        if required.len() == old {
            break;
        }
    }
    for &value in &required {
        if let Op::Phi { block, .. } = graph.values[value.index()].op {
            phis.get_mut(&block).unwrap().push(value);
        }
    }
    let mut edge_uses = BTreeMap::<(BlockId, BlockId), BTreeSet<ValueId>>::new();
    for (&block, list) in &phis {
        for &phi in list {
            if let Op::Phi { incoming, .. } = &graph.values[phi.index()].op {
                for &(pred, value) in incoming {
                    if let Some(pred) = pred {
                        atom_dependencies(
                            graph,
                            value,
                            edge_uses.entry((pred, block)).or_default(),
                        );
                    }
                }
            }
        }
    }
    let mut block_use = BTreeMap::new();
    let mut block_def = BTreeMap::new();
    let mut live_in: BTreeMap<_, BTreeSet<_>> = graph
        .reachable
        .iter()
        .map(|&id| (id, BTreeSet::new()))
        .collect();
    let mut live_out = live_in.clone();
    for (&block, list) in &points {
        let mut uses = BTreeSet::new();
        let mut defs: BTreeSet<_> = phis[&block].iter().copied().collect();
        for point in list {
            uses.extend(point.uses.difference(&defs).copied());
            defs.extend(point.definitions.intersection(&required).copied());
        }
        block_use.insert(block, uses);
        block_def.insert(block, defs);
    }
    loop {
        let mut changed = false;
        for &block in graph.reachable.iter().rev() {
            let mut out = BTreeSet::new();
            for next in control::successors(graph, block) {
                out.extend(live_in[&next].iter().copied());
                if let Some(uses) = edge_uses.get(&(block, next)) {
                    out.extend(uses.iter().copied());
                }
            }
            let mut input = block_use[&block].clone();
            input.extend(out.difference(&block_def[&block]).copied());
            changed |= out != live_out[&block] || input != live_in[&block];
            live_out.insert(block, out);
            live_in.insert(block, input);
        }
        if !changed {
            break;
        }
    }
    let mut interference: BTreeMap<_, BTreeSet<_>> =
        required.iter().map(|&id| (id, BTreeSet::new())).collect();
    let mut peak_live = 0;
    let mut clique = |live: &BTreeSet<ValueId>| {
        peak_live = peak_live.max(live.len());
        for &a in live {
            for &b in live {
                if a != b {
                    interference.get_mut(&a).unwrap().insert(b);
                }
            }
        }
    };
    let mut initial = live_in[&graph.source.entry].clone();
    for &phi in &phis[&graph.source.entry] {
        if let Op::Phi { incoming, .. } = &graph.values[phi.index()].op {
            for &(pred, value) in incoming {
                if pred.is_none() {
                    atom_dependencies(graph, value, &mut initial);
                }
            }
        }
    }
    clique(&initial);
    for (&block, list) in &points {
        let mut live = live_out[&block].clone();
        clique(&live);
        for point in list.iter().rev() {
            // Definitions happen simultaneously (opaque outputs). Even an
            // unused output cannot overwrite a different still-live atom.
            let mut at_definition = live.clone();
            at_definition.extend(point.definitions.intersection(&required).copied());
            clique(&at_definition);
            for definition in &point.definitions {
                live.remove(definition);
            }
            live.extend(point.uses.iter().copied());
            clique(&live);
        }
        live.extend(phis[&block].iter().copied());
        clique(&live);
    }
    // Parallel phi copies only coalesce genuinely noninterfering source atoms.
    // Pure source expressions keep an explicit copy, evaluated before writes.
    let mut groups: Vec<_> = (0..graph.values.len()).collect();
    fn find(groups: &[usize], mut id: usize) -> usize {
        while groups[id] != id {
            id = groups[id];
        }
        id
    }
    let mut coalesced = 0;
    for list in phis.values() {
        for &phi in list {
            if let Op::Phi { incoming, .. } = &graph.values[phi.index()].op {
                for &(_, value) in incoming {
                    let value = graph.resolve(value);
                    if !required.contains(&value) {
                        continue;
                    }
                    let a = find(&groups, phi.index());
                    let b = find(&groups, value.index());
                    if a == b {
                        continue;
                    }
                    let conflict = required
                        .iter()
                        .filter(|id| find(&groups, id.index()) == a)
                        .any(|id| {
                            interference[id]
                                .iter()
                                .any(|other| find(&groups, other.index()) == b)
                        });
                    if !conflict {
                        groups[b] = a;
                        coalesced += 1;
                    }
                }
            }
        }
    }
    let mut group_edges = BTreeMap::<usize, BTreeSet<usize>>::new();
    for &value in &required {
        let group = find(&groups, value.index());
        let edges = group_edges.entry(group).or_default();
        for other in &interference[&value] {
            let other = find(&groups, other.index());
            if group != other {
                edges.insert(other);
            }
        }
    }
    let mut order: Vec<_> = group_edges.keys().copied().collect();
    order.sort_by_key(|id| (std::cmp::Reverse(group_edges[id].len()), *id));
    let mut assigned = BTreeMap::new();
    let mut count = 0;
    for group in order {
        let used: BTreeSet<_> = group_edges[&group]
            .iter()
            .filter_map(|id| assigned.get(id).copied())
            .collect();
        let color = (0..).find(|color| !used.contains(color)).unwrap();
        count = count.max(color + 1);
        assigned.insert(group, color);
    }
    let mut colors = vec![None; graph.values.len()];
    for &value in &required {
        colors[value.index()] = Some(assigned[&find(&groups, value.index())]);
    }
    Allocation {
        colors,
        count,
        atomic_values: required.len(),
        coalesced,
        peak_live,
        phis,
        required,
    }
}

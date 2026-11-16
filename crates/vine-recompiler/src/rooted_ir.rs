//! Invocation-rooted scalar SSA over the independent decoder's normal CFG.
//!
//! The source value IR supplies instruction semantics and ordered effects.
//! Only its selected entry admits host CPU input here: a discovered CALL
//! continuation is not an edge, and indirect control flow leaves this graph.
//! Flag producers with one static shape carry numeric operands through phis;
//! heterogeneous joins carry a boolean, never a runtime recipe/tag array.

use crate::{ir, value_ir as v};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

pub use v::{BinaryOp, BlockId, EffectId, EffectOutput, FlagKind, ScalarWidth};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ValueId(pub u32);
impl ValueId {
    pub const fn index(self) -> usize {
        self.0 as usize
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Input {
    Gpr(u8),
    OtherFlags,
    DefinedFlags,
    FlagBit(u8),
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Op {
    Constant(u32),
    Input(Input),
    Phi {
        block: BlockId,
        incoming: Vec<(Option<BlockId>, ValueId)>,
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
    Flag {
        bit: u8,
        kind: FlagKind,
        width: ScalarWidth,
        args: [ValueId; 4],
    },
    EffectResult {
        effect: EffectId,
        output: EffectOutput,
    },
    Alias(ValueId),
    /// An unmapped source value in a block with no normal path from entry.
    /// Retaining its explicit identity avoids inventing a numeric fallback.
    Unreachable,
}

impl Op {
    /// Includes edge operands of phis. Clients that schedule pure DAGs treat
    /// Phi/Input/EffectResult as atomic values instead of recursively emitting.
    pub fn dependencies(&self) -> Vec<ValueId> {
        match self {
            Self::Phi { incoming, .. } => incoming.iter().map(|&(_, value)| value).collect(),
            Self::Binary { left, right, .. } => vec![*left, *right],
            Self::Select { condition, yes, no } => vec![*condition, *yes, *no],
            Self::Flag { args, .. } => args.to_vec(),
            Self::Alias(value) => vec![*value],
            _ => Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Node {
    pub home: Option<BlockId>,
    pub op: Op,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    pub source: v::OptimizationStats,
    pub phis_simplified: usize,
    pub values_shared: usize,
    pub constants_folded: usize,
    pub producer_flag_parameters: usize,
    pub boolean_flag_parameters: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Graph {
    pub source: v::Graph,
    pub reachable: BTreeSet<BlockId>,
    pub predecessors: BTreeMap<BlockId, Vec<BlockId>>,
    pub dominators: BTreeMap<BlockId, BTreeSet<BlockId>>,
    pub values: Vec<Node>,
    pub value_map: Vec<ValueId>,
    pub flag_map: Vec<ValueId>,
    pub root_inputs: Vec<(ValueId, Input)>,
    pub normal_values: BTreeSet<ValueId>,
    pub exit_values: BTreeSet<ValueId>,
    pub stats: Stats,
}

fn require(condition: bool, message: &str) -> Result<(), v::Error> {
    if condition {
        Ok(())
    } else {
        Err(v::Error(message.into()))
    }
}

fn targets(terminator: &v::Terminator) -> Vec<BlockId> {
    let mut result = BTreeSet::new();
    let mut add = |target| {
        if let v::Target::Block(block) = target {
            result.insert(block);
        }
    };
    match *terminator {
        v::Terminator::Direct { target, .. } => add(target),
        v::Terminator::Conditional {
            taken, fallthrough, ..
        } => {
            add(taken);
            add(fallthrough);
        }
        v::Terminator::Indirect { .. } | v::Terminator::Stop { .. } => {}
    }
    result.into_iter().collect()
}

fn exit_state(terminator: &v::Terminator) -> v::SnapshotId {
    match *terminator {
        v::Terminator::Direct { state, .. }
        | v::Terminator::Conditional { state, .. }
        | v::Terminator::Indirect { state, .. }
        | v::Terminator::Stop { state } => state,
    }
}

type Edges = BTreeMap<BlockId, Vec<BlockId>>;
fn cfg(source: &v::Graph) -> (BTreeSet<BlockId>, Edges) {
    let mut reachable = BTreeSet::new();
    let mut work = vec![source.entry];
    while let Some(block) = work.pop() {
        if reachable.insert(block) {
            work.extend(targets(&source.blocks[block.index()].terminator));
        }
    }
    let mut predecessors: Edges = reachable.iter().map(|&block| (block, Vec::new())).collect();
    for &block in &reachable {
        for target in targets(&source.blocks[block.index()].terminator) {
            predecessors.get_mut(&target).unwrap().push(block);
        }
    }
    (reachable, predecessors)
}

fn dominators(
    entry: BlockId,
    reachable: &BTreeSet<BlockId>,
    predecessors: &Edges,
) -> BTreeMap<BlockId, BTreeSet<BlockId>> {
    let mut result: BTreeMap<_, _> = reachable
        .iter()
        .map(|&block| {
            (
                block,
                if block == entry {
                    BTreeSet::from([entry])
                } else {
                    reachable.clone()
                },
            )
        })
        .collect();
    loop {
        let mut changed = false;
        for &block in reachable {
            if block == entry {
                continue;
            }
            let mut next = reachable.clone();
            for predecessor in &predecessors[&block] {
                next = next.intersection(&result[predecessor]).copied().collect();
            }
            next.insert(block);
            if next != result[&block] {
                result.insert(block, next);
                changed = true;
            }
        }
        if !changed {
            return result;
        }
    }
}

impl Graph {
    fn push(&mut self, home: Option<BlockId>, op: Op) -> ValueId {
        let id = ValueId(self.values.len() as u32);
        self.values.push(Node { home, op });
        id
    }
    fn input(&mut self, input: Input) -> ValueId {
        if let Some(&(id, _)) = self.root_inputs.iter().find(|(_, field)| *field == input) {
            return id;
        }
        let id = self.push(None, Op::Input(input));
        self.root_inputs.push((id, input));
        id
    }
    fn checked_resolve(&self, mut id: ValueId) -> Result<ValueId, v::Error> {
        for _ in 0..=self.values.len() {
            let node = self
                .values
                .get(id.index())
                .ok_or_else(|| v::Error("invalid rooted value reference".into()))?;
            if let Op::Alias(next) = node.op {
                id = next;
            } else {
                return Ok(id);
            }
        }
        Err(v::Error("cyclic rooted aliases".into()))
    }
    /// Graphs returned by lift/from_source have passed alias-cycle validation.
    pub fn resolve(&self, id: ValueId) -> ValueId {
        self.checked_resolve(id).expect("verified rooted value")
    }
    pub fn dependencies(&self, id: ValueId) -> Vec<ValueId> {
        self.values[self.resolve(id).index()]
            .op
            .dependencies()
            .into_iter()
            .map(|id| self.resolve(id))
            .collect()
    }
    pub fn mapped_value(&self, id: v::ValueId) -> ValueId {
        self.resolve(self.value_map[id.index()])
    }
    pub fn mapped_flag(&self, id: v::FlagRef) -> ValueId {
        self.resolve(self.flag_map[id.index()])
    }
    fn incoming_cpu(&self, block: BlockId) -> Vec<(Option<BlockId>, v::CpuValues)> {
        self.predecessors[&block]
            .iter()
            .map(|&predecessor| {
                let state = exit_state(&self.source.blocks[predecessor.index()].terminator);
                (
                    Some(predecessor),
                    self.source.snapshots[state.index()].cpu.clone(),
                )
            })
            .collect()
    }
    fn dominates(&self, home: Option<BlockId>, block: BlockId) -> bool {
        home.is_none_or(|home| self.dominators[&block].contains(&home))
    }
}

type Shape = (FlagKind, ScalarWidth);

/// Monotone producer-shape analysis includes normal backedges. Root input is
/// a materialized boolean; an arbitrary runtime entry never inherits a guessed
/// predecessor's arithmetic recipe.
fn flag_shapes(graph: &Graph) -> Vec<BTreeSet<Shape>> {
    let count = graph.source.flags.len();
    let mut shapes = vec![BTreeSet::new(); count];
    let mut dependencies = vec![Vec::new(); count];
    let mut users = vec![Vec::new(); count];
    let mut work = VecDeque::new();
    let mut queued = vec![false; count];
    for (index, flag) in graph.source.flags.iter().enumerate() {
        if !graph.reachable.contains(&flag.home) {
            continue;
        }
        match flag.op {
            v::FlagOp::Leaf { kind, width, .. } => {
                shapes[index].insert((kind, width));
            }
            v::FlagOp::Parameter { block, bit } => {
                if block == graph.source.entry {
                    shapes[index].insert((FlagKind::Materialized, ScalarWidth::W32));
                }
                dependencies[index].extend(
                    graph
                        .incoming_cpu(block)
                        .iter()
                        .map(|(_, cpu)| cpu.flags.arithmetic[bit as usize].index()),
                );
            }
            v::FlagOp::Select { yes, no, .. } => {
                dependencies[index].extend([yes.index(), no.index()])
            }
            v::FlagOp::Alias(flag) => dependencies[index].push(flag.index()),
        }
        work.push_back(index);
        queued[index] = true;
    }
    for (index, inputs) in dependencies.iter().enumerate() {
        for &input in inputs {
            users[input].push(index);
        }
    }
    while let Some(index) = work.pop_front() {
        queued[index] = false;
        let mut next = shapes[index].clone();
        for &input in &dependencies[index] {
            next.extend(shapes[input].iter().copied());
        }
        if next != shapes[index] {
            shapes[index] = next;
            for &user in &users[index] {
                if !queued[user] {
                    work.push_back(user);
                    queued[user] = true;
                }
            }
        }
    }
    shapes
}

pub fn lift(region: &ir::Region) -> Result<Graph, v::Error> {
    from_source(v::lift(region)?)
}

pub fn from_source(mut source: v::Graph) -> Result<Graph, v::Error> {
    let source_stats = v::optimize(&mut source)?;
    let (reachable, predecessors) = cfg(&source);
    let dominance = dominators(source.entry, &reachable, &predecessors);
    let mut graph = Graph {
        value_map: (0..source.values.len())
            .map(|index| ValueId(index as u32))
            .collect(),
        flag_map: (0..source.flags.len())
            .map(|index| ValueId((source.values.len() + index) as u32))
            .collect(),
        values: source
            .values
            .iter()
            .map(|value| Node {
                home: value.home,
                op: Op::Unreachable,
            })
            .chain(source.flags.iter().map(|flag| Node {
                home: Some(flag.home),
                op: Op::Unreachable,
            }))
            .collect(),
        source,
        reachable,
        predecessors,
        dominators: dominance,
        root_inputs: Vec::new(),
        normal_values: BTreeSet::new(),
        exit_values: BTreeSet::new(),
        stats: Stats {
            source: source_stats,
            ..Stats::default()
        },
    };
    for index in 0..graph.source.values.len() {
        let value = graph.source.values[index].clone();
        if value
            .home
            .is_some_and(|home| !graph.reachable.contains(&home))
        {
            continue;
        }
        let map = |id: v::ValueId| graph.value_map[id.index()];
        let op = match value.op {
            v::ValueOp::Constant(value) => Op::Constant(value),
            v::ValueOp::Parameter { block, field } => {
                let mut incoming: Vec<_> = graph
                    .incoming_cpu(block)
                    .into_iter()
                    .map(|(edge, cpu)| {
                        let value = match field {
                            v::InputField::Gpr(index) => cpu.gpr[index as usize],
                            v::InputField::OtherFlags => cpu.flags.other,
                            v::InputField::DefinedFlags => cpu.flags.defined,
                        };
                        (edge, graph.value_map[value.index()])
                    })
                    .collect();
                if block == graph.source.entry {
                    let input = graph.input(match field {
                        v::InputField::Gpr(index) => Input::Gpr(index),
                        v::InputField::OtherFlags => Input::OtherFlags,
                        v::InputField::DefinedFlags => Input::DefinedFlags,
                    });
                    incoming.insert(0, (None, input));
                }
                Op::Phi { block, incoming }
            }
            v::ValueOp::Binary { op, left, right } => Op::Binary {
                op,
                left: map(left),
                right: map(right),
            },
            v::ValueOp::Select { condition, yes, no } => Op::Select {
                condition: map(condition),
                yes: map(yes),
                no: map(no),
            },
            v::ValueOp::EvaluateFlag(flag) => Op::Alias(graph.flag_map[flag.index()]),
            v::ValueOp::EffectResult { effect, output } => Op::EffectResult { effect, output },
            v::ValueOp::Alias(value) => Op::Alias(map(value)),
        };
        graph.values[index].op = op;
    }
    let shapes = flag_shapes(&graph);
    let mut recipes = vec![None; graph.source.flags.len()];
    for (index, shape) in shapes.iter().enumerate() {
        if shape.len() == 1 {
            let home = Some(graph.source.flags[index].home);
            recipes[index] = Some(std::array::from_fn(|_| graph.push(home, Op::Unreachable)));
        }
    }
    let zero = graph.push(None, Op::Constant(0));
    for (index, recipe) in recipes.iter().enumerate() {
        let flag = graph.source.flags[index].clone();
        if !graph.reachable.contains(&flag.home) {
            continue;
        }
        require(
            !shapes[index].is_empty(),
            "reachable flag lacks an input or producer",
        )?;
        if let Some(args) = *recipe {
            let &(kind, width) = shapes[index].first().unwrap();
            for (argument, id) in args.into_iter().enumerate() {
                let op = match flag.op {
                    v::FlagOp::Leaf { args, .. } => {
                        Op::Alias(graph.value_map[args[argument].index()])
                    }
                    v::FlagOp::Alias(source) => {
                        Op::Alias(recipes[source.index()].unwrap()[argument])
                    }
                    v::FlagOp::Select { condition, yes, no } => Op::Select {
                        condition: graph.value_map[condition.index()],
                        yes: recipes[yes.index()].unwrap()[argument],
                        no: recipes[no.index()].unwrap()[argument],
                    },
                    v::FlagOp::Parameter { block, bit } => {
                        let mut incoming: Vec<_> = graph
                            .incoming_cpu(block)
                            .into_iter()
                            .map(|(edge, cpu)| {
                                (
                                    edge,
                                    recipes[cpu.flags.arithmetic[bit as usize].index()].unwrap()
                                        [argument],
                                )
                            })
                            .collect();
                        if block == graph.source.entry {
                            let value = if argument == 0 {
                                graph.input(Input::FlagBit(bit))
                            } else {
                                zero
                            };
                            incoming.insert(0, (None, value));
                        }
                        Op::Phi { block, incoming }
                    }
                };
                graph.values[id.index()].op = op;
            }
            if matches!(flag.op, v::FlagOp::Parameter { .. }) {
                graph.stats.producer_flag_parameters += 1;
            }
            graph.values[graph.flag_map[index].index()].op = Op::Flag {
                bit: flag.bit,
                kind,
                width,
                args,
            };
        } else {
            let op = match flag.op {
                v::FlagOp::Parameter { block, bit } => {
                    let mut incoming: Vec<_> = graph
                        .incoming_cpu(block)
                        .into_iter()
                        .map(|(edge, cpu)| {
                            (
                                edge,
                                graph.flag_map[cpu.flags.arithmetic[bit as usize].index()],
                            )
                        })
                        .collect();
                    if block == graph.source.entry {
                        let value = graph.input(Input::FlagBit(bit));
                        incoming.insert(0, (None, value));
                    }
                    graph.stats.boolean_flag_parameters += 1;
                    Op::Phi { block, incoming }
                }
                v::FlagOp::Alias(source) => Op::Alias(graph.flag_map[source.index()]),
                v::FlagOp::Select { condition, yes, no } => Op::Select {
                    condition: graph.value_map[condition.index()],
                    yes: graph.flag_map[yes.index()],
                    no: graph.flag_map[no.index()],
                },
                v::FlagOp::Leaf { .. } => {
                    return Err(v::Error(
                        "leaf acquired multiple static producer shapes".into(),
                    ));
                }
            };
            graph.values[graph.flag_map[index].index()].op = op;
        }
    }
    simplify(&mut graph)?;
    let (normal, exits) = demands(&graph);
    graph.normal_values = normal;
    graph.exit_values = exits;
    verify(&graph)?;
    Ok(graph)
}

fn rewrite(op: &mut Op, mapping: &[ValueId]) {
    let remap = |value: &mut ValueId| *value = mapping[value.index()];
    match op {
        Op::Phi { incoming, .. } => {
            for (_, value) in incoming {
                remap(value);
            }
        }
        Op::Binary { left, right, .. } => {
            remap(left);
            remap(right);
        }
        Op::Select { condition, yes, no } => {
            remap(condition);
            remap(yes);
            remap(no);
        }
        Op::Flag { args, .. } => {
            for value in args {
                remap(value);
            }
        }
        Op::Alias(value) => remap(value),
        _ => {}
    }
}

fn simplify(graph: &mut Graph) -> Result<(), v::Error> {
    // This bounded fixpoint is optional optimization, not a correctness limit.
    // Unresolved loop phis remain explicit when the budget is exhausted.
    for _ in 0..32 {
        let mut changed = false;
        let mapping: Vec<_> = (0..graph.values.len())
            .map(|index| graph.checked_resolve(ValueId(index as u32)))
            .collect::<Result<_, _>>()?;
        let mut shared: BTreeMap<Op, Vec<ValueId>> = BTreeMap::new();
        for index in 0..graph.values.len() {
            let id = ValueId(index as u32);
            if matches!(graph.values[index].op, Op::Unreachable | Op::Alias(_)) {
                continue;
            }
            let mut op = graph.values[index].op.clone();
            rewrite(&mut op, &mapping);
            let constant = |value: ValueId| match graph.values[value.index()].op {
                Op::Constant(value) => Some(value),
                _ => None,
            };
            let replacement = match &op {
                Op::Phi { incoming, .. } => {
                    let distinct: BTreeSet<_> = incoming
                        .iter()
                        .map(|&(_, value)| value)
                        .filter(|&value| value != id)
                        .collect();
                    (distinct.len() == 1).then(|| Op::Alias(*distinct.first().unwrap()))
                }
                Op::Binary { op, left, right } => {
                    if let (Some(a), Some(b)) = (constant(*left), constant(*right)) {
                        Some(Op::Constant(v::fold_binary(*op, a, b)))
                    } else if *right == *left {
                        match op {
                            BinaryOp::And | BinaryOp::Or => Some(Op::Alias(*left)),
                            BinaryOp::Sub
                            | BinaryOp::Xor
                            | BinaryOp::Ne
                            | BinaryOp::LtU
                            | BinaryOp::LtS => Some(Op::Constant(0)),
                            BinaryOp::Eq | BinaryOp::LeU | BinaryOp::LeS => Some(Op::Constant(1)),
                            _ => None,
                        }
                    } else {
                        None
                    }
                }
                Op::Select { condition, yes, no } => {
                    if yes == no {
                        Some(Op::Alias(*yes))
                    } else {
                        constant(*condition)
                            .map(|value| Op::Alias(if value == 0 { *no } else { *yes }))
                    }
                }
                Op::Flag {
                    bit,
                    kind,
                    width,
                    args,
                } => {
                    if let [Some(a), Some(b), Some(c), Some(d)] = args.map(constant) {
                        Some(Op::Constant(v::evaluate_leaf(
                            *bit,
                            *kind,
                            *width,
                            [a, b, c, d],
                        )))
                    } else {
                        None
                    }
                }
                _ => None,
            };
            if let Some(replacement) = replacement {
                if matches!(op, Op::Phi { .. }) {
                    graph.stats.phis_simplified += 1;
                }
                if matches!(replacement, Op::Constant(_)) {
                    graph.stats.constants_folded += 1;
                }
                op = replacement;
            }
            if matches!(op, Op::Constant(_) | Op::Input(_)) {
                graph.values[index].home = None;
            }
            if !matches!(op, Op::Alias(_) | Op::EffectResult { .. } | Op::Unreachable) {
                let candidate = shared.get(&op).and_then(|candidates| {
                    candidates
                        .iter()
                        .copied()
                        .find(|&candidate| match graph.values[index].home {
                            Some(block) => {
                                graph.dominates(graph.values[candidate.index()].home, block)
                            }
                            None => graph.values[candidate.index()].home.is_none(),
                        })
                });
                if let Some(candidate) = candidate {
                    op = Op::Alias(candidate);
                    graph.stats.values_shared += 1;
                } else {
                    shared.entry(op.clone()).or_default().push(id);
                }
            }
            if op != graph.values[index].op {
                changed = true;
                graph.values[index].op = op;
            }
        }
        if !changed {
            break;
        }
    }
    for index in 0..graph.values.len() {
        let id = ValueId(index as u32);
        let resolved = graph.checked_resolve(id)?;
        if resolved != id {
            graph.values[index].op = Op::Alias(resolved);
        }
    }
    Ok(())
}

fn cpu_roots(graph: &Graph, cpu: &v::CpuValues, roots: &mut Vec<ValueId>) {
    roots.extend(cpu.gpr.map(|id| graph.value_map[id.index()]));
    roots.extend([
        graph.value_map[cpu.flags.other.index()],
        graph.value_map[cpu.flags.defined.index()],
    ]);
    roots.extend(cpu.flags.arithmetic.map(|id| graph.flag_map[id.index()]));
}

fn closure(graph: &Graph, mut work: Vec<ValueId>) -> BTreeSet<ValueId> {
    let mut result = BTreeSet::new();
    while let Some(value) = work.pop() {
        let value = graph.resolve(value);
        if result.insert(value) {
            work.extend(graph.values[value.index()].op.dependencies());
        }
    }
    result
}

pub fn demands(graph: &Graph) -> (BTreeSet<ValueId>, BTreeSet<ValueId>) {
    let mut normal = Vec::new();
    let mut exits = Vec::new();
    for snapshot in &graph.source.snapshots {
        if graph.reachable.contains(&snapshot.block) {
            cpu_roots(graph, &snapshot.cpu, &mut exits);
            exits.push(graph.value_map[snapshot.eip.index()]);
        }
    }
    for effect in &graph.source.effects {
        if !graph.reachable.contains(&effect.block) {
            continue;
        }
        if let Some(guard) = effect.guard {
            normal.push(graph.value_map[guard.index()]);
        }
        match &effect.kind {
            v::EffectKind::Load { address, .. } => {
                normal.push(graph.value_map[address.offset.index()])
            }
            v::EffectKind::Store { address, value, .. } => normal.extend([
                graph.value_map[address.offset.index()],
                graph.value_map[value.index()],
            ]),
            v::EffectKind::Opaque {
                inputs, contract, ..
            } => {
                normal.push(graph.value_map[inputs.flags.defined.index()]);
                for (index, value) in inputs.gpr.iter().enumerate() {
                    if contract.gpr_read & (1 << index) != 0 {
                        normal.push(graph.value_map[value.index()]);
                    }
                }
                for (bit, mask) in v::FLAG_MASKS.iter().enumerate() {
                    if contract.flags_read & mask != 0 {
                        normal.push(graph.flag_map[inputs.flags.arithmetic[bit].index()]);
                    }
                }
                if contract.flags_read & !v::ARITHMETIC_MASK != 0 {
                    normal.push(graph.value_map[inputs.flags.other.index()]);
                }
            }
        }
    }
    for &block in &graph.reachable {
        let terminator = &graph.source.blocks[block.index()].terminator;
        let materialize = match *terminator {
            v::Terminator::Direct { target, .. } => matches!(target, v::Target::External(_)),
            v::Terminator::Conditional {
                condition,
                taken,
                fallthrough,
                ..
            } => {
                normal.push(graph.value_map[condition.index()]);
                matches!(taken, v::Target::External(_))
                    || matches!(fallthrough, v::Target::External(_))
            }
            v::Terminator::Indirect { target, .. } => {
                normal.push(graph.value_map[target.index()]);
                true
            }
            v::Terminator::Stop { .. } => false,
        };
        if materialize {
            cpu_roots(
                graph,
                &graph.source.snapshots[exit_state(terminator).index()].cpu,
                &mut normal,
            );
        }
    }
    (closure(graph, normal), closure(graph, exits))
}

/// Pure dependencies stop at Phi (the incoming value is defined on an edge).
/// Effect results remain atomic records of completed work, never load recipes.
fn pure_order(graph: &Graph) -> Result<Vec<ValueId>, v::Error> {
    let mut colors = vec![0; graph.values.len()];
    let mut order = Vec::new();
    for root in 0..graph.values.len() {
        if colors[root] != 0 {
            continue;
        }
        let mut work = vec![(ValueId(root as u32), false)];
        while let Some((id, done)) = work.pop() {
            if done {
                colors[id.index()] = 2;
                order.push(id);
                continue;
            }
            if colors[id.index()] == 2 {
                continue;
            }
            require(colors[id.index()] == 0, "cyclic rooted pure dependencies")?;
            colors[id.index()] = 1;
            work.push((id, true));
            if !matches!(graph.values[id.index()].op, Op::Phi { .. }) {
                for dependency in graph.values[id.index()].op.dependencies() {
                    require(
                        dependency.index() < graph.values.len(),
                        "invalid rooted dependency",
                    )?;
                    if colors[dependency.index()] != 2 {
                        work.push((dependency, false));
                    }
                }
            }
        }
    }
    Ok(order)
}

pub fn verify(graph: &Graph) -> Result<(), v::Error> {
    v::verify(&graph.source)?;
    require(
        graph.values.len() <= 8_000_000,
        "rooted graph exceeds value bound",
    )?;
    let (reachable, predecessors) = cfg(&graph.source);
    require(
        graph.reachable == reachable && graph.predecessors == predecessors,
        "rooted normal CFG changed",
    )?;
    require(
        graph.dominators == dominators(graph.source.entry, &reachable, &predecessors),
        "incorrect rooted dominance",
    )?;
    require(
        graph.value_map.len() == graph.source.values.len()
            && graph.flag_map.len() == graph.source.flags.len(),
        "incomplete rooted source mapping",
    )?;
    for &value in graph.value_map.iter().chain(&graph.flag_map) {
        graph.checked_resolve(value)?;
    }
    let mut input_fields = BTreeSet::new();
    for &(id, input) in &graph.root_inputs {
        let node = graph
            .values
            .get(id.index())
            .ok_or_else(|| v::Error("invalid rooted input registration".into()))?;
        require(
            node.home.is_none() && node.op == Op::Input(input) && input_fields.insert(input),
            "root input registration is duplicated or not an input",
        )?;
    }
    for (index, node) in graph.values.iter().enumerate() {
        graph.checked_resolve(ValueId(index as u32))?;
        for dependency in node.op.dependencies() {
            require(
                dependency.index() < graph.values.len(),
                "invalid rooted dependency",
            )?;
        }
        match &node.op {
            Op::Unreachable => require(
                node.home.is_some_and(|block| !reachable.contains(&block)),
                "unfilled reachable rooted node",
            )?,
            Op::Constant(_) => require(node.home.is_none(), "rooted constant has block lifetime")?,
            Op::Input(input) => {
                require(node.home.is_none(), "root CPU input has block lifetime")?;
                require(
                    match *input {
                        Input::Gpr(index) => index < 8,
                        Input::FlagBit(bit) => bit < 6,
                        _ => true,
                    },
                    "invalid rooted CPU input",
                )?;
                require(
                    graph.root_inputs.contains(&(ValueId(index as u32), *input)),
                    "unregistered rooted input",
                )?;
            }
            Op::Phi { block, incoming } => {
                require(
                    node.home == Some(*block) && reachable.contains(block),
                    "invalid rooted phi home",
                )?;
                let expected: BTreeSet<_> = predecessors[block]
                    .iter()
                    .copied()
                    .map(Some)
                    .chain((Some(*block) == Some(graph.source.entry)).then_some(None))
                    .collect();
                let actual: BTreeSet<_> = incoming.iter().map(|&(edge, _)| edge).collect();
                require(
                    actual == expected && actual.len() == incoming.len(),
                    "phi edges do not match rooted predecessors",
                )?;
                for &(edge, value) in incoming {
                    let value = graph.checked_resolve(value)?;
                    let definition = &graph.values[value.index()];
                    require(
                        !matches!(definition.op, Op::Unreachable),
                        "phi reads unreachable definition",
                    )?;
                    if let Some(edge) = edge {
                        require(
                            graph.dominates(definition.home, edge),
                            "phi incoming value does not dominate its edge",
                        )?;
                    } else {
                        require(
                            definition.home.is_none(),
                            "root phi input depends on executed guest work",
                        )?;
                    }
                }
            }
            _ => {
                if let Op::Alias(target) = node.op
                    && node.home.is_none()
                {
                    require(
                        graph.values[graph.checked_resolve(target)?.index()]
                            .home
                            .is_none(),
                        "global alias depends on guest block",
                    )?;
                    continue;
                }
                let home = node
                    .home
                    .ok_or_else(|| v::Error("rooted expression lacks home".into()))?;
                // An alias of a constant/global input retains its source home.
                require(reachable.contains(&home), "expression in unreachable block")?;
                for dependency in node.op.dependencies() {
                    let definition = &graph.values[graph.checked_resolve(dependency)?.index()];
                    require(
                        !matches!(definition.op, Op::Unreachable)
                            && graph.dominates(definition.home, home),
                        "rooted expression dependency does not dominate use",
                    )?;
                }
                if let Op::EffectResult { effect, output } = node.op {
                    let effect = graph
                        .source
                        .effects
                        .get(effect.index())
                        .ok_or_else(|| v::Error("invalid rooted effect result".into()))?;
                    require(
                        effect.block == home,
                        "rooted effect result crosses defining block",
                    )?;
                    require(
                        match (&effect.kind, output) {
                            (v::EffectKind::Load { .. }, EffectOutput::Loaded) => true,
                            (v::EffectKind::Opaque { contract, .. }, EffectOutput::Gpr(index)) => {
                                index < 8 && contract.gpr_written & (1 << index) != 0
                            }
                            (
                                v::EffectKind::Opaque { .. },
                                EffectOutput::PackedFlags | EffectOutput::DefinedFlags,
                            ) => true,
                            _ => false,
                        },
                        "rooted output is not produced by effect",
                    )?;
                }
                if let Op::Flag { bit, .. } = node.op {
                    require(bit < 6, "invalid rooted flag bit")?;
                }
            }
        }
    }
    let order = pure_order(graph)?;
    let mut effect_dependencies: Vec<BTreeSet<EffectId>> =
        vec![BTreeSet::new(); graph.values.len()];
    for id in order {
        if let Op::EffectResult { effect, .. } = graph.values[id.index()].op {
            effect_dependencies[id.index()].insert(effect);
        } else if !matches!(graph.values[id.index()].op, Op::Phi { .. }) {
            let mut dependencies = BTreeSet::new();
            for value in graph.values[id.index()].op.dependencies() {
                dependencies.extend(effect_dependencies[value.index()].iter().copied());
            }
            effect_dependencies[id.index()] = dependencies;
        }
    }
    let check_use =
        |value: ValueId, block: BlockId, completed: &BTreeSet<EffectId>| -> Result<(), v::Error> {
            let value = graph.checked_resolve(value)?;
            let node = &graph.values[value.index()];
            require(
                !matches!(node.op, Op::Unreachable) && graph.dominates(node.home, block),
                "mapped value does not dominate use",
            )?;
            for &effect in &effect_dependencies[value.index()] {
                let producer = &graph.source.effects[effect.index()];
                require(
                    producer.block != block || completed.contains(&effect),
                    "rooted use reorders or reads an incomplete effect",
                )?;
            }
            Ok(())
        };
    let check_snapshot =
        |state: v::SnapshotId, block, completed: &BTreeSet<EffectId>| -> Result<(), v::Error> {
            let snapshot = &graph.source.snapshots[state.index()];
            let mut roots = Vec::new();
            cpu_roots(graph, &snapshot.cpu, &mut roots);
            roots.push(graph.value_map[snapshot.eip.index()]);
            for value in roots {
                check_use(value, block, completed)?;
            }
            Ok(())
        };
    for &block in &reachable {
        let mut completed = BTreeSet::new();
        for step in &graph.source.blocks[block.index()].steps {
            check_snapshot(step.before, block, &completed)?;
            for &id in &step.effects {
                let effect = &graph.source.effects[id.index()];
                let mut roots = Vec::new();
                if let Some(guard) = effect.guard {
                    roots.push(graph.value_map[guard.index()]);
                }
                match &effect.kind {
                    v::EffectKind::Load { address, .. } => {
                        roots.push(graph.value_map[address.offset.index()])
                    }
                    v::EffectKind::Store { address, value, .. } => roots.extend([
                        graph.value_map[address.offset.index()],
                        graph.value_map[value.index()],
                    ]),
                    v::EffectKind::Opaque { inputs, .. } => cpu_roots(graph, inputs, &mut roots),
                }
                for value in roots {
                    check_use(value, block, &completed)?;
                }
                completed.insert(id);
            }
            check_snapshot(step.after, block, &completed)?;
        }
        let terminator = &graph.source.blocks[block.index()].terminator;
        check_snapshot(exit_state(terminator), block, &completed)?;
        if let v::Terminator::Conditional { condition, .. }
        | v::Terminator::Indirect {
            target: condition, ..
        } = *terminator
        {
            check_use(graph.value_map[condition.index()], block, &completed)?;
        }
        for node in &graph.values {
            if let Op::Phi { incoming, .. } = &node.op {
                for &(edge, value) in incoming {
                    if edge == Some(block) {
                        check_use(value, block, &completed)?;
                    }
                }
            }
        }
    }
    let (normal, exits) = demands(graph);
    require(
        graph.normal_values == normal && graph.exit_values == exits,
        "rooted demand closure omits normal or precise-exit state",
    )?;
    Ok(())
}

#[cfg(test)]
mod tests;

//! Static continuation discovery and scalar lifting are implemented here so
//! AOT and demand compilation build exactly the same semantic graph.
use super::*;
use crate::continuation_ir::{Binding, FlagValue, Signature, Templates};

struct Builder<'a> {
    source: &'a v::Graph,
    numeric: p::Graph,
    graph: Graph,
    templates: Templates,
    signatures: Vec<Option<Signature>>,
    versions: BTreeMap<(BlockId, Signature), NodeId>,
    joined: Vec<Option<NodeId>>,
    table: BTreeMap<(NodeId, Op), ValueId>,
    numeric_constants: BTreeMap<p::ValueId, ValueId>,
    numeric_nodes: Vec<Vec<p::ValueId>>,
    scalar_nodes: Vec<Vec<v::ValueId>>,
    flag_nodes: Vec<Vec<v::FlagRef>>,
    typed: bool,
}

impl Graph {
    pub fn build_continuations(source: &v::Graph) -> Result<Self, v::Error> {
        Self::build_entries(source, &source.blocks.iter().map(|b| b.id).collect(), true)
    }
    pub fn build_entries(
        source: &v::Graph,
        entries: &BTreeSet<BlockId>,
        typed: bool,
    ) -> Result<Self, v::Error> {
        v::verify(source)?;
        if !entries.contains(&source.entry)
            || entries.iter().any(|id| id.index() >= source.blocks.len())
        {
            return Err(v::Error("invalid architectural entry set".into()));
        }
        let numeric = p::Graph::build(source)?;
        let mut templates = Templates::new();
        let signatures = if typed {
            joins::signatures(source, entries, &mut templates)?
        } else {
            vec![None; source.blocks.len()]
        };
        let graph = Self {
            values: vec![],
            raw_nodes: vec![None; source.blocks.len()],
            parameters: vec![],
            predecessors: vec![],
            successors: vec![],
            node_blocks: vec![],
            targets: vec![],
            maps: vec![],
            constant_map: BTreeMap::new(),
            dom_ranges: vec![],
            canonicalized_edges: 0,
            expanded_instructions: 0,
            template_folded: 0,
            template_shared: 0,
            stats: Stats::default(),
        };
        let mut b = Builder {
            source,
            numeric,
            graph,
            templates,
            signatures: vec![],
            versions: BTreeMap::new(),
            joined: vec![None; source.blocks.len()],
            table: BTreeMap::new(),
            numeric_constants: BTreeMap::new(),
            numeric_nodes: vec![vec![]; source.blocks.len()],
            scalar_nodes: vec![vec![]; source.blocks.len()],
            flag_nodes: vec![vec![]; source.blocks.len()],
            typed,
        };
        b.node(None, None);
        b.node(None, None);
        let raw = b.templates.raw();
        for &block in entries {
            let node = b.node(Some(block), Some(raw.clone()));
            b.graph.raw_nodes[block.index()] = Some(node);
        }
        for (index, signature) in signatures.into_iter().enumerate() {
            if let Some(signature) = signature {
                let block = BlockId(index as u32);
                let existing = b.versions.get(&(block, signature.clone())).copied();
                b.joined[index] =
                    Some(existing.unwrap_or_else(|| b.node(Some(block), Some(signature))));
            }
        }
        b.graph
            .edge(ROOT, INGRESS, b.graph.parameters[ROOT.0].clone())?;
        for node in b
            .graph
            .raw_nodes
            .iter()
            .flatten()
            .copied()
            .collect::<Vec<_>>()
        {
            b.graph
                .edge(INGRESS, node, b.graph.parameters[INGRESS.0].clone())?;
        }
        for (i, node) in b.numeric.values.iter().enumerate() {
            if let Some(home) = node.home {
                b.numeric_nodes[home.index()].push(p::ValueId(i as u32));
            }
        }
        for (i, node) in source.values.iter().enumerate() {
            if let Some(home) = node.home {
                b.scalar_nodes[home.index()].push(v::ValueId(i as u32));
            }
        }
        for index in v::dependency_order(source)? {
            if index >= source.values.len() {
                let i = index - source.values.len();
                b.flag_nodes[source.flags[i].home.index()].push(v::FlagRef(i as u32));
            }
        }
        for index in 0..b.numeric.values.len() {
            if let p::Op::Constant(n) = b.numeric.values[index].op {
                let value = b.scalar(ROOT, Op::Constant(n));
                b.numeric_constants.insert(p::ValueId(index as u32), value);
            }
        }
        for (i, node) in source.values.iter().enumerate() {
            if node.home.is_none() {
                b.graph.constant_map.insert(
                    v::ValueId(i as u32),
                    b.numeric_constants[&b.numeric.value_map[i]],
                );
            }
        }
        let mut next = 2;
        while next < b.graph.node_blocks.len() {
            b.lift(NodeId(next))?;
            next += 1;
        }
        for list in &mut b.graph.predecessors {
            list.sort_unstable();
        }
        for value in &mut b.graph.values {
            if let Op::Phi { incoming } = &mut value.op {
                incoming.sort_unstable();
            }
        }
        b.graph.dom_ranges = dominance::ranges(&b.graph.successors, &b.graph.predecessors)?;
        b.graph.verify(source)?;
        b.graph.optimize();
        b.graph.verify(source)?;
        Ok(b.graph)
    }
}

impl Builder<'_> {
    fn node(&mut self, block: Option<BlockId>, signature: Option<Signature>) -> NodeId {
        let node = NodeId(self.graph.parameters.len());
        let fields = signature
            .as_ref()
            .map_or(FIELDS, |s| 10 + s.extras as usize);
        let parameters = (0..fields)
            .map(|field| {
                self.graph.add(
                    node,
                    if node == ROOT {
                        Op::Input(field as u8)
                    } else {
                        Op::Phi { incoming: vec![] }
                    },
                )
            })
            .collect();
        self.graph.parameters.push(parameters);
        self.graph.predecessors.push(vec![]);
        self.graph.successors.push(vec![]);
        self.graph.node_blocks.push(block);
        self.graph.targets.push(BTreeMap::new());
        self.graph.maps.push(NodeMap::default());
        if let Some(block) = block {
            self.versions
                .insert((block, signature.as_ref().unwrap().clone()), node);
            self.graph.expanded_instructions += self.source.blocks[block.index()].steps.len();
        }
        self.signatures.push(signature);
        node
    }
    fn scalar(&mut self, mut home: NodeId, mut op: Op) -> ValueId {
        let mut alias = None;
        let mut constant = None;
        match &mut op {
            Op::Binary { op, left, right } => {
                use BinaryOp::*;
                if matches!(op, Add | Mul | And | Or | Xor | Eq | Ne) && right < left {
                    std::mem::swap(left, right);
                }
                let a = self.graph.number(*left);
                let b = self.graph.number(*right);
                if let (Some(a), Some(b)) = (a, b) {
                    constant = Some(v::fold_binary(*op, a, b));
                } else {
                    alias = match op {
                        Add | Or | Xor if a == Some(0) => Some(*right),
                        Add | Sub | Or | Xor | Shl | ShrU | ShrS if b == Some(0) => Some(*left),
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
                if yes == no {
                    alias = Some(*yes);
                } else if let Some(n) = self.graph.number(*condition) {
                    alias = Some(if n == 0 { *no } else { *yes });
                }
            }
            Op::Popcnt(id) => constant = self.graph.number(*id).map(u32::count_ones),
            _ => {}
        }
        if let Some(id) = alias {
            self.graph.template_folded += 1;
            return id;
        }
        if let Some(n) = constant {
            self.graph.template_folded += 1;
            op = Op::Constant(n);
        }
        if matches!(op, Op::Constant(_)) {
            home = ROOT;
        }
        let key = (home, op.clone());
        if let Some(&id) = self.table.get(&key) {
            self.graph.template_shared += 1;
            return id;
        }
        let id = self.graph.add(home, op);
        self.table.insert(key, id);
        id
    }
    fn template(&mut self, node: NodeId, template: usize, args: [ValueId; 4]) -> ValueId {
        let template = self.templates.values[template].clone();
        let mut mapping = vec![];
        for op in template.values {
            let op = match op {
                p::Op::Parameter {
                    field: p::Parameter::Gpr(index),
                    ..
                } => {
                    mapping.push(args[index as usize]);
                    continue;
                }
                p::Op::Constant(n) => Op::Constant(n),
                p::Op::Binary { op, left, right } => Op::Binary {
                    op,
                    left: mapping[left.index()],
                    right: mapping[right.index()],
                },
                p::Op::Select { condition, yes, no } => Op::Select {
                    condition: mapping[condition.index()],
                    yes: mapping[yes.index()],
                    no: mapping[no.index()],
                },
                p::Op::Popcnt(id) => Op::Popcnt(mapping[id.index()]),
                _ => unreachable!("pure continuation template"),
            };
            mapping.push(self.scalar(node, op));
        }
        mapping[template.output.index()]
    }
    fn initial_flags(&mut self, node: NodeId) -> [FlagValue; 6] {
        let shape = self.signatures[node.0].as_ref().unwrap().clone();
        std::array::from_fn(|bit| {
            let flag = &shape.flags[bit];
            let args = flag.args.map(|arg| match arg {
                Binding::Constant(n) => self.scalar(ROOT, Op::Constant(n)),
                Binding::Extra(n) => self.graph.parameters[node.0][10 + n as usize],
            });
            let payload = self.template(node, flag.template, args);
            FlagValue {
                template: flag.template,
                args,
                payload,
            }
        })
    }
    fn numeric(&self, mapping: &BTreeMap<p::ValueId, ValueId>, id: p::ValueId) -> ValueId {
        mapping
            .get(&id)
            .copied()
            .or_else(|| self.numeric_constants.get(&id).copied())
            .expect("numeric DAG dependency")
    }
    fn lift(&mut self, node: NodeId) -> Result<(), v::Error> {
        let block = self.graph.node_blocks[node.0].unwrap();
        let initial = self.initial_flags(node);
        let mut mapping = BTreeMap::new();
        for id in self.numeric_nodes[block.index()].clone() {
            let op = match self.numeric.values[id.index()].op {
                p::Op::Parameter { field, .. } => {
                    let value = match field {
                        p::Parameter::Gpr(n) => self.graph.parameters[node.0][n as usize],
                        p::Parameter::OtherFlags => self.graph.parameters[node.0][8],
                        p::Parameter::DefinedFlags => self.graph.parameters[node.0][9],
                        p::Parameter::Predicate(bit) => initial[bit as usize].payload,
                    };
                    mapping.insert(id, value);
                    continue;
                }
                p::Op::Constant(n) => Op::Constant(n),
                p::Op::Binary { op, left, right } => Op::Binary {
                    op,
                    left: self.numeric(&mapping, left),
                    right: self.numeric(&mapping, right),
                },
                p::Op::Select { condition, yes, no } => Op::Select {
                    condition: self.numeric(&mapping, condition),
                    yes: self.numeric(&mapping, yes),
                    no: self.numeric(&mapping, no),
                },
                p::Op::Popcnt(id) => Op::Popcnt(self.numeric(&mapping, id)),
                p::Op::EffectResult { effect, output } => Op::EffectResult { effect, output },
            };
            let value = self.scalar(node, op);
            mapping.insert(id, value);
        }
        for id in self.scalar_nodes[block.index()].clone() {
            let value = self.numeric(&mapping, self.numeric.value_map[id.index()]);
            self.graph.maps[node.0].values.insert(id, value);
        }
        for id in self.flag_nodes[block.index()].clone() {
            let payload = self.numeric(&mapping, self.numeric.payloads[id.index()]);
            let condition = self.numeric(&mapping, self.numeric.conditions[id.index()]);
            self.graph.maps[node.0].payloads.insert(id, payload);
            self.graph.maps[node.0].conditions.insert(id, condition);
        }
        let (targets, state) = match self.source.blocks[block.index()].terminator {
            v::Terminator::Direct { target, state } => (vec![target], state),
            v::Terminator::Conditional {
                taken,
                fallthrough,
                state,
                ..
            } => (vec![taken, fallthrough], state),
            v::Terminator::Indirect { state, .. } => {
                let frame = self.raw_frame(node, state);
                self.graph.edge(node, INGRESS, frame)?;
                return Ok(());
            }
            v::Terminator::Stop { .. } => return Ok(()),
        };
        if targets.iter().all(|t| matches!(t, v::Target::External(_))) {
            return Ok(());
        }
        let symbolic = self.symbolic_flags(node, &initial);
        let snapshot = &self.source.snapshots[state.index()];
        let gprs = snapshot.cpu.gpr.map(|id| self.graph.value_at(node, id));
        let flags = snapshot
            .cpu
            .flags
            .arithmetic
            .map(|id| symbolic[&id].clone());
        for target in targets {
            let v::Target::Block(target) = target else {
                continue;
            };
            let (destination, frame) = if self.typed {
                let destination = self.joined[target.index()]
                    .ok_or_else(|| v::Error("missing internal flag join".into()))?;
                let signature = self.signatures[destination.0].as_ref().unwrap();
                let mut frame = gprs.to_vec();
                frame.extend([
                    self.graph.value_at(node, snapshot.cpu.flags.other),
                    self.graph.value_at(node, snapshot.cpu.flags.defined),
                ]);
                for (bit, incoming) in flags.iter().enumerate() {
                    let joined = &signature.flags[bit];
                    let args = if joined.template == self.templates.identity {
                        self.graph.canonicalized_edges +=
                            usize::from(incoming.template != joined.template);
                        [incoming.payload; 4]
                    } else if joined.template == incoming.template {
                        incoming.args
                    } else {
                        return Err(v::Error("incompatible deferred flag join".into()));
                    };
                    for (arg, binding) in joined.args.iter().enumerate() {
                        if let Binding::Extra(index) = *binding {
                            if 10 + index as usize != frame.len() {
                                return Err(v::Error("invalid flag join argument layout".into()));
                            }
                            frame.push(args[arg]);
                        }
                    }
                }
                (destination, frame)
            } else {
                let raw = self.templates.raw();
                let raw_key = (target, raw.clone());
                let destination = if let Some(&id) = self.versions.get(&raw_key) {
                    id
                } else {
                    self.node(Some(target), Some(raw))
                };
                (destination, self.raw_frame(node, state))
            };
            self.graph.targets[node.0].insert(target, destination);
            self.graph.edge(node, destination, frame)?;
        }
        Ok(())
    }
    fn raw_frame(&self, node: NodeId, state: v::SnapshotId) -> Vec<ValueId> {
        let cpu = &self.source.snapshots[state.index()].cpu;
        let mut result = cpu.gpr.map(|id| self.graph.value_at(node, id)).to_vec();
        result.extend([
            self.graph.value_at(node, cpu.flags.other),
            self.graph.value_at(node, cpu.flags.defined),
        ]);
        result.extend(
            cpu.flags
                .arithmetic
                .map(|id| self.graph.payload_at(node, id)),
        );
        result
    }
    fn symbolic_flags(
        &mut self,
        node: NodeId,
        initial: &[FlagValue; 6],
    ) -> BTreeMap<v::FlagRef, FlagValue> {
        let block = self.graph.node_blocks[node.0].unwrap();
        let mut symbolic = BTreeMap::<v::FlagRef, FlagValue>::new();
        for id in self.flag_nodes[block.index()].clone() {
            let flag = &self.source.flags[id.index()];
            let payload = self.graph.payload_at(node, id);
            let value = match flag.op {
                v::FlagOp::Parameter { bit, .. } => initial[bit as usize].clone(),
                v::FlagOp::Alias(other) => symbolic[&other].clone(),
                v::FlagOp::Leaf { kind, width, args } => {
                    let mut args = args.map(|id| self.graph.value_at(node, id));
                    let result = self.templates.result(kind, width);
                    args[3] = self.template(node, result, args);
                    let template = self.templates.leaf(flag.bit, kind, width);
                    FlagValue {
                        template,
                        args,
                        payload,
                    }
                }
                v::FlagOp::Select { condition, yes, no } => {
                    let condition = self.graph.value_at(node, condition);
                    let yes = &symbolic[&yes];
                    let no = &symbolic[&no];
                    if let Some(n) = self.graph.number(condition) {
                        if n == 0 { no.clone() } else { yes.clone() }
                    } else if yes.template == no.template {
                        let template = yes.template;
                        let a = yes.args;
                        let b = no.args;
                        let args = std::array::from_fn(|i| {
                            self.scalar(
                                node,
                                Op::Select {
                                    condition,
                                    yes: a[i],
                                    no: b[i],
                                },
                            )
                        });
                        FlagValue {
                            template,
                            args,
                            payload,
                        }
                    } else {
                        FlagValue {
                            template: self.templates.identity,
                            args: [payload; 4],
                            payload,
                        }
                    }
                }
            };
            symbolic.insert(id, value);
        }
        symbolic
    }
}

use super::*;
use crate::flow_ir::{self as f, NodeId, Op, ValueId as FlowValue};

pub(super) struct Plan {
    pub graph: f::Graph,
    pub storage: Vec<Option<u32>>,
    pub locals: u32,
    temporary_base: u32,
    temporary_count: Cell<u32>,
    current: Cell<NodeId>,
    completed: Cell<bool>,
    prefix: Cell<u32>,
    chains: Vec<Vec<NodeId>>,
    allocation: flow_allocate::Allocation,
    components: Vec<Vec<NodeId>>,
    routes: Vec<(usize, usize)>,
    cyclic: Vec<bool>,
    copies: Cell<usize>,
}
impl Plan {
    pub(super) fn new(
        source: &Graph,
        entries: &BTreeSet<BlockId>,
        base: u32,
    ) -> Result<Self, String> {
        let graph = f::Graph::build_entries(source, entries, true).map_err(|e| e.to_string())?;
        // Fuse straight, sole-predecessor chains after continuation interning.
        // Entry ports and shared suffixes remain real join points. A cell is
        // never given an artificial architectural ingress just to emit it.
        let count = graph.node_blocks.len();
        let next = |id: usize| -> Option<NodeId> {
            let block = &source.blocks[graph.node_blocks[id]?.index()];
            if !matches!(
                block.terminator,
                Terminator::Direct {
                    target: Target::Block(_),
                    ..
                }
            ) || graph.successors[id].len() != 1
            {
                return None;
            }
            let to = graph.successors[id][0];
            (to.0 >= 2 && to.0 != id && graph.predecessors[to.0] == [NodeId(id)]).then_some(to)
        };
        let mut can_fuse = vec![false; count];
        for id in 2..count {
            if let Some(to) = next(id) {
                can_fuse[to.0] = true;
            }
        }
        let mut chains = vec![vec![]; count];
        let mut owner = vec![usize::MAX; count];
        let mut leaders = Vec::new();
        for leader in (2..count).filter(|id| !can_fuse[*id]).chain(2..count) {
            if owner[leader] != usize::MAX {
                continue;
            }
            let chain = leaders.len();
            leaders.push(NodeId(leader));
            let mut node = NodeId(leader);
            loop {
                owner[node.0] = chain;
                chains[leader].push(node);
                let Some(to) = next(node.0) else {
                    break;
                };
                if owner[to.0] != usize::MAX {
                    break;
                }
                node = to;
            }
        }
        // Only chain leaders have budget admission/recovery. Interior cells
        // can still fault at their actual effects, but never at a fabricated
        // instruction boundary. Allocate against that same emitted control.
        let admission = leaders.iter().copied().collect();
        let allocation = flow_allocate::allocate(&graph, source, &admission);
        let storage: Vec<_> = allocation
            .colors
            .iter()
            .map(|c| c.map(|n| base + n))
            .collect();
        let locals = allocation.count;
        if locals > 49_000 {
            return Err("flow SSA exceeds bounded Wasm local storage".into());
        }
        let edges: Vec<Vec<usize>> = leaders
            .iter()
            .map(|leader| {
                let tail = chains[leader.0].last().unwrap();
                graph.successors[tail.0]
                    .iter()
                    .filter(|n| n.0 >= 2)
                    .map(|n| owner[n.0])
                    .collect()
            })
            .collect();
        let mut groups = f::components(&edges);
        for group in &mut groups {
            let members: BTreeSet<_> = group.iter().copied().collect();
            let entries: Vec<_> = group
                .iter()
                .copied()
                .filter(|id| {
                    graph.predecessors[leaders[*id].0].iter().any(|p| {
                        p.0 == f::INGRESS.0 || (p.0 >= 2 && !members.contains(&owner[p.0]))
                    })
                })
                .collect();
            if entries.len() == 1 {
                let at = group.iter().position(|n| *n == entries[0]).unwrap();
                group.swap(0, at);
            }
        }
        let cyclic = groups
            .iter()
            .map(|g| g.len() > 1 || edges[g[0]].contains(&g[0]))
            .collect();
        let mut routes = vec![(0, 0); count];
        for (c, group) in groups.iter().enumerate() {
            for (slot, &chain) in group.iter().enumerate() {
                for node in &chains[leaders[chain].0] {
                    routes[node.0] = (c, slot);
                }
            }
        }
        for (from, list) in edges.iter().enumerate() {
            for &to in list {
                if routes[leaders[from].0].0 > routes[leaders[to].0].0 {
                    return Err("flow control condensation is not topological".into());
                }
            }
        }
        let components = groups
            .into_iter()
            .map(|g| g.into_iter().map(|n| leaders[n]).collect())
            .collect();
        Ok(Self {
            graph,
            storage,
            locals,
            temporary_base: base + allocation.count,
            temporary_count: Cell::new(0),
            current: Cell::new(f::ROOT),
            completed: Cell::new(false),
            prefix: Cell::new(0),
            chains,
            allocation,
            components,
            routes,
            cyclic,
            copies: Cell::new(0),
        })
    }
    pub(super) fn prefix(&self) -> u32 {
        self.prefix.get()
    }
    pub(super) fn current(&self) -> NodeId {
        self.current.get()
    }
    pub(super) fn local_count(&self) -> u32 {
        self.locals + self.temporary_count.get()
    }
    pub(super) fn metadata(&self) -> Vec<u8> {
        let raw = self.graph.raw_nodes.iter().flatten().count();
        format!("{{\"version\":3,\"nodes\":{},\"rawEntries\":{},\"sourceCells\":{},\"fusedChains\":{},\"typedVersions\":{},\"expandedInstructions\":{},\"canonicalizedEdges\":{},\"values\":{},\"phisRemoved\":{},\"folded\":{},\"shared\":{},\"templateFolded\":{},\"templateShared\":{},\"atomicLocals\":{},\"ssaLocals\":{},\"peakLiveAtoms\":{},\"coalesced\":{},\"edgeCopies\":{},\"staticComponents\":{},\"cyclicComponents\":{},\"arbitraryEntry\":true,\"localFaultRecovery\":true}}",
            self.graph.successors.len(),raw,self.graph.raw_nodes.len(),self.chains.iter().filter(|c| !c.is_empty()).count(),self.graph.node_blocks.len()-2-raw,self.graph.expanded_instructions,self.graph.canonicalized_edges,self.graph.values.len(),self.graph.stats.phis_removed,self.graph.stats.folded,self.graph.stats.shared,self.graph.template_folded,self.graph.template_shared,self.allocation.count,self.local_count(),self.allocation.peak,self.allocation.coalesced,self.copies.get(),self.components.len(),self.cyclic.iter().filter(|v| **v).count()).into_bytes()
    }
    pub(super) fn value(&self, e: &mut Emitter, id: FlowValue, state: &mut Emission) {
        // The public discovery bound permits long scalar chains. Their depth
        // is not permission to recurse through the compiler's Wasm stack.
        let mut work = vec![(id, false)];
        while let Some((id, finish)) = work.pop() {
            let id = self.graph.resolve(id);
            let op = &self.graph.values[id.0].op;
            if !finish
                && self.allocation.materialized[id.0]
                && (self.completed.get() || self.graph.values[id.0].home != self.current.get())
            {
                e.get(self.storage[id.0].expect("live materialized SSA value"));
                continue;
            }
            if finish {
                e.op(match op {
                    Op::Binary { op, .. } => binary_opcode(*op),
                    Op::Select { .. } => 0x1b,
                    Op::Popcnt(_) => 0x69,
                    _ => unreachable!("pure finish"),
                });
                let local = self.temporary_base + state.flow_values.len() as u32;
                self.temporary_count.set(
                    self.temporary_count
                        .get()
                        .max(local - self.temporary_base + 1),
                );
                e.tee(local);
                state.flow_values.insert(id, local);
                continue;
            }
            match *op {
                Op::Constant(n) => {
                    e.num(n);
                    continue;
                }
                Op::Input(_) | Op::Phi { .. } | Op::EffectResult { .. } => {
                    e.get(self.storage[id.0].expect("live SSA atom"));
                    continue;
                }
                _ => {}
            }
            if let Some(&local) = state.flow_values.get(&id) {
                e.get(local);
                continue;
            }
            work.push((id, true));
            match *op {
                Op::Binary { left, right, .. } => {
                    work.push((right, false));
                    work.push((left, false));
                }
                Op::Select { condition, yes, no } => {
                    work.push((condition, false));
                    work.push((no, false));
                    work.push((yes, false));
                }
                Op::Popcnt(value) => work.push((value, false)),
                _ => unreachable!("atom handled before pure emission"),
            }
        }
    }
    fn materialize(&self, e: &mut Emitter, state: &mut Emission) {
        // Parallel definitions may share a color with dying inputs. Evaluate
        // every result before overwriting those inputs, exactly as for phis.
        let mut destinations = Vec::new();
        for &id in &self.allocation.definitions[self.current.get().0] {
            if let Some(local) = self.storage[id.0] {
                self.value(e, id, state);
                destinations.push(local);
            }
        }
        for local in destinations.into_iter().rev() {
            e.set(local);
        }
        self.completed.set(true);
    }
    pub(super) fn transfer(&self, e: &mut Emitter, from: NodeId, to: NodeId, state: &mut Emission) {
        let mut copies = Vec::new();
        for &phi in &self.allocation.phis[to.0] {
            let Op::Phi { incoming } = &self.graph.values[phi.0].op else {
                unreachable!()
            };
            let source = self.graph.resolve(
                incoming
                    .iter()
                    .find(|(n, _)| *n == from)
                    .expect("real CFG edge")
                    .1,
            );
            let destination = self.storage[phi.0].expect("live phi");
            if self.storage[source.0] == Some(destination)
                && (self.allocation.materialized[source.0]
                    || matches!(
                        self.graph.values[source.0].op,
                        Op::Input(_) | Op::Phi { .. } | Op::EffectResult { .. }
                    ))
            {
                continue;
            }
            self.value(e, source, state);
            copies.push(destination);
        }
        self.copies.set(self.copies.get() + copies.len());
        for local in copies.into_iter().rev() {
            e.set(local);
        }
    }
}

impl super::Plan {
    pub(in crate::wasm) fn is_flow(&self) -> bool {
        self.flow.is_some()
    }
    pub(in crate::wasm) fn flow_partial_jump(&self, e: &mut Emitter, pc: u32) {
        if let Some(&id) = self.graph.entries.get(&pc) {
            let label = format!("flow-partial-{}", id.0);
            if e.labels.contains(&label) {
                e.branch(&label);
            } else {
                e.num(id.0);
                e.set(ROUTE);
                e.branch("flow-partial-dispatch");
            }
        } else {
            e.exit(2);
        }
    }
    pub(in crate::wasm) fn flow_partial_indirect(&self, e: &mut Emitter) {
        let entries = self
            .graph
            .entries
            .iter()
            .map(|(&pc, id)| (pc, id.index()))
            .collect::<Vec<_>>();
        e.num(u32::MAX);
        e.set(ROUTE);
        e.lookup_tree(&entries);
        e.branch("flow-partial-dispatch");
    }
    fn emit_flow_chain(&self, e: &mut Emitter, leader: NodeId) {
        let plan = self.flow.as_ref().unwrap();
        let chain = &plan.chains[leader.0];
        plan.current.set(leader);
        plan.completed.set(false);
        plan.prefix.set(0);
        self.initialize_entry(e);
        let steps: usize = chain
            .iter()
            .map(|id| {
                self.graph.blocks[plan.graph.node_blocks[id.0].unwrap().index()]
                    .steps
                    .len()
            })
            .sum();
        e.get(LEFT);
        e.num(steps as u32);
        e.op(0x4f);
        e.if_();
        let mut state = Emission::default();
        let mut prefix = 0;
        for (index, &id) in chain.iter().enumerate() {
            plan.current.set(id);
            plan.completed.set(false);
            plan.prefix.set(prefix);
            // Cross-cell values now have allocated SSA storage. Expression
            // temporaries only belong to this cell, including its local fault
            // recovery, and must not grow with the length of a fused chain.
            state.flow_values.clear();
            let block = &self.graph.blocks[plan.graph.node_blocks[id.0].unwrap().index()];
            self.emit_effects(e, block, &mut state);
            plan.materialize(e, &mut state);
            if let Some(&next) = chain.get(index + 1) {
                plan.transfer(e, id, next, &mut state);
            } else {
                self.emit_terminator(e, block, &mut state);
            }
            prefix += block.steps.len() as u32;
        }
        e.op(0x05);
        plan.current.set(leader);
        plan.completed.set(false);
        plan.prefix.set(0);
        let block = &self.graph.blocks[plan.graph.node_blocks[leader.0].unwrap().index()];
        self.emit_exit_snapshot(e, ExitContext::exact(block.steps[0].before));
        e.num(block.id.0);
        e.set(ROUTE);
        e.branch("flow-partial");
        e.end();
    }
    fn flow_canonical_ingress(&self, e: &mut Emitter) {
        let plan = self.flow.as_ref().unwrap();
        e.get(self.base + READY);
        e.op(0x45);
        e.if_();
        // Architectural scratch can change in exact opaque operations. The
        // root's SSA inputs therefore have private storage, initialized in a
        // parallel transfer before entering the graph.
        for field in 0..f::FIELDS {
            match field {
                0..=7 => e.get(3 + field as u32),
                8 => {
                    e.get(FLAGS);
                    e.num(!ARITHMETIC_MASK);
                    e.op(0x71);
                }
                9 => e.get(DEFINED),
                _ => {
                    let bit = field - 10;
                    e.get(FLAGS);
                    e.num(FLAG_MASKS[bit].trailing_zeros());
                    e.op(0x76);
                    e.num(1);
                    e.op(0x71);
                    match bit {
                        1 | 3 => e.op(0x45),
                        2 => {
                            e.num(4);
                            e.op(0x74);
                        }
                        4 | 5 => {
                            e.num(31);
                            e.op(0x74);
                        }
                        _ => {}
                    }
                }
            }
        }
        for &id in plan.graph.parameters[f::ROOT.0].iter().rev() {
            if let Some(local) = plan.storage[id.0] {
                e.set(local);
            } else {
                e.op(0x1a);
            }
        }
        plan.transfer(e, f::ROOT, f::INGRESS, &mut Emission::default());
        e.end();
    }
    fn flow_lookup(&self, e: &mut Emitter, entries: &[(u32, BlockId)]) {
        if entries.is_empty() {
            return;
        }
        let plan = self.flow.as_ref().unwrap();
        let mid = entries.len() / 2;
        let (pc, id) = entries[mid];
        e.get(PC);
        e.num(pc);
        e.op(0x46);
        e.if_();
        let raw = plan.graph.raw_nodes[id.index()].expect("architectural entry port");
        plan.transfer(e, f::INGRESS, raw, &mut Emission::default());
        let (component, slot) = plan.routes[raw.0];
        e.num(slot as u32);
        e.set(ROUTE);
        e.num(component as u32);
        e.set(self.base + EXIT_CONTEXT);
        e.op(0x05);
        if entries.len() > 1 {
            e.get(PC);
            e.num(pc);
            e.op(0x49);
            e.if_();
            self.flow_lookup(e, &entries[..mid]);
            e.op(0x05);
            self.flow_lookup(e, &entries[mid + 1..]);
            e.end();
        }
        e.end();
    }
    pub(super) fn flow_jump(&self, e: &mut Emitter, target: NodeId) {
        let plan = self.flow.as_ref().unwrap();
        let (c, slot) = plan.routes[target.0];
        let block = format!("flow-b{}", target.0);
        if e.labels.contains(&block) {
            e.branch(&block);
            return;
        }
        let component = format!("flow-c{c}");
        if e.labels.contains(&component) {
            e.num(slot as u32);
            e.set(ROUTE);
            e.num(1);
            e.set(self.base); // enter through the selected interior port
            e.branch(&component);
        } else {
            e.num(slot as u32);
            e.set(ROUTE);
            e.num(u32::from(slot != 0));
            e.set(self.base);
            e.branch(&format!("flow-loop{c}"));
        }
    }
    pub(in crate::wasm) fn emit_flow(&self, e: &mut Emitter, region: &Region) {
        let plan = self.flow.as_ref().unwrap();
        e.open(0x03, "lookup".into());
        self.flow_canonical_ingress(e);
        e.num(u32::MAX);
        e.set(self.base + EXIT_CONTEXT);
        let entries: Vec<_> = self
            .graph
            .entries
            .iter()
            .filter(|(_, id)| plan.graph.raw_nodes[id.index()].is_some())
            .map(|(&pc, &id)| (pc, id))
            .collect();
        self.flow_lookup(e, &entries);
        e.get(self.base + EXIT_CONTEXT);
        e.num(u32::MAX);
        e.op(0x46);
        e.if_();
        let mut state = Emission::default();
        let frame = &plan.graph.parameters[f::INGRESS.0];
        for (i, &id) in frame[..8].iter().enumerate() {
            plan.value(e, id, &mut state);
            e.set(3 + i as u32);
        }
        plan.value(e, frame[8], &mut state);
        for (bit, mask) in FLAG_MASKS.iter().enumerate() {
            plan.value(e, frame[10 + bit], &mut state);
            Self::predicate_condition(e, bit);
            e.num(mask.trailing_zeros());
            e.op(0x74);
            e.op(0x72);
        }
        e.set(FLAGS);
        plan.value(e, frame[9], &mut state);
        e.set(DEFINED);
        e.exit(2);
        e.end();
        e.num(1);
        e.set(self.base);
        e.open(0x02, "flow-partial".into());
        e.open(0x02, "flow-invalid-component".into());
        for c in (0..plan.components.len()).rev() {
            e.open(0x02, format!("flow-c{c}"));
        }
        e.get(self.base + EXIT_CONTEXT);
        e.op(0x0e);
        uleb(&mut e.code, plan.components.len() as u32);
        for c in 0..plan.components.len() {
            uleb(&mut e.code, c as u32);
        }
        // Invalid selectors have already exited above; trap, never publish.
        uleb(&mut e.code, plan.components.len() as u32);
        for (c, group) in plan.components.iter().enumerate() {
            e.end();
            if plan.cyclic[c] {
                e.open(0x03, format!("flow-loop{c}"));
                e.open(0x02, format!("flow-invalid-port{c}"));
                for id in group.iter().rev() {
                    e.open(0x02, format!("flow-b{}", id.0));
                }
                e.get(self.base);
                e.if_();
                e.get(ROUTE);
                e.op(0x0e);
                uleb(&mut e.code, group.len() as u32);
                for slot in 0..group.len() {
                    uleb(&mut e.code, 1 + slot as u32);
                }
                // A compiler-owned selector outside the emitted ports traps.
                uleb(&mut e.code, 1 + group.len() as u32);
                e.end();
            }
            for id in group {
                if plan.cyclic[c] {
                    e.end();
                }
                self.emit_flow_chain(e, *id);
            }
            if plan.cyclic[c] {
                e.end();
                e.op(0x00);
                e.end();
            }
        }
        e.end();
        e.op(0x00); // no terminator can fall through the static condensation
        e.end(); // flow-partial: only packed architectural locals are live
        let normalized = flow_blocks::normalize(region).expect("validated flow blocks");
        e.open(0x03, "flow-partial-dispatch".into());
        e.open(0x02, "flow-invalid-partial".into());
        for block in self.graph.blocks.iter().rev() {
            e.open(0x02, format!("flow-partial-{}", block.id.0));
        }
        e.get(ROUTE);
        e.op(0x0e);
        uleb(&mut e.code, self.graph.blocks.len() as u32);
        for block in &self.graph.blocks {
            uleb(&mut e.code, block.id.0);
        }
        uleb(&mut e.code, self.graph.blocks.len() as u32);
        e.flow_partial = true;
        for block in &self.graph.blocks {
            e.end();
            let old = &normalized.blocks[&block.start];
            for instruction in &old.instructions {
                e.instruction(instruction, None);
            }
            e.block_fallthrough(old);
        }
        e.flow_partial = false;
        e.end();
        e.exit(2); // unknown indirect target, with canonical architectural locals
        e.end();
        e.end();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode::{CodeSource, discover};

    struct Code(BTreeMap<u32, u8>);
    impl CodeSource for Code {
        fn read_code(&self, address: u32, max: usize) -> Vec<u8> {
            (0..max)
                .map_while(|i| self.0.get(&address.wrapping_add(i as u32)).copied())
                .collect()
        }
    }
    #[test]
    fn basic_blocks_keep_raw_entries_and_ordered_instruction_snapshots() {
        let parts: &[(u32, &[u8])] = &[
            (0x1000, &[0x74, 0x0e, 0x83, 0xc0, 1, 0xeb, 0x19]),
            (0x1010, &[0x31, 0xd2, 0xeb, 0x0c]),
            (0x1020, &[0x01, 0xcb, 0x8b, 0x06, 0x47, 0xeb, 0x09]),
            (0x1030, &[0x0f, 0x0b]),
        ];
        let code = Code(
            parts
                .iter()
                .flat_map(|(at, bytes)| {
                    bytes
                        .iter()
                        .enumerate()
                        .map(move |(i, b)| (at + i as u32, *b))
                })
                .collect(),
        );
        let region = discover(&code, 0x1000, 128);
        let mut source = v::lift(&flow_blocks::normalize(&region).unwrap()).unwrap();
        v::optimize(&mut source).unwrap();
        let entries = region.blocks.keys().map(|pc| source.entries[pc]).collect();
        let plan = Plan::new(&source, &entries, 50).unwrap();
        let before = source.entries[&0x1020];
        assert!(
            plan.graph
                .node_blocks
                .iter()
                .filter(|b| **b == Some(before))
                .count()
                <= 2
        );
        assert!(!source.entries.contains_key(&0x1022));
        assert_eq!(source.blocks[before.index()].steps.len(), 4);
        assert!(plan.graph.raw_nodes[before.index()].is_some());
        assert!(plan.graph.raw_nodes[source.entry.index()].is_some());
    }
}

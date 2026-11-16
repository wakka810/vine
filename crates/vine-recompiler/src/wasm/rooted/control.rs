//! Structure only real normal edges. No guest block is cloned and no guest PC
//! search occurs on these edges. Multi-entry SCCs retain a local dense route.
use crate::rooted_ir::Graph;
use crate::value_ir::{BlockId, Target, Terminator};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug)]
pub(super) enum Part {
    Block(BlockId),
    Loop { header: BlockId, body: Vec<Part> },
    Multiple(Vec<BlockId>),
}

impl Part {
    pub(super) fn entries(&self) -> Vec<BlockId> {
        match self {
            Self::Block(id) => vec![*id],
            Self::Loop { header, .. } => vec![*header],
            Self::Multiple(ids) => ids.clone(),
        }
    }
}

pub(super) fn successors(graph: &Graph, block: BlockId) -> Vec<BlockId> {
    let targets = match graph.source.blocks[block.index()].terminator {
        Terminator::Direct { target, .. } => vec![target],
        Terminator::Conditional {
            taken, fallthrough, ..
        } => vec![taken, fallthrough],
        _ => vec![],
    };
    let mut result = Vec::new();
    for target in targets {
        if let Target::Block(id) = target
            && graph.reachable.contains(&id)
            && !result.contains(&id)
        {
            result.push(id);
        }
    }
    result
}

pub(super) fn structure(graph: &Graph) -> Vec<Part> {
    order(graph, &graph.reachable)
}

fn order(graph: &Graph, nodes: &BTreeSet<BlockId>) -> Vec<Part> {
    // Kosaraju, followed by a stable topological SCC order. Bounds are the
    // decoded unit's bounds, independent of guest addresses and hotness.
    fn visit(
        graph: &Graph,
        node: BlockId,
        nodes: &BTreeSet<BlockId>,
        seen: &mut BTreeSet<BlockId>,
        post: &mut Vec<BlockId>,
    ) {
        if !nodes.contains(&node) || !seen.insert(node) {
            return;
        }
        for target in successors(graph, node) {
            visit(graph, target, nodes, seen, post);
        }
        post.push(node);
    }
    fn reverse(
        graph: &Graph,
        node: BlockId,
        nodes: &BTreeSet<BlockId>,
        seen: &mut BTreeSet<BlockId>,
        component: &mut BTreeSet<BlockId>,
    ) {
        if !nodes.contains(&node) || !seen.insert(node) {
            return;
        }
        component.insert(node);
        for &pred in &graph.predecessors[&node] {
            reverse(graph, pred, nodes, seen, component);
        }
    }
    let mut seen = BTreeSet::new();
    let mut post = Vec::new();
    for &node in nodes {
        visit(graph, node, nodes, &mut seen, &mut post);
    }
    seen.clear();
    let mut components = Vec::new();
    for node in post.into_iter().rev() {
        if seen.contains(&node) {
            continue;
        }
        let mut component = BTreeSet::new();
        reverse(graph, node, nodes, &mut seen, &mut component);
        components.push(component);
    }
    let index: BTreeMap<_, _> = components
        .iter()
        .enumerate()
        .flat_map(|(i, set)| set.iter().map(move |&id| (id, i)))
        .collect();
    let mut incoming = vec![BTreeSet::new(); components.len()];
    for &node in nodes {
        for next in successors(graph, node) {
            if nodes.contains(&next) && index[&node] != index[&next] {
                incoming[index[&next]].insert(index[&node]);
            }
        }
    }
    let mut done = BTreeSet::new();
    let mut result = Vec::new();
    while done.len() != components.len() {
        let component_index = (0..components.len())
            .find(|i| !done.contains(i) && incoming[*i].is_subset(&done))
            .expect("SCC condensation is acyclic");
        done.insert(component_index);
        let component = &components[component_index];
        let first = *component.first().expect("nonempty SCC");
        let cyclic = component.len() > 1 || successors(graph, first).contains(&first);
        if !cyclic {
            result.push(Part::Block(first));
            continue;
        }
        let entries: BTreeSet<_> = component
            .iter()
            .copied()
            .filter(|id| {
                *id == graph.source.entry
                    || graph.predecessors[id]
                        .iter()
                        .any(|pred| !component.contains(pred))
            })
            .collect();
        if entries.len() == 1 {
            let header = *entries.first().unwrap();
            let mut rest = component.clone();
            rest.remove(&header);
            let mut body = vec![Part::Block(header)];
            body.extend(order(graph, &rest));
            result.push(Part::Loop { header, body });
        } else {
            result.push(Part::Multiple(component.iter().copied().collect()));
        }
    }
    result
}

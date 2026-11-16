//! Same-function control regions for multi-entry machine code.
//!
//! Cyclic strongly connected components get separate Wasm loop scopes. The
//! component DAG is laid out topologically, so static transfers between scopes
//! are forward branches. Every decoded block keeps its original admission PC;
//! no function boundary, conventional CALL/RET pairing or hot entry is assumed.
use crate::ir::Region;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug)]
pub(crate) struct Target {
    pub group: usize,
    pub index: usize,
}

#[derive(Debug)]
pub(crate) struct Group {
    pub blocks: Vec<u32>,
    pub first: usize,
    pub cyclic: bool,
}

#[derive(Debug)]
pub(crate) struct Plan {
    pub groups: Vec<Group>,
    pub targets: BTreeMap<u32, Target>,
}

impl Plan {
    pub fn new(region: &Region) -> Self {
        let pcs: Vec<_> = region.blocks.keys().copied().collect();
        let indices: BTreeMap<_, _> = pcs.iter().enumerate().map(|(i, &pc)| (pc, i)).collect();
        let edges: Vec<Vec<usize>> = pcs
            .iter()
            .map(|pc| {
                region.blocks[pc]
                    .successors
                    .iter()
                    .filter_map(|next| indices.get(next).copied())
                    .collect()
            })
            .collect();
        let mut reverse = vec![Vec::new(); pcs.len()];
        for (source, successors) in edges.iter().enumerate() {
            for &target in successors {
                reverse[target].push(source);
            }
        }
        // Iterative Kosaraju keeps compiler stack use bounded even for the
        // public maximum-sized region or adversarial cycles and entry order.
        let mut seen = vec![false; pcs.len()];
        let mut order = Vec::new();
        for root in 0..pcs.len() {
            let mut work = vec![(root, false)];
            while let Some((node, done)) = work.pop() {
                if done {
                    order.push(node);
                } else if !seen[node] {
                    seen[node] = true;
                    work.push((node, true));
                    work.extend(edges[node].iter().rev().map(|&next| (next, false)));
                }
            }
        }
        let mut components = Vec::<Vec<usize>>::new();
        let mut owners = vec![usize::MAX; pcs.len()];
        for root in order.into_iter().rev() {
            if owners[root] != usize::MAX {
                continue;
            }
            let id = components.len();
            let mut members = Vec::new();
            let mut work = vec![root];
            while let Some(node) = work.pop() {
                if owners[node] == usize::MAX {
                    owners[node] = id;
                    members.push(node);
                    work.extend(&reverse[node]);
                }
            }
            members.sort_unstable();
            components.push(members);
        }
        let mut outgoing = vec![BTreeSet::new(); components.len()];
        let mut incoming = vec![0; components.len()];
        for (source, successors) in edges.iter().enumerate() {
            for &target in successors {
                let a = owners[source];
                let b = owners[target];
                if a != b && outgoing[a].insert(b) {
                    incoming[b] += 1;
                }
            }
        }
        // Preserve address order where the component DAG leaves a choice.
        let mut ready: BTreeSet<_> = components
            .iter()
            .enumerate()
            .filter(|(id, _)| incoming[*id] == 0)
            .map(|(id, nodes)| (pcs[nodes[0]], id))
            .collect();
        let mut groups = Vec::new();
        let mut targets = BTreeMap::new();
        while let Some((_, id)) = ready.pop_first() {
            let nodes = &components[id];
            let first = targets.len();
            let group = groups.len();
            let blocks: Vec<_> = nodes.iter().map(|&node| pcs[node]).collect();
            for (offset, &pc) in blocks.iter().enumerate() {
                targets.insert(
                    pc,
                    Target {
                        group,
                        index: first + offset,
                    },
                );
            }
            groups.push(Group {
                blocks,
                first,
                cyclic: nodes.len() > 1 || edges[nodes[0]].contains(&nodes[0]),
            });
            for &next in &outgoing[id] {
                incoming[next] -= 1;
                if incoming[next] == 0 {
                    ready.insert((pcs[components[next][0]], next));
                }
            }
        }
        assert_eq!(
            targets.len(),
            pcs.len(),
            "component DAG contains every entry"
        );
        Self { groups, targets }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::Block;

    fn graph(edges: &[(u32, &[u32])]) -> Region {
        Region {
            entry: edges[0].0,
            limit_reached: false,
            blocks: edges
                .iter()
                .map(|&(pc, next)| {
                    (
                        pc,
                        Block {
                            start: pc,
                            instructions: Vec::new(),
                            successors: next.to_vec(),
                        },
                    )
                })
                .collect(),
        }
    }
    fn verify(region: &Region, plan: &Plan) {
        assert_eq!(plan.targets.len(), region.blocks.len());
        let mut all = BTreeSet::new();
        for (id, group) in plan.groups.iter().enumerate() {
            for (offset, pc) in group.blocks.iter().enumerate() {
                assert!(all.insert(*pc));
                assert_eq!(plan.targets[pc].group, id);
                assert_eq!(plan.targets[pc].index, group.first + offset);
                for next in &region.blocks[pc].successors {
                    if let Some(target) = plan.targets.get(next) {
                        assert!(target.group >= id);
                    }
                }
            }
        }
    }

    #[test]
    fn backward_address_edges_become_forward_component_edges() {
        let region = graph(&[(5, &[2]), (2, &[8]), (8, &[99])]);
        let plan = Plan::new(&region);
        verify(&region, &plan);
        assert_eq!(
            plan.groups.iter().map(|g| g.blocks[0]).collect::<Vec<_>>(),
            [5, 2, 8]
        );
        assert!(plan.groups.iter().all(|g| !g.cyclic));
    }

    #[test]
    fn multiple_entry_cycles_share_one_body_and_keep_all_ports() {
        let region = graph(&[(1, &[3, 2]), (2, &[4]), (3, &[2, 5]), (4, &[3]), (5, &[])]);
        let plan = Plan::new(&region);
        verify(&region, &plan);
        assert_eq!(plan.groups[1].blocks, [2, 3, 4]);
        assert!(plan.groups[1].cyclic);
        assert_eq!(plan.targets[&5].group, 2);
    }

    #[test]
    fn disjoint_cycles_and_single_block_loops_have_separate_scopes() {
        let region = graph(&[
            (1, &[1, 7]),
            (2, &[3]),
            (3, &[2, 7]),
            (7, &[]),
            (9, &[11]),
            (11, &[9]),
        ]);
        let plan = Plan::new(&region);
        verify(&region, &plan);
        assert_eq!(plan.groups.iter().filter(|g| g.cyclic).count(), 3);
        assert_ne!(plan.targets[&1].group, plan.targets[&2].group);
        assert_ne!(plan.targets[&2].group, plan.targets[&9].group);
    }

    #[test]
    fn maximum_cycle_is_iterative_and_keeps_every_arbitrary_entry() {
        let mut region = graph(&[(0, &[])]);
        for pc in 0..8192 {
            region.blocks.insert(
                pc,
                Block {
                    start: pc,
                    instructions: Vec::new(),
                    successors: vec![(pc + 1) % 8192],
                },
            );
        }
        let plan = Plan::new(&region);
        verify(&region, &plan);
        assert_eq!(plan.groups.len(), 1);
        assert!(plan.groups[0].cyclic);
    }
}

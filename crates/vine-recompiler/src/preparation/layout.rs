//! Partition a bounded discovery batch before assigning native code owners.
//! Bodies occur once. Calls, their continuations and guarded returns guide
//! placement without enlarging the generated unit's instruction allowance.
use crate::control_ir::Graph;
use crate::ir::{Block, Opcode, Operand, Region};
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn partition(graph: Graph, limit: usize) -> Vec<Graph> {
    assert!(limit > 0);
    let mut region = Region {
        entry: graph.region.entry,
        blocks: BTreeMap::new(),
        limit_reached: graph.region.limit_reached,
    };
    // A long straight-line body can cross an ownership boundary. Its suffix is
    // a real raw entry, and the prefix retains an explicit successor to it.
    for block in graph.region.blocks.into_values() {
        let chunks: Vec<_> = block.instructions.chunks(limit).collect();
        for (index, chunk) in chunks.iter().enumerate() {
            let start = chunk[0].eip;
            region.blocks.insert(
                start,
                Block {
                    start,
                    instructions: chunk.to_vec(),
                    successors: chunks
                        .get(index + 1)
                        .map_or_else(|| block.successors.clone(), |next| vec![next[0].eip]),
                },
            );
        }
    }
    let continuations = crate::return_ir::Plan {
        returns: graph.return_edges,
    };
    let mut unseen: BTreeSet<_> = region.blocks.keys().copied().collect();
    let mut pending = vec![region.entry];
    let mut order = Vec::new();
    while !unseen.is_empty() {
        let pc = pending
            .pop()
            .unwrap_or_else(|| *unseen.first().expect("remaining block"));
        if !unseen.remove(&pc) {
            continue;
        }
        let block = &region.blocks[&pc];
        let last = block.instructions.last().expect("decoded nonempty block");
        let targets = match (last.opcode, last.operands.first()) {
            (Opcode::Call, Some(Operand::Imm(target))) => vec![*target, last.next_eip()],
            (Opcode::Ret, _) => continuations
                .returns
                .get(&last.eip)
                .cloned()
                .unwrap_or_default(),
            _ => block.successors.clone(),
        };
        pending.extend(
            targets
                .into_iter()
                .rev()
                .filter(|target| unseen.contains(target)),
        );
        order.push(pc);
    }
    let mut units = Vec::<Region>::new();
    let mut count = 0;
    for pc in order {
        let block = region.blocks.remove(&pc).expect("unique assigned block");
        if units.is_empty() || count + block.instructions.len() > limit {
            units.push(Region {
                entry: pc,
                blocks: BTreeMap::new(),
                limit_reached: region.limit_reached,
            });
            count = 0;
        }
        count += block.instructions.len();
        units
            .last_mut()
            .expect("current unit")
            .blocks
            .insert(pc, block);
    }
    units
        .into_iter()
        .map(|region| {
            let instruction_pcs: BTreeSet<_> = region
                .blocks
                .values()
                .flat_map(|b| b.instructions.iter().map(|i| i.eip))
                .collect();
            let mut dispatches = graph.dispatches.clone();
            dispatches.retain(|pc, dispatch| {
                if !instruction_pcs.contains(pc) {
                    return false;
                }
                for target in &mut dispatch.targets {
                    if target.is_some_and(|pc| !region.blocks.contains_key(&pc)) {
                        *target = None;
                    }
                }
                dispatch.targets.iter().any(Option::is_some)
            });
            let return_edges = continuations
                .returns
                .iter()
                .filter_map(|(&pc, targets)| {
                    if !instruction_pcs.contains(&pc) {
                        return None;
                    }
                    let targets: Vec<_> = targets
                        .iter()
                        .copied()
                        .filter(|pc| region.blocks.contains_key(pc))
                        .collect();
                    (!targets.is_empty()).then_some((pc, targets))
                })
                .collect();
            Graph {
                region,
                dispatches,
                return_edges,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::{EXECUTE, MirrorMemory, READ, SEALED};

    #[test]
    fn partition_preserves_unique_bodies_bounds_and_cross_owner_return_hints() {
        let mut memory = MirrorMemory::new(2).unwrap();
        let mut code = [0xcc; 4096];
        code[..7].copy_from_slice(&[0xe8, 11, 0, 0, 0, 0x0f, 0x0b]);
        code[16..26].copy_from_slice(&[0xe8, 11, 0, 0, 0, 0xe9, 22, 0, 0, 0]);
        code[32] = 0xc3;
        code[48] = 0xc3;
        memory
            .map_page(0, 1, READ | EXECUTE | SEALED, &code)
            .unwrap();
        let graph = Graph::new(crate::decode::discover(&memory, 0, 64), BTreeMap::new());
        let expected: BTreeSet<_> = graph
            .region
            .blocks
            .values()
            .flat_map(|b| b.instructions.iter().map(|i| i.eip))
            .collect();
        let mut actual = BTreeSet::new();
        let mut retained_cross_owner_hint = false;
        for unit in partition(graph, 3) {
            unit.verify().unwrap();
            assert!(
                unit.region
                    .blocks
                    .values()
                    .map(|b| b.instructions.len())
                    .sum::<usize>()
                    <= 3
            );
            for pc in unit
                .region
                .blocks
                .values()
                .flat_map(|b| b.instructions.iter().map(|i| i.eip))
            {
                assert!(actual.insert(pc), "a body must have a single owner");
            }
            if unit.return_edges.get(&48) == Some(&vec![5]) {
                assert!(!unit.region.blocks.contains_key(&0));
                retained_cross_owner_hint = true;
            }
        }
        assert_eq!(actual, expected);
        assert!(
            retained_cross_owner_hint,
            "return placement must not forget a call in another unit"
        );
    }

    #[test]
    fn a_long_body_splits_at_exact_instruction_entries_without_losing_the_tail() {
        let mut memory = MirrorMemory::new(2).unwrap();
        let mut code = [0x90; 4096];
        code[10..12].copy_from_slice(&[0x0f, 0x0b]);
        memory
            .map_page(0, 1, READ | EXECUTE | SEALED, &code)
            .unwrap();
        let graph = Graph::new(crate::decode::discover(&memory, 0, 64), BTreeMap::new());
        let units = partition(graph, 3);
        let mut pcs = Vec::new();
        for (index, unit) in units.iter().enumerate() {
            unit.verify().unwrap();
            let block = &unit.region.blocks[&unit.region.entry];
            pcs.extend(block.instructions.iter().map(|i| i.eip));
            if let Some(next) = units.get(index + 1) {
                assert_eq!(block.successors, [next.region.entry]);
            }
        }
        assert_eq!(pcs, (0..=10).collect::<Vec<_>>());
        assert_eq!(
            units.last().unwrap().region.blocks[&9]
                .instructions
                .last()
                .unwrap()
                .opcode,
            Opcode::Ud2
        );
    }
}

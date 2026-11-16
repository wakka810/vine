//! Flag demand across ordinary block edges, guarded by a finite guest budget.
//!
//! A distance proves that every path overwrites a flag before reading it or
//! reaching an observable exit. Faulting instructions are barriers even when
//! they would write that flag on success. Cycles without a guaranteed write
//! never acquire a finite proof. No value, memory check or effect is hoisted.
use crate::ir::{Block, Region};
use crate::optimize::{can_fault, flags_needed_with_live, unconditional_flag_writes};
use std::collections::{BTreeMap, VecDeque};

const UNKNOWN: u32 = u32::MAX;
type Distances = [u32; 32];

#[derive(Clone, Debug)]
pub(crate) struct Admission {
    pub budget: u32,
    pub needed: Vec<u32>,
}

fn before(block: &Block, mut after: Distances, limit: u32) -> Distances {
    for instruction in block.instructions.iter().rev() {
        if can_fault(instruction) {
            after = [UNKNOWN; 32];
            continue;
        }
        let writes = unconditional_flag_writes(instruction);
        for (bit, distance) in after.iter_mut().enumerate() {
            let flag = 1 << bit;
            *distance = if instruction.flags_read & flag != 0 {
                UNKNOWN
            } else if writes & flag != 0 {
                1
            } else {
                distance
                    .checked_add(1)
                    .filter(|&n| n <= limit)
                    .unwrap_or(UNKNOWN)
            };
        }
    }
    after
}

fn join(successors: &[Option<usize>], entries: &[Distances]) -> Distances {
    if successors.is_empty() {
        return [UNKNOWN; 32];
    }
    std::array::from_fn(|bit| {
        successors
            .iter()
            .map(|target| target.map_or(UNKNOWN, |i| entries[i][bit]))
            .max()
            .unwrap()
    })
}

pub(crate) fn discover(region: &Region) -> BTreeMap<u32, Admission> {
    let blocks: Vec<_> = region.blocks.values().collect();
    let indices: BTreeMap<_, _> = blocks
        .iter()
        .enumerate()
        .map(|(i, b)| (b.start, i))
        .collect();
    let limit = blocks
        .iter()
        .fold(0u32, |n, b| n.saturating_add(b.instructions.len() as u32));
    let valid: Vec<_> = blocks
        .iter()
        .map(|b| {
            !b.instructions.is_empty()
                && b.instructions[..b.instructions.len() - 1]
                    .iter()
                    .all(|i| !i.ends_block())
        })
        .collect();
    // Use actual instruction edges, not optional discovery hints (CALL return
    // continuations, jump-table candidates, or entries supplied by the loader).
    let successors: Vec<Vec<_>> = blocks
        .iter()
        .map(|b| {
            b.instructions.last().map_or_else(Vec::new, |i| {
                i.successors()
                    .iter()
                    .map(|pc| indices.get(pc).copied())
                    .collect()
            })
        })
        .collect();
    let mut predecessors = vec![Vec::new(); blocks.len()];
    for (source, targets) in successors.iter().enumerate() {
        for &target in targets.iter().flatten() {
            predecessors[target].push(source);
        }
    }
    let mut entries = vec![[UNKNOWN; 32]; blocks.len()];
    let mut queued = vec![true; blocks.len()];
    let mut pending: VecDeque<_> = (0..blocks.len()).collect();
    while let Some(index) = pending.pop_front() {
        queued[index] = false;
        if !valid[index] {
            continue;
        }
        let next = before(blocks[index], join(&successors[index], &entries), limit);
        if next == entries[index] {
            continue;
        }
        debug_assert!(next.iter().zip(entries[index]).all(|(&a, b)| a <= b));
        entries[index] = next;
        for &source in &predecessors[index] {
            if !queued[source] {
                queued[source] = true;
                pending.push_back(source);
            }
        }
    }
    let mut plans = BTreeMap::new();
    for (index, block) in blocks.into_iter().enumerate() {
        if !valid[index] {
            continue;
        }
        let distances = join(&successors[index], &entries);
        let dead = distances.iter().enumerate().fold(0, |mask, (bit, &n)| {
            mask | if n != UNKNOWN { 1 << bit } else { 0 }
        });
        let needed = flags_needed_with_live(block, !dead);
        let previous = flags_needed_with_live(block, u32::MAX);
        let removed = previous
            .iter()
            .zip(&needed)
            .fold(0, |mask, (&a, &b)| mask | (a & !b));
        if removed == 0 {
            continue;
        }
        let reserve = distances
            .iter()
            .enumerate()
            .filter(|(bit, _)| removed & (1 << bit) != 0)
            .map(|(_, &n)| n)
            .max()
            .expect("removed flags have a successor proof");
        let Some(budget) = (block.instructions.len() as u32).checked_add(reserve) else {
            continue;
        };
        plans.insert(block.start, Admission { budget, needed });
    }
    plans
}

#[cfg(test)]
mod tests {
    use super::*;
    fn region(parts: &[(u32, &[u8])]) -> Region {
        let blocks = parts
            .iter()
            .map(|&(start, bytes)| {
                let mut instructions = Vec::new();
                let mut offset = 0;
                while offset < bytes.len() {
                    let i = crate::decode::decode_one(start + offset as u32, &bytes[offset..]);
                    assert_ne!(i.len, 0);
                    offset += usize::from(i.len);
                    instructions.push(i);
                }
                let successors = instructions.last().unwrap().successors();
                (
                    start,
                    Block {
                        start,
                        instructions,
                        successors,
                    },
                )
            })
            .collect();
        Region {
            entry: parts[0].0,
            blocks,
            limit_reached: false,
        }
    }

    #[test]
    fn loop_requires_budget_through_the_next_definition() {
        let r = region(&[(0x1000, &[0x83, 0xc0, 1, 0xeb, 0xfb])]);
        let p = discover(&r);
        assert_eq!(p[&0x1000].budget, 3);
        assert_eq!(p[&0x1000].needed, [0, 0]);
    }

    #[test]
    fn conditional_join_keeps_consumed_flags_and_the_longest_path() {
        let r = region(&[
            (0x1000, &[0x39, 0xd8, 0x74, 4]),
            (0x1004, &[0x90, 0x83, 0xc1, 1]),
            (0x1008, &[0x83, 0xc2, 1]),
        ]);
        let p = discover(&r);
        assert_eq!(p[&0x1000].budget, 4);
        assert_eq!(p[&0x1000].needed, [64, 0]);
    }

    #[test]
    fn fault_unknown_edge_and_write_free_cycle_prevent_the_proof() {
        for target in [
            &[0x8b, 0x06, 0x83, 0xc1, 1][..],
            &[0xeb, 0xfe],
            &[0xeb, 0x10],
        ] {
            let r = region(&[(0x1000, &[0x83, 0xc0, 1, 0xeb, 0]), (0x1005, target)]);
            assert!(!discover(&r).contains_key(&0x1000));
        }
        let r = region(&[
            (0x1000, &[0x83, 0xc0, 1, 0x74, 4]),
            (0x1005, &[0x83, 0xc1, 1]),
        ]);
        assert!(!discover(&r).contains_key(&0x1000));
    }

    #[test]
    fn carry_inputs_partial_writes_and_zero_shifts_remain_observable() {
        for (target, expected) in [(&[0x83, 0xd1, 1][..], 1), (&[0x41][..], 1)] {
            let r = region(&[(0x1000, &[0x83, 0xc0, 1, 0xeb, 0]), (0x1005, target)]);
            assert_eq!(discover(&r)[&0x1000].needed, [expected, 0]);
        }
        let r = region(&[(0x1000, &[0x83, 0xc0, 1, 0xeb, 0]), (0x1005, &[0xd3, 0xe1])]);
        assert!(!discover(&r).contains_key(&0x1000));
    }

    #[test]
    fn faulting_producer_keeps_flags_for_post_store_invalidation() {
        let r = region(&[
            (0x1000, &[0x83, 0x06, 1, 0xeb, 0]),
            (0x1005, &[0x83, 0xc1, 1]),
        ]);
        assert!(!discover(&r).contains_key(&0x1000));
    }
}

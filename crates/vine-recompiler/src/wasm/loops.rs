//! Bounded structured copies of direct loops. Ordinary block entries remain
//! available for indirect branches and resumes into a loop's interior.
//!
//! The accepted body is a DAG after edges to its header are removed. This
//! deliberately excludes inner cycles and irreducible layouts; their original
//! dispatcher lowering remains available. No profile values or fixed PCs are
//! consulted, and the cloned instruction budget is at most one region's size.

use super::*;
use std::collections::{BTreeSet, VecDeque};

#[derive(Debug, PartialEq, Eq)]
pub(super) struct StructuredLoop {
    pub header: u32,
    pub body: Vec<u32>,
}

pub(super) fn find(region: &Region) -> BTreeMap<u32, StructuredLoop> {
    let mut latches = BTreeMap::<u32, u32>::new();
    for (&pc, block) in &region.blocks {
        let Some(last) = block.instructions.last() else {
            continue;
        };
        if !matches!(last.opcode, Opcode::Jcc | Opcode::Jmp) {
            continue;
        }
        if let Some(Operand::Imm(header)) = last.operands.first()
            && *header <= pc
            && region.blocks.contains_key(header)
        {
            latches
                .entry(*header)
                .and_modify(|end| *end = (*end).max(pc))
                .or_insert(pc);
        }
    }
    let mut candidates = Vec::new();
    for (header, end) in latches {
        let mut body = BTreeSet::new();
        let mut pending = VecDeque::from([header]);
        let mut instructions = 0;
        let mut valid = true;
        while let Some(pc) = pending.pop_front() {
            if !body.insert(pc) {
                continue;
            }
            let block = &region.blocks[&pc];
            instructions += block.instructions.len();
            if instructions > 256 {
                valid = false;
                break;
            }
            for &next in &block.successors {
                if next == header {
                    continue;
                }
                if next >= header && next <= end && region.blocks.contains_key(&next) {
                    // Only the chosen header can be a backward destination.
                    // Address order is then a valid topological body order.
                    if next <= pc {
                        valid = false;
                        break;
                    }
                    pending.push_back(next);
                }
            }
            if !valid {
                break;
            }
        }
        if valid && body.contains(&end) {
            let clone_cost = instructions - region.blocks[&header].instructions.len();
            candidates.push((
                clone_cost,
                StructuredLoop {
                    header,
                    body: body.into_iter().collect(),
                },
            ));
        }
    }
    candidates.sort_by_key(|(cost, candidate)| (*cost, candidate.header));
    let mut budget: usize = region.blocks.values().map(|b| b.instructions.len()).sum();
    let mut used = BTreeSet::new();
    let mut selected = BTreeMap::new();
    for (cost, candidate) in candidates {
        if cost > budget || candidate.body.iter().any(|pc| used.contains(pc)) {
            continue;
        }
        budget -= cost;
        used.extend(candidate.body.iter().copied());
        selected.insert(candidate.header, candidate);
    }
    selected
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Code(Vec<u8>);
    impl crate::decode::CodeSource for Code {
        fn read_code(&self, address: u32, max_len: usize) -> Vec<u8> {
            let start = address.wrapping_sub(0x1000) as usize;
            self.0
                .get(start..)
                .unwrap_or_default()
                .iter()
                .copied()
                .take(max_len)
                .collect()
        }
    }

    #[test]
    fn preserves_header_and_interior_entries_for_conditional_loop() {
        // test eax,eax; jz join; inc ebx; join: dec ecx; jnz header; ud2
        let region = crate::decode::discover_many(
            &Code(vec![
                0x85, 0xc0, 0x74, 1, 0x43, 0x49, 0x75, 0xf8, 0x0f, 0x0b,
            ]),
            &[0x1005, 0x1000],
            32,
        );
        let loops = find(&region);
        assert_eq!(loops[&0x1000].body, vec![0x1000, 0x1004, 0x1005]);
        assert!(region.blocks.contains_key(&0x1005));
    }

    #[test]
    fn rejects_outer_body_containing_another_cycle() {
        // inc eax; inner: dec ecx; jnz inner; dec edx; jnz outer; ud2
        let region = crate::decode::discover(
            &Code(vec![0x40, 0x49, 0x75, 0xfd, 0x4a, 0x75, 0xf9, 0x0f, 0x0b]),
            0x1000,
            32,
        );
        let loops = find(&region);
        assert_eq!(loops.len(), 1);
        assert_eq!(loops[&0x1001].body, vec![0x1001]);
    }
}

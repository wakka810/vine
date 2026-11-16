//! Normalize overlapping decoded blocks before SSA construction. Straight
//! instruction sequences share one value graph; real ingress/control targets
//! remain boundaries. Ordered instruction snapshots remain inside each block.
use crate::ir::{Block, Instruction, Region};
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn normalize(region: &Region) -> Result<Region, String> {
    let mut instructions = BTreeMap::<u32, Instruction>::new();
    let mut leaders: BTreeSet<_> = region.blocks.keys().copied().collect();
    leaders.insert(region.entry);
    for block in region.blocks.values() {
        for instruction in &block.instructions {
            if let Some(previous) = instructions.insert(instruction.eip, instruction.clone())
                && previous != *instruction
            {
                return Err("conflicting decoded instruction at a continuation boundary".into());
            }
            if instruction.ends_block() {
                leaders.extend(instruction.successors());
            }
        }
    }
    let mut blocks = BTreeMap::new();
    // Visit explicit leaders first. A fallback leader can only be a disconnected
    // decoded fragment; it must not consume a real ingress or branch target.
    let order: Vec<_> = leaders.iter().chain(instructions.keys()).copied().collect();
    for start in order {
        let Some(first) = instructions.remove(&start) else {
            continue;
        };
        let mut body = vec![first];
        loop {
            let tail = body.last().unwrap();
            let next = tail.next_eip();
            if tail.ends_block() || leaders.contains(&next) {
                break;
            }
            let Some(instruction) = instructions.remove(&next) else {
                break;
            };
            body.push(instruction);
        }
        let successors = body.last().unwrap().successors();
        blocks.insert(
            start,
            Block {
                start,
                instructions: body,
                successors,
            },
        );
    }
    Ok(Region {
        entry: region.entry,
        blocks,
        limit_reached: region.limit_reached,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode::{CodeSource, discover};

    struct Code(Vec<u8>);
    impl CodeSource for Code {
        fn read_code(&self, address: u32, max: usize) -> Vec<u8> {
            let Some(offset) = address.checked_sub(0x1000) else {
                return vec![];
            };
            self.0
                .iter()
                .skip(offset as usize)
                .take(max)
                .copied()
                .collect()
        }
    }

    #[test]
    fn overlapping_raw_entry_and_interior_branch_keep_their_exact_instruction_suffixes() {
        let code = Code(vec![0x40, 0x43, 0x42, 0x49, 0x75, 0xfb, 0x0f, 0x0b]);
        let mut region = discover(&code, 0x1000, 64);
        // Independent discovery can retain a suffix as another raw entry.
        region
            .blocks
            .insert(0x1002, discover(&code, 0x1002, 64).blocks[&0x1002].clone());
        let normalized = normalize(&region).unwrap();
        for pc in region.blocks.keys() {
            assert!(normalized.blocks.contains_key(pc), "lost raw entry {pc:x}");
        }
        let mut seen = BTreeSet::new();
        for block in normalized.blocks.values() {
            for instruction in &block.instructions {
                assert!(seen.insert(instruction.eip), "duplicate instruction body");
            }
        }
        assert_eq!(normalized.blocks[&0x1000].successors, [0x1001]);
        assert_eq!(normalized.blocks[&0x1001].successors, [0x1002]);
        assert_eq!(normalized.blocks[&0x1002].instructions.len(), 3);
        assert_eq!(normalized.blocks[&0x1002].successors, [0x1006, 0x1001]);
    }

    #[test]
    fn conflicting_decode_is_not_silently_replaced() {
        let mut region = discover(&Code(vec![0x40, 0x43, 0x0f, 0x0b]), 0x1000, 64);
        let mut conflicting = region.blocks[&0x1000].clone();
        conflicting.start = 0x1001;
        conflicting.instructions[0].len = 2;
        region.blocks.insert(conflicting.start, conflicting);
        assert!(normalize(&region).is_err());
    }
}

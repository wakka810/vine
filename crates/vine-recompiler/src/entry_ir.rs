//! Immutable bucket selection for already admitted guest instruction entries.
//! Hashing only narrows the candidate set; generated code must still compare
//! the complete u32 PC before publishing a route. No guest memory is a table.

#[derive(Debug)]
pub(crate) struct Plan {
    pub multiplier: u32,
    pub shift: u32,
    pub slots: Vec<Option<usize>>,
    pub buckets: Vec<Vec<(u32, usize)>>,
}

impl Plan {
    pub fn new(entries: &[(u32, usize)]) -> Self {
        assert!((8..=8192).contains(&entries.len()));
        let size = (entries.len() * 2).next_power_of_two();
        let shift = 32 - size.trailing_zeros();
        // Bounded compile-time selection, independent of execution history.
        let multipliers = [
            0x9e37_79b1u32,
            0x85eb_ca6b,
            0xc2b2_ae35,
            0x27d4_eb2d,
            0x1656_67b1,
            0xd3a2_646d,
            0xfd70_46c5,
            0xb55a_4f09,
        ];
        let mut counts = vec![0usize; size];
        let mut best = (usize::MAX, usize::MAX);
        let mut multiplier = multipliers[0];
        for candidate in multipliers {
            counts.fill(0);
            for &(pc, _) in entries {
                counts[(pc.wrapping_mul(candidate) >> shift) as usize] += 1;
            }
            let score = (
                *counts.iter().max().unwrap(),
                counts.iter().map(|count| count * count).sum(),
            );
            if score < best {
                best = score;
                multiplier = candidate;
            }
        }
        let mut slots = vec![None; size];
        let mut buckets = Vec::<Vec<(u32, usize)>>::new();
        for &(pc, route) in entries {
            let slot = (pc.wrapping_mul(multiplier) >> shift) as usize;
            let bucket = *slots[slot].get_or_insert_with(|| {
                buckets.push(Vec::new());
                buckets.len() - 1
            });
            buckets[bucket].push((pc, route));
        }
        Self {
            multiplier,
            shift,
            slots,
            buckets,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    fn resolve(plan: &Plan, pc: u32) -> Option<usize> {
        let bucket = plan.slots[(pc.wrapping_mul(plan.multiplier) >> plan.shift) as usize]?;
        plan.buckets[bucket]
            .iter()
            .find_map(|&(key, route)| (key == pc).then_some(route))
    }

    #[test]
    fn all_ports_and_unknown_collisions_keep_exact_full_width_identity() {
        let entries: Vec<_> = (0..137)
            .map(|i| (0x8000_1000 + i as u32 * 7, 136 - i))
            .collect();
        let plan = Plan::new(&entries);
        let expected: BTreeMap<_, _> = entries.into_iter().collect();
        for pc in 0x8000_0f00..0x8000_2000 {
            assert_eq!(resolve(&plan, pc), expected.get(&pc).copied());
        }
        for pc in [0, 0x1000, 0xffff_ffff, 0x7fff_ffff] {
            assert_eq!(resolve(&plan, pc), expected.get(&pc).copied());
        }
    }

    #[test]
    fn maximum_layout_has_bounded_storage_and_no_lost_entries() {
        let entries: Vec<_> = (0..8192)
            .map(|i| ((i as u32).wrapping_mul(0x01ff_0001), i))
            .collect();
        let plan = Plan::new(&entries);
        assert_eq!(plan.slots.len(), 16384);
        assert!(plan.buckets.len() <= entries.len());
        for (pc, route) in entries {
            assert_eq!(resolve(&plan, pc), Some(route));
        }
    }
}

//! Guarded return continuations derived from decoded calls and their bodies.
//! These edges describe useful layouts, never a replacement guest call stack.
use crate::ir::{Opcode, Operand, Region};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Default)]
pub struct Plan {
    /// RET instruction PC -> known in-region return PCs. Execution must compare
    /// the actual checked stack load before using any of these destinations.
    pub returns: BTreeMap<u32, Vec<u32>>,
}

impl Plan {
    pub fn new(region: &Region) -> Self {
        let mut returns = BTreeMap::<u32, BTreeSet<u32>>::new();
        for call in region.blocks.values().flat_map(|block| &block.instructions) {
            if call.opcode != Opcode::Call || !region.blocks.contains_key(&call.next_eip()) {
                continue;
            }
            let Some(Operand::Imm(callee)) = call.operands.first() else {
                continue;
            };
            let mut pending = vec![*callee];
            let mut seen = BTreeSet::new();
            while let Some(pc) = pending.pop() {
                if !seen.insert(pc) {
                    continue;
                }
                let Some(block) = region.blocks.get(&pc) else {
                    continue;
                };
                let Some(last) = block.instructions.last() else {
                    continue;
                };
                match last.opcode {
                    Opcode::Ret => {
                        returns.entry(last.eip).or_default().insert(call.next_eip());
                    }
                    // A nested call's return address belongs to that call. The
                    // outer continuation resumes at its next instruction, if
                    // it returns at all. Unknown callees grant no extra edge.
                    Opcode::Call => pending.push(last.next_eip()),
                    _ => pending.extend(block.successors.iter().copied()),
                }
            }
        }
        Self {
            returns: returns
                .into_iter()
                .map(|(pc, targets)| (pc, targets.into_iter().collect()))
                .collect(),
        }
    }

    pub fn guarded(&self, instruction: u32) -> &[u32] {
        // Bound emitted comparisons for polymorphic helpers. A larger set
        // remains useful for layout but retains ordinary exact-PC dispatch.
        self.returns
            .get(&instruction)
            .filter(|targets| targets.len() <= 4)
            .map_or(&[], Vec::as_slice)
    }

    pub fn layout_region(&self, region: &Region) -> Region {
        let mut result = region.clone();
        for block in result.blocks.values_mut() {
            if let Some(last) = block.instructions.last() {
                for &target in self.guarded(last.eip) {
                    if !block.successors.contains(&target) {
                        block.successors.push(target);
                    }
                }
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::{EXECUTE, MirrorMemory, READ, SEALED};

    #[test]
    fn nested_calls_and_tail_transfers_keep_distinct_return_values() {
        let mut memory = MirrorMemory::new(2).unwrap();
        let mut code = [0xcc; 4096];
        code[..7].copy_from_slice(&[0xe8, 11, 0, 0, 0, 0x0f, 0x0b]);
        code[16..26].copy_from_slice(&[0xe8, 11, 0, 0, 0, 0xe9, 22, 0, 0, 0]);
        code[32] = 0xc3;
        code[48] = 0xc3;
        memory
            .map_page(0, 1, READ | EXECUTE | SEALED, &code)
            .unwrap();
        let graph =
            crate::control_ir::Graph::new(crate::decode::discover(&memory, 0, 64), BTreeMap::new());
        assert_eq!(graph.return_edges.get(&32).unwrap(), &[21]);
        assert_eq!(graph.return_edges.get(&48).unwrap(), &[5]);
        graph.verify().unwrap();
        let mut bad = graph.clone();
        bad.return_edges.insert(0, vec![5]);
        assert!(
            bad.verify().is_err(),
            "a call cannot masquerade as a RET guard"
        );
        bad = graph;
        bad.return_edges.insert(32, vec![22]);
        assert!(
            bad.verify().is_err(),
            "an instruction interior is not an admitted target"
        );
    }
}

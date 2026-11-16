//! Binary-derived control edges shared by ahead-of-time and demand conversion.
//!
//! Table contents and range observations are discovery hints, never assumptions
//! about an arbitrary architectural entry. The guest load still executes with
//! its original fault ordering. Lowering must guard both the actual index and
//! loaded target; a changed table can always take the ordinary indirect edge.
use crate::decode::{self, CodeSource};
use crate::ir::{Condition, Instruction, MemoryOperand, Opcode, Operand, Region, Segment, Width};
use std::collections::{BTreeMap, BTreeSet};

const MAXIMUM_CASES: u32 = 256;

#[cfg(test)]
mod tests;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TableDispatch {
    pub operand: MemoryOperand,
    /// Missing/unreadable/non-executable hints remain dynamic edges.
    pub targets: Vec<Option<u32>>,
}

#[derive(Clone, Debug)]
pub struct Graph {
    pub region: Region,
    pub dispatches: BTreeMap<u32, TableDispatch>,
    /// Guarded layout hints; the actual RET stack load remains authoritative.
    pub return_edges: BTreeMap<u32, Vec<u32>>,
}

impl Graph {
    pub fn new(region: Region, dispatches: BTreeMap<u32, TableDispatch>) -> Self {
        let return_edges = crate::return_ir::Plan::new(&region).returns;
        Self {
            region,
            dispatches,
            return_edges,
        }
    }
    pub fn verify(&self) -> Result<(), String> {
        for (&pc, targets) in &self.return_edges {
            let instruction = self
                .region
                .blocks
                .values()
                .flat_map(|b| &b.instructions)
                .find(|i| i.eip == pc);
            if instruction.is_none_or(|i| i.opcode != Opcode::Ret)
                || targets.is_empty()
                || targets.windows(2).any(|pair| pair[0] >= pair[1])
                || targets
                    .iter()
                    .any(|pc| !self.region.blocks.contains_key(pc))
            {
                return Err("invalid guarded return edge".into());
            }
        }
        for (&pc, dispatch) in &self.dispatches {
            let instruction = self
                .region
                .blocks
                .values()
                .flat_map(|b| &b.instructions)
                .find(|i| i.eip == pc)
                .ok_or("control edge has no instruction")?;
            let operand = dispatch.operand;
            if instruction.opcode != Opcode::Jmp
                || instruction.operands != [Operand::Mem(operand)]
                || operand.base.is_some()
                || !operand.index.is_some_and(|r| r < 8)
                || operand.scale != 4
                || operand.segment != Segment::Flat
                || operand.width != Width::W32
                || dispatch.targets.is_empty()
                || dispatch.targets.len() > MAXIMUM_CASES as usize
                || dispatch
                    .targets
                    .iter()
                    .flatten()
                    .any(|pc| !self.region.blocks.contains_key(pc))
            {
                return Err("invalid guarded table edge".into());
            }
        }
        Ok(())
    }
}

pub fn discover(source: &impl CodeSource, entries: &[u32], limit: usize) -> Graph {
    let mut dispatches = BTreeMap::new();
    let region = decode::discover_with_edges(source, entries, limit, |i, decoded| {
        let Some(dispatch) = table_dispatch(source, i, decoded) else {
            return Vec::new();
        };
        let targets = dispatch
            .targets
            .iter()
            .flatten()
            .copied()
            .collect::<BTreeSet<_>>();
        dispatches.insert(i.eip, dispatch);
        targets.into_iter().collect()
    });
    // Discovery may hit its bound halfway through a table. Only resident code
    // is lowered to static branches; omitted targets keep the dynamic edge.
    for dispatch in dispatches.values_mut() {
        for target in &mut dispatch.targets {
            if target.is_some_and(|pc| !region.blocks.contains_key(&pc)) {
                *target = None;
            }
        }
    }
    dispatches.retain(|_, dispatch| dispatch.targets.iter().any(Option::is_some));
    Graph::new(region, dispatches)
}

fn table_dispatch(
    source: &impl CodeSource,
    instruction: &Instruction,
    decoded: &BTreeMap<u32, Instruction>,
) -> Option<TableDispatch> {
    if instruction.opcode != Opcode::Jmp {
        return None;
    }
    let [Operand::Mem(operand)] = instruction.operands.as_slice() else {
        return None;
    };
    let index = operand.index?;
    if operand.base.is_some()
        || operand.scale != 4
        || operand.width != Width::W32
        || operand.segment != Segment::Flat
    {
        return None;
    }
    // This local code pattern supplies a bounded discovery extent, not a
    // dominance proof: other edges or external entries can bypass it.
    let count = decoded
        .values()
        .filter_map(|branch| {
            if branch.opcode != Opcode::Jcc {
                return None;
            }
            let target = match branch.operands.first()? {
                Operand::Imm(pc) => *pc,
                _ => return None,
            };
            let inclusive = match branch.condition? {
                Condition::A if branch.next_eip() == instruction.eip => true,
                Condition::Ae if branch.next_eip() == instruction.eip => false,
                Condition::Be if target == instruction.eip => true,
                Condition::B if target == instruction.eip => false,
                _ => return None,
            };
            let compare = decoded
                .values()
                .find(|i| i.next_eip() == branch.eip && i.opcode == Opcode::Cmp)?;
            let [Operand::Reg(reg), Operand::Imm(bound)] = compare.operands.as_slice() else {
                return None;
            };
            if reg.index != index || reg.width != Width::W32 || reg.high8 {
                return None;
            }
            let count = bound.checked_add(u32::from(inclusive))?;
            (count > 0 && count <= MAXIMUM_CASES).then_some(count)
        })
        .max()?;
    let mut targets = Vec::with_capacity(count as usize);
    for index in 0..count {
        let target = operand
            .displacement
            .checked_add(index * 4)
            .and_then(|at| source.read_data(at, 4).try_into().ok())
            .map(u32::from_le_bytes)
            .filter(|&pc| !source.read_code(pc, 1).is_empty());
        targets.push(target);
    }
    targets
        .iter()
        .any(Option::is_some)
        .then_some(TableDispatch {
            operand: *operand,
            targets,
        })
}

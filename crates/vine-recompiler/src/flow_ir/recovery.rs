//! Availability at an ordered effect is stricter than block dominance. A
//! recovery recipe must never consume the result of the faulting instruction.
use super::*;

pub(super) fn verify(g: &Graph, source: &v::Graph) -> Result<(), v::Error> {
    let bad = || v::Error("continuation uses an incomplete effect result".into());
    let mut positions = vec![0; source.effects.len()];
    for block in &source.blocks {
        for (position, &effect) in block.steps.iter().flat_map(|s| &s.effects).enumerate() {
            positions[effect.index()] = position + 1;
        }
    }
    // Pure definitions have already been checked to be a DAG. Phi inputs
    // belong to completed predecessor executions and deliberately stop here.
    let mut latest = vec![0; g.values.len()];
    for (index, value) in g.values.iter().enumerate() {
        if let Op::EffectResult { effect, .. } = value.op {
            latest[index] = positions[effect.index()];
        }
    }
    for (index, value) in g.values.iter().enumerate() {
        for input in operands(&value.op) {
            let input = g.resolve(input);
            if g.values[input.0].home == value.home {
                latest[index] = latest[index].max(latest[input.0]);
            }
        }
    }
    for (index, source_block) in g.node_blocks.iter().enumerate().skip(2) {
        let node = NodeId(index);
        let block = &source.blocks[source_block.unwrap().index()];
        let available = |value: ValueId, completed: usize| {
            let value = g.resolve(value);
            if g.values[value.0].home == node && latest[value.0] > completed {
                Err(bad())
            } else {
                Ok(())
            }
        };
        let scalar = |value, completed| available(g.value_at(node, value), completed);
        let cpu = |cpu: &v::CpuValues, completed| -> Result<(), v::Error> {
            for &value in &cpu.gpr {
                scalar(value, completed)?;
            }
            scalar(cpu.flags.other, completed)?;
            scalar(cpu.flags.defined, completed)?;
            for &flag in &cpu.flags.arithmetic {
                available(g.payload_at(node, flag), completed)?;
                available(g.condition_at(node, flag), completed)?;
            }
            Ok(())
        };
        let snapshot = |id: v::SnapshotId, completed| -> Result<(), v::Error> {
            let state = &source.snapshots[id.index()];
            cpu(&state.cpu, completed)?;
            scalar(state.eip, completed)
        };
        let mut completed = 0;
        for step in &block.steps {
            snapshot(step.before, completed)?;
            for &id in &step.effects {
                let effect = &source.effects[id.index()];
                if let Some(guard) = effect.guard {
                    scalar(guard, completed)?;
                }
                match &effect.kind {
                    v::EffectKind::Load { address, .. } => scalar(address.offset, completed)?,
                    v::EffectKind::Store { address, value, .. } => {
                        scalar(address.offset, completed)?;
                        scalar(*value, completed)?;
                    }
                    v::EffectKind::Opaque { inputs, .. } => cpu(inputs, completed)?,
                }
                completed += 1;
            }
            snapshot(step.after, completed)?;
        }
        match block.terminator {
            v::Terminator::Direct { state, .. } | v::Terminator::Stop { state } => {
                snapshot(state, completed)?
            }
            v::Terminator::Conditional {
                state, condition, ..
            } => {
                snapshot(state, completed)?;
                scalar(condition, completed)?;
            }
            v::Terminator::Indirect { state, target } => {
                snapshot(state, completed)?;
                scalar(target, completed)?;
            }
        }
    }
    Ok(())
}

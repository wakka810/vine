use super::*;

fn require(condition: bool, message: &str) -> Result<(), Error> {
    if condition {
        Ok(())
    } else {
        Err(Error(message.into()))
    }
}

struct Check<'a> {
    graph: &'a Graph,
}
impl Check<'_> {
    fn block(&self, block: BlockId) -> Result<(), Error> {
        require(
            block.index() < self.graph.blocks.len(),
            "invalid block reference",
        )
    }
    fn value(&self, value: ValueId, block: BlockId) -> Result<(), Error> {
        let value = self
            .graph
            .values
            .get(value.index())
            .ok_or_else(|| Error("invalid value reference".into()))?;
        require(
            value.home.is_none() || value.home == Some(block),
            "value crosses a block without a parameter",
        )
    }
    fn flag(&self, flag: FlagRef, block: BlockId, bit: u8) -> Result<(), Error> {
        let flag = self
            .graph
            .flags
            .get(flag.index())
            .ok_or_else(|| Error("invalid flag reference".into()))?;
        require(
            flag.home == block && flag.bit == bit,
            "flag reference changes block or architectural bit",
        )
    }
    fn cpu(&self, cpu: &CpuValues, block: BlockId) -> Result<(), Error> {
        for &value in &cpu.gpr {
            self.value(value, block)?;
        }
        self.value(cpu.flags.other, block)?;
        self.value(cpu.flags.defined, block)?;
        for (bit, &flag) in cpu.flags.arithmetic.iter().enumerate() {
            self.flag(flag, block, bit as u8)?;
        }
        Ok(())
    }
    fn snapshot(&self, id: SnapshotId, block: BlockId) -> Result<&ExitState, Error> {
        let snapshot = self
            .graph
            .snapshots
            .get(id.index())
            .ok_or_else(|| Error("invalid snapshot reference".into()))?;
        require(snapshot.block == block, "snapshot crosses a block")?;
        Ok(snapshot)
    }
    fn target(&self, target: Target) -> Result<(), Error> {
        if let Target::Block(block) = target {
            self.block(block)?;
        }
        Ok(())
    }
}

/// Verify numeric/flag dependency order without recursion. A FlagRef may refer
/// to a numeric value that explicitly evaluates an older flag (ADC/SBB), but
/// a runtime recipe cannot contain another recipe. The bipartite DAG check
/// prevents an optimizer from accidentally making a cyclic history carrier.
pub(crate) fn dependency_order(graph: &Graph) -> Result<Vec<usize>, Error> {
    let count = graph.values.len() + graph.flags.len();
    let mut colors = vec![0_u8; count];
    let mut order = Vec::with_capacity(count);
    for root in 0..count {
        if colors[root] != 0 {
            continue;
        }
        let mut work = vec![(root, false)];
        while let Some((node, finished)) = work.pop() {
            if finished {
                colors[node] = 2;
                order.push(node);
                continue;
            }
            match colors[node] {
                2 => continue,
                1 => return Err(Error("cyclic numeric/flag dependency".into())),
                _ => {}
            }
            colors[node] = 1;
            work.push((node, true));
            let mut children = Vec::with_capacity(4);
            if node < graph.values.len() {
                match graph.values[node].op {
                    ValueOp::Binary { left, right, .. } => {
                        children.extend([left.index(), right.index()])
                    }
                    ValueOp::Select { condition, yes, no } => {
                        children.extend([condition.index(), yes.index(), no.index()])
                    }
                    ValueOp::EvaluateFlag(flag) => children.push(graph.values.len() + flag.index()),
                    ValueOp::Alias(value) => children.push(value.index()),
                    _ => {}
                }
            } else {
                match graph.flags[node - graph.values.len()].op {
                    FlagOp::Leaf { args, .. } => children.extend(args.map(ValueId::index)),
                    FlagOp::Select { condition, yes, no } => children.extend([
                        condition.index(),
                        graph.values.len() + yes.index(),
                        graph.values.len() + no.index(),
                    ]),
                    FlagOp::Alias(flag) => children.push(graph.values.len() + flag.index()),
                    _ => {}
                }
            }
            for child in children.into_iter().rev() {
                require(child < count, "invalid dependency reference")?;
                if colors[child] != 2 {
                    work.push((child, false));
                }
            }
        }
    }
    Ok(order)
}

/// Check the boundary between pure values and ordered guest effects. This
/// verifies structure, not decoder truth: the decoded instruction remains the
/// source of opcode semantics, and generated-code tests check the lowering.
pub fn verify(graph: &Graph) -> Result<(), Error> {
    require(!graph.blocks.is_empty(), "empty value graph")?;
    // Discovery itself is bounded. These limits also bound verification and
    // prevent malformed internal graphs from allocating an unbounded worklist.
    require(
        graph.blocks.len() <= 65_536
            && graph.values.len() <= 2_000_000
            && graph.flags.len() <= 1_000_000
            && graph.effects.len() <= 1_000_000
            && graph.snapshots.len() <= 1_000_000,
        "value graph exceeds structural resource bound",
    )?;
    let check = Check { graph };
    check.block(graph.entry)?;
    require(
        graph.entries.len() == graph.blocks.len(),
        "entry map does not preserve every block",
    )?;
    for (index, block) in graph.blocks.iter().enumerate() {
        require(
            block.id.index() == index && graph.entries.get(&block.start) == Some(&block.id),
            "block identity disagrees with entry map",
        )?;
        require(!block.steps.is_empty(), "empty value block")?;
        check.cpu(&block.parameters, block.id)?;
        for (register, &value) in block.parameters.gpr.iter().enumerate() {
            require(
                graph.values[value.index()].op
                    == (ValueOp::Parameter {
                        block: block.id,
                        field: InputField::Gpr(register as u8),
                    }),
                "GPR entry parameter was specialized without an entry guard",
            )?;
        }
        for (field, value) in [
            (InputField::OtherFlags, block.parameters.flags.other),
            (InputField::DefinedFlags, block.parameters.flags.defined),
        ] {
            require(
                graph.values[value.index()].op
                    == (ValueOp::Parameter {
                        block: block.id,
                        field,
                    }),
                "flag metadata entry parameter was specialized",
            )?;
        }
        for (bit, &flag) in block.parameters.flags.arithmetic.iter().enumerate() {
            require(
                graph.flags[flag.index()].op
                    == (FlagOp::Parameter {
                        block: block.id,
                        bit: bit as u8,
                    }),
                "flag entry carrier was specialized",
            )?;
        }
        let mut previous = None;
        let mut previous_after = None;
        for (ordinal, step) in block.steps.iter().enumerate() {
            // A missing fetch is an unretired decoder sentinel, not a decoded
            // zero-byte instruction. Keep it so the generated execution path
            // can report the precise fetch fault after preceding effects.
            let missing_fetch = step.source.opcode == ir::Opcode::Unsupported
                && step.source.unsupported == Some(ir::UnsupportedReason::FetchUnavailable);
            require(
                (step.source.len != 0 || missing_fetch) && step.source.len <= 15,
                "invalid guest instruction length",
            )?;
            let before = check.snapshot(step.before, block.id)?;
            let after = check.snapshot(step.after, block.id)?;
            require(
                before.completed_prefix == ordinal as u32,
                "pre-instruction retirement prefix changed",
            )?;
            let retires = !matches!(
                step.source.opcode,
                ir::Opcode::Unsupported
                    | ir::Opcode::Ud2
                    | ir::Opcode::Rdtsc
                    | ir::Opcode::DiagnosticStop
            );
            require(
                after.completed_prefix == ordinal as u32 + u32::from(retires),
                "post-instruction retirement prefix changed",
            )?;
            if let Some(previous_after) = previous_after {
                let previous_after = check.snapshot(previous_after, block.id)?;
                require(
                    previous_after.cpu == before.cpu && previous_after.eip == before.eip
                        || previous_after.cpu == before.cpu
                            && constant(graph, previous_after.eip) == Some(step.source.eip),
                    "instruction state is not continuous",
                )?;
            }
            require(
                constant(graph, before.eip) == Some(step.source.eip),
                "fault recipe does not identify its instruction",
            )?;
            for &id in &step.effects {
                let effect = graph
                    .effects
                    .get(id.index())
                    .ok_or_else(|| Error("invalid effect reference".into()))?;
                require(
                    effect.block == block.id
                        && effect.previous == previous
                        && effect.before == step.before
                        && effect.after == step.after,
                    "effect ordering or precise exit recipe changed",
                )?;
                previous = Some(id);
            }
            previous_after = Some(step.after);
        }
        match block.terminator {
            Terminator::Direct { target, state } => {
                check.target(target)?;
                check.snapshot(state, block.id)?;
            }
            Terminator::Conditional {
                condition,
                taken,
                fallthrough,
                state,
            } => {
                check.value(condition, block.id)?;
                check.target(taken)?;
                check.target(fallthrough)?;
                check.snapshot(state, block.id)?;
            }
            Terminator::Indirect { target, state } => {
                check.value(target, block.id)?;
                check.snapshot(state, block.id)?;
            }
            Terminator::Stop { state } => {
                check.snapshot(state, block.id)?;
            }
        }
    }
    for value in &graph.values {
        if let Some(block) = value.home {
            check.block(block)?;
        }
        match value.op {
            ValueOp::Constant(_) => require(
                value.home.is_none(),
                "constant must be invocation independent",
            )?,
            ValueOp::Parameter { block, field } => {
                require(
                    value.home == Some(block),
                    "parameter home differs from its block",
                )?;
                if let InputField::Gpr(index) = field {
                    require(index < 8, "invalid GPR parameter")?;
                }
            }
            _ => {
                let block = value
                    .home
                    .ok_or_else(|| Error("nonconstant has no defining block".into()))?;
                match value.op {
                    ValueOp::Binary { left, right, .. } => {
                        check.value(left, block)?;
                        check.value(right, block)?;
                    }
                    ValueOp::Select { condition, yes, no } => {
                        check.value(condition, block)?;
                        check.value(yes, block)?;
                        check.value(no, block)?;
                    }
                    ValueOp::EvaluateFlag(flag) => {
                        let definition = graph
                            .flags
                            .get(flag.index())
                            .ok_or_else(|| Error("invalid demanded flag".into()))?;
                        check.flag(flag, block, definition.bit)?;
                    }
                    ValueOp::EffectResult { effect, output } => {
                        let effect = graph
                            .effects
                            .get(effect.index())
                            .ok_or_else(|| Error("invalid effect result".into()))?;
                        require(effect.block == block, "effect result crosses a block")?;
                        let valid = match (&effect.kind, output) {
                            (EffectKind::Load { .. }, EffectOutput::Loaded) => true,
                            (EffectKind::Opaque { contract, .. }, EffectOutput::Gpr(index)) => {
                                index < 8 && contract.gpr_written & (1 << index) != 0
                            }
                            (
                                EffectKind::Opaque { .. },
                                EffectOutput::PackedFlags | EffectOutput::DefinedFlags,
                            ) => true,
                            _ => false,
                        };
                        require(valid, "effect result is not produced by this effect")?;
                    }
                    ValueOp::Alias(target) => check.value(target, block)?,
                    _ => unreachable!(),
                }
            }
        }
    }
    for flag in &graph.flags {
        check.block(flag.home)?;
        require(flag.bit < 6, "invalid arithmetic flag bit")?;
        match flag.op {
            FlagOp::Parameter { block, bit } => require(
                block == flag.home && bit == flag.bit,
                "flag parameter identity mismatch",
            )?,
            FlagOp::Leaf { args, .. } => {
                for value in args {
                    check.value(value, flag.home)?;
                }
            }
            FlagOp::Select { condition, yes, no } => {
                check.value(condition, flag.home)?;
                check.flag(yes, flag.home, flag.bit)?;
                check.flag(no, flag.home, flag.bit)?;
            }
            FlagOp::Alias(target) => check.flag(target, flag.home, flag.bit)?,
        }
    }
    for snapshot in &graph.snapshots {
        check.block(snapshot.block)?;
        check.cpu(&snapshot.cpu, snapshot.block)?;
        check.value(snapshot.eip, snapshot.block)?;
    }
    let mut used_effects = vec![false; graph.effects.len()];
    for block in &graph.blocks {
        for step in &block.steps {
            for &id in &step.effects {
                require(!used_effects[id.index()], "effect appears twice")?;
                used_effects[id.index()] = true;
            }
        }
    }
    require(
        used_effects.iter().all(|used| *used),
        "unattached guest effect",
    )?;
    let order = dependency_order(graph)?;
    // Maximum producer index of each pure expression lets us reject reads of
    // future effects without repeatedly walking every exit recipe.
    let mut dependency = vec![None::<usize>; graph.values.len() + graph.flags.len()];
    let max_value = |id: ValueId, dependency: &[Option<usize>]| dependency[id.index()];
    let max_flag =
        |id: FlagRef, dependency: &[Option<usize>]| dependency[graph.values.len() + id.index()];
    for node in order {
        dependency[node] = if node < graph.values.len() {
            match graph.values[node].op {
                ValueOp::Binary { left, right, .. } => {
                    max_value(left, &dependency).max(max_value(right, &dependency))
                }
                ValueOp::Select { condition, yes, no } => max_value(condition, &dependency)
                    .max(max_value(yes, &dependency))
                    .max(max_value(no, &dependency)),
                ValueOp::EvaluateFlag(flag) => max_flag(flag, &dependency),
                ValueOp::EffectResult { effect, .. } => Some(effect.index()),
                ValueOp::Alias(value) => max_value(value, &dependency),
                _ => None,
            }
        } else {
            match graph.flags[node - graph.values.len()].op {
                FlagOp::Leaf { args, .. } => args
                    .into_iter()
                    .filter_map(|id| max_value(id, &dependency))
                    .max(),
                FlagOp::Select { condition, yes, no } => max_value(condition, &dependency)
                    .max(max_flag(yes, &dependency))
                    .max(max_flag(no, &dependency)),
                FlagOp::Alias(flag) => max_flag(flag, &dependency),
                _ => None,
            }
        };
    }
    let cpu_dependency = |cpu: &CpuValues| {
        cpu.gpr
            .iter()
            .copied()
            .chain([cpu.flags.other, cpu.flags.defined])
            .filter_map(|id| max_value(id, &dependency))
            .chain(
                cpu.flags
                    .arithmetic
                    .iter()
                    .filter_map(|&id| max_flag(id, &dependency)),
            )
            .max()
    };
    for (index, effect) in graph.effects.iter().enumerate() {
        check.block(effect.block)?;
        let mut latest = cpu_dependency(&check.snapshot(effect.before, effect.block)?.cpu);
        if let Some(guard) = effect.guard {
            check.value(guard, effect.block)?;
            latest = latest.max(max_value(guard, &dependency));
        }
        match &effect.kind {
            EffectKind::Load { address, access } => {
                require(
                    *access != Access::Write && effect.guard.is_none(),
                    "load lacks read permission or was conditionally suppressed",
                )?;
                check.value(address.offset, effect.block)?;
                latest = latest.max(max_value(address.offset, &dependency));
            }
            EffectKind::Store {
                address,
                value,
                checked_by,
            } => {
                check.value(address.offset, effect.block)?;
                check.value(*value, effect.block)?;
                latest = latest
                    .max(max_value(address.offset, &dependency))
                    .max(max_value(*value, &dependency));
                if let Some(id) = checked_by {
                    let proof = graph
                        .effects
                        .get(id.index())
                        .ok_or_else(|| Error("invalid RMW proof".into()))?;
                    require(
                        id.index() < index
                            && proof.block == effect.block
                            && proof.before == effect.before
                            && proof.after == effect.after
                            && matches!(proof.kind, EffectKind::Load { address: proof_address, access: Access::ReadWrite } if proof_address == *address),
                        "RMW store reuses a different or stale access proof",
                    )?;
                }
            }
            EffectKind::Opaque {
                inputs, contract, ..
            } => {
                check.cpu(inputs, effect.block)?;
                latest = latest.max(cpu_dependency(inputs));
                require(
                    contract.banks_read & !15 == 0 && contract.banks_written & !15 == 0,
                    "invalid opaque bank mask",
                )?;
                require(
                    effect.guard.is_none(),
                    "opaque instruction was conditionally suppressed",
                )?;
            }
        }
        require(
            latest.is_none_or(|producer| producer < index),
            "guest effect depends on an uncompleted effect",
        )?;
        let after = check.snapshot(effect.after, effect.block)?;
        let last_of_step = graph.blocks[effect.block.index()]
            .steps
            .iter()
            .find(|step| step.before == effect.before)
            .and_then(|step| step.effects.last())
            .copied()
            .ok_or_else(|| Error("effect has no owning instruction".into()))?;
        require(
            cpu_dependency(&after.cpu).is_none_or(|producer| producer <= last_of_step.index()),
            "post-instruction recipe reads a later instruction",
        )?;
    }
    Ok(())
}

fn constant(graph: &Graph, mut value: ValueId) -> Option<u32> {
    // Called only for diagnostics before the general cycle check. Bounded
    // traversal keeps malformed aliases from hanging the verifier itself.
    for _ in 0..=graph.values.len() {
        match graph.values.get(value.index())?.op {
            ValueOp::Constant(value) => return Some(value),
            ValueOp::Alias(next) => value = next,
            _ => return None,
        }
    }
    None
}

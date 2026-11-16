//! Pure, bounded graph simplification. Memory effects, guest retirement, entry
//! identities and precise snapshots are never removed or reordered here.
use super::*;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OptimizationStats {
    pub constants_folded: usize,
    pub identities_removed: usize,
    pub values_shared: usize,
    pub flag_leaves_shared: usize,
    pub flag_selections_removed: usize,
    pub normal_values: usize,
    pub normal_flag_evaluations: usize,
    pub exit_only_values: usize,
    pub exit_only_flag_evaluations: usize,
}

/// Values required by successful execution versus values only reachable from
/// exit recipes. `carried_flags` copies flat leaves; `evaluated_flags` computes
/// architectural bits. Conflating those sets would restore eager flags at
/// every edge. Sets include dependency closure, and IDs remain graph IDs.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Demand {
    pub values: BTreeSet<ValueId>,
    pub carried_flags: BTreeSet<FlagRef>,
    pub evaluated_flags: BTreeSet<FlagRef>,
}

pub fn fold_binary(op: BinaryOp, left: u32, right: u32) -> u32 {
    match op {
        BinaryOp::Add => left.wrapping_add(right),
        BinaryOp::Sub => left.wrapping_sub(right),
        BinaryOp::Mul => left.wrapping_mul(right),
        BinaryOp::And => left & right,
        BinaryOp::Or => left | right,
        BinaryOp::Xor => left ^ right,
        BinaryOp::Shl => left.wrapping_shl(right),
        BinaryOp::ShrU => left.wrapping_shr(right),
        BinaryOp::ShrS => ((left as i32) >> (right & 31)) as u32,
        BinaryOp::Eq => u32::from(left == right),
        BinaryOp::Ne => u32::from(left != right),
        BinaryOp::LtU => u32::from(left < right),
        BinaryOp::LtS => u32::from((left as i32) < (right as i32)),
        BinaryOp::LeU => u32::from(left <= right),
        BinaryOp::LeS => u32::from((left as i32) <= (right as i32)),
    }
}

/// Reference evaluation for a leaf whose operands are already available.
/// Undefined AF/OF/CF values are deterministic but not added to `defined`.
/// This function is also useful to test the emitter's leaf evaluator without
/// depending on the old backend's private lazy-flags representation.
pub fn evaluate_leaf(bit: u8, kind: FlagKind, width: ScalarWidth, args: [u32; 4]) -> u32 {
    let bits = width.bits();
    let mask = width.mask();
    let sign = 1_u32 << (bits - 1);
    let [a, b, carry, _] = args;
    if kind == FlagKind::Materialized {
        return a & 1;
    }
    let a = a & mask;
    let b_width = b & mask;
    let (result, cf, of, af) = match kind {
        FlagKind::Add => {
            let full = u64::from(a) + u64::from(b_width) + u64::from(carry & 1);
            let result = full as u32 & mask;
            (
                result,
                u32::from(full > u64::from(mask)),
                u32::from((!(a ^ b_width) & (a ^ result) & sign) != 0),
                ((a ^ b_width ^ result) >> 4) & 1,
            )
        }
        FlagKind::Sub => {
            let right = u64::from(b_width) + u64::from(carry & 1);
            let result = a.wrapping_sub(b_width).wrapping_sub(carry & 1) & mask;
            (
                result,
                u32::from(u64::from(a) < right),
                u32::from(((a ^ b_width) & (a ^ result) & sign) != 0),
                ((a ^ b_width ^ result) >> 4) & 1,
            )
        }
        FlagKind::Logic => (a, 0, 0, 0),
        FlagKind::Shl | FlagKind::Shr | FlagKind::Sar => {
            let count = b & 31;
            let signed = ((a << (32 - bits)) as i32) >> (32 - bits);
            let result = match kind {
                FlagKind::Shl => a.wrapping_shl(count),
                FlagKind::Shr => a.wrapping_shr(count),
                _ => (signed >> count) as u32,
            } & mask;
            let cf = if count == 0 {
                0
            } else if kind == FlagKind::Shl {
                if count <= bits {
                    (a >> (bits - count)) & 1
                } else {
                    0
                }
            } else if count <= bits {
                (a >> (count - 1)) & 1
            } else if kind == FlagKind::Sar {
                u32::from(a & sign != 0)
            } else {
                0
            };
            // For count>1 OF is architecturally undefined. Preserve the
            // existing backend's deterministic bit, while the separately
            // lifted defined mask continues to exclude it.
            let of = match kind {
                FlagKind::Shl => u32::from(result & sign != 0) ^ cf,
                FlagKind::Shr => u32::from(a & sign != 0),
                _ => 0,
            };
            (result, cf, of, 0)
        }
        FlagKind::Materialized => unreachable!(),
    };
    match bit {
        0 => cf,
        1 => u32::from((result as u8).count_ones() & 1 == 0),
        2 => af,
        3 => u32::from(result == 0),
        4 => u32::from(result & sign != 0),
        5 => of,
        _ => 0,
    }
}

fn commutative(op: BinaryOp) -> bool {
    matches!(
        op,
        BinaryOp::Add
            | BinaryOp::Mul
            | BinaryOp::And
            | BinaryOp::Or
            | BinaryOp::Xor
            | BinaryOp::Eq
            | BinaryOp::Ne
    )
}

/// Fold wrapping pure operations and share identical producers. Block homes
/// deliberately remain in CSE keys: all blocks admit arbitrary guest entry,
/// so a known predecessor does not dominate the target's external entry.
pub fn optimize(graph: &mut Graph) -> Result<OptimizationStats, Error> {
    verify(graph)?;
    let order = super::verify::dependency_order(graph)?;
    let value_count = graph.values.len();
    let mut values: Vec<_> = (0..value_count)
        .map(|index| ValueId(index as u32))
        .collect();
    let mut flags: Vec<_> = (0..graph.flags.len())
        .map(|index| FlagRef(index as u32))
        .collect();
    let mut value_table = BTreeMap::new();
    let mut flag_table = BTreeMap::new();
    let mut stats = OptimizationStats::default();
    for node in order {
        if node < value_count {
            let id = ValueId(node as u32);
            let mut op = graph.values[node].op.clone();
            let constant = |id: ValueId| match graph.values[id.index()].op {
                ValueOp::Constant(value) => Some(value),
                _ => None,
            };
            match &mut op {
                ValueOp::Binary { op, left, right } => {
                    *left = values[left.index()];
                    *right = values[right.index()];
                    if commutative(*op) && *right < *left {
                        std::mem::swap(left, right);
                    }
                }
                ValueOp::Select { condition, yes, no } => {
                    *condition = values[condition.index()];
                    *yes = values[yes.index()];
                    *no = values[no.index()];
                }
                ValueOp::EvaluateFlag(flag) => *flag = flags[flag.index()],
                ValueOp::Alias(value) => *value = values[value.index()],
                _ => {}
            }
            let replacement = match op {
                ValueOp::Binary { op, left, right } => {
                    let a = constant(left);
                    let b = constant(right);
                    if let (Some(a), Some(b)) = (a, b) {
                        Some(ValueOp::Constant(fold_binary(op, a, b)))
                    } else if matches!(op, BinaryOp::Add | BinaryOp::Or | BinaryOp::Xor)
                        && a == Some(0)
                    {
                        Some(ValueOp::Alias(right))
                    } else if matches!(
                        op,
                        BinaryOp::Add
                            | BinaryOp::Sub
                            | BinaryOp::Or
                            | BinaryOp::Xor
                            | BinaryOp::Shl
                            | BinaryOp::ShrU
                            | BinaryOp::ShrS
                    ) && b == Some(0)
                    {
                        Some(ValueOp::Alias(left))
                    } else if matches!(op, BinaryOp::Mul | BinaryOp::And)
                        && (a == Some(0) || b == Some(0))
                    {
                        Some(ValueOp::Constant(0))
                    } else if op == BinaryOp::Mul && a == Some(1)
                        || op == BinaryOp::And && a == Some(u32::MAX)
                    {
                        Some(ValueOp::Alias(right))
                    } else if op == BinaryOp::Mul && b == Some(1)
                        || op == BinaryOp::And && b == Some(u32::MAX)
                    {
                        Some(ValueOp::Alias(left))
                    } else if left == right {
                        match op {
                            BinaryOp::And | BinaryOp::Or => Some(ValueOp::Alias(left)),
                            BinaryOp::Sub
                            | BinaryOp::Xor
                            | BinaryOp::Ne
                            | BinaryOp::LtU
                            | BinaryOp::LtS => Some(ValueOp::Constant(0)),
                            BinaryOp::Eq | BinaryOp::LeU | BinaryOp::LeS => {
                                Some(ValueOp::Constant(1))
                            }
                            _ => None,
                        }
                    } else {
                        None
                    }
                }
                ValueOp::Select { condition, yes, no } => {
                    if yes == no {
                        Some(ValueOp::Alias(yes))
                    } else {
                        constant(condition)
                            .map(|condition| ValueOp::Alias(if condition == 0 { no } else { yes }))
                    }
                }
                ValueOp::EvaluateFlag(flag) => {
                    if let FlagOp::Leaf { kind, width, args } = graph.flags[flag.index()].op {
                        let constants = args.map(constant);
                        if let [Some(a), Some(b), Some(c), Some(d)] = constants {
                            Some(ValueOp::Constant(evaluate_leaf(
                                graph.flags[flag.index()].bit,
                                kind,
                                width,
                                [a, b, c, d],
                            )))
                        } else if kind == FlagKind::Materialized {
                            Some(ValueOp::Alias(args[0]))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }
                _ => None,
            };
            if let Some(replacement) = replacement {
                if matches!(replacement, ValueOp::Constant(_)) {
                    stats.constants_folded += 1;
                } else {
                    stats.identities_removed += 1;
                }
                op = replacement;
            }
            if let ValueOp::Alias(target) = op {
                values[node] = target;
                graph.values[node].op = ValueOp::Alias(target);
                continue;
            }
            let home = if matches!(op, ValueOp::Constant(_)) {
                None
            } else {
                graph.values[node].home
            };
            let key = (home, op.clone());
            if let Some(&previous) = value_table.get(&key) {
                values[node] = previous;
                graph.values[node] = if home.is_none() {
                    Value { home, op }
                } else {
                    Value {
                        home,
                        op: ValueOp::Alias(previous),
                    }
                };
                stats.values_shared += 1;
            } else {
                graph.values[node] = Value { home, op };
                value_table.insert(key, id);
            }
        } else {
            let index = node - value_count;
            let id = FlagRef(index as u32);
            let flag = &graph.flags[index];
            let mut op = flag.op.clone();
            match &mut op {
                FlagOp::Leaf { args, .. } => {
                    for value in args {
                        *value = values[value.index()];
                    }
                }
                FlagOp::Select { condition, yes, no } => {
                    *condition = values[condition.index()];
                    *yes = flags[yes.index()];
                    *no = flags[no.index()];
                }
                FlagOp::Alias(flag) => *flag = flags[flag.index()],
                _ => {}
            }
            if let FlagOp::Select { condition, yes, no } = op {
                if yes == no {
                    op = FlagOp::Alias(yes);
                } else if let ValueOp::Constant(value) = graph.values[condition.index()].op {
                    op = FlagOp::Alias(if value == 0 { no } else { yes });
                }
                if matches!(op, FlagOp::Alias(_)) {
                    stats.flag_selections_removed += 1;
                }
            }
            if let FlagOp::Alias(target) = op {
                flags[index] = target;
                graph.flags[index].op = op;
                continue;
            }
            let key = (flag.home, flag.bit, op.clone());
            if let Some(&previous) = flag_table.get(&key) {
                flags[index] = previous;
                graph.flags[index].op = FlagOp::Alias(previous);
                stats.flag_leaves_shared += 1;
            } else {
                graph.flags[index].op = op;
                flag_table.insert(key, id);
            }
        }
    }
    let cpu = |cpu: &mut CpuValues| {
        for value in &mut cpu.gpr {
            *value = values[value.index()];
        }
        cpu.flags.other = values[cpu.flags.other.index()];
        cpu.flags.defined = values[cpu.flags.defined.index()];
        for flag in &mut cpu.flags.arithmetic {
            *flag = flags[flag.index()];
        }
    };
    for snapshot in &mut graph.snapshots {
        cpu(&mut snapshot.cpu);
        snapshot.eip = values[snapshot.eip.index()];
    }
    for effect in &mut graph.effects {
        effect.guard = effect.guard.map(|id| values[id.index()]);
        match &mut effect.kind {
            EffectKind::Load { address, .. } => address.offset = values[address.offset.index()],
            EffectKind::Store { address, value, .. } => {
                address.offset = values[address.offset.index()];
                *value = values[value.index()];
            }
            EffectKind::Opaque { inputs, .. } => cpu(inputs),
        }
    }
    for block in &mut graph.blocks {
        cpu(&mut block.parameters);
        match &mut block.terminator {
            Terminator::Conditional { condition, .. } => *condition = values[condition.index()],
            Terminator::Indirect { target, .. } => *target = values[target.index()],
            _ => {}
        }
    }
    verify(graph)?;
    let (normal, exits) = demands(graph)?;
    stats.normal_values = normal.values.len();
    stats.normal_flag_evaluations = normal.evaluated_flags.len();
    stats.exit_only_values = exits.values.difference(&normal.values).count();
    stats.exit_only_flag_evaluations = exits
        .evaluated_flags
        .difference(&normal.evaluated_flags)
        .count();
    Ok(stats)
}

#[derive(Clone, Copy)]
enum Need {
    Value(ValueId),
    Carry(FlagRef),
    Evaluate(FlagRef),
}

fn close(graph: &Graph, mut work: Vec<Need>) -> Demand {
    let mut demand = Demand::default();
    while let Some(need) = work.pop() {
        match need {
            Need::Value(id) => {
                if !demand.values.insert(id) {
                    continue;
                }
                match graph.values[id.index()].op {
                    ValueOp::Binary { left, right, .. } => {
                        work.extend([Need::Value(left), Need::Value(right)])
                    }
                    ValueOp::Select { condition, yes, no } => {
                        work.extend([Need::Value(condition), Need::Value(yes), Need::Value(no)])
                    }
                    ValueOp::EvaluateFlag(flag) => work.push(Need::Evaluate(flag)),
                    ValueOp::Alias(value) => work.push(Need::Value(value)),
                    _ => {}
                }
            }
            Need::Carry(id) | Need::Evaluate(id) => {
                let evaluate = matches!(need, Need::Evaluate(_));
                let newly_carried = demand.carried_flags.insert(id);
                let newly_evaluated = evaluate && demand.evaluated_flags.insert(id);
                if !newly_carried && !newly_evaluated {
                    continue;
                }
                let flag_need = |id| {
                    if evaluate {
                        Need::Evaluate(id)
                    } else {
                        Need::Carry(id)
                    }
                };
                match graph.flags[id.index()].op {
                    FlagOp::Leaf { args, .. } => work.extend(args.map(Need::Value)),
                    FlagOp::Select { condition, yes, no } => {
                        work.extend([Need::Value(condition), flag_need(yes), flag_need(no)])
                    }
                    FlagOp::Alias(flag) => work.push(flag_need(flag)),
                    _ => {}
                }
            }
        }
    }
    demand
}

fn cpu_roots(cpu: &CpuValues, evaluate: bool, work: &mut Vec<Need>) {
    work.extend(cpu.gpr.map(Need::Value));
    work.extend([Need::Value(cpu.flags.other), Need::Value(cpu.flags.defined)]);
    work.extend(cpu.flags.arithmetic.map(|id| {
        if evaluate {
            Need::Evaluate(id)
        } else {
            Need::Carry(id)
        }
    }));
}

/// Return (successful-path demand, precise-exit demand). Edge carriers keep
/// their numeric inputs alive without computing the flag bits themselves.
/// The emitter can further split each set by block/instruction and only emit
/// the exceptional closure in the corresponding cold exit branch.
pub fn demands(graph: &Graph) -> Result<(Demand, Demand), Error> {
    verify(graph)?;
    let mut normal = Vec::new();
    let mut exits = Vec::new();
    for snapshot in &graph.snapshots {
        cpu_roots(&snapshot.cpu, true, &mut exits);
        exits.push(Need::Value(snapshot.eip));
    }
    for effect in &graph.effects {
        if let Some(guard) = effect.guard {
            normal.push(Need::Value(guard));
        }
        match &effect.kind {
            EffectKind::Load { address, .. } => normal.push(Need::Value(address.offset)),
            EffectKind::Store { address, value, .. } => {
                normal.extend([Need::Value(address.offset), Need::Value(*value)])
            }
            EffectKind::Opaque {
                inputs, contract, ..
            } => {
                // Existing precise adapters update validity from its incoming
                // value; dynamic zero-count instructions may preserve it all.
                normal.push(Need::Value(inputs.flags.defined));
                for (index, value) in inputs.gpr.iter().enumerate() {
                    if contract.gpr_read & (1 << index) != 0 {
                        normal.push(Need::Value(*value));
                    }
                }
                for (bit, mask) in FLAG_MASKS.iter().enumerate() {
                    if contract.flags_read & mask != 0 {
                        normal.push(Need::Evaluate(inputs.flags.arithmetic[bit]));
                    }
                }
                if contract.flags_read & !ARITHMETIC_MASK != 0 {
                    normal.push(Need::Value(inputs.flags.other));
                }
            }
        }
    }
    for block in &graph.blocks {
        let (state, materialize) = match block.terminator {
            Terminator::Direct { target, state } => (state, matches!(target, Target::External(_))),
            Terminator::Conditional {
                condition,
                taken,
                fallthrough,
                state,
            } => {
                normal.push(Need::Value(condition));
                (
                    state,
                    matches!(taken, Target::External(_))
                        || matches!(fallthrough, Target::External(_)),
                )
            }
            Terminator::Indirect { target, state } => {
                normal.push(Need::Value(target));
                (state, true)
            }
            Terminator::Stop { .. } => continue,
        };
        // External/indirect edges currently cross the packed linked ABI, and
        // consequently are explicit materialization gates. Known local edges
        // only carry the flat recipes to the target's entry parameters.
        cpu_roots(
            &graph.snapshots[state.index()].cpu,
            materialize,
            &mut normal,
        );
    }
    Ok((close(graph, normal), close(graph, exits)))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn integer_folding_is_modulo_32_with_masked_shift_counts() {
        assert_eq!(fold_binary(BinaryOp::Add, u32::MAX, 1), 0);
        assert_eq!(fold_binary(BinaryOp::Sub, 0, 1), u32::MAX);
        assert_eq!(fold_binary(BinaryOp::Mul, 0x8000_0000, 2), 0);
        assert_eq!(fold_binary(BinaryOp::Shl, 3, 32), 3);
        assert_eq!(fold_binary(BinaryOp::ShrS, 0x8000_0000, 255), u32::MAX);
    }
    #[test]
    fn leaf_carry_overflow_and_borrow_include_input_carry() {
        for width in [ScalarWidth::W8, ScalarWidth::W16, ScalarWidth::W32] {
            let mask = width.mask();
            let sign = 1 << (width.bits() - 1);
            assert_eq!(evaluate_leaf(0, FlagKind::Add, width, [mask, 0, 1, 0]), 1);
            assert_eq!(evaluate_leaf(3, FlagKind::Add, width, [mask, 0, 1, 0]), 1);
            assert_eq!(
                evaluate_leaf(5, FlagKind::Add, width, [sign - 1, 0, 1, 0]),
                1
            );
            assert_eq!(evaluate_leaf(0, FlagKind::Sub, width, [0, mask, 1, 0]), 1);
            assert_eq!(evaluate_leaf(3, FlagKind::Sub, width, [0, mask, 1, 0]), 1);
            assert_eq!(evaluate_leaf(5, FlagKind::Sub, width, [sign, 0, 1, 0]), 1);
        }
    }
}

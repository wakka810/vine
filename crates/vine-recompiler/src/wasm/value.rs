//! Value/effect lowering over the existing checked memory and complex-operation
//! emitters. Direct edges carry bounded flag recipes; only consumers and exits
//! evaluate their bits. No guest-visible state or host helper ABI is added.
use super::*;
use crate::predicate_ir as p;
use crate::value_ir::{self as v, *};
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet};
mod flow;
mod flow_allocate;
mod flow_blocks;

const PARAMETER_WORDS: u32 = 40;
const READY: u32 = 40;
const BLOCK_LEFT: u32 = 41;
const EXIT_CONTEXT: u32 = 42;
const FIRST_VALUE: u32 = 43;

/// All members describe compiler-owned state. REP alone may have committed
/// element progress without retiring the current guest instruction.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct ExitContext {
    snapshot: SnapshotId,
    preserve_gprs: u8,
    preserve_flags: bool,
    preserve_defined: bool,
    preserve_position: bool,
}

#[derive(Default)]
struct ColdExits {
    indices: BTreeMap<ExitContext, u32>,
    contexts: Vec<ExitContext>,
}
impl ColdExits {
    fn intern(&mut self, context: ExitContext) -> u32 {
        if let Some(&index) = self.indices.get(&context) {
            return index;
        }
        let index = self.contexts.len() as u32;
        self.contexts.push(context);
        self.indices.insert(context, index);
        index
    }
}
impl ExitContext {
    fn exact(snapshot: SnapshotId) -> Self {
        Self {
            snapshot,
            preserve_gprs: 0,
            preserve_flags: false,
            preserve_defined: false,
            preserve_position: false,
        }
    }
}

#[derive(Clone, Default)]
struct Emission {
    values: BTreeSet<ValueId>,
    carriers: BTreeSet<FlagRef>,
    predicates: BTreeSet<p::ValueId>,
    flow_values: BTreeMap<crate::flow_ir::ValueId, u32>,
    exit_only: bool,
}

pub(super) struct Plan {
    graph: Graph,
    stats: OptimizationStats,
    base: u32,
    locals: u32,
    value_locals: Vec<u32>,
    carrier_locals: Vec<u32>,
    outputs: BTreeMap<EffectId, Vec<(ValueId, EffectOutput)>>,
    flag_function: u32,
    normal_evaluations: Cell<u32>,
    exit_evaluations: Cell<u32>,
    carrier_copies: Cell<u32>,
    opaque_calls: Cell<u32>,
    cold_exits: RefCell<ColdExits>,
    cold_exit_sites: Cell<u32>,
    cold_exit_branch_bytes: Cell<u32>,
    cold_exit_body_bytes: Cell<u32>,
    snapshot_emissions: Cell<u32>,
    snapshot_bytes: Cell<u32>,
    predicate: Option<PredicatePlan>,
    flow: Option<flow::Plan>,
    local_recovery: RefCell<Option<(String, Emission)>>,
}

struct PredicatePlan {
    graph: p::Graph,
    locals: Vec<u32>,
}

fn width(width: ScalarWidth) -> Width {
    match width {
        ScalarWidth::W8 => Width::W8,
        ScalarWidth::W16 => Width::W16,
        ScalarWidth::W32 => Width::W32,
    }
}
fn tag(bit: u8, kind: FlagKind, width: ScalarWidth) -> u32 {
    let kind = match kind {
        FlagKind::Materialized => 0,
        FlagKind::Add => 1,
        FlagKind::Sub => 2,
        FlagKind::Logic => 3,
        FlagKind::Shl => 4,
        FlagKind::Shr => 5,
        FlagKind::Sar => 6,
    };
    let size = match width {
        ScalarWidth::W8 => 0,
        ScalarWidth::W16 => 1,
        ScalarWidth::W32 => 2,
    };
    u32::from(bit) | (kind << 3) | (size << 6)
}
fn binary_opcode(op: BinaryOp) -> u8 {
    match op {
        BinaryOp::Add => 0x6a,
        BinaryOp::Sub => 0x6b,
        BinaryOp::Mul => 0x6c,
        BinaryOp::And => 0x71,
        BinaryOp::Or => 0x72,
        BinaryOp::Xor => 0x73,
        BinaryOp::Shl => 0x74,
        BinaryOp::ShrS => 0x75,
        BinaryOp::ShrU => 0x76,
        BinaryOp::Eq => 0x46,
        BinaryOp::Ne => 0x47,
        BinaryOp::LtS => 0x48,
        BinaryOp::LtU => 0x49,
        BinaryOp::LeS => 0x4c,
        BinaryOp::LeU => 0x4d,
    }
}

impl Plan {
    pub(super) fn new(
        region: &Region,
        base: u32,
        flag_function: u32,
        predicate: bool,
        flow: bool,
    ) -> Result<Self, String> {
        let normalized = if flow {
            Some(flow_blocks::normalize(region)?)
        } else {
            None
        };
        let mut graph =
            v::lift(normalized.as_ref().unwrap_or(region)).map_err(|error| error.to_string())?;
        let stats = v::optimize(&mut graph).map_err(|error| error.to_string())?;
        v::verify(&graph).map_err(|error| error.to_string())?;
        let mut value_locals = vec![u32::MAX; graph.values.len()];
        let mut carrier_locals = vec![u32::MAX; graph.flags.len()];
        let mut outputs: BTreeMap<EffectId, Vec<_>> = BTreeMap::new();
        let mut locals = FIRST_VALUE;
        // Only one block executes at a time. Reuse its SSA storage across
        // blocks; incoming parameters have a separate parallel-copy bank.
        for block in graph.blocks.iter().filter(|_| !flow) {
            let mut next = FIRST_VALUE;
            for (index, value) in graph.values.iter().enumerate() {
                if value.home != Some(block.id) {
                    continue;
                }
                value_locals[index] = match value.op {
                    ValueOp::Parameter { field, .. } => {
                        base + match field {
                            InputField::Gpr(index) => u32::from(index),
                            InputField::OtherFlags => 8,
                            InputField::DefinedFlags => 9,
                        }
                    }
                    ValueOp::Constant(_) | ValueOp::Alias(_) => u32::MAX,
                    _ => {
                        let local = base + next;
                        next += 1;
                        local
                    }
                };
                if let ValueOp::EffectResult { effect, output } = value.op {
                    outputs
                        .entry(effect)
                        .or_default()
                        .push((ValueId(index as u32), output));
                }
            }
            for (index, flag) in graph.flags.iter().enumerate() {
                if flag.home == block.id && matches!(flag.op, FlagOp::Select { .. }) {
                    carrier_locals[index] = base + next;
                    next += 5;
                }
            }
            locals = locals.max(next);
        }
        let predicate = if predicate {
            let ir = p::Graph::build(&graph).map_err(|error| error.to_string())?;
            let mut allocated = vec![u32::MAX; ir.values.len()];
            let mut next = BTreeMap::<BlockId, u32>::new();
            locals = FIRST_VALUE;
            for (index, node) in ir.values.iter().enumerate() {
                allocated[index] = match node.op {
                    p::Op::Constant(_) => u32::MAX,
                    p::Op::Parameter { field, .. } => {
                        base + match field {
                            p::Parameter::Gpr(n) => u32::from(n),
                            p::Parameter::OtherFlags => 8,
                            p::Parameter::DefinedFlags => 9,
                            p::Parameter::Predicate(bit) => 10 + u32::from(bit),
                        }
                    }
                    _ => {
                        let n = next
                            .entry(node.home.expect("nonconstant node home"))
                            .or_insert(FIRST_VALUE);
                        let local = base + *n;
                        *n += 1;
                        locals = locals.max(*n);
                        local
                    }
                };
            }
            for (index, id) in ir.value_map.iter().enumerate() {
                value_locals[index] = allocated[id.index()];
            }
            Some(PredicatePlan {
                graph: ir,
                locals: allocated,
            })
        } else {
            None
        };
        let flow = if flow {
            for (index, value) in graph.values.iter().enumerate() {
                if let ValueOp::EffectResult { effect, output } = value.op {
                    outputs
                        .entry(effect)
                        .or_default()
                        .push((ValueId(index as u32), output));
                }
            }
            let entries = region.blocks.keys().map(|pc| graph.entries[pc]).collect();
            let plan = flow::Plan::new(&graph, &entries, base + FIRST_VALUE)?;
            locals = FIRST_VALUE + plan.locals;
            Some(plan)
        } else {
            None
        };
        Ok(Self {
            graph,
            stats,
            base,
            locals,
            value_locals,
            carrier_locals,
            outputs,
            flag_function,
            normal_evaluations: Cell::new(0),
            exit_evaluations: Cell::new(0),
            carrier_copies: Cell::new(0),
            opaque_calls: Cell::new(0),
            cold_exits: RefCell::new(ColdExits::default()),
            cold_exit_sites: Cell::new(0),
            cold_exit_branch_bytes: Cell::new(0),
            cold_exit_body_bytes: Cell::new(0),
            snapshot_emissions: Cell::new(0),
            snapshot_bytes: Cell::new(0),
            predicate,
            flow,
            local_recovery: RefCell::new(None),
        })
    }

    pub(super) fn local_count(&self) -> u32 {
        self.flow
            .as_ref()
            .map_or(self.locals, |plan| FIRST_VALUE + plan.local_count())
    }

    pub(super) fn metadata(&self) -> Vec<u8> {
        if let Some(plan) = &self.flow {
            return plan.metadata();
        }
        if let Some(plan) = &self.predicate {
            return format!("{{\"version\":1,\"blocks\":{},\"values\":{},\"folded\":{},\"shared\":{},\"ssaLocals\":{},\"predicateWords\":6,\"runtimeFlagTags\":0,\"internalIndirectPublication\":false,\"opaqueAdapters\":{}}}",
                self.graph.blocks.len(), plan.graph.values.len(), plan.graph.stats.folded, plan.graph.stats.shared, self.locals, self.opaque_calls.get()).into_bytes();
        }
        format!(
            concat!(
                "{{\"version\":1,\"blocks\":{},\"ssaLocals\":{},",
                "\"normalFlagEvaluations\":{},\"exitFlagEvaluations\":{},",
                "\"carrierCopies\":{},\"opaqueAdapters\":{},",
                "\"normalValues\":{},\"exitOnlyValues\":{},",
                "\"normalDemandedFlags\":{},\"exitOnlyDemandedFlags\":{},",
                "\"valuesShared\":{},\"constantsFolded\":{},",
                "\"flagCarrierWords\":30,\"memoryChecks\":\"existing-precise\",",
                "\"coldExitBranchSites\":{},\"coldExitSnapshots\":{},",
                "\"coldExitBranchBytes\":{},\"coldExitBodyBytes\":{},",
                "\"snapshotEmissions\":{},\"snapshotBytes\":{}}}"
            ),
            self.graph.blocks.len(),
            self.locals,
            self.normal_evaluations.get(),
            self.exit_evaluations.get(),
            self.carrier_copies.get(),
            self.opaque_calls.get(),
            self.stats.normal_values,
            self.stats.exit_only_values,
            self.stats.normal_flag_evaluations,
            self.stats.exit_only_flag_evaluations,
            self.stats.values_shared,
            self.stats.constants_folded,
            self.cold_exit_sites.get(),
            self.cold_exits.borrow().contexts.len(),
            self.cold_exit_branch_bytes.get(),
            self.cold_exit_body_bytes.get(),
            self.snapshot_emissions.get(),
            self.snapshot_bytes.get()
        )
        .into_bytes()
    }

    fn value(&self, e: &mut Emitter, id: ValueId, state: &mut Emission) {
        if let Some(plan) = &self.flow {
            plan.value(e, plan.graph.value_at(plan.current(), id), state);
            return;
        }
        if let Some(plan) = &self.predicate {
            self.predicate_value(e, plan.graph.value_map[id.index()], state);
            return;
        }
        let op = &self.graph.values[id.index()].op;
        match *op {
            ValueOp::Constant(value) => {
                e.num(value);
                return;
            }
            ValueOp::Parameter { .. } | ValueOp::EffectResult { .. } => {
                e.get(self.value_locals[id.index()]);
                return;
            }
            ValueOp::Alias(other) => {
                self.value(e, other, state);
                return;
            }
            _ => {}
        }
        if state.values.contains(&id) {
            e.get(self.value_locals[id.index()]);
            return;
        }
        match *op {
            ValueOp::Binary { op, left, right } => {
                self.value(e, left, state);
                self.value(e, right, state);
                e.op(binary_opcode(op));
            }
            ValueOp::Select { condition, yes, no } => {
                self.value(e, yes, state);
                self.value(e, no, state);
                self.value(e, condition, state);
                e.op(0x1b);
            }
            ValueOp::EvaluateFlag(flag) => self.evaluate_flag(e, flag, state),
            _ => unreachable!("handled scalar producers"),
        }
        e.tee(self.value_locals[id.index()]);
        state.values.insert(id);
    }

    fn predicate_value(&self, e: &mut Emitter, id: p::ValueId, state: &mut Emission) {
        let plan = self.predicate.as_ref().expect("predicate lowering");
        match plan.graph.values[id.index()].op {
            p::Op::Constant(n) => {
                e.num(n);
                return;
            }
            p::Op::Parameter { .. } | p::Op::EffectResult { .. } => {
                e.get(plan.locals[id.index()]);
                return;
            }
            _ => {}
        }
        if state.predicates.contains(&id) {
            e.get(plan.locals[id.index()]);
            return;
        }
        match plan.graph.values[id.index()].op {
            p::Op::Binary { op, left, right } => {
                self.predicate_value(e, left, state);
                self.predicate_value(e, right, state);
                e.op(binary_opcode(op));
            }
            p::Op::Select { condition, yes, no } => {
                self.predicate_value(e, yes, state);
                self.predicate_value(e, no, state);
                self.predicate_value(e, condition, state);
                e.op(0x1b);
            }
            p::Op::Popcnt(value) => {
                self.predicate_value(e, value, state);
                e.op(0x69);
            }
            _ => unreachable!("atomic predicate values handled above"),
        }
        e.tee(plan.locals[id.index()]);
        state.predicates.insert(id);
    }

    fn predicate_condition(e: &mut Emitter, bit: usize) {
        match bit {
            0 => {
                e.num(1);
                e.op(0x71);
            }
            1 => {
                e.num(255);
                e.op(0x71);
                e.op(0x69);
                e.num(1);
                e.op(0x71);
                e.op(0x45);
            }
            2 => {
                e.num(4);
                e.op(0x76);
                e.num(1);
                e.op(0x71);
            }
            3 => e.op(0x45),
            4 | 5 => {
                e.num(31);
                e.op(0x76);
            }
            _ => unreachable!("architectural predicate"),
        }
    }

    /// Only the shared lookup-miss path publishes an internal continuation.
    /// A successful lookup enters with the numeric parameter bank still live.
    pub(super) fn emit_lookup_miss(&self, e: &mut Emitter) {
        if self.predicate.is_none() {
            return;
        }
        e.get(self.base + READY);
        e.if_();
        for index in 0..8 {
            e.get(self.base + index);
            e.set(3 + index);
        }
        e.get(self.base + 8);
        for (bit, mask) in FLAG_MASKS.iter().enumerate() {
            e.get(self.base + 10 + bit as u32);
            Self::predicate_condition(e, bit);
            e.num(mask.trailing_zeros());
            e.op(0x74);
            e.op(0x72);
        }
        e.set(FLAGS);
        e.get(self.base + 9);
        e.set(DEFINED);
        e.num(0);
        e.set(self.base + READY);
        e.end();
    }

    fn carrier(&self, e: &mut Emitter, id: FlagRef, component: u32, state: &mut Emission) {
        let flag = &self.graph.flags[id.index()];
        match flag.op {
            FlagOp::Parameter { bit, .. } => e.get(self.base + 10 + u32::from(bit) * 5 + component),
            FlagOp::Leaf { kind, width, args } => {
                if component == 0 {
                    e.num(tag(flag.bit, kind, width));
                } else {
                    self.value(e, args[component as usize - 1], state);
                }
            }
            FlagOp::Alias(other) => self.carrier(e, other, component, state),
            FlagOp::Select { condition, yes, no } => {
                let local = self.carrier_locals[id.index()];
                if !state.carriers.contains(&id) {
                    for field in 0..5 {
                        self.carrier(e, yes, field, state);
                        self.carrier(e, no, field, state);
                        self.value(e, condition, state);
                        e.op(0x1b);
                    }
                    for field in (0..5).rev() {
                        e.set(local + field);
                    }
                    state.carriers.insert(id);
                    self.carrier_copies.set(self.carrier_copies.get() + 1);
                }
                e.get(local + component);
            }
        }
    }

    fn result(
        &self,
        e: &mut Emitter,
        kind: FlagKind,
        size: ScalarWidth,
        args: [ValueId; 4],
        state: &mut Emission,
    ) {
        self.value(e, args[0], state);
        match kind {
            FlagKind::Add | FlagKind::Sub => {
                self.value(e, args[1], state);
                e.op(if kind == FlagKind::Add { 0x6a } else { 0x6b });
                self.value(e, args[2], state);
                e.num(1);
                e.op(0x71);
                e.op(if kind == FlagKind::Add { 0x6a } else { 0x6b });
            }
            FlagKind::Shl | FlagKind::Shr | FlagKind::Sar => {
                if kind == FlagKind::Sar {
                    e.sign_extend(width(size));
                }
                self.value(e, args[1], state);
                e.op(match kind {
                    FlagKind::Shl => 0x74,
                    FlagKind::Shr => 0x76,
                    _ => 0x75,
                });
            }
            FlagKind::Logic | FlagKind::Materialized => {}
        }
        e.truncate(width(size));
    }

    fn leaf(
        &self,
        e: &mut Emitter,
        bit: u8,
        kind: FlagKind,
        size: ScalarWidth,
        args: [ValueId; 4],
        state: &mut Emission,
    ) {
        if kind == FlagKind::Materialized {
            self.value(e, args[0], state);
            e.num(1);
            e.op(0x71);
            return;
        }
        // Static recipes omit the runtime tag switch. Dynamic incoming leaves
        // share one small module-local evaluator instead of cloning it at exits.
        match bit {
            0 => match kind {
                FlagKind::Add => {
                    self.value(e, args[0], state);
                    e.op(0xad);
                    self.value(e, args[1], state);
                    e.op(0xad);
                    e.op(0x7c);
                    self.value(e, args[2], state);
                    e.num(1);
                    e.op(0x71);
                    e.op(0xad);
                    e.op(0x7c);
                    e.op(0x42);
                    sleb(&mut e.code, i64::from(size.mask()));
                    e.op(0x56);
                }
                FlagKind::Sub => {
                    self.value(e, args[0], state);
                    e.op(0xad);
                    self.value(e, args[1], state);
                    e.op(0xad);
                    self.value(e, args[2], state);
                    e.num(1);
                    e.op(0x71);
                    e.op(0xad);
                    e.op(0x7c);
                    e.op(0x54);
                }
                FlagKind::Shl | FlagKind::Shr | FlagKind::Sar => {
                    self.shift_carry(e, kind, size, args, state)
                }
                _ => e.num(0),
            },
            1 => {
                self.result(e, kind, size, args, state);
                e.num(255);
                e.op(0x71);
                e.op(0x69);
                e.num(1);
                e.op(0x71);
                e.op(0x45);
            }
            2 => {
                if matches!(kind, FlagKind::Add | FlagKind::Sub) {
                    self.value(e, args[0], state);
                    self.value(e, args[1], state);
                    e.op(0x73);
                    self.result(e, kind, size, args, state);
                    e.op(0x73);
                    e.num(4);
                    e.op(0x76);
                    e.num(1);
                    e.op(0x71);
                } else {
                    e.num(0);
                }
            }
            3 => {
                self.result(e, kind, size, args, state);
                e.op(0x45);
            }
            4 => {
                self.result(e, kind, size, args, state);
                e.num(size.bits() - 1);
                e.op(0x76);
                e.num(1);
                e.op(0x71);
            }
            5 => {
                if matches!(kind, FlagKind::Add | FlagKind::Sub) {
                    self.value(e, args[0], state);
                    self.value(e, args[1], state);
                    e.op(0x73);
                    if kind == FlagKind::Add {
                        e.num(u32::MAX);
                        e.op(0x73);
                    }
                    self.value(e, args[0], state);
                    self.result(e, kind, size, args, state);
                    e.op(0x73);
                    e.op(0x71);
                    e.num(size.bits() - 1);
                    e.op(0x76);
                    e.num(1);
                    e.op(0x71);
                } else if kind == FlagKind::Shl {
                    self.result(e, kind, size, args, state);
                    e.num(size.bits() - 1);
                    e.op(0x76);
                    e.num(1);
                    e.op(0x71);
                    self.shift_carry(e, kind, size, args, state);
                    e.op(0x73);
                } else if kind == FlagKind::Shr {
                    self.value(e, args[0], state);
                    e.num(size.bits() - 1);
                    e.op(0x76);
                    e.num(1);
                    e.op(0x71);
                } else {
                    e.num(0);
                }
            }
            _ => unreachable!("verified flag bit"),
        }
    }

    fn shift_carry(
        &self,
        e: &mut Emitter,
        kind: FlagKind,
        size: ScalarWidth,
        args: [ValueId; 4],
        state: &mut Emission,
    ) {
        self.value(e, args[0], state);
        if kind == FlagKind::Shl {
            e.num(size.bits());
            self.value(e, args[1], state);
            e.op(0x6b);
        } else {
            self.value(e, args[1], state);
            e.num(1);
            e.op(0x6b);
        }
        e.op(0x76);
        e.num(1);
        e.op(0x71);
        if kind == FlagKind::Sar {
            self.value(e, args[0], state);
            e.num(size.bits() - 1);
            e.op(0x76);
            e.num(1);
            e.op(0x71);
        } else {
            e.num(0);
        }
        self.value(e, args[1], state);
        e.num(size.bits());
        e.op(0x4d);
        e.op(0x1b);
        e.num(0);
        self.value(e, args[1], state);
        e.num(0);
        e.op(0x47);
        e.op(0x1b);
    }

    fn evaluate_flag(&self, e: &mut Emitter, id: FlagRef, state: &mut Emission) {
        if let Some(plan) = &self.flow {
            plan.value(e, plan.graph.condition_at(plan.current(), id), state);
            return;
        }
        let counter = if state.exit_only {
            &self.exit_evaluations
        } else {
            &self.normal_evaluations
        };
        counter.set(counter.get() + 1);
        if let Some(plan) = &self.predicate {
            self.predicate_value(e, plan.graph.conditions[id.index()], state);
            return;
        }
        let flag = &self.graph.flags[id.index()];
        match flag.op {
            FlagOp::Alias(other) => self.evaluate_flag(e, other, state),
            FlagOp::Leaf { kind, width, args } => self.leaf(e, flag.bit, kind, width, args, state),
            _ => {
                for component in 0..5 {
                    self.carrier(e, id, component, state);
                }
                e.op(0x10);
                uleb(&mut e.code, self.flag_function);
            }
        }
    }

    fn flags(&self, e: &mut Emitter, flags: &Flags, mask: u32, state: &mut Emission) {
        if mask & !ARITHMETIC_MASK != 0 {
            self.value(e, flags.other, state);
            e.num(mask & !ARITHMETIC_MASK);
            e.op(0x71);
        } else {
            e.num(0);
        }
        for (bit, flag_mask) in FLAG_MASKS.iter().enumerate() {
            if mask & flag_mask == 0 {
                continue;
            }
            self.evaluate_flag(e, flags.arithmetic[bit], state);
            e.num(flag_mask.trailing_zeros());
            e.op(0x74);
            e.op(0x72);
        }
    }

    fn position(&self, e: &mut Emitter, snapshot: &ExitState, state: &mut Emission) {
        self.value(e, snapshot.eip, state);
        e.set(PC);
        e.get(self.base + BLOCK_LEFT);
        e.num(snapshot.completed_prefix + self.flow.as_ref().map_or(0, |p| p.prefix()));
        e.op(0x6b);
        e.set(LEFT);
    }

    pub(super) fn emit_exit_snapshot(&self, e: &mut Emitter, context: ExitContext) {
        let start = e.code.len();
        self.snapshot_emissions
            .set(self.snapshot_emissions.get() + 1);
        let snapshot = &self.graph.snapshots[context.snapshot.index()];
        let mut state = self
            .local_recovery
            .borrow()
            .as_ref()
            .map(|(_, s)| s.clone())
            .unwrap_or_default();
        state.exit_only = true;
        for (index, value) in snapshot.cpu.gpr.iter().enumerate() {
            if context.preserve_gprs & (1 << index) == 0 {
                self.value(e, *value, &mut state);
                e.set(3 + index as u32);
            }
        }
        if !context.preserve_flags {
            self.flags(e, &snapshot.cpu.flags, u32::MAX, &mut state);
            e.set(FLAGS);
        }
        if !context.preserve_defined {
            self.value(e, snapshot.cpu.flags.defined, &mut state);
            e.set(DEFINED);
        }
        if !context.preserve_position {
            self.position(e, snapshot, &mut state);
        }
        e.num(0);
        e.set(self.base + READY);
        self.snapshot_bytes
            .set(self.snapshot_bytes.get() + (e.code.len() - start) as u32);
    }

    /// Only the failing branch selects a recipe. Its parameter bank and
    /// completed effect outputs stay in this function's locals; nothing is
    /// stored or recomputed on the successful path for this cold transfer.
    pub(super) fn emit_cold_exit(&self, e: &mut Emitter, context: ExitContext) {
        if self.flow.is_some() {
            if let Some((label, _)) = self.local_recovery.borrow().as_ref() {
                e.branch(label);
            } else {
                self.emit_exit_snapshot(e, context);
                e.value_exit = None;
                e.branch("exit");
                e.value_exit = Some(context);
            }
            return;
        }
        let index = self.cold_exits.borrow_mut().intern(context);
        let start = e.code.len();
        e.num(index);
        e.set(self.base + EXIT_CONTEXT);
        e.branch("value-cold");
        self.cold_exit_sites.set(self.cold_exit_sites.get() + 1);
        self.cold_exit_branch_bytes
            .set(self.cold_exit_branch_bytes.get() + (e.code.len() - start) as u32);
    }

    /// All precision/progress distinctions are part of the key. A shared
    /// recipe uses only pure expressions and already completed effect values;
    /// it cannot repeat a load, store, stack operation, or REP element.
    pub(super) fn emit_cold_exit_bodies(&self, e: &mut Emitter) {
        assert!(
            e.value_exit.is_none(),
            "cold table cannot recursively outline itself"
        );
        let contexts = self.cold_exits.borrow().contexts.clone();
        let start = e.code.len();
        e.open(0x02, "value-cold-invalid".into());
        for index in (0..contexts.len()).rev() {
            e.open(0x02, format!("value-cold-{index}"));
        }
        e.get(self.base + EXIT_CONTEXT);
        e.op(0x0e);
        uleb(&mut e.code, contexts.len() as u32);
        for index in 0..contexts.len() {
            uleb(&mut e.code, index as u32);
        }
        // An impossible compiler-owned selector must not publish stale state.
        uleb(&mut e.code, contexts.len() as u32);
        for context in contexts {
            e.end();
            self.emit_exit_snapshot(e, context);
            e.branch("exit");
        }
        e.end();
        e.op(0x00); // unreachable invalid selector; guest cannot address it.
        self.cold_exit_body_bytes.set((e.code.len() - start) as u32);
    }

    fn address(&self, e: &mut Emitter, address: Address, state: &mut Emission) {
        self.value(e, address.offset, state);
        e.set(ADDR);
        if address.segment == Segment::Fs {
            e.get(ADDR);
            e.num(u32::MAX - (address.width.bytes() - 1));
            e.op(0x4b);
            e.check(6, ADDR, 0);
            e.get(ADDR);
            e.load(0, 44 + 4 * 16 + 4);
            e.op(0x6a);
            e.set(ADDR);
            e.get(ADDR);
            e.num(u32::MAX - (address.width.bytes() - 1));
            e.op(0x4b);
            e.if_();
            e.exit(3);
            e.end();
        }
    }

    fn effect(&self, e: &mut Emitter, id: EffectId, state: &mut Emission) {
        let effect = &self.graph.effects[id.index()];
        e.value_exit = Some(ExitContext::exact(effect.before));
        // Every fault of this ordered effect reaches one local island. There
        // is no function-wide join that keeps all blocks' recovery atoms live.
        if self.flow.is_some() {
            e.open(0x02, format!("flow-effect-done-{}", id.0));
            let label = format!("flow-effect-fault-{}", id.0);
            e.open(0x02, label.clone());
            *self.local_recovery.borrow_mut() = Some((label, state.clone()));
        }
        // Guard evaluation is pure. It never guards the preceding RMW check.
        if let Some(guard) = effect.guard {
            self.value(e, guard, state);
            e.if_();
        }
        match &effect.kind {
            EffectKind::Load { address, access } => {
                self.address(e, *address, state);
                e.access_checked(
                    width(address.width),
                    *access != Access::Read,
                    address.segment == Segment::Ss,
                    *access == Access::ReadWrite,
                );
                e.read_prepared_memory(width(address.width));
                let outputs = &self.outputs[&id];
                assert_eq!(outputs.len(), 1);
                self.output(e, outputs[0].0);
            }
            EffectKind::Store {
                address,
                value,
                checked_by,
            } => {
                if checked_by.is_some() {
                    // Pure value computations cannot change ADDR/ENTRY/PTR2.
                    // The verifier ties this proof to the same instruction.
                    e.dirty_preflight();
                    e.load(ENTRY, 0);
                    e.get(ADDR);
                    e.num(4095);
                    e.op(0x71);
                    e.op(0x6a);
                    e.set(PTR);
                } else {
                    self.address(e, *address, state);
                    e.access(width(address.width), true, address.segment == Segment::Ss);
                }
                self.value(e, *value, state);
                e.set(R);
                e.write_memory(width(address.width));
            }
            EffectKind::Opaque {
                instruction,
                inputs,
                contract,
            } => {
                self.opaque_calls.set(self.opaque_calls.get() + 1);
                for index in 0..8 {
                    if contract.gpr_read & (1 << index) != 0 {
                        self.value(e, inputs.gpr[index], state);
                        e.set(3 + index as u32);
                    }
                }
                let partial = matches!(instruction.opcode, Opcode::String(_, _));
                let scan = matches!(
                    instruction.opcode,
                    Opcode::String(StringOpcode::Scas | StringOpcode::Cmps, _)
                );
                let flags_read = if scan { u32::MAX } else { contract.flags_read };
                if flags_read != 0 {
                    e.get(FLAGS);
                    e.num(!flags_read);
                    e.op(0x71);
                    self.flags(e, &inputs.flags, flags_read, state);
                    e.op(0x72);
                    e.set(FLAGS);
                }
                self.value(e, inputs.flags.defined, state);
                e.set(DEFINED);
                self.position(e, &self.graph.snapshots[effect.before.index()], state);
                if partial {
                    e.value_exit = Some(ExitContext {
                        snapshot: effect.before,
                        preserve_gprs: contract.gpr_written,
                        preserve_flags: scan,
                        preserve_defined: scan,
                        preserve_position: true,
                    });
                }
                e.value_adapter = true;
                e.instruction(instruction, Some(instruction.flags_written));
                e.value_adapter = false;
                if let Some(outputs) = self.outputs.get(&id) {
                    for &(value, output) in outputs {
                        e.get(match output {
                            EffectOutput::Gpr(index) => 3 + u32::from(index),
                            EffectOutput::PackedFlags => FLAGS,
                            EffectOutput::DefinedFlags => DEFINED,
                            EffectOutput::Loaded => unreachable!("opaque load result"),
                        });
                        self.output(e, value);
                    }
                }
            }
        }
        let context = e.value_exit.take();
        if effect.guard.is_some() {
            e.end();
        }
        if self.flow.is_some() {
            e.branch(&format!("flow-effect-done-{}", id.0));
            e.end();
            self.emit_exit_snapshot(e, context.expect("precise effect recovery"));
            e.branch("exit");
            e.end();
            *self.local_recovery.borrow_mut() = None;
        }
        // Computing inside a conditional store must not make a value appear
        // available on the count-zero path. Effect outputs are unconditional.
        if effect.guard.is_some() {
            state.values.clear();
            state.carriers.clear();
            state.predicates.clear();
            state.flow_values.clear();
        }
    }

    fn output(&self, e: &mut Emitter, id: ValueId) {
        if let Some(plan) = &self.flow {
            let value = plan.graph.resolve(plan.graph.value_at(plan.current(), id));
            if let Some(local) = plan.storage[value.0] {
                e.set(local);
            } else {
                e.op(0x1a);
            }
            return;
        }
        let local = self.value_locals[id.index()];
        if local == u32::MAX {
            e.op(0x1a);
        } else {
            e.set(local);
        }
    }

    fn initialize_entry(&self, e: &mut Emitter) {
        if self.flow.is_some() {
            e.get(LEFT);
            e.set(self.base + BLOCK_LEFT);
            return;
        }
        e.get(self.base + READY);
        e.op(0x45);
        e.if_();
        for index in 0..8 {
            e.get(3 + index);
            e.set(self.base + index);
        }
        e.get(FLAGS);
        e.num(!ARITHMETIC_MASK);
        e.op(0x71);
        e.set(self.base + 8);
        e.get(DEFINED);
        e.set(self.base + 9);
        for (bit, mask) in FLAG_MASKS.iter().enumerate() {
            if self.predicate.is_some() {
                e.get(FLAGS);
                e.num(mask.trailing_zeros());
                e.op(0x76);
                e.num(1);
                e.op(0x71);
                match bit {
                    1 | 3 => e.op(0x45),
                    2 => {
                        e.num(4);
                        e.op(0x74);
                    }
                    4 | 5 => {
                        e.num(31);
                        e.op(0x74);
                    }
                    _ => {}
                }
                e.set(self.base + 10 + bit as u32);
                continue;
            }
            let first = self.base + 10 + bit as u32 * 5;
            e.num(tag(bit as u8, FlagKind::Materialized, ScalarWidth::W32));
            e.set(first);
            e.get(FLAGS);
            e.num(mask.trailing_zeros());
            e.op(0x76);
            e.num(1);
            e.op(0x71);
            e.set(first + 1);
            for component in 2..5 {
                e.num(0);
                e.set(first + component);
            }
        }
        e.end();
        e.num(0);
        e.set(self.base + READY);
        e.get(LEFT);
        e.set(self.base + BLOCK_LEFT);
    }

    fn edge(&self, e: &mut Emitter, target: Target, snapshot: SnapshotId, state: &mut Emission) {
        let snapshot_value = &self.graph.snapshots[snapshot.index()];
        if let (Some(plan), Target::Block(block)) = (&self.flow, target) {
            let from = plan.current();
            let target = plan.graph.targets[from.0][&block];
            plan.transfer(e, from, target, state);
            e.get(self.base + BLOCK_LEFT);
            e.num(snapshot_value.completed_prefix + self.flow.as_ref().map_or(0, |p| p.prefix()));
            e.op(0x6b);
            e.set(LEFT);
            self.flow_jump(e, target);
            return;
        }
        if let Target::Block(block) = target {
            // Keep all sources live until every destination has been pushed.
            // This also handles self loops and arbitrary register permutations.
            for value in snapshot_value.cpu.gpr {
                self.value(e, value, state);
            }
            self.value(e, snapshot_value.cpu.flags.other, state);
            self.value(e, snapshot_value.cpu.flags.defined, state);
            for flag in snapshot_value.cpu.flags.arithmetic {
                if let Some(plan) = &self.predicate {
                    self.predicate_value(e, plan.graph.payloads[flag.index()], state);
                } else {
                    for component in 0..5 {
                        self.carrier(e, flag, component, state);
                    }
                }
            }
            // Position may depend on parameters that are about to be replaced.
            self.value(e, snapshot_value.eip, state);
            e.set(PC);
            for local in (0..if self.predicate.is_some() {
                16
            } else {
                PARAMETER_WORDS
            })
                .rev()
            {
                e.set(self.base + local);
            }
            e.get(self.base + BLOCK_LEFT);
            e.num(snapshot_value.completed_prefix + self.flow.as_ref().map_or(0, |p| p.prefix()));
            e.op(0x6b);
            e.set(LEFT);
            e.num(1);
            e.set(self.base + READY);
            e.jump(self.graph.blocks[block.index()].start);
        } else {
            self.emit_exit_snapshot(e, ExitContext::exact(snapshot));
            if let Target::External(pc) = target {
                e.num(pc);
                e.set(PC);
            }
            e.exit(2);
        }
    }

    fn emit_effects(&self, e: &mut Emitter, block: &v::Block, state: &mut Emission) {
        for step in &block.steps {
            for effect in &step.effects {
                self.effect(e, *effect, state);
            }
            if step.effects.iter().any(|id| {
                matches!(
                    self.graph.effects[id.index()].kind,
                    EffectKind::Store { .. } | EffectKind::Opaque { .. }
                )
            }) {
                e.get(SMC);
                e.if_();
                e.value_exit = Some(ExitContext::exact(step.after));
                e.exit(7);
                e.value_exit = None;
                e.end();
            }
        }
    }
    fn emit_terminator(&self, e: &mut Emitter, block: &v::Block, state: &mut Emission) {
        match block.terminator {
            Terminator::Direct {
                target,
                state: snapshot,
            } => self.edge(e, target, snapshot, state),
            Terminator::Conditional {
                condition,
                taken,
                fallthrough,
                state: snapshot,
            } => {
                self.value(e, condition, state);
                e.if_();
                let saved_values = state.values.clone();
                let saved_carriers = state.carriers.clone();
                let saved_predicates = state.predicates.clone();
                let saved_flow = state.flow_values.clone();
                self.edge(e, taken, snapshot, state);
                e.op(0x05);
                state.values = saved_values;
                state.carriers = saved_carriers;
                state.predicates = saved_predicates;
                state.flow_values = saved_flow;
                self.edge(e, fallthrough, snapshot, state);
                e.end();
            }
            Terminator::Indirect {
                target,
                state: snapshot,
            } => {
                if let Some(plan) = &self.flow {
                    let snapshot = &self.graph.snapshots[snapshot.index()];
                    self.value(e, target, state);
                    e.set(PC);
                    plan.transfer(e, plan.current(), crate::flow_ir::INGRESS, state);
                    e.get(self.base + BLOCK_LEFT);
                    e.num(snapshot.completed_prefix + self.flow.as_ref().map_or(0, |p| p.prefix()));
                    e.op(0x6b);
                    e.set(LEFT);
                    e.num(1);
                    e.set(self.base + READY);
                    e.branch("lookup");
                } else if let Some(plan) = &self.predicate {
                    let snapshot = &self.graph.snapshots[snapshot.index()];
                    // Evaluate target before replacing any incoming parameter.
                    self.value(e, target, state);
                    e.set(PC);
                    for value in snapshot.cpu.gpr {
                        self.value(e, value, state);
                    }
                    self.value(e, snapshot.cpu.flags.other, state);
                    self.value(e, snapshot.cpu.flags.defined, state);
                    for flag in snapshot.cpu.flags.arithmetic {
                        self.predicate_value(e, plan.graph.payloads[flag.index()], state);
                    }
                    for local in (0..16).rev() {
                        e.set(self.base + local);
                    }
                    e.get(self.base + BLOCK_LEFT);
                    e.num(snapshot.completed_prefix + self.flow.as_ref().map_or(0, |p| p.prefix()));
                    e.op(0x6b);
                    e.set(LEFT);
                    e.num(1);
                    e.set(self.base + READY);
                    e.indirect();
                } else {
                    self.emit_exit_snapshot(e, ExitContext::exact(snapshot));
                    // Snapshot packing does not alter the incoming parameter bank.
                    self.value(e, target, state);
                    e.set(PC);
                    e.indirect();
                }
            }
            Terminator::Stop { .. } => {
                e.exit(3);
            }
        }
    }
    pub(super) fn emit_block(&self, e: &mut Emitter, old: &crate::ir::Block) {
        let block = &self.graph.blocks[self.graph.entries[&old.start].index()];
        self.initialize_entry(e);
        e.get(LEFT);
        e.num(block.steps.len() as u32);
        e.op(0x4f);
        e.if_();
        let mut state = Emission::default();
        self.emit_effects(e, block, &mut state);
        self.emit_terminator(e, block, &mut state);
        e.op(0x05);
        self.emit_exit_snapshot(e, ExitContext::exact(block.steps[0].before));
        if self.flow.is_some() {
            // The SSA frame is dead after the exact architectural snapshot.
            // All static versions share this source block's partial executor.
            e.num(block.id.0);
            e.set(ROUTE);
            e.branch("flow-partial");
            e.end();
            return;
        }
        // A partial budget keeps the established instruction-by-instruction
        // retirement/fault path. It is a cold boundary, never a fake retirement.
        for instruction in &old.instructions {
            e.instruction(instruction, None);
        }
        e.block_fallthrough(old);
        e.end();
    }
}

/// Five i32 parameters (tag,a,b,carry,unused), one boolean i32 result.
/// This pure function cannot inspect or mutate CPU/memory/runtime authority.
pub(super) fn flag_function_body() -> Vec<u8> {
    let mut b = FlagEncoder { code: Vec::new() };
    b.u(2);
    b.u(10);
    b.op(0x7f);
    b.u(1);
    b.op(0x7e);
    // Locals:5kind,6bits,7mask,8result,9CF,10OF,11AF,12count,
    // 13signed,14width-truncated right,15wide(i64).
    b.get(0);
    b.n(3);
    b.op(0x76);
    b.n(7);
    b.op(0x71);
    b.set(5);
    b.get(5);
    b.op(0x45);
    b.if_();
    b.get(1);
    b.n(1);
    b.op(0x71);
    b.op(0x0f);
    b.end();
    b.n(8);
    b.get(0);
    b.n(6);
    b.op(0x76);
    b.n(3);
    b.op(0x71);
    b.op(0x74);
    b.set(6);
    b.n(u32::MAX);
    b.n(32);
    b.get(6);
    b.op(0x6b);
    b.op(0x76);
    b.set(7);
    b.get(1);
    b.get(7);
    b.op(0x71);
    b.set(1);
    b.get(2);
    b.get(7);
    b.op(0x71);
    b.set(14);
    b.get(5);
    b.n(1);
    b.op(0x46);
    b.if_();
    b.get(1);
    b.op(0xad);
    b.get(14);
    b.op(0xad);
    b.op(0x7c);
    b.get(3);
    b.n(1);
    b.op(0x71);
    b.op(0xad);
    b.op(0x7c);
    b.set(15);
    b.get(15);
    b.op(0xa7);
    b.get(7);
    b.op(0x71);
    b.set(8);
    b.get(15);
    b.get(7);
    b.op(0xad);
    b.op(0x56);
    b.set(9);
    b.get(1);
    b.get(14);
    b.op(0x73);
    b.n(u32::MAX);
    b.op(0x73);
    b.get(1);
    b.get(8);
    b.op(0x73);
    b.op(0x71);
    b.sign_bit();
    b.set(10);
    b.af();
    b.end();
    b.get(5);
    b.n(2);
    b.op(0x46);
    b.if_();
    b.get(14);
    b.op(0xad);
    b.get(3);
    b.n(1);
    b.op(0x71);
    b.op(0xad);
    b.op(0x7c);
    b.set(15);
    b.get(1);
    b.get(15);
    b.op(0xa7);
    b.op(0x6b);
    b.get(7);
    b.op(0x71);
    b.set(8);
    b.get(1);
    b.op(0xad);
    b.get(15);
    b.op(0x54);
    b.set(9);
    b.get(1);
    b.get(14);
    b.op(0x73);
    b.get(1);
    b.get(8);
    b.op(0x73);
    b.op(0x71);
    b.sign_bit();
    b.set(10);
    b.af();
    b.end();
    b.get(5);
    b.n(3);
    b.op(0x46);
    b.if_();
    b.get(1);
    b.set(8);
    b.end();
    b.get(5);
    b.n(4);
    b.op(0x4f);
    b.if_();
    b.get(2);
    b.n(31);
    b.op(0x71);
    b.set(12);
    b.get(1);
    b.n(32);
    b.get(6);
    b.op(0x6b);
    b.op(0x74);
    b.n(32);
    b.get(6);
    b.op(0x6b);
    b.op(0x75);
    b.set(13);
    b.get(5);
    b.n(4);
    b.op(0x46);
    b.if_();
    b.get(1);
    b.get(12);
    b.op(0x74);
    b.get(7);
    b.op(0x71);
    b.set(8);
    b.get(12);
    b.n(0);
    b.op(0x47);
    b.get(12);
    b.get(6);
    b.op(0x4d);
    b.op(0x71);
    b.if_();
    b.get(1);
    b.get(6);
    b.get(12);
    b.op(0x6b);
    b.op(0x76);
    b.n(1);
    b.op(0x71);
    b.set(9);
    b.end();
    b.get(8);
    b.sign_bit();
    b.get(9);
    b.op(0x73);
    b.set(10);
    b.op(0x05);
    b.get(5);
    b.n(5);
    b.op(0x46);
    b.if_();
    b.get(1);
    b.get(12);
    b.op(0x76);
    b.set(8);
    b.get(1);
    b.sign_bit();
    b.set(10);
    b.op(0x05);
    b.get(13);
    b.get(12);
    b.op(0x75);
    b.get(7);
    b.op(0x71);
    b.set(8);
    b.end();
    b.get(12);
    b.n(0);
    b.op(0x47);
    b.if_();
    b.get(12);
    b.get(6);
    b.op(0x4d);
    b.if_();
    b.get(1);
    b.get(12);
    b.n(1);
    b.op(0x6b);
    b.op(0x76);
    b.n(1);
    b.op(0x71);
    b.set(9);
    b.op(0x05);
    b.get(5);
    b.n(6);
    b.op(0x46);
    b.if_();
    b.get(1);
    b.sign_bit();
    b.set(9);
    b.end();
    b.end();
    b.end();
    b.end();
    b.end();
    b.get(0);
    b.n(7);
    b.op(0x71);
    b.set(0);
    for (bit, local) in [(0, 9), (2, 11), (5, 10)] {
        b.get(0);
        b.n(bit);
        b.op(0x46);
        b.if_();
        b.get(local);
        b.op(0x0f);
        b.end();
    }
    b.get(0);
    b.n(1);
    b.op(0x46);
    b.if_();
    b.get(8);
    b.n(255);
    b.op(0x71);
    b.op(0x69);
    b.n(1);
    b.op(0x71);
    b.op(0x45);
    b.op(0x0f);
    b.end();
    b.get(0);
    b.n(3);
    b.op(0x46);
    b.if_();
    b.get(8);
    b.op(0x45);
    b.op(0x0f);
    b.end();
    b.get(8);
    b.sign_bit();
    b.end();
    b.code
}

struct FlagEncoder {
    code: Vec<u8>,
}
impl FlagEncoder {
    fn op(&mut self, opcode: u8) {
        self.code.push(opcode);
    }
    fn u(&mut self, value: u32) {
        uleb(&mut self.code, value);
    }
    fn n(&mut self, value: u32) {
        self.op(0x41);
        sleb(&mut self.code, i64::from(value as i32));
    }
    fn get(&mut self, local: u32) {
        self.op(0x20);
        self.u(local);
    }
    fn set(&mut self, local: u32) {
        self.op(0x21);
        self.u(local);
    }
    fn if_(&mut self) {
        self.op(0x04);
        self.op(0x40);
    }
    fn end(&mut self) {
        self.op(0x0b);
    }
    fn sign_bit(&mut self) {
        self.get(6);
        self.n(1);
        self.op(0x6b);
        self.op(0x76);
        self.n(1);
        self.op(0x71);
    }
    fn af(&mut self) {
        self.get(1);
        self.get(14);
        self.op(0x73);
        self.get(8);
        self.op(0x73);
        self.n(4);
        self.op(0x76);
        self.n(1);
        self.op(0x71);
        self.set(11);
    }
}

#[cfg(test)]
mod cold_exit_tests {
    use super::*;

    #[test]
    fn recovery_boundaries_share_only_when_all_progress_is_identical() {
        let before = ExitContext::exact(SnapshotId(0));
        let mut registry = ColdExits::default();
        let initial = registry.intern(before);
        // Span, either page and journal exhaustion can all leave this same
        // pre-instruction state; their kind/address/error stay separate locals.
        assert_eq!(registry.intern(before), initial);
        let variants = [
            ExitContext::exact(SnapshotId(1)), // Completed store / SMC.
            ExitContext {
                preserve_gprs: 1 << 1, // REP count progress.
                ..before
            },
            ExitContext {
                preserve_gprs: 1 << 7, // String destination progress.
                ..before
            },
            ExitContext {
                preserve_flags: true, // SCAS fault finalizer owns flags.
                ..before
            },
            ExitContext {
                preserve_defined: true,
                ..before
            },
            ExitContext {
                preserve_position: true, // REP remains unretired.
                ..before
            },
        ];
        let mut identities = BTreeSet::from([initial]);
        for context in variants {
            let index = registry.intern(context);
            assert!(identities.insert(index));
            assert_eq!(registry.intern(context), index);
        }
        assert_eq!(registry.contexts.len(), identities.len());
    }
}

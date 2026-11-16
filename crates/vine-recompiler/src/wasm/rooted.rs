//! Single-root normal CFG execution. Architectural state crosses only actual
//! external/indirect boundaries; SSA phis and static producer demands govern
//! normal edges. Checked memory and complex instruction semantics are shared.
use super::*;
use crate::rooted_ir::{self as r, Graph, Input, Op, ValueId};
use crate::value_ir::{
    self as v, Access, Address, BlockId, EffectId, EffectKind, EffectOutput, ExitState, FlagKind,
    Flags, ScalarWidth, SnapshotId, Target, Terminator,
};
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet};
mod allocate;
mod control;

const BLOCK_LEFT: u32 = 0;
const FIRST_ATOM: u32 = 1;
// Match the supported V8 Wasm implementation limits, including the public
// compiler's larger instruction limit. Exceeding them is a compile error.
const MAX_FUNCTION_PARAMETERS: usize = 1000;
const MAX_FUNCTION_LOCALS: u32 = 50_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct ExitContext {
    snapshot: SnapshotId,
    preserve_gprs: u8,
    preserve_flags: bool,
    preserve_defined: bool,
    preserve_position: bool,
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
#[derive(Default)]
struct ColdExits {
    indices: BTreeMap<ExitContext, u32>,
    functions: Vec<SnapshotFunction>,
}
#[derive(Clone)]
struct SnapshotFunction {
    context: ExitContext,
    atoms: Vec<ValueId>,
    destinations: Vec<u32>,
}
pub(super) struct SnapshotBody {
    pub(super) parameters: u32,
    pub(super) results: u32,
    pub(super) code: Vec<u8>,
}
#[derive(Clone, Default)]
struct Emission {
    values: BTreeMap<ValueId, u32>,
    next_temp: u32,
    exit_only: bool,
    // Present only in a pure snapshot function. Main-function colors must
    // never be referenced from its separate parameter/local namespace.
    atoms: Option<BTreeMap<ValueId, u32>>,
    temporary_base: Option<u32>,
}
#[derive(Clone)]
struct Transfer {
    label: String,
    route: Option<u32>,
}
#[derive(Default)]
struct Control {
    forward: BTreeMap<BlockId, Transfer>,
    backward: BTreeMap<BlockId, Transfer>,
    next_label: u32,
}

pub(super) struct Plan {
    graph: Graph,
    allocation: allocate::Allocation,
    base: u32,
    temporary_base: u32,
    temporary_count: u32,
    temporary_peak: Cell<u32>,
    first_snapshot_function: u32,
    outputs: BTreeMap<EffectId, Vec<(ValueId, EffectOutput)>>,
    normal_evaluations: Cell<u32>,
    exit_evaluations: Cell<u32>,
    phi_copies: Cell<u32>,
    eliminated_copies: Cell<u32>,
    direct_branches: Cell<u32>,
    routed_branches: Cell<u32>,
    structured_loops: Cell<u32>,
    irreducible_components: Cell<u32>,
    opaque_calls: Cell<u32>,
    cold_exits: RefCell<ColdExits>,
    cold_exit_sites: Cell<u32>,
    snapshot_bytes: Cell<u32>,
    snapshot_call_bytes: Cell<u32>,
    inline_snapshot_bytes: Cell<u32>,
    snapshot_peak_locals: Cell<u32>,
}
fn width(width: ScalarWidth) -> Width {
    match width {
        ScalarWidth::W8 => Width::W8,
        ScalarWidth::W16 => Width::W16,
        ScalarWidth::W32 => Width::W32,
    }
}
fn binary_opcode(op: r::BinaryOp) -> u8 {
    use r::BinaryOp::*;
    match op {
        Add => 0x6a,
        Sub => 0x6b,
        Mul => 0x6c,
        And => 0x71,
        Or => 0x72,
        Xor => 0x73,
        Shl => 0x74,
        ShrS => 0x75,
        ShrU => 0x76,
        Eq => 0x46,
        Ne => 0x47,
        LtS => 0x48,
        LtU => 0x49,
        LeS => 0x4c,
        LeU => 0x4d,
    }
}
impl Plan {
    pub(super) fn new(
        region: &Region,
        base: u32,
        first_snapshot_function: u32,
    ) -> Result<Self, String> {
        let graph = r::lift(region).map_err(|error| error.to_string())?;
        let allocation = allocate::allocate(&graph);
        let temporary_base = base + FIRST_ATOM + allocation.count;
        // A recipe never evaluates more pure nodes than exist in this bounded
        // unit. Only used temporaries receive instructions; they are separate
        // from atomic storage even on cold recovery paths.
        let temporary_count = graph
            .values
            .iter()
            .filter(|node| {
                matches!(
                    node.op,
                    Op::Binary { .. } | Op::Select { .. } | Op::Flag { .. }
                )
            })
            .count() as u32;
        let mut outputs: BTreeMap<EffectId, Vec<_>> = BTreeMap::new();
        for (index, node) in graph.values.iter().enumerate() {
            if let Op::EffectResult { effect, output } = node.op {
                let id = ValueId(index as u32);
                if allocation.required.contains(&id) {
                    outputs.entry(effect).or_default().push((id, output));
                }
            }
        }
        Ok(Self {
            graph,
            allocation,
            base,
            temporary_base,
            temporary_count,
            temporary_peak: Cell::new(0),
            first_snapshot_function,
            outputs,
            normal_evaluations: Cell::new(0),
            exit_evaluations: Cell::new(0),
            phi_copies: Cell::new(0),
            eliminated_copies: Cell::new(0),
            direct_branches: Cell::new(0),
            routed_branches: Cell::new(0),
            structured_loops: Cell::new(0),
            irreducible_components: Cell::new(0),
            opaque_calls: Cell::new(0),
            cold_exits: RefCell::default(),
            cold_exit_sites: Cell::new(0),
            snapshot_bytes: Cell::new(0),
            snapshot_call_bytes: Cell::new(0),
            inline_snapshot_bytes: Cell::new(0),
            snapshot_peak_locals: Cell::new(0),
        })
    }
    pub(super) fn local_count(&self) -> u32 {
        FIRST_ATOM + self.allocation.count + self.temporary_peak.get()
    }
    pub(super) fn metadata(&self) -> Vec<u8> {
        format!(concat!(
            "{{\"version\":1,\"entryPolicy\":\"single-root\",\"blocks\":{},\"discoveredBlocks\":{},\"emittedInstructions\":{},",
            "\"atomicValues\":{},\"atomicLocals\":{},\"temporaryLocals\":{},\"peakLogicalLive\":{},",
            "\"coalescedAtoms\":{},\"phisSimplified\":{},\"valuesShared\":{},\"producerFlagParameters\":{},\"booleanFlagParameters\":{},",
            "\"normalFlagEvaluations\":{},\"exitFlagEvaluations\":{},\"phiCopies\":{},\"eliminatedPhiCopies\":{},",
            "\"directBranches\":{},\"routedBranches\":{},\"structuredLoops\":{},\"irreducibleComponents\":{},",
            "\"opaqueAdapters\":{},\"coldExitSites\":{},\"coldSnapshots\":{},\"snapshotBytes\":{},",
            "\"snapshotCallBytes\":{},\"inlineSnapshotBytes\":{},\"snapshotPeakLocals\":{},\"coldTableEntries\":0,\"snapshotStorage\":\"pure-functions\",",
            "\"flagCarrierWords\":0,\"memoryChecks\":\"existing-precise\"}}"),
            self.graph.reachable.len(), self.graph.source.blocks.len(), self.graph.reachable.iter().map(|id| self.graph.source.blocks[id.index()].steps.len()).sum::<usize>(), self.allocation.atomic_values, self.allocation.count, self.temporary_peak.get(), self.allocation.peak_live,
            self.allocation.coalesced, self.graph.stats.phis_simplified, self.graph.stats.values_shared, self.graph.stats.producer_flag_parameters, self.graph.stats.boolean_flag_parameters,
            self.normal_evaluations.get(), self.exit_evaluations.get(), self.phi_copies.get(), self.eliminated_copies.get(),
            self.direct_branches.get(), self.routed_branches.get(), self.structured_loops.get(), self.irreducible_components.get(),
            self.opaque_calls.get(), self.cold_exit_sites.get(), self.cold_exits.borrow().functions.len(), self.snapshot_bytes.get(), self.snapshot_call_bytes.get(), self.inline_snapshot_bytes.get(), self.snapshot_peak_locals.get()).into_bytes()
    }
    fn atom_local(&self, id: ValueId) -> u32 {
        self.base
            + FIRST_ATOM
            + self.allocation.colors[self.graph.resolve(id).index()]
                .expect("demanded SSA atom has a color")
    }
    fn source_value(&self, e: &mut Emitter, id: v::ValueId, state: &mut Emission) {
        self.value(e, self.graph.mapped_value(id), state);
    }
    fn value(&self, e: &mut Emitter, id: ValueId, state: &mut Emission) {
        let id = self.graph.resolve(id);
        let op = &self.graph.values[id.index()].op;
        match *op {
            Op::Constant(value) => {
                e.num(value);
                return;
            }
            Op::Input(_) | Op::Phi { .. } | Op::EffectResult { .. } => {
                e.get(match &state.atoms {
                    Some(atoms) => *atoms
                        .get(&id)
                        .expect("snapshot atom is an explicit parameter"),
                    None => self.atom_local(id),
                });
                return;
            }
            Op::Unreachable => unreachable!("unreachable rooted value requested"),
            _ => {}
        }
        if let Some(&local) = state.values.get(&id) {
            e.get(local);
            return;
        }
        match *op {
            Op::Binary { op, left, right } => {
                self.value(e, left, state);
                self.value(e, right, state);
                e.op(binary_opcode(op));
            }
            Op::Select { condition, yes, no } => {
                self.value(e, yes, state);
                self.value(e, no, state);
                self.value(e, condition, state);
                e.op(0x1b);
            }
            Op::Flag {
                bit,
                kind,
                width,
                args,
            } => {
                let counter = if state.exit_only {
                    &self.exit_evaluations
                } else {
                    &self.normal_evaluations
                };
                counter.set(counter.get() + 1);
                self.leaf(e, bit, kind, width, args, state);
            }
            _ => unreachable!("resolved pure node"),
        }
        assert!(state.next_temp < self.temporary_count);
        let local = state.temporary_base.unwrap_or(self.temporary_base) + state.next_temp;
        state.next_temp += 1;
        if state.atoms.is_none() {
            self.temporary_peak
                .set(self.temporary_peak.get().max(state.next_temp));
        }
        state.values.insert(id, local);
        e.tee(local);
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
        // Static recipes omit the runtime tag switch. Incoming dynamic flag
        // values are explicit SSA atoms, including heterogeneous producer phis.
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

    fn flags(&self, e: &mut Emitter, flags: &Flags, mask: u32, state: &mut Emission) {
        if mask & !v::ARITHMETIC_MASK != 0 {
            self.source_value(e, flags.other, state);
            e.num(mask & !v::ARITHMETIC_MASK);
            e.op(0x71);
        } else {
            e.num(0);
        }
        for (bit, flag_mask) in v::FLAG_MASKS.iter().enumerate() {
            if mask & flag_mask == 0 {
                continue;
            }
            self.value(e, self.graph.mapped_flag(flags.arithmetic[bit]), state);
            e.num(flag_mask.trailing_zeros());
            e.op(0x74);
            e.op(0x72);
        }
    }

    fn position(&self, e: &mut Emitter, snapshot: &ExitState, state: &mut Emission) {
        self.source_value(e, snapshot.eip, state);
        e.set(PC);
        e.get(self.base + BLOCK_LEFT);
        e.num(snapshot.completed_prefix);
        e.op(0x6b);
        e.set(LEFT);
    }

    fn snapshot_function(&self, context: ExitContext) -> (u32, SnapshotFunction) {
        if let Some(&index) = self.cold_exits.borrow().indices.get(&context) {
            return (
                index,
                self.cold_exits.borrow().functions[index as usize].clone(),
            );
        }
        let snapshot = &self.graph.source.snapshots[context.snapshot.index()];
        let mut roots = Vec::new();
        let mut destinations = Vec::new();
        for (index, value) in snapshot.cpu.gpr.iter().enumerate() {
            if context.preserve_gprs & (1 << index) == 0 {
                roots.push(self.graph.mapped_value(*value));
                destinations.push(3 + index as u32);
            }
        }
        if !context.preserve_flags {
            roots.push(self.graph.mapped_value(snapshot.cpu.flags.other));
            roots.extend(
                snapshot
                    .cpu
                    .flags
                    .arithmetic
                    .map(|flag| self.graph.mapped_flag(flag)),
            );
            destinations.push(FLAGS);
        }
        if !context.preserve_defined {
            roots.push(self.graph.mapped_value(snapshot.cpu.flags.defined));
            destinations.push(DEFINED);
        }
        if !context.preserve_position {
            roots.push(self.graph.mapped_value(snapshot.eip));
            destinations.extend([PC, LEFT]);
        }
        let mut atoms = BTreeSet::new();
        for root in roots {
            allocate::atom_dependencies(&self.graph, root, &mut atoms);
        }
        let function = SnapshotFunction {
            context,
            atoms: atoms.into_iter().collect(),
            destinations,
        };
        let mut exits = self.cold_exits.borrow_mut();
        let index = exits.functions.len() as u32;
        exits.indices.insert(context, index);
        exits.functions.push(function.clone());
        (index, function)
    }

    /// Arguments are values already live at the logical fault/exit point.
    /// Results touch only the non-preserved architectural fields. There is no
    /// common cold local merge and no successful-path snapshot preparation.
    fn emit_snapshot_call(&self, e: &mut Emitter, context: ExitContext) {
        let start = e.code.len();
        let (index, function) = self.snapshot_function(context);
        for atom in function.atoms {
            e.get(self.atom_local(atom));
        }
        if !context.preserve_position {
            e.get(self.base + BLOCK_LEFT);
        }
        e.op(0x10);
        uleb(&mut e.code, self.first_snapshot_function + index);
        for destination in function.destinations.into_iter().rev() {
            e.set(destination);
        }
        self.snapshot_call_bytes
            .set(self.snapshot_call_bytes.get() + (e.code.len() - start) as u32);
    }

    /// Normal external/indirect transitions and partial-budget entry keep
    /// their existing inline restoration. Outlining failure paths must not
    /// add a function call to a successful region transfer.
    pub(super) fn emit_exit_snapshot(&self, e: &mut Emitter, context: ExitContext) {
        let start = e.code.len();
        let snapshot = &self.graph.source.snapshots[context.snapshot.index()];
        let mut state = Emission {
            exit_only: true,
            ..Emission::default()
        };
        for (index, value) in snapshot.cpu.gpr.iter().enumerate() {
            if context.preserve_gprs & (1 << index) == 0 {
                self.source_value(e, *value, &mut state);
                e.set(3 + index as u32);
            }
        }
        if !context.preserve_flags {
            self.flags(e, &snapshot.cpu.flags, u32::MAX, &mut state);
            e.set(FLAGS);
        }
        if !context.preserve_defined {
            self.source_value(e, snapshot.cpu.flags.defined, &mut state);
            e.set(DEFINED);
        }
        if !context.preserve_position {
            self.position(e, snapshot, &mut state);
        }
        self.inline_snapshot_bytes
            .set(self.inline_snapshot_bytes.get() + (e.code.len() - start) as u32);
    }

    pub(super) fn emit_cold_exit(&self, e: &mut Emitter, context: ExitContext) {
        self.emit_snapshot_call(e, context);
        // The exit hook is lexical emission state, so restore it for the next
        // branch in this instruction after emitting this terminal transfer.
        let pending = e.rooted_exit.take();
        e.branch("exit");
        e.rooted_exit = pending;
        self.cold_exit_sites.set(self.cold_exit_sites.get() + 1);
    }

    /// Only pure SSA arithmetic is emitted here: inputs, phis, and completed
    /// effects are explicit parameters. These functions cannot access guest
    /// memory, imports, globals, FP state, or runtime state. The main function
    /// retains fault metadata and owns its existing vector/FP final flush.
    pub(super) fn snapshot_bodies(&self) -> Result<Vec<SnapshotBody>, String> {
        if self.base + self.local_count() > MAX_FUNCTION_LOCALS {
            return Err("rooted SSA main function exceeds Wasm local limit".into());
        }
        let mut bodies = Vec::new();
        for function in &self.cold_exits.borrow().functions {
            let context = function.context;
            let parameters = function.atoms.len() + usize::from(!context.preserve_position);
            if parameters > MAX_FUNCTION_PARAMETERS {
                return Err(format!(
                    "rooted SSA snapshot requires {parameters} Wasm parameters (limit {MAX_FUNCTION_PARAMETERS})"
                ));
            }
            let mut e = Emitter {
                local_map: None,
                function_cfg: None,
                scope_plan: None,
                continuations: None,
                code: Vec::new(),
                labels: Vec::new(),
                blocks: BTreeMap::new(),
                structured_header: None,
                structured_body: Vec::new(),
                affine: Vec::new(),
                loop_base: 0,
                loop_context: None,
                loop_instruction: 0,
                division_scratch: 0,
                string_scratch: 0,
                rep_fuel: 0,
                link_enabled: 0,
                link_target: 0,
                has_mmx: false,
                fp_base: 0,
                fp_plan: BTreeMap::new(),
                fp_calls: BTreeMap::new(),
                fp_leaf: false,
                typed_numeric: false,
                sse_numeric: [0; 10],
                scas_import: 0,
                unit_id: 0,
                value: None,
                value_exit: None,
                value_adapter: false,
                coalesce_retirement: false,
                retirement: None,
                retirement_exits: BTreeMap::new(),
                flag_plan: BTreeMap::new(),
                flag_admission: BTreeMap::new(),
                flag_saved: [u32::MAX; 4],
                flow_partial: false,
                rooted: None,
                rooted_exit: None,
                sampled_pc: false,
                binding_global: false,
                direct_links: false,
                dispatches: std::rc::Rc::default(),
            };
            let mut state = Emission {
                exit_only: true,
                atoms: Some(
                    function
                        .atoms
                        .iter()
                        .enumerate()
                        .map(|(index, &id)| (id, index as u32))
                        .collect(),
                ),
                temporary_base: Some(parameters as u32),
                ..Emission::default()
            };
            let snapshot = &self.graph.source.snapshots[context.snapshot.index()];
            for (index, value) in snapshot.cpu.gpr.iter().enumerate() {
                if context.preserve_gprs & (1 << index) == 0 {
                    self.source_value(&mut e, *value, &mut state);
                }
            }
            if !context.preserve_flags {
                self.flags(&mut e, &snapshot.cpu.flags, u32::MAX, &mut state);
            }
            if !context.preserve_defined {
                self.source_value(&mut e, snapshot.cpu.flags.defined, &mut state);
            }
            if !context.preserve_position {
                self.source_value(&mut e, snapshot.eip, &mut state);
                e.get(function.atoms.len() as u32); // BLOCK_LEFT parameter
                e.num(snapshot.completed_prefix);
                e.op(0x6b);
            }
            e.op(0x0b);
            if parameters as u32 + state.next_temp > MAX_FUNCTION_LOCALS {
                return Err("rooted SSA snapshot function exceeds Wasm local limit".into());
            }
            let mut code = Vec::new();
            uleb(&mut code, u32::from(state.next_temp != 0));
            if state.next_temp != 0 {
                uleb(&mut code, state.next_temp);
                code.push(0x7f);
            }
            code.extend(e.code);
            self.snapshot_peak_locals.set(
                self.snapshot_peak_locals
                    .get()
                    .max(parameters as u32 + state.next_temp),
            );
            self.snapshot_bytes
                .set(self.snapshot_bytes.get() + code.len() as u32);
            bodies.push(SnapshotBody {
                parameters: parameters as u32,
                results: function.destinations.len() as u32,
                code,
            });
        }
        Ok(bodies)
    }

    fn address(&self, e: &mut Emitter, address: Address, state: &mut Emission) {
        self.source_value(e, address.offset, state);
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
        let effect = &self.graph.source.effects[id.index()];
        e.rooted_exit = Some(ExitContext::exact(effect.before));
        // Guard evaluation is pure. It never guards the preceding RMW check.
        if let Some(guard) = effect.guard {
            self.source_value(e, guard, state);
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
                if let Some(outputs) = self.outputs.get(&id) {
                    assert_eq!(outputs.len(), 1);
                    e.set(self.atom_local(outputs[0].0));
                } else {
                    e.op(0x1a);
                }
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
                self.source_value(e, *value, state);
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
                        self.source_value(e, inputs.gpr[index], state);
                        e.set(3 + index as u32);
                    }
                }
                let partial = matches!(instruction.opcode, Opcode::String(_, _));
                let scan = matches!(instruction.opcode, Opcode::String(StringOpcode::Scas, _));
                let flags_read = if scan { u32::MAX } else { contract.flags_read };
                if flags_read != 0 {
                    e.get(FLAGS);
                    e.num(!flags_read);
                    e.op(0x71);
                    self.flags(e, &inputs.flags, flags_read, state);
                    e.op(0x72);
                    e.set(FLAGS);
                }
                self.source_value(e, inputs.flags.defined, state);
                e.set(DEFINED);
                self.position(
                    e,
                    &self.graph.source.snapshots[effect.before.index()],
                    state,
                );
                if partial {
                    e.rooted_exit = Some(ExitContext {
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
                        e.set(self.atom_local(value));
                    }
                }
            }
        }
        e.rooted_exit = None;
        if effect.guard.is_some() {
            e.end();
        }
        // Computing inside a conditional store must not make a value appear
        // available on the count-zero path. Effect outputs are unconditional.
        if effect.guard.is_some() {
            state.values.clear();
            state.next_temp = 0;
        }
    }

    fn copies(
        &self,
        e: &mut Emitter,
        block: BlockId,
        predecessor: Option<BlockId>,
        state: &mut Emission,
    ) {
        let mut destinations = Vec::new();
        for &phi in &self.allocation.phis[&block] {
            let Op::Phi { incoming, .. } = &self.graph.values[phi.index()].op else {
                unreachable!()
            };
            let source = incoming
                .iter()
                .find(|(pred, _)| *pred == predecessor)
                .expect("verified phi edge")
                .1;
            let source = self.graph.resolve(source);
            let destination = self.atom_local(phi);
            if self.allocation.colors[source.index()]
                .is_some_and(|_| self.atom_local(source) == destination)
            {
                self.eliminated_copies.set(self.eliminated_copies.get() + 1);
                continue;
            }
            self.value(e, source, state);
            destinations.push(destination);
        }
        // All source expressions are evaluated before any phi is changed.
        // Loop permutations therefore cannot clobber another incoming value.
        for destination in destinations.iter().rev() {
            e.set(*destination);
        }
        self.phi_copies
            .set(self.phi_copies.get() + destinations.len() as u32);
    }
    fn edge(
        &self,
        e: &mut Emitter,
        from: BlockId,
        target: Target,
        snapshot: SnapshotId,
        state: &mut Emission,
        control: &Control,
    ) {
        if let Target::Block(target) = target {
            self.copies(e, target, Some(from), state);
            e.get(self.base + BLOCK_LEFT);
            e.num(self.graph.source.snapshots[snapshot.index()].completed_prefix);
            e.op(0x6b);
            e.set(LEFT);
            let transfer = control
                .forward
                .get(&target)
                .or_else(|| control.backward.get(&target))
                .expect("structured real CFG transfer");
            if let Some(route) = transfer.route {
                e.num(route);
                e.set(ROUTE);
                self.routed_branches.set(self.routed_branches.get() + 1);
            } else {
                self.direct_branches.set(self.direct_branches.get() + 1);
            }
            e.branch(&transfer.label);
        } else {
            self.emit_exit_snapshot(e, ExitContext::exact(snapshot));
            if let Target::External(pc) = target {
                e.num(pc);
                e.set(PC);
            }
            e.exit(2);
        }
    }
    fn emit_block(&self, e: &mut Emitter, id: BlockId, region: &Region, control: &Control) {
        let block = &self.graph.source.blocks[id.index()];
        e.get(LEFT);
        e.set(self.base + BLOCK_LEFT);
        e.get(LEFT);
        e.num(block.steps.len() as u32);
        e.op(0x4f);
        e.if_();
        let mut state = Emission::default();
        for step in &block.steps {
            for effect in &step.effects {
                self.effect(e, *effect, &mut state);
            }
            if step.effects.iter().any(|id| {
                matches!(
                    self.graph.source.effects[id.index()].kind,
                    EffectKind::Store { .. } | EffectKind::Opaque { .. }
                )
            }) {
                e.get(SMC);
                e.if_();
                e.rooted_exit = Some(ExitContext::exact(step.after));
                e.exit(7);
                e.rooted_exit = None;
                e.end();
            }
        }
        match block.terminator {
            Terminator::Direct {
                target,
                state: snapshot,
            } => self.edge(e, id, target, snapshot, &mut state, control),
            Terminator::Conditional {
                condition,
                taken,
                fallthrough,
                state: snapshot,
            } => {
                self.source_value(e, condition, &mut state);
                e.if_();
                let saved = state.clone();
                self.edge(e, id, taken, snapshot, &mut state, control);
                e.op(0x05);
                state = saved;
                self.edge(e, id, fallthrough, snapshot, &mut state, control);
                e.end();
            }
            Terminator::Indirect {
                target,
                state: snapshot,
            } => {
                // The target expression is also in the precise snapshot's EIP;
                // capture it before publishing ABI locals for the resolver.
                self.source_value(e, target, &mut state);
                e.set(TMP);
                self.emit_exit_snapshot(e, ExitContext::exact(snapshot));
                e.get(TMP);
                e.set(PC);
                e.exit(2);
            }
            Terminator::Stop { state: snapshot } => {
                // A terminal adapter normally already exited. Retain exact
                // state if a newly supported adapter returns unexpectedly.
                self.emit_exit_snapshot(e, ExitContext::exact(snapshot));
                e.exit(3);
            }
        }
        e.op(0x05);
        self.emit_exit_snapshot(e, ExitContext::exact(block.steps[0].before));
        // Exact partial-budget execution is outside the normal SSA path. A
        // taken control transfer leaves for a rooted version, never re-enters
        // an interior block using stale SSA values.
        for instruction in &region.blocks[&block.start].instructions {
            e.instruction(instruction, None);
        }
        e.block_fallthrough(&region.blocks[&block.start]);
        e.end();
    }
    fn sequence(
        &self,
        e: &mut Emitter,
        parts: &[control::Part],
        region: &Region,
        controls: &mut Control,
    ) {
        let labels: Vec<_> = parts
            .iter()
            .map(|_| {
                let label = format!("rooted-seq{}", controls.next_label);
                controls.next_label += 1;
                label
            })
            .collect();
        for (part, label) in parts.iter().zip(&labels).rev() {
            e.open(0x02, label.clone());
            for (index, entry) in part.entries().into_iter().enumerate() {
                controls.forward.insert(
                    entry,
                    Transfer {
                        label: label.clone(),
                        route: matches!(part, control::Part::Multiple(_)).then_some(index as u32),
                    },
                );
            }
        }
        for (part, label) in parts.iter().zip(&labels) {
            assert_eq!(e.labels.last(), Some(label));
            e.end();
            for entry in part.entries() {
                controls.forward.remove(&entry);
            }
            match part {
                control::Part::Block(id) => self.emit_block(e, *id, region, controls),
                control::Part::Loop { header, body } => {
                    let loop_label = format!("rooted-loop{}", header.0);
                    e.open(0x03, loop_label.clone());
                    let previous = controls.backward.insert(
                        *header,
                        Transfer {
                            label: loop_label,
                            route: None,
                        },
                    );
                    self.structured_loops.set(self.structured_loops.get() + 1);
                    self.sequence(e, body, region, controls);
                    e.end();
                    controls.backward.remove(header);
                    if let Some(previous) = previous {
                        controls.backward.insert(*header, previous);
                    }
                }
                control::Part::Multiple(blocks) => {
                    self.irreducible_components
                        .set(self.irreducible_components.get() + 1);
                    let dispatch = format!("rooted-multi{}", blocks[0].0);
                    e.open(0x02, format!("{dispatch}-invalid"));
                    e.open(0x03, dispatch.clone());
                    for (index, &id) in blocks.iter().enumerate().rev() {
                        let label = format!("{dispatch}-{index}");
                        e.open(0x02, label.clone());
                        controls.forward.insert(id, Transfer { label, route: None });
                        controls.backward.insert(
                            id,
                            Transfer {
                                label: dispatch.clone(),
                                route: Some(index as u32),
                            },
                        );
                    }
                    e.get(ROUTE);
                    e.op(0x0e);
                    uleb(&mut e.code, blocks.len() as u32);
                    for index in 0..blocks.len() {
                        uleb(&mut e.code, index as u32);
                    }
                    uleb(&mut e.code, blocks.len() as u32 + 1);
                    for &id in blocks {
                        e.end();
                        controls.forward.remove(&id);
                        self.emit_block(e, id, region, controls);
                    }
                    e.end();
                    e.end();
                    e.op(0x00);
                    for id in blocks {
                        controls.backward.remove(id);
                    }
                }
            }
        }
    }
    pub(super) fn emit(&self, e: &mut Emitter, region: &Region) {
        // An interior PC is not an implicit SSA entry, including at direct
        // table invocation. The Rust cache resolver only advertises this root.
        e.get(PC);
        e.num(region.entry);
        e.op(0x47);
        e.if_();
        e.exit(2);
        e.end();
        for &(id, field) in &self.graph.root_inputs {
            if !self.allocation.required.contains(&id) {
                continue;
            }
            match field {
                Input::Gpr(index) => e.get(3 + u32::from(index)),
                Input::OtherFlags => {
                    e.get(FLAGS);
                    e.num(!v::ARITHMETIC_MASK);
                    e.op(0x71);
                }
                Input::DefinedFlags => e.get(DEFINED),
                Input::FlagBit(bit) => {
                    e.get(FLAGS);
                    e.num(v::FLAG_MASKS[bit as usize].trailing_zeros());
                    e.op(0x76);
                    e.num(1);
                    e.op(0x71);
                }
            }
            e.set(self.atom_local(id));
        }
        self.copies(e, self.graph.source.entry, None, &mut Emission::default());
        let parts = control::structure(&self.graph);
        if let Some(control::Part::Multiple(ids)) = parts.first() {
            let entry = ids
                .iter()
                .position(|id| *id == self.graph.source.entry)
                .expect("root SCC first");
            e.num(entry as u32);
            e.set(ROUTE);
        }
        self.sequence(e, &parts, region, &mut Control::default());
    }
}

#[cfg(test)]
mod cold_function_tests {
    use super::*;
    use crate::decode::{CodeSource, discover};

    struct Code(Vec<u8>);
    impl CodeSource for Code {
        fn read_code(&self, address: u32, max_len: usize) -> Vec<u8> {
            self.0
                .get(address.wrapping_sub(0x1000) as usize..)
                .unwrap_or_default()
                .iter()
                .take(max_len)
                .copied()
                .collect()
        }
    }
    fn plan() -> Plan {
        // ADD/SHL flags and a completed load must be restored without calling
        // an arithmetic helper or executing the load again.
        let code = Code(vec![
            0x8b, 0x06, 0x83, 0xc0, 1, 0xd1, 0xe0, 0x8b, 0x1f, 0x0f, 0x0b,
        ]);
        Plan::new(&discover(&code, 0x1000, 64), 30, 3).unwrap()
    }
    fn leb(code: &[u8], offset: &mut usize) -> u32 {
        let mut value = 0;
        let mut shift = 0;
        loop {
            let byte = code[*offset];
            *offset += 1;
            value |= u32::from(byte & 127) << shift;
            if byte & 128 == 0 {
                return value;
            }
            shift += 7;
        }
    }
    #[test]
    fn snapshot_functions_are_pure_and_declare_only_used_temporaries() {
        let plan = plan();
        for index in 0..plan.graph.source.snapshots.len() {
            plan.snapshot_function(ExitContext::exact(SnapshotId(index as u32)));
        }
        let bodies = plan.snapshot_bodies().unwrap();
        assert!(!bodies.is_empty());
        for body in bodies {
            assert_eq!(body.results, 12);
            let mut offset = 0;
            let groups = leb(&body.code, &mut offset);
            let mut locals = 0;
            for _ in 0..groups {
                locals += leb(&body.code, &mut offset);
                assert_eq!(body.code[offset], 0x7f);
                offset += 1;
            }
            let mut highest_local = None;
            while offset < body.code.len() {
                let opcode = body.code[offset];
                offset += 1;
                match opcode {
                    0x20 | 0x22 => {
                        let local = leb(&body.code, &mut offset);
                        assert!(local < body.parameters + locals);
                        highest_local = Some(highest_local.unwrap_or(0).max(local));
                    }
                    0x41 | 0x42 => {
                        // Consume signed LEB without interpreting its value.
                        while body.code[offset] & 128 != 0 {
                            offset += 1;
                        }
                        offset += 1;
                    }
                    0x0b => assert_eq!(offset, body.code.len()),
                    0x1b | 0x45..=0x4f | 0x54 | 0x56 | 0x69..=0x76 | 0x7c | 0xad => {}
                    _ => panic!("snapshot has non-pure opcode {opcode:#x}"),
                }
            }
            if locals != 0 {
                assert_eq!(highest_local, Some(body.parameters + locals - 1));
            }
        }
        assert_eq!(
            plan.temporary_peak.get(),
            0,
            "helper temporaries do not extend main locals"
        );
        assert_eq!(plan.local_count(), FIRST_ATOM + plan.allocation.count);
    }
    #[test]
    fn preserve_fields_change_signature_and_do_not_cross_function_boundaries() {
        let plan = plan();
        let snapshot = SnapshotId(0);
        for preserve_gprs in [0, 0x54, 0xff] {
            for preserve_flags in [false, true] {
                for preserve_defined in [false, true] {
                    for preserve_position in [false, true] {
                        let context = ExitContext {
                            snapshot,
                            preserve_gprs,
                            preserve_flags,
                            preserve_defined,
                            preserve_position,
                        };
                        let (index, function) = plan.snapshot_function(context);
                        assert_eq!(plan.snapshot_function(context).0, index);
                        assert_eq!(
                            function.destinations.len(),
                            (8 - preserve_gprs.count_ones()) as usize
                                + usize::from(!preserve_flags)
                                + usize::from(!preserve_defined)
                                + 2 * usize::from(!preserve_position)
                        );
                        for gpr in 0..8 {
                            assert_eq!(
                                function.destinations.contains(&(3 + gpr)),
                                preserve_gprs & (1 << gpr) == 0
                            );
                        }
                        assert_eq!(function.destinations.contains(&FLAGS), !preserve_flags);
                        assert_eq!(function.destinations.contains(&DEFINED), !preserve_defined);
                        assert_eq!(function.destinations.contains(&PC), !preserve_position);
                        assert_eq!(function.destinations.contains(&LEFT), !preserve_position);
                    }
                }
            }
        }
        let bodies = plan.snapshot_bodies().unwrap();
        assert_eq!(bodies.len(), 24);
        assert!(
            bodies
                .iter()
                .any(|body| body.parameters == 0 && body.results == 0)
        );
    }
    #[test]
    fn oversized_snapshot_signatures_and_local_counts_fail_explicitly() {
        let plan = plan();
        plan.cold_exits
            .borrow_mut()
            .functions
            .push(SnapshotFunction {
                context: ExitContext::exact(SnapshotId(0)),
                atoms: vec![ValueId(0); MAX_FUNCTION_PARAMETERS],
                destinations: vec![],
            });
        assert!(
            plan.snapshot_bodies()
                .err()
                .unwrap()
                .contains("parameters (limit 1000)")
        );
        plan.temporary_peak.set(MAX_FUNCTION_LOCALS);
        assert!(
            plan.snapshot_bodies()
                .err()
                .unwrap()
                .contains("main function exceeds Wasm local limit")
        );
    }
}

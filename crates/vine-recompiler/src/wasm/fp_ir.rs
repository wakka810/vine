//! Explicit architectural x87 effects. No runtime descriptor/State::prepare.
//! Raw80 registers remain in their private physical bank; environment values
//! stay local across blocks and are published at every precise exit.
use super::*;
use crate::fp_ir::{Destination, Effect, Instruction, MemoryEffect, Source, format_id};
use vine_x87::{Arithmetic, Format};

const CONTROL: u32 = 0;
const STATUS: u32 = 1;
const TAG: u32 = 2;
const FOP: u32 = 3;
const FIP: u32 = 4;
const FCS: u32 = 5;
const FDP: u32 = 6;
const FDS: u32 = 7;
const VA: u32 = 8;
const VB: u32 = 11;
const VR: u32 = 14;
const BITS: u32 = 17;
const EXCEPTIONS: u32 = 19;
const ROUND: u32 = 20;
const EMPTY: u32 = 21;
const OVERFLOW: u32 = 22;
const UNMASKED: u32 = 23;
const NEXT_STATUS: u32 = 24;
const COMMIT: u32 = 25;
const INDEX: u32 = 26;
const INDEX2: u32 = 27;
const POINTER: u32 = 28;
const OFFSET: u32 = 29;
const SOURCE_EXCEPTIONS: u32 = 30;
const OTHER_EXCEPTIONS: u32 = 31;
const SIGNALING: u32 = 32;
const OUTPUT: u32 = 33;
const MEMORY: u32 = 34;
const TEMP: u32 = 37;
pub(super) const LOCAL_COUNT: u32 = 38;

/// The numeric operation must explicitly prepare an exponent-biased register
/// result before generated exception handling may commit an unmasked UE.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ResultDestination {
    Memory,
    Register,
    UnderflowAdjustedRegister,
    RangeAdjustedRegister,
}

/// A repeated statically typed plan has one module-local function. EIP/FOP and
/// effective address are dynamic inputs, never specialization keys.
fn normalized(mut plan: Instruction) -> Instruction {
    plan.eip = 0;
    plan.fop = 0;
    if let Some(memory) = &mut plan.memory {
        memory.operand.base = None;
        memory.operand.index = None;
        memory.operand.scale = 1;
        memory.operand.displacement = 0;
        memory.operand.segment = Segment::Flat;
    }
    plan
}

pub(super) fn function_body(plan: Instruction, typed_numeric: bool) -> Vec<u8> {
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
        fp_base: 30,
        fp_plan: BTreeMap::new(),
        fp_calls: BTreeMap::new(),
        fp_leaf: true,
        typed_numeric,
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
    // 0 state,1 table,2 effective offset,3 EIP,4 FOP,5..12 environment.
    // Preserve the ordinary emitter scratch indices13..29; FP locals30+.
    uleb(&mut e.code, 1);
    uleb(&mut e.code, 17 + LOCAL_COUNT);
    e.op(0x7f);
    for field in 0..8 {
        e.get(5 + field);
        e.fs(field);
    }
    e.open(0x02, "exit".into());
    e.fp_instruction(plan);
    e.end();
    for field in 0..8 {
        e.fg(field);
    }
    for local in [KIND, FAULT, ERROR, SMC] {
        e.get(local);
    }
    e.op(0x0b);
    e.code
}

impl Emitter {
    pub(super) fn fp_function_plans(&mut self, first: u32) -> Vec<Instruction> {
        let mut plans = Vec::new();
        for (&eip, &plan) in &self.fp_plan {
            let plan = normalized(plan);
            let index = plans
                .iter()
                .position(|existing| *existing == plan)
                .unwrap_or_else(|| {
                    plans.push(plan);
                    plans.len() - 1
                });
            self.fp_calls.insert(eip, first + index as u32);
        }
        plans
    }
    pub(super) fn fp_call(&mut self, instruction: Instruction) {
        if self.has_mmx {
            self.flush_mmx();
            self.fp_initialize();
        }
        self.get(0);
        self.get(1);
        if let Some(memory) = instruction.memory {
            self.effective(memory.operand, false);
        } else {
            self.num(0);
        }
        self.num(instruction.eip);
        self.num(u32::from(instruction.fop));
        for field in 0..8 {
            self.fg(field);
        }
        self.op(0x10);
        uleb(&mut self.code, self.fp_calls[&instruction.eip]);
        for local in [SMC, ERROR, FAULT, KIND] {
            self.set(local);
        }
        for field in (0..8).rev() {
            self.fs(field);
        }
        self.get(KIND);
        self.if_();
        self.branch("exit");
        self.end();
        if self.has_mmx {
            self.fp_flush();
            self.initialize_mmx();
        }
    }
    fn fg(&mut self, local: u32) {
        self.get(self.fp_base + local);
    }
    fn fs(&mut self, local: u32) {
        self.set(self.fp_base + local);
    }
    fn fc(&mut self, local: u32, value: u32) {
        self.num(value);
        self.fs(local);
    }
    fn fcopy(&mut self, to: u32, from: u32, words: u32) {
        for word in 0..words {
            self.fg(from + word);
            self.fs(to + word);
        }
    }
    pub(super) fn fp_initialize(&mut self) {
        if self.fp_plan.is_empty() {
            return;
        }
        for field in 0..8 {
            self.load(0, 236 + 4 * field);
            self.fs(field);
        }
    }
    pub(super) fn fp_flush(&mut self) {
        if self.fp_plan.is_empty() {
            return;
        }
        for field in 0..8 {
            self.store(0, 236 + 4 * field, self.fp_base + field);
        }
    }
    pub(super) fn fp_status_ax(&mut self) {
        self.fg(STATUS);
        self.set(R);
        self.assign(Operand::Reg(Register {
            index: 0,
            width: Width::W16,
            high8: false,
        }));
    }
    fn fp_pending(&mut self) {
        self.fg(CONTROL);
        self.num(u32::MAX);
        self.op(0x73);
        self.fg(STATUS);
        self.op(0x71);
        self.num(0x3f);
        self.op(0x71);
        self.tee(ERROR);
        self.if_();
        self.num(0);
        self.set(FAULT);
        self.exit(crate::x87::FLOATING_POINT_EXIT);
        self.end();
    }
    fn fp_index(&mut self, logical: u8, local: u32) {
        self.fg(STATUS);
        self.num(11);
        self.op(0x76);
        self.num(u32::from(logical));
        self.op(0x6a);
        self.num(7);
        self.op(0x71);
        self.fs(local);
    }
    fn fp_pointer(&mut self, index: u32) {
        self.get(0);
        self.fg(index);
        self.num(12);
        self.op(0x6c);
        self.op(0x6a);
        self.num(140);
        self.op(0x6a);
        self.fs(POINTER);
    }
    fn fp_empty(&mut self, index: u32) {
        self.fg(TAG);
        self.fg(index);
        self.num(1);
        self.op(0x74);
        self.op(0x76);
        self.num(3);
        self.op(0x71);
        self.num(3);
        self.op(0x46);
    }
    fn fp_indefinite(&mut self, value: u32) {
        self.fc(value, 0);
        self.fc(value + 1, 0xc000_0000);
        self.fc(value + 2, 0xffff);
    }
    fn fp_read(&mut self, logical: u8, value: u32, ex: u32) {
        self.fp_index(logical, INDEX);
        self.fp_empty(INDEX);
        self.if_();
        self.fp_indefinite(value);
        self.fc(ex, 65);
        self.fc(EMPTY, 1);
        self.op(0x05);
        self.fp_pointer(INDEX);
        for word in 0..3 {
            self.load(self.fp_base + POINTER, word * 4);
            if word == 2 {
                self.num(0xffff);
                self.op(0x71);
            }
            self.fs(value + word);
        }
        self.fc(ex, 0);
        self.end();
    }
    /// Classify the entire raw80 value; payloads are never converted to f64.
    fn fp_tag_of(&mut self, value: u32) {
        self.fg(value + 2);
        self.num(0x7fff);
        self.op(0x71);
        self.fs(TEMP);
        self.fg(TEMP);
        self.op(0x45);
        self.fg(value);
        self.fg(value + 1);
        self.op(0x72);
        self.op(0x45);
        self.op(0x71);
        self.if_();
        self.fc(TEMP, 1);
        self.op(0x05);
        self.fg(TEMP);
        self.op(0x45);
        self.fg(TEMP);
        self.num(0x7fff);
        self.op(0x46);
        self.op(0x72);
        self.fg(value + 1);
        self.num(31);
        self.op(0x76);
        self.op(0x45);
        self.op(0x72);
        self.if_();
        self.fc(TEMP, 2);
        self.op(0x05);
        self.fc(TEMP, 0);
        self.end();
        self.end();
        self.fg(TEMP);
    }
    fn fp_write(&mut self, index: u32, value: u32) {
        self.fp_pointer(index);
        for word in 0..3 {
            self.store(
                self.fp_base + POINTER,
                word * 4,
                self.fp_base + value + word,
            );
        }
        self.fp_tag_of(value);
        self.fs(TEMP);
        self.fg(TAG);
        self.num(3);
        self.fg(index);
        self.num(1);
        self.op(0x74);
        self.op(0x74);
        self.num(u32::MAX);
        self.op(0x73);
        self.op(0x71);
        self.fg(TEMP);
        self.fg(index);
        self.num(1);
        self.op(0x74);
        self.op(0x74);
        self.op(0x72);
        self.fs(TAG);
    }
    fn fp_set_top(&mut self, index: u32) {
        self.fg(STATUS);
        self.num(!(7 << 11));
        self.op(0x71);
        self.fg(index);
        self.num(7);
        self.op(0x71);
        self.num(11);
        self.op(0x74);
        self.op(0x72);
        self.fs(STATUS);
    }
    fn fp_pop(&mut self) {
        self.fp_index(0, INDEX);
        self.fg(TAG);
        self.num(3);
        self.fg(INDEX);
        self.num(1);
        self.op(0x74);
        self.op(0x74);
        self.op(0x72);
        self.fs(TAG);
        self.fg(INDEX);
        self.num(1);
        self.op(0x6a);
        self.fs(INDEX);
        self.fp_set_top(INDEX);
    }
    fn fp_nan(&mut self, value: u32) {
        self.fg(value + 2);
        self.num(0x7fff);
        self.op(0x71);
        self.num(0x7fff);
        self.op(0x46);
        self.fg(value + 1);
        self.num(0x7fff_ffff);
        self.op(0x71);
        self.fg(value);
        self.op(0x72);
        self.op(0x45);
        self.op(0x45);
        self.op(0x71);
    }
    fn fp_numeric(&mut self, operation: u32, left: u32, right: u32) {
        if !self.typed_numeric {
            self.op(0x23);
            uleb(&mut self.code, 0);
            self.num(operation);
        }
        self.fg(CONTROL);
        self.num(0xffff);
        self.op(0x71);
        for value in [left, right] {
            for word in 0..3 {
                self.fg(value + word);
            }
        }
        self.op(0x10);
        uleb(
            &mut self.code,
            if self.typed_numeric { 1 + operation } else { 1 },
        );
        self.fs(OUTPUT);
        self.fg(OUTPUT);
        self.op(0x45);
        self.if_();
        if self.typed_numeric {
            // Read the pure helper's error before any later numeric call can
            // replace its scratch. No architectural state has committed.
            self.op(0x10);
            uleb(&mut self.code, crate::fp_ir::NUMERIC_KERNEL_COUNT + 1);
            self.set(ERROR);
        }
        self.exit(3);
        self.end();
        for (target, offset) in [
            (VR, 0),
            (VR + 1, 4),
            (VR + 2, 8),
            (BITS, 12),
            (BITS + 1, 16),
            (EXCEPTIONS, 20),
            (ROUND, 24),
        ] {
            self.load(self.fp_base + OUTPUT, offset);
            self.fs(target);
        }
    }
    fn fp_memory(&mut self, memory: MemoryEffect) {
        if self.fp_leaf {
            self.get(2);
        } else {
            self.effective(memory.operand, false);
        }
        self.fs(OFFSET);
        let segment = 44 + memory.segment * 16;
        if memory.segment == 1 {
            self.exit(3);
        }
        if memory.segment != 4 {
            self.load(0, segment + 4);
            self.if_();
            self.exit(3);
            self.end();
        }
        self.load(0, segment + 12);
        self.num(0);
        self.op(0x47);
        self.load(0, segment + 12);
        self.num(0xf2);
        self.op(0x47);
        self.op(0x71);
        self.if_();
        self.exit(3);
        self.end();
        let bytes = memory.operand.width.bytes();
        self.fg(OFFSET);
        self.op(0xad);
        self.op(0x42);
        sleb(&mut self.code, i64::from(bytes - 1));
        self.op(0x7c);
        self.load(0, segment + 8);
        self.op(0xad);
        self.op(0x56);
        self.check(
            if memory.segment == 2 { 9 } else { 6 },
            self.fp_base + OFFSET,
            0,
        );
        self.fg(OFFSET);
        self.load(0, segment + 4);
        self.op(0x6a);
        self.set(ADDR);
        self.get(ADDR);
        self.num(u32::MAX - (bytes - 1));
        self.op(0x4b);
        self.check(if memory.segment == 4 { 3 } else { 6 }, ADDR, 0);
        // Both virtual endpoints are checked before any load/store. The table
        // and allocation pointers are backend-owned sealed metadata, exactly
        // as in scalar lowering. Numeric helpers cannot mutate mappings.
        let need = 8 | if memory.write { 2 } else { 1 };
        for (entry, last) in [(ENTRY, false), (ENTRY2, true)] {
            self.get(ADDR);
            if last {
                self.num(bytes - 1);
                self.op(0x6a);
            }
            self.tee(TMP);
            self.num(12);
            self.op(0x76);
            self.num(4);
            self.op(0x74);
            self.get(1);
            self.op(0x6a);
            self.set(entry);
            self.load(entry, 4);
            self.num(need);
            self.op(0x71);
            self.num(need);
            self.op(0x47);
            self.if_();
            self.get(TMP);
            if last {
                self.num(0xffff_f000);
                self.op(0x71);
            }
            self.set(FAULT);
            self.load(entry, 4);
            self.num(3);
            self.op(0x76);
            self.num(1);
            self.op(0x71);
            self.num(if memory.write { 6 } else { 4 });
            self.op(0x72);
            self.set(ERROR);
            self.exit(4);
            self.end();
        }
        self.fp_memory_pointers();
    }
    fn fp_memory_pointers(&mut self) {
        self.load(ENTRY, 0);
        self.get(ADDR);
        self.num(4095);
        self.op(0x71);
        self.op(0x6a);
        self.set(PTR);
        self.load(ENTRY2, 0);
        self.set(PTR2);
    }
    fn fp_memory_read(&mut self, width: Width) {
        for word in 0..3 {
            self.fc(MEMORY + word, 0);
        }
        self.get(ENTRY);
        self.get(ENTRY2);
        self.op(0x46);
        self.if_();
        for offset in (0..width.bytes()).step_by(4) {
            self.get(PTR);
            self.mem(
                if width.bytes() - offset < 4 {
                    0x2f
                } else {
                    0x28
                },
                offset,
            );
            self.fs(MEMORY + offset / 4);
        }
        self.op(0x05);
        for byte in 0..width.bytes() {
            self.byte_pointer(byte);
            self.mem(0x2d, 0);
            self.num((byte % 4) * 8);
            self.op(0x74);
            self.fg(MEMORY + byte / 4);
            self.op(0x72);
            self.fs(MEMORY + byte / 4);
        }
        self.end();
    }
    fn fp_memory_write(&mut self, width: Width, value: u32) {
        // Only a real store reserves journal space, after numeric suppression
        // decisions but before any byte or architectural FP state changes.
        self.dirty_preflight();
        self.fp_memory_pointers();
        self.get(ENTRY);
        self.get(ENTRY2);
        self.op(0x46);
        self.if_();
        for offset in (0..width.bytes()).step_by(4) {
            self.get(PTR);
            self.fg(value + offset / 4);
            self.mem(
                if width.bytes() - offset < 4 {
                    0x3b
                } else {
                    0x36
                },
                offset,
            );
        }
        self.op(0x05);
        for byte in 0..width.bytes() {
            self.byte_pointer(byte);
            self.fg(value + byte / 4);
            self.num((byte % 4) * 8);
            self.op(0x76);
            self.mem(0x3a, 0);
        }
        self.end();
        self.mark_dirty();
        self.load(ENTRY, 4);
        self.load(ENTRY2, 4);
        self.op(0x72);
        self.num(4 | 32);
        self.op(0x71);
        self.set(SMC);
    }

    fn fp_source(&mut self, source: Source, value: u32, ex: u32, arithmetic: bool) {
        match source {
            Source::Register(index) => self.fp_read(index, value, ex),
            Source::Constant(kind) => {
                let nearest = kind.value(0).words();
                for (index, word) in nearest.into_iter().enumerate() {
                    self.fc(value + index as u32, word);
                }
                for rc in 1..4 {
                    let words = kind.value(rc << 10).words();
                    if words == nearest {
                        continue;
                    }
                    self.fg(CONTROL);
                    self.num(10);
                    self.op(0x76);
                    self.num(3);
                    self.op(0x71);
                    self.num(u32::from(rc));
                    self.op(0x46);
                    self.if_();
                    for (index, word) in words.into_iter().enumerate() {
                        if word != nearest[index] {
                            self.fc(value + index as u32, word);
                        }
                    }
                    self.end();
                }
                self.fc(ex, 0);
            }
            Source::Memory(format) => {
                if format == Format::Float80 {
                    self.fcopy(value, MEMORY, 3);
                    self.fc(ex, 0);
                    return;
                }
                // Widening a memory arithmetic operand must retain its SNaN
                // signaling bit for the later arithmetic NaN selection.
                self.fc(SIGNALING, 0);
                if arithmetic && matches!(format, Format::Float32 | Format::Float64) {
                    let (word, exponent, fraction, quiet) = if format == Format::Float32 {
                        (MEMORY, 0x7f80_0000, 0x007f_ffff, 0x0040_0000)
                    } else {
                        (MEMORY + 1, 0x7ff0_0000, 0x000f_ffff, 0x0008_0000)
                    };
                    self.fg(word);
                    self.num(exponent);
                    self.op(0x71);
                    self.num(exponent);
                    self.op(0x46);
                    self.fg(word);
                    self.num(fraction);
                    self.op(0x71);
                    if format == Format::Float64 {
                        self.fg(MEMORY);
                        self.op(0x72);
                    }
                    self.op(0x45);
                    self.op(0x45);
                    self.op(0x71);
                    self.fg(word);
                    self.num(quiet);
                    self.op(0x71);
                    self.op(0x45);
                    self.op(0x71);
                    self.fs(SIGNALING);
                }
                self.fp_numeric(6 + format_id(format), MEMORY, MEMORY);
                self.fcopy(value, VR, 3);
                self.fg(EXCEPTIONS);
                self.fs(ex);
                self.fg(SIGNALING);
                self.if_();
                self.fg(value + 1);
                self.num(!0x4000_0000);
                self.op(0x71);
                self.fs(value + 1);
                self.end();
            }
        }
    }

    fn fp_summary(&mut self, status: u32) {
        self.fg(status);
        self.fg(CONTROL);
        self.num(u32::MAX);
        self.op(0x73);
        self.op(0x71);
        self.num(0x3f);
        self.op(0x71);
        self.if_();
        self.fg(status);
        self.num(0x8080);
        self.op(0x72);
        self.fs(status);
        self.op(0x05);
        self.fg(status);
        self.num(!0x8080);
        self.op(0x71);
        self.fs(status);
        self.end();
    }
    fn fp_unmasked(&mut self) {
        self.fg(EXCEPTIONS);
        self.fg(CONTROL);
        self.num(u32::MAX);
        self.op(0x73);
        self.op(0x71);
        self.num(0x3f);
        self.op(0x71);
        self.fs(UNMASKED);
    }
    /// Compute a small transaction in locals. Unsupported range results exit
    /// before status/location/stack or bytes become visible.
    fn fp_apply_exceptions(&mut self, memory_store: bool) {
        self.fp_apply_result_exceptions(if memory_store {
            ResultDestination::Memory
        } else {
            ResultDestination::Register
        });
    }
    fn fp_apply_result_exceptions(&mut self, destination: ResultDestination) {
        let memory_store = destination == ResultDestination::Memory;
        let unsupported_range = match destination {
            ResultDestination::UnderflowAdjustedRegister => 8,
            ResultDestination::RangeAdjustedRegister => 0,
            _ => 24,
        };
        self.fp_unmasked();
        self.fg(UNMASKED);
        self.num(1);
        self.op(0x71);
        self.if_();
        self.fg(EXCEPTIONS);
        self.num(65);
        self.op(0x71);
        self.fs(EXCEPTIONS);
        self.op(0x05);
        self.fg(UNMASKED);
        self.num(2);
        self.op(0x71);
        self.if_();
        self.fg(EXCEPTIONS);
        self.num(2);
        self.op(0x71);
        self.fs(EXCEPTIONS);
        self.op(0x05);
        self.fg(UNMASKED);
        self.num(4);
        self.op(0x71);
        self.if_();
        self.fg(EXCEPTIONS);
        self.num(4);
        self.op(0x71);
        self.fs(EXCEPTIONS);
        self.end();
        self.end();
        self.end();
        self.fp_unmasked();
        self.fg(UNMASKED);
        self.num(unsupported_range);
        self.op(0x71);
        self.if_();
        if memory_store {
            self.fg(EXCEPTIONS);
            self.num(!32);
            self.op(0x71);
            self.fs(EXCEPTIONS);
        } else {
            self.exit(3);
        }
        self.end();
        self.fg(STATUS);
        self.num(!0x200);
        self.op(0x71);
        self.fg(EXCEPTIONS);
        self.op(0x72);
        self.fs(NEXT_STATUS);
        self.fg(EXCEPTIONS);
        self.num(64);
        self.op(0x71);
        self.op(0x45);
        self.op(0x45);
        self.fg(OVERFLOW);
        self.op(0x71);
        self.fg(ROUND);
        self.fg(UNMASKED);
        self.num(7 | unsupported_range);
        self.op(0x71);
        self.op(0x45);
        self.op(0x71);
        self.op(0x72);
        self.if_();
        self.fg(NEXT_STATUS);
        self.num(0x200);
        self.op(0x72);
        self.fs(NEXT_STATUS);
        self.end();
        self.fp_summary(NEXT_STATUS);
        self.fg(UNMASKED);
        self.num(if memory_store { 31 } else { 7 });
        self.op(0x71);
        self.op(0x45);
        self.fs(COMMIT);
    }
    fn fp_operand_exceptions(&mut self) {
        self.fg(EMPTY);
        self.if_();
        self.fp_indefinite(VR);
        self.fc(EXCEPTIONS, 65);
        self.fc(ROUND, 0);
        self.op(0x05);
        self.fg(SOURCE_EXCEPTIONS);
        self.fg(OTHER_EXCEPTIONS);
        self.op(0x72);
        self.num(1);
        self.op(0x71);
        self.if_();
        self.fc(EXCEPTIONS, 1);
        self.fc(ROUND, 0);
        self.op(0x05);
        self.fg(EXCEPTIONS);
        self.num(5);
        self.op(0x71);
        self.op(0x45);
        self.fp_nan(VA);
        self.op(0x45);
        self.op(0x71);
        self.fp_nan(VB);
        self.op(0x45);
        self.op(0x71);
        self.if_();
        self.fg(EXCEPTIONS);
        self.fg(SOURCE_EXCEPTIONS);
        self.op(0x72);
        self.fg(OTHER_EXCEPTIONS);
        self.op(0x72);
        self.fs(EXCEPTIONS);
        self.end();
        self.end();
        self.end();
    }
    fn fp_location(&mut self, instruction: Instruction) {
        if !instruction.record_location {
            return;
        }
        if self.fp_leaf {
            self.get(3);
            self.fs(FIP);
            self.get(4);
            self.num(0x7ff);
            self.op(0x71);
            self.fs(FOP);
        } else {
            self.fc(FIP, instruction.eip);
            self.fc(FOP, u32::from(instruction.fop & 0x7ff));
        }
        self.load(0, 44 + 16);
        self.num(0xffff);
        self.op(0x71);
        self.fs(FCS);
        if let Some(memory) = instruction.memory {
            self.fg(OFFSET);
            self.fs(FDP);
            self.load(0, 44 + memory.segment * 16);
            self.num(0xffff);
            self.op(0x71);
            self.fs(FDS);
        }
    }

    pub(super) fn fp_instruction(&mut self, instruction: Instruction) {
        // MMX and FP share a physical raw80 bank. The dispatch wrapper keeps
        // the two environment caches synchronized at a mixed-mode boundary.
        if self.has_mmx {
            self.flush_mmx();
            self.fp_initialize();
        }
        if instruction.waiting {
            self.fp_pending();
        }
        if let Some(memory) = instruction.memory {
            self.fp_memory(memory);
            if !memory.write {
                self.fp_memory_read(memory.operand.width);
            }
        }
        for local in [
            EMPTY,
            OVERFLOW,
            SOURCE_EXCEPTIONS,
            OTHER_EXCEPTIONS,
            EXCEPTIONS,
            ROUND,
        ] {
            self.fc(local, 0);
        }
        match instruction.effect {
            Effect::Load(source) => {
                self.fp_source(source, VA, SOURCE_EXCEPTIONS, false);
                self.fp_index(7, INDEX2);
                self.fp_empty(INDEX2);
                self.op(0x45);
                self.fs(OVERFLOW);
                self.fg(SOURCE_EXCEPTIONS);
                self.fs(EXCEPTIONS);
                self.fg(OVERFLOW);
                self.if_();
                self.fp_indefinite(VA);
                self.fc(EXCEPTIONS, 65);
                self.fc(ROUND, 0);
                self.end();
                // FLD retains the documented unmasked-DE push exception.
                self.fg(EXCEPTIONS);
                self.num(2);
                self.op(0x46);
                self.fs(TEMP);
                self.fp_apply_exceptions(false);
                self.fg(NEXT_STATUS);
                self.fs(STATUS);
                self.fg(COMMIT);
                self.fg(TEMP);
                self.op(0x72);
                self.if_();
                self.fp_set_top(INDEX2);
                self.fp_write(INDEX2, VA);
                self.end();
            }
            Effect::Store { destination, pop } => {
                self.fp_read(0, VA, SOURCE_EXCEPTIONS);
                match destination {
                    Destination::Register(index) => {
                        self.fg(SOURCE_EXCEPTIONS);
                        self.fs(EXCEPTIONS);
                        self.fp_apply_exceptions(false);
                        self.fg(NEXT_STATUS);
                        self.fs(STATUS);
                        self.fg(COMMIT);
                        self.if_();
                        self.fp_index(index, INDEX2);
                        self.fp_write(INDEX2, VA);
                        if pop {
                            self.fp_pop();
                        }
                        self.end();
                    }
                    Destination::Memory(format) => {
                        if format == Format::Float80 {
                            self.fc(EXCEPTIONS, 0);
                            self.fc(ROUND, 0);
                        } else {
                            self.fp_numeric(12 + format_id(format), VA, VA);
                        }
                        self.fg(EMPTY);
                        self.if_();
                        self.fc(EXCEPTIONS, 65);
                        self.fc(ROUND, 0);
                        self.end();
                        self.fp_apply_exceptions(true);
                        self.fg(COMMIT);
                        self.if_();
                        self.fp_memory_write(
                            instruction.memory.unwrap().operand.width,
                            if format == Format::Float80 { VA } else { BITS },
                        );
                        self.end();
                        self.fg(NEXT_STATUS);
                        self.fs(STATUS);
                        if pop {
                            self.fg(COMMIT);
                            self.if_();
                            self.fp_pop();
                            self.end();
                        }
                    }
                }
            }
            Effect::Arithmetic {
                operation,
                left,
                right,
                destination,
                pop,
            } => {
                self.fp_source(left, VA, SOURCE_EXCEPTIONS, true);
                if matches!(
                    operation,
                    Arithmetic::SquareRoot | Arithmetic::RoundToInteger
                ) {
                    for word in 0..3 {
                        self.fc(VB + word, 0);
                    }
                } else {
                    self.fp_source(right, VB, OTHER_EXCEPTIONS, true);
                }
                // Empty stack operands take priority over numeric helper
                // validation and do not need an arithmetic result at all.
                self.fg(EMPTY);
                self.if_();
                self.fp_indefinite(VR);
                self.fc(EXCEPTIONS, 65);
                self.fc(ROUND, 0);
                self.op(0x05);
                self.fp_numeric(operation as u32, VA, VB);
                self.fp_operand_exceptions();
                self.end();
                self.fp_apply_exceptions(false);
                self.fg(NEXT_STATUS);
                self.fs(STATUS);
                self.fg(COMMIT);
                self.if_();
                self.fp_index(destination, INDEX2);
                self.fp_write(INDEX2, VR);
                if pop {
                    self.fp_pop();
                }
                self.end();
            }
            Effect::Log2Multiply | Effect::Arctangent => {
                self.fp_read(0, VA, SOURCE_EXCEPTIONS);
                self.fp_read(1, VB, OTHER_EXCEPTIONS);
                self.fg(EMPTY);
                self.if_();
                self.fp_indefinite(VR);
                self.fc(EXCEPTIONS, 65);
                self.fc(ROUND, 0);
                self.op(0x05);
                self.fp_numeric(
                    if instruction.effect == Effect::Arctangent {
                        21
                    } else {
                        20
                    },
                    VA,
                    VB,
                );
                self.end();
                // Numeric classification is one combined operation. Do not
                // leak intermediate log/multiply exceptions into guest state.
                self.fp_apply_exceptions(false);
                self.fg(NEXT_STATUS);
                self.fs(STATUS);
                self.fg(COMMIT);
                self.if_();
                self.fp_index(1, INDEX2);
                self.fp_write(INDEX2, VR);
                self.fp_pop();
                self.end();
            }
            Effect::Cosine | Effect::Sine => {
                self.fp_read(0, VA, SOURCE_EXCEPTIONS);
                self.fg(EMPTY);
                self.if_();
                self.fp_indefinite(VR);
                self.fc(EXCEPTIONS, 65);
                self.fc(ROUND, 0);
                self.fc(BITS, 0);
                self.op(0x05);
                self.fp_numeric(
                    if instruction.effect == Effect::Sine {
                        23
                    } else {
                        22
                    },
                    VA,
                    VA,
                );
                self.end();
                self.fg(BITS);
                self.if_();
                // Out-of-range finite input retires with ST0 unchanged.
                // C1 is undefined here; use zero consistently with State.
                self.fg(STATUS);
                self.num(!0x200);
                self.op(0x71);
                self.num(0x400);
                self.op(0x72);
                self.fs(STATUS);
                self.op(0x05);
                self.fp_apply_result_exceptions(if instruction.effect == Effect::Sine {
                    ResultDestination::UnderflowAdjustedRegister
                } else {
                    ResultDestination::Register
                });
                self.fg(NEXT_STATUS);
                self.num(!0x400);
                self.op(0x71);
                self.fs(STATUS);
                self.fg(COMMIT);
                self.if_();
                self.fp_index(0, INDEX2);
                self.fp_write(INDEX2, VR);
                self.end();
                self.end();
            }
            Effect::ExponentialMinusOne | Effect::Scale => {
                let scale = instruction.effect == Effect::Scale;
                self.fp_read(0, VA, SOURCE_EXCEPTIONS);
                if scale {
                    self.fp_read(1, VB, OTHER_EXCEPTIONS);
                }
                self.fg(EMPTY);
                self.if_();
                self.fp_indefinite(VR);
                self.fc(EXCEPTIONS, 65);
                self.fc(ROUND, 0);
                self.op(0x05);
                self.fp_numeric(if scale { 25 } else { 24 }, VA, if scale { VB } else { VA });
                self.end();
                self.fp_apply_result_exceptions(if scale {
                    ResultDestination::RangeAdjustedRegister
                } else {
                    ResultDestination::UnderflowAdjustedRegister
                });
                self.fg(NEXT_STATUS);
                self.fs(STATUS);
                self.fg(COMMIT);
                self.if_();
                self.fp_index(0, INDEX2);
                self.fp_write(INDEX2, VR);
                self.end();
            }
            Effect::Compare { right, quiet, pops } => {
                self.fp_read(0, VA, SOURCE_EXCEPTIONS);
                self.fp_source(right, VB, OTHER_EXCEPTIONS, true);
                self.fp_numeric(if quiet { 19 } else { 18 }, VA, VB);
                self.fg(EMPTY);
                self.if_();
                self.fc(BITS, 3);
                self.end();
                self.fp_operand_exceptions();
                self.fp_apply_exceptions(false);
                self.fg(NEXT_STATUS);
                self.fs(STATUS);
                self.fg(COMMIT);
                self.if_();
                self.fg(BITS);
                self.op(0x45);
                self.if_();
                self.fc(TEMP, 0x4000);
                self.op(0x05);
                self.fg(BITS);
                self.num(1);
                self.op(0x46);
                self.if_();
                self.fc(TEMP, 0x100);
                self.op(0x05);
                self.fg(BITS);
                self.num(2);
                self.op(0x46);
                self.if_();
                self.fc(TEMP, 0);
                self.op(0x05);
                self.fc(TEMP, 0x4500);
                self.end();
                self.end();
                self.end();
                self.fg(STATUS);
                self.num(!0x4500);
                self.op(0x71);
                self.fg(TEMP);
                self.op(0x72);
                self.fs(STATUS);
                for _ in 0..pops {
                    self.fp_pop();
                }
                self.end();
            }
            Effect::Exchange(index) => {
                self.fp_read(0, VA, SOURCE_EXCEPTIONS);
                self.fp_read(index, VB, OTHER_EXCEPTIONS);
                self.fg(EMPTY);
                self.if_();
                self.fc(EXCEPTIONS, 65);
                self.end();
                self.fp_apply_exceptions(false);
                self.fg(NEXT_STATUS);
                self.fs(STATUS);
                self.fg(COMMIT);
                self.if_();
                self.fp_index(0, INDEX2);
                self.fp_write(INDEX2, VB);
                self.fp_index(index, INDEX2);
                self.fp_write(INDEX2, VA);
                self.end();
            }
            Effect::ChangeSign | Effect::Absolute => {
                self.fp_read(0, VA, SOURCE_EXCEPTIONS);
                self.fg(SOURCE_EXCEPTIONS);
                self.fs(EXCEPTIONS);
                self.fg(EMPTY);
                self.op(0x45);
                self.if_();
                self.fg(VA + 2);
                self.num(if instruction.effect == Effect::Absolute {
                    0x7fff
                } else {
                    0x8000
                });
                self.op(if instruction.effect == Effect::Absolute {
                    0x71
                } else {
                    0x73
                });
                self.fs(VA + 2);
                self.end();
                self.fp_apply_exceptions(false);
                self.fg(NEXT_STATUS);
                self.fs(STATUS);
                self.fg(COMMIT);
                self.if_();
                self.fp_index(0, INDEX2);
                self.fp_write(INDEX2, VA);
                self.end();
            }
            Effect::Free(index) => {
                self.fp_index(index, INDEX);
                self.fg(TAG);
                self.num(3);
                self.fg(INDEX);
                self.num(1);
                self.op(0x74);
                self.op(0x74);
                self.op(0x72);
                self.fs(TAG);
            }
            Effect::LoadControl => {
                self.fg(MEMORY);
                self.fs(CONTROL);
                self.fp_summary(STATUS);
            }
            Effect::StoreControl | Effect::StoreStatus => {
                self.fp_memory_write(
                    Width::W16,
                    if instruction.effect == Effect::StoreControl {
                        CONTROL
                    } else {
                        STATUS
                    },
                );
            }
            Effect::ClearExceptions => {
                self.fg(STATUS);
                self.num(!0x80ff);
                self.op(0x71);
                self.fs(STATUS);
            }
            Effect::Initialize => {
                for local in 0..8 {
                    self.fc(
                        local,
                        match local {
                            CONTROL => 0x37f,
                            TAG => 0xffff,
                            _ => 0,
                        },
                    );
                }
            }
            Effect::Wait => {}
        }
        self.fp_location(instruction);
        if self.has_mmx {
            self.fp_flush();
            self.initialize_mmx();
        }
    }
}

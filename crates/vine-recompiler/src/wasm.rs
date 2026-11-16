//! Small direct Wasm encoder. Guest instructions never call a v86 helper.
use crate::ir::*;
use std::collections::BTreeMap;
mod affine;
mod control;
mod cpuid;
mod divide;
mod entries;
mod fp_ir;
mod function_cfg;
mod lazy_flags;
mod loop_memory;
mod loops;
mod masked_store;
mod mmx;
mod multiply;
mod preparation;
mod retirement;
mod rooted;
mod scopes;
mod shift_double;
mod sse;
mod string;
mod value;
mod x87;

/// Experimental code-quality switch. Both modes have identical CPU, memory,
/// exception and retirement contracts; adoption requires measured evidence.
#[derive(Clone, Copy, Debug, Default)]
pub struct CompileOptions {
    /// Partition static loop SCCs into private tail-call functions.
    pub function_cfg: bool,
    pub loop_memory: bool,
    pub structured_loops: bool,
    pub affine_memory: bool,
    pub fp_ir: bool,
    pub typed_numeric: bool,
    pub numeric_mode: crate::numeric_mode::NumericMode,
    pub value_ir: bool,
    pub predicate_ir: bool,
    pub flow_ir: bool,
    pub rooted_ssa: bool,
    /// Diagnostic-only atomic phase/PC stores to a separate imported memory.
    pub sampled_pc: bool,
    /// Keep an instance binding out of reusable code bytes.
    pub binding_global: bool,
    /// Read Rust-maintained protected PC bindings inside an admitted chain.
    pub direct_links: bool,
}

const FLAGS: u32 = 11;
const PC: u32 = 12;
const LEFT: u32 = 13;
const ROUTE: u32 = 14;
const A: u32 = 15;
const B: u32 = 16;
const R: u32 = 17;
const ADDR: u32 = 18;
const PTR: u32 = 19;
const PTR2: u32 = 20;
const ENTRY: u32 = 21;
const ENTRY2: u32 = 22;
const KIND: u32 = 23;
const FAULT: u32 = 24;
const ERROR: u32 = 25;
const DEFINED: u32 = 26;
const CARRY: u32 = 27;
const SMC: u32 = 28;
const TMP: u32 = 29;
const CF: u32 = 1;
const PF: u32 = 4;
const AF: u32 = 16;
const ZF: u32 = 64;
const SF: u32 = 128;
const OF: u32 = 2048;
const ARITH: u32 = CF | PF | AF | ZF | SF | OF;

fn uleb(out: &mut Vec<u8>, mut x: u32) {
    loop {
        let b = (x & 127) as u8;
        x >>= 7;
        out.push(b | if x == 0 { 0 } else { 128 });
        if x == 0 {
            break;
        }
    }
}
fn sleb(out: &mut Vec<u8>, mut x: i64) {
    loop {
        let b = (x & 127) as u8;
        x >>= 7;
        let last = (x == 0 && b & 64 == 0) || (x == -1 && b & 64 != 0);
        out.push(b | if last { 0 } else { 128 });
        if last {
            break;
        }
    }
}
fn name(out: &mut Vec<u8>, s: &str) {
    uleb(out, s.len() as u32);
    out.extend(s.as_bytes());
}
fn section(out: &mut Vec<u8>, id: u8, data: &[u8]) {
    out.push(id);
    uleb(out, data.len() as u32);
    out.extend(data);
}

#[derive(Clone)]
struct Emitter {
    local_map: Option<std::rc::Rc<Vec<u32>>>,
    function_cfg: Option<std::rc::Rc<function_cfg::Context>>,
    scope_plan: Option<std::rc::Rc<crate::control_layout::Plan>>,
    continuations: Option<std::rc::Rc<crate::return_ir::Plan>>,
    code: Vec<u8>,
    labels: Vec<String>,
    blocks: BTreeMap<u32, usize>,
    structured_header: Option<u32>,
    structured_body: Vec<u32>,
    affine: Vec<affine::Certificate>,
    loop_base: u32,
    loop_context: Option<loop_memory::Context>,
    loop_instruction: u32,
    division_scratch: u32,
    string_scratch: u32,
    rep_fuel: u32,
    link_enabled: u32,
    link_target: u32,
    has_mmx: bool,
    fp_base: u32,
    fp_plan: BTreeMap<u32, crate::fp_ir::Instruction>,
    fp_calls: BTreeMap<u32, u32>,
    fp_leaf: bool,
    typed_numeric: bool,
    sse_numeric: [u32; 10],
    scas_import: u32,
    unit_id: u32,
    value: Option<std::rc::Rc<value::Plan>>,
    value_exit: Option<value::ExitContext>,
    value_adapter: bool,
    coalesce_retirement: bool,
    retirement: Option<retirement::Position>,
    retirement_exits: BTreeMap<retirement::Position, u32>,
    flag_plan: BTreeMap<u32, crate::flag_ir::Recipe>,
    flag_admission: BTreeMap<u32, crate::flag_flow::Admission>,
    flag_saved: [u32; 4],
    flow_partial: bool,
    rooted: Option<std::rc::Rc<rooted::Plan>>,
    rooted_exit: Option<rooted::ExitContext>,
    sampled_pc: bool,
    binding_global: bool,
    direct_links: bool,
    dispatches: std::rc::Rc<BTreeMap<u32, crate::control_ir::TableDispatch>>,
}
impl Emitter {
    fn direct_root(&mut self) {
        self.op(0x23);
        uleb(&mut self.code, 1 + u32::from(self.binding_global));
    }
    fn resolve_link(&mut self) {
        if self.direct_links {
            self.op(0x42);
            sleb(&mut self.code, 0);
            self.set(self.link_target);
            self.direct_root();
            self.set(PTR);
            self.load(PTR, 0);
            self.if_(); // admitted synchronous invocation
            self.get(PTR);
            self.get(PC);
            self.num(12);
            self.op(0x76);
            self.num(2);
            self.op(0x74);
            self.op(0x6a);
            self.set(ENTRY);
            self.load(ENTRY, 16);
            self.tee(ENTRY);
            self.if_();
            self.get(ENTRY);
            self.get(PC);
            self.num(4095);
            self.op(0x71);
            self.num(3);
            self.op(0x74);
            self.op(0x6a);
            self.code.extend([0x29, 3, 0]); // aligned i64 binding
            self.set(self.link_target);
            self.end();
            self.end();
            self.get(self.link_target);
            self.op(0x50);
            self.if_();
            self.load(PTR, 12);
            self.num(1);
            self.op(0x6a);
            self.set(TMP);
            self.store(PTR, 12, TMP);
        }
        self.op(0x23);
        uleb(&mut self.code, 0); // backend handle
        self.get(PC);
        self.op(0x10);
        uleb(&mut self.code, 0);
        self.set(self.link_target);
        if self.direct_links {
            self.op(0x05); // else: direct binding hit
            self.load(PTR, 8);
            self.num(1);
            self.op(0x6a);
            self.set(TMP);
            self.store(PTR, 8, TMP);
            self.get(self.link_target);
            self.op(0x42);
            sleb(&mut self.code, 32);
            self.op(0x88);
            self.op(0xa7);
            self.set(TMP);
            self.store(PTR, 4, TMP); // final selected unit, consumed at outer end
            self.end();
        }
        self.get(self.link_target);
    }
    fn unit_identity(&mut self) {
        if self.binding_global {
            self.code.extend([0x23, 1]); // immutable env.unit, after env.backend
        } else {
            self.num(self.unit_id);
        }
    }
    fn sample_location(&mut self, phase: u32, pc: u32) {
        if !self.sampled_pc {
            return;
        }
        self.num(0);
        self.op(0x42);
        sleb(
            &mut self.code,
            ((u64::from(phase) << 32) | u64::from(pc)) as i64,
        );
        // i64.atomic.store, alignment=8, explicit memory index1, offset0.
        // The observer cannot tear the phase/PC pair. Memory0 stays private.
        self.code.extend([0xfe, 0x18, 0x43, 1, 0]);
    }
    fn op(&mut self, x: u8) {
        self.code.push(x);
    }
    fn num(&mut self, x: u32) {
        self.op(0x41);
        sleb(&mut self.code, i64::from(x as i32));
    }
    fn get(&mut self, x: u32) {
        let x = self.local_map.as_ref().map_or(x, |map| map[x as usize]);
        self.op(0x20);
        uleb(&mut self.code, x);
    }
    fn set(&mut self, x: u32) {
        let x = self.local_map.as_ref().map_or(x, |map| map[x as usize]);
        self.op(0x21);
        uleb(&mut self.code, x);
    }
    fn tee(&mut self, x: u32) {
        let x = self.local_map.as_ref().map_or(x, |map| map[x as usize]);
        self.op(0x22);
        uleb(&mut self.code, x);
    }
    fn mem(&mut self, op: u8, offset: u32) {
        self.op(op);
        uleb(&mut self.code, 0);
        uleb(&mut self.code, offset);
    }
    fn load(&mut self, base: u32, offset: u32) {
        self.get(base);
        self.mem(0x28, offset);
    }
    fn store(&mut self, base: u32, offset: u32, value: u32) {
        self.get(base);
        self.get(value);
        self.mem(0x36, offset);
    }
    fn open(&mut self, op: u8, label: String) {
        self.op(op);
        self.op(0x40);
        self.labels.push(label);
    }
    fn end(&mut self) {
        self.op(0x0b);
        self.labels.pop().expect("balanced labels");
    }
    fn branch(&mut self, label: &str) {
        if label == "exit" && self.retirement.is_some() {
            self.outline_retirement_exit();
            return;
        }
        if label == "exit" && self.private_exit() {
            return;
        }
        if label == "exit"
            && let Some(context) = self.value_exit
        {
            let plan = self.value.as_ref().expect("value exit has a plan").clone();
            plan.emit_cold_exit(self, context);
            return;
        }
        if label == "exit"
            && let Some(context) = self.rooted_exit
        {
            let plan = self
                .rooted
                .as_ref()
                .expect("rooted exit has a plan")
                .clone();
            plan.emit_cold_exit(self, context);
            return;
        }
        self.op(0x0c);
        let depth = self
            .labels
            .iter()
            .rev()
            .position(|x| x == label)
            .expect("branch label");
        uleb(&mut self.code, depth as u32);
    }
    fn if_(&mut self) {
        self.open(0x04, String::new());
    }
    fn exit(&mut self, kind: u32) {
        self.num(kind);
        self.set(KIND);
        self.branch("exit");
    }
    fn check(&mut self, kind: u32, address: u32, error: u32) {
        self.if_();
        self.get(address);
        self.set(FAULT);
        self.num(error);
        self.set(ERROR);
        self.exit(kind);
        self.end();
    }
    fn truncate(&mut self, width: Width) {
        if matches!(width, Width::W8 | Width::W16) {
            self.num((1u32 << width.bits()) - 1);
            self.op(0x71);
        }
    }
    fn effective(&mut self, m: MemoryOperand, segment: bool) {
        self.num(m.displacement);
        if let Some(r) = m.base {
            self.get(3 + u32::from(r));
            self.op(0x6a);
        }
        if let Some(r) = m.index {
            self.get(3 + u32::from(r));
            self.num(u32::from(m.scale));
            self.op(0x6c);
            self.op(0x6a);
        }
        if segment && m.segment == Segment::Fs {
            // Segment bounds apply to the effective offset, before base
            // addition. A wrapping base must not hide an invalid offset span.
            self.set(ADDR);
            self.get(ADDR);
            self.num(u32::MAX - (m.width.bytes() - 1));
            self.op(0x4b);
            self.check(6, ADDR, 0);
            self.get(ADDR);
            self.load(0, 44 + 4 * 16 + 4);
            self.op(0x6a);
            self.set(ADDR);
            // A valid FS operand whose linear span wraps needs split linear
            // translation; do not misreport that unimplemented case as #GP.
            self.get(ADDR);
            self.num(u32::MAX - (m.width.bytes() - 1));
            self.op(0x4b);
            self.if_();
            self.exit(3);
            self.end();
            self.get(ADDR);
        }
    }
    fn dirty_preflight(&mut self) {
        self.load(ENTRY, 8);
        self.num(1);
        self.op(0x47);
        self.set(TMP);
        self.get(ENTRY);
        self.get(ENTRY2);
        self.op(0x47);
        self.if_();
        self.load(ENTRY2, 8);
        self.num(1);
        self.op(0x47);
        self.get(TMP);
        self.op(0x6a);
        self.set(TMP);
        self.end();
        self.dirty_capacity();
    }
    // TMP is the number of new journal entries required by this instruction.
    fn dirty_capacity(&mut self) {
        self.get(1);
        self.num(16);
        self.op(0x6b);
        self.set(PTR);
        self.load(PTR, 4);
        self.load(PTR, 8);
        self.op(0x4b);
        self.check(10, ADDR, 0);
        self.get(TMP);
        self.load(PTR, 8);
        self.load(PTR, 4);
        self.op(0x6b);
        self.op(0x4b);
        self.check(10, ADDR, 0);
    }
    fn mark_dirty(&mut self) {
        if self.loop_mark_dirty() {
            return;
        }
        self.get(1);
        self.num(16);
        self.op(0x6b);
        self.set(TMP);
        for entry in [ENTRY, ENTRY2] {
            // ENTRY2 equals ENTRY for the ordinary one-page access; marking
            // the first entry makes the second check false automatically.
            self.mark_dirty_one(entry);
        }
    }
    // TMP holds the journal header. Capacity must be checked before bytes change.
    fn mark_dirty_one(&mut self, entry: u32) {
        self.load(entry, 8);
        self.num(1);
        self.op(0x47);
        self.if_();
        self.load(TMP, 0);
        self.load(TMP, 4);
        self.num(2);
        self.op(0x74);
        self.op(0x6a);
        self.get(entry);
        self.get(1);
        self.op(0x6b);
        self.num(4);
        self.op(0x76);
        self.mem(0x36, 0);
        self.get(TMP);
        self.load(TMP, 4);
        self.num(1);
        self.op(0x6a);
        self.mem(0x36, 4);
        self.get(entry);
        self.num(1);
        self.mem(0x36, 8);
        self.end();
    }
    // Check both endpoints before a write. Page metadata and pool offsets are
    // compiler-owned, and guest addresses can never name metadata directly.
    fn access(&mut self, width: Width, write: bool, stack: bool) {
        self.access_checked(width, write, stack, false);
    }
    fn access_checked(&mut self, width: Width, write: bool, stack: bool, rmw: bool) {
        self.access_span(width, write, stack, rmw, write && !rmw);
    }
    fn access_span(&mut self, width: Width, write: bool, stack: bool, rmw: bool, dirty: bool) {
        if self.loop_access(width, write) {
            return;
        }
        let bytes = width.bytes();
        self.get(ADDR);
        self.num(u32::MAX - (bytes - 1));
        self.op(0x4b);
        self.check(if stack { 9 } else { 6 }, ADDR, 0);
        self.get(1);
        self.get(ADDR);
        self.num(12);
        self.op(0x76);
        self.num(4);
        self.op(0x74);
        self.op(0x6a);
        self.set(ENTRY);
        let need = 8 | if rmw {
            3
        } else if write {
            2
        } else {
            1
        };
        self.load(ENTRY, 4);
        self.num(need);
        self.op(0x71);
        self.num(need);
        self.op(0x47);
        // x86 P bit is taken from the mapping, below, rather than assumed.
        self.if_();
        self.get(ADDR);
        self.set(FAULT);
        self.load(ENTRY, 4);
        self.num(3);
        self.op(0x76);
        self.num(1);
        self.op(0x71);
        self.num(if write { 6 } else { 4 });
        self.op(0x72);
        self.set(ERROR);
        self.exit(4);
        self.end();
        self.get(ADDR);
        self.num(bytes - 1);
        self.op(0x6a);
        self.set(TMP);
        self.get(1);
        self.get(TMP);
        self.num(12);
        self.op(0x76);
        self.num(4);
        self.op(0x74);
        self.op(0x6a);
        self.set(ENTRY2);
        self.load(ENTRY2, 4);
        self.num(need);
        self.op(0x71);
        self.num(need);
        self.op(0x47);
        self.if_();
        self.get(TMP);
        self.num(0xffff_f000);
        self.op(0x71);
        self.set(FAULT);
        self.load(ENTRY2, 4);
        self.num(3);
        self.op(0x76);
        self.num(1);
        self.op(0x71);
        self.num(if write { 6 } else { 4 });
        self.op(0x72);
        self.set(ERROR);
        self.exit(4);
        self.end();
        if dirty {
            self.dirty_preflight();
        }
        self.load(ENTRY, 0);
        self.get(ADDR);
        self.num(4095);
        self.op(0x71);
        self.op(0x6a);
        self.set(PTR);
        self.load(ENTRY2, 0);
        self.set(PTR2);
    }
    fn read_memory(&mut self, width: Width, stack: bool) {
        self.access(width, false, stack);
        self.read_prepared_memory(width);
    }
    fn read_prepared_memory(&mut self, width: Width) {
        if self.loop_context.is_some() {
            self.get(PTR);
            self.mem(
                match width {
                    Width::W8 => 0x2d,
                    Width::W16 => 0x2f,
                    Width::W32 => 0x28,
                    _ => unreachable!("wide modules own loads"),
                },
                0,
            );
            return;
        }
        self.get(ENTRY);
        self.get(ENTRY2);
        self.op(0x46);
        self.if_();
        self.get(PTR);
        self.mem(
            match width {
                Width::W8 => 0x2d,
                Width::W16 => 0x2f,
                Width::W32 => 0x28,
                Width::W64 | Width::W80 | Width::W128 => unreachable!("wide modules own loads"),
            },
            0,
        );
        self.set(R);
        self.op(0x05);
        self.num(0);
        self.set(R);
        for n in 0..width.bytes() {
            self.byte_pointer(n);
            self.mem(0x2d, 0);
            self.num(n * 8);
            self.op(0x74);
            self.get(R);
            self.op(0x72);
            self.set(R);
        }
        self.end();
        self.get(R);
    }
    fn byte_pointer(&mut self, n: u32) {
        self.get(ADDR);
        self.num(n);
        self.op(0x6a);
        self.tee(TMP);
        self.num(4095);
        self.op(0x71);
        self.set(TMP);
        self.get(ADDR);
        self.num(4095);
        self.op(0x71);
        self.num(n);
        self.op(0x6a);
        self.num(4096);
        self.op(0x49);
        self.op(0x04);
        self.op(0x7f);
        self.load(ENTRY, 0);
        self.op(0x05);
        self.get(PTR2);
        self.op(0x0b);
        self.get(TMP);
        self.op(0x6a);
    }
    fn write_memory(&mut self, width: Width) {
        // Call only after access(write) succeeds, with result in R.
        if self.loop_context.is_some() {
            self.get(PTR);
            self.get(R);
            self.mem(
                match width {
                    Width::W8 => 0x3a,
                    Width::W16 => 0x3b,
                    Width::W32 => 0x36,
                    _ => unreachable!("wide modules own stores"),
                },
                0,
            );
            self.mark_dirty();
            return;
        }
        self.get(ENTRY);
        self.get(ENTRY2);
        self.op(0x46);
        self.if_();
        self.get(PTR);
        self.get(R);
        self.mem(
            match width {
                Width::W8 => 0x3a,
                Width::W16 => 0x3b,
                Width::W32 => 0x36,
                Width::W64 | Width::W80 | Width::W128 => unreachable!("wide modules own stores"),
            },
            0,
        );
        self.op(0x05);
        for n in 0..width.bytes() {
            self.byte_pointer(n);
            self.get(R);
            self.num(n * 8);
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
    fn operand(&mut self, o: Operand) {
        match o {
            Operand::Imm(n) => self.num(n),
            Operand::Reg(r) => {
                self.get(3 + u32::from(r.index));
                if r.high8 {
                    self.num(8);
                    self.op(0x76);
                }
                self.truncate(r.width);
            }
            Operand::Mem(m) => {
                if !self.affine_read(m) {
                    self.effective(m, true);
                    self.set(ADDR);
                    self.read_memory(m.width, m.segment == Segment::Ss);
                }
            }
            Operand::Mmx(_) | Operand::Xmm(_) => unreachable!("vector modules own operands"),
        }
    }
    fn prepare_store(&mut self, o: Operand) {
        if let Operand::Mem(m) = o
            && !self.affine_store(m)
        {
            self.effective(m, true);
            self.set(ADDR);
            self.access(m.width, true, m.segment == Segment::Ss);
        }
    }
    fn rmw_operand(&mut self, operand: Operand) {
        if let Operand::Mem(memory) = operand {
            if !self.affine_rmw_read(memory) {
                self.effective(memory, true);
                self.set(ADDR);
                // A memory destination is one RMW access, including a masked
                // zero shift. Check its whole span with write-fault semantics
                // before reading or exposing any architectural mutation.
                self.access_checked(memory.width, true, memory.segment == Segment::Ss, true);
                self.read_prepared_memory(memory.width);
            }
        } else {
            self.operand(operand);
        }
    }
    fn prepare_rmw_store(&mut self, operand: Operand) {
        if matches!(operand, Operand::Mem(_)) {
            // The source register/immediate does not disturb the checked
            // address entries. Delay capacity until an actual store; count
            // zero must not manufacture a resource failure or dirty page.
            self.dirty_preflight();
            self.load(ENTRY, 0);
            self.get(ADDR);
            self.num(4095);
            self.op(0x71);
            self.op(0x6a);
            self.set(PTR);
        }
    }
    fn assign(&mut self, o: Operand) {
        match o {
            Operand::Reg(r) => {
                if r.width == Width::W32 {
                    self.get(R);
                    self.set(3 + u32::from(r.index));
                } else {
                    let shift = if r.high8 { 8 } else { 0 };
                    let mask = ((1u32 << r.width.bits()) - 1) << shift;
                    self.get(3 + u32::from(r.index));
                    self.num(!mask);
                    self.op(0x71);
                    self.get(R);
                    self.truncate(r.width);
                    self.num(shift);
                    self.op(0x74);
                    self.op(0x72);
                    self.set(3 + u32::from(r.index));
                }
            }
            Operand::Mem(m) => self.write_memory(m.width),
            Operand::Imm(_) => unreachable!("validated destination"),
            Operand::Mmx(_) | Operand::Xmm(_) => unreachable!("vector modules own destinations"),
        }
    }
    fn flag_bit(&mut self, mask: u32) {
        self.get(FLAGS);
        self.num(mask);
        self.op(0x71);
        self.op(0x45);
        self.op(0x45);
    }
    fn condition(&mut self, c: Condition) {
        use Condition::*;
        match c {
            O | No => self.flag_bit(OF),
            B | Ae => self.flag_bit(CF),
            E | Ne => self.flag_bit(ZF),
            S | Ns => self.flag_bit(SF),
            P | Np => self.flag_bit(PF),
            Be | A => {
                self.flag_bit(CF);
                self.flag_bit(ZF);
                self.op(0x72);
            }
            L | Ge => {
                self.flag_bit(SF);
                self.flag_bit(OF);
                self.op(0x73);
            }
            Le | G => {
                self.flag_bit(SF);
                self.flag_bit(OF);
                self.op(0x73);
                self.flag_bit(ZF);
                self.op(0x72);
            }
        }
        if matches!(c, No | Ae | Ne | A | Ns | Np | Ge | G) {
            self.op(0x45);
        }
    }
    fn add_flag(&mut self, bit: u32) {
        self.num(bit.trailing_zeros());
        self.op(0x74);
        self.get(FLAGS);
        self.op(0x72);
        self.set(FLAGS);
    }
    fn common_flags(&mut self, width: Width, mask: u32) {
        self.common_flags_using(width, mask, lazy_flags::Inputs::Scratch);
    }
    fn common_flags_using(&mut self, width: Width, mask: u32, inputs: lazy_flags::Inputs) {
        if mask & ZF != 0 {
            self.flag_input(inputs, 2);
            self.op(0x45);
            self.add_flag(ZF);
        }
        if mask & SF != 0 {
            self.flag_input(inputs, 2);
            self.num(width.bits() - 1);
            self.op(0x76);
            self.num(1);
            self.op(0x71);
            self.add_flag(SF);
        }
        if mask & PF != 0 {
            self.flag_input(inputs, 2);
            self.num(255);
            self.op(0x71);
            self.op(0x69);
            self.num(1);
            self.op(0x71);
            self.op(0x45);
            self.add_flag(PF);
        }
    }
    fn arithmetic_flags(&mut self, op: Opcode, width: Width, mask: u32) {
        self.arithmetic_flags_using(op, width, mask, lazy_flags::Inputs::Scratch);
    }
    fn arithmetic_flags_using(
        &mut self,
        op: Opcode,
        width: Width,
        mask: u32,
        inputs: lazy_flags::Inputs,
    ) {
        let subtract = matches!(
            op,
            Opcode::Sub | Opcode::Cmp | Opcode::Sbb | Opcode::Dec | Opcode::Neg
        );
        self.get(FLAGS);
        self.num(!mask);
        self.op(0x71);
        self.set(FLAGS);
        self.common_flags_using(width, mask, inputs);
        if mask & CF != 0 {
            if matches!(op, Opcode::Adc | Opcode::Sbb) {
                self.flag_input(inputs, 0);
                self.op(0xad);
                self.flag_input(inputs, 1);
                self.op(0xad);
                self.flag_input(inputs, 3);
                self.op(0xad);
                self.op(0x7c);
                if subtract {
                    self.op(0x54);
                } else {
                    self.op(0x7c);
                    self.op(0x42);
                    sleb(&mut self.code, ((1u64 << width.bits()) - 1) as i64);
                    self.op(0x56);
                }
            } else if subtract {
                self.flag_input(inputs, 0);
                self.flag_input(inputs, 1);
                self.op(0x49);
            } else {
                self.flag_input(inputs, 2);
                self.flag_input(inputs, 0);
                self.op(0x49);
            }
            self.add_flag(CF);
        }
        if mask & AF != 0 {
            self.flag_input(inputs, 0);
            self.flag_input(inputs, 1);
            self.op(0x73);
            self.flag_input(inputs, 2);
            self.op(0x73);
            self.num(4);
            self.op(0x76);
            self.num(1);
            self.op(0x71);
            self.add_flag(AF);
        }
        if mask & OF != 0 {
            self.flag_input(inputs, 0);
            self.flag_input(inputs, 1);
            self.op(0x73);
            if !subtract {
                self.num(u32::MAX);
                self.op(0x73);
            }
            self.flag_input(inputs, 0);
            self.flag_input(inputs, 2);
            self.op(0x73);
            self.op(0x71);
            self.num(width.bits() - 1);
            self.op(0x76);
            self.num(1);
            self.op(0x71);
            self.add_flag(OF);
        }
    }
    fn retire(&mut self, instruction: &Instruction) {
        let call = instruction.opcode == Opcode::Call;
        if let Some(position) = &mut self.retirement {
            position.pc = instruction.next_eip();
            position.call_target = call;
            position.completed += 1;
        } else {
            if call {
                self.get(B);
            } else {
                self.num(instruction.next_eip());
            }
            self.set(PC);
            self.get(LEFT);
            self.num(1);
            self.op(0x6b);
            self.set(LEFT);
        }
        if self.loop_context.is_some() {
            return;
        }
        if self.retirement.is_some() && !retirement::may_invalidate_code(instruction) {
            return;
        }
        self.get(SMC);
        self.if_();
        self.exit(7);
        self.end();
    }
    fn jump(&mut self, target: u32) {
        if self.flow_partial {
            self.num(target);
            self.set(PC);
            let plan = self
                .value
                .as_ref()
                .expect("flow partial continuation")
                .clone();
            plan.flow_partial_jump(self, target);
            return;
        }
        if self.loop_jump(target) {
            return;
        }
        self.num(target);
        self.set(PC);
        if self.rooted.is_some() {
            self.get(LEFT);
            self.op(0x45);
            self.if_();
            self.exit(1);
            self.end();
            self.exit(2);
            return;
        }
        if self.structured_header == Some(target) {
            self.branch(&format!("loop{target}"));
            return;
        }
        if self.scope_jump(target) {
            return;
        }
        if self.structured_body.contains(&target) {
            self.branch(&format!("s{target}"));
            return;
        }
        if let Some(&index) = self.blocks.get(&target) {
            let label = format!("b{index}");
            if self.labels.contains(&label) {
                self.branch(&label);
            } else {
                self.num(index as u32);
                self.set(ROUTE);
                self.branch("dispatch");
            }
        } else if !self.private_jump(target) {
            self.exit(2);
        }
    }
    fn indirect(&mut self) {
        if self.flow_partial {
            let plan = self
                .value
                .as_ref()
                .expect("flow partial continuation")
                .clone();
            plan.flow_partial_indirect(self);
            return;
        }
        if self.rooted.is_some() {
            self.get(LEFT);
            self.op(0x45);
            self.if_();
            self.exit(1);
            self.end();
            self.exit(2);
            return;
        }
        if self.function_cfg.is_some() {
            self.num(u32::MAX);
            self.set(ROUTE);
        }
        self.branch("lookup");
    }
    fn lookup_tree(&mut self, entries: &[(u32, usize)]) {
        if entries.is_empty() {
            return;
        }
        let mid = entries.len() / 2;
        let (pc, index) = entries[mid];
        self.get(PC);
        self.num(pc);
        self.op(0x46);
        self.if_();
        self.num(index as u32);
        self.set(ROUTE);
        self.op(0x05);
        if entries.len() > 1 {
            self.get(PC);
            self.num(pc);
            self.op(0x49);
            self.if_();
            self.lookup_tree(&entries[..mid]);
            self.op(0x05);
            self.lookup_tree(&entries[mid + 1..]);
            self.end();
        }
        self.end();
    }
    fn instruction(&mut self, i: &Instruction, needed_flags: Option<u32>) {
        self.loop_instruction = i.eip;
        if let Some(position) = &mut self.retirement {
            position.pc = i.eip;
            position.call_target = false;
        }
        if needed_flags.is_none() {
            self.get(LEFT);
            let completed = self.retirement.map_or(0, |position| position.completed);
            if completed == 0 {
                self.op(0x45);
            } else {
                self.num(completed);
                self.op(0x46);
            }
            self.if_();
            self.exit(1);
            self.end();
        }
        if self.retirement.is_none() {
            self.num(i.eip);
            self.set(PC);
        }
        let operands = &i.operands;
        let width = i.width;
        use Opcode::*;
        if i.opcode == DiagnosticStop {
            self.exit(crate::state::exit_kind::DIAGNOSTIC_STOP);
            return;
        }
        if i.opcode == Rdtsc {
            self.num(u32::from(i.len));
            self.set(ERROR);
            self.exit(crate::state::exit_kind::TIMESTAMP_COUNTER);
            return;
        }
        if i.opcode == Ud2 {
            self.exit(8);
            return;
        }
        if i.opcode == Unsupported {
            if matches!(
                i.unsupported,
                Some(UnsupportedReason::FetchUnavailable | UnsupportedReason::TruncatedInstruction)
            ) {
                for n in 0..15 {
                    let Some(at) = i.eip.checked_add(n) else {
                        self.exit(6);
                        break;
                    };
                    self.num(at);
                    self.set(ADDR);
                    self.get(1);
                    self.num((at >> 12) * 16);
                    self.op(0x6a);
                    self.set(ENTRY);
                    self.load(ENTRY, 4);
                    self.num(8 | 4 | 16);
                    self.op(0x71);
                    self.num(8 | 4 | 16);
                    self.op(0x47);
                    self.if_();
                    self.get(ADDR);
                    self.set(FAULT);
                    self.load(ENTRY, 4);
                    self.num(3);
                    self.op(0x76);
                    self.num(1);
                    self.op(0x71);
                    self.num(20);
                    self.op(0x72);
                    self.set(ERROR);
                    self.exit(4);
                    self.end();
                }
            }
            self.exit(
                if i.unsupported == Some(UnsupportedReason::InvalidInstruction) {
                    8
                } else {
                    3
                },
            );
            return;
        }
        self.prepare_lazy_flags(i);
        let dynamic_defined = matches!(
            i.opcode,
            Shl | Shr | Sar | Shld | Shrd | String(StringOpcode::Scas | StringOpcode::Cmps, _)
        );
        if dynamic_defined {
            self.commit_defined_flags();
        }
        // All access checks precede register/flags/stack mutation for this instruction.
        match i.opcode {
            Mmx(op) => {
                self.fp_flush();
                self.mmx_instruction(op, operands);
                if !self.fp_plan.is_empty() {
                    self.flush_mmx();
                    self.fp_initialize();
                }
            }
            Sse(op) => self.sse_instruction(op, operands),
            String(op, repeat) => self.string_instruction(i, op, repeat),
            Nop => {}
            Cld | Std => {
                self.get(FLAGS);
                self.num(if i.opcode == Cld { !0x400 } else { 0x400 });
                self.op(if i.opcode == Cld { 0x71 } else { 0x72 });
                self.set(FLAGS);
            }
            Sahf => {
                let mask = needed_flags.unwrap_or(0xd5) & 0xd5;
                if mask != 0 {
                    self.get(FLAGS);
                    self.num(!mask);
                    self.op(0x71);
                    self.get(3); // EAX remains unchanged; AH is an implicit input.
                    self.num(8);
                    self.op(0x76);
                    self.num(mask);
                    self.op(0x71);
                    self.op(0x72);
                    self.set(FLAGS);
                }
            }
            Cpuid => self.cpuid_instruction(),
            Xchg => {
                // Snapshot both sources before either destination changes.
                // This also handles overlapping AH/AL and identical registers.
                self.operand(operands[0]);
                self.set(A);
                self.operand(operands[1]);
                self.set(R);
                self.assign(operands[0]);
                self.get(A);
                self.set(R);
                self.assign(operands[1]);
            }
            SignExtendPair => {
                self.get(3);
                self.sign_extend(width);
                self.num(31);
                self.op(0x75);
                self.set(R);
                self.assign(Operand::Reg(Register {
                    index: 2,
                    width,
                    high8: false,
                }));
            }
            Mov | Movzx | Movsx | Lea => {
                if i.opcode == Lea {
                    if let Operand::Mem(m) = operands[1] {
                        self.effective(m, false);
                    } else {
                        unreachable!();
                    }
                } else {
                    self.operand(operands[1]);
                    if i.opcode == Movsx {
                        match operands[1] {
                            Operand::Reg(r) => self.sign_extend(r.width),
                            Operand::Mem(m) => self.sign_extend(m.width),
                            _ => {}
                        }
                    }
                }
                self.set(R);
                self.prepare_store(operands[0]);
                self.assign(operands[0]);
            }
            Add | Adc | Sub | Sbb | Cmp | Test | And | Or | Xor | Inc | Dec | Neg | Not => {
                if matches!(i.opcode, Cmp | Test) {
                    self.operand(operands[0]);
                } else {
                    self.rmw_operand(operands[0]);
                }
                self.set(A);
                if matches!(i.opcode, Inc | Dec) {
                    self.num(1);
                } else if matches!(i.opcode, Neg | Not) {
                    self.num(0);
                } else {
                    self.operand(operands[1]);
                }
                self.truncate(width);
                self.set(B);
                if i.opcode == Neg {
                    self.get(A);
                    self.set(B);
                    self.num(0);
                    self.set(A);
                }
                if !matches!(i.opcode, Cmp | Test) {
                    self.prepare_rmw_store(operands[0]);
                }
                self.flag_bit(CF);
                self.set(CARRY);
                self.get(A);
                if i.opcode == Not {
                    self.num(u32::MAX);
                    self.op(0x73);
                } else {
                    self.get(B);
                    self.op(match i.opcode {
                        Add | Adc | Inc => 0x6a,
                        Sub | Sbb | Cmp | Dec | Neg => 0x6b,
                        And | Test => 0x71,
                        Or => 0x72,
                        Xor => 0x73,
                        _ => unreachable!(),
                    });
                    if matches!(i.opcode, Adc | Sbb) {
                        self.get(CARRY);
                        self.op(if i.opcode == Adc { 0x6a } else { 0x6b });
                    }
                }
                self.truncate(width);
                self.set(R);
                if i.opcode != Not {
                    self.ordinary_flags(i, needed_flags.unwrap_or(i.flags_written));
                }
                if !matches!(i.opcode, Cmp | Test) {
                    self.assign(operands[0]);
                }
            }
            Shl | Shr | Sar => {
                self.rmw_operand(operands[0]);
                self.set(A);
                self.operand(operands[1]);
                self.num(31);
                self.op(0x71);
                self.set(B);
                self.get(B);
                self.if_();
                self.prepare_rmw_store(operands[0]);
                self.get(A);
                if i.opcode == Sar {
                    self.sign_extend(width);
                }
                self.get(B);
                self.op(match i.opcode {
                    Shl => 0x74,
                    Shr => 0x76,
                    _ => 0x75,
                });
                self.truncate(width);
                self.set(R);
                self.get(FLAGS);
                self.num(!ARITH);
                self.op(0x71);
                self.set(FLAGS);
                self.common_flags(width, ARITH);
                self.get(A);
                if i.opcode == Sar {
                    self.sign_extend(width);
                }
                self.get(B);
                self.num(1);
                self.op(0x6b);
                self.op(if i.opcode == Shl {
                    0x74
                } else if i.opcode == Sar {
                    0x75
                } else {
                    0x76
                });
                self.num(if i.opcode == Shl { width.bits() - 1 } else { 0 });
                self.op(0x76);
                self.num(1);
                self.op(0x71);
                self.add_flag(CF);
                if i.opcode == Shr {
                    self.get(A);
                    self.num(width.bits() - 1);
                    self.op(0x76);
                    self.add_flag(OF);
                } else if i.opcode == Shl {
                    self.get(R);
                    self.num(width.bits() - 1);
                    self.op(0x76);
                    self.flag_bit(CF);
                    self.op(0x73);
                    self.add_flag(OF);
                }
                self.assign(operands[0]);
                self.get(DEFINED);
                self.num(!ARITH);
                self.op(0x71);
                self.num(PF | ZF | SF);
                self.op(0x72);
                self.set(DEFINED);
                if i.opcode == Sar {
                    self.num(1);
                } else {
                    self.get(B);
                    self.num(width.bits());
                    self.op(0x49);
                }
                self.if_();
                self.get(DEFINED);
                self.num(CF);
                self.op(0x72);
                self.set(DEFINED);
                self.end();
                self.get(B);
                self.num(1);
                self.op(0x46);
                self.if_();
                self.get(DEFINED);
                self.num(OF);
                self.op(0x72);
                self.set(DEFINED);
                self.end();
                self.end();
            }
            Div | Idiv => self.divide(i),
            Mul | ImulWide => self.wide_multiply(i),
            Shld => self.shift_double_left(i),
            Shrd => self.shift_double_right(i),
            Imul => {
                if operands.len() < 2 {
                    self.exit(3);
                    return;
                }
                self.operand(operands[if operands.len() == 3 { 1 } else { 0 }]);
                self.sign_extend(width);
                self.set(A);
                self.operand(operands[if operands.len() == 3 { 2 } else { 1 }]);
                self.sign_extend(width);
                self.set(B);
                self.get(A);
                self.get(B);
                self.op(0x6c);
                self.truncate(width);
                self.set(R);
                self.get(FLAGS);
                self.num(!(CF | OF));
                self.op(0x71);
                self.set(FLAGS);
                self.get(A);
                self.op(0xac);
                self.get(B);
                self.op(0xac);
                self.op(0x7e);
                self.get(R);
                self.sign_extend(width);
                self.op(0xac);
                self.op(0x52);
                self.tee(CARRY);
                self.add_flag(CF);
                self.get(CARRY);
                self.add_flag(OF);
                self.assign(operands[0]);
            }
            Push | Call => {
                self.operand(operands[0]);
                self.set(B);
                self.get(7);
                self.num(if i.opcode == Call { 4 } else { width.bytes() });
                self.op(0x6b);
                self.set(ADDR);
                self.access(
                    if i.opcode == Call { Width::W32 } else { width },
                    true,
                    true,
                );
                if i.opcode == Call {
                    self.num(i.next_eip());
                } else {
                    self.get(B);
                }
                self.set(R);
                self.write_memory(if i.opcode == Call { Width::W32 } else { width });
                self.get(ADDR);
                self.set(7);
            }
            Pop => {
                // POP r/m computes a stack-based destination using the advanced ESP.
                if !matches!(operands[0], Operand::Reg(_)) {
                    self.exit(3);
                    return;
                }
                self.get(7);
                self.set(ADDR);
                self.read_memory(width, true);
                self.set(R);
                self.get(7);
                self.num(width.bytes());
                self.op(0x6a);
                self.set(7);
                self.assign(operands[0]);
            }
            Leave => {
                // Stack address size is 32 at admission, independently of the
                // operand size. A failed pop preserves the original ESP/EBP.
                self.get(8);
                self.set(ADDR);
                self.read_memory(width, true);
                self.set(R);
                self.get(ADDR);
                self.num(width.bytes());
                self.op(0x6a);
                self.set(7);
                self.assign(Operand::Reg(Register {
                    index: 5,
                    width,
                    high8: false,
                }));
            }
            Ret => {
                self.get(7);
                self.set(ADDR);
                self.read_memory(Width::W32, true);
                self.set(B);
                self.get(7);
                self.num(
                    4 + match operands.first() {
                        Some(Operand::Imm(n)) => *n,
                        _ => 0,
                    },
                );
                self.op(0x6a);
                self.set(7);
            }
            Cmovcc => {
                // Intel SDM Vol.2A CMOVcc reads its source unconditionally,
                // including memory faults when the condition is false.
                self.operand(operands[1]);
                self.set(R);
                self.condition(i.condition.expect("condition"));
                self.if_();
                self.assign(operands[0]);
                self.end();
            }
            Setcc => {
                self.condition(i.condition.expect("condition"));
                self.set(R);
                self.prepare_store(operands[0]);
                self.assign(operands[0]);
            }
            Jmp => {
                self.operand(operands[0]);
                self.set(B);
            }
            Loop => {
                // Intel LOOP uses address-size ECX, wraps on decrement, and
                // preserves every flag. Control16/Address16 are not admitted.
                self.get(4);
                self.num(1);
                self.op(0x6b);
                self.set(4);
            }
            Jcc | Jecxz => {}
            X87 {
                descriptor,
                fop,
                segment,
            } => {
                if let Some(plan) = self.fp_plan.get(&i.eip).copied() {
                    self.fp_call(plan);
                } else {
                    self.x87_instruction(
                        descriptor,
                        fop,
                        segment,
                        operands.first().copied(),
                        i.eip,
                    );
                }
            }
            X87StatusAx => {
                if self.fp_plan.is_empty() {
                    self.x87_status_ax();
                } else {
                    self.fp_status_ax();
                }
            }
            Ud2 | Rdtsc | DiagnosticStop | Unsupported => unreachable!(),
        }
        if !dynamic_defined {
            self.define_flags(i.flags_undefined, i.flags_written);
        }
        self.invalidate_affine(crate::optimize::written_gprs(i));
        if self.value_adapter {
            return;
        }
        self.retire(i);
        if matches!(i.opcode, Jcc | Loop | Jecxz | Jmp | Call | Ret) {
            // Both conditional edges must see the same completed prefix.
            self.commit_retirement();
        }
        match i.opcode {
            Jcc | Loop | Jecxz => {
                if i.opcode == Jcc {
                    self.condition(i.condition.expect("condition"));
                } else {
                    self.get(4);
                    self.num(0);
                    self.op(if i.opcode == Jecxz { 0x46 } else { 0x47 });
                    if let Some(condition) = i.condition {
                        self.condition(condition);
                        self.op(0x71);
                    }
                }
                self.if_();
                if let Operand::Imm(t) = operands[0] {
                    self.jump(t);
                }
                self.end();
                self.jump(i.next_eip());
            }
            Jmp | Call => {
                if let Operand::Imm(t) = operands[0] {
                    self.jump(t);
                } else {
                    self.get(B);
                    self.set(PC);
                    if !self.table_dispatch(i) {
                        self.indirect();
                    }
                }
            }
            Ret => {
                self.get(B);
                self.set(PC);
                if let Some(plan) = self.continuations.clone() {
                    for &target in plan.guarded(i.eip) {
                        self.get(PC);
                        self.num(target);
                        self.op(0x46);
                        self.if_();
                        self.jump(target);
                        self.end();
                    }
                }
                self.indirect();
            }
            _ => {}
        }
    }
    fn sign_extend(&mut self, width: Width) {
        match width {
            Width::W8 => self.op(0xc0),
            Width::W16 => self.op(0xc1),
            Width::W32 => {}
            Width::W64 | Width::W80 | Width::W128 => unreachable!("scalar sign extension width"),
        }
    }

    fn block_body(&mut self, block: &Block) {
        self.sample_location(1, block.start);
        // Admission covers this block and any proven successor overwrite.
        // Otherwise every instruction retains exact partial-budget flags.
        let (budget, needed) = self.flag_admission.get(&block.start).map_or_else(
            || {
                (
                    block.instructions.len() as u32,
                    crate::optimize::flags_needed_after(block),
                )
            },
            |plan| (plan.budget, plan.needed.clone()),
        );
        let optimize = block
            .instructions
            .iter()
            .zip(&needed)
            .any(|(i, &mask)| mask != i.flags_written);
        if optimize {
            self.get(LEFT);
            self.num(budget);
            self.op(0x4f);
            self.if_();
            self.begin_retirement(block.start);
            for (instruction, mask) in block.instructions.iter().zip(needed) {
                self.instruction(instruction, Some(mask));
            }
            self.commit_retirement();
            self.block_fallthrough(block);
            self.op(0x05);
        }
        self.begin_retirement(block.start);
        for instruction in &block.instructions {
            self.instruction(instruction, None);
        }
        self.commit_retirement();
        self.block_fallthrough(block);
        if optimize {
            self.end();
        }
    }

    fn block_fallthrough(&mut self, block: &Block) {
        if let Some(last) = block.instructions.last() {
            self.jump(last.next_eip());
        } else {
            self.exit(3);
        }
    }

    fn structured_loop(&mut self, region: &Region, plan: &loops::StructuredLoop) {
        self.structured_header = Some(plan.header);
        self.structured_body.clone_from(&plan.body);
        self.open(0x03, format!("loop{}", plan.header));
        for &pc in plan.body.iter().skip(1).rev() {
            self.open(0x02, format!("s{pc}"));
        }
        for (index, &pc) in plan.body.iter().enumerate() {
            if index > 0 {
                self.end();
            }
            self.block_body(&region.blocks[&pc]);
        }
        self.end();
        self.structured_header = None;
        self.structured_body.clear();
    }
}

impl Emitter {
    fn finish(
        &mut self,
        has_mmx: bool,
        has_sse: bool,
        has_scas: bool,
        has_timestamp: bool,
        numeric_imports: u32,
    ) {
        // Vector aliases are shared architectural memory at this first link ABI.
        // GPRs, flags, budget and REP progress never round-trip through CPU memory.
        self.sample_location(5, 0);
        if has_mmx {
            self.flush_mmx();
        }
        if has_sse {
            self.flush_sse();
        }
        self.fp_flush();
        // Compilation ownership must not change an exhausted instruction budget
        // into an external-PC request. No target fetch/admission is needed until
        // the next slicself. Actual faults and completed SMC exits keep precedencself.
        self.get(KIND);
        self.num(crate::state::exit_kind::OUTSIDE_REGION);
        self.op(0x46);
        self.get(LEFT);
        self.op(0x45);
        self.op(0x71);
        self.if_();
        self.num(crate::state::exit_kind::BUDGET);
        self.set(KIND);
        self.end();
        self.get(KIND);
        self.num(2);
        self.op(0x46);
        self.get(LEFT);
        self.num(0);
        self.op(0x47);
        self.op(0x71);
        self.get(self.link_enabled);
        self.op(0x71);
        self.if_();
        if self.direct_links {
            self.resolve_link();
        } else {
            // Preserve the ordinary module bytes when the experiment is off.
            self.op(0x23);
            uleb(&mut self.code, 0);
            self.get(PC);
            self.op(0x10);
            uleb(&mut self.code, 0);
            self.tee(self.link_target);
        }
        self.op(0x50); // i64.eqz
        self.op(0x45); // i32.eqz
        self.if_();
        for local in 0..=13 {
            self.get(local);
        }
        self.get(DEFINED);
        self.get(self.link_enabled);
        self.get(self.link_target);
        self.op(0x42);
        sleb(&mut self.code, 32);
        self.op(0x88); // i64.shr_u
        self.op(0xa7); // i32.wrap_i64: target identity
        self.get(self.rep_fuel);
        self.get(self.link_target);
        self.op(0xa7);
        self.num(1);
        self.op(0x6b); // slot+1 -> table index
        self.op(0x13); // return_call_indirect: bounded host stack, no JS boundary
        uleb(&mut self.code, 0); // uniform link type
        uleb(&mut self.code, 0); // shared table
        self.end();
        self.end();
        for n in 0..8 {
            self.store(0, n * 4, 3 + n);
        }
        self.store(0, 32, PC);
        self.store(0, 36, FLAGS);
        self.store(0, 40, DEFINED);
        self.store(0, 400, KIND);
        self.store(0, 404, PC);
        self.store(0, 408, FAULT);
        self.store(0, 412, ERROR);
        self.get(2);
        self.get(LEFT);
        self.op(0x6b);
        self.set(R);
        self.store(0, 416, R);
        if has_scas {
            // The finalizer alone restores instruction-entry flags on a REP fault.
            // Architectural progress has already been published, and no subsequent
            // guest instruction or host callback can observe the intermediate flags.
            self.op(0x23);
            uleb(&mut self.code, 0);
            self.op(0x10);
            uleb(&mut self.code, self.scas_import + 2);
            self.if_();
            self.num(crate::state::exit_kind::UNSUPPORTED);
            self.set(KIND);
            self.store(0, 400, KIND);
            self.end();
        }
        if has_timestamp {
            // Only after the exact CPU exit is public may Rust bind a one-use
            // completion ticket to that context. The clock remains in Runtimself.
            self.get(KIND);
            self.num(crate::state::exit_kind::TIMESTAMP_COUNTER);
            self.op(0x46);
            self.if_();
            self.op(0x23);
            uleb(&mut self.code, 0);
            self.unit_identity();
            self.op(0x10);
            uleb(&mut self.code, 1 + numeric_imports);
            self.op(0x45);
            self.if_();
            self.num(crate::state::exit_kind::UNSUPPORTED);
            self.set(KIND);
            self.store(0, 400, KIND);
            self.end();
            self.end();
        }
        self.get(R);
        self.op(0x0b);
    }
}

/// Default comparison mode: direct forward edges use Wasm branches, backedges
/// use a dense internal label index, and indirect edges search guest PCs.
/// Architectural state remains in locals until one precise exit.
pub fn compile(region: &Region) -> Result<Vec<u8>, String> {
    compile_with_options(region, CompileOptions::default())
}

/// Experimental structured loops bypass the dense backedge dispatcher while
/// preserving its ordinary entries for arbitrary indirect targets and resumes.
pub fn compile_with_options(region: &Region, options: CompileOptions) -> Result<Vec<u8>, String> {
    compile_with_identity(region, options, 0)
}

/// The immutable Rust cache identity is checked at every internal tail entry.
/// Identity zero is reserved for standalone code-generation inspection.
pub fn compile_with_identity(
    region: &Region,
    options: CompileOptions,
    unit_id: u32,
) -> Result<Vec<u8>, String> {
    compile_with_identity_and_capacity(
        region,
        options,
        unit_id,
        crate::code_cache::DEFAULT_MAXIMUM_UNITS as u32,
    )
}

/// The backend's immutable resource budget also fixes its host table limits.
/// This changes only import admission, not generated guest operations.
pub fn compile_with_identity_and_capacity(
    region: &Region,
    options: CompileOptions,
    unit_id: u32,
    link_capacity: u32,
) -> Result<Vec<u8>, String> {
    compile_graph(
        region,
        &BTreeMap::new(),
        &crate::return_ir::Plan::new(region).returns,
        options,
        unit_id,
        link_capacity,
    )
}

pub fn compile_control_graph(
    graph: &crate::control_ir::Graph,
    options: CompileOptions,
    unit_id: u32,
    link_capacity: u32,
) -> Result<Vec<u8>, String> {
    graph.verify()?;
    compile_graph(
        &graph.region,
        &graph.dispatches,
        &graph.return_edges,
        options,
        unit_id,
        link_capacity,
    )
}

fn compile_graph(
    region: &Region,
    dispatches: &BTreeMap<u32, crate::control_ir::TableDispatch>,
    return_edges: &BTreeMap<u32, Vec<u32>>,
    options: CompileOptions,
    unit_id: u32,
    link_capacity: u32,
) -> Result<Vec<u8>, String> {
    if options.numeric_mode != crate::numeric_mode::NumericMode::Strict && !options.fp_ir {
        return Err("approximation requires explicit floating-point IR".into());
    }
    if (options.predicate_ir || options.flow_ir)
        && (options.value_ir
            || options.rooted_ssa
            || options.function_cfg
            || options.loop_memory
            || options.structured_loops
            || options.affine_memory
            || options.sampled_pc
            || options.direct_links
            || !dispatches.is_empty())
    {
        return Err("predicate IR requires its own value/control lowering".into());
    }
    if options.predicate_ir && options.flow_ir {
        return Err("flow IR and predicate IR are distinct lowering modes".into());
    }
    let value_control = options.value_ir || options.predicate_ir || options.flow_ir;
    if !dispatches.is_empty()
        && (options.value_ir
            || options.rooted_ssa
            || options.function_cfg
            || options.loop_memory
            || options.structured_loops
            || options.affine_memory)
    {
        return Err("binary-derived control graph requires ordinary instruction lowering".into());
    }
    if !(1..=crate::code_cache::MAXIMUM_UNITS as u32).contains(&link_capacity) {
        return Err("compiled link table capacity outside supported bound".into());
    }
    if region.blocks.is_empty() {
        return Err("empty region".into());
    }
    if options.direct_links
        && (options.value_ir
            || options.rooted_ssa
            || options.function_cfg
            || options.loop_memory
            || options.structured_loops
            || options.affine_memory)
    {
        return Err("direct links require ordinary instruction lowering".into());
    }
    if options.sampled_pc
        && (options.value_ir
            || options.rooted_ssa
            || options.structured_loops
            || options.affine_memory)
    {
        return Err("PC sampling requires ordinary control-flow lowering".into());
    }
    if options.value_ir && (options.structured_loops || options.affine_memory) {
        return Err("scalar value IR cannot share legacy structured-loop or affine proofs".into());
    }
    if options.rooted_ssa && (options.value_ir || options.structured_loops || options.affine_memory)
    {
        return Err(
            "rooted SSA cannot share legacy value, structured-loop or affine lowering".into(),
        );
    }
    if options.function_cfg
        && (options.loop_memory
            || options.structured_loops
            || options.affine_memory
            || options.value_ir
            || options.rooted_ssa)
    {
        return Err("function CFG requires ordinary instruction lowering".into());
    }
    let ordinary = !(options.function_cfg
        || options.loop_memory
        || options.structured_loops
        || options.affine_memory
        || options.value_ir
        || options.predicate_ir
        || options.flow_ir
        || options.rooted_ssa);
    let continuations = ordinary.then(|| {
        std::rc::Rc::new(crate::return_ir::Plan {
            returns: return_edges.clone(),
        })
    });
    let scope_plan = continuations.as_ref().map(|plan| {
        std::rc::Rc::new(crate::control_layout::Plan::new(
            &plan.layout_region(region),
        ))
    });
    let blocks: BTreeMap<_, _> = region
        .blocks
        .keys()
        .enumerate()
        .map(|(i, &pc)| {
            (
                pc,
                scope_plan
                    .as_ref()
                    .map_or(i, |plan| plan.targets[&pc].index),
            )
        })
        .collect();
    let mut fp_plan = BTreeMap::new();
    if options.fp_ir {
        for instruction in region.blocks.values().flat_map(|b| &b.instructions) {
            if let Opcode::X87 {
                descriptor,
                fop,
                segment,
            } = instruction.opcode
            {
                fp_plan.insert(
                    instruction.eip,
                    crate::fp_ir::lower(
                        descriptor,
                        fop,
                        segment,
                        instruction.operands.first().copied(),
                        instruction.eip,
                    )?,
                );
            }
        }
    }
    let mut e = Emitter {
        local_map: None,
        function_cfg: None,
        scope_plan,
        continuations,
        code: Vec::new(),
        labels: Vec::new(),
        blocks,
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
        fp_plan,
        fp_calls: BTreeMap::new(),
        fp_leaf: false,
        typed_numeric: options.typed_numeric && options.fp_ir,
        sse_numeric: [0; 10],
        scas_import: 0,
        unit_id,
        value: None,
        value_exit: None,
        value_adapter: false,
        coalesce_retirement: ordinary,
        retirement: None,
        retirement_exits: BTreeMap::new(),
        flag_plan: BTreeMap::new(),
        flag_admission: if ordinary {
            crate::flag_flow::discover(region)
        } else {
            BTreeMap::new()
        },
        flag_saved: [u32::MAX; 4],
        flow_partial: false,
        rooted: None,
        rooted_exit: None,
        sampled_pc: options.sampled_pc,
        binding_global: options.binding_global,
        direct_links: options.direct_links,
        dispatches: std::rc::Rc::new(dispatches.clone()),
    };
    if e.coalesce_retirement {
        e.flag_plan = crate::flag_ir::discover(region);
    }
    let loop_memory = if options.loop_memory {
        if options.structured_loops
            || options.affine_memory
            || options.value_ir
            || options.rooted_ssa
        {
            return Err(
                "loop memory proof cannot combine experimental control/affine emitters".into(),
            );
        }
        crate::loop_ir::discover(region)
            .into_iter()
            .map(|(pc, plan)| (pc, std::rc::Rc::new(plan)))
            .collect::<BTreeMap<_, _>>()
    } else {
        BTreeMap::new()
    };
    let structured = if options.structured_loops {
        loops::find(region)
    } else {
        BTreeMap::new()
    };
    let has_mmx = region.blocks.values().any(|block| {
        block
            .instructions
            .iter()
            .any(|i| matches!(i.opcode, Opcode::Mmx(_)))
    });
    let has_sse = region.blocks.values().any(|block| {
        block
            .instructions
            .iter()
            .any(|i| {
                matches!(i.opcode, Opcode::Sse(op) if !matches!(op, SseOpcode::Ldmxcsr | SseOpcode::Stmxcsr))
                    || i.operands.iter().any(|operand| matches!(operand, Operand::Xmm(_)))
            })
    });
    let has_vector = has_mmx || has_sse;
    e.has_mmx = has_mmx;
    let has_x87 = region.blocks.values().any(|block| {
        block
            .instructions
            .iter()
            .any(|i| matches!(i.opcode, Opcode::X87 { .. }))
    });
    let has_fp_ir = !e.fp_plan.is_empty();
    let typed_numeric = has_fp_ir && options.typed_numeric;
    let numeric_imports = if typed_numeric {
        crate::fp_ir::NUMERIC_KERNEL_COUNT + 1
    } else {
        u32::from(has_x87)
    };
    let has_timestamp = region
        .blocks
        .values()
        .any(|block| block.instructions.iter().any(|i| i.opcode == Opcode::Rdtsc));
    let sse_numeric_operations = [
        SseOpcode::Cvttss2si,
        SseOpcode::Cvttsd2si,
        SseOpcode::Cvtdq2ps,
        SseOpcode::Mulps,
        SseOpcode::Addps,
        SseOpcode::Subps,
        SseOpcode::Cvtsd2si,
        SseOpcode::Addpd,
        SseOpcode::Subpd,
        SseOpcode::Mulpd,
    ]
    .map(|operation| {
        region.blocks.values().any(|block| {
            block.instructions.iter().any(|instruction| {
                instruction.opcode == Opcode::Sse(operation)
                    || (operation == SseOpcode::Cvttss2si
                        && instruction.opcode == Opcode::Sse(SseOpcode::Cvttps2dq))
                    || matches!(
                        (operation, instruction.opcode),
                        (SseOpcode::Addpd, Opcode::Sse(SseOpcode::Addsd))
                            | (SseOpcode::Subpd, Opcode::Sse(SseOpcode::Subsd))
                            | (SseOpcode::Mulpd, Opcode::Sse(SseOpcode::Mulsd))
                    )
            })
        })
    });
    let sse_imports = sse_numeric_operations
        .iter()
        .map(|needed| u32::from(*needed))
        .sum::<u32>();
    let has_sse32 = sse_numeric_operations[..7].iter().any(|needed| *needed);
    let has_sse64 = sse_numeric_operations[7..].iter().any(|needed| *needed);
    let sse_first = 1 + numeric_imports + u32::from(has_timestamp);
    let mut sse_next = sse_first;
    e.sse_numeric = sse_numeric_operations.map(|needed| {
        let index = sse_next;
        sse_next += u32::from(needed);
        index
    });
    let has_scas = region.blocks.values().any(|block| {
        block.instructions.iter().any(|i| {
            matches!(
                i.opcode,
                Opcode::String(StringOpcode::Scas | StringOpcode::Cmps, true)
            )
        })
    });
    e.scas_import = sse_first + sse_imports;
    let first = e.scas_import + if has_scas { 3 } else { 0 };
    let fp_leaves = e.fp_function_plans(first + 3);
    let has_division = region.blocks.values().any(|block| {
        block.instructions.iter().any(|i| {
            matches!(
                i.opcode,
                Opcode::Div | Opcode::Idiv | Opcode::Mul | Opcode::ImulWide
            )
        })
    });
    let has_repeat = region.blocks.values().any(|block| {
        block
            .instructions
            .iter()
            .any(|i| matches!(i.opcode, Opcode::String(_, true)))
    });
    e.division_scratch = if has_sse {
        52
    } else if has_vector {
        43
    } else {
        30
    };
    if options.affine_memory {
        let first = e.division_scratch + u32::from(has_division);
        e.affine = crate::optimize::affine_ranges(region)
            .into_iter()
            .enumerate()
            .map(|(index, range)| affine::Certificate {
                range,
                local: first + index as u32 * 3,
            })
            .collect();
    }
    e.loop_base = e.division_scratch + u32::from(has_division) + e.affine.len() as u32 * 3;
    let loop_locals = loop_memory
        .values()
        .map(|plan| 1 + plan.ranges.len() as u32 * 3)
        .max()
        .unwrap_or(0);
    e.string_scratch = e.loop_base + loop_locals;
    let extra_i32 = e.affine.len() as u32 * 3 + loop_locals + if has_repeat { 2 } else { 0 };
    e.rep_fuel = e.string_scratch + if has_repeat { 2 } else { 0 };
    e.link_enabled = e.rep_fuel + 1;
    e.link_target = e.rep_fuel + 2;
    e.fp_base = e.link_target + 1;
    let saved_inputs = e
        .flag_plan
        .values()
        .fold(0, |mask, recipe| mask | recipe.saved_inputs());
    let mut saved_count = 0;
    for index in 0..4 {
        if saved_inputs & (1 << index) != 0 {
            e.flag_saved[index] =
                e.fp_base + if has_fp_ir { fp_ir::LOCAL_COUNT } else { 0 } + saved_count;
            saved_count += 1;
        }
    }
    if value_control {
        e.value = Some(std::rc::Rc::new(value::Plan::new(
            region,
            e.fp_base + if has_fp_ir { fp_ir::LOCAL_COUNT } else { 0 },
            first + 3 + fp_leaves.len() as u32,
            options.predicate_ir,
            options.flow_ir,
        )?));
    }
    if options.rooted_ssa {
        e.rooted = Some(std::rc::Rc::new(rooted::Plan::new(
            region,
            e.fp_base + if has_fp_ir { fp_ir::LOCAL_COUNT } else { 0 },
            first + 3 + fp_leaves.len() as u32,
        )?));
    }
    uleb(
        &mut e.code,
        (if has_sse {
            4
        } else if has_mmx {
            3
        } else {
            1
        }) + u32::from(has_division)
            + u32::from(extra_i32 != 0)
            + u32::from(has_fp_ir)
            + u32::from(value_control)
            + u32::from(options.rooted_ssa)
            + u32::from(saved_count != 0)
            + 2,
    );
    // Eighteen ABI parameters preserve GPR/flags locals3..13. Keep all
    // existing scratch/vector indices unchanged by reducing declared locals.
    uleb(&mut e.code, if has_vector { 15 } else { 12 });
    e.op(0x7f);
    if has_vector {
        uleb(&mut e.code, 9);
        e.op(0x7b);
        uleb(&mut e.code, 1);
        e.op(0x7e);
    }
    if has_sse {
        // Eight architectural XMM locals and one private staged numeric result.
        uleb(&mut e.code, 9);
        e.op(0x7b);
    }
    if has_division {
        uleb(&mut e.code, 1);
        e.op(0x7e);
    }
    if extra_i32 != 0 {
        uleb(&mut e.code, extra_i32);
        e.op(0x7f);
    }
    uleb(&mut e.code, 2);
    e.op(0x7f);
    uleb(&mut e.code, 1);
    e.op(0x7e);
    if has_fp_ir {
        uleb(&mut e.code, fp_ir::LOCAL_COUNT);
        e.op(0x7f);
    }
    let value_local_count = if let Some(plan) = &e.value {
        let start = e.code.len();
        uleb(&mut e.code, plan.local_count());
        let end = e.code.len();
        e.op(0x7f);
        Some(start..end)
    } else {
        None
    };
    let rooted_local_count = if let Some(plan) = &e.rooted {
        let start = e.code.len();
        uleb(&mut e.code, plan.local_count());
        let end = e.code.len();
        e.op(0x7f);
        Some(start..end)
    } else {
        None
    };
    if saved_count != 0 {
        uleb(&mut e.code, saved_count);
        e.op(0x7f);
    }
    // The preparation export enters with null host pointers and zero fuel.
    // Ordinary zero-budget execution still publishes its precise guest yield.
    // Return before CPU, memory, helpers, instrumentation or identity access.
    // This lets Chrome compile the body before launch without guest execution.
    e.get(0);
    e.op(0x45);
    e.get(1);
    e.op(0x45);
    e.op(0x71);
    e.get(2);
    e.op(0x45);
    e.op(0x71);
    e.if_();
    e.num(0);
    e.op(0x0f);
    e.end();
    let function_output = if options.function_cfg {
        Some(function_cfg::emit(
            &mut e,
            region,
            first + 3 + fp_leaves.len() as u32,
            has_sse,
            has_scas,
            has_timestamp,
            numeric_imports,
        ))
    } else {
        None
    };
    if !options.function_cfg {
        e.sample_location(3, region.entry);
        e.get(14);
        e.set(DEFINED);
        e.get(15);
        e.set(e.link_enabled);
        e.get(17);
        e.set(e.rep_fuel);
        if has_mmx {
            e.initialize_mmx();
        }
        if has_sse {
            e.initialize_sse();
        }
        e.fp_initialize();
        e.open(0x02, "exit".into());
        e.get(16);
        e.unit_identity();
        e.op(0x47);
        e.if_();
        e.num(1); // Invalid transport binding, not a fabricated guest fault.
        e.set(ERROR);
        e.exit(3);
        e.end();
        if let Some(plan) = e.rooted.clone() {
            plan.emit(&mut e, region);
        } else if let Some(plan) = e.value.clone().filter(|plan| plan.is_flow()) {
            plan.emit_flow(&mut e, region);
        } else {
            if value_control {
                e.open(0x02, "value-cold".into());
            }
            if e.coalesce_retirement {
                e.open(0x02, "retirement-cold".into());
            }
            e.open(0x03, "lookup".into());
            e.sample_location(4, 0);
            // Indirect dispatch is backend-owned and includes only decoded code.
            e.num(u32::MAX);
            e.set(ROUTE);
            let entries: Vec<_> = e.blocks.iter().map(|(&pc, &index)| (pc, index)).collect();
            if e.scope_plan.is_some() && entries.len() >= 8 {
                e.lookup_buckets(&entries);
            } else {
                e.lookup_tree(&entries);
            }
            e.get(ROUTE);
            e.num(u32::MAX);
            e.op(0x46);
            e.if_();
            if let Some(plan) = e.value.clone() {
                plan.emit_lookup_miss(&mut e);
            }
            e.exit(2);
            e.end();
            if e.scope_plan.is_some() {
                e.emit_scopes(region);
            } else {
                e.open(0x03, "dispatch".into());
                e.sample_location(2, 0);
                for index in (0..region.blocks.len()).rev() {
                    e.open(0x02, format!("b{index}"));
                }
                e.get(ROUTE);
                e.op(0x0e);
                uleb(&mut e.code, region.blocks.len() as u32);
                for index in 0..region.blocks.len() {
                    uleb(&mut e.code, index as u32);
                }
                uleb(
                    &mut e.code,
                    region.blocks.len() as u32
                        + 2
                        + u32::from(value_control)
                        + u32::from(e.coalesce_retirement),
                );
                for block in region.blocks.values() {
                    e.end();
                    if let Some(plan) = e.value.clone() {
                        plan.emit_block(&mut e, block);
                    } else if let Some(plan) = structured.get(&block.start) {
                        e.structured_loop(region, plan);
                    } else if let Some(plan) = loop_memory.get(&block.start) {
                        e.loop_memory(region, plan.clone());
                    } else {
                        e.block_body(block);
                    }
                }
                e.end();
            }
            e.end();
            if let Some(plan) = e.value.clone() {
                e.end(); // value-cold: preserve function locals for the shared recipes.
                plan.emit_cold_exit_bodies(&mut e);
            }
            if e.coalesce_retirement {
                e.end();
                e.emit_retirement_exits();
            }
        }
        e.end();
        e.finish(has_mmx, has_sse, has_scas, has_timestamp, numeric_imports);
    }
    if let Some(plan) = e.value.as_ref().filter(|plan| plan.is_flow()) {
        if plan.local_count() > 49_000 {
            return Err("continuation SSA exceeds bounded Wasm local storage".into());
        }
        let mut count = Vec::new();
        uleb(&mut count, plan.local_count());
        e.code
            .splice(value_local_count.expect("flow local declaration"), count);
    }
    // Rooted temporaries are reused for each normal expression sequence.
    // Declare only indices actually emitted, not the whole CFG's node count.
    let snapshot_bodies = if let Some(plan) = &e.rooted {
        let mut count = Vec::new();
        uleb(&mut count, plan.local_count());
        e.code
            .splice(rooted_local_count.expect("rooted local declaration"), count);
        plan.snapshot_bodies()?
    } else {
        Vec::new()
    };
    let mut out = b"\0asm\x01\0\0\0".to_vec();
    let snapshot_type = 3
        + u32::from(has_x87)
        + u32::from(has_timestamp)
        + u32::from(has_fp_ir)
        + u32::from(typed_numeric)
        + u32::from(has_sse32)
        + u32::from(has_sse64)
        + u32::from(options.value_ir)
        + if has_scas { 2 } else { 0 };
    let mut types = Vec::new();
    uleb(
        &mut types,
        snapshot_type + snapshot_bodies.len() as u32 + u32::from(options.function_cfg) + 1,
    );
    types.extend([0x60, 18]);
    types.extend([0x7f; 18]);
    types.extend([1, 0x7f, 0x60, 3, 0x7f, 0x7f, 0x7f, 1, 0x7f]);
    types.extend([0x60, 2, 0x7f, 0x7f, 1, 0x7e]); // resolver
    if has_x87 {
        let parameters = if typed_numeric {
            7
        } else if has_fp_ir {
            9
        } else {
            6
        };
        types.extend([0x60, parameters]);
        types.extend(std::iter::repeat_n(0x7f, usize::from(parameters)));
        types.extend([1, 0x7f]);
    }
    if has_timestamp {
        types.extend([0x60, 2, 0x7f, 0x7f, 1, 0x7f]);
    }
    if has_fp_ir {
        types.extend([0x60, 13]);
        types.extend([0x7f; 13]);
        types.push(12);
        types.extend([0x7f; 12]);
    }
    if typed_numeric {
        types.extend([0x60, 0, 1, 0x7f]); // fixed numeric failure code
    }
    let sse_type = 3
        + u8::from(has_x87)
        + u8::from(has_timestamp)
        + u8::from(has_fp_ir)
        + u8::from(typed_numeric);
    if has_sse32 {
        // Pure lane arithmetic: MXCSR and raw bits -> packed result/flags.
        types.extend([0x60, 3, 0x7f, 0x7f, 0x7f, 1, 0x7e]);
    }
    let sse64_type = sse_type + u8::from(has_sse32);
    if has_sse64 {
        // Binary64 value bits -> instance-owned numeric scratch address.
        types.extend([0x60, 3, 0x7f, 0x7e, 0x7e, 1, 0x7f]);
    }
    let scas_type = sse64_type + u8::from(has_sse64);
    if has_scas {
        types.extend([0x60, 9]);
        types.extend([0x7f; 9]);
        types.extend([1, 0x7f]);
        types.extend([0x60, 1, 0x7f, 1, 0x7f]);
    }
    let value_type = scas_type + if has_scas { 2 } else { 0 };
    if options.value_ir {
        types.extend([0x60, 5, 0x7f, 0x7f, 0x7f, 0x7f, 0x7f, 1, 0x7f]);
    }
    for snapshot in &snapshot_bodies {
        types.push(0x60);
        uleb(&mut types, snapshot.parameters);
        types.extend(std::iter::repeat_n(0x7f, snapshot.parameters as usize));
        uleb(&mut types, snapshot.results);
        types.extend(std::iter::repeat_n(0x7f, snapshot.results as usize));
    }
    if let Some(output) = &function_output {
        types.push(0x60);
        uleb(&mut types, output.parameters.len() as u32);
        types.extend(&output.parameters);
        types.extend([1, 0x7f]);
    }
    types.extend([0x60, 0, 1, 0x7f]); // preparation: () -> i32
    section(&mut out, 1, &types);
    let mut imports = vec![
        4 + numeric_imports as u8
            + u8::from(options.sampled_pc)
            + u8::from(options.binding_global)
            + u8::from(options.direct_links)
            + u8::from(has_timestamp)
            + sse_imports as u8
            + if has_scas { 3 } else { 0 },
    ];
    name(&mut imports, "env");
    name(&mut imports, "memory");
    imports.extend([2, 0, 1]);
    if options.sampled_pc {
        name(&mut imports, "env");
        name(&mut imports, "sampled_pc");
        imports.extend([2, 3, 1, 1]); // one fixed shared64KiB observation memory
    }
    name(&mut imports, "env");
    name(&mut imports, "links");
    imports.extend([1, 0x70, 1]); // Rust-owned table; AOT can grow its residency
    uleb(&mut imports, link_capacity);
    uleb(&mut imports, crate::code_cache::MAXIMUM_LINK_UNITS as u32);
    name(&mut imports, "env");
    name(&mut imports, "link_resolve");
    imports.extend([0, 2]);
    name(&mut imports, "env");
    name(&mut imports, "backend");
    imports.extend([3, 0x7f, 0]);
    if options.binding_global {
        name(&mut imports, "env");
        name(&mut imports, "unit");
        imports.extend([3, 0x7f, 0]);
    }
    if options.direct_links {
        name(&mut imports, "env");
        name(&mut imports, "link_directory");
        imports.extend([3, 0x7f, 0]);
    }
    if typed_numeric {
        for operation in 0..crate::fp_ir::NUMERIC_KERNEL_COUNT {
            name(&mut imports, "env");
            name(
                &mut imports,
                &format!(
                    "{}_{operation}",
                    options.numeric_mode.import_prefix(operation)
                ),
            );
            imports.extend([0, 3]);
        }
        name(&mut imports, "env");
        name(&mut imports, "fp_kernel_error");
        imports.extend([
            0,
            3 + u8::from(has_x87) + u8::from(has_timestamp) + u8::from(has_fp_ir),
        ]);
    } else if has_x87 {
        name(&mut imports, "env");
        name(&mut imports, if has_fp_ir { "fp_numeric" } else { "x87" });
        imports.extend([0, 3]);
    }
    if has_timestamp {
        name(&mut imports, "env");
        name(&mut imports, "begin_timestamp_counter");
        imports.extend([0, 3 + u8::from(has_x87)]);
    }
    for (index, needed) in sse_numeric_operations.into_iter().enumerate() {
        if needed {
            name(&mut imports, "env");
            name(
                &mut imports,
                [
                    "sse_cvtt_f32",
                    "sse_cvtt_f64",
                    "sse_i32_to_f32",
                    "sse_mul_f32",
                    "sse_add_f32",
                    "sse_sub_f32",
                    "sse_cvt_f64",
                    "sse_add_f64",
                    "sse_sub_f64",
                    "sse_mul_f64",
                ][index],
            );
            imports.extend([0, if index >= 7 { sse64_type } else { sse_type }]);
        }
    }
    if has_scas {
        for (name_, type_) in [
            ("scas_begin", scas_type),
            ("scas_complete", scas_type + 1),
            ("scas_finish", scas_type + 1),
        ] {
            name(&mut imports, "env");
            name(&mut imports, name_);
            imports.extend([0, type_]);
        }
    }
    section(&mut out, 2, &imports);
    let function_count = function_output
        .as_ref()
        .map_or(0, |o| o.bodies.len() as u32);
    let mut functions = Vec::new();
    uleb(
        &mut functions,
        4 + fp_leaves.len() as u32
            + u32::from(options.value_ir)
            + snapshot_bodies.len() as u32
            + function_count,
    );
    functions.extend([0, 1, 1]);
    for _ in &fp_leaves {
        uleb(
            &mut functions,
            3 + u32::from(has_x87) + u32::from(has_timestamp),
        );
    }
    if options.value_ir {
        uleb(&mut functions, u32::from(value_type));
    }
    for index in 0..snapshot_bodies.len() {
        uleb(&mut functions, snapshot_type + index as u32);
    }
    for _ in 0..function_count {
        uleb(&mut functions, snapshot_type + snapshot_bodies.len() as u32);
    }
    uleb(
        &mut functions,
        snapshot_type + snapshot_bodies.len() as u32 + u32::from(options.function_cfg),
    );
    section(&mut out, 3, &functions);
    let mut exports = vec![4];
    for (export, index) in [
        ("link", first),
        ("run", first + 1),
        ("run_linked", first + 2),
        (
            "prepare",
            first
                + 3
                + fp_leaves.len() as u32
                + u32::from(options.value_ir)
                + snapshot_bodies.len() as u32
                + function_count,
        ),
    ] {
        name(&mut exports, export);
        exports.push(0);
        uleb(&mut exports, index);
    }
    section(&mut out, 7, &exports);
    let mut body = Vec::new();
    uleb(
        &mut body,
        4 + fp_leaves.len() as u32
            + u32::from(options.value_ir)
            + snapshot_bodies.len() as u32
            + function_count,
    );
    uleb(&mut body, e.code.len() as u32);
    body.extend(e.code);
    for linked in [0, 1] {
        let mut wrapper = vec![0]; // no additional locals
        for index in 0..3 {
            wrapper.extend([0x20, index]);
        }
        for offset in (0..=36).step_by(4) {
            // Eight GPRs, EFLAGS, EIP (ABI order differs from CpuState).
            let offset = match offset {
                32 => 36,
                36 => 32,
                other => other,
            };
            wrapper.extend([0x20, 0, 0x28, 0]);
            uleb(&mut wrapper, offset);
        }
        wrapper.extend([0x20, 2, 0x20, 0, 0x28, 0, 40]); // remaining, defined
        wrapper.push(0x41);
        sleb(&mut wrapper, linked);
        if options.binding_global {
            wrapper.extend([0x23, 1]);
        } else {
            wrapper.push(0x41);
            sleb(&mut wrapper, i64::from(unit_id as i32));
        }
        wrapper.push(0x41);
        sleb(&mut wrapper, i64::from(string::REP_ELEMENT_BUDGET));
        wrapper.push(0x12);
        uleb(&mut wrapper, first); // return_call core
        wrapper.push(0x0b);
        uleb(&mut body, wrapper.len() as u32);
        body.extend(wrapper);
    }
    for plan in &fp_leaves {
        let leaf = fp_ir::function_body(*plan, typed_numeric);
        uleb(&mut body, leaf.len() as u32);
        body.extend(leaf);
    }
    if options.value_ir {
        let leaf = value::flag_function_body();
        uleb(&mut body, leaf.len() as u32);
        body.extend(leaf);
    }
    for snapshot in snapshot_bodies {
        uleb(&mut body, snapshot.code.len() as u32);
        body.extend(snapshot.code);
    }
    if let Some(output) = &function_output {
        for code in &output.bodies {
            uleb(&mut body, code.len() as u32);
            body.extend(code);
        }
    }
    let preparation = preparation::function_body(first);
    uleb(&mut body, preparation.len() as u32);
    body.extend(preparation);
    section(&mut out, 10, &body);
    if !dispatches.is_empty() {
        let mut metadata = Vec::new();
        name(&mut metadata, "vine.control-graph.v1");
        let pcs: Vec<_> = dispatches.keys().copied().collect();
        let cases: usize = dispatches
            .values()
            .map(|d| d.targets.iter().flatten().count())
            .sum();
        metadata.extend(format!("{{\"switches\":{pcs:?},\"cases\":{cases}}}").as_bytes());
        section(&mut out, 0, &metadata);
    }
    if let Some(output) = &function_output {
        let mut diagnostic = Vec::new();
        name(&mut diagnostic, "vine.function-cfg.v1");
        diagnostic.extend(
            format!(
                "{{\"instructions\":{:?},\"parameters\":{}}}",
                output.instructions,
                output.parameters.len()
            )
            .as_bytes(),
        );
        section(&mut out, 0, &diagnostic);
    }
    if options.loop_memory {
        let mut diagnostic = Vec::new();
        name(&mut diagnostic, "vine.loop-memory.v1");
        diagnostic.extend(
            format!(
                "{{\"loops\":{},\"ranges\":{},\"instructions\":{}}}",
                loop_memory.len(),
                loop_memory.values().map(|p| p.ranges.len()).sum::<usize>(),
                loop_memory
                    .values()
                    .map(|p| p.maximum_instructions)
                    .sum::<u32>()
            )
            .as_bytes(),
        );
        section(&mut out, 0, &diagnostic);
    }
    if let Some(plan) = &e.value {
        let mut diagnostic = Vec::new();
        name(
            &mut diagnostic,
            if options.flow_ir {
                "vine.flow-ir.v3"
            } else if options.predicate_ir {
                "vine.predicate-ir.v1"
            } else {
                "vine.value-ir.v1"
            },
        );
        diagnostic.extend(plan.metadata());
        section(&mut out, 0, &diagnostic);
    }
    if let Some(plan) = &e.rooted {
        let mut diagnostic = Vec::new();
        name(&mut diagnostic, "vine.rooted-ssa.v1");
        diagnostic.extend(plan.metadata());
        section(&mut out, 0, &diagnostic);
    }
    if options.affine_memory {
        let mut diagnostic = Vec::new();
        name(&mut diagnostic, "vine.affine.v1");
        diagnostic.push(e.affine.len() as u8);
        for c in &e.affine {
            diagnostic.push(c.range.base);
            for value in [
                c.range.minimum as u32,
                c.range.maximum as u32,
                c.range.permissions,
                c.range.references as u32,
                c.range.distinct_offsets as u32,
            ] {
                diagnostic.extend(value.to_le_bytes());
            }
        }
        section(&mut out, 0, &diagnostic);
    }
    if has_fp_ir {
        let mut diagnostic = Vec::new();
        name(&mut diagnostic, "vine.fp-ir.v1");
        diagnostic.extend((e.fp_plan.len() as u32).to_le_bytes());
        diagnostic.extend(0u32.to_le_bytes()); // no legacy descriptor fallbacks
        section(&mut out, 0, &diagnostic);
        let mut shared = Vec::new();
        name(&mut shared, "vine.fp-ir.functions.v1");
        shared.extend((fp_leaves.len() as u32).to_le_bytes());
        section(&mut out, 0, &shared);
    }
    if typed_numeric {
        let mut diagnostic = Vec::new();
        name(&mut diagnostic, "vine.fp-kernels.v1");
        diagnostic.extend(crate::fp_ir::NUMERIC_KERNEL_COUNT.to_le_bytes());
        section(&mut out, 0, &diagnostic);
    }
    Ok(out)
}

#[cfg(test)]
mod cache_capacity_tests {
    use super::*;
    use crate::memory::{EXECUTE, MirrorMemory, PAGE_SIZE, READ, SEALED};

    #[test]
    fn table_capacity_changes_only_import_limits_and_keeps_default_bytes() {
        let mut memory = MirrorMemory::new(1).unwrap();
        memory
            .map_page(0x1000, 1, READ | EXECUTE | SEALED, &[0x90; PAGE_SIZE])
            .unwrap();
        let region = crate::decode::discover(&memory, 0x1000, 1);
        let options = CompileOptions::default();
        let old_api = compile_with_identity(&region, options, 17).unwrap();
        let control = compile_with_identity_and_capacity(&region, options, 17, 64).unwrap();
        let candidate = compile_with_identity_and_capacity(&region, options, 17, 512).unwrap();
        assert_eq!(old_api, control);
        fn sections(bytes: &[u8]) -> Vec<(u8, &[u8])> {
            let mut offset = 8;
            let mut result = Vec::new();
            while offset < bytes.len() {
                let kind = bytes[offset];
                offset += 1;
                let mut length = 0usize;
                let mut shift = 0;
                loop {
                    let byte = bytes[offset];
                    offset += 1;
                    length |= usize::from(byte & 127) << shift;
                    if byte & 128 == 0 {
                        break;
                    }
                    shift += 7;
                }
                result.push((kind, &bytes[offset..offset + length]));
                offset += length;
            }
            result
        }
        let before = sections(&control);
        let after = sections(&candidate);
        assert_eq!(before.len(), after.len());
        for ((kind, old), (new_kind, new)) in before.into_iter().zip(after) {
            assert_eq!(kind, new_kind);
            if kind == 2 {
                let marker = b"\x03env\x05links\x01\x70\x01\x40\x80\x24";
                let position = old
                    .windows(marker.len())
                    .position(|bytes| bytes == marker)
                    .unwrap();
                let mut expected = old[..position].to_vec();
                expected.extend(b"\x03env\x05links\x01\x70\x01\x80\x04\x80\x24");
                expected.extend(&old[position + marker.len()..]);
                assert_eq!(new, expected);
            } else {
                assert_eq!(old, new, "capacity changed non-import section {kind}");
            }
        }
        assert!(compile_with_identity_and_capacity(&region, options, 17, 0).is_err());
        assert!(compile_with_identity_and_capacity(&region, options, 17, 513).is_err());
    }
}

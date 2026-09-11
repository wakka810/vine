//! Bounded counted-loop lowering. Proof failures fall through to the ordinary JIT.
//! No guest state changes until every access in the chunk has a valid TLB mapping.
use crate::cpu::{cpu, global_pointers as gp};
use crate::cpu_context::CpuContext;
use crate::jit::{BasicBlock, BasicBlockType, JitContext};
use crate::wasmgen::wasm_builder::{WasmBuilder, WasmLocal};
use crate::{codegen, modrm, regs};
use std::collections::HashMap;

#[derive(Clone, Copy)]
struct Address {
    base: Option<u32>,
    index: Option<u32>,
    shift: u8,
    offset: i32,
}
struct Access {
    address: Address,
    bytes: i32,
    write: bool,
    root: usize,
}
#[derive(Clone, Copy)]
enum Packed {
    UnpackBytes,
    SubWords,
    MulWords,
    AddWords,
    PackBytes,
}
#[derive(Clone, Copy)]
enum Source {
    Register(u32),
    Memory(usize),
}
#[derive(Clone, Copy)]
enum Op {
    Load(u32, usize),
    Store(u32, usize),
    Or(u32, u32),
    AddMemory(u32, usize),
    AddImmediate(u32, i32, bool), // true preserves CF (INC/DEC)
    Shift(u32, bool, Option<u8>), // left; None means CL
    Compare(u32, i32),
    Branch(u8, u32),
    MmxLoad(u32, usize),
    MmxStore(u32, usize),
    MmxBinary(Packed, u32, Source),
    MmxShift(u32, u8),
}
impl Op {
    fn access(self) -> Option<usize> {
        match self {
            Self::Load(_, m)
            | Self::Store(_, m)
            | Self::AddMemory(_, m)
            | Self::MmxLoad(_, m)
            | Self::MmxStore(_, m)
            | Self::MmxBinary(_, _, Source::Memory(m)) => Some(m),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, PartialEq)]
enum Value {
    Unknown,
    Range(Option<u32>, i64, i64),
}
impl Value {
    fn add(self, other: Value) -> Value {
        match (self, other) {
            (Self::Range(a, lo, hi), Self::Range(b, l, h)) if a.is_none() || b.is_none() => {
                Self::Range(a.or(b), lo + l, hi + h)
            }
            _ => Self::Unknown,
        }
    }
    fn merge(self, other: Value) -> Value {
        match (self, other) {
            (Self::Range(a, lo, hi), Self::Range(b, l, h)) if a == b => {
                Self::Range(a, lo.min(l), hi.max(h))
            }
            _ => Self::Unknown,
        }
    }
}
struct Span {
    low: i32,
    high: i32,
    stride: i32,
    write: bool,
}
struct Plan {
    ops: Vec<Op>,
    addresses: Vec<u32>,
    accesses: Vec<Access>,
    spans: [Option<Span>; 8],
    counter: u32,
    carry_register: u32,
    carry_add: i32,
    mmx_used: u8,
    mmx_written: u8,
}

fn memory(
    c: &mut CpuContext,
    byte: u8,
    bytes: i32,
    write: bool,
    all: &mut Vec<Access>,
) -> Option<usize> {
    if byte >= 0xc0 {
        return None;
    }
    let (base, index, shift, offset) = modrm::decode(c, byte).flat_parts()?;
    let m = all.len();
    all.push(Access {
        address: Address {
            base,
            index,
            shift,
            offset,
        },
        bytes,
        write,
        root: 0,
    });
    Some(m)
}

fn decode(c: &mut CpuContext, accesses: &mut Vec<Access>) -> Option<Op> {
    let opcode = c.read_imm8();
    Some(match opcode {
        0x40..=0x47 => Op::AddImmediate((opcode & 7) as u32, 1, true),
        0x48..=0x4f => Op::AddImmediate((opcode & 7) as u32, -1, true),
        0x89 => {
            let b = c.read_imm8();
            Op::Store((b >> 3 & 7) as u32, memory(c, b, 4, true, accesses)?)
        }
        0x09 | 0x0b => {
            let b = c.read_imm8();
            if b < 0xc0 {
                return None;
            }
            let (r, s) = ((b >> 3 & 7) as u32, (b & 7) as u32);
            if opcode == 9 {
                Op::Or(s, r)
            } else {
                Op::Or(r, s)
            }
        }
        0x03 | 0x8b => {
            let b = c.read_imm8();
            let r = (b >> 3 & 7) as u32;
            let m = memory(c, b, 4, false, accesses)?;
            if opcode == 3 {
                Op::AddMemory(r, m)
            } else {
                Op::Load(r, m)
            }
        }
        0x81 | 0x83 => {
            let b = c.read_imm8();
            if b < 0xc0 {
                return None;
            }
            let r = (b & 7) as u32;
            let v = if opcode == 0x83 {
                c.read_imm8s() as i32
            } else {
                c.read_imm32() as i32
            };
            match b >> 3 & 7 {
                0 => Op::AddImmediate(r, v, false),
                7 => Op::Compare(r, v),
                _ => return None,
            }
        }
        0x3d => Op::Compare(regs::EAX, c.read_imm32() as i32),
        0xc1 | 0xd1 | 0xd3 => {
            let b = c.read_imm8();
            if b < 0xc0 || !matches!(b >> 3 & 7, 4 | 5) {
                return None;
            }
            Op::Shift(
                (b & 7) as u32,
                b >> 3 & 7 == 4,
                if opcode == 0xc1 {
                    Some(c.read_imm8() & 31)
                } else if opcode == 0xd1 {
                    Some(1)
                } else {
                    None
                },
            )
        }
        0x70..=0x7f => {
            let offset = c.read_imm8s() as i32;
            Op::Branch(opcode & 15, c.eip.wrapping_add(offset as u32))
        }
        0x0f => {
            let op = c.read_imm8();
            let b = c.read_imm8();
            let r = (b >> 3 & 7) as u32;
            match op {
                0xb6 => Op::Load(r, memory(c, b, 1, false, accesses)?),
                0x6e | 0x6f => Op::MmxLoad(
                    r,
                    memory(c, b, if op == 0x6e { 4 } else { 8 }, false, accesses)?,
                ),
                0x7e | 0x7f => Op::MmxStore(
                    r,
                    memory(c, b, if op == 0x7e { 4 } else { 8 }, true, accesses)?,
                ),
                0x71 if b >= 0xc0 && r == 4 => Op::MmxShift((b & 7) as u32, c.read_imm8().min(15)),
                0x60 | 0xf9 | 0xd5 | 0xfd | 0x67 => {
                    let source = if b >= 0xc0 {
                        Source::Register((b & 7) as u32)
                    } else {
                        Source::Memory(memory(
                            c,
                            b,
                            if op == 0x60 { 4 } else { 8 },
                            false,
                            accesses,
                        )?)
                    };
                    let op = match op {
                        0x60 => Packed::UnpackBytes,
                        0xf9 => Packed::SubWords,
                        0xd5 => Packed::MulWords,
                        0xfd => Packed::AddWords,
                        _ => Packed::PackBytes,
                    };
                    Op::MmxBinary(op, r, source)
                }
                _ => return None,
            }
        }
        _ => return None,
    })
}

fn merge(into: &mut Option<[Value; 8]>, from: [Value; 8]) {
    if let Some(existing) = into {
        for r in 0..8 {
            existing[r] = existing[r].merge(from[r]);
        }
    } else {
        *into = Some(from);
    }
}

fn plan(ctx: &JitContext, blocks: &HashMap<u32, BasicBlock>, root: u32) -> Option<Plan> {
    if !ctx.cpu.state_flags.is_32() || !ctx.cpu.has_flat_segmentation() {
        return None;
    }
    let tails: Vec<_> = blocks
        .values()
        .filter(|b| {
            b.addr >= root
                && b.addr - root < 256
                && matches!(b.ty, BasicBlockType::ConditionalJump { condition: 5 | 0x75 | 0x85,
            next_block_branch_taken_addr: Some(t), .. } if t == root)
        })
        .collect();
    if tails.len() != 1 {
        return None;
    }
    let end = tails[0].end_addr;
    if root >> 12 != end >> 12 {
        return None;
    }
    let mut c = ctx.cpu.clone();
    c.eip = root;
    c.prefixes = 0;
    let mut p = Plan {
        ops: Vec::new(),
        addresses: Vec::new(),
        accesses: Vec::new(),
        spans: std::array::from_fn(|_| None),
        counter: 0,
        carry_register: 0,
        carry_add: 0,
        mmx_used: 0,
        mmx_written: 0,
    };
    while c.eip < end && p.ops.len() < 64 {
        if crate::jit::is_near_end_of_page(c.eip) {
            return None;
        }
        p.addresses.push(c.eip);
        p.ops.push(decode(&mut c, &mut p.accesses)?);
    }
    let len = p.ops.len();
    if c.eip != end || len < 4 {
        return None;
    }
    match p.ops[len - 1] {
        Op::Branch(5, t) if t == root => {}
        _ => return None,
    }
    p.counter = match p.ops[len - 2] {
        Op::AddImmediate(r, -1, true) => r,
        _ => return None,
    };
    (p.carry_register, p.carry_add) = match p.ops[len - 3] {
        Op::AddImmediate(r, n, false) if r != p.counter => (r, n),
        _ => return None,
    };
    let mut states: Vec<Option<[Value; 8]>> = vec![None; len + 1];
    states[0] = Some(std::array::from_fn(|r| Value::Range(Some(r as u32), 0, 0)));
    for i in 0..len {
        let mut state = states[i]?;
        let op = p.ops[i];
        if let Some(m) = op.access() {
            let a = &mut p.accesses[m];
            let mut value = Value::Range(None, a.address.offset as i64, a.address.offset as i64);
            if let Some(r) = a.address.base {
                value = value.add(state[r as usize]);
            }
            if let Some(r) = a.address.index {
                let scaled = match state[r as usize] {
                    Value::Range(None, lo, hi) => {
                        Value::Range(None, lo << a.address.shift, hi << a.address.shift)
                    }
                    v if a.address.shift == 0 => v,
                    _ => return None,
                };
                value = value.add(scaled);
            }
            let (r, lo, hi) = match value {
                Value::Range(Some(r), lo, hi) => (r as usize, lo, hi + a.bytes as i64 - 1),
                _ => return None,
            };
            let lo = i32::try_from(lo).ok()?;
            let hi = i32::try_from(hi).ok()?;
            a.root = r;
            if let Some(span) = &mut p.spans[r] {
                span.low = span.low.min(lo);
                span.high = span.high.max(hi);
                span.write |= a.write;
            } else {
                p.spans[r] = Some(Span {
                    low: lo,
                    high: hi,
                    write: a.write,
                    stride: 0,
                });
            }
        }
        match op {
            Op::Load(r, m) => {
                state[r as usize] = if p.accesses[m].bytes == 1 {
                    Value::Range(None, 0, 255)
                } else {
                    Value::Unknown
                }
            }
            Op::AddMemory(r, _) => state[r as usize] = Value::Unknown,
            Op::Or(r, _) => state[r as usize] = Value::Unknown,
            Op::AddImmediate(r, n, _) => {
                state[r as usize] = state[r as usize].add(Value::Range(None, n as i64, n as i64))
            }
            Op::Shift(r, left, count) => {
                state[r as usize] = match (state[r as usize], count) {
                    (Value::Range(None, lo, hi), Some(n))
                        if lo >= 0
                            && hi <= u32::MAX as i64
                            && (!left || hi << n <= u32::MAX as i64) =>
                    {
                        if left {
                            Value::Range(None, lo << n, hi << n)
                        } else {
                            Value::Range(None, lo >> n, hi >> n)
                        }
                    }
                    _ => Value::Unknown,
                }
            }
            Op::Branch(condition, target) if i + 1 < len => {
                let j = p.addresses.iter().position(|&a| a == target)?;
                // Every path executes the final ADD/DEC pair, which defines exit flags.
                if j <= i || j > len - 3 {
                    return None;
                }
                match (condition, i.checked_sub(1).map(|i| p.ops[i])) {
                    (3, Some(Op::Compare(r, n))) if n > 0 => {
                        merge(&mut states[j], state);
                        state[r as usize] = Value::Range(None, 0, n as i64 - 1);
                    }
                    (14, Some(Op::AddMemory(_, _))) => merge(&mut states[j], state),
                    _ => return None,
                }
            }
            Op::MmxLoad(r, _) | Op::MmxShift(r, _) => {
                p.mmx_used |= 1 << r;
                p.mmx_written |= 1 << r;
            }
            Op::MmxStore(r, _) => p.mmx_used |= 1 << r,
            Op::MmxBinary(_, r, source) => {
                p.mmx_used |= 1 << r;
                p.mmx_written |= 1 << r;
                if let Source::Register(s) = source {
                    p.mmx_used |= 1 << s;
                }
            }
            _ => {}
        }
        merge(&mut states[i + 1], state);
    }
    let last = states[len]?;
    if last[p.counter as usize] != Value::Range(Some(p.counter), -1, -1)
        || !p.accesses.iter().any(|a| a.write)
    {
        return None;
    }
    for (r, span) in p.spans.iter_mut().enumerate() {
        if let Some(span) = span {
            // ponytail: one page per stream, or two contiguous pages for invariants.
            if span.high as i64 - span.low as i64 >= 4096 {
                return None;
            }
            span.stride = match last[r] {
                Value::Range(Some(base), lo, hi)
                    if base == r as u32 && lo == hi && (0..=64).contains(&lo) =>
                {
                    lo as i32
                }
                _ => return None,
            };
        }
    }
    Some(p)
}

static mut ITERATIONS: u64 = 0;
#[no_mangle]
pub unsafe fn vine_affine_iterations() -> f64 {
    ITERATIONS as f64
}

fn min_local(b: &mut WasmBuilder, n: &WasmLocal) {
    let value = b.set_new_local();
    b.get_local(&value);
    b.get_local(n);
    b.get_local(&value);
    b.get_local(n);
    b.ltu_i32();
    b.select();
    b.set_local(n);
    b.free_local(value);
}
fn address(ctx: &mut JitContext, a: &Access, translations: &[Option<WasmLocal>; 8]) {
    let b = &mut ctx.builder;
    b.const_i32(a.address.offset);
    if let Some(r) = a.address.base {
        b.get_local(&ctx.register_locals[r as usize]);
        b.add_i32();
    }
    if let Some(r) = a.address.index {
        b.get_local(&ctx.register_locals[r as usize]);
        b.const_i32(a.address.shift as i32);
        b.shl_i32();
        b.add_i32();
    }
    b.get_local(translations[a.root].as_ref().unwrap());
    b.add_i32();
}
fn read(ctx: &mut JitContext, a: &Access, translations: &[Option<WasmLocal>; 8]) {
    address(ctx, a, translations);
    match a.bytes {
        1 => ctx.builder.load_u8(0),
        4 => ctx.builder.load_unaligned_i32(0),
        _ => ctx.builder.load_unaligned_i64(0),
    }
}
pub fn generate(ctx: &mut JitContext, blocks: &HashMap<u32, BasicBlock>, root: u32) {
    let Some(p) = plan(ctx, blocks, root) else {
        return;
    };
    let fallback = ctx.builder.block_void();
    if p.mmx_used != 0 {
        ctx.builder.load_fixed_i32(gp::cr as u32);
        ctx.builder.const_i32(cpu::CR0_EM | cpu::CR0_TS);
        ctx.builder.and_i32();
        ctx.builder.br_if(fallback);
    }
    let counter = ctx.reg(p.counter);
    ctx.builder.get_local(&counter);
    ctx.builder.const_i32(2);
    ctx.builder.leu_i32();
    ctx.builder.br_if(fallback);
    ctx.builder.get_local(&counter);
    ctx.builder.const_i32(1);
    ctx.builder.sub_i32();
    let n = ctx.builder.set_new_local();
    ctx.builder.const_i32(cpu::LOOP_COUNTER);
    let limit = ctx.builder.set_new_local();
    if unsafe { cpu::vine_jit_exact_instruction_budget } {
        ctx.builder.call_fn0_ret("vine_jit_instruction_limit");
        min_local(ctx.builder, &limit);
    }
    ctx.builder.get_local(&limit);
    ctx.builder.get_local(&ctx.instruction_counter);
    ctx.builder.leu_i32();
    ctx.builder.br_if(fallback);
    ctx.builder.get_local(&limit);
    ctx.builder.get_local(&ctx.instruction_counter);
    ctx.builder.sub_i32();
    ctx.builder.const_i32(p.ops.len() as i32);
    ctx.builder.div_u_i32();
    min_local(ctx.builder, &n);
    ctx.builder.free_local(limit);
    let mut translations: [Option<WasmLocal>; 8] = std::array::from_fn(|_| None);
    for (r, span) in p.spans.iter().enumerate() {
        let Some(s) = span else {
            continue;
        };
        ctx.builder.get_local(&ctx.register_locals[r]);
        ctx.builder.const_i32(s.low);
        ctx.builder.add_i32();
        let low = ctx.builder.set_new_local();
        ctx.builder.get_local(&low);
        ctx.builder.const_i32(0xfff);
        ctx.builder.and_i32();
        let offset = ctx.builder.set_new_local();
        let width = s.high - s.low + 1;
        if s.stride != 0 {
            ctx.builder.get_local(&offset);
            ctx.builder.const_i32(4096 - width);
            ctx.builder.gtu_i32();
            ctx.builder.br_if(fallback);
            ctx.builder.const_i32(4096 - width);
            ctx.builder.get_local(&offset);
            ctx.builder.sub_i32();
            ctx.builder.const_i32(s.stride);
            ctx.builder.div_u_i32();
            ctx.builder.const_i32(1);
            ctx.builder.add_i32();
            min_local(ctx.builder, &n);
        }
        ctx.builder.get_local(&low);
        ctx.builder.const_i32(12);
        ctx.builder.shr_u_i32();
        ctx.builder.const_i32(2);
        ctx.builder.shl_i32();
        ctx.builder
            .load_aligned_i32(unsafe { &cpu::tlb_data[0] as *const i32 as u32 });
        let entry = ctx.builder.tee_new_local();
        let mask = 0xfff
            & !cpu::TLB_GLOBAL
            & !(if ctx.cpu.cpl3() { 0 } else { cpu::TLB_NO_USER })
            & !(if s.write {
                0
            } else {
                cpu::TLB_READONLY | cpu::TLB_HAS_CODE
            });
        ctx.builder.const_i32(mask);
        ctx.builder.and_i32();
        ctx.builder.const_i32(cpu::TLB_VALID);
        ctx.builder.ne_i32();
        ctx.builder.br_if(fallback);
        ctx.builder.get_local(&entry);
        ctx.builder.const_i32(!0xfff);
        ctx.builder.and_i32();
        ctx.builder.get_local(&low);
        ctx.builder.xor_i32();
        ctx.builder.get_local(&low);
        ctx.builder.sub_i32();
        ctx.builder.set_local(&entry);
        if s.stride == 0 && width > 1 {
            ctx.builder.get_local(&offset);
            ctx.builder.const_i32(4096 - width);
            ctx.builder.gtu_i32();
            ctx.builder.if_void();
            ctx.builder.get_local(&low);
            ctx.builder.const_i32(width - 1);
            ctx.builder.add_i32();
            let high = ctx.builder.tee_new_local();
            ctx.builder.const_i32(12);
            ctx.builder.shr_u_i32();
            ctx.builder.const_i32(2);
            ctx.builder.shl_i32();
            ctx.builder
                .load_aligned_i32(unsafe { &cpu::tlb_data[0] as *const i32 as u32 });
            let second = ctx.builder.tee_new_local();
            ctx.builder.const_i32(mask);
            ctx.builder.and_i32();
            ctx.builder.const_i32(cpu::TLB_VALID);
            ctx.builder.ne_i32();
            ctx.builder.br_if(fallback);
            ctx.builder.get_local(&second);
            ctx.builder.const_i32(!0xfff);
            ctx.builder.and_i32();
            ctx.builder.get_local(&high);
            ctx.builder.xor_i32();
            ctx.builder.get_local(&high);
            ctx.builder.sub_i32();
            ctx.builder.get_local(&entry);
            ctx.builder.ne_i32();
            ctx.builder.br_if(fallback);
            ctx.builder.free_local(high);
            ctx.builder.free_local(second);
            ctx.builder.block_end();
        }
        translations[r] = Some(entry);
        ctx.builder.free_local(low);
        ctx.builder.free_local(offset);
    }
    ctx.builder.get_local(&n);
    ctx.builder.const_i32(2);
    ctx.builder.ltu_i32();
    ctx.builder.br_if(fallback);
    ctx.builder.get_local(&n);
    let total = ctx.builder.set_new_local();
    let mmx: Vec<_> = (0..8)
        .map(|r| {
            if p.mmx_used & (1 << r) == 0 {
                return None;
            }
            ctx.builder.load_fixed_i64(gp::get_reg_mmx_offset(r));
            ctx.builder.splat_i64x2();
            Some(ctx.builder.set_new_local_v128())
        })
        .collect();
    ctx.builder.const_i32(0);
    let dirty = ctx.builder.set_new_local();
    ctx.builder.const_i32(0);
    let signed_le = ctx.builder.set_new_local();
    let looping = ctx.builder.loop_void();
    let mut labels = HashMap::new();
    let mut targets: Vec<_> = p.ops[..p.ops.len() - 1]
        .iter()
        .filter_map(|o| match o {
            Op::Branch(_, t) => Some(*t),
            _ => None,
        })
        .collect();
    targets.sort();
    targets.dedup();
    for t in targets.iter().rev() {
        labels.insert(*t, ctx.builder.block_void());
    }
    for (i, &op) in p.ops.iter().enumerate() {
        if labels.contains_key(&p.addresses[i]) {
            ctx.builder.block_end();
        }
        ctx.builder.get_local(&ctx.instruction_counter);
        ctx.builder.const_i32(1);
        ctx.builder.add_i32();
        ctx.builder.set_local(&ctx.instruction_counter);
        let mut mmx_write = None;
        match op {
            Op::Load(r, m) => {
                read(ctx, &p.accesses[m], &translations);
                ctx.builder.set_local(&ctx.reg(r));
            }
            Op::Store(r, m) => {
                address(ctx, &p.accesses[m], &translations);
                ctx.builder.get_local(&ctx.reg(r));
                ctx.builder.store_unaligned_i32(0);
            }
            Op::Or(r, s) => {
                ctx.builder.get_local(&ctx.reg(r));
                ctx.builder.get_local(&ctx.reg(s));
                ctx.builder.or_i32();
                ctx.builder.set_local(&ctx.reg(r));
            }
            Op::AddMemory(r, m) => {
                read(ctx, &p.accesses[m], &translations);
                let rhs = ctx.builder.set_new_local();
                if matches!(p.ops.get(i + 1), Some(Op::Branch(14, _))) {
                    ctx.builder.get_local(&ctx.reg(r));
                    ctx.builder.extend_signed_i32_to_i64();
                    ctx.builder.get_local(&rhs);
                    ctx.builder.extend_signed_i32_to_i64();
                    ctx.builder.add_i64();
                    ctx.builder.const_i64(0);
                    ctx.builder.le_i64();
                    ctx.builder.set_local(&signed_le);
                }
                ctx.builder.get_local(&ctx.reg(r));
                ctx.builder.get_local(&rhs);
                ctx.builder.add_i32();
                ctx.builder.set_local(&ctx.reg(r));
                ctx.builder.free_local(rhs);
            }
            Op::AddImmediate(r, v, _) => {
                ctx.builder.get_local(&ctx.reg(r));
                ctx.builder.const_i32(v);
                ctx.builder.add_i32();
                ctx.builder.set_local(&ctx.reg(r));
            }
            Op::Shift(r, left, count) => {
                ctx.builder.get_local(&ctx.reg(r));
                if let Some(n) = count {
                    ctx.builder.const_i32(n as i32);
                } else {
                    ctx.builder.get_local(&ctx.reg(regs::ECX));
                }
                if left {
                    ctx.builder.shl_i32();
                } else {
                    ctx.builder.shr_u_i32();
                }
                ctx.builder.set_local(&ctx.reg(r));
            }
            Op::Compare(_, _) => {}
            Op::Branch(condition, target) if i + 1 < p.ops.len() => {
                if condition == 14 {
                    ctx.builder.get_local(&signed_le);
                } else if let Op::Compare(r, v) = p.ops[i - 1] {
                    ctx.builder.get_local(&ctx.reg(r));
                    ctx.builder.const_i32(v);
                    ctx.builder.geu_i32();
                }
                ctx.builder.br_if(labels[&target]);
            }
            Op::Branch(_, _) => {}
            Op::MmxLoad(r, m) => {
                read(ctx, &p.accesses[m], &translations);
                if p.accesses[m].bytes == 4 {
                    ctx.builder.extend_unsigned_i32_to_i64();
                }
                ctx.builder.splat_i64x2();
                ctx.builder
                    .set_local_v128(mmx[r as usize].as_ref().unwrap());
                mmx_write = Some(r);
            }
            Op::MmxStore(r, m) => {
                address(ctx, &p.accesses[m], &translations);
                ctx.builder
                    .get_local_v128(mmx[r as usize].as_ref().unwrap());
                ctx.builder.extract_low_i64x2();
                if p.accesses[m].bytes == 4 {
                    ctx.builder.wrap_i64_to_i32();
                    ctx.builder.store_unaligned_i32(0);
                } else {
                    ctx.builder.store_unaligned_i64(0);
                }
                mmx_write = Some(8);
            }
            Op::MmxShift(r, shift) => {
                ctx.builder
                    .get_local_v128(mmx[r as usize].as_ref().unwrap());
                ctx.builder.const_i32(shift as i32);
                ctx.builder.shr_s_i16x8();
                ctx.builder
                    .set_local_v128(mmx[r as usize].as_ref().unwrap());
                mmx_write = Some(r);
            }
            Op::MmxBinary(op, r, source) => {
                ctx.builder
                    .get_local_v128(mmx[r as usize].as_ref().unwrap());
                match source {
                    Source::Register(s) => ctx
                        .builder
                        .get_local_v128(mmx[s as usize].as_ref().unwrap()),
                    Source::Memory(m) => {
                        read(ctx, &p.accesses[m], &translations);
                        if p.accesses[m].bytes == 4 {
                            ctx.builder.extend_unsigned_i32_to_i64();
                        }
                        ctx.builder.splat_i64x2();
                    }
                }
                match op {
                    Packed::UnpackBytes => ctx
                        .builder
                        .i8x16_shuffle([0, 16, 1, 17, 2, 18, 3, 19, 4, 20, 5, 21, 6, 22, 7, 23]),
                    Packed::SubWords => ctx.builder.sub_i16x8(),
                    Packed::MulWords => ctx.builder.mul_i16x8(),
                    Packed::AddWords => ctx.builder.add_i16x8(),
                    Packed::PackBytes => {
                        ctx.builder.i8x16_shuffle([
                            0, 1, 2, 3, 4, 5, 6, 7, 16, 17, 18, 19, 20, 21, 22, 23,
                        ]);
                        ctx.builder.v128_const([0; 16]);
                        ctx.builder.i8x16_narrow_i16x8_u();
                    }
                }
                ctx.builder
                    .set_local_v128(mmx[r as usize].as_ref().unwrap());
                mmx_write = Some(r);
            }
        }
        if let Some(r) = mmx_write {
            ctx.builder.get_local(&dirty);
            ctx.builder.const_i32(1 << r);
            ctx.builder.or_i32();
            ctx.builder.set_local(&dirty);
        }
    }
    ctx.builder.get_local(&n);
    ctx.builder.const_i32(1);
    ctx.builder.sub_i32();
    ctx.builder.tee_local(&n);
    ctx.builder.br_if(looping);
    ctx.builder.block_end();
    for (r, local) in mmx.into_iter().enumerate() {
        let Some(local) = local else {
            continue;
        };
        if p.mmx_written & (1 << r) != 0 {
            ctx.builder.get_local(&dirty);
            ctx.builder.const_i32(1 << r);
            ctx.builder.and_i32();
            ctx.builder.if_void();
            let offset = gp::get_reg_mmx_offset(r as u32);
            ctx.builder.const_i32(0);
            ctx.builder.get_local_v128(&local);
            ctx.builder.extract_low_i64x2();
            ctx.builder.store_aligned_i64(offset);
            ctx.builder.const_i32(0);
            ctx.builder.const_i32(0xffff);
            ctx.builder.store_aligned_u16(offset + 8);
            ctx.builder.block_end();
        }
        ctx.builder.free_local_v128(local);
    }
    ctx.builder.get_local(&dirty);
    ctx.builder.if_void();
    for pointer in [gp::fpu_stack_empty, gp::fpu_stack_ptr] {
        ctx.builder.const_i32(0);
        ctx.builder.const_i32(0);
        ctx.builder.store_u8(pointer as u32);
    }
    ctx.builder.block_end();
    // Reconstruct the exact final ADD's carry and DEC's lazy arithmetic flags.
    ctx.builder.const_i32(0);
    ctx.builder.load_fixed_i32(gp::flags as u32);
    ctx.builder.const_i32(!1);
    ctx.builder.and_i32();
    ctx.builder.get_local(&ctx.reg(p.carry_register));
    ctx.builder.get_local(&ctx.reg(p.carry_register));
    ctx.builder.const_i32(p.carry_add);
    ctx.builder.sub_i32();
    ctx.builder.ltu_i32();
    ctx.builder.or_i32();
    ctx.builder.store_aligned_i32(gp::flags as u32);
    ctx.builder.const_i32(0);
    ctx.builder.get_local(&counter);
    ctx.builder.const_i32(1);
    ctx.builder.add_i32();
    ctx.builder.store_aligned_i32(gp::last_op1 as u32);
    codegen::gen_set_last_result(ctx.builder, &counter);
    codegen::gen_set_last_op_size_and_flags_changed(
        ctx.builder,
        cpu::OPSIZE_32,
        cpu::FLAGS_ALL & !1 | cpu::FLAG_SUB,
    );
    let metric = std::ptr::addr_of_mut!(ITERATIONS) as u32;
    ctx.builder.const_i32(0);
    ctx.builder.load_fixed_i64(metric);
    ctx.builder.get_local(&total);
    ctx.builder.extend_unsigned_i32_to_i64();
    ctx.builder.add_i64();
    ctx.builder.store_aligned_i64(metric);
    ctx.builder.free_local(total);
    ctx.builder.free_local(dirty);
    ctx.builder.free_local(signed_le);
    ctx.builder.free_local(n);
    for t in translations.into_iter().flatten() {
        ctx.builder.free_local(t);
    }
    ctx.builder.block_end();
}

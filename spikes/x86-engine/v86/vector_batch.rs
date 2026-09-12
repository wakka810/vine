//! Checked straight-line SIMD batches; a failed preflight runs the original JIT.
//! Guest mappings and bytes remain owned by the existing execution island.
use crate::cpu::{cpu, global_pointers as gp};
use crate::cpu_context::CpuContext;
use crate::jit::{BasicBlock, JitContext};
use crate::wasmgen::wasm_builder::{Label, WasmLocal};
use crate::{codegen, jit_instructions, modrm};

#[derive(Clone, Copy)]
struct Address { base: Option<u32>, index: Option<u32>, shift: u8, offset: i32 }
struct Access { address: Address, bytes: i32, write: bool, span: usize, aligned: bool }
#[derive(Clone, Copy)]
enum Source { Reg(u32), Mem(usize) }
#[derive(Clone, Copy)]
enum Op {
    Load(bool, u32, usize), MmxToGpr(u32, u32), Byte(u32, u32), Shr(u32, u8),
    Insert(u32, usize, u8), Binary(bool, u8, u32, Source), Unary(u8, u32, u32),
    MaskStore(u32, u32, usize), Store(u32, usize), Move(u32, u32), MmxToXmm(u32,u32),
    Shift(u8,u32,u8), Shuffle(u32,Source,u8),
}
#[derive(Clone, Copy)]
struct Value { terms: [i32; 8], low: i64, high: i64 }
struct Span { terms: [i32; 8], low: i32, high: i32, write: bool }
struct Plan {
    ops: Vec<Op>, accesses: Vec<Access>, spans: Vec<Span>, end: u32,
    xmm_used: u8, xmm_written: u8, mmx_used: u8, mmx_written: u8,
}
fn access(c: &mut CpuContext, b: u8, bytes: i32, all: &mut Vec<Access>) -> Option<usize> {
    if b >= 0xc0 { return None; }
    let (base, index, shift, offset) = modrm::decode(c, b).flat_parts()?;
    let n = all.len();
    all.push(Access {address: Address {base, index, shift, offset}, bytes, write: false, span: 0, aligned: bytes == 16});
    Some(n)
}
fn decode(c: &mut CpuContext, all: &mut Vec<Access>) -> Option<Op> {
    let first = c.read_imm8();
    if first == 0xc1 {
        let b = c.read_imm8();
        if b & 0xf8 != 0xe8 { return None; }
        return Some(Op::Shr((b & 7) as u32, c.read_imm8() & 31));
    }
    let (prefix, escape) = if matches!(first, 0x66 | 0xf3) { (first, c.read_imm8()) } else { (0, first) };
    if escape != 0x0f { return None; }
    let op = c.read_imm8(); let b = c.read_imm8();
    let (r, s) = ((b >> 3 & 7) as u32, (b & 7) as u32);
    Some(match (prefix, op) {
        (0xf3, 0x7e) => Op::Load(true, r, access(c,b,8,all)?),
        (0, 0x6f) => Op::Load(false, r, access(c,b,8,all)?),
        (0, 0x7e) if b >= 0xc0 => Op::MmxToGpr(s,r),
        (0, 0xb6) if b >= 0xc0 => Op::Byte(r,s),
        (0x66, 0xc4) if b < 0xc0 => {
            let m = access(c,b,2,all)?; Op::Insert(r,m,c.read_imm8() & 7)
        }
        (0, 0x67) if b >= 0xc0 => Op::Binary(false,op,r,Source::Reg(s)),
        (0x66, 0x60 | 0x61 | 0x69 | 0x6b | 0x67 | 0xef | 0xeb | 0xfd) if b >= 0xc0 =>
            Op::Binary(true,op,r,Source::Reg(s)),
        (0, 0x58 | 0x59 | 0x5c) => Op::Binary(true,op,r,
            if b >= 0xc0 {Source::Reg(s)} else {Source::Mem(access(c,b,16,all)?)}),
        (0, 0x28) | (0x66, 0x6f) => if b >= 0xc0 {Op::Move(r,s)} else {Op::Load(true,r,access(c,b,16,all)?)},
        (0, 0x29) | (0x66, 0x7f) => if b >= 0xc0 {Op::Move(s,r)} else {
            let m = access(c,b,16,all)?; all[m].write = true; Op::Store(r,m)
        },
        (0xf3,0x10) if b < 0xc0 => Op::Load(true,r,access(c,b,4,all)?),
        (0xf3,0xd6) if b >= 0xc0 => Op::MmxToXmm(r,s),
        (0x66,0x71 | 0x72) if b >= 0xc0 && r == 4 => Op::Shift(op,s,c.read_imm8()),
        (0,0xc6) => {
            let source = if b >= 0xc0 {Source::Reg(s)} else {Source::Mem(access(c,b,16,all)?)};
            Op::Shuffle(r,source,c.read_imm8())
        },
        (0, 0x5b) if b >= 0xc0 => Op::Unary(0,r,s),
        (0xf3, 0x5b) if b >= 0xc0 => Op::Unary(1,r,s),
        (0x66, 0xf7) if b >= 0xc0 => {
            let m = all.len();
            all.push(Access {address: Address {base: Some(7), index: None, shift: 0, offset: 0},
                bytes: 16, write: true, span: 0, aligned: false});
            Op::MaskStore(r,s,m)
        }
        _ => return None,
    })
}
fn plan(ctx: &JitContext, block: &BasicBlock) -> Option<Plan> {
    if !ctx.cpu.state_flags.is_32() || !ctx.cpu.has_flat_segmentation() || block.has_sti { return None; }
    let start = ctx.cpu.eip;
    let mut p = Plan {ops: Vec::new(), accesses: Vec::new(), spans: Vec::new(), end: start,
        xmm_used: 0, xmm_written: 0, mmx_used: 0, mmx_written: 0};
    let mut c = ctx.cpu.clone(); c.prefixes = 0;
    // ponytail: one existing basic block, <=192 instructions. No control-flow framework.
    while c.eip < block.last_instruction_addr && p.ops.len() < 192 {
        let old = p.accesses.len();
        let Some(op) = decode(&mut c, &mut p.accesses) else { p.accesses.truncate(old); break };
        if c.eip > block.last_instruction_addr { p.accesses.truncate(old); break; }
        p.ops.push(op); p.end = c.eip;
    }
    if p.ops.len() < 12 || p.accesses.is_empty() { return None; }
    let mut state: [Value;8] = std::array::from_fn(|r| {
        let mut terms = [0;8]; terms[r] = 1; Value {terms,low:0,high:0}
    });
    for &op in &p.ops {
        let m = match op {
            Op::Load(_,_,m) | Op::Insert(_,m,_) | Op::MaskStore(_,_,m) | Op::Store(_,m)
            | Op::Binary(_,_,_,Source::Mem(m)) | Op::Shuffle(_,Source::Mem(m),_) => Some(m), _ => None
        };
        if let Some(m) = m {
            let a = &mut p.accesses[m];
            let mut value = Value {terms:[0;8],low:a.address.offset as i64,high:a.address.offset as i64};
            for (r, scale) in [(a.address.base,1), (a.address.index,1 << a.address.shift)] {
                if let Some(r) = r {
                    let s = state[r as usize];
                    // Alignment guards run before the body: require unchanged address registers.
                    if a.aligned && (s.low != 0 || s.high != 0 || s.terms.iter().enumerate()
                        .any(|(i,&v)| v != if i == r as usize {1} else {0})) { return None; }
                    for i in 0..8 { value.terms[i] += s.terms[i] * scale; }
                    value.low += s.low * scale as i64; value.high += s.high * scale as i64;
                }
            }
            let low = i32::try_from(value.low).ok()?;
            let high = i32::try_from(value.high + a.bytes as i64 - 1).ok()?;
            if high as i64 - low as i64 >= 4096 { return None; }
            a.span = if let Some(i) = p.spans.iter().position(|s| s.terms == value.terms) {
                let s = &mut p.spans[i]; s.low = s.low.min(low); s.high = s.high.max(high); s.write |= a.write;
                if s.high as i64 - s.low as i64 >= 4096 { return None; } i
            } else { let i = p.spans.len(); p.spans.push(Span {terms:value.terms,low,high,write:a.write}); i };
        }
        let mut read = |xmm:bool,r:u32| { if xmm { p.xmm_used |= 1 << r; } else { p.mmx_used |= 1 << r; } };
        match op {
            Op::MmxToGpr(r,s) => { read(false,s); state[r as usize] = Value {terms:[0;8],low:0,high:u32::MAX as i64}; }
            Op::Byte(r,_) => state[r as usize] = Value {terms:[0;8],low:0,high:255},
            Op::Shr(r,n) => {
                if n != 0 { state[r as usize] = Value {terms:[0;8],low:0,high:(u32::MAX >> n) as i64}; }
            }
            Op::Load(xmm,r,_) => { read(xmm,r); if xmm {p.xmm_written |= 1 << r;} else {p.mmx_written |= 1 << r;} }
            Op::Insert(r,_,_) => { read(true,r); p.xmm_written |= 1 << r; }
            Op::Binary(xmm,_,r,s) => {
                read(xmm,r); if let Source::Reg(s) = s {read(xmm,s);}
                if xmm {p.xmm_written |= 1 << r;} else {p.mmx_written |= 1 << r;}
            }
            Op::Unary(_,r,s) | Op::Move(r,s) => { read(true,r); read(true,s); p.xmm_written |= 1 << r; }
            Op::MmxToXmm(r,s) => { read(true,r); read(false,s); p.xmm_written |= 1 << r; }
            Op::Shift(_,r,_) => { read(true,r); p.xmm_written |= 1 << r; }
            Op::Shuffle(r,s,_) => {
                read(true,r); if let Source::Reg(s) = s {read(true,s);} p.xmm_written |= 1 << r;
            }
            Op::Store(r,_) => read(true,r),
            Op::MaskStore(r,s,_) => { read(true,r); read(true,s); }
        }
    }
    if p.spans.len() > 8 || (p.xmm_written.count_ones() + p.mmx_written.count_ones()) < 2 { return None; }
    Some(p)
}

fn checked_translation(ctx: &mut JitContext, address: &WasmLocal, write:bool, fallback:Label) -> WasmLocal {
    let b = &mut ctx.builder;
    b.get_local(address); b.const_i32(12); b.shr_u_i32(); b.const_i32(2); b.shl_i32();
    b.load_aligned_i32(std::ptr::addr_of!(cpu::tlb_data) as u32);
    let entry = b.tee_new_local();
    b.const_i32(0xfff & !cpu::TLB_GLOBAL & !(if ctx.cpu.cpl3() {0} else {cpu::TLB_NO_USER})
        & !(if write {0} else {cpu::TLB_READONLY | cpu::TLB_HAS_CODE}));
    b.and_i32(); b.const_i32(cpu::TLB_VALID); b.ne_i32(); b.br_if(fallback);
    b.get_local(&entry); b.const_i32(!0xfff); b.and_i32(); b.get_local(address); b.xor_i32();
    b.get_local(address); b.sub_i32(); b.set_local(&entry); entry
}
fn address(ctx: &mut JitContext, a:&Access, translations:&[WasmLocal]) {
    let b = &mut ctx.builder;
    b.const_i32(a.address.offset);
    if let Some(r) = a.address.base { b.get_local(&ctx.register_locals[r as usize]); b.add_i32(); }
    if let Some(r) = a.address.index {
        b.get_local(&ctx.register_locals[r as usize]); b.const_i32(a.address.shift as i32); b.shl_i32(); b.add_i32();
    }
    b.get_local(&translations[a.span]); b.add_i32();
}
static mut BATCHES: u64 = 0;
#[no_mangle]
pub unsafe fn vine_vector_batches() -> f64 { BATCHES as f64 }

pub fn generate(ctx: &mut JitContext, block:&BasicBlock) -> Option<(u32,Label)> {
    let p = plan(ctx,block)?;
    let done = ctx.builder.block_void(); let fallback = ctx.builder.block_void();
    ctx.builder.load_fixed_i32(gp::cr as u32); ctx.builder.const_i32(cpu::CR0_EM | cpu::CR0_TS);
    ctx.builder.and_i32(); ctx.builder.br_if(fallback);
    if p.xmm_used != 0 {
        ctx.builder.load_fixed_i32(gp::cr as u32 + 16); ctx.builder.const_i32(cpu::CR4_OSFXSR);
        ctx.builder.and_i32(); ctx.builder.eqz_i32(); ctx.builder.br_if(fallback);
    }
    ctx.builder.load_fixed_i32(gp::mxcsr as u32); ctx.builder.const_i32(!0x3f); ctx.builder.and_i32();
    ctx.builder.const_i32(0x1f80); ctx.builder.ne_i32(); ctx.builder.br_if(fallback);
    let mut translations = Vec::new();
    for s in &p.spans {
        ctx.builder.const_i32(s.low);
        for (r,&scale) in s.terms.iter().enumerate() {
            if scale == 0 {continue;}
            ctx.builder.get_local(&ctx.register_locals[r]);
            if scale != 1 { ctx.builder.const_i32(scale); ctx.builder.mul_i32(); }
            ctx.builder.add_i32();
        }
        let low = ctx.builder.set_new_local();
        let translation = checked_translation(ctx,&low,s.write,fallback);
        ctx.builder.get_local(&low); ctx.builder.const_i32(0xfff); ctx.builder.and_i32();
        ctx.builder.const_i32(4095 - (s.high-s.low)); ctx.builder.gtu_i32(); ctx.builder.if_void();
        ctx.builder.get_local(&low); ctx.builder.const_i32(s.high-s.low); ctx.builder.add_i32();
        let high = ctx.builder.set_new_local();
        let other = checked_translation(ctx,&high,s.write,fallback);
        ctx.builder.get_local(&translation); ctx.builder.get_local(&other); ctx.builder.ne_i32(); ctx.builder.br_if(fallback);
        ctx.builder.free_local(high); ctx.builder.free_local(other); ctx.builder.block_end();
        ctx.builder.free_local(low); translations.push(translation);
    }
    for a in p.accesses.iter().filter(|a| a.aligned) {
        address(ctx,a,&translations); ctx.builder.const_i32(15); ctx.builder.and_i32(); ctx.builder.br_if(fallback);
    }
    let mmx:Vec<_> = (0..8).map(|r| if p.mmx_used & (1 << r) == 0 {None} else {
        ctx.builder.load_fixed_v128(gp::get_reg_mmx_offset(r)); Some(ctx.builder.set_new_local_v128())
    }).collect();
    let xmm:Vec<_> = (0..8).map(|r| if p.xmm_used & (1 << r) == 0 {None} else {
        ctx.builder.load_fixed_v128(gp::get_reg_xmm_offset(r)); Some(ctx.builder.set_new_local_v128())
    }).collect();
    for &op in &p.ops {
        match op {
            Op::Load(is_xmm,r,m) => {
                address(ctx,&p.accesses[m],&translations);
                let size = p.accesses[m].bytes;
                if size == 16 {ctx.builder.load_aligned_v128(0);}
                else if size == 8 {ctx.builder.load_unaligned_i64(0); ctx.builder.splat_i64x2();}
                else {ctx.builder.load_unaligned_i32(0); ctx.builder.splat_i32x4();}
                if is_xmm && size < 16 {
                    ctx.builder.v128_const([0;16]);
                    ctx.builder.i8x16_shuffle(std::array::from_fn(|i| if i < size as usize {i as u8} else {16}));
                }
                ctx.builder.set_local_v128(if is_xmm {xmm[r as usize].as_ref().unwrap()} else {mmx[r as usize].as_ref().unwrap()});
            }
            Op::Move(r,s) | Op::MmxToXmm(r,s) => {
                let from_mmx = matches!(op,Op::MmxToXmm(..));
                ctx.builder.get_local_v128(if from_mmx {mmx[s as usize].as_ref().unwrap()} else {xmm[s as usize].as_ref().unwrap()});
                if from_mmx {
                    ctx.builder.v128_const([0;16]);
                    ctx.builder.i8x16_shuffle([0,1,2,3,4,5,6,7,16,17,18,19,20,21,22,23]);
                }
                ctx.builder.set_local_v128(xmm[r as usize].as_ref().unwrap());
            }
            Op::Store(r,m) => {
                address(ctx,&p.accesses[m],&translations); ctx.builder.get_local_v128(xmm[r as usize].as_ref().unwrap());
                ctx.builder.store_aligned_v128(0);
            }
            Op::Shift(op,r,n) => {
                ctx.builder.get_local_v128(xmm[r as usize].as_ref().unwrap());
                ctx.builder.const_i32(n.min(if op == 0x71 {15} else {31}) as i32);
                if op == 0x71 {ctx.builder.shr_s_i16x8();} else {ctx.builder.shr_s_i32x4();}
                ctx.builder.set_local_v128(xmm[r as usize].as_ref().unwrap());
            }
            Op::Shuffle(r,s,imm) => {
                ctx.builder.get_local_v128(xmm[r as usize].as_ref().unwrap());
                match s {
                    Source::Reg(s) => ctx.builder.get_local_v128(xmm[s as usize].as_ref().unwrap()),
                    Source::Mem(m) => {address(ctx,&p.accesses[m],&translations); ctx.builder.load_aligned_v128(0);}
                }
                ctx.builder.i8x16_shuffle(std::array::from_fn(|i| {
                    (if i < 8 {0} else {16}) + (imm >> (i/4*2) & 3) * 4 + (i%4) as u8
                }));
                ctx.builder.set_local_v128(xmm[r as usize].as_ref().unwrap());
            }
            Op::MmxToGpr(r,s) => {
                ctx.builder.get_local_v128(mmx[s as usize].as_ref().unwrap()); ctx.builder.extract_low_i64x2();
                ctx.builder.wrap_i64_to_i32(); ctx.builder.set_local(&ctx.reg(r));
            }
            Op::Byte(r,s) => {
                codegen::gen_get_reg8(ctx,s); ctx.builder.set_local(&ctx.reg(r));
            }
            Op::Shr(r,n) => jit_instructions::instr32_C1_5_reg_jit(ctx,r,n as u32),
            Op::Insert(r,m,lane) => {
                ctx.builder.get_local_v128(xmm[r as usize].as_ref().unwrap());
                address(ctx,&p.accesses[m],&translations); ctx.builder.load_unaligned_u16(0); ctx.builder.replace_i16x8_lane(lane);
                ctx.builder.set_local_v128(xmm[r as usize].as_ref().unwrap());
            }
            Op::Binary(is_xmm,op,r,s) => {
                let vectors = if is_xmm {&xmm} else {&mmx};
                ctx.builder.get_local_v128(vectors[r as usize].as_ref().unwrap());
                match s {
                    Source::Reg(s) => ctx.builder.get_local_v128(vectors[s as usize].as_ref().unwrap()),
                    Source::Mem(m) => {address(ctx,&p.accesses[m],&translations); ctx.builder.load_aligned_v128(0);}
                }
                match op {
                    0x60 => ctx.builder.i8x16_shuffle([0,16,1,17,2,18,3,19,4,20,5,21,6,22,7,23]),
                    0x61 => ctx.builder.i8x16_shuffle([0,1,16,17,2,3,18,19,4,5,20,21,6,7,22,23]),
                    0x69 => ctx.builder.i8x16_shuffle([8,9,24,25,10,11,26,27,12,13,28,29,14,15,30,31]),
                    0x6b => ctx.builder.i16x8_narrow_i32x4_s(),
                    0x67 => {
                        if !is_xmm {
                            ctx.builder.i8x16_shuffle([0,1,2,3,4,5,6,7,16,17,18,19,20,21,22,23]);
                            ctx.builder.v128_const([0;16]);
                        }
                        ctx.builder.i8x16_narrow_i16x8_u();
                    }
                    0x58 => ctx.builder.add_f32x4(),
                    0x59 => ctx.builder.mul_f32x4(),
                    0x5c => ctx.builder.sub_f32x4(),
                    0xef => ctx.builder.xor_v128(),
                    0xeb => ctx.builder.or_v128(),
                    0xfd => ctx.builder.add_i16x8(),
                    _ => unreachable!(),
                }
                ctx.builder.set_local_v128(vectors[r as usize].as_ref().unwrap());
            }
            Op::Unary(op,r,s) => {
                let source = xmm[s as usize].as_ref().unwrap();
                ctx.builder.get_local_v128(source);
                if op == 0 {ctx.builder.convert_i32x4_s();} else {
                    ctx.builder.trunc_sat_f32x4_s();
                    ctx.builder.v128_const([0,0,0,128,0,0,0,128,0,0,0,128,0,0,0,128]);
                    ctx.builder.get_local_v128(source);
                    ctx.builder.v128_const([0,0,0,79,0,0,0,79,0,0,0,79,0,0,0,79]);
                    ctx.builder.lt_f32x4(); ctx.builder.bitselect_v128();
                }
                ctx.builder.set_local_v128(xmm[r as usize].as_ref().unwrap());
            }
            Op::MaskStore(r,s,m) => {
                address(ctx,&p.accesses[m],&translations); let dest = ctx.builder.tee_new_local();
                ctx.builder.get_local_v128(xmm[r as usize].as_ref().unwrap());
                ctx.builder.get_local(&dest); ctx.builder.load_aligned_v128(0);
                ctx.builder.get_local_v128(xmm[s as usize].as_ref().unwrap()); ctx.builder.v128_const([0;16]);
                ctx.builder.lt_s_i8x16(); ctx.builder.bitselect_v128(); ctx.builder.store_aligned_v128(0);
                ctx.builder.free_local(dest);
            }
        }
    }
    for (is_xmm,vectors) in [(true,xmm),(false,mmx)] {
        let written = if is_xmm {p.xmm_written} else {p.mmx_written};
        for (r,v) in vectors.into_iter().enumerate() {
            let Some(v) = v else {continue;};
            if written & (1 << r) != 0 {
                ctx.builder.const_i32(0); ctx.builder.get_local_v128(&v);
                if is_xmm {ctx.builder.store_aligned_v128(gp::get_reg_xmm_offset(r as u32));}
                else {
                    ctx.builder.extract_low_i64x2(); let offset = gp::get_reg_mmx_offset(r as u32);
                    ctx.builder.store_aligned_i64(offset);
                    ctx.builder.const_i32(0); ctx.builder.const_i32(0xffff); ctx.builder.store_aligned_u16(offset+8);
                }
            }
            ctx.builder.free_local_v128(v);
        }
    }
    if p.mmx_used != 0 {
        for ptr in [gp::fpu_stack_empty,gp::fpu_stack_ptr] {
            ctx.builder.const_i32(0); ctx.builder.const_i32(0); ctx.builder.store_u8(ptr as u32);
        }
    }
    for t in translations {ctx.builder.free_local(t);}
    let metric = std::ptr::addr_of_mut!(BATCHES) as u32;
    ctx.builder.const_i32(0); ctx.builder.load_fixed_i64(metric); ctx.builder.const_i64(1);
    ctx.builder.add_i64(); ctx.builder.store_aligned_i64(metric);
    ctx.builder.br(done); ctx.builder.block_end();
    Some((p.end,done))
}

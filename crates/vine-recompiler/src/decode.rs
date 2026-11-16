//! Bounded x86 discovery using the MIT-licensed iced-x86 decoder only.
//!
//! Instruction semantics, graph ownership, and exits belong to this backend.
//! A failed fetch or unsupported instruction is retained at its exact guest PC
//! so that execution can stop there without replaying completed instructions.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
mod x87;

use iced_x86::{
    Code, ConditionCode, Decoder, DecoderError, DecoderOptions, Instruction as X86Instruction,
    Mnemonic, OpKind, Register as X86Register, RflagsBits,
};

use crate::ir::{
    Block, Condition, Instruction, MemoryOperand, MmxOpcode, Opcode, Operand, Region, Register,
    Segment, SseOpcode, StringOpcode, UnsupportedReason, Width,
};

/// Returns a contiguous prefix of executable guest bytes, bounded by max_len.
/// It must stop at an unmapped/non-executable address or a 32-bit wrap. Reads
/// neither mutate guest memory nor bypass the authoritative mapping checks.
pub trait CodeSource {
    fn read_code(&self, address: u32, max_len: usize) -> Vec<u8>;

    /// Optional authorized data read for binary-derived control discovery.
    /// Unavailable/truncated data supplies no hint; it is never a guest fault.
    /// Execution still performs the original access and all of its checks.
    fn read_data(&self, _address: u32, _max_len: usize) -> Vec<u8> {
        Vec::new()
    }

    /// A host observation boundary, checked before instruction fetch/decode.
    /// Ordinary sources never introduce one or modify guest code bytes.
    fn diagnostic_stop(&self, _address: u32) -> bool {
        false
    }

    /// A known raw entry should split a straight-line block when encountered.
    /// This is an admission port, not a new control-flow edge or code byte.
    fn entry_hint(&self, _address: u32) -> bool {
        false
    }

    /// An already prepared entry can be reached through the ordinary external
    /// edge instead of duplicating its body. Invocation still validates code.
    fn external_entry(&self, _address: u32) -> bool {
        false
    }
}

pub fn discover(source: &impl CodeSource, entry: u32, max_instructions: usize) -> Region {
    discover_many(source, &[entry], max_instructions)
}

/// Discover a union of reached entry points with one shared instruction bound.
/// Initial work is round-robin breadth-first, so a long sequential path from
/// one root cannot consume the entire allowance before another root is visited.
pub fn discover_many(source: &impl CodeSource, entries: &[u32], max_instructions: usize) -> Region {
    discover_with_edges(source, entries, max_instructions, |_, _| Vec::new())
}

pub(crate) fn discover_with_edges(
    source: &impl CodeSource,
    entries: &[u32],
    max_instructions: usize,
    mut extra_edges: impl FnMut(&Instruction, &BTreeMap<u32, Instruction>) -> Vec<u32>,
) -> Region {
    let entry = entries.first().copied().unwrap_or_default();
    let mut instructions = BTreeMap::new();
    let mut leaders: BTreeSet<_> = entries.iter().copied().collect();
    let mut pending = VecDeque::new();
    let mut seeded = BTreeSet::new();
    pending.extend(
        entries
            .iter()
            .copied()
            .filter(|entry| seeded.insert(*entry)),
    );
    let mut incoming = BTreeMap::<u32, usize>::new();
    let mut discovered_edges = BTreeMap::new();
    while instructions.len() < max_instructions {
        if pending.is_empty() {
            // A separate AOT root may decode a table transfer before the code
            // that describes its range. Reach a bounded edge fixed point after
            // each wave instead of making semantics/coverage root-order dependent.
            for instruction in instructions.values() {
                for target in extra_edges(instruction, &instructions) {
                    let edges = discovered_edges
                        .entry(instruction.eip)
                        .or_insert_with(Vec::new);
                    if !edges.contains(&target) {
                        edges.push(target);
                        leaders.insert(target);
                        *incoming.entry(target).or_default() += 1;
                        if !instructions.contains_key(&target) {
                            pending.push_back(target);
                        }
                    }
                }
            }
        }
        let Some(eip) = pending.pop_front() else {
            break;
        };
        if instructions.contains_key(&eip) {
            continue;
        }
        if !seeded.contains(&eip) && source.external_entry(eip) {
            continue;
        }
        if source.entry_hint(eip) {
            leaders.insert(eip);
        }
        let instruction = if source.diagnostic_stop(eip) {
            leaders.insert(eip);
            Instruction {
                eip,
                len: 0,
                opcode: Opcode::DiagnosticStop,
                width: Width::W32,
                operands: Vec::new(),
                condition: None,
                flags_read: 0,
                flags_written: 0,
                flags_undefined: 0,
                unsupported: None,
            }
        } else {
            decode_one(eip, &source.read_code(eip, 15))
        };
        if matches!(instruction.opcode, Opcode::String(_, true)) {
            // A partially completed REP resumes at this exact instruction,
            // independently of earlier instructions in the discovery region.
            leaders.insert(eip);
        }
        let mut successors = instruction.successors();
        for target in extra_edges(&instruction, &instructions) {
            if !successors.contains(&target) {
                successors.push(target);
            }
        }
        if instruction.ends_block() {
            leaders.extend(&successors);
        }
        discovered_edges.insert(eip, successors.clone());
        for target in successors {
            *incoming.entry(target).or_default() += 1;
            if !instructions.contains_key(&target) {
                pending.push_back(target);
            }
        }
        instructions.insert(eip, instruction);
    }
    let limit_reached = pending.iter().any(|eip| !instructions.contains_key(eip));
    leaders.extend(
        incoming
            .into_iter()
            .filter_map(|(eip, count)| (count > 1).then_some(eip)),
    );

    // Build maximal straight-line blocks after all known edges are present.
    // Separate decodings at overlapping x86 instruction boundaries are legal.
    let mut starts: Vec<_> = leaders.iter().copied().collect();
    starts.extend(
        instructions
            .keys()
            .copied()
            .filter(|eip| !leaders.contains(eip)),
    );
    let mut blocks = BTreeMap::new();
    for start in starts {
        if !instructions.contains_key(&start) {
            continue;
        }
        let mut eip = start;
        let mut body = Vec::new();
        let successors = loop {
            let instruction = instructions.remove(&eip).expect("block instruction exists");
            let successors = discovered_edges
                .remove(&eip)
                .expect("discovered instruction edges");
            let ends_block = instruction.ends_block();
            body.push(instruction);
            if ends_block
                || successors.len() != 1
                || leaders.contains(&successors[0])
                || !instructions.contains_key(&successors[0])
            {
                break successors;
            }
            eip = successors[0];
        };
        blocks.insert(
            start,
            Block {
                start,
                instructions: body,
                successors,
            },
        );
    }
    Region {
        entry,
        blocks,
        limit_reached,
    }
}

pub fn decode_one(eip: u32, bytes: &[u8]) -> Instruction {
    let bytes = &bytes[..bytes.len().min(15)];
    if bytes.is_empty() {
        return unsupported(eip, 0, UnsupportedReason::FetchUnavailable);
    }
    let mut decoder = Decoder::with_ip(32, bytes, u64::from(eip), DecoderOptions::NONE);
    let decoded = decoder.decode();
    let len = decoded.len() as u8;
    if decoded.is_invalid() {
        // iced reports both malformed encodings and overlength instructions as
        // InvalidInstruction after consuming its full 15-byte window. These
        // do not have the same x86 exception (#UD versus #GP); until separated,
        // retain an explicit unsupported exit instead of manufacturing #UD.
        let reason = if len == 15 {
            UnsupportedReason::Instruction
        } else if decoder.last_error() == DecoderError::NoMoreBytes {
            UnsupportedReason::TruncatedInstruction
        } else {
            UnsupportedReason::InvalidInstruction
        };
        return unsupported(eip, len, reason);
    }
    let prefixes = &bytes[..usize::from(len)];
    let string_operation = if matches!(
        decoded.mnemonic(),
        Mnemonic::Scasb | Mnemonic::Scasw | Mnemonic::Scasd
    ) && decoded.op1_kind() == OpKind::MemoryESEDI
    {
        Some(StringOpcode::Scas)
    } else if matches!(
        decoded.mnemonic(),
        Mnemonic::Cmpsb | Mnemonic::Cmpsw | Mnemonic::Cmpsd
    ) && decoded.op0_kind() == OpKind::MemorySegESI
        && decoded.op1_kind() == OpKind::MemoryESEDI
    {
        Some(StringOpcode::Cmps)
    } else if decoded.op0_kind() == OpKind::MemoryESEDI {
        match decoded.mnemonic() {
            Mnemonic::Movsb | Mnemonic::Movsw | Mnemonic::Movsd => Some(StringOpcode::Movs),
            Mnemonic::Stosb | Mnemonic::Stosw | Mnemonic::Stosd => Some(StringOpcode::Stos),
            _ => None,
        }
    } else {
        None
    };
    let xmm_movq = matches!(
        decoded.code(),
        Code::Movq_xmm_xmmm64 | Code::Movq_xmmm64_xmm
    );
    let mut operand16 = false;
    for &prefix in prefixes {
        match prefix {
            0x67 => return unsupported(eip, len, UnsupportedReason::Address16),
            0xf0 => return unsupported(eip, len, UnsupportedReason::LockPrefix),
            // Compilers emit F3 C3 as REP RET. iced decodes it as the ordinary
            // near return; there is no string repetition or ECX consumption.
            0xf3 if decoded.mnemonic() == Mnemonic::Ret => {}
            0xf3 if string_operation.is_some() => {}
            0xf2 if matches!(
                string_operation,
                Some(StringOpcode::Scas | StringOpcode::Cmps)
            ) => {}
            // These are mandatory opcode prefixes, not string repetition.
            0xf3 if decoded.mnemonic() == Mnemonic::Cvttss2si => {}
            0xf3 if decoded.code() == Code::Cvttps2dq_xmm_xmmm128 => {}
            0xf3 if decoded.mnemonic() == Mnemonic::Cvtdq2pd => {}
            0xf3 if decoded.mnemonic() == Mnemonic::Movss => {}
            0xf3 if decoded.mnemonic() == Mnemonic::Movq2dq => {}
            0xf3 if decoded.code() == Code::Movq_xmm_xmmm64 => {}
            0xf2 if decoded.mnemonic() == Mnemonic::Cvttsd2si => {}
            0xf2 if matches!(
                decoded.mnemonic(),
                Mnemonic::Cvtsd2si | Mnemonic::Addsd | Mnemonic::Subsd | Mnemonic::Mulsd
            ) => {}
            0xf2 if decoded.mnemonic() == Mnemonic::Movdq2q => {}
            0xf2 if decoded.mnemonic() == Mnemonic::Movsd && string_operation.is_none() => {}
            0xf2 | 0xf3 => return unsupported(eip, len, UnsupportedReason::RepeatPrefix),
            0x65 if matches!(
                string_operation,
                Some(StringOpcode::Stos | StringOpcode::Scas)
            ) => {}
            0x65 => return unsupported(eip, len, UnsupportedReason::Segment),
            // Do not infer a CPU-specific priority for competing mandatory
            // prefixes. This unsupported exit is not an invented #UD.
            0x66 if xmm_movq && decoded.code() != Code::Movq_xmmm64_xmm => {
                return unsupported(eip, len, UnsupportedReason::Instruction);
            }
            0x66 if decoded.code() == Code::Cvttps2dq_xmm_xmmm128 => {
                return unsupported(eip, len, UnsupportedReason::Instruction);
            }
            0x66 if decoded.mnemonic() == Mnemonic::Cvtdq2pd => {
                return unsupported(eip, len, UnsupportedReason::Instruction);
            }
            0x66 if matches!(
                decoded.mnemonic(),
                Mnemonic::Cvtsd2si | Mnemonic::Addsd | Mnemonic::Subsd | Mnemonic::Mulsd
            ) =>
            {
                return unsupported(eip, len, UnsupportedReason::Instruction);
            }
            0x66 => operand16 = true,
            0x26 | 0x2e | 0x36 | 0x3e | 0x64 => {}
            _ => break,
        }
    }

    if let Some(operation) = string_operation {
        let width = match decoded.mnemonic() {
            Mnemonic::Movsb | Mnemonic::Stosb | Mnemonic::Scasb | Mnemonic::Cmpsb => Width::W8,
            Mnemonic::Movsw | Mnemonic::Stosw | Mnemonic::Scasw | Mnemonic::Cmpsw => Width::W16,
            _ => Width::W32,
        };
        let memory = |base, segment| {
            Operand::Mem(MemoryOperand {
                base: Some(base),
                index: None,
                scale: 1,
                displacement: 0,
                segment,
                width,
            })
        };
        let source = if matches!(operation, StringOpcode::Movs | StringOpcode::Cmps) {
            let segment = match decoded.memory_segment() {
                X86Register::DS | X86Register::ES => Segment::Flat,
                X86Register::SS => Segment::Ss,
                X86Register::FS => Segment::Fs,
                _ => return unsupported(eip, len, UnsupportedReason::Segment),
            };
            memory(6, segment)
        } else {
            Operand::Reg(Register {
                index: 0,
                width,
                high8: false,
            })
        };
        return Instruction {
            eip,
            len,
            opcode: Opcode::String(
                operation,
                decoded.has_rep_prefix() || decoded.has_repne_prefix(),
            ),
            width,
            operands: vec![memory(7, Segment::Flat), source],
            condition: if matches!(operation, StringOpcode::Scas | StringOpcode::Cmps) {
                Some(if decoded.has_repne_prefix() {
                    Condition::Ne
                } else {
                    Condition::E
                })
            } else {
                None
            },
            flags_read: 0x400,
            flags_written: if matches!(operation, StringOpcode::Scas | StringOpcode::Cmps) {
                crate::state::ARITHMETIC_FLAGS
            } else {
                0
            },
            flags_undefined: 0,
            unsupported: None,
        };
    }

    if let Some(instruction) = x87::lift(&decoded, prefixes, eip, len) {
        return instruction;
    }
    let opcode = match decoded.mnemonic() {
        Mnemonic::Nop => Opcode::Nop,
        Mnemonic::Cld => Opcode::Cld,
        Mnemonic::Std => Opcode::Std,
        Mnemonic::Sahf => Opcode::Sahf,
        Mnemonic::Cpuid => Opcode::Cpuid,
        Mnemonic::Rdtsc => Opcode::Rdtsc,
        Mnemonic::Ud2 => Opcode::Ud2,
        Mnemonic::Mov => Opcode::Mov,
        Mnemonic::Xchg => Opcode::Xchg,
        Mnemonic::Movzx => Opcode::Movzx,
        Mnemonic::Movsx | Mnemonic::Cbw | Mnemonic::Cwde => Opcode::Movsx,
        Mnemonic::Lea => Opcode::Lea,
        Mnemonic::Add => Opcode::Add,
        Mnemonic::Adc => Opcode::Adc,
        Mnemonic::Sub => Opcode::Sub,
        Mnemonic::Sbb => Opcode::Sbb,
        Mnemonic::Cmp => Opcode::Cmp,
        Mnemonic::Test => Opcode::Test,
        Mnemonic::And => Opcode::And,
        Mnemonic::Or => Opcode::Or,
        Mnemonic::Xor => Opcode::Xor,
        Mnemonic::Inc => Opcode::Inc,
        Mnemonic::Dec => Opcode::Dec,
        Mnemonic::Neg => Opcode::Neg,
        Mnemonic::Not => Opcode::Not,
        Mnemonic::Shl | Mnemonic::Sal => Opcode::Shl,
        Mnemonic::Shr => Opcode::Shr,
        Mnemonic::Sar => Opcode::Sar,
        Mnemonic::Imul if decoded.op_count() == 1 => Opcode::ImulWide,
        Mnemonic::Imul => Opcode::Imul,
        Mnemonic::Mul => Opcode::Mul,
        Mnemonic::Shld => Opcode::Shld,
        Mnemonic::Shrd => Opcode::Shrd,
        Mnemonic::Div => Opcode::Div,
        Mnemonic::Idiv => Opcode::Idiv,
        Mnemonic::Cwd | Mnemonic::Cdq => Opcode::SignExtendPair,
        Mnemonic::Push => Opcode::Push,
        Mnemonic::Pop => Opcode::Pop,
        Mnemonic::Leave => Opcode::Leave,
        Mnemonic::Seto
        | Mnemonic::Setno
        | Mnemonic::Setb
        | Mnemonic::Setae
        | Mnemonic::Sete
        | Mnemonic::Setne
        | Mnemonic::Setbe
        | Mnemonic::Seta
        | Mnemonic::Sets
        | Mnemonic::Setns
        | Mnemonic::Setp
        | Mnemonic::Setnp
        | Mnemonic::Setl
        | Mnemonic::Setge
        | Mnemonic::Setle
        | Mnemonic::Setg => Opcode::Setcc,
        Mnemonic::Cmovo
        | Mnemonic::Cmovno
        | Mnemonic::Cmovb
        | Mnemonic::Cmovae
        | Mnemonic::Cmove
        | Mnemonic::Cmovne
        | Mnemonic::Cmovbe
        | Mnemonic::Cmova
        | Mnemonic::Cmovs
        | Mnemonic::Cmovns
        | Mnemonic::Cmovp
        | Mnemonic::Cmovnp
        | Mnemonic::Cmovl
        | Mnemonic::Cmovge
        | Mnemonic::Cmovle
        | Mnemonic::Cmovg => Opcode::Cmovcc,
        Mnemonic::Jo
        | Mnemonic::Jno
        | Mnemonic::Jb
        | Mnemonic::Jae
        | Mnemonic::Je
        | Mnemonic::Jne
        | Mnemonic::Jbe
        | Mnemonic::Ja
        | Mnemonic::Js
        | Mnemonic::Jns
        | Mnemonic::Jp
        | Mnemonic::Jnp
        | Mnemonic::Jl
        | Mnemonic::Jge
        | Mnemonic::Jle
        | Mnemonic::Jg => Opcode::Jcc,
        Mnemonic::Loop | Mnemonic::Loope | Mnemonic::Loopne => Opcode::Loop,
        Mnemonic::Jecxz => Opcode::Jecxz,
        Mnemonic::Jmp => Opcode::Jmp,
        Mnemonic::Call => Opcode::Call,
        Mnemonic::Ret => Opcode::Ret,
        Mnemonic::Movd
            if (X86Register::XMM0..=X86Register::XMM7).contains(&decoded.op0_register())
                || (X86Register::XMM0..=X86Register::XMM7).contains(&decoded.op1_register()) =>
        {
            Opcode::Sse(SseOpcode::Movd)
        }
        Mnemonic::Movd => Opcode::Mmx(MmxOpcode::Movd),
        // String MOVSD returned above. Only the explicit SSE operand form
        // reaches this match; its mandatory F2 prefix does not consume ECX.
        Mnemonic::Movsd => Opcode::Sse(SseOpcode::Movsd),
        Mnemonic::Movlpd | Mnemonic::Movlps => Opcode::Sse(SseOpcode::Movlpd),
        Mnemonic::Movss => Opcode::Sse(SseOpcode::Movss),
        Mnemonic::Movq if xmm_movq => Opcode::Sse(SseOpcode::Movq),
        Mnemonic::Movq => Opcode::Mmx(MmxOpcode::Movq),
        Mnemonic::Movq2dq => Opcode::Mmx(MmxOpcode::Movq2dq),
        Mnemonic::Movdq2q => Opcode::Mmx(MmxOpcode::Movdq2q),
        Mnemonic::Movntq => Opcode::Mmx(MmxOpcode::Movntq),
        Mnemonic::Punpcklbw
            if (X86Register::XMM0..=X86Register::XMM7).contains(&decoded.op0_register()) =>
        {
            Opcode::Sse(SseOpcode::Punpcklbw)
        }
        Mnemonic::Punpcklbw => Opcode::Mmx(MmxOpcode::Punpcklbw),
        Mnemonic::Punpckhbw => Opcode::Mmx(MmxOpcode::Punpckhbw),
        Mnemonic::Punpckhwd if decoded.code() == Code::Punpckhwd_xmm_xmmm128 => {
            Opcode::Sse(SseOpcode::Punpckhwd)
        }
        Mnemonic::Punpckhwd => Opcode::Mmx(MmxOpcode::Punpckhwd),
        Mnemonic::Punpckhdq => Opcode::Mmx(MmxOpcode::Punpckhdq),
        Mnemonic::Psubw if decoded.code() == Code::Psubw_xmm_xmmm128 => {
            Opcode::Sse(SseOpcode::Psubw)
        }
        Mnemonic::Psubw => Opcode::Mmx(MmxOpcode::Psubw),
        Mnemonic::Pmullw
            if (X86Register::XMM0..=X86Register::XMM7).contains(&decoded.op0_register()) =>
        {
            Opcode::Sse(SseOpcode::Pmullw)
        }
        Mnemonic::Pmullw => Opcode::Mmx(MmxOpcode::Pmullw),
        Mnemonic::Psraw
            if matches!(
                decoded.code(),
                Code::Psraw_xmm_imm8 | Code::Psraw_xmm_xmmm128
            ) =>
        {
            Opcode::Sse(SseOpcode::Psraw)
        }
        Mnemonic::Psraw => Opcode::Mmx(MmxOpcode::Psraw),
        Mnemonic::Psrlw
            if (X86Register::XMM0..=X86Register::XMM7).contains(&decoded.op0_register()) =>
        {
            Opcode::Sse(SseOpcode::Psrlw)
        }
        Mnemonic::Psrlw => Opcode::Mmx(MmxOpcode::Psrlw),
        Mnemonic::Psrad
            if matches!(
                decoded.code(),
                Code::Psrad_xmm_imm8 | Code::Psrad_xmm_xmmm128
            ) =>
        {
            Opcode::Sse(SseOpcode::Psrad)
        }
        Mnemonic::Pslld
            if matches!(
                decoded.code(),
                Code::Pslld_xmm_imm8 | Code::Pslld_xmm_xmmm128
            ) =>
        {
            Opcode::Sse(SseOpcode::Pslld)
        }
        Mnemonic::Pinsrw if decoded.code() == Code::Pinsrw_xmm_r32m16_imm8 => {
            Opcode::Sse(SseOpcode::Pinsrw)
        }
        Mnemonic::Maskmovdqu if decoded.code() == Code::Maskmovdqu_rDI_xmm_xmm => {
            Opcode::Sse(SseOpcode::Maskmovdqu)
        }
        Mnemonic::Paddw if decoded.code() == Code::Paddw_xmm_xmmm128 => {
            Opcode::Sse(SseOpcode::Paddw)
        }
        Mnemonic::Paddw => Opcode::Mmx(MmxOpcode::Paddw),
        Mnemonic::Packuswb
            if (X86Register::XMM0..=X86Register::XMM7).contains(&decoded.op0_register()) =>
        {
            Opcode::Sse(SseOpcode::Packuswb)
        }
        Mnemonic::Packuswb => Opcode::Mmx(MmxOpcode::Packuswb),
        Mnemonic::Packsswb if decoded.code() == Code::Packsswb_xmm_xmmm128 => {
            Opcode::Sse(SseOpcode::Packsswb)
        }
        Mnemonic::Packssdw if decoded.code() == Code::Packssdw_xmm_xmmm128 => {
            Opcode::Sse(SseOpcode::Packssdw)
        }
        Mnemonic::Emms => Opcode::Mmx(MmxOpcode::Emms),
        Mnemonic::Xorps | Mnemonic::Xorpd => Opcode::Sse(SseOpcode::Xorps),
        Mnemonic::Andps | Mnemonic::Andpd => Opcode::Sse(SseOpcode::And),
        Mnemonic::Orps | Mnemonic::Orpd => Opcode::Sse(SseOpcode::Por),
        Mnemonic::Psrlq
        | Mnemonic::Psllq
        | Mnemonic::Psubq
        | Mnemonic::Psubd
        | Mnemonic::Pcmpeqd
        | Mnemonic::Pmaxsw
            if (X86Register::XMM0..=X86Register::XMM7).contains(&decoded.op0_register()) =>
        {
            Opcode::Sse(match decoded.mnemonic() {
                Mnemonic::Psrlq => SseOpcode::Psrlq,
                Mnemonic::Psllq => SseOpcode::Psllq,
                Mnemonic::Psubq => SseOpcode::Psubq,
                Mnemonic::Psubd => SseOpcode::Psubd,
                Mnemonic::Pcmpeqd => SseOpcode::Pcmpeqd,
                Mnemonic::Pmaxsw => SseOpcode::Pmaxsw,
                _ => unreachable!(),
            })
        }
        Mnemonic::Pmovmskb | Mnemonic::Pextrw
            if (X86Register::XMM0..=X86Register::XMM7).contains(&decoded.op1_register()) =>
        {
            Opcode::Sse(if decoded.mnemonic() == Mnemonic::Pmovmskb {
                SseOpcode::Pmovmskb
            } else {
                SseOpcode::Pextrw
            })
        }
        Mnemonic::Unpcklpd => Opcode::Sse(SseOpcode::Unpcklpd),
        Mnemonic::Cvtdq2pd => Opcode::Sse(SseOpcode::Cvtdq2pd),
        Mnemonic::Pxor
            if (X86Register::MM0..=X86Register::MM7).contains(&decoded.op0_register()) =>
        {
            Opcode::Mmx(MmxOpcode::Pxor)
        }
        Mnemonic::Pxor => Opcode::Sse(SseOpcode::Pxor),
        Mnemonic::Por if decoded.code() == Code::Por_xmm_xmmm128 => Opcode::Sse(SseOpcode::Por),
        Mnemonic::Punpcklwd if decoded.code() == Code::Punpcklwd_mm_mmm32 => {
            Opcode::Mmx(MmxOpcode::Punpcklwd)
        }
        Mnemonic::Punpcklwd => Opcode::Sse(SseOpcode::Punpcklwd),
        Mnemonic::Movups => Opcode::Sse(SseOpcode::Movups),
        Mnemonic::Movdqa => Opcode::Sse(SseOpcode::Movdqa),
        Mnemonic::Movaps | Mnemonic::Movapd => Opcode::Sse(SseOpcode::Movaps),
        Mnemonic::Movntps => Opcode::Sse(SseOpcode::Movntps),
        Mnemonic::Ldmxcsr => Opcode::Sse(SseOpcode::Ldmxcsr),
        Mnemonic::Stmxcsr => Opcode::Sse(SseOpcode::Stmxcsr),
        Mnemonic::Cvttss2si => Opcode::Sse(SseOpcode::Cvttss2si),
        Mnemonic::Cvttsd2si => Opcode::Sse(SseOpcode::Cvttsd2si),
        Mnemonic::Cvtsd2si => Opcode::Sse(SseOpcode::Cvtsd2si),
        Mnemonic::Addsd => Opcode::Sse(SseOpcode::Addsd),
        Mnemonic::Subsd => Opcode::Sse(SseOpcode::Subsd),
        Mnemonic::Mulsd => Opcode::Sse(SseOpcode::Mulsd),
        Mnemonic::Addpd => Opcode::Sse(SseOpcode::Addpd),
        Mnemonic::Subpd => Opcode::Sse(SseOpcode::Subpd),
        Mnemonic::Mulpd => Opcode::Sse(SseOpcode::Mulpd),
        Mnemonic::Cvtdq2ps if decoded.code() == Code::Cvtdq2ps_xmm_xmmm128 => {
            Opcode::Sse(SseOpcode::Cvtdq2ps)
        }
        Mnemonic::Cvttps2dq if decoded.code() == Code::Cvttps2dq_xmm_xmmm128 => {
            Opcode::Sse(SseOpcode::Cvttps2dq)
        }
        Mnemonic::Mulps if decoded.code() == Code::Mulps_xmm_xmmm128 => {
            Opcode::Sse(SseOpcode::Mulps)
        }
        Mnemonic::Addps if decoded.code() == Code::Addps_xmm_xmmm128 => {
            Opcode::Sse(SseOpcode::Addps)
        }
        Mnemonic::Subps if decoded.code() == Code::Subps_xmm_xmmm128 => {
            Opcode::Sse(SseOpcode::Subps)
        }
        Mnemonic::Shufps if decoded.code() == Code::Shufps_xmm_xmmm128_imm8 => {
            Opcode::Sse(SseOpcode::Shufps)
        }
        Mnemonic::Pshufd if decoded.code() == Code::Pshufd_xmm_xmmm128_imm8 => {
            Opcode::Sse(SseOpcode::Pshufd)
        }
        Mnemonic::Prefetchnta => Opcode::Nop,
        _ => return unsupported(eip, len, UnsupportedReason::Instruction),
    };
    if operand16
        && matches!(
            opcode,
            Opcode::Jcc | Opcode::Loop | Opcode::Jecxz | Opcode::Jmp | Opcode::Call | Opcode::Ret
        )
    {
        return unsupported(eip, len, UnsupportedReason::Control16);
    }
    if opcode == Opcode::Shld && (operand16 || decoded.op0_kind() != OpKind::Register) {
        // Reached 32-bit register form. Memory RMW and 16-bit overshifts need
        // their own contracts; do not silently infer them from this lowering.
        return unsupported(eip, len, UnsupportedReason::Operand);
    }
    if opcode == Opcode::Shrd && operand16 {
        // The 16-bit form admits masked counts larger than its width, whose
        // result/flags are undefined. This lowering owns only the 32-bit form.
        return unsupported(eip, len, UnsupportedReason::Operand);
    }
    let default_width =
        if operand16 && !matches!(opcode, Opcode::Cpuid | Opcode::Rdtsc | Opcode::Sahf) {
            Width::W16
        } else {
            Width::W32
        };
    let mut operands = Vec::new();
    for index in 0..decoded.op_count() {
        // Intel SDM Vol.2B NOP: the ModRM operand is encoding space, not a
        // memory operation. In particular, a CS override is not a CS read.
        // PREFETCHh likewise does not perform an architectural data access.
        if matches!(decoded.mnemonic(), Mnemonic::Nop | Mnemonic::Prefetchnta) {
            break;
        }
        let Some(operand) = operand(&decoded, index, default_width, opcode == Opcode::Lea) else {
            return unsupported(eip, len, UnsupportedReason::Operand);
        };
        operands.push(operand);
    }
    if matches!(decoded.mnemonic(), Mnemonic::Cbw | Mnemonic::Cwde) {
        // Intel SDM Vol.2A CBW/CWDE: implicit AX <- sext(AL), or EAX <-
        // sext(AX), with no flags affected. Keep the original PC/length while
        // using the ordinary MOVSX IR, including its partial-register write.
        let (destination, source) = if decoded.mnemonic() == Mnemonic::Cbw {
            (Width::W16, Width::W8)
        } else {
            (Width::W32, Width::W16)
        };
        operands.extend([destination, source].map(|width| {
            Operand::Reg(Register {
                index: 0,
                width,
                high8: false,
            })
        }));
    }
    if opcode == Opcode::Xchg
        && !matches!(operands.as_slice(), [Operand::Reg(a), Operand::Reg(b)]
            if a.width == b.width && matches!(a.width, Width::W8 | Width::W16 | Width::W32))
    {
        // A memory XCHG is implicitly locked even without F0. Do not lower it
        // as an ordinary load/store pair or publish either operand partially.
        return unsupported(eip, len, UnsupportedReason::Operand);
    }
    if let Opcode::Mmx(op) = opcode {
        let valid = match op {
            MmxOpcode::Emms => operands.is_empty(),
            MmxOpcode::Movq2dq => matches!(operands.as_slice(), [Operand::Xmm(_), Operand::Mmx(_)]),
            MmxOpcode::Movdq2q => matches!(operands.as_slice(), [Operand::Mmx(_), Operand::Xmm(_)]),
            MmxOpcode::Movd => matches!(
                operands.as_slice(),
                [
                    Operand::Mmx(_),
                    Operand::Reg(Register {
                        width: Width::W32,
                        ..
                    }) | Operand::Mem(MemoryOperand {
                        width: Width::W32,
                        ..
                    })
                ] | [
                    Operand::Reg(Register {
                        width: Width::W32,
                        ..
                    }) | Operand::Mem(MemoryOperand {
                        width: Width::W32,
                        ..
                    }),
                    Operand::Mmx(_)
                ]
            ),
            MmxOpcode::Movq => matches!(
                operands.as_slice(),
                [
                    Operand::Mmx(_),
                    Operand::Mmx(_)
                        | Operand::Mem(MemoryOperand {
                            width: Width::W64,
                            ..
                        })
                ] | [
                    Operand::Mem(MemoryOperand {
                        width: Width::W64,
                        ..
                    }),
                    Operand::Mmx(_)
                ]
            ),
            MmxOpcode::Movntq => matches!(
                operands.as_slice(),
                [
                    Operand::Mem(MemoryOperand {
                        width: Width::W64,
                        ..
                    }),
                    Operand::Mmx(_)
                ]
            ),
            MmxOpcode::Psraw => matches!(operands.as_slice(), [Operand::Mmx(_), Operand::Imm(_)]),
            MmxOpcode::Punpcklwd => matches!(
                operands.as_slice(),
                [
                    Operand::Mmx(_),
                    Operand::Mmx(_)
                        | Operand::Mem(MemoryOperand {
                            width: Width::W32,
                            ..
                        })
                ]
            ),
            MmxOpcode::Psrlw => matches!(
                operands.as_slice(),
                [
                    Operand::Mmx(_),
                    Operand::Imm(_)
                        | Operand::Mmx(_)
                        | Operand::Mem(MemoryOperand {
                            width: Width::W64,
                            ..
                        })
                ]
            ),
            _ => matches!(
                operands.as_slice(),
                [
                    Operand::Mmx(_),
                    Operand::Mmx(_)
                        | Operand::Mem(MemoryOperand {
                            width: Width::W64,
                            ..
                        })
                ]
            ),
        };
        if !valid {
            return unsupported(eip, len, UnsupportedReason::Operand);
        }
    } else if let Opcode::Sse(op) = opcode {
        let masked_store = op == SseOpcode::Maskmovdqu
            && matches!(
                operands.as_slice(),
                [
                    Operand::Mem(MemoryOperand {
                        base: Some(7),
                        index: None,
                        displacement: 0,
                        width: Width::W128,
                        ..
                    }),
                    Operand::Xmm(_),
                    Operand::Xmm(_)
                ]
            );
        let shuffle = matches!(op, SseOpcode::Shufps | SseOpcode::Pshufd)
            && matches!(
                operands.as_slice(),
                [
                    Operand::Xmm(_),
                    Operand::Xmm(_)
                        | Operand::Mem(MemoryOperand {
                            width: Width::W128,
                            ..
                        }),
                    Operand::Imm(0..=255)
                ]
            );
        let unpack = matches!(
            op,
            SseOpcode::Punpcklbw
                | SseOpcode::Punpcklwd
                | SseOpcode::Punpckhwd
                | SseOpcode::Paddw
                | SseOpcode::Psubw
                | SseOpcode::Pmullw
                | SseOpcode::Packuswb
                | SseOpcode::Packsswb
                | SseOpcode::Packssdw
                | SseOpcode::Psubq
                | SseOpcode::Psubd
                | SseOpcode::Pcmpeqd
                | SseOpcode::Pmaxsw
                | SseOpcode::Unpcklpd
        ) && matches!(
            operands.as_slice(),
            [
                Operand::Xmm(_),
                Operand::Xmm(_)
                    | Operand::Mem(MemoryOperand {
                        width: Width::W128,
                        ..
                    })
            ]
        );
        let shift = matches!(
            op,
            SseOpcode::Psrlw
                | SseOpcode::Psrad
                | SseOpcode::Psraw
                | SseOpcode::Pslld
                | SseOpcode::Psrlq
                | SseOpcode::Psllq
        ) && matches!(
            operands.as_slice(),
            [
                Operand::Xmm(_),
                Operand::Imm(_)
                    | Operand::Xmm(_)
                    | Operand::Mem(MemoryOperand {
                        width: Width::W128,
                        ..
                    })
            ]
        );
        let insert = op == SseOpcode::Pinsrw
            && matches!(
                operands.as_slice(),
                [
                    Operand::Xmm(_),
                    Operand::Reg(Register {
                        width: Width::W32,
                        ..
                    }) | Operand::Mem(MemoryOperand {
                        width: Width::W16,
                        ..
                    }),
                    Operand::Imm(0..=255)
                ]
            );
        let float_move = op == SseOpcode::Movss
            && matches!(
                operands.as_slice(),
                [
                    Operand::Xmm(_),
                    Operand::Xmm(_)
                        | Operand::Mem(MemoryOperand {
                            width: Width::W32,
                            ..
                        })
                ] | [
                    Operand::Mem(MemoryOperand {
                        width: Width::W32,
                        ..
                    }),
                    Operand::Xmm(_)
                ]
            );
        let double_move = matches!(op, SseOpcode::Movsd | SseOpcode::Movq | SseOpcode::Movlpd)
            && (op != SseOpcode::Movlpd || operands.iter().any(|o| matches!(o, Operand::Mem(_))))
            && matches!(
                operands.as_slice(),
                [
                    Operand::Xmm(_),
                    Operand::Xmm(_)
                        | Operand::Mem(MemoryOperand {
                            width: Width::W64,
                            ..
                        })
                ] | [
                    Operand::Mem(MemoryOperand {
                        width: Width::W64,
                        ..
                    }),
                    Operand::Xmm(_)
                ]
            );
        let extract = match op {
            SseOpcode::Pextrw => matches!(
                operands.as_slice(),
                [
                    Operand::Reg(Register {
                        width: Width::W32,
                        ..
                    }),
                    Operand::Xmm(_),
                    Operand::Imm(0..=255)
                ]
            ),
            SseOpcode::Pmovmskb => matches!(
                operands.as_slice(),
                [
                    Operand::Reg(Register {
                        width: Width::W32,
                        ..
                    }),
                    Operand::Xmm(_)
                ]
            ),
            SseOpcode::Cvtdq2pd => matches!(
                operands.as_slice(),
                [
                    Operand::Xmm(_),
                    Operand::Xmm(_)
                        | Operand::Mem(MemoryOperand {
                            width: Width::W64,
                            ..
                        })
                ]
            ),
            _ => false,
        };
        let scalar_move = op == SseOpcode::Movd
            && matches!(
                operands.as_slice(),
                [
                    Operand::Xmm(_),
                    Operand::Reg(Register {
                        width: Width::W32,
                        ..
                    }) | Operand::Mem(MemoryOperand {
                        width: Width::W32,
                        ..
                    })
                ] | [
                    Operand::Reg(Register {
                        width: Width::W32,
                        ..
                    }) | Operand::Mem(MemoryOperand {
                        width: Width::W32,
                        ..
                    }),
                    Operand::Xmm(_)
                ]
            );
        let conversion = matches!(
            op,
            SseOpcode::Cvttss2si | SseOpcode::Cvttsd2si | SseOpcode::Cvtsd2si
        ) && matches!(
            operands.as_slice(),
            [
                Operand::Reg(Register {
                    width: Width::W32,
                    ..
                }),
                Operand::Xmm(_)
            ] | [
                Operand::Reg(Register {
                    width: Width::W32,
                    ..
                }),
                Operand::Mem(_)
            ]
        ) && match operands[1] {
            Operand::Mem(memory) => {
                memory.width
                    == if op == SseOpcode::Cvttss2si {
                        Width::W32
                    } else {
                        Width::W64
                    }
            }
            _ => true,
        };
        let double_numeric = matches!(op, SseOpcode::Addsd | SseOpcode::Subsd | SseOpcode::Mulsd)
            && matches!(
                operands.as_slice(),
                [
                    Operand::Xmm(_),
                    Operand::Xmm(_)
                        | Operand::Mem(MemoryOperand {
                            width: Width::W64,
                            ..
                        })
                ]
            );
        let control = matches!(op, SseOpcode::Ldmxcsr | SseOpcode::Stmxcsr)
            && matches!(
                operands.as_slice(),
                [Operand::Mem(MemoryOperand {
                    width: Width::W32,
                    ..
                })]
            );
        let input = matches!(
            op,
            SseOpcode::Xorps
                | SseOpcode::And
                | SseOpcode::Cvtdq2ps
                | SseOpcode::Cvttps2dq
                | SseOpcode::Mulps
                | SseOpcode::Addps
                | SseOpcode::Subps
                | SseOpcode::Addpd
                | SseOpcode::Subpd
                | SseOpcode::Mulpd
                | SseOpcode::Pxor
                | SseOpcode::Por
                | SseOpcode::Movups
                | SseOpcode::Movdqa
                | SseOpcode::Movaps
        ) && matches!(
            operands.as_slice(),
            [
                Operand::Xmm(_),
                Operand::Xmm(_)
                    | Operand::Mem(MemoryOperand {
                        width: Width::W128,
                        ..
                    })
            ]
        );
        let output = matches!(
            op,
            SseOpcode::Movups | SseOpcode::Movdqa | SseOpcode::Movaps | SseOpcode::Movntps
        ) && matches!(
            operands.as_slice(),
            [
                Operand::Mem(MemoryOperand {
                    width: Width::W128,
                    ..
                }),
                Operand::Xmm(_)
            ]
        );
        if !input
            && !output
            && !control
            && !conversion
            && !double_numeric
            && !scalar_move
            && !double_move
            && !extract
            && !float_move
            && !unpack
            && !shift
            && !shuffle
            && !insert
            && !masked_store
        {
            return unsupported(eip, len, UnsupportedReason::Operand);
        }
    } else if operands.iter().any(|operand| {
        matches!(
            operand,
            Operand::Mmx(_)
                | Operand::Xmm(_)
                | Operand::Mem(MemoryOperand {
                    width: Width::W64 | Width::W80 | Width::W128,
                    ..
                })
        )
    }) {
        return unsupported(eip, len, UnsupportedReason::Operand);
    }
    let width = operands
        .first()
        .map_or(default_width, |operand| match operand {
            Operand::Reg(register) => register.width,
            Operand::Mem(memory) => memory.width,
            Operand::Imm(_) => default_width,
            Operand::Mmx(_) => Width::W64,
            Operand::Xmm(_) => Width::W128,
        });
    Instruction {
        eip,
        len,
        opcode,
        width,
        operands,
        condition: match decoded.mnemonic() {
            Mnemonic::Loope => Some(Condition::E),
            Mnemonic::Loopne => Some(Condition::Ne),
            _ => condition(decoded.condition_code()),
        },
        flags_read: flags(decoded.rflags_read()),
        flags_written: flags(
            decoded.rflags_written() | decoded.rflags_set() | decoded.rflags_cleared(),
        ),
        flags_undefined: flags(decoded.rflags_undefined()),
        unsupported: None,
    }
}

fn unsupported(eip: u32, len: u8, reason: UnsupportedReason) -> Instruction {
    Instruction {
        eip,
        len,
        opcode: Opcode::Unsupported,
        width: Width::W32,
        operands: Vec::new(),
        condition: None,
        flags_read: 0,
        flags_written: 0,
        flags_undefined: 0,
        unsupported: Some(reason),
    }
}

fn operand(
    instruction: &X86Instruction,
    index: u32,
    default_width: Width,
    lea: bool,
) -> Option<Operand> {
    Some(match instruction.op_kind(index) {
        OpKind::Register => {
            let value = instruction.op_register(index);
            match value {
                X86Register::MM0
                | X86Register::MM1
                | X86Register::MM2
                | X86Register::MM3
                | X86Register::MM4
                | X86Register::MM5
                | X86Register::MM6
                | X86Register::MM7 => Operand::Mmx((value as u32 - X86Register::MM0 as u32) as u8),
                X86Register::XMM0
                | X86Register::XMM1
                | X86Register::XMM2
                | X86Register::XMM3
                | X86Register::XMM4
                | X86Register::XMM5
                | X86Register::XMM6
                | X86Register::XMM7 => {
                    Operand::Xmm((value as u32 - X86Register::XMM0 as u32) as u8)
                }
                _ => Operand::Reg(register(value)?),
            }
        }
        OpKind::Immediate8 => Operand::Imm(u32::from(instruction.immediate8())),
        OpKind::Immediate16 => Operand::Imm(u32::from(instruction.immediate16())),
        OpKind::Immediate32 => Operand::Imm(instruction.immediate32()),
        OpKind::Immediate8to16 => Operand::Imm(instruction.immediate8to16() as i32 as u32),
        OpKind::Immediate8to32 => Operand::Imm(instruction.immediate8to32() as u32),
        OpKind::NearBranch32 => Operand::Imm(instruction.near_branch32()),
        OpKind::MemorySegEDI if instruction.code() == Code::Maskmovdqu_rDI_xmm_xmm => {
            let segment = match instruction.memory_segment() {
                X86Register::FS => Segment::Fs,
                X86Register::SS => Segment::Ss,
                X86Register::DS | X86Register::ES => Segment::Flat,
                _ => return None,
            };
            Operand::Mem(MemoryOperand {
                base: Some(7),
                index: None,
                scale: 1,
                displacement: 0,
                segment,
                width: Width::W128,
            })
        }
        OpKind::Memory => {
            let segment = match instruction.memory_segment() {
                X86Register::FS => Segment::Fs,
                X86Register::SS => Segment::Ss,
                X86Register::DS | X86Register::ES => Segment::Flat,
                X86Register::CS if lea => Segment::Flat,
                _ => return None,
            };
            let width = if lea {
                default_width
            } else {
                match instruction.memory_size().size() {
                    1 => Width::W8,
                    2 => Width::W16,
                    4 => Width::W32,
                    8 => Width::W64,
                    10 => Width::W80,
                    16 => Width::W128,
                    _ => return None,
                }
            };
            Operand::Mem(MemoryOperand {
                base: address_register(instruction.memory_base())?,
                index: address_register(instruction.memory_index())?,
                scale: instruction.memory_index_scale() as u8,
                displacement: instruction.memory_displacement32(),
                segment,
                width,
            })
        }
        _ => return None,
    })
}

fn register(register: X86Register) -> Option<Register> {
    let (index, width, high8) = match register {
        X86Register::EAX => (0, Width::W32, false),
        X86Register::ECX => (1, Width::W32, false),
        X86Register::EDX => (2, Width::W32, false),
        X86Register::EBX => (3, Width::W32, false),
        X86Register::ESP => (4, Width::W32, false),
        X86Register::EBP => (5, Width::W32, false),
        X86Register::ESI => (6, Width::W32, false),
        X86Register::EDI => (7, Width::W32, false),
        X86Register::AX => (0, Width::W16, false),
        X86Register::CX => (1, Width::W16, false),
        X86Register::DX => (2, Width::W16, false),
        X86Register::BX => (3, Width::W16, false),
        X86Register::SP => (4, Width::W16, false),
        X86Register::BP => (5, Width::W16, false),
        X86Register::SI => (6, Width::W16, false),
        X86Register::DI => (7, Width::W16, false),
        X86Register::AL => (0, Width::W8, false),
        X86Register::CL => (1, Width::W8, false),
        X86Register::DL => (2, Width::W8, false),
        X86Register::BL => (3, Width::W8, false),
        X86Register::AH => (0, Width::W8, true),
        X86Register::CH => (1, Width::W8, true),
        X86Register::DH => (2, Width::W8, true),
        X86Register::BH => (3, Width::W8, true),
        _ => return None,
    };
    Some(Register {
        index,
        width,
        high8,
    })
}

fn address_register(value: X86Register) -> Option<Option<u8>> {
    if value == X86Register::None {
        return Some(None);
    }
    let register = register(value)?;
    (register.width == Width::W32).then_some(Some(register.index))
}

fn condition(value: ConditionCode) -> Option<Condition> {
    Some(match value {
        ConditionCode::o => Condition::O,
        ConditionCode::no => Condition::No,
        ConditionCode::b => Condition::B,
        ConditionCode::ae => Condition::Ae,
        ConditionCode::e => Condition::E,
        ConditionCode::ne => Condition::Ne,
        ConditionCode::be => Condition::Be,
        ConditionCode::a => Condition::A,
        ConditionCode::s => Condition::S,
        ConditionCode::ns => Condition::Ns,
        ConditionCode::p => Condition::P,
        ConditionCode::np => Condition::Np,
        ConditionCode::l => Condition::L,
        ConditionCode::ge => Condition::Ge,
        ConditionCode::le => Condition::Le,
        ConditionCode::g => Condition::G,
        _ => return None,
    })
}

fn flags(value: u32) -> u32 {
    [
        (RflagsBits::CF, 0x001),
        (RflagsBits::PF, 0x004),
        (RflagsBits::AF, 0x010),
        (RflagsBits::ZF, 0x040),
        (RflagsBits::SF, 0x080),
        (RflagsBits::DF, 0x400),
        (RflagsBits::OF, 0x800),
    ]
    .into_iter()
    .filter_map(|(decoder, architectural)| (value & decoder != 0).then_some(architectural))
    .fold(0, |result, flag| result | flag)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Code(Vec<u8>);

    impl CodeSource for Code {
        fn read_code(&self, address: u32, max_len: usize) -> Vec<u8> {
            let offset = address.wrapping_sub(0x1000) as usize;
            self.0
                .get(offset..)
                .unwrap_or_default()
                .iter()
                .take(max_len)
                .copied()
                .collect()
        }
    }

    #[test]
    fn count_branches_preserve_flags_and_discover_both_paths() {
        for (byte, opcode, condition, read) in [
            (0xe0, Opcode::Loop, Some(Condition::Ne), 0x40),
            (0xe1, Opcode::Loop, Some(Condition::E), 0x40),
            (0xe2, Opcode::Loop, None, 0),
            (0xe3, Opcode::Jecxz, None, 0),
        ] {
            let instruction = decode_one(0x1000, &[byte, 0xfe]);
            assert_eq!(instruction.opcode, opcode);
            assert_eq!(instruction.condition, condition);
            assert_eq!(instruction.flags_read, read);
            assert_eq!(instruction.flags_written, 0);
            assert_eq!(instruction.flags_undefined, 0);
            assert!(instruction.ends_block());
            assert_eq!(instruction.successors(), vec![0x1002, 0x1000]);
            assert_eq!(
                crate::optimize::written_gprs(&instruction),
                if byte == 0xe3 { 0 } else { 2 }
            );
            let region = discover(&Code(vec![byte, 0xfe, 0x0f, 0x0b]), 0x1000, 4);
            assert_eq!(region.blocks.len(), 2);
            assert_eq!(region.blocks[&0x1000].instructions.len(), 1);
        }
    }

    #[test]
    fn count_branch_displacements_wrap_at_32_bits() {
        for byte in [0xe0, 0xe1, 0xe2, 0xe3] {
            for (eip, displacement, target) in [
                (0xffff_fffe, 0x7f, 0x7f),
                (0, 0x80, 0xffff_ff82),
                (0x496f4a, 0xdc, 0x496f28),
            ] {
                let instruction = decode_one(eip, &[byte, displacement]);
                assert_eq!(instruction.operands, vec![Operand::Imm(target)]);
                assert_eq!(instruction.next_eip(), eip.wrapping_add(2));
            }
            assert_eq!(
                decode_one(0x1000, &[0x67, byte, 0]).unsupported,
                Some(UnsupportedReason::Address16)
            );
            // Operand size controls the branch-IP truncation independently of
            // the count register. Keep the established Control16 boundary.
            assert_eq!(
                decode_one(0x1000, &[0x66, byte, 0]).unsupported,
                Some(UnsupportedReason::Control16)
            );
            assert!(decode_one(0x1000, &[0xf0, byte, 0]).unsupported.is_some());
        }
    }

    #[test]
    fn retains_subregister_address_and_sign_extension() {
        let high = decode_one(0x1000, &[0xb4, 0x12]);
        assert_eq!(high.opcode, Opcode::Mov);
        assert_eq!(
            high.operands[0],
            Operand::Reg(Register {
                index: 0,
                width: Width::W8,
                high8: true
            })
        );
        let fs = decode_one(0x1000, &[0x64, 0x8b, 0x44, 0x8b, 0xfc]);
        assert_eq!(
            fs.operands[1],
            Operand::Mem(MemoryOperand {
                base: Some(3),
                index: Some(1),
                scale: 4,
                displacement: 0xffff_fffc,
                segment: Segment::Fs,
                width: Width::W32,
            })
        );
        let signed = decode_one(0x1000, &[0x83, 0xc0, 0xff]);
        assert_eq!(signed.operands[1], Operand::Imm(u32::MAX));
        assert_eq!(signed.flags_written, 0x8d5);
    }

    #[test]
    fn cpuid_has_fixed_32_bit_outputs_and_no_flags_or_memory_operands() {
        for bytes in [&[0x0f, 0xa2][..], &[0x66, 0x0f, 0xa2][..]] {
            let instruction = decode_one(0x1000, bytes);
            assert_eq!(instruction.opcode, Opcode::Cpuid);
            assert_eq!(instruction.width, Width::W32);
            assert_eq!(instruction.len as usize, bytes.len());
            assert!(instruction.operands.is_empty());
            assert_eq!(
                instruction.flags_read | instruction.flags_written | instruction.flags_undefined,
                0
            );
            assert_eq!(instruction.successors(), vec![0x1000 + bytes.len() as u32]);
        }
        assert_eq!(
            decode_one(0x1000, &[0xf0, 0x0f, 0xa2]).unsupported,
            Some(UnsupportedReason::InvalidInstruction)
        );
    }

    #[test]
    fn timestamp_decode_preserves_full_pair_and_precise_handoff_boundary() {
        let mut longest = vec![0x66; 13];
        longest.extend([0x0f, 0x31]);
        for bytes in [
            &[0x0f, 0x31][..],
            &[0x66, 0x0f, 0x31][..],
            longest.as_slice(),
        ] {
            let instruction = decode_one(0x1000, bytes);
            assert_eq!(instruction.opcode, Opcode::Rdtsc);
            assert_eq!(instruction.width, Width::W32);
            assert_eq!(instruction.len as usize, bytes.len());
            assert!(instruction.operands.is_empty() && instruction.ends_block());
            assert_eq!(
                instruction.flags_read | instruction.flags_written | instruction.flags_undefined,
                0
            );
            assert_eq!(instruction.successors(), vec![0x1000 + bytes.len() as u32]);
        }
        assert_eq!(
            decode_one(0x1000, &[0xf0, 0x0f, 0x31]).unsupported,
            Some(UnsupportedReason::InvalidInstruction)
        );
    }

    #[test]
    fn register_exchange_has_two_writes_and_no_flag_effects() {
        for (bytes, width) in [
            (&[0x94][..], Width::W32),
            (&[0x66, 0x94][..], Width::W16),
            (&[0x87, 0xcb][..], Width::W32),
            (&[0x86, 0xe0][..], Width::W8),
        ] {
            let instruction = decode_one(0x1000, bytes);
            assert_eq!(instruction.opcode, Opcode::Xchg);
            assert_eq!(instruction.width, width);
            assert_eq!(instruction.len as usize, bytes.len());
            assert!(matches!(
                instruction.operands.as_slice(),
                [Operand::Reg(_), Operand::Reg(_)]
            ));
            assert_eq!(
                instruction.flags_read | instruction.flags_written | instruction.flags_undefined,
                0
            );
        }
        let overlap = decode_one(0x1000, &[0x86, 0xe0]);
        assert!(
            matches!(overlap.operands.as_slice(), [Operand::Reg(a), Operand::Reg(b)]
            if a.index == b.index && a.high8 != b.high8)
        );
        assert_eq!(decode_one(0x1000, &[0x90]).opcode, Opcode::Nop);
    }

    #[test]
    fn memory_exchange_remains_an_explicit_locked_operation_boundary() {
        for bytes in [
            &[0x86, 0x03][..],
            &[0x66, 0x87, 0x03][..],
            &[0x87, 0x03][..],
            &[0xf0, 0x87, 0x03][..],
        ] {
            let instruction = decode_one(0x1234, bytes);
            assert_eq!(instruction.opcode, Opcode::Unsupported);
            assert_eq!(instruction.eip, 0x1234);
            assert_eq!(instruction.len as usize, bytes.len());
        }
        // LOCK with registers must never become a supported exchange.
        assert_eq!(
            decode_one(0x1000, &[0xf0, 0x87, 0xc3]).opcode,
            Opcode::Unsupported
        );
    }

    #[test]
    fn unsupported_instruction_is_exact_side_exit_after_known_prefix() {
        // FXTRACT remains unsupported; FSIN is now a reached implementation.
        let region = discover(&Code(vec![0x40, 0xd9, 0xf4, 0x40]), 0x1000, 20);
        let block = &region.blocks[&0x1000];
        assert_eq!(block.instructions.len(), 2);
        assert_eq!(block.instructions[1].eip, 0x1001);
        assert_eq!(
            block.instructions[1].unsupported,
            Some(UnsupportedReason::Instruction)
        );
        assert!(block.successors.is_empty());
    }

    #[test]
    fn ud2_is_a_defined_trap_and_terminates_discovery() {
        let region = discover(&Code(vec![0x40, 0x0f, 0x0b, 0x40]), 0x1000, 20);
        let block = &region.blocks[&0x1000];
        assert_eq!(block.instructions.len(), 2);
        let trap = &block.instructions[1];
        assert_eq!(trap.eip, 0x1001);
        assert_eq!(trap.len, 2);
        assert_eq!(trap.opcode, Opcode::Ud2);
        assert_eq!(trap.unsupported, None);
        assert!(block.successors.is_empty());
    }

    #[test]
    fn reached_cs_prefixed_nop_has_no_memory_operand_and_retains_length_limit() {
        for prefixes in [4, 6, 7] {
            let mut bytes = vec![0x66; prefixes];
            bytes.extend([0x2e, 0x0f, 0x1f, 0x84, 0, 0, 0, 0, 0]);
            let instruction = decode_one(0x1000, &bytes);
            if bytes.len() <= 15 {
                assert_eq!(instruction.opcode, Opcode::Nop);
                assert_eq!(instruction.len as usize, bytes.len());
                assert!(instruction.operands.is_empty());
                assert_eq!(instruction.flags_written, 0);
            } else {
                assert_eq!(instruction.opcode, Opcode::Unsupported);
                assert_eq!(
                    instruction.unsupported,
                    Some(UnsupportedReason::Instruction)
                );
            }
        }
        assert_eq!(
            decode_one(0x1000, &[0x2e, 0x8b, 0]).opcode,
            Opcode::Unsupported
        );
    }

    #[test]
    fn reached_cmov_preserves_condition_source_access_and_operand_width() {
        let reached = decode_one(0x1000, &[0x0f, 0x43, 0xf2]);
        assert_eq!(reached.opcode, Opcode::Cmovcc);
        assert_eq!(reached.condition, Some(Condition::Ae));
        assert_eq!(reached.flags_read, 1);
        assert_eq!(reached.flags_written, 0);
        let memory = decode_one(0x1000, &[0x66, 0x0f, 0x44, 0x03]);
        assert_eq!(memory.width, Width::W16);
        assert!(matches!(
            memory.operands[1],
            Operand::Mem(MemoryOperand {
                width: Width::W16,
                ..
            })
        ));
    }

    #[test]
    fn discovers_loop_and_both_conditional_edges() {
        // inc eax; cmp eax, 10; jl entry; ret
        let region = discover(
            &Code(vec![0x40, 0x83, 0xf8, 0x0a, 0x7c, 0xfa, 0xc3]),
            0x1000,
            20,
        );
        assert_eq!(region.blocks.len(), 2);
        assert_eq!(region.blocks[&0x1000].instructions.len(), 3);
        assert_eq!(region.blocks[&0x1000].successors, vec![0x1006, 0x1000]);
        assert_eq!(region.blocks[&0x1006].instructions[0].opcode, Opcode::Ret);
        assert!(!region.limit_reached);
    }

    #[test]
    fn discovers_call_target_and_return_continuation() {
        let region = discover(&Code(vec![0xe8, 1, 0, 0, 0, 0xc3, 0x40, 0xc3]), 0x1000, 20);
        assert_eq!(region.blocks[&0x1000].successors, vec![0x1005, 0x1006]);
        assert_eq!(region.blocks[&0x1006].instructions.len(), 2);
        assert_eq!(region.blocks[&0x1005].instructions[0].opcode, Opcode::Ret);
    }

    #[test]
    fn rejects_unmodelled_prefixes_and_leaves_limit_as_external_edge() {
        for bytes in [&[0x67, 0x8b, 0x00][..], &[0xf3, 0x90], &[0xf0, 0x01, 0x00]] {
            assert_eq!(decode_one(0x1000, bytes).opcode, Opcode::Unsupported);
        }
        let region = discover(&Code(vec![0x40; 8]), 0x1000, 2);
        assert!(region.limit_reached);
        assert_eq!(region.blocks[&0x1000].successors, vec![0x1002]);
        assert!(!region.blocks.contains_key(&0x1002));
    }

    #[test]
    fn accumulator_sign_extension_has_explicit_operands_and_original_boundaries() {
        for (bytes, destination, source) in [
            (&[0x98][..], Width::W32, Width::W16),
            (&[0x66, 0x98][..], Width::W16, Width::W8),
            (&[0x66, 0x66, 0x98][..], Width::W16, Width::W8),
            (&[0x64, 0x98][..], Width::W32, Width::W16),
        ] {
            let instruction = decode_one(0x12345000, bytes);
            assert_eq!(instruction.opcode, Opcode::Movsx);
            assert_eq!(instruction.width, destination);
            assert_eq!(instruction.len, bytes.len() as u8);
            assert_eq!(
                instruction.successors(),
                vec![0x12345000 + bytes.len() as u32]
            );
            assert_eq!(instruction.flags_read, 0);
            assert_eq!(instruction.flags_written, 0);
            assert_eq!(instruction.flags_undefined, 0);
            assert_eq!(crate::optimize::written_gprs(&instruction), 1);
            assert_eq!(
                instruction.operands,
                [destination, source]
                    .map(|width| Operand::Reg(Register {
                        index: 0,
                        width,
                        high8: false
                    }))
                    .to_vec()
            );
        }
        assert_eq!(decode_one(u32::MAX, &[0x98]).successors(), vec![0]);
        for bytes in [&[0xf0, 0x98][..], &[0xf0, 0x66, 0x98], &[0x66, 0xf0, 0x98]] {
            assert_eq!(
                decode_one(0x1000, bytes).unsupported,
                Some(UnsupportedReason::InvalidInstruction)
            );
        }
        assert_eq!(
            decode_one(0x1000, &[0x66]).unsupported,
            Some(UnsupportedReason::TruncatedInstruction)
        );
        assert_eq!(
            decode_one(0x1000, &[0x67, 0x98]).unsupported,
            Some(UnsupportedReason::Address16)
        );
    }

    #[test]
    fn short_fetch_does_not_turn_into_an_instruction_or_advance_pc() {
        let instruction = decode_one(0xffff_fffe, &[0xb8, 0x00]);
        assert_eq!(instruction.eip, 0xffff_fffe);
        assert_eq!(
            instruction.unsupported,
            Some(UnsupportedReason::TruncatedInstruction)
        );
        assert!(instruction.successors().is_empty());
        assert_eq!(
            decode_one(0x1000, &[]).unsupported,
            Some(UnsupportedReason::FetchUnavailable)
        );
    }

    #[test]
    fn compiler_rep_ret_is_a_single_near_return() {
        let instruction = decode_one(0x1000, &[0xf3, 0xc3]);
        assert_eq!(instruction.opcode, Opcode::Ret);
        assert_eq!(instruction.len, 2);
        assert!(instruction.operands.is_empty());
        assert!(instruction.successors().is_empty());
        assert_eq!(instruction.flags_read, 0);
    }

    #[test]
    fn multiple_reached_roots_share_one_bounded_graph() {
        let region = discover_many(
            &Code(vec![0x40, 0x40, 0x40, 0xc3, 0xc3]),
            &[0x1000, 0x1004],
            2,
        );
        assert!(region.limit_reached);
        assert_eq!(region.entry, 0x1000);
        assert_eq!(region.blocks[&0x1000].instructions.len(), 1);
        assert_eq!(region.blocks[&0x1004].instructions[0].opcode, Opcode::Ret);
    }

    #[test]
    fn stack_segment_identity_is_retained_for_memory_faults() {
        for bytes in [
            &[0x8b, 0x45, 0][..],
            &[0x8b, 0x04, 0x24],
            &[0x36, 0xa1, 0, 0x20, 0, 0],
        ] {
            let instruction = decode_one(0x1000, bytes);
            assert_eq!(instruction.opcode, Opcode::Mov);
            assert!(matches!(
                instruction.operands[1],
                Operand::Mem(MemoryOperand {
                    segment: Segment::Ss,
                    ..
                })
            ));
        }
    }

    #[test]
    fn cs_memory_access_is_not_treated_as_a_flat_writable_data_segment() {
        assert_eq!(
            decode_one(0x1000, &[0x2e, 0xa3, 0, 0x20, 0, 0]).unsupported,
            Some(UnsupportedReason::Operand)
        );
        assert_eq!(decode_one(0x1000, &[0x2e, 0x8d, 0x03]).opcode, Opcode::Lea);
        assert_eq!(decode_one(0x1000, &[0x2e, 0x74, 0]).opcode, Opcode::Jcc);
        assert_eq!(decode_one(0x1000, &[0x2e, 0x90]).opcode, Opcode::Nop);
    }

    #[test]
    fn full_decode_window_failure_does_not_claim_wrong_exception_kind() {
        assert_eq!(
            decode_one(0x1000, &[0x66; 15]).unsupported,
            Some(UnsupportedReason::Instruction)
        );
        assert_eq!(
            decode_one(0x1000, &[0x0f, 0x04, 0xcc]).unsupported,
            Some(UnsupportedReason::InvalidInstruction)
        );
    }

    #[test]
    fn reached_pixel_operations_retain_mmx_identity_and_memory_width() {
        for (bytes, expected) in [
            (&[0x0f, 0x6e, 0xcf][..], MmxOpcode::Movd),
            (&[0x0f, 0x60, 0xc8][..], MmxOpcode::Punpcklbw),
            (&[0x0f, 0x6f, 0xd1][..], MmxOpcode::Movq),
            (&[0x0f, 0xf9, 0xd9][..], MmxOpcode::Psubw),
            (&[0x0f, 0xd5, 0xd9][..], MmxOpcode::Pmullw),
            (&[0x0f, 0x71, 0xe3, 7][..], MmxOpcode::Psraw),
            (&[0x0f, 0xfd, 0xd3][..], MmxOpcode::Paddw),
            (&[0x0f, 0x67, 0xd0][..], MmxOpcode::Packuswb),
            (&[0x0f, 0x77][..], MmxOpcode::Emms),
        ] {
            let instruction = decode_one(0x1000, bytes);
            assert_eq!(instruction.opcode, Opcode::Mmx(expected));
            assert_eq!(instruction.flags_written, 0);
        }
        let memory = decode_one(0x1000, &[0x0f, 0x6f, 0x4c, 0xc5, 0]);
        assert!(matches!(
            memory.operands[1],
            Operand::Mem(MemoryOperand {
                width: Width::W64,
                segment: Segment::Ss,
                ..
            })
        ));
        assert_eq!(
            decode_one(0x1000, &[0x66, 0x0f, 0x6f, 0xc1]).opcode,
            Opcode::Sse(SseOpcode::Movdqa)
        );
        assert_eq!(
            decode_one(0x1000, &[0x0f, 0xe1, 0xc1]).opcode,
            Opcode::Unsupported
        );
    }

    #[test]
    fn double_left_shift_has_explicit_register_width_and_count_operands() {
        let immediate = decode_one(0x1000, &[0x0f, 0xa4, 0xf2, 10]);
        assert_eq!(immediate.opcode, Opcode::Shld);
        assert_eq!(immediate.width, Width::W32);
        assert_eq!(immediate.operands[2], Operand::Imm(10));
        let variable = decode_one(0x1000, &[0x0f, 0xa5, 0xc9]);
        assert_eq!(variable.opcode, Opcode::Shld);
        assert_eq!(
            variable.operands[2],
            Operand::Reg(Register {
                index: 1,
                width: Width::W8,
                high8: false
            })
        );
        for bytes in [
            &[0x66, 0x0f, 0xa4, 0xf2, 10][..],
            &[0x0f, 0xa4, 0x30, 10][..],
        ] {
            assert_eq!(decode_one(0x1000, bytes).opcode, Opcode::Unsupported);
        }
    }

    #[test]
    fn double_right_shift_keeps_32_bit_register_memory_and_count_identity() {
        for destination in 0..8 {
            for source in 0..8 {
                let modrm = 0xc0 | (source << 3) | destination;
                for count in 0..=255u8 {
                    let instruction = decode_one(0x1000, &[0x0f, 0xac, modrm, count]);
                    assert_eq!(instruction.opcode, Opcode::Shrd);
                    assert_eq!(instruction.width, Width::W32);
                    assert_eq!(instruction.operands[2], Operand::Imm(u32::from(count)));
                }
                let instruction = decode_one(0x1000, &[0x0f, 0xad, modrm]);
                assert_eq!(instruction.opcode, Opcode::Shrd);
                assert_eq!(
                    instruction.operands,
                    vec![
                        Operand::Reg(Register {
                            index: destination,
                            width: Width::W32,
                            high8: false
                        }),
                        Operand::Reg(Register {
                            index: source,
                            width: Width::W32,
                            high8: false
                        }),
                        Operand::Reg(Register {
                            index: 1,
                            width: Width::W8,
                            high8: false
                        }),
                    ]
                );
            }
        }
        for (bytes, segment, base, index) in [
            (&[0x0f, 0xac, 0x10, 24][..], Segment::Flat, Some(0), None),
            (&[0x64, 0x0f, 0xad, 0x10][..], Segment::Fs, Some(0), None),
            (&[0x0f, 0xad, 0x55, 0][..], Segment::Ss, Some(5), None),
            (
                &[0x36, 0x0f, 0xad, 0x14, 0x88][..],
                Segment::Ss,
                Some(0),
                Some(1),
            ),
        ] {
            let instruction = decode_one(0x1000, bytes);
            assert_eq!(instruction.opcode, Opcode::Shrd);
            assert!(matches!(instruction.operands[0], Operand::Mem(m)
                if m.segment == segment && m.base == base && m.index == index && m.width == Width::W32));
        }
        for bytes in [
            &[0x66, 0x0f, 0xac, 0xd0, 24][..],
            &[0x67, 0x0f, 0xad, 0x10][..],
        ] {
            assert_eq!(decode_one(0x1000, bytes).opcode, Opcode::Unsupported);
        }
        for bytes in [
            &[0xf0, 0x0f, 0xac, 0xd0, 0][..],
            &[0xf0, 0x0f, 0xad, 0x10][..],
        ] {
            assert_eq!(
                decode_one(0x1000, bytes).unsupported,
                Some(UnsupportedReason::InvalidInstruction)
            );
        }
    }

    #[test]
    fn signed_wide_multiply_keeps_source_identity_and_implicit_destinations() {
        for (bytes, width, source) in [
            (
                &[0xf6, 0xec][..],
                Width::W8,
                Operand::Reg(Register {
                    index: 0,
                    width: Width::W8,
                    high8: true,
                }),
            ),
            (
                &[0x66, 0xf7, 0xe9][..],
                Width::W16,
                Operand::Reg(Register {
                    index: 1,
                    width: Width::W16,
                    high8: false,
                }),
            ),
            (
                &[0xf7, 0xe9][..],
                Width::W32,
                Operand::Reg(Register {
                    index: 1,
                    width: Width::W32,
                    high8: false,
                }),
            ),
            (
                &[0xf7, 0xea][..],
                Width::W32,
                Operand::Reg(Register {
                    index: 2,
                    width: Width::W32,
                    high8: false,
                }),
            ),
        ] {
            let instruction = decode_one(0x1000, bytes);
            assert_eq!(instruction.opcode, Opcode::ImulWide);
            assert_eq!(instruction.width, width);
            assert_eq!(instruction.operands, vec![source]);
            assert_eq!(instruction.flags_written, 0x801);
            assert_eq!(instruction.flags_undefined, 0xd4);
            assert_eq!(
                crate::optimize::written_gprs(&instruction),
                if width == Width::W8 { 1 } else { 5 }
            );
        }
        for (bytes, width) in [
            (&[0xf6, 0x28][..], Width::W8),
            (&[0x66, 0xf7, 0x2a][..], Width::W16),
            (&[0x64, 0xf7, 0x2c, 0x10][..], Width::W32),
        ] {
            let instruction = decode_one(0x1000, bytes);
            assert_eq!(instruction.opcode, Opcode::ImulWide);
            assert!(
                matches!(instruction.operands.as_slice(), [Operand::Mem(m)] if m.width == width)
            );
        }
        for (bytes, operands) in [
            (&[0x0f, 0xaf, 0xc1][..], 2),
            (&[0x6b, 0xc1, 3][..], 3),
            (&[0x66, 0x0f, 0xaf, 0xc1][..], 2),
        ] {
            let instruction = decode_one(0x1000, bytes);
            assert_eq!(instruction.opcode, Opcode::Imul);
            assert_eq!(instruction.operands.len(), operands);
        }
        assert_eq!(
            decode_one(0x1000, &[0xf0, 0xf7, 0xe9]).unsupported,
            Some(UnsupportedReason::InvalidInstruction)
        );
    }

    #[test]
    fn unsigned_multiply_retains_source_width_and_defined_overflow_flags() {
        for (bytes, width) in [
            (&[0xf6, 0xe4][..], Width::W8),
            (&[0x66, 0xf7, 0xe1][..], Width::W16),
            (&[0xf7, 0xe2][..], Width::W32),
        ] {
            let instruction = decode_one(0x1000, bytes);
            assert_eq!(instruction.opcode, Opcode::Mul);
            assert_eq!(instruction.width, width);
            assert_eq!(instruction.operands.len(), 1);
            assert_eq!(instruction.flags_written, 0x801);
            assert_eq!(instruction.flags_undefined, 0xd4);
            assert_eq!(
                crate::optimize::written_gprs(&instruction),
                if width == Width::W8 { 1 } else { 5 }
            );
        }
    }

    #[test]
    fn sse_movd_keeps_scalar_width_direction_and_mmx_identity_separate() {
        for xmm in 0..8 {
            for gpr in 0..8 {
                let register = Operand::Reg(Register {
                    index: gpr,
                    width: Width::W32,
                    high8: false,
                });
                for (encoding, operands, written) in [
                    (0x6e, vec![Operand::Xmm(xmm), register], 0),
                    (0x7e, vec![register, Operand::Xmm(xmm)], 1 << gpr),
                ] {
                    let modrm = 0xc0 | xmm << 3 | gpr;
                    let instruction = decode_one(0x1000, &[0x66, 0x0f, encoding, modrm]);
                    assert_eq!(instruction.opcode, Opcode::Sse(SseOpcode::Movd));
                    assert_eq!(instruction.operands, operands);
                    assert_eq!(
                        instruction.flags_read
                            | instruction.flags_written
                            | instruction.flags_undefined,
                        0
                    );
                    assert_eq!(crate::optimize::written_gprs(&instruction), written);
                    let mmx = decode_one(0x1000, &[0x0f, encoding, modrm]);
                    assert_eq!(mmx.opcode, Opcode::Mmx(MmxOpcode::Movd));
                    assert!(mmx.operands.contains(&Operand::Mmx(xmm)));
                }
            }
        }
        for (encoding, memory_index) in [(0x6e, 1), (0x7e, 0)] {
            for (bytes, segment, base, displacement) in [
                (
                    vec![0x66, 0x0f, encoding, 0x4d, 0xff],
                    Segment::Ss,
                    5,
                    u32::MAX,
                ),
                (
                    vec![0x64, 0x66, 0x0f, encoding, 0x48, 0x01],
                    Segment::Fs,
                    0,
                    1,
                ),
            ] {
                let instruction = decode_one(0x1000, &bytes);
                assert_eq!(instruction.opcode, Opcode::Sse(SseOpcode::Movd));
                assert_eq!(
                    instruction.operands[memory_index],
                    Operand::Mem(MemoryOperand {
                        base: Some(base),
                        index: None,
                        scale: 1,
                        displacement,
                        segment,
                        width: Width::W32,
                    })
                );
                assert_eq!(instruction.operands[1 - memory_index], Operand::Xmm(1));
                assert_eq!(crate::optimize::written_gprs(&instruction), 0);
            }
            assert_eq!(
                decode_one(0x1000, &[0xf0, 0x66, 0x0f, encoding, 0x00]).unsupported,
                Some(UnsupportedReason::InvalidInstruction)
            );
            assert_eq!(
                decode_one(0x1000, &[0x67, 0x66, 0x0f, encoding, 0x00]).unsupported,
                Some(UnsupportedReason::Address16)
            );
        }
        assert_eq!(
            decode_one(0x1000, &[0xc5, 0xf9, 0x6e, 0xc0]).opcode,
            Opcode::Unsupported,
            "VMOVD must not silently enter the legacy SSE path"
        );
    }

    #[test]
    fn sse_movq_retains_exact_width_direction_and_mmx_identity() {
        for destination in 0..8 {
            for source in 0..8 {
                for (prefix, encoding, modrm) in [
                    (0xf3, 0x7e, 0xc0 | destination << 3 | source),
                    (0x66, 0xd6, 0xc0 | source << 3 | destination),
                ] {
                    let instruction = decode_one(0x1000, &[prefix, 0x0f, encoding, modrm]);
                    assert_eq!(instruction.opcode, Opcode::Sse(SseOpcode::Movq));
                    assert_eq!(
                        instruction.operands,
                        vec![Operand::Xmm(destination), Operand::Xmm(source)]
                    );
                    assert_eq!(
                        instruction.flags_read
                            | instruction.flags_written
                            | instruction.flags_undefined,
                        0
                    );
                    assert_eq!(crate::optimize::written_gprs(&instruction), 0);
                }
            }
        }
        for (prefix, encoding, memory_index) in [(0xf3, 0x7e, 1), (0x66, 0xd6, 0)] {
            for (segment_prefix, base, segment) in [(0x64, 0, Segment::Fs), (0x36, 5, Segment::Ss)]
            {
                let instruction = decode_one(
                    0x1000,
                    &[segment_prefix, prefix, 0x0f, encoding, 0x48 | base, 0x10],
                );
                assert_eq!(instruction.opcode, Opcode::Sse(SseOpcode::Movq));
                assert_eq!(
                    instruction.operands[memory_index],
                    Operand::Mem(MemoryOperand {
                        base: Some(base),
                        index: None,
                        scale: 1,
                        displacement: 0x10,
                        segment,
                        width: Width::W64,
                    })
                );
                assert_eq!(instruction.operands[1 - memory_index], Operand::Xmm(1));
            }
            assert_eq!(
                decode_one(0x1000, &[0xf0, prefix, 0x0f, encoding, 0]).unsupported,
                Some(UnsupportedReason::InvalidInstruction)
            );
            assert_eq!(
                decode_one(0x1000, &[0x67, prefix, 0x0f, encoding, 0]).unsupported,
                Some(UnsupportedReason::Address16)
            );
            assert_eq!(
                decode_one(0x1000, &[0x65, prefix, 0x0f, encoding, 0]).unsupported,
                Some(UnsupportedReason::Segment)
            );
        }
        let reached = decode_one(0x1000, &[0xf3, 0x0f, 0x7e, 0x4e, 0x10]);
        assert_eq!(reached.opcode, Opcode::Sse(SseOpcode::Movq));
        assert_eq!(reached.len, 5);
        assert_eq!(
            reached.operands,
            vec![
                Operand::Xmm(1),
                Operand::Mem(MemoryOperand {
                    base: Some(6),
                    index: None,
                    scale: 1,
                    displacement: 0x10,
                    segment: Segment::Flat,
                    width: Width::W64,
                })
            ]
        );
        for encoding in [0x6f, 0x7f] {
            assert_eq!(
                decode_one(0x1000, &[0x0f, encoding, 0xc1]).opcode,
                Opcode::Mmx(MmxOpcode::Movq)
            );
        }
        for bytes in [
            &[0x66, 0xf3, 0x0f, 0x7e, 0xc1][..],
            &[0xf3, 0x66, 0x0f, 0x7e, 0xc1][..],
            &[0xc5, 0xfa, 0x7e, 0xc1][..],
            &[0xc5, 0xf9, 0xd6, 0xc1][..],
        ] {
            let instruction = decode_one(0x1000, bytes);
            assert_eq!(instruction.opcode, Opcode::Unsupported);
            assert_ne!(
                instruction.unsupported,
                Some(UnsupportedReason::InvalidInstruction),
                "unimplemented encoding must not become a manufactured #UD"
            );
        }
    }

    #[test]
    fn scalar_sse_movsd_distinguishes_string_copy_and_exact_memory_width() {
        for destination in 0..8 {
            for source in 0..8 {
                for (encoding, modrm) in [
                    (0x10, 0xc0 | destination << 3 | source),
                    (0x11, 0xc0 | source << 3 | destination),
                ] {
                    let instruction = decode_one(0x1000, &[0xf2, 0x0f, encoding, modrm]);
                    assert_eq!(instruction.opcode, Opcode::Sse(SseOpcode::Movsd));
                    assert_eq!(
                        instruction.operands,
                        vec![Operand::Xmm(destination), Operand::Xmm(source)]
                    );
                    assert_eq!(
                        instruction.flags_read
                            | instruction.flags_written
                            | instruction.flags_undefined,
                        0
                    );
                    assert_eq!(crate::optimize::written_gprs(&instruction), 0);
                }
            }
        }
        let fixture = decode_one(0x1000, &[0xf2, 0x0f, 0x11, 0x8c, 0x24, 0xf8, 0, 0, 0]);
        assert_eq!(fixture.opcode, Opcode::Sse(SseOpcode::Movsd));
        assert_eq!(fixture.len, 9);
        assert_eq!(
            fixture.operands,
            vec![
                Operand::Mem(MemoryOperand {
                    base: Some(4),
                    index: None,
                    scale: 1,
                    displacement: 0xf8,
                    segment: Segment::Ss,
                    width: Width::W64,
                }),
                Operand::Xmm(1)
            ]
        );
        for (encoding, memory_index) in [(0x10, 1), (0x11, 0)] {
            let instruction = decode_one(0x1000, &[0x64, 0xf2, 0x0f, encoding, 0x58, 0x01]);
            assert_eq!(instruction.opcode, Opcode::Sse(SseOpcode::Movsd));
            assert_eq!(
                instruction.operands[memory_index],
                Operand::Mem(MemoryOperand {
                    base: Some(0),
                    index: None,
                    scale: 1,
                    displacement: 1,
                    segment: Segment::Fs,
                    width: Width::W64,
                })
            );
            assert_eq!(instruction.operands[1 - memory_index], Operand::Xmm(3));
            assert_eq!(
                decode_one(0x1000, &[0xf0, 0xf2, 0x0f, encoding, 0x00]).unsupported,
                Some(UnsupportedReason::InvalidInstruction)
            );
            assert_eq!(
                decode_one(0x1000, &[0x67, 0xf2, 0x0f, encoding, 0x00]).unsupported,
                Some(UnsupportedReason::Address16)
            );
        }
        for (bytes, repeat) in [(&[0xa5][..], false), (&[0xf3, 0xa5][..], true)] {
            let instruction = decode_one(0x1000, bytes);
            assert_eq!(
                instruction.opcode,
                Opcode::String(StringOpcode::Movs, repeat)
            );
            assert_eq!(instruction.width, Width::W32);
        }
        assert_eq!(
            decode_one(0x1000, &[0xf2, 0xa5]).unsupported,
            Some(UnsupportedReason::RepeatPrefix)
        );
        assert_eq!(
            decode_one(0x1000, &[0xc5, 0xfb, 0x10, 0x00]).opcode,
            Opcode::Unsupported,
            "VMOVSD must not silently use legacy upper-lane semantics"
        );
    }

    #[test]
    fn scalar_sse_movss_retains_exact_memory_width_and_register_direction() {
        for destination in 0..8 {
            for source in 0..8 {
                for (encoding, modrm) in [
                    (0x10, 0xc0 | destination << 3 | source),
                    (0x11, 0xc0 | source << 3 | destination),
                ] {
                    let instruction = decode_one(0x1000, &[0xf3, 0x0f, encoding, modrm]);
                    assert_eq!(instruction.opcode, Opcode::Sse(SseOpcode::Movss));
                    assert_eq!(
                        instruction.operands,
                        vec![Operand::Xmm(destination), Operand::Xmm(source)]
                    );
                    assert_eq!(
                        instruction.flags_read
                            | instruction.flags_written
                            | instruction.flags_undefined,
                        0
                    );
                    assert_eq!(crate::optimize::written_gprs(&instruction), 0);
                }
            }
        }
        let fixture = decode_one(0x1000, &[0xf3, 0x0f, 0x10, 0x05, 0x20, 0x42, 0x40, 0]);
        assert_eq!(fixture.opcode, Opcode::Sse(SseOpcode::Movss));
        assert_eq!(
            fixture.operands,
            vec![
                Operand::Xmm(0),
                Operand::Mem(MemoryOperand {
                    base: None,
                    index: None,
                    scale: 1,
                    displacement: 0x404220,
                    segment: Segment::Flat,
                    width: Width::W32,
                })
            ]
        );
        for (encoding, memory_index) in [(0x10, 1), (0x11, 0)] {
            let instruction = decode_one(0x1000, &[0x64, 0xf3, 0x0f, encoding, 0x58, 0x01]);
            assert_eq!(instruction.opcode, Opcode::Sse(SseOpcode::Movss));
            assert_eq!(
                instruction.operands[memory_index],
                Operand::Mem(MemoryOperand {
                    base: Some(0),
                    index: None,
                    scale: 1,
                    displacement: 1,
                    segment: Segment::Fs,
                    width: Width::W32,
                })
            );
            assert_eq!(instruction.operands[1 - memory_index], Operand::Xmm(3));
            assert_eq!(
                decode_one(0x1000, &[0xf0, 0xf3, 0x0f, encoding, 0x00]).unsupported,
                Some(UnsupportedReason::InvalidInstruction)
            );
        }
        assert_eq!(
            decode_one(0x1000, &[0xc5, 0xfa, 0x10, 0x00]).opcode,
            Opcode::Unsupported
        );
    }

    #[test]
    fn mmx_unpack_low_words_reads_m32_without_entering_the_xmm_bank() {
        for destination in 0..8 {
            for source in 0..8 {
                let instruction =
                    decode_one(0x1000, &[0x0f, 0x61, 0xc0 | destination << 3 | source]);
                assert_eq!(instruction.opcode, Opcode::Mmx(MmxOpcode::Punpcklwd));
                assert_eq!(
                    instruction.operands,
                    vec![Operand::Mmx(destination), Operand::Mmx(source)]
                );
                assert_eq!(
                    instruction.flags_read
                        | instruction.flags_written
                        | instruction.flags_undefined,
                    0
                );
                assert_eq!(crate::optimize::written_gprs(&instruction), 0);
            }
        }
        for (prefix, segment) in [
            (0x64, Segment::Fs),
            (0x36, Segment::Ss),
            (0x3e, Segment::Flat),
        ] {
            let instruction = decode_one(0x1000, &[prefix, 0x0f, 0x61, 0x4b, 1]);
            assert_eq!(instruction.opcode, Opcode::Mmx(MmxOpcode::Punpcklwd));
            assert_eq!(
                instruction.operands,
                vec![
                    Operand::Mmx(1),
                    Operand::Mem(MemoryOperand {
                        base: Some(3),
                        index: None,
                        scale: 1,
                        displacement: 1,
                        segment,
                        width: Width::W32
                    })
                ]
            );
        }
        for code in [&[0x67, 0x0f, 0x61, 0x03][..], &[0x65, 0x0f, 0x61, 0x03][..]] {
            let instruction = decode_one(0x1000, code);
            assert_eq!(instruction.opcode, Opcode::Unsupported);
            assert_ne!(
                instruction.unsupported,
                Some(UnsupportedReason::InvalidInstruction)
            );
        }
        assert_eq!(
            decode_one(0x1000, &[0xf0, 0x0f, 0x61, 0x03]).unsupported,
            Some(UnsupportedReason::InvalidInstruction)
        );
    }

    #[test]
    fn sse_unpack_low_words_owns_xmm_sources_and_full_aligned_memory_operand() {
        for destination in 0..8 {
            for source in 0..8 {
                let modrm = 0xc0 | destination << 3 | source;
                let instruction = decode_one(0x1000, &[0x66, 0x0f, 0x61, modrm]);
                assert_eq!(instruction.opcode, Opcode::Sse(SseOpcode::Punpcklwd));
                assert_eq!(
                    instruction.operands,
                    vec![Operand::Xmm(destination), Operand::Xmm(source)]
                );
                assert_eq!(
                    instruction.flags_read
                        | instruction.flags_written
                        | instruction.flags_undefined,
                    0
                );
                assert_eq!(crate::optimize::written_gprs(&instruction), 0);
                assert_eq!(
                    decode_one(0x1000, &[0x0f, 0x61, modrm]).opcode,
                    Opcode::Mmx(MmxOpcode::Punpcklwd),
                    "MMX unpack must not use SSE register state or semantics"
                );
            }
        }
        for (bytes, segment) in [
            (&[0x66, 0x0f, 0x61, 0x45, 0x10][..], Segment::Ss),
            (&[0x64, 0x66, 0x0f, 0x61, 0x40, 0x01][..], Segment::Fs),
        ] {
            let instruction = decode_one(0x1000, bytes);
            assert_eq!(instruction.opcode, Opcode::Sse(SseOpcode::Punpcklwd));
            assert!(
                matches!(instruction.operands[1], Operand::Mem(MemoryOperand {width: Width::W128, segment: s, ..}) if s == segment)
            );
        }
        assert_eq!(
            decode_one(0x1000, &[0xf0, 0x66, 0x0f, 0x61, 0x00]).unsupported,
            Some(UnsupportedReason::InvalidInstruction)
        );
        assert_eq!(
            decode_one(0x1000, &[0xc5, 0xf9, 0x61, 0xc0]).opcode,
            Opcode::Unsupported
        );
    }

    #[test]
    fn sse_packed_operations_use_full_xmm_width_without_accepting_other_encodings() {
        for (encoding, operation) in [
            (0xeb, SseOpcode::Por),
            (0x63, SseOpcode::Packsswb),
            (0x6b, SseOpcode::Packssdw),
            (0xe2, SseOpcode::Psrad),
            (0xe1, SseOpcode::Psraw),
            (0xf2, SseOpcode::Pslld),
            (0xfd, SseOpcode::Paddw),
            (0xf9, SseOpcode::Psubw),
            (0x69, SseOpcode::Punpckhwd),
        ] {
            for destination in 0..8 {
                for source in 0..8 {
                    let instruction = decode_one(
                        0x1000,
                        &[0x66, 0x0f, encoding, 0xc0 | destination << 3 | source],
                    );
                    assert_eq!(instruction.opcode, Opcode::Sse(operation));
                    assert_eq!(
                        instruction.operands,
                        vec![Operand::Xmm(destination), Operand::Xmm(source)]
                    );
                    assert_eq!(
                        instruction.flags_read
                            | instruction.flags_written
                            | instruction.flags_undefined,
                        0
                    );
                    assert_eq!(crate::optimize::written_gprs(&instruction), 0);
                }
            }
            for (prefix, base, segment) in [(0x64, 0, Segment::Fs), (0x36, 5, Segment::Ss)] {
                let instruction =
                    decode_one(0x1000, &[prefix, 0x66, 0x0f, encoding, 0x48 | base, 0x10]);
                assert_eq!(instruction.opcode, Opcode::Sse(operation));
                assert_eq!(
                    instruction.operands,
                    vec![
                        Operand::Xmm(1),
                        Operand::Mem(MemoryOperand {
                            base: Some(base),
                            index: None,
                            scale: 1,
                            displacement: 0x10,
                            segment,
                            width: Width::W128,
                        })
                    ]
                );
            }
            for bytes in [
                &[0x67, 0x66, 0x0f, encoding, 0x03][..],
                &[0x65, 0x66, 0x0f, encoding, 0x03][..],
                &[0xc5, 0xf9, encoding, 0xc1][..],
            ] {
                let instruction = decode_one(0x1000, bytes);
                assert_eq!(instruction.opcode, Opcode::Unsupported);
                assert_ne!(
                    instruction.unsupported,
                    Some(UnsupportedReason::InvalidInstruction)
                );
            }
            for bytes in [&[0x0f, encoding, 0xc1][..], &[0x0f, encoding, 0x03][..]] {
                let expected = match operation {
                    SseOpcode::Paddw => Opcode::Mmx(MmxOpcode::Paddw),
                    SseOpcode::Psubw => Opcode::Mmx(MmxOpcode::Psubw),
                    SseOpcode::Punpckhwd => Opcode::Mmx(MmxOpcode::Punpckhwd),
                    _ => Opcode::Unsupported,
                };
                let instruction = decode_one(0x1000, bytes);
                assert_eq!(instruction.opcode, expected);
                if expected == Opcode::Unsupported {
                    assert_ne!(
                        instruction.unsupported,
                        Some(UnsupportedReason::InvalidInstruction)
                    );
                }
            }
            assert_eq!(
                decode_one(0x1000, &[0xf0, 0x66, 0x0f, encoding, 0x03]).unsupported,
                Some(UnsupportedReason::InvalidInstruction)
            );
            for prefix in [0xf2, 0xf3] {
                assert_eq!(
                    decode_one(0x1000, &[prefix, 0x66, 0x0f, encoding, 0xc1]).opcode,
                    Opcode::Unsupported
                );
            }
        }
    }

    #[test]
    fn pshufd_keeps_selector_source_width_and_rejects_distinct_shuffle_encodings() {
        for destination in 0..8 {
            for source in 0..8 {
                for selectors in 0..=255 {
                    let instruction = decode_one(
                        0x1000,
                        &[
                            0x66,
                            0x0f,
                            0x70,
                            0xc0 | destination << 3 | source,
                            selectors,
                        ],
                    );
                    assert_eq!(instruction.opcode, Opcode::Sse(SseOpcode::Pshufd));
                    assert_eq!(
                        instruction.operands,
                        vec![
                            Operand::Xmm(destination),
                            Operand::Xmm(source),
                            Operand::Imm(u32::from(selectors))
                        ]
                    );
                    assert_eq!(
                        instruction.flags_read
                            | instruction.flags_written
                            | instruction.flags_undefined,
                        0
                    );
                    assert_eq!(crate::optimize::written_gprs(&instruction), 0);
                }
            }
        }
        for (prefix, segment) in [(0x64, Segment::Fs), (0x36, Segment::Ss)] {
            let instruction = decode_one(0x1000, &[prefix, 0x66, 0x0f, 0x70, 0x53, 0x10, 0]);
            assert_eq!(instruction.opcode, Opcode::Sse(SseOpcode::Pshufd));
            assert_eq!(
                instruction.operands,
                vec![
                    Operand::Xmm(2),
                    Operand::Mem(MemoryOperand {
                        base: Some(3),
                        index: None,
                        scale: 1,
                        displacement: 16,
                        segment,
                        width: Width::W128
                    }),
                    Operand::Imm(0)
                ]
            );
        }
        for bytes in [
            &[0x0f, 0x70, 0xc1, 0][..],
            &[0xf2, 0x0f, 0x70, 0xc1, 0][..],
            &[0xf3, 0x0f, 0x70, 0xc1, 0][..],
            &[0x67, 0x66, 0x0f, 0x70, 0x03, 0][..],
            &[0x65, 0x66, 0x0f, 0x70, 0x03, 0][..],
            &[0xc5, 0xf9, 0x70, 0xc1, 0][..],
        ] {
            let instruction = decode_one(0x1000, bytes);
            assert_eq!(instruction.opcode, Opcode::Unsupported);
            assert_ne!(
                instruction.unsupported,
                Some(UnsupportedReason::InvalidInstruction)
            );
        }
        assert_eq!(
            decode_one(0x1000, &[0xf0, 0x66, 0x0f, 0x70, 0x03, 0]).unsupported,
            Some(UnsupportedReason::InvalidInstruction)
        );
    }

    #[test]
    fn packed_shuffle_retains_all_selector_bits_and_full_source_width() {
        for destination in 0..8 {
            for source in 0..8 {
                for selectors in 0..=255 {
                    let instruction = decode_one(
                        0x1000,
                        &[0x0f, 0xc6, 0xc0 | destination << 3 | source, selectors],
                    );
                    assert_eq!(instruction.opcode, Opcode::Sse(SseOpcode::Shufps));
                    assert_eq!(
                        instruction.operands,
                        vec![
                            Operand::Xmm(destination),
                            Operand::Xmm(source),
                            Operand::Imm(u32::from(selectors))
                        ]
                    );
                    assert_eq!(
                        instruction.flags_read
                            | instruction.flags_written
                            | instruction.flags_undefined,
                        0
                    );
                    assert_eq!(crate::optimize::written_gprs(&instruction), 0);
                }
            }
        }
        for (prefix, base, segment) in [(0x64, 0, Segment::Fs), (0x36, 5, Segment::Ss)] {
            let instruction = decode_one(0x1000, &[prefix, 0x0f, 0xc6, 0x48 | base, 0x20, 0x44]);
            assert_eq!(instruction.opcode, Opcode::Sse(SseOpcode::Shufps));
            assert_eq!(
                instruction.operands,
                vec![
                    Operand::Xmm(1),
                    Operand::Mem(MemoryOperand {
                        base: Some(base),
                        index: None,
                        scale: 1,
                        displacement: 32,
                        segment,
                        width: Width::W128,
                    }),
                    Operand::Imm(0x44)
                ]
            );
        }
        for bytes in [
            &[0x66, 0x0f, 0xc6, 0xc1, 0x44][..], // SHUFPD.
            &[0x67, 0x0f, 0xc6, 0x03, 0x44][..],
            &[0x65, 0x0f, 0xc6, 0x03, 0x44][..],
            &[0xc5, 0xf8, 0xc6, 0xc1, 0x44][..],
        ] {
            let instruction = decode_one(0x1000, bytes);
            assert_eq!(instruction.opcode, Opcode::Unsupported);
            assert_ne!(
                instruction.unsupported,
                Some(UnsupportedReason::InvalidInstruction)
            );
        }
        assert_eq!(
            decode_one(0x1000, &[0xf0, 0x0f, 0xc6, 0x03, 0x44]).unsupported,
            Some(UnsupportedReason::InvalidInstruction)
        );
        for prefix in [0xf2, 0xf3] {
            assert_eq!(
                decode_one(0x1000, &[prefix, 0x0f, 0xc6, 0xc1, 0x44]).opcode,
                Opcode::Unsupported
            );
        }
    }

    #[test]
    fn packed_single_truncation_retains_full_width_and_mandatory_prefix() {
        for destination in 0..8 {
            for source in 0..8 {
                let instruction = decode_one(
                    0x1000,
                    &[0xf3, 0x0f, 0x5b, 0xc0 | destination << 3 | source],
                );
                assert_eq!(instruction.opcode, Opcode::Sse(SseOpcode::Cvttps2dq));
                assert_eq!(
                    instruction.operands,
                    vec![Operand::Xmm(destination), Operand::Xmm(source)]
                );
                assert_eq!(
                    instruction.flags_read
                        | instruction.flags_written
                        | instruction.flags_undefined,
                    0
                );
                assert_eq!(crate::optimize::written_gprs(&instruction), 0);
            }
        }
        for (prefix, base, segment) in [(0x64, 0, Segment::Fs), (0x36, 5, Segment::Ss)] {
            let instruction = decode_one(0x1000, &[prefix, 0xf3, 0x0f, 0x5b, 0x48 | base, 0x10]);
            assert_eq!(instruction.opcode, Opcode::Sse(SseOpcode::Cvttps2dq));
            assert_eq!(
                instruction.operands,
                vec![
                    Operand::Xmm(1),
                    Operand::Mem(MemoryOperand {
                        base: Some(base),
                        index: None,
                        scale: 1,
                        displacement: 16,
                        segment,
                        width: Width::W128,
                    })
                ]
            );
        }
        for bytes in [
            &[0x66, 0x0f, 0x5b, 0xc1][..], // CVTPS2DQ uses MXCSR rounding.
            &[0x66, 0xf3, 0x0f, 0x5b, 0xc1][..],
            &[0xf3, 0x66, 0x0f, 0x5b, 0xc1][..],
            &[0x67, 0xf3, 0x0f, 0x5b, 0x03][..],
            &[0x65, 0xf3, 0x0f, 0x5b, 0x03][..],
            &[0xc5, 0xfa, 0x5b, 0xc1][..],
        ] {
            let instruction = decode_one(0x1000, bytes);
            assert_eq!(instruction.opcode, Opcode::Unsupported);
            assert_ne!(
                instruction.unsupported,
                Some(UnsupportedReason::InvalidInstruction)
            );
        }
        assert_eq!(
            decode_one(0x1000, &[0xf0, 0xf3, 0x0f, 0x5b, 0x03]).unsupported,
            Some(UnsupportedReason::InvalidInstruction)
        );
        assert_eq!(
            decode_one(0x1000, &[0x0f, 0x5b, 0xc1]).opcode,
            Opcode::Sse(SseOpcode::Cvtdq2ps)
        );
    }

    #[test]
    fn packed_single_add_sub_keep_order_full_width_and_distinct_encodings() {
        for (byte, opcode) in [(0x58, SseOpcode::Addps), (0x5c, SseOpcode::Subps)] {
            for destination in 0..8 {
                for source in 0..8 {
                    let instruction =
                        decode_one(0x1000, &[0x0f, byte, 0xc0 | destination << 3 | source]);
                    assert_eq!(instruction.opcode, Opcode::Sse(opcode));
                    assert_eq!(
                        instruction.operands,
                        vec![Operand::Xmm(destination), Operand::Xmm(source)]
                    );
                    assert_eq!(
                        instruction.flags_read
                            | instruction.flags_written
                            | instruction.flags_undefined,
                        0
                    );
                    assert_eq!(crate::optimize::written_gprs(&instruction), 0);
                }
            }
            for (prefix, base, segment) in [(0x64, 0, Segment::Fs), (0x36, 5, Segment::Ss)] {
                let instruction = decode_one(0x1000, &[prefix, 0x0f, byte, 0x48 | base, 0x10]);
                assert_eq!(instruction.opcode, Opcode::Sse(opcode));
                assert_eq!(
                    instruction.operands,
                    vec![
                        Operand::Xmm(1),
                        Operand::Mem(MemoryOperand {
                            base: Some(base),
                            index: None,
                            scale: 1,
                            displacement: 16,
                            segment,
                            width: Width::W128,
                        })
                    ]
                );
            }
            for bytes in [
                vec![0xf3, 0x0f, byte, 0xc1], // scalar single
                vec![0x67, 0x0f, byte, 0x03],
                vec![0x65, 0x0f, byte, 0x03],
                vec![0xc5, 0xf8, byte, 0xc1],
            ] {
                let instruction = decode_one(0x1000, &bytes);
                assert_eq!(instruction.opcode, Opcode::Unsupported);
                assert_ne!(
                    instruction.unsupported,
                    Some(UnsupportedReason::InvalidInstruction)
                );
            }
            assert_eq!(
                decode_one(0x1000, &[0xf0, 0x0f, byte, 0x03]).unsupported,
                Some(UnsupportedReason::InvalidInstruction)
            );
        }
    }

    #[test]
    fn packed_single_multiply_keeps_order_full_width_and_other_encodings_distinct() {
        for destination in 0..8 {
            for source in 0..8 {
                let instruction =
                    decode_one(0x1000, &[0x0f, 0x59, 0xc0 | destination << 3 | source]);
                assert_eq!(instruction.opcode, Opcode::Sse(SseOpcode::Mulps));
                assert_eq!(
                    instruction.operands,
                    vec![Operand::Xmm(destination), Operand::Xmm(source)]
                );
                assert_eq!(
                    instruction.flags_read
                        | instruction.flags_written
                        | instruction.flags_undefined,
                    0
                );
                assert_eq!(crate::optimize::written_gprs(&instruction), 0);
            }
        }
        for (prefix, base, segment) in [(0x64, 0, Segment::Fs), (0x36, 5, Segment::Ss)] {
            let instruction = decode_one(0x1000, &[prefix, 0x0f, 0x59, 0x48 | base, 0x10]);
            assert_eq!(instruction.opcode, Opcode::Sse(SseOpcode::Mulps));
            assert_eq!(
                instruction.operands,
                vec![
                    Operand::Xmm(1),
                    Operand::Mem(MemoryOperand {
                        base: Some(base),
                        index: None,
                        scale: 1,
                        displacement: 16,
                        segment,
                        width: Width::W128,
                    })
                ]
            );
        }
        for bytes in [
            &[0xf3, 0x0f, 0x59, 0xc1][..], // MULSS.
            &[0x67, 0x0f, 0x59, 0x03][..],
            &[0x65, 0x0f, 0x59, 0x03][..],
            &[0xc5, 0xf8, 0x59, 0xc1][..],
        ] {
            let instruction = decode_one(0x1000, bytes);
            assert_eq!(instruction.opcode, Opcode::Unsupported);
            assert_ne!(
                instruction.unsupported,
                Some(UnsupportedReason::InvalidInstruction)
            );
        }
        assert_eq!(
            decode_one(0x1000, &[0xf0, 0x0f, 0x59, 0x03]).unsupported,
            Some(UnsupportedReason::InvalidInstruction)
        );
    }

    #[test]
    fn double_numeric_forms_own_width_prefix_and_fault_boundaries() {
        for (byte, scalar, packed) in [
            (0x58, SseOpcode::Addsd, SseOpcode::Addpd),
            (0x5c, SseOpcode::Subsd, SseOpcode::Subpd),
            (0x59, SseOpcode::Mulsd, SseOpcode::Mulpd),
        ] {
            for (prefix, op, width) in [(0xf2, scalar, Width::W64), (0x66, packed, Width::W128)] {
                let decoded = decode_one(0x1000, &[0x64, prefix, 0x0f, byte, 0x13]);
                assert_eq!(decoded.opcode, Opcode::Sse(op));
                assert!(
                    matches!(decoded.operands[1], Operand::Mem(m) if m.width == width && m.segment == Segment::Fs)
                );
                assert_eq!(crate::optimize::written_gprs(&decoded), 0);
                for destination in 0..8 {
                    for source in 0..8 {
                        let decoded = decode_one(
                            0x1000,
                            &[prefix, 0x0f, byte, 0xc0 | destination << 3 | source],
                        );
                        assert_eq!(
                            decoded.operands,
                            vec![Operand::Xmm(destination), Operand::Xmm(source)]
                        );
                        assert_eq!(decoded.opcode, Opcode::Sse(op));
                    }
                }
            }
            for bytes in [
                [0x66, 0xf2, 0x0f, byte, 0xc1],
                [0xf2, 0x66, 0x0f, byte, 0xc1],
            ] {
                assert_eq!(decode_one(0x1000, &bytes).opcode, Opcode::Unsupported);
            }
        }
        for destination in 0..8 {
            let decoded = decode_one(0x1000, &[0xf2, 0x0f, 0x2d, 0xc0 | destination << 3]);
            assert_eq!(decoded.opcode, Opcode::Sse(SseOpcode::Cvtsd2si));
            assert_eq!(crate::optimize::written_gprs(&decoded), 1 << destination);
        }
    }

    #[test]
    fn packed_integer_to_single_keeps_full_source_and_distinct_conversion_encodings() {
        for destination in 0..8 {
            for source in 0..8 {
                let instruction =
                    decode_one(0x1000, &[0x0f, 0x5b, 0xc0 | destination << 3 | source]);
                assert_eq!(instruction.opcode, Opcode::Sse(SseOpcode::Cvtdq2ps));
                assert_eq!(
                    instruction.operands,
                    vec![Operand::Xmm(destination), Operand::Xmm(source)]
                );
                assert_eq!(
                    instruction.flags_read
                        | instruction.flags_written
                        | instruction.flags_undefined,
                    0
                );
                assert_eq!(crate::optimize::written_gprs(&instruction), 0);
            }
        }
        for (prefix, base, segment) in [(0x64, 0, Segment::Fs), (0x36, 5, Segment::Ss)] {
            let instruction = decode_one(0x1000, &[prefix, 0x0f, 0x5b, 0x48 | base, 0x10]);
            assert_eq!(instruction.opcode, Opcode::Sse(SseOpcode::Cvtdq2ps));
            assert_eq!(
                instruction.operands,
                vec![
                    Operand::Xmm(1),
                    Operand::Mem(MemoryOperand {
                        base: Some(base),
                        index: None,
                        scale: 1,
                        displacement: 16,
                        segment,
                        width: Width::W128,
                    })
                ]
            );
        }
        for bytes in [
            &[0x66, 0x0f, 0x5b, 0xc1][..], // CVTPS2DQ, not CVTDQ2PS.
            &[0x67, 0x0f, 0x5b, 0x03][..],
            &[0x65, 0x0f, 0x5b, 0x03][..],
            &[0xc5, 0xf8, 0x5b, 0xc1][..],
        ] {
            let instruction = decode_one(0x1000, bytes);
            assert_eq!(instruction.opcode, Opcode::Unsupported);
            assert_ne!(
                instruction.unsupported,
                Some(UnsupportedReason::InvalidInstruction)
            );
        }
        assert_eq!(
            decode_one(0x1000, &[0xf0, 0x0f, 0x5b, 0x03]).unsupported,
            Some(UnsupportedReason::InvalidInstruction)
        );
        assert_eq!(
            decode_one(0x1000, &[0xf3, 0x0f, 0x5b, 0xc1]).opcode,
            Opcode::Sse(SseOpcode::Cvttps2dq)
        );
    }

    #[test]
    fn sse_psraw_immediate_retains_full_count_without_changing_mmx_coverage() {
        for destination in 0..8 {
            for count in 0..=255 {
                let instruction =
                    decode_one(0x1000, &[0x66, 0x0f, 0x71, 0xe0 | destination, count]);
                assert_eq!(instruction.opcode, Opcode::Sse(SseOpcode::Psraw));
                assert_eq!(
                    instruction.operands,
                    vec![Operand::Xmm(destination), Operand::Imm(u32::from(count))]
                );
                assert_eq!(
                    instruction.flags_read
                        | instruction.flags_written
                        | instruction.flags_undefined,
                    0
                );
                assert_eq!(crate::optimize::written_gprs(&instruction), 0);
            }
        }
        assert_eq!(
            decode_one(0x1000, &[0x0f, 0x71, 0xe0, 3]).opcode,
            Opcode::Mmx(MmxOpcode::Psraw)
        );
        for bytes in [&[0x0f, 0xe1, 0xc1][..], &[0xc5, 0xf9, 0x71, 0xe0, 3][..]] {
            let instruction = decode_one(0x1000, bytes);
            assert_eq!(instruction.opcode, Opcode::Unsupported);
            assert_ne!(
                instruction.unsupported,
                Some(UnsupportedReason::InvalidInstruction)
            );
        }
        for bytes in [
            &[0xf0, 0x66, 0x0f, 0x71, 0xe0, 3][..],
            &[0x66, 0x0f, 0x71, 0x20, 3][..],
        ] {
            assert_eq!(
                decode_one(0x1000, bytes).unsupported,
                Some(UnsupportedReason::InvalidInstruction)
            );
        }
        for prefix in [0xf2, 0xf3] {
            assert_eq!(
                decode_one(0x1000, &[prefix, 0x66, 0x0f, 0x71, 0xe0, 3]).unsupported,
                Some(UnsupportedReason::InvalidInstruction)
            );
        }
    }

    #[test]
    fn sse_pslld_immediate_preserves_count_and_pinsrw_preserves_exact_source_width() {
        for destination in 0..8 {
            for immediate in 0..=255 {
                let shift = decode_one(0x1000, &[0x66, 0x0f, 0x72, 0xf0 | destination, immediate]);
                assert_eq!(shift.opcode, Opcode::Sse(SseOpcode::Pslld));
                assert_eq!(
                    shift.operands,
                    vec![
                        Operand::Xmm(destination),
                        Operand::Imm(u32::from(immediate))
                    ]
                );
                for source in 0..8 {
                    let insert = decode_one(
                        0x1000,
                        &[
                            0x66,
                            0x0f,
                            0xc4,
                            0xc0 | destination << 3 | source,
                            immediate,
                        ],
                    );
                    assert_eq!(insert.opcode, Opcode::Sse(SseOpcode::Pinsrw));
                    assert_eq!(
                        insert.operands,
                        vec![
                            Operand::Xmm(destination),
                            Operand::Reg(Register {
                                index: source,
                                width: Width::W32,
                                high8: false
                            }),
                            Operand::Imm(u32::from(immediate))
                        ]
                    );
                    assert_eq!(
                        insert.flags_read | insert.flags_written | insert.flags_undefined,
                        0
                    );
                    assert_eq!(crate::optimize::written_gprs(&insert), 0);
                }
            }
        }
        for (prefix, segment) in [
            (0x64, Segment::Fs),
            (0x36, Segment::Ss),
            (0x3e, Segment::Flat),
        ] {
            let instruction = decode_one(0x1000, &[prefix, 0x66, 0x0f, 0xc4, 0x4b, 1, 0xff]);
            assert_eq!(instruction.opcode, Opcode::Sse(SseOpcode::Pinsrw));
            assert_eq!(
                instruction.operands,
                vec![
                    Operand::Xmm(1),
                    Operand::Mem(MemoryOperand {
                        base: Some(3),
                        index: None,
                        scale: 1,
                        displacement: 1,
                        segment,
                        width: Width::W16
                    }),
                    Operand::Imm(0xff)
                ]
            );
        }
        for code in [
            &[0x0f, 0xc4, 0xc1, 0][..],
            &[0xc5, 0xf9, 0xc4, 0xc1, 0][..],
            &[0x67, 0x66, 0x0f, 0xc4, 0x03, 0][..],
            &[0x65, 0x66, 0x0f, 0xc4, 0x03, 0][..],
            &[0x0f, 0x72, 0xf0, 0][..],
            &[0xc5, 0xf9, 0x72, 0xf0, 0][..],
        ] {
            let instruction = decode_one(0x1000, code);
            assert_eq!(instruction.opcode, Opcode::Unsupported);
            assert_ne!(
                instruction.unsupported,
                Some(UnsupportedReason::InvalidInstruction)
            );
        }
        for code in [
            &[0xf0, 0x66, 0x0f, 0xc4, 0xc1, 0][..],
            &[0xf0, 0x66, 0x0f, 0x72, 0xf0, 0][..],
            &[0x66, 0x0f, 0x72, 0x30, 0][..],
        ] {
            assert_eq!(
                decode_one(0x1000, code).unsupported,
                Some(UnsupportedReason::InvalidInstruction)
            );
        }
    }

    #[test]
    fn sse_maskmovdqu_exposes_implicit_edi_memory_without_changing_register_banks() {
        for data in 0..8 {
            for mask in 0..8 {
                for (prefix, segment) in [
                    (0x64, Segment::Fs),
                    (0x36, Segment::Ss),
                    (0x3e, Segment::Flat),
                    (0x26, Segment::Flat),
                ] {
                    let instruction =
                        decode_one(0x1000, &[prefix, 0x66, 0x0f, 0xf7, 0xc0 | data << 3 | mask]);
                    assert_eq!(instruction.opcode, Opcode::Sse(SseOpcode::Maskmovdqu));
                    assert_eq!(
                        instruction.operands,
                        vec![
                            Operand::Mem(MemoryOperand {
                                base: Some(7),
                                index: None,
                                scale: 1,
                                displacement: 0,
                                segment,
                                width: Width::W128
                            }),
                            Operand::Xmm(data),
                            Operand::Xmm(mask)
                        ]
                    );
                    assert_eq!(
                        instruction.flags_read
                            | instruction.flags_written
                            | instruction.flags_undefined,
                        0
                    );
                    assert_eq!(crate::optimize::written_gprs(&instruction), 0);
                }
            }
        }
        for code in [
            &[0x0f, 0xf7, 0xc1][..],
            &[0xc5, 0xf9, 0xf7, 0xc1][..],
            &[0x67, 0x66, 0x0f, 0xf7, 0xc1][..],
            &[0x65, 0x66, 0x0f, 0xf7, 0xc1][..],
            &[0x2e, 0x66, 0x0f, 0xf7, 0xc1][..],
        ] {
            let instruction = decode_one(0x1000, code);
            assert_eq!(instruction.opcode, Opcode::Unsupported);
            assert_ne!(
                instruction.unsupported,
                Some(UnsupportedReason::InvalidInstruction)
            );
        }
        for code in [
            &[0xf0, 0x66, 0x0f, 0xf7, 0xc1][..],
            &[0x66, 0x0f, 0xf7, 0x01][..],
        ] {
            assert_eq!(
                decode_one(0x1000, code).unsupported,
                Some(UnsupportedReason::InvalidInstruction)
            );
        }
    }

    #[test]
    fn sse_psrad_immediate_keeps_all_count_bits_and_rejects_other_register_banks() {
        for destination in 0..8 {
            for count in 0..=255 {
                let instruction =
                    decode_one(0x1000, &[0x66, 0x0f, 0x72, 0xe0 | destination, count]);
                assert_eq!(instruction.opcode, Opcode::Sse(SseOpcode::Psrad));
                assert_eq!(
                    instruction.operands,
                    vec![Operand::Xmm(destination), Operand::Imm(u32::from(count))]
                );
                assert_eq!(
                    instruction.flags_read
                        | instruction.flags_written
                        | instruction.flags_undefined,
                    0
                );
                assert_eq!(crate::optimize::written_gprs(&instruction), 0);
            }
        }
        for bytes in [
            &[0x0f, 0x72, 0xe0, 16][..],
            &[0xc5, 0xf9, 0x72, 0xe0, 16][..],
        ] {
            let instruction = decode_one(0x1000, bytes);
            assert_eq!(instruction.opcode, Opcode::Unsupported);
            assert_ne!(
                instruction.unsupported,
                Some(UnsupportedReason::InvalidInstruction)
            );
        }
        assert_eq!(
            decode_one(0x1000, &[0xf0, 0x66, 0x0f, 0x72, 0xe0, 16]).unsupported,
            Some(UnsupportedReason::InvalidInstruction)
        );
        for prefix in [0xf2, 0xf3] {
            assert_eq!(
                decode_one(0x1000, &[prefix, 0x66, 0x0f, 0x72, 0xe0, 16]).opcode,
                Opcode::Unsupported
            );
        }
    }

    #[test]
    fn reached_sse_bitwise_and_move_preserve_full_width_and_register_identity() {
        let xor = decode_one(0x1000, &[0x0f, 0x57, 0xc0]);
        assert_eq!(xor.opcode, Opcode::Sse(SseOpcode::Xorps));
        assert_eq!(xor.operands, vec![Operand::Xmm(0), Operand::Xmm(0)]);
        assert_eq!(xor.flags_written, 0);
        let pxor = decode_one(0x1000, &[0x66, 0x0f, 0xef, 0xc1]);
        assert_eq!(pxor.opcode, Opcode::Sse(SseOpcode::Pxor));
        assert_eq!(pxor.operands, vec![Operand::Xmm(0), Operand::Xmm(1)]);
        assert_eq!(pxor.flags_written, 0);
        assert_eq!(
            decode_one(0x1000, &[0x0f, 0xef, 0xc1]).opcode,
            Opcode::Mmx(MmxOpcode::Pxor),
            "MMX PXOR must not enter the SSE path and lose x87 alias effects"
        );
        let store = decode_one(0x1000, &[0x0f, 0x11, 0x44, 0x24, 8]);
        assert_eq!(store.opcode, Opcode::Sse(SseOpcode::Movups));
        assert!(matches!(
            store.operands[0],
            Operand::Mem(MemoryOperand {
                width: Width::W128,
                segment: Segment::Ss,
                ..
            })
        ));
        assert_eq!(store.operands[1], Operand::Xmm(0));
        let aligned_store = decode_one(0x1000, &[0x66, 0x0f, 0x7f, 0x01]);
        assert_eq!(aligned_store.opcode, Opcode::Sse(SseOpcode::Movdqa));
        assert!(matches!(
            aligned_store.operands[0],
            Operand::Mem(MemoryOperand {
                width: Width::W128,
                ..
            })
        ));
        assert_eq!(aligned_store.operands[1], Operand::Xmm(0));
        assert_eq!(
            decode_one(0x1000, &[0x66, 0x0f, 0x57, 0xc0]).opcode,
            Opcode::Sse(SseOpcode::Xorps)
        );
    }

    #[test]
    fn packed_xor_keeps_mmx_and_sse_register_files_and_memory_widths_distinct() {
        for destination in 0..8 {
            for source in 0..8 {
                let modrm = 0xc0 | destination << 3 | source;
                let mmx = decode_one(0x1000, &[0x0f, 0xef, modrm]);
                assert_eq!(mmx.opcode, Opcode::Mmx(MmxOpcode::Pxor));
                assert_eq!(mmx.width, Width::W64);
                assert_eq!(
                    mmx.operands,
                    vec![Operand::Mmx(destination), Operand::Mmx(source)]
                );
                assert_eq!(mmx.flags_read | mmx.flags_written | mmx.flags_undefined, 0);
                assert_eq!(crate::optimize::written_gprs(&mmx), 0);
                let sse = decode_one(0x1000, &[0x66, 0x0f, 0xef, modrm]);
                assert_eq!(sse.opcode, Opcode::Sse(SseOpcode::Pxor));
                assert_eq!(sse.width, Width::W128);
                assert_eq!(
                    sse.operands,
                    vec![Operand::Xmm(destination), Operand::Xmm(source)]
                );
            }
        }
        for (bytes, segment, base, displacement) in [
            (&[0x0f, 0xef, 0x48, 0x01][..], Segment::Flat, 0, 1),
            (&[0x64, 0x0f, 0xef, 0x48, 0x01][..], Segment::Fs, 0, 1),
            (&[0x0f, 0xef, 0x4d, 0xf9][..], Segment::Ss, 5, -7),
        ] {
            let instruction = decode_one(0x1000, bytes);
            assert_eq!(instruction.opcode, Opcode::Mmx(MmxOpcode::Pxor));
            assert_eq!(instruction.operands[0], Operand::Mmx(1));
            assert_eq!(
                instruction.operands[1],
                Operand::Mem(MemoryOperand {
                    base: Some(base),
                    index: None,
                    scale: 1,
                    displacement: displacement as u32,
                    segment,
                    width: Width::W64,
                })
            );
        }
        assert_eq!(
            decode_one(0x1000, &[0xf0, 0x0f, 0xef, 0x00]).unsupported,
            Some(UnsupportedReason::InvalidInstruction)
        );
    }

    #[test]
    fn mmx_high_unpack_uses_the_high_half_of_raw64_and_preserves_operand_banks() {
        for (encoding, opcode) in [
            (0x68, MmxOpcode::Punpckhbw),
            (0x69, MmxOpcode::Punpckhwd),
            (0x6a, MmxOpcode::Punpckhdq),
        ] {
            for destination in 0..8 {
                for source in 0..8 {
                    let modrm = 0xc0 | destination << 3 | source;
                    let instruction = decode_one(0x1000, &[0x0f, encoding, modrm]);
                    assert_eq!(instruction.opcode, Opcode::Mmx(opcode));
                    assert_eq!(instruction.width, Width::W64);
                    assert_eq!(
                        instruction.operands,
                        vec![Operand::Mmx(destination), Operand::Mmx(source)]
                    );
                    assert_eq!(
                        instruction.flags_read
                            | instruction.flags_written
                            | instruction.flags_undefined,
                        0
                    );
                    assert_eq!(crate::optimize::written_gprs(&instruction), 0);
                    // The mandatory-66 XMM form must never enter the MMX bank.
                    let xmm = decode_one(0x1000, &[0x66, 0x0f, encoding, modrm]);
                    if encoding == 0x69 {
                        assert_eq!(xmm.opcode, Opcode::Sse(SseOpcode::Punpckhwd));
                        assert_eq!(
                            xmm.operands,
                            vec![Operand::Xmm(destination), Operand::Xmm(source)]
                        );
                    } else {
                        assert_eq!(xmm.unsupported, Some(UnsupportedReason::Operand));
                    }
                }
            }
            for (prefix, modrm, displacement, segment, base) in [
                (None, 0x48, 1u8, Segment::Flat, 0),
                (Some(0x64), 0x48, 1, Segment::Fs, 0),
                (None, 0x4d, 0xf9, Segment::Ss, 5),
            ] {
                let mut bytes = Vec::new();
                bytes.extend(prefix);
                bytes.extend([0x0f, encoding, modrm, displacement]);
                let instruction = decode_one(0x1000, &bytes);
                assert_eq!(instruction.opcode, Opcode::Mmx(opcode));
                assert_eq!(instruction.operands[0], Operand::Mmx(1));
                assert_eq!(
                    instruction.operands[1],
                    Operand::Mem(MemoryOperand {
                        base: Some(base),
                        index: None,
                        scale: 1,
                        displacement: i32::from(displacement as i8) as u32,
                        segment,
                        width: Width::W64,
                    })
                );
            }
            assert_eq!(
                decode_one(0x1000, &[0xf0, 0x0f, encoding, 0x00]).unsupported,
                Some(UnsupportedReason::InvalidInstruction)
            );
            let xmm = decode_one(0x1000, &[0x66, 0x0f, encoding, 0x00]);
            if encoding == 0x69 {
                assert_eq!(xmm.opcode, Opcode::Sse(SseOpcode::Punpckhwd));
                assert!(matches!(
                    xmm.operands[1],
                    Operand::Mem(MemoryOperand {
                        width: Width::W128,
                        ..
                    })
                ));
            } else {
                assert_eq!(xmm.unsupported, Some(UnsupportedReason::Operand));
            }
        }
    }

    #[test]
    fn mmx_logical_word_shift_preserves_full_immediate_and_count_operand_identity() {
        for destination in 0..8 {
            for count in 0..=255 {
                let instruction = decode_one(0x1000, &[0x0f, 0x71, 0xd0 | destination, count]);
                assert_eq!(instruction.opcode, Opcode::Mmx(MmxOpcode::Psrlw));
                assert_eq!(instruction.width, Width::W64);
                assert_eq!(
                    instruction.operands,
                    vec![Operand::Mmx(destination), Operand::Imm(u32::from(count))]
                );
                assert_eq!(
                    instruction.flags_read
                        | instruction.flags_written
                        | instruction.flags_undefined,
                    0
                );
                assert_eq!(crate::optimize::written_gprs(&instruction), 0);
            }
            for source in 0..8 {
                let instruction =
                    decode_one(0x1000, &[0x0f, 0xd1, 0xc0 | destination << 3 | source]);
                assert_eq!(instruction.opcode, Opcode::Mmx(MmxOpcode::Psrlw));
                assert_eq!(
                    instruction.operands,
                    vec![Operand::Mmx(destination), Operand::Mmx(source)]
                );
            }
        }
        for (bytes, segment) in [
            (&[0x0f, 0xd1, 0x48, 1][..], Segment::Flat),
            (&[0x64, 0x0f, 0xd1, 0x48, 1][..], Segment::Fs),
        ] {
            let instruction = decode_one(0x1000, bytes);
            assert_eq!(instruction.opcode, Opcode::Mmx(MmxOpcode::Psrlw));
            assert_eq!(
                instruction.operands,
                vec![
                    Operand::Mmx(1),
                    Operand::Mem(MemoryOperand {
                        base: Some(0),
                        index: None,
                        scale: 1,
                        displacement: 1,
                        segment,
                        width: Width::W64
                    })
                ]
            );
        }
        assert_eq!(
            decode_one(0x1000, &[0x66, 0x0f, 0x71, 0xd1, 8]).opcode,
            Opcode::Sse(SseOpcode::Psrlw),
            "SSE PSRLW has separate full-width register state"
        );
        assert_eq!(
            decode_one(0x1000, &[0xf0, 0x0f, 0xd1, 0x08]).unsupported,
            Some(UnsupportedReason::InvalidInstruction)
        );
        assert_eq!(
            decode_one(0x1000, &[0x0f, 0x71, 0x10, 8]).unsupported,
            Some(UnsupportedReason::InvalidInstruction)
        );
    }

    #[test]
    fn cross_bank_mmx_transfers_keep_both_register_files_and_pending_fault_category() {
        for destination in 0..8 {
            for source in 0..8 {
                for (prefix, operation, operands) in [
                    (
                        0xf3,
                        MmxOpcode::Movq2dq,
                        vec![Operand::Xmm(destination), Operand::Mmx(source)],
                    ),
                    (
                        0xf2,
                        MmxOpcode::Movdq2q,
                        vec![Operand::Mmx(destination), Operand::Xmm(source)],
                    ),
                ] {
                    let instruction = decode_one(
                        0x1000,
                        &[prefix, 0x0f, 0xd6, 0xc0 | destination << 3 | source],
                    );
                    assert_eq!(instruction.opcode, Opcode::Mmx(operation));
                    assert_eq!(instruction.operands, operands);
                    assert_eq!(
                        instruction.flags_read
                            | instruction.flags_written
                            | instruction.flags_undefined,
                        0
                    );
                    assert_eq!(crate::optimize::written_gprs(&instruction), 0);
                }
            }
        }
        for prefix in [0xf2, 0xf3] {
            for bytes in [
                vec![prefix, 0x0f, 0xd6, 0x00],
                vec![0xf0, prefix, 0x0f, 0xd6, 0xc0],
            ] {
                assert_eq!(
                    decode_one(0x1000, &bytes).unsupported,
                    Some(UnsupportedReason::InvalidInstruction)
                );
            }
        }
    }

    #[test]
    fn packed_sse_pixel_operations_keep_128_bit_sources_and_mmx_forms_distinct() {
        for (encoding, sse, mmx) in [
            (0x60, SseOpcode::Punpcklbw, MmxOpcode::Punpcklbw),
            (0xd5, SseOpcode::Pmullw, MmxOpcode::Pmullw),
            (0xd1, SseOpcode::Psrlw, MmxOpcode::Psrlw),
            (0x67, SseOpcode::Packuswb, MmxOpcode::Packuswb),
        ] {
            for destination in 0..8 {
                for source in 0..8 {
                    let modrm = 0xc0 | destination << 3 | source;
                    let instruction = decode_one(0x1000, &[0x66, 0x0f, encoding, modrm]);
                    assert_eq!(instruction.opcode, Opcode::Sse(sse));
                    assert_eq!(instruction.width, Width::W128);
                    assert_eq!(
                        instruction.operands,
                        vec![Operand::Xmm(destination), Operand::Xmm(source)]
                    );
                    assert_eq!(
                        instruction.flags_read
                            | instruction.flags_written
                            | instruction.flags_undefined,
                        0
                    );
                    assert_eq!(crate::optimize::written_gprs(&instruction), 0);
                    assert_eq!(
                        decode_one(0x1000, &[0x0f, encoding, modrm]).opcode,
                        Opcode::Mmx(mmx)
                    );
                }
            }
            let memory = decode_one(0x1000, &[0x64, 0x66, 0x0f, encoding, 0x50, 0x10]);
            assert_eq!(memory.opcode, Opcode::Sse(sse));
            assert_eq!(
                memory.operands,
                vec![
                    Operand::Xmm(2),
                    Operand::Mem(MemoryOperand {
                        base: Some(0),
                        index: None,
                        scale: 1,
                        displacement: 16,
                        segment: Segment::Fs,
                        width: Width::W128,
                    })
                ]
            );
            assert_eq!(
                decode_one(0x1000, &[0xf0, 0x66, 0x0f, encoding, 0x00]).unsupported,
                Some(UnsupportedReason::InvalidInstruction)
            );
        }
        for destination in 0..8 {
            for count in 0..=255 {
                let instruction =
                    decode_one(0x1000, &[0x66, 0x0f, 0x71, 0xd0 | destination, count]);
                assert_eq!(instruction.opcode, Opcode::Sse(SseOpcode::Psrlw));
                assert_eq!(
                    instruction.operands,
                    vec![Operand::Xmm(destination), Operand::Imm(u32::from(count))]
                );
                assert_eq!(
                    instruction.flags_read
                        | instruction.flags_written
                        | instruction.flags_undefined,
                    0
                );
            }
        }
        assert_eq!(
            decode_one(0x1000, &[0x66, 0x0f, 0x71, 0x10, 8]).unsupported,
            Some(UnsupportedReason::InvalidInstruction)
        );
    }

    #[test]
    fn non_temporal_mmx_store_keeps_m64_source_and_segment_identity() {
        for source in 0..8 {
            let instruction = decode_one(0x1000, &[0x0f, 0xe7, 0x04 | source << 3, 0x02]);
            assert_eq!(instruction.opcode, Opcode::Mmx(MmxOpcode::Movntq));
            assert_eq!(instruction.width, Width::W64);
            assert_eq!(
                instruction.operands,
                vec![
                    Operand::Mem(MemoryOperand {
                        base: Some(2),
                        index: Some(0),
                        scale: 1,
                        displacement: 0,
                        segment: Segment::Flat,
                        width: Width::W64,
                    }),
                    Operand::Mmx(source),
                ]
            );
            assert_eq!(
                instruction.flags_read | instruction.flags_written | instruction.flags_undefined,
                0
            );
            assert_eq!(crate::optimize::written_gprs(&instruction), 0);
        }
        for (bytes, segment) in [
            (&[0x64, 0x0f, 0xe7, 0x40, 0x01][..], Segment::Fs),
            (&[0x0f, 0xe7, 0x45, 0xf9][..], Segment::Ss),
        ] {
            let instruction = decode_one(0x1000, bytes);
            assert_eq!(instruction.opcode, Opcode::Mmx(MmxOpcode::Movntq));
            assert!(matches!(instruction.operands[0], Operand::Mem(memory)
                if memory.segment == segment && memory.width == Width::W64));
        }
        for bytes in [&[0x0f, 0xe7, 0xc1][..], &[0xf0, 0x0f, 0xe7, 0x00][..]] {
            assert_eq!(
                decode_one(0x1000, bytes).unsupported,
                Some(UnsupportedReason::InvalidInstruction)
            );
        }
        assert_eq!(
            decode_one(0x1000, &[0x66, 0x0f, 0xe7, 0x00]).opcode,
            Opcode::Unsupported,
            "MOVNTDQ must not enter the MMX path"
        );
    }

    #[test]
    fn aligned_packed_transfers_keep_direction_width_and_non_temporal_store_only() {
        for destination in 0..8 {
            for source in 0..8 {
                for (encoding, modrm) in [
                    (0x28, 0xc0 | destination << 3 | source),
                    (0x29, 0xc0 | source << 3 | destination),
                ] {
                    let instruction = decode_one(0x1000, &[0x0f, encoding, modrm]);
                    assert_eq!(instruction.opcode, Opcode::Sse(SseOpcode::Movaps));
                    assert_eq!(instruction.width, Width::W128);
                    assert_eq!(
                        instruction.operands,
                        vec![Operand::Xmm(destination), Operand::Xmm(source)]
                    );
                    assert_eq!(instruction.flags_read, 0);
                    assert_eq!(instruction.flags_written, 0);
                    assert_eq!(instruction.flags_undefined, 0);
                    assert_eq!(crate::optimize::written_gprs(&instruction), 0);
                }
            }
        }
        for (bytes, operation, memory_index, segment, base, index, displacement) in [
            (
                &[0x0f, 0x28, 0x00][..],
                SseOpcode::Movaps,
                1,
                Segment::Flat,
                0,
                None,
                0,
            ),
            (
                &[0x0f, 0x29, 0x45, 0xf0][..],
                SseOpcode::Movaps,
                0,
                Segment::Ss,
                5,
                None,
                -16,
            ),
            (
                &[0x0f, 0x2b, 0x04, 0x02][..],
                SseOpcode::Movntps,
                0,
                Segment::Flat,
                2,
                Some(0),
                0,
            ),
            (
                &[0x64, 0x0f, 0x28, 0x40, 0x08][..],
                SseOpcode::Movaps,
                1,
                Segment::Fs,
                0,
                None,
                8,
            ),
            (
                &[0x64, 0x0f, 0x2b, 0x40, 0x08][..],
                SseOpcode::Movntps,
                0,
                Segment::Fs,
                0,
                None,
                8,
            ),
        ] {
            let instruction = decode_one(0x1000, bytes);
            assert_eq!(instruction.opcode, Opcode::Sse(operation));
            assert_eq!(instruction.width, Width::W128);
            let Operand::Mem(memory) = instruction.operands[memory_index] else {
                panic!("expected m128 operand");
            };
            assert_eq!(memory.width, Width::W128);
            assert_eq!(memory.segment, segment);
            assert_eq!(memory.base, Some(base));
            assert_eq!(memory.index, index);
            assert_eq!(memory.displacement, displacement as u32);
            assert_eq!(instruction.operands[1 - memory_index], Operand::Xmm(0));
            assert_eq!(instruction.flags_written | instruction.flags_undefined, 0);
        }
        let register_nt = decode_one(0x1000, &[0x0f, 0x2b, 0xc1]);
        assert_eq!(
            register_nt.unsupported,
            Some(UnsupportedReason::InvalidInstruction)
        );
        for bytes in [
            &[0x66, 0x0f, 0x2b, 0x00][..], // MOVNTPD is a distinct operation.
            &[0x66, 0x0f, 0xe7, 0x00][..], // MOVNTDQ remains unsupported.
        ] {
            assert_eq!(decode_one(0x1000, bytes).opcode, Opcode::Unsupported);
        }
        for bytes in [&[0xf0, 0x0f, 0x28, 0x00][..], &[0xf0, 0x0f, 0x2b, 0x00][..]] {
            assert_eq!(
                decode_one(0x1000, bytes).unsupported,
                Some(UnsupportedReason::InvalidInstruction)
            );
        }
    }

    #[test]
    fn scalar_sse_truncation_keeps_r32_destination_and_exact_source_width() {
        for (bytes, operation, source) in [
            (
                &[0xf2, 0x0f, 0x2c, 0x04, 0x24][..],
                SseOpcode::Cvttsd2si,
                Operand::Mem(MemoryOperand {
                    base: Some(4),
                    index: None,
                    scale: 1,
                    displacement: 0,
                    segment: Segment::Ss,
                    width: Width::W64,
                }),
            ),
            (
                &[0xf3, 0x0f, 0x2c, 0xc7][..],
                SseOpcode::Cvttss2si,
                Operand::Xmm(7),
            ),
            (
                &[0x64, 0xf3, 0x0f, 0x2c, 0x40, 4][..],
                SseOpcode::Cvttss2si,
                Operand::Mem(MemoryOperand {
                    base: Some(0),
                    index: None,
                    scale: 1,
                    displacement: 4,
                    segment: Segment::Fs,
                    width: Width::W32,
                }),
            ),
        ] {
            let instruction = decode_one(0x1000, bytes);
            assert_eq!(instruction.opcode, Opcode::Sse(operation));
            assert_eq!(instruction.len as usize, bytes.len());
            assert_eq!(instruction.width, Width::W32);
            assert_eq!(
                instruction.operands,
                vec![
                    Operand::Reg(Register {
                        index: 0,
                        width: Width::W32,
                        high8: false,
                    }),
                    source
                ]
            );
            assert_eq!(instruction.flags_written | instruction.flags_undefined, 0);
            assert_eq!(crate::optimize::written_gprs(&instruction), 1);
        }
        for bytes in [
            &[0xf0, 0xf2, 0x0f, 0x2c, 0xc0][..],
            &[0x67, 0xf3, 0x0f, 0x2c, 0x00][..],
            &[0xc5, 0xfa, 0x2c, 0xc0][..],
            &[0x0f, 0x2c, 0xc0][..],
        ] {
            assert_eq!(decode_one(0x1000, bytes).opcode, Opcode::Unsupported);
        }
    }

    #[test]
    fn mxcsr_transfers_keep_exact_m32_memory_effects_and_reject_invalid_encodings() {
        for (bytes, opcode, segment) in [
            (
                &[0x0f, 0xae, 0x5c, 0x24, 4][..],
                SseOpcode::Stmxcsr,
                Segment::Ss,
            ),
            (&[0x0f, 0xae, 0x10][..], SseOpcode::Ldmxcsr, Segment::Flat),
            (
                &[0x64, 0x0f, 0xae, 0x18][..],
                SseOpcode::Stmxcsr,
                Segment::Fs,
            ),
        ] {
            let instruction = decode_one(0x1000, bytes);
            assert_eq!(instruction.opcode, Opcode::Sse(opcode));
            assert_eq!(instruction.len as usize, bytes.len());
            assert_eq!(instruction.width, Width::W32);
            assert!(matches!(instruction.operands.as_slice(), [Operand::Mem(m)]
                if m.width == Width::W32 && m.segment == segment));
            assert_eq!(instruction.flags_written, 0);
            assert_eq!(instruction.flags_undefined, 0);
        }
        for bytes in [
            &[0x66, 0x0f, 0xae, 0x10][..],
            &[0x66, 0x0f, 0xae, 0x18][..],
            &[0xf0, 0x0f, 0xae, 0x10][..],
            &[0xf0, 0x0f, 0xae, 0x18][..],
            &[0x0f, 0xae, 0xd0][..],
            &[0x0f, 0xae, 0xd8][..],
        ] {
            assert_eq!(
                decode_one(0x1000, bytes).unsupported,
                Some(UnsupportedReason::InvalidInstruction)
            );
        }
    }

    #[test]
    fn reached_prefetch_hint_does_not_become_an_architectural_load() {
        let instruction = decode_one(0x1000, &[0x0f, 0x18, 0x81, 0x80, 0, 0, 0]);
        assert_eq!(instruction.opcode, Opcode::Nop);
        assert!(instruction.operands.is_empty());
        assert_eq!(instruction.next_eip(), 0x1007);
    }
    #[test]
    fn string_decode_preserves_rep_width_source_segment_and_fixed_destination() {
        for (bytes, operation, width, repeat) in [
            (&[0xf3, 0xa4][..], StringOpcode::Movs, Width::W8, true),
            (
                &[0xf3, 0x66, 0xa5][..],
                StringOpcode::Movs,
                Width::W16,
                true,
            ),
            (&[0xa5][..], StringOpcode::Movs, Width::W32, false),
            (
                &[0x64, 0xf3, 0xab][..],
                StringOpcode::Stos,
                Width::W32,
                true,
            ),
        ] {
            let i = decode_one(0x1000, bytes);
            assert_eq!(i.opcode, Opcode::String(operation, repeat));
            assert_eq!(i.width, width);
            assert!(matches!(
                i.operands[0],
                Operand::Mem(MemoryOperand {
                    base: Some(7),
                    segment: Segment::Flat,
                    ..
                })
            ));
        }
        let fs = decode_one(0x1000, &[0x64, 0xf3, 0xa4]);
        assert!(matches!(
            fs.operands[1],
            Operand::Mem(MemoryOperand {
                base: Some(6),
                segment: Segment::Fs,
                ..
            })
        ));
        assert_eq!(
            decode_one(0x1000, &[0x67, 0xf3, 0xa4]).unsupported,
            Some(UnsupportedReason::Address16)
        );
        assert_eq!(
            decode_one(0x1000, &[0xf2, 0xa4]).unsupported,
            Some(UnsupportedReason::RepeatPrefix)
        );
        assert_eq!(
            decode_one(0x1000, &[0x2e, 0xa4]).unsupported,
            Some(UnsupportedReason::Segment)
        );
    }

    #[test]
    fn repeat_instruction_and_its_continuation_are_explicit_resume_entries() {
        let region = discover(&Code(vec![0x40, 0xf3, 0xa4, 0x41, 0xcc]), 0x1000, 16);
        assert!(region.blocks.contains_key(&0x1000));
        assert!(region.blocks.contains_key(&0x1001));
        assert!(region.blocks.contains_key(&0x1003));
        assert_eq!(region.blocks[&0x1001].instructions.len(), 1);
        assert_eq!(region.blocks[&0x1001].successors, vec![0x1003]);
    }

    #[test]
    fn cmps_decode_tracks_both_indices_source_segment_and_repeat_condition() {
        for (bytes, width, repeat, condition, segment) in [
            (&[0xa6][..], Width::W8, false, Condition::E, Segment::Flat),
            (
                &[0xf2, 0xa6][..],
                Width::W8,
                true,
                Condition::Ne,
                Segment::Flat,
            ),
            (
                &[0x66, 0xf3, 0xa7][..],
                Width::W16,
                true,
                Condition::E,
                Segment::Flat,
            ),
            (
                &[0x64, 0xf3, 0xa7][..],
                Width::W32,
                true,
                Condition::E,
                Segment::Fs,
            ),
            (
                &[0x36, 0xa7][..],
                Width::W32,
                false,
                Condition::E,
                Segment::Ss,
            ),
        ] {
            let instruction = decode_one(0x1000, bytes);
            assert_eq!(
                instruction.opcode,
                Opcode::String(StringOpcode::Cmps, repeat)
            );
            assert_eq!(instruction.width, width);
            assert_eq!(instruction.condition, Some(condition));
            assert_eq!(instruction.flags_written, crate::state::ARITHMETIC_FLAGS);
            for (operand, base, expected_segment) in [
                (instruction.operands[0], 7, Segment::Flat),
                (instruction.operands[1], 6, segment),
            ] {
                assert!(
                    matches!(operand, Operand::Mem(m) if m.base == Some(base) && m.segment == expected_segment)
                );
            }
            assert_eq!(
                crate::optimize::written_gprs(&instruction),
                0xc0 | if repeat { 2 } else { 0 }
            );
        }
        assert_eq!(
            decode_one(0x1000, &[0x67, 0xf3, 0xa7]).unsupported,
            Some(UnsupportedReason::Address16)
        );
        assert_eq!(
            decode_one(0x1000, &[0x65, 0xa6]).unsupported,
            Some(UnsupportedReason::Segment)
        );
        assert!(decode_one(0x1000, &[0xf0, 0xa6]).unsupported.is_some());
        // Scalar SSE CMPSD is a different instruction, never a string compare.
        assert!(!matches!(
            decode_one(0x1000, &[0xf2, 0x0f, 0xc2, 0xc1, 0]).opcode,
            Opcode::String(_, _)
        ));
    }

    #[test]
    fn scas_decode_preserves_comparison_width_repeat_condition_and_fixed_es() {
        for (bytes, width, repeat, condition) in [
            (&[0xae][..], Width::W8, false, Condition::E),
            (&[0xf2, 0xae][..], Width::W8, true, Condition::Ne),
            (&[0x66, 0xf3, 0xaf][..], Width::W16, true, Condition::E),
            (&[0x65, 0xf2, 0xaf][..], Width::W32, true, Condition::Ne),
            (&[0x64, 0xaf][..], Width::W32, false, Condition::E),
        ] {
            let instruction = decode_one(0x1000, bytes);
            assert_eq!(
                instruction.opcode,
                Opcode::String(StringOpcode::Scas, repeat)
            );
            assert_eq!(instruction.width, width);
            assert_eq!(instruction.condition, Some(condition));
            assert_eq!(instruction.flags_written, crate::state::ARITHMETIC_FLAGS);
            assert!(matches!(
                instruction.operands[0],
                Operand::Mem(MemoryOperand {
                    base: Some(7),
                    segment: Segment::Flat,
                    ..
                })
            ));
            assert_eq!(
                crate::optimize::written_gprs(&instruction),
                (1 << 7) | if repeat { 1 << 1 } else { 0 }
            );
        }
        assert_eq!(
            decode_one(0x1000, &[0x67, 0xf2, 0xae]).unsupported,
            Some(UnsupportedReason::Address16)
        );
        assert!(decode_one(0x1000, &[0xf0, 0xae]).unsupported.is_some());
    }

    #[test]
    fn sahf_reads_implicit_ah_and_defines_only_five_status_flags() {
        for bytes in [&[0x9e][..], &[0x66, 0x9e][..], &[0x64, 0x9e][..]] {
            let instruction = decode_one(0x1000, bytes);
            assert_eq!(instruction.opcode, Opcode::Sahf);
            assert_eq!(instruction.width, Width::W32);
            assert!(instruction.operands.is_empty());
            assert_eq!(instruction.flags_read, 0);
            assert_eq!(instruction.flags_written, 0xd5);
            assert_eq!(instruction.flags_undefined, 0);
            assert_eq!(instruction.next_eip(), 0x1000 + bytes.len() as u32);
        }
        let locked = decode_one(0x1000, &[0xf0, 0x9e]);
        assert_eq!(locked.opcode, Opcode::Unsupported);
        assert_eq!(
            locked.unsupported,
            Some(UnsupportedReason::InvalidInstruction)
        );
        assert_eq!(decode_one(0x1000, &[0x9f]).opcode, Opcode::Unsupported);
    }
    #[test]
    fn direction_flag_instructions_define_only_df_without_register_or_memory_operands() {
        for (bytes, opcode) in [
            (&[0xfc][..], Opcode::Cld),
            (&[0xfd][..], Opcode::Std),
            (&[0x66, 0xfc][..], Opcode::Cld),
            (&[0x64, 0xfd][..], Opcode::Std),
        ] {
            let instruction = decode_one(0x1000, bytes);
            assert_eq!(instruction.opcode, opcode);
            assert!(instruction.operands.is_empty());
            assert_eq!(instruction.flags_read, 0);
            assert_eq!(instruction.flags_written, 0x400);
            assert_eq!(instruction.flags_undefined, 0);
            assert_eq!(crate::optimize::written_gprs(&instruction), 0);
        }
        assert_eq!(
            decode_one(0x1000, &[0xf0, 0xfc]).unsupported,
            Some(UnsupportedReason::InvalidInstruction)
        );
        assert_eq!(
            decode_one(0x1000, &[0xf0, 0xfd]).unsupported,
            Some(UnsupportedReason::InvalidInstruction)
        );
    }
}

//! Architecture-level operations shared by discovery and code generation.
//!
//! Register indices follow the x86 encoding order (EAX, ECX, EDX, EBX, ESP,
//! EBP, ESI, EDI). No decoder or previous execution engine state is an ABI.

use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Width {
    W8,
    W16,
    W32,
    W64,
    W80,
    W128,
}

impl Width {
    pub const fn bits(self) -> u32 {
        match self {
            Self::W8 => 8,
            Self::W16 => 16,
            Self::W32 => 32,
            Self::W64 => 64,
            Self::W80 => 80,
            Self::W128 => 128,
        }
    }

    pub const fn bytes(self) -> u32 {
        self.bits() / 8
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Register {
    pub index: u8,
    pub width: Width,
    pub high8: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Segment {
    Flat,
    Ss,
    Fs,
}

/// Address arithmetic wraps at 32 bits before adding the selected segment base.
/// `Lea` computes the effective offset and does not add a segment base.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MemoryOperand {
    pub base: Option<u8>,
    pub index: Option<u8>,
    pub scale: u8,
    pub displacement: u32,
    pub segment: Segment,
    pub width: Width,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operand {
    Reg(Register),
    /// Physical MMX register, aliasing bits 0..63 of the same x87 slot.
    Mmx(u8),
    Xmm(u8),
    /// Sign-extended immediates are represented as their 32-bit two's-complement
    /// value. The operation width determines subsequent truncation.
    Imm(u32),
    Mem(MemoryOperand),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Condition {
    O,
    No,
    B,
    Ae,
    E,
    Ne,
    Be,
    A,
    S,
    Ns,
    P,
    Np,
    L,
    Ge,
    Le,
    G,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Opcode {
    Nop,
    Cld,
    Std,
    /// Copy AH into SF/ZF/AF/PF/CF, preserving all other flags and every GPR.
    Sahf,
    /// Fixed guest capability profile. Serializing, with EAX/ECX inputs and
    /// EAX/EBX/ECX/EDX outputs; neither EFLAGS nor guest memory is changed.
    Cpuid,
    /// Runtime-owned timestamp sample. Exits before retirement; completion
    /// writes the full EDX:EAX pair regardless of the operand-size prefix.
    Rdtsc,
    /// Architecturally defined invalid-opcode trap; never retires.
    Ud2,
    /// Host diagnostic boundary before attempting the selected guest PC.
    /// It is not an x86 instruction, exception, or retired operation.
    DiagnosticStop,
    Mov,
    /// Register-only exchange. Memory exchange requires implicit locked RMW.
    Xchg,
    Movzx,
    Movsx,
    Lea,
    Add,
    Adc,
    Sub,
    Sbb,
    Cmp,
    Test,
    And,
    Or,
    Xor,
    Inc,
    Dec,
    Neg,
    Not,
    Shl,
    Shr,
    Sar,
    Shld,
    Shrd,
    Imul,
    /// One explicit source; implicit AL/AX/EAX input and full AX/DX:AX/EDX:EAX output.
    ImulWide,
    Mul,
    Div,
    Idiv,
    SignExtendPair,
    Push,
    Pop,
    Leave,
    Setcc,
    Cmovcc,
    Jcc,
    /// Decrement ECX without changing flags; optional E/Ne tests the prior ZF.
    Loop,
    /// Branch on ECX == 0 without changing ECX or any flags.
    Jecxz,
    Jmp,
    Call,
    Ret,
    Mmx(MmxOpcode),
    Sse(SseOpcode),
    X87 {
        descriptor: u32,
        fop: u16,
        segment: u32,
    },
    X87StatusAx,
    /// String operation with architectural per-element progress. Boolean is REP;
    /// SCAS/CMPS use `condition` E/Ne to distinguish REPE and REPNE.
    String(StringOpcode, bool),
    Unsupported,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MmxOpcode {
    Movd,
    Movq,
    /// Cross-bank transfers still enter MMX state and deliver pending #MF.
    Movq2dq,
    Movdq2q,
    /// m64 store with a non-temporal hint; no forced alignment in AC=0 state.
    Movntq,
    Pxor,
    Punpcklbw,
    Punpcklwd,
    Punpckhbw,
    Punpckhwd,
    Punpckhdq,
    Psubw,
    Pmullw,
    Psraw,
    Psrlw,
    Paddw,
    Packuswb,
    Emms,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SseOpcode {
    Xorps,
    Pxor,
    Por,
    Punpcklbw,
    Punpcklwd,
    Punpckhwd,
    Paddw,
    Psubw,
    Pmullw,
    Psrlw,
    Psrad,
    Psraw,
    Pslld,
    /// Insert low16 from r32/m16 at imm8[2:0], preserving the other seven words.
    Pinsrw,
    /// Implicit segment:EDI span, data XMM, mask XMM; high bits select byte writes.
    Maskmovdqu,
    Packuswb,
    Packsswb,
    Packssdw,
    /// r32/m32 bit transfer; loading an XMM register clears its upper 96 bits.
    Movd,
    /// Raw low64 transfer; every XMM destination clears its upper64.
    Movq,
    /// Raw low64 transfer; an XMM source preserves, an m64 source clears upper64.
    Movsd,
    /// Raw low64 memory transfer preserving the destination's upper64.
    Movlpd,
    /// Raw low32 transfer; an XMM source preserves, an m32 source clears upper96.
    Movss,
    Movups,
    Movdqa,
    Movaps,
    /// Aligned m128 store; non-temporal cache hint has no guest byte effect.
    Movntps,
    Ldmxcsr,
    Stmxcsr,
    /// Scalar IEEE input to signed r32, truncating independently of MXCSR.RC.
    Cvttss2si,
    Cvttsd2si,
    Cvtdq2ps,
    Mulps,
    Shufps,
    /// Four independently selected raw32 lanes from one XMM/m128 source.
    Pshufd,
    Addps,
    Subps,
    Cvttps2dq,
    /// Raw packed bits; ANDPS and ANDPD share this operation and memory contract.
    And,
    Psrlq,
    Psllq,
    Psubq,
    Psubd,
    Pcmpeqd,
    Pmaxsw,
    Pmovmskb,
    Pextrw,
    Unpcklpd,
    /// Two exact signed i32 to binary64 conversions; no MXCSR exceptions.
    Cvtdq2pd,
    Cvtsd2si,
    Addsd,
    Subsd,
    Mulsd,
    Addpd,
    Subpd,
    Mulpd,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StringOpcode {
    Movs,
    Stos,
    Scas,
    Cmps,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnsupportedReason {
    FetchUnavailable,
    TruncatedInstruction,
    InvalidInstruction,
    Address16,
    LockPrefix,
    RepeatPrefix,
    Segment,
    Operand,
    Control16,
    Instruction,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Instruction {
    pub eip: u32,
    pub len: u8,
    pub opcode: Opcode,
    pub width: Width,
    /// Intel order: destination first, followed by source(s). A direct branch
    /// has an `Imm` absolute guest address. RET optionally has an immediate
    /// stack adjustment in bytes, applied after reading its return address.
    pub operands: Vec<Operand>,
    pub condition: Option<Condition>,
    /// Architectural EFLAGS masks, not decoder-private flag numbering.
    pub flags_read: u32,
    pub flags_written: u32,
    pub flags_undefined: u32,
    pub unsupported: Option<UnsupportedReason>,
}

impl Instruction {
    pub fn next_eip(&self) -> u32 {
        self.eip.wrapping_add(u32::from(self.len))
    }

    pub fn ends_block(&self) -> bool {
        matches!(
            self.opcode,
            Opcode::Jcc
                | Opcode::Loop
                | Opcode::Jecxz
                | Opcode::Jmp
                | Opcode::Call
                | Opcode::Ret
                | Opcode::Ud2
                | Opcode::DiagnosticStop
                | Opcode::Rdtsc
                | Opcode::Unsupported
                | Opcode::String(_, true)
        )
    }

    /// Discovery edges include the return continuation of CALL. This is not
    /// permission to replace guest CALL/RET stack effects with host calls.
    pub fn successors(&self) -> Vec<u32> {
        let direct = match self.operands.first() {
            Some(Operand::Imm(target)) => Some(*target),
            _ => None,
        };
        match self.opcode {
            Opcode::Jcc | Opcode::Loop | Opcode::Jecxz | Opcode::Call => {
                let mut result = vec![self.next_eip()];
                if let Some(target) = direct
                    && !result.contains(&target)
                {
                    result.push(target);
                }
                result
            }
            Opcode::Jmp => direct.into_iter().collect(),
            Opcode::Ret | Opcode::Ud2 | Opcode::DiagnosticStop | Opcode::Unsupported => Vec::new(),
            _ => vec![self.next_eip()],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Block {
    pub start: u32,
    pub instructions: Vec<Instruction>,
    /// A target absent from `Region::blocks` is an explicit external exit.
    pub successors: Vec<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Region {
    pub entry: u32,
    pub blocks: BTreeMap<u32, Block>,
    /// Discovery is bounded. Reaching the bound leaves undiscovered targets as
    /// external exits; they must never be interpreted as successful execution.
    pub limit_reached: bool,
}

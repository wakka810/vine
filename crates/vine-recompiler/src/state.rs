//! Architectural interchange state. No v86 layout or compiler scratch is ABI.

pub const EIP_OFFSET: u32 = 32;
pub const EFLAGS_OFFSET: u32 = 36;
pub const DEFINED_FLAGS_OFFSET: u32 = 40;
pub const EXIT_OFFSET: u32 = 400;
pub const STATE_BYTES: usize = 420;
pub const ARITHMETIC_FLAGS: u32 = 0x8d5;

pub mod exit_kind {
    pub const NONE: u32 = 0;
    pub const BUDGET: u32 = 1;
    pub const OUTSIDE_REGION: u32 = 2;
    pub const UNSUPPORTED: u32 = 3;
    pub const PAGE_FAULT: u32 = 4;
    pub const RUNTIME: u32 = 5;
    pub const GENERAL_PROTECTION: u32 = 6;
    pub const CODE_MODIFIED: u32 = 7;
    pub const INVALID_OPCODE: u32 = 8;
    pub const STACK_FAULT: u32 = 9;
    pub const RESOURCE_LIMIT: u32 = 10;
    pub const DIVIDE_ERROR: u32 = 11;
    /// Bounded REP iteration yield, with an incomplete guest instruction.
    pub const REP_PARTIAL_BUDGET: u32 = 12;
    /// An element wrote code (possibly through an alias); REP is incomplete.
    pub const REP_PARTIAL_CODE_MODIFIED: u32 = 13;
    /// A Runtime timestamp sample is needed before this instruction retires.
    /// `address` is its one-use handoff ticket and `error` its decoded length.
    pub const TIMESTAMP_COUNTER: u32 = 15;
    /// Synchronous #XM at the unretired instruction. MXCSR records newly
    /// raised flags; destination registers are unchanged. No error code.
    pub const SIMD_FLOATING_POINT: u32 = 16;
    /// Host diagnostic boundary before attempting the selected guest PC.
    /// No instruction has retired, and this is not a guest exception.
    pub const DIAGNOSTIC_STOP: u32 = 17;
}

/// Segment descriptor state is CPU state; selectors alone do not describe FS.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SegmentState {
    pub selector: u32,
    pub base: u32,
    pub limit: u32,
    pub attributes: u32,
}

impl Default for SegmentState {
    fn default() -> Self {
        Self {
            selector: 0,
            base: 0,
            limit: u32::MAX,
            attributes: 0,
        }
    }
}

/// Physical x87 registers, not logical ST(i): low/high significand and raw
/// sign/exponent. MMX aliases the low 64 bits of these same registers. No f64
/// conversion is permitted at this boundary. TOP is part of `status`.
pub type X87State = vine_x87::State;

/// `fault_eip` identifies the instruction that did not complete. `completed`
/// counts completed guest instructions in this invocation, not IR operations.
/// EIP remains the actual continuation, including partial REP progress when
/// that instruction family is implemented. A fault never restarts the region.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ExitState {
    pub kind: u32,
    pub fault_eip: u32,
    pub address: u32,
    /// Architectural error code for faults. For REP_PARTIAL_* only, the
    /// positive number of completed elements in the current REP invocation.
    /// EIP/ECX/ESI/EDI carry its resumable architectural state; no hidden cursor.
    pub error: u32,
    pub completed: u32,
}

#[repr(C)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CpuState {
    /// EAX, ECX, EDX, EBX, ESP, EBP, ESI, EDI.
    pub gpr: [u32; 8],
    pub eip: u32,
    /// Materialized architectural flags. Undefined flag values are retained;
    /// `defined_flags` describes which are meaningful for semantic comparison.
    pub eflags: u32,
    pub defined_flags: u32,
    /// ES, CS, SS, DS, FS, GS.
    pub segments: [SegmentState; 6],
    pub x87: X87State,
    pub xmm: [[u32; 4]; 8],
    pub mxcsr: u32,
    pub exit: ExitState,
}

impl Default for CpuState {
    fn default() -> Self {
        Self {
            gpr: [0; 8],
            eip: 0,
            eflags: 2,
            defined_flags: u32::MAX,
            segments: [SegmentState::default(); 6],
            x87: X87State::default(),
            xmm: [[0; 4]; 8],
            mxcsr: 0x1f80,
            exit: ExitState::default(),
        }
    }
}

impl CpuState {
    pub fn from_snapshot(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() != STATE_BYTES {
            return Err("invalid architectural snapshot length".into());
        }
        let mut words = [0u32; STATE_BYTES / 4];
        for (word, bytes) in words.iter_mut().zip(bytes.chunks_exact(4)) {
            *word = u32::from_le_bytes(bytes.try_into().expect("four bytes"));
        }
        let mut state = Self::default();
        state.gpr.copy_from_slice(&words[..8]);
        state.eip = words[8];
        state.eflags = words[9];
        state.defined_flags = words[10];
        for (n, segment) in state.segments.iter_mut().enumerate() {
            let at = 11 + n * 4;
            *segment = SegmentState {
                selector: words[at],
                base: words[at + 1],
                limit: words[at + 2],
                attributes: words[at + 3],
            };
        }
        for (n, register) in state.x87.registers.iter_mut().enumerate() {
            register.copy_from_slice(&words[35 + n * 3..38 + n * 3]);
        }
        state.x87.control = words[59];
        state.x87.status = words[60];
        state.x87.tag = words[61];
        state.x87.opcode = words[62];
        state.x87.instruction_pointer = words[63];
        state.x87.instruction_selector = words[64];
        state.x87.data_pointer = words[65];
        state.x87.data_selector = words[66];
        for (n, register) in state.xmm.iter_mut().enumerate() {
            register.copy_from_slice(&words[67 + n * 4..71 + n * 4]);
        }
        state.mxcsr = words[99];
        state.exit = ExitState {
            kind: words[100],
            fault_eip: words[101],
            address: words[102],
            error: words[103],
            completed: words[104],
        };
        Ok(state)
    }
    pub fn thread(entry: u32, stack: u32, fs_base: u32) -> Self {
        let mut state = Self {
            eip: entry,
            ..Self::default()
        };
        state.gpr[4] = stack;
        state.segments[4].base = fs_base;
        for (index, segment) in state.segments.iter_mut().enumerate() {
            segment.attributes = if index == 1 { 0xfa } else { 0xf2 };
        }
        state
    }

    /// Explicit little-endian private snapshot; no host padding or unsafe cast.
    pub fn snapshot(&self) -> [u8; STATE_BYTES] {
        let mut words = [0u32; STATE_BYTES / 4];
        words[..8].copy_from_slice(&self.gpr);
        words[8..11].copy_from_slice(&[self.eip, self.eflags, self.defined_flags]);
        for (n, segment) in self.segments.iter().enumerate() {
            words[11 + n * 4..15 + n * 4].copy_from_slice(&[
                segment.selector,
                segment.base,
                segment.limit,
                segment.attributes,
            ]);
        }
        for (n, register) in self.x87.registers.iter().enumerate() {
            words[35 + n * 3..38 + n * 3].copy_from_slice(register);
        }
        words[59..67].copy_from_slice(&[
            self.x87.control,
            self.x87.status,
            self.x87.tag,
            self.x87.opcode,
            self.x87.instruction_pointer,
            self.x87.instruction_selector,
            self.x87.data_pointer,
            self.x87.data_selector,
        ]);
        for (n, register) in self.xmm.iter().enumerate() {
            words[67 + n * 4..71 + n * 4].copy_from_slice(register);
        }
        words[99] = self.mxcsr;
        words[100..105].copy_from_slice(&[
            self.exit.kind,
            self.exit.fault_eip,
            self.exit.address,
            self.exit.error,
            self.exit.completed,
        ]);
        let mut bytes = [0; STATE_BYTES];
        for (n, word) in words.iter().enumerate() {
            bytes[n * 4..n * 4 + 4].copy_from_slice(&word.to_le_bytes());
        }
        bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn architectural_layout_preserves_extended_precision_and_alias_storage() {
        assert_eq!(std::mem::size_of::<CpuState>(), STATE_BYTES);
        assert_eq!(std::mem::offset_of!(CpuState, eip), EIP_OFFSET as usize);
        assert_eq!(
            std::mem::offset_of!(CpuState, eflags),
            EFLAGS_OFFSET as usize
        );
        assert_eq!(
            std::mem::offset_of!(CpuState, defined_flags),
            DEFINED_FLAGS_OFFSET as usize
        );
        assert_eq!(std::mem::offset_of!(CpuState, exit), EXIT_OFFSET as usize);
        let mut state = CpuState::default();
        state.x87.registers[0] = [1, 0x8000_0000, 0x3fff];
        state.x87.status = 7 << 11;
        state.x87.tag = 0xfffc;
        let saved = state.clone();
        assert_eq!(saved.x87.registers[0], [1, 0x8000_0000, 0x3fff]);
        assert_eq!(saved, state);
    }
}

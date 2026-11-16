//! REP comparison restart metadata. This is backend-private thread state, not
//! an additional architectural register or a guest-memory cursor.
use crate::{
    Backend,
    decode::{CodeSource, decode_one},
    ir::{Condition, Opcode, StringOpcode},
    state::{CpuState, exit_kind},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ComparisonContinuation {
    pub eip: u32,
    pub original_flags: u32,
    pub original_defined: u32,
    pub encoding: [u8; 16],
    pub length: u32,
    pub descriptor: u32,
    /// EAX (SCAS) or ESI (CMPS), ECX, EDI, EFLAGS, defined mask at the last yield.
    pub expected: [u32; 5],
    pub in_flight: bool,
}

pub(crate) fn descriptor(instruction: &crate::ir::Instruction) -> Option<u32> {
    match instruction.opcode {
        Opcode::String(operation @ (StringOpcode::Scas | StringOpcode::Cmps), true) => Some(
            instruction.width.bytes()
                | if operation == StringOpcode::Cmps {
                    0x400
                } else {
                    0
                }
                | if instruction.condition == Some(Condition::Ne) {
                    0x200
                } else {
                    0x100
                },
        ),
        _ => None,
    }
}

fn progress(state: &CpuState, descriptor: u32) -> [u32; 5] {
    [
        state.gpr[if descriptor & 0x400 != 0 { 6 } else { 0 }],
        state.gpr[1],
        state.gpr[7],
        state.eflags,
        state.defined_flags,
    ]
}

impl ComparisonContinuation {
    pub fn validate(&self, state: &CpuState) -> Result<(), String> {
        if self.in_flight
            || !(1..=15).contains(&self.length)
            || self.encoding[self.length as usize..]
                .iter()
                .any(|&byte| byte != 0)
            || self.eip != state.eip
            || state.gpr[1] == 0
            || self.expected != progress(state, self.descriptor)
            || !matches!(
                state.exit.kind,
                exit_kind::REP_PARTIAL_BUDGET | exit_kind::BUDGET | exit_kind::DIAGNOSTIC_STOP
            )
            || (state.exit.kind == exit_kind::REP_PARTIAL_BUDGET
                && !(1..=4096).contains(&state.exit.error))
            || (self.original_flags ^ state.eflags) & !crate::state::ARITHMETIC_FLAGS != 0
            || (self.original_defined | crate::state::ARITHMETIC_FLAGS) != state.defined_flags
        {
            return Err("invalid suspended string comparison context".into());
        }
        let instruction = decode_one(self.eip, &self.encoding[..self.length as usize]);
        if u32::from(instruction.len) != self.length
            || descriptor(&instruction) != Some(self.descriptor)
        {
            return Err("invalid suspended string comparison encoding".into());
        }
        Ok(())
    }
}

impl Backend {
    pub(crate) fn validate_comparison_admission(&self) -> Result<(), String> {
        let Some(scas) = &self.comparison else {
            return Ok(());
        };
        scas.validate(&self.state)?;
        let bytes = self.memory.read_code(scas.eip, scas.length as usize);
        // Missing or NX bytes are left to the ordinary instruction-fetch fault
        // path. A replacement encoding is an explicit unsupported continuation,
        // never permission to apply an old flags recipe to a new instruction.
        if bytes != scas.encoding[..bytes.len()] {
            return Err("unsupported SCAS continuation: executable encoding changed".into());
        }
        Ok(())
    }

    pub(crate) fn finish_suspended_comparison_fetch(&mut self) {
        if let Some(scas) = &self.comparison
            && !scas.in_flight
            && self.state.eip == scas.eip
            && matches!(
                self.state.exit.kind,
                exit_kind::PAGE_FAULT | exit_kind::GENERAL_PROTECTION
            )
        {
            // A mapping can disappear while the thread is parked. A fetch-fault
            // stub has no SCAS imports; finalize that same attempt at the common
            // outer boundary before Runtime receives the architectural fault.
            self.state.eflags = scas.original_flags;
            self.state.defined_flags = scas.original_defined;
            self.comparison = None;
        }
    }
    #[expect(
        clippy::too_many_arguments,
        reason = "bounded Wasm instruction boundary transports current locals"
    )]
    pub(crate) fn begin_comparison(
        &mut self,
        unit: u32,
        eip: u32,
        flags: u32,
        defined: u32,
        source: u32,
        ecx: u32,
        edi: u32,
        expected_descriptor: u32,
    ) -> Result<(), String> {
        let bytes = self.memory.read_code(eip, 15);
        let instruction = decode_one(eip, &bytes);
        if descriptor(&instruction) != Some(expected_descriptor) || ecx == 0 {
            return Err("SCAS begin does not match a repeated comparison".into());
        }
        let length = u32::from(instruction.len);
        self.cache
            .check_code_span(unit, eip, length, &self.memory)?;
        let mut encoding = [0; 16];
        encoding[..length as usize].copy_from_slice(&bytes[..length as usize]);
        let expected = [source, ecx, edi, flags, defined];
        if let Some(prior) = &self.comparison {
            if prior.in_flight
                || prior.eip != eip
                || prior.encoding != encoding
                || prior.length != length
                || prior.descriptor != expected_descriptor
                || prior.expected != expected
            {
                return Err("SCAS continuation encoding or progress changed".into());
            }
        } else {
            self.comparison = Some(ComparisonContinuation {
                eip,
                original_flags: flags,
                original_defined: defined,
                encoding,
                length,
                descriptor: expected_descriptor,
                expected,
                in_flight: false,
            });
        }
        self.comparison
            .as_mut()
            .expect("created continuation")
            .in_flight = true;
        Ok(())
    }

    /// Called after final CPU publication, before any host sees the outcome.
    pub(crate) fn finish_comparison(&mut self) -> Result<(), String> {
        let Some(continuation) = self.comparison.as_mut().filter(|c| c.in_flight) else {
            return Ok(());
        };
        if self.state.eip != continuation.eip {
            return Err("SCAS exit does not name its current instruction".into());
        }
        if self.state.exit.kind == exit_kind::REP_PARTIAL_BUDGET && self.state.exit.error > 0 {
            let mut suspended = continuation.clone();
            suspended.expected = progress(&self.state, continuation.descriptor);
            suspended.in_flight = false;
            suspended.validate(&self.state)?;
            *continuation = suspended;
        } else if matches!(
            self.state.exit.kind,
            exit_kind::PAGE_FAULT | exit_kind::GENERAL_PROTECTION | exit_kind::STACK_FAULT
        ) {
            // Intel REP(E/NE) exception restart restores the flags from before
            // the instruction; ECX/EDI and CMPS ESI retain completed progress.
            self.state.eflags = continuation.original_flags;
            self.state.defined_flags = continuation.original_defined;
            self.comparison = None;
        } else {
            return Err("unexpected SCAS partial exit".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        memory::{EXECUTE, PAGE_SIZE, READ, SEALED},
        snapshot,
    };
    fn backend() -> Backend {
        let mut b = Backend::new(1).unwrap();
        let mut code = [0x90; PAGE_SIZE];
        code[..2].copy_from_slice(&[0xf2, 0xae]);
        b.memory
            .map_page(0x1000, 1, READ | EXECUTE | SEALED, &code)
            .unwrap();
        b.compile(0x1000, 1).unwrap();
        b.state.eip = 0x1000;
        b.state.eflags = 0x246;
        b.state.defined_flags = !crate::state::ARITHMETIC_FLAGS;
        b.state.gpr[0] = 7;
        b.state.gpr[1] = 9000;
        b.state.gpr[7] = 0x2000;
        b
    }
    fn begin(b: &mut Backend) -> Result<(), String> {
        b.begin_comparison(
            b.cache.selected_id(),
            b.state.eip,
            b.state.eflags,
            b.state.defined_flags,
            b.state.gpr[0],
            b.state.gpr[1],
            b.state.gpr[7],
            0x201,
        )
    }
    #[test]
    fn cmps_snapshot_checks_source_progress_and_retains_flags_across_backend_replacement() {
        let mut b = backend();
        let mut code = [0x90; PAGE_SIZE];
        code[..3].copy_from_slice(&[0x66, 0xf3, 0xa7]);
        b.memory
            .reset_page(0x1000, 1, READ | EXECUTE | SEALED, &code)
            .unwrap();
        b.compile(0x1000, 1).unwrap();
        b.state.gpr[6] = 0x6000;
        b.begin_comparison(
            b.cache.selected_id(),
            0x1000,
            b.state.eflags,
            b.state.defined_flags,
            0x6000,
            9000,
            0x2000,
            0x502,
        )
        .unwrap();
        let original = [b.state.eflags, b.state.defined_flags];
        b.state.gpr[1] -= 4096;
        b.state.gpr[6] += 8192;
        b.state.gpr[7] += 8192;
        b.state.eflags = 0x246;
        b.state.defined_flags = u32::MAX;
        b.state.exit.kind = exit_kind::REP_PARTIAL_BUDGET;
        b.state.exit.error = 4096;
        b.finish_comparison().unwrap();
        let bytes = snapshot::encode(&b.state, b.comparison.as_ref()).unwrap();
        let mut altered = bytes;
        altered[24] ^= 1; // ESI cannot change independently of saved progress.
        assert!(snapshot::decode(&altered).is_err());
        let mut other = backend();
        other
            .memory
            .reset_page(0x1000, 1, READ | EXECUTE | SEALED, &code)
            .unwrap();
        other.compile(0x1000, 1).unwrap();
        let (state, continuation) = snapshot::decode(&bytes).unwrap();
        *other.state = state;
        other.comparison = continuation;
        other
            .begin_comparison(
                other.cache.selected_id(),
                0x1000,
                other.state.eflags,
                other.state.defined_flags,
                0x8000,
                4904,
                0x4000,
                0x502,
            )
            .unwrap();
        other.state.exit.kind = exit_kind::PAGE_FAULT;
        other.finish_comparison().unwrap();
        assert_eq!([other.state.eflags, other.state.defined_flags], original);
        assert_eq!(
            [other.state.gpr[1], other.state.gpr[6], other.state.gpr[7]],
            [4904, 0x8000, 0x4000]
        );
        assert!(other.comparison.is_none());
    }

    #[test]
    fn partial_snapshot_transfers_original_flags_to_equivalent_new_backend() {
        let mut b = backend();
        let original = [b.state.eflags, b.state.defined_flags];
        begin(&mut b).unwrap();
        b.state.gpr[1] -= 4096;
        b.state.gpr[7] += 4096;
        b.state.eflags = 0x293;
        b.state.defined_flags = u32::MAX;
        b.state.exit.kind = exit_kind::REP_PARTIAL_BUDGET;
        b.state.exit.error = 4096;
        b.finish_comparison().unwrap();
        let bytes = snapshot::encode(&b.state, b.comparison.as_ref()).unwrap();
        let mut other = backend();
        let (state, continuation) = snapshot::decode(&bytes).unwrap();
        *other.state = state;
        other.comparison = continuation;
        begin(&mut other).unwrap();
        other.state.gpr[1] -= 3;
        other.state.gpr[7] += 3;
        other.state.exit.kind = exit_kind::PAGE_FAULT;
        other.state.exit.error = 4;
        other.finish_comparison().unwrap();
        assert_eq!([other.state.eflags, other.state.defined_flags], original);
        assert_eq!(other.state.gpr[1], 4901);
        assert_eq!(other.state.gpr[7], 0x3003);
        assert!(other.comparison.is_none());
        // A delivered fault ends the old attempt: handler flags become the
        // original flags of an independently restarted instruction.
        other.state.eflags = 0x602;
        begin(&mut other).unwrap();
        assert_eq!(other.comparison.as_ref().unwrap().original_flags, 0x602);
    }
    #[test]
    fn changed_encoding_or_progress_never_reuses_an_original_flags_recipe() {
        let mut b = backend();
        begin(&mut b).unwrap();
        b.state.gpr[1] -= 1;
        b.state.gpr[7] += 1;
        b.state.defined_flags = u32::MAX;
        b.state.exit.kind = exit_kind::REP_PARTIAL_BUDGET;
        b.state.exit.error = 1;
        b.finish_comparison().unwrap();
        let prior = b.comparison.clone();
        b.state.gpr[0] ^= 1;
        assert!(begin(&mut b).is_err());
        assert_eq!(b.comparison, prior);
        b.state.gpr[0] ^= 1;
        let mut code = [0x90; PAGE_SIZE];
        code[..2].copy_from_slice(&[0xf3, 0xae]);
        b.memory
            .reset_page(0x1000, 1, READ | EXECUTE | SEALED, &code)
            .unwrap();
        b.compile(0x1000, 1).unwrap();
        assert!(
            b.begin_comparison(
                b.cache.selected_id(),
                b.state.eip,
                b.state.eflags,
                b.state.defined_flags,
                b.state.gpr[0],
                b.state.gpr[1],
                b.state.gpr[7],
                0x101
            )
            .is_err()
        );
        assert_eq!(b.comparison, prior);
    }
    #[test]
    fn parked_fetch_fault_and_zero_budget_preserve_the_same_attempt() {
        let mut b = backend();
        let flags = b.state.eflags;
        let defined = b.state.defined_flags;
        begin(&mut b).unwrap();
        b.state.gpr[1] -= 1;
        b.state.gpr[7] += 1;
        b.state.eflags = 0x293;
        b.state.defined_flags = u32::MAX;
        b.state.exit.kind = exit_kind::REP_PARTIAL_BUDGET;
        b.state.exit.error = 1;
        b.finish_comparison().unwrap();
        b.state.exit.kind = exit_kind::BUDGET;
        b.state.exit.error = 0;
        assert!(snapshot::encode(&b.state, b.comparison.as_ref()).is_ok());
        b.memory.protect_page(0x1000, 1, READ | SEALED).unwrap();
        b.validate_comparison_admission().unwrap();
        b.state.exit.kind = exit_kind::PAGE_FAULT;
        b.state.exit.error = 21;
        b.finish_suspended_comparison_fetch();
        assert_eq!([b.state.eflags, b.state.defined_flags], [flags, defined]);
        assert!(b.comparison.is_none());
        assert_eq!(b.state.gpr[1], 8999);
    }
}

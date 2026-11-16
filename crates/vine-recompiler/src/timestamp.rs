//! One-use CPU/Runtime handoff for RDTSC. There is no clock in this backend.
//! Intel SDM Vol.2B, RDTSC: full EDX:EAX output, flags unchanged, LOCK #UD.
//! https://cdrdv2.intel.com/v1/dl/getContent/671241
use crate::{
    Backend,
    decode::{self, CodeSource},
    ir::Opcode,
    state::{CpuState, ExitState, exit_kind},
};

pub(crate) struct PendingTimestamp {
    unit_id: u32,
    state: CpuState,
}

impl Backend {
    fn check_timestamp_code(&self, unit_id: u32, eip: u32, length: u32) -> Result<(), String> {
        self.validate_cpu()?;
        self.cache
            .check_code_span(unit_id, eip, length, &self.memory)?;
        let instruction = decode::decode_one(eip, &self.memory.read_code(eip, 15));
        if instruction.opcode != Opcode::Rdtsc || u32::from(instruction.len) != length {
            return Err("timestamp instruction changed during CPU completion".into());
        }
        Ok(())
    }
    pub(crate) fn begin_timestamp(&mut self, unit_id: u32) -> Result<u32, String> {
        let exit = self.state.exit;
        if self.pending_timestamp.is_some()
            || exit.kind != exit_kind::TIMESTAMP_COUNTER
            || exit.address != 0
            || exit.fault_eip != self.state.eip
            || !(1..=15).contains(&exit.error)
        {
            return Err("invalid or already pending timestamp handoff".into());
        }
        self.check_timestamp_code(unit_id, self.state.eip, exit.error)?;
        exit.completed
            .checked_add(1)
            .ok_or("timestamp retirement counter overflow")?;
        let ticket = self
            .timestamp_next
            .checked_add(1)
            .ok_or("timestamp ticket exhaustion")?;
        let mut snapshot = (*self.state).clone();
        snapshot.exit.address = ticket;
        // All fallible checks precede publication of the one-use identity.
        self.timestamp_next = ticket;
        self.state.exit.address = ticket;
        self.pending_timestamp = Some(PendingTimestamp {
            unit_id,
            state: snapshot,
        });
        Ok(ticket)
    }
    pub(crate) fn complete_timestamp(
        &mut self,
        expected_eip: u32,
        low: u32,
        high: u32,
    ) -> Result<(), String> {
        let pending = self
            .pending_timestamp
            .as_ref()
            .ok_or("no pending timestamp handoff")?;
        if self.state.eip != expected_eip || *self.state != pending.state {
            return Err("timestamp completion CPU context changed".into());
        }
        self.check_timestamp_code(pending.unit_id, expected_eip, self.state.exit.error)?;
        let completed = self
            .state
            .exit
            .completed
            .checked_add(1)
            .ok_or("timestamp retirement counter overflow")?;
        let next = expected_eip.wrapping_add(self.state.exit.error);
        // Runtime sampling failure never calls this operation. Failed checks
        // retain CPU state and the ticket for diagnostics or an explicit retry.
        self.state.gpr[0] = low;
        self.state.gpr[2] = high;
        self.state.eip = next;
        self.state.exit = ExitState {
            completed,
            ..Default::default()
        };
        self.pending_timestamp = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::{EXECUTE, PAGE_SIZE, READ, SEALED};

    fn pending(eip: u32, bytes: &[u8]) -> (Backend, u32) {
        let mut backend = Backend::new(4).unwrap();
        let mut page = [0x90; PAGE_SIZE];
        let offset = (eip & 4095) as usize;
        page[offset..offset + bytes.len()].copy_from_slice(bytes);
        backend
            .memory
            .map_page(eip & !4095, 1, READ | EXECUTE | SEALED, &page)
            .unwrap();
        backend.compile(eip, 1).unwrap();
        backend.state.eip = eip;
        backend.state.gpr = [11, 22, 33, 44, 55, 66, 77, 88];
        backend.state.eflags = 0xad7;
        backend.state.exit = ExitState {
            kind: exit_kind::TIMESTAMP_COUNTER,
            fault_eip: eip,
            address: 0,
            error: bytes.len() as u32,
            completed: 7,
        };
        let id = backend.cache.selected_id();
        let ticket = backend.begin_timestamp(id).unwrap();
        (backend, ticket)
    }
    #[test]
    fn timestamp_completes_full_pair_once_without_flags_or_other_register_changes() {
        let (mut backend, ticket) = pending(0x1000, &[0x66, 0x0f, 0x31]);
        assert_ne!(ticket, 0);
        let old = (*backend.state).clone();
        assert!(backend.complete_timestamp(0x1001, 1, 2).is_err());
        assert_eq!(*backend.state, old);
        backend
            .complete_timestamp(0x1000, 0x89abcdef, 0x12345678)
            .unwrap();
        assert_eq!(
            backend.state.gpr,
            [0x89abcdef, 22, 0x12345678, 44, 55, 66, 77, 88]
        );
        assert_eq!(
            (
                backend.state.eip,
                backend.state.eflags,
                backend.state.exit.completed
            ),
            (0x1003, old.eflags, 8)
        );
        assert_eq!(backend.state.exit.kind, 0);
        let complete = (*backend.state).clone();
        assert!(backend.complete_timestamp(0x1000, 3, 4).is_err());
        assert_eq!(*backend.state, complete);
        *backend.state = old.clone();
        assert!(backend.complete_timestamp(0x1000, 3, 4).is_err());
        assert_eq!(*backend.state, old);
    }
    #[test]
    fn timestamp_rejects_changed_context_code_owner_and_cancelled_ticket_atomically() {
        let (mut backend, _) = pending(0x2000, &[0x0f, 0x31]);
        let original = (*backend.state).clone();
        backend.state.gpr[1] += 1;
        let changed = (*backend.state).clone();
        assert!(backend.complete_timestamp(0x2000, 1, 2).is_err());
        assert_eq!(*backend.state, changed);
        *backend.state = original;
        backend.memory.unmap_page(0x2000).unwrap();
        let mut page = [0x90; PAGE_SIZE];
        page[..2].copy_from_slice(&[0x0f, 0x31]);
        backend
            .memory
            .map_page(0x2000, 2, READ | EXECUTE | SEALED, &page)
            .unwrap();
        let before = (*backend.state).clone();
        assert!(backend.complete_timestamp(0x2000, 1, 2).is_err());
        assert_eq!(*backend.state, before);
        assert!(backend.pending_timestamp.is_some());
        backend.pending_timestamp = None;
        assert!(backend.complete_timestamp(0x2000, 1, 2).is_err());
        assert_eq!(*backend.state, before);
    }
    #[test]
    fn timestamp_next_eip_wraps_and_exhausted_tickets_do_not_publish() {
        let (mut backend, _) = pending(0xffff_fffe, &[0x0f, 0x31]);
        backend.complete_timestamp(0xffff_fffe, 1, 2).unwrap();
        assert_eq!(backend.state.eip, 0);
        backend.state.eip = 0xffff_fffe;
        backend.state.exit = ExitState {
            kind: 15,
            fault_eip: 0xffff_fffe,
            error: 2,
            ..Default::default()
        };
        backend.timestamp_next = u32::MAX;
        let before = (*backend.state).clone();
        assert!(
            backend
                .begin_timestamp(backend.cache.selected_id())
                .is_err()
        );
        assert_eq!(*backend.state, before);
        assert!(backend.pending_timestamp.is_none());
    }
    #[test]
    fn timestamp_changed_instruction_and_invalid_begin_preserve_cpu() {
        let (mut backend, _) = pending(0x1000, &[0x0f, 0x31]);
        let before = (*backend.state).clone();
        backend
            .memory
            .reset_page(0x1000, 1, READ | EXECUTE | SEALED, &[0x90; PAGE_SIZE])
            .unwrap();
        assert!(backend.complete_timestamp(0x1000, 1, 2).is_err());
        assert_eq!(*backend.state, before);
        backend.pending_timestamp = None;
        backend.state.exit.address = 0;
        let before = (*backend.state).clone();
        assert!(
            backend
                .begin_timestamp(backend.cache.selected_id())
                .is_err()
        );
        assert_eq!(*backend.state, before);
        assert!(backend.pending_timestamp.is_none());
    }
}

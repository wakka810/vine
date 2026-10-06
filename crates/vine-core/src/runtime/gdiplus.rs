use crate::gdiplus::{GDIPLUS_STATUS_OK, GDIPLUS_STATUS_OUT_OF_MEMORY};
use std::collections::BTreeSet;

use super::RuntimeStateError;
use crate::memory::{GuestAddress, GuestMemory};

pub const RUNTIME_GDIPLUS_SESSION_TOKEN_PREFIX: u32 = 0x4750_0000;
pub const RUNTIME_GDIPLUS_SESSION_LIMIT: usize = 256;

#[derive(Debug)]
pub(super) struct RuntimeGdiplus {
    sessions: BTreeSet<u32>,
    next_token_id: u16,
}

impl Default for RuntimeGdiplus {
    fn default() -> Self {
        Self {
            sessions: BTreeSet::new(),
            next_token_id: 1,
        }
    }
}

impl RuntimeGdiplus {
    pub(super) fn startup(
        &mut self,
        memory: &mut GuestMemory,
        output: GuestAddress,
    ) -> Result<u32, RuntimeStateError> {
        if self.sessions.len() >= RUNTIME_GDIPLUS_SESSION_LIMIT || self.next_token_id == 0 {
            return Ok(GDIPLUS_STATUS_OUT_OF_MEMORY);
        }
        let token = RUNTIME_GDIPLUS_SESSION_TOKEN_PREFIX | u32::from(self.next_token_id);
        if self.sessions.contains(&token) {
            return Err(RuntimeStateError::new(
                "GDI+ allocator selected a live token",
            ));
        }
        let next_token_id = self.next_token_id.checked_add(1).unwrap_or(0);
        // The memory transaction handles guest-write failure before changing token ownership.
        let mut transaction = memory.begin_transaction();
        transaction
            .write(output, &token.to_le_bytes())
            .map_err(RuntimeStateError::from_memory)?;
        self.sessions.insert(token);
        self.next_token_id = next_token_id;
        transaction.commit_transaction();
        Ok(GDIPLUS_STATUS_OK)
    }

    pub(super) fn shutdown(&mut self, token: u32) -> Result<(), RuntimeStateError> {
        if token & 0xffff == 0 || token & 0xffff_0000 != RUNTIME_GDIPLUS_SESSION_TOKEN_PREFIX {
            return Err(RuntimeStateError::new("invalid GDI+ token"));
        }
        if !self.sessions.remove(&token) {
            return Err(RuntimeStateError::new("GDI+ token is not live"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::{MemoryLimits, MemoryOwnerKind, MemoryRegionKind, PageProtection};
    const OUTPUT: GuestAddress = GuestAddress(0x1000);

    fn memory() -> GuestMemory {
        let mut memory = GuestMemory::new(MemoryLimits {
            max_physical_pages: 1,
            max_regions: 1,
            max_owners: 1,
            max_access_bytes: 0x1000,
        })
        .unwrap();
        let owner = memory.create_owner(MemoryOwnerKind::Runtime).unwrap();
        let range = memory
            .reserve_fixed(owner, MemoryRegionKind::Runtime, OUTPUT, 0x1000)
            .unwrap();
        memory
            .commit(
                owner,
                range,
                PageProtection {
                    read: true,
                    write: true,
                    execute: false,
                },
            )
            .unwrap();
        memory.seal_owner(owner).unwrap();
        let reflection = memory.pending_reflection().unwrap();
        memory
            .acknowledge_delta(reflection.invalidation.generation)
            .unwrap();
        memory
    }

    fn startup_token(gdiplus: &mut RuntimeGdiplus, memory: &mut GuestMemory) -> u32 {
        assert_eq!(gdiplus.startup(memory, OUTPUT).unwrap(), GDIPLUS_STATUS_OK);
        let delta = memory.pending_delta().unwrap();
        memory.acknowledge_delta(delta.generation).unwrap();
        u32::from_le_bytes(memory.read(OUTPUT, 4).unwrap().try_into().unwrap())
    }

    #[test]
    fn distinct_tokens_retire_exact_sessions_without_reuse() {
        let mut gdiplus = RuntimeGdiplus::default();
        let mut memory = memory();
        let first = startup_token(&mut gdiplus, &mut memory);
        let second = startup_token(&mut gdiplus, &mut memory);
        assert_ne!(first, second);
        gdiplus.shutdown(first).unwrap();
        assert!(gdiplus.shutdown(first).is_err());
        assert!(gdiplus.shutdown(1).is_err());
        assert_eq!(gdiplus.next_token_id, 3);
        assert_eq!(gdiplus.sessions.len(), 1);
        assert!(gdiplus.sessions.contains(&second));
        let third = startup_token(&mut gdiplus, &mut memory);
        assert!(third > second);
    }

    #[test]
    fn invalid_guest_output_preserves_session_and_memory() {
        let mut gdiplus = RuntimeGdiplus::default();
        let mut memory = memory();
        let memory_before = memory.stats();
        assert!(gdiplus.startup(&mut memory, GuestAddress(0x1ffe)).is_err());
        assert!(gdiplus.sessions.is_empty());
        assert_eq!(gdiplus.next_token_id, 1);
        assert_eq!(memory.stats(), memory_before);
        assert_eq!(memory.read(OUTPUT, 4).unwrap(), [0; 4]);
        let session = startup_token(&mut gdiplus, &mut memory);
        let memory_before = memory.stats();
        assert!(gdiplus.startup(&mut memory, GuestAddress(0x1ffe)).is_err());
        assert_eq!(gdiplus.sessions.len(), 1);
        assert!(gdiplus.sessions.contains(&session));
        assert_eq!(gdiplus.next_token_id, 2);
        assert_eq!(memory.stats(), memory_before);
        assert_eq!(memory.read(OUTPUT, 4).unwrap(), session.to_le_bytes());
    }

    #[test]
    fn capacity_failure_preserves_guest_output_and_allocator() {
        let mut gdiplus = RuntimeGdiplus::default();
        let mut memory = memory();
        for _ in 0..RUNTIME_GDIPLUS_SESSION_LIMIT {
            startup_token(&mut gdiplus, &mut memory);
        }
        let memory_before = memory.stats();
        let output = memory.read(OUTPUT, 4).unwrap();
        assert_eq!(
            gdiplus.startup(&mut memory, OUTPUT).unwrap(),
            GDIPLUS_STATUS_OUT_OF_MEMORY
        );
        assert_eq!(gdiplus.sessions.len(), RUNTIME_GDIPLUS_SESSION_LIMIT);
        assert!((1..=RUNTIME_GDIPLUS_SESSION_LIMIT as u32).all(|id| {
            gdiplus
                .sessions
                .contains(&(RUNTIME_GDIPLUS_SESSION_TOKEN_PREFIX | id))
        }));
        assert_eq!(
            gdiplus.next_token_id,
            RUNTIME_GDIPLUS_SESSION_LIMIT as u16 + 1
        );
        assert_eq!(memory.stats(), memory_before);
        assert_eq!(memory.read(OUTPUT, 4).unwrap(), output);
        gdiplus
            .shutdown(RUNTIME_GDIPLUS_SESSION_TOKEN_PREFIX | 1)
            .unwrap();
        let repaired = startup_token(&mut gdiplus, &mut memory);
        assert_eq!(repaired, RUNTIME_GDIPLUS_SESSION_TOKEN_PREFIX | 257);
        assert_eq!(memory.read(OUTPUT, 4).unwrap(), repaired.to_le_bytes());
    }
}

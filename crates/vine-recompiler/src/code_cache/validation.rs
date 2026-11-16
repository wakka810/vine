//! Successful code validation belongs to the resident unit, not an entry PC.
//! Reuse requires the SAME synchronous invocation's executable-code proof.
//! An unchanged epoch or empty journal alone cannot authorize an outer entry:
//! admission also checks independent page dirty flags, including all aliases.
use super::Unit;
use crate::memory::MirrorMemory;
use std::cell::Cell;
use std::ops::Deref;

#[derive(Clone, Copy)]
struct Validation {
    epoch: u64,
    invocation: u64,
    executable: bool,
}

pub(super) struct CachedUnit {
    pub unit: Unit,
    validation: Cell<Option<Validation>>,
}

impl Deref for CachedUnit {
    type Target = Unit;
    fn deref(&self) -> &Unit {
        &self.unit
    }
}

impl CachedUnit {
    pub fn new(unit: Unit) -> Self {
        Self {
            unit,
            validation: Cell::new(None),
        }
    }

    pub fn is_current(&self, memory: &MirrorMemory) -> bool {
        self.is_current_for(memory, 0)
    }

    pub fn is_current_for(&self, memory: &MirrorMemory, invocation: u64) -> bool {
        let epoch = memory.epoch();
        if let Some(proof) = self.validation.get()
            && invocation != 0
            && proof.epoch == epoch
            && proof.invocation == invocation
            && proof.executable
        {
            return true;
        }
        if !self.unit.is_current(memory) {
            self.validation.set(None);
            return false;
        }
        self.validation.set(Some(Validation {
            epoch,
            invocation,
            executable: self.unit.permits_invocation_proof(memory),
        }));
        true
    }

    /// Called immediately after successful selection/admission, before any
    /// guest execution. Only all-executable (or absent) dependencies can retain
    /// a proof while generated data stores await the outer finish operation.
    pub fn certify_invocation(&self, epoch: u64, invocation: u64) -> bool {
        let Some(mut proof) = self.validation.get() else {
            return false;
        };
        if proof.epoch != epoch || !proof.executable || invocation == 0 {
            return false;
        }
        proof.invocation = invocation;
        self.validation.set(Some(proof));
        true
    }
}

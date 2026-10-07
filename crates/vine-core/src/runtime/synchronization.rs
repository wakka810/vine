use std::collections::{BTreeMap, VecDeque};

use crate::memory::GuestAddress;

use super::{RUNTIME_GUEST_THREAD_LIMIT, RuntimeStateError, RuntimeThreadId};

pub const RUNTIME_CRITICAL_SECTION_LIMIT: usize = 32_768;
pub const RUNTIME_MUTEX_LIMIT: usize = 4096;
pub const X86_CRITICAL_SECTION_BYTE_LENGTH: usize = 24;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeCriticalSection {
    pub address: GuestAddress,
    pub spin_count: u32,
    pub owning_thread_id: Option<RuntimeThreadId>,
    pub recursion_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct RuntimeMutexId(pub u64);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeMutexNameA {
    pub bytes_with_nul: Vec<u8>,
    pub utf16_units_with_nul: Vec<u16>,
}

impl RuntimeMutexNameA {
    pub fn new(
        bytes_with_nul: Vec<u8>,
        utf16_units_with_nul: Vec<u16>,
    ) -> Result<Self, RuntimeStateError> {
        if bytes_with_nul.len() < 2
            || bytes_with_nul.last() != Some(&0)
            || bytes_with_nul[..bytes_with_nul.len() - 1].contains(&0)
            || utf16_units_with_nul.len() < 2
            || utf16_units_with_nul.last() != Some(&0)
            || utf16_units_with_nul[..utf16_units_with_nul.len() - 1].contains(&0)
        {
            return Err(RuntimeStateError::new(
                "named mutex identity must be nonempty and singly NUL-terminated",
            ));
        }
        Ok(Self {
            bytes_with_nul,
            utf16_units_with_nul,
        })
    }

    fn identity(&self) -> Vec<u16> {
        self.utf16_units_with_nul[..self.utf16_units_with_nul.len() - 1].to_vec()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeMutex {
    pub id: RuntimeMutexId,
    pub name_a: Option<RuntimeMutexNameA>,
    pub owning_thread_id: Option<RuntimeThreadId>,
    pub recursion_count: u32,
    pub abandoned: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeSynchronization {
    critical_sections: BTreeMap<GuestAddress, RuntimeCriticalSection>,
    critical_section_waiters: BTreeMap<GuestAddress, VecDeque<RuntimeThreadId>>,
    mutexes: BTreeMap<RuntimeMutexId, RuntimeMutex>,
    named_mutexes: BTreeMap<Vec<u16>, RuntimeMutexId>,
    next_mutex_id: u64,
}

impl Default for RuntimeSynchronization {
    fn default() -> Self {
        Self {
            critical_sections: BTreeMap::new(),
            critical_section_waiters: BTreeMap::new(),
            mutexes: BTreeMap::new(),
            named_mutexes: BTreeMap::new(),
            next_mutex_id: 1,
        }
    }
}

impl RuntimeSynchronization {
    pub(super) fn initialize(
        &mut self,
        address: GuestAddress,
        spin_count: u32,
    ) -> Result<&RuntimeCriticalSection, RuntimeStateError> {
        if address.0 == 0 || !address.0.is_multiple_of(4) {
            return Err(RuntimeStateError::new(
                "critical section storage must be a nonzero 4-byte-aligned guest address",
            ));
        }
        if self.critical_sections.contains_key(&address) {
            return Err(RuntimeStateError::new(format!(
                "critical section storage {:#010x} is already initialized",
                address.0
            )));
        }
        if self.critical_sections.len() >= RUNTIME_CRITICAL_SECTION_LIMIT {
            return Err(RuntimeStateError::new(
                "process critical section count exceeds the runtime bound",
            ));
        }
        Ok(self
            .critical_sections
            .entry(address)
            .or_insert(RuntimeCriticalSection {
                address,
                spin_count,
                owning_thread_id: None,
                recursion_count: 0,
            }))
    }

    pub(super) fn enter(
        &mut self,
        address: GuestAddress,
        current_thread_id: RuntimeThreadId,
    ) -> Result<&RuntimeCriticalSection, RuntimeStateError> {
        let mut next = *self.critical_section(address)?;
        match next.owning_thread_id {
            None if next.recursion_count == 0 => {
                next.owning_thread_id = Some(current_thread_id);
                next.recursion_count = 1;
            }
            Some(owner) if owner == current_thread_id && next.recursion_count != 0 => {
                next.recursion_count = next.recursion_count.checked_add(1).ok_or_else(|| {
                    RuntimeStateError::new("critical section recursion count overflowed")
                })?;
            }
            Some(owner) if owner != current_thread_id && next.recursion_count != 0 => {
                return Err(RuntimeStateError::new(format!(
                    "critical section {:#010x} is owned by thread {}",
                    address.0, owner.0
                )));
            }
            _ => {
                return Err(RuntimeStateError::new(
                    "critical section owner and recursion count disagree",
                ));
            }
        }
        let section = self
            .critical_sections
            .get_mut(&address)
            .expect("validated critical section");
        *section = next;
        Ok(section)
    }

    pub(super) fn validate_wait(
        &self,
        address: GuestAddress,
        current_thread_id: RuntimeThreadId,
    ) -> Result<(), RuntimeStateError> {
        let previous = *self.critical_section(address)?;
        let Some(owner) = previous.owning_thread_id else {
            return Err(RuntimeStateError::new(
                "critical section wait requires a live owner",
            ));
        };
        if owner == current_thread_id || previous.recursion_count == 0 {
            return Err(RuntimeStateError::new(
                "critical section wait requires a different owning thread",
            ));
        }
        if self
            .critical_section_waiters
            .values()
            .any(|waiters| waiters.contains(&current_thread_id))
        {
            return Err(RuntimeStateError::new(
                "runtime thread is already waiting for a critical section",
            ));
        }
        if self
            .critical_section_waiters
            .get(&address)
            .is_some_and(|waiters| waiters.len() >= RUNTIME_GUEST_THREAD_LIMIT)
        {
            return Err(RuntimeStateError::new(
                "critical section waiter count exceeds the runtime thread bound",
            ));
        }
        Ok(())
    }

    // The aggregate validates this queue and the scheduler before either changes.
    pub(super) fn enqueue_waiter(&mut self, address: GuestAddress, thread_id: RuntimeThreadId) {
        self.critical_section_waiters
            .entry(address)
            .or_default()
            .push_back(thread_id);
    }

    pub(super) fn validate_leave(
        &self,
        address: GuestAddress,
        current_thread_id: RuntimeThreadId,
    ) -> Result<Option<RuntimeThreadId>, RuntimeStateError> {
        let previous = *self.critical_section(address)?;
        if previous.owning_thread_id != Some(current_thread_id) || previous.recursion_count == 0 {
            return Err(RuntimeStateError::new(format!(
                "critical section {:#010x} is not owned by current thread {}",
                address.0, current_thread_id.0
            )));
        }
        Ok(if previous.recursion_count == 1 {
            self.critical_section_waiters
                .get(&address)
                .and_then(|waiters| waiters.front())
                .copied()
        } else {
            None
        })
    }

    pub(super) fn delete(
        &mut self,
        address: GuestAddress,
    ) -> Result<RuntimeCriticalSection, RuntimeStateError> {
        let previous = *self.critical_section(address)?;
        if previous.owning_thread_id.is_some() || previous.recursion_count != 0 {
            return Err(RuntimeStateError::new(format!(
                "critical section {:#010x} cannot be deleted while owned",
                address.0
            )));
        }
        if self
            .critical_section_waiters
            .get(&address)
            .is_some_and(|waiters| !waiters.is_empty())
        {
            return Err(RuntimeStateError::new(format!(
                "critical section {:#010x} cannot be deleted while threads are waiting",
                address.0
            )));
        }
        self.critical_sections.remove(&address);
        self.critical_section_waiters.remove(&address);
        Ok(previous)
    }

    pub(super) fn leave(
        &mut self,
        address: GuestAddress,
        thread_id: RuntimeThreadId,
    ) -> Result<&RuntimeCriticalSection, RuntimeStateError> {
        let awakened = self.validate_leave(address, thread_id)?;
        let section = self
            .critical_sections
            .get_mut(&address)
            .expect("validated critical section");
        section.recursion_count -= 1;
        if section.recursion_count == 0 {
            section.owning_thread_id = awakened;
            if awakened.is_some() {
                section.recursion_count = 1;
                let waiters = self
                    .critical_section_waiters
                    .get_mut(&address)
                    .expect("validated waiter");
                waiters.pop_front();
                if waiters.is_empty() {
                    self.critical_section_waiters.remove(&address);
                }
            }
        }
        Ok(section)
    }

    #[cfg(test)]
    pub(super) fn critical_section_waiters(&self, address: GuestAddress) -> Vec<RuntimeThreadId> {
        self.critical_section_waiters
            .get(&address)
            .map(|waiters| waiters.iter().copied().collect())
            .unwrap_or_default()
    }

    pub(super) fn critical_section(
        &self,
        address: GuestAddress,
    ) -> Result<&RuntimeCriticalSection, RuntimeStateError> {
        self.critical_sections.get(&address).ok_or_else(|| {
            RuntimeStateError::new(format!(
                "critical section storage {:#010x} is not initialized",
                address.0
            ))
        })
    }

    pub(super) fn validate_mutex_creation(
        &self,
        name_a: Option<&RuntimeMutexNameA>,
    ) -> Result<(RuntimeMutexId, bool), RuntimeStateError> {
        let name_identity = name_a.map(RuntimeMutexNameA::identity);
        if let Some(id) = name_identity
            .as_ref()
            .and_then(|identity| self.named_mutexes.get(identity))
            .copied()
        {
            self.mutexes.get(&id).ok_or_else(|| {
                RuntimeStateError::new(
                    "named mutex index references a missing synchronization object",
                )
            })?;
            return Ok((id, true));
        }
        if self.mutexes.len() >= RUNTIME_MUTEX_LIMIT {
            return Err(RuntimeStateError::new(
                "process mutex count exceeds the runtime bound",
            ));
        }
        let id = RuntimeMutexId(self.next_mutex_id);
        self.next_mutex_id
            .checked_add(1)
            .ok_or_else(|| RuntimeStateError::new("mutex ID space is exhausted"))?;
        if self.mutexes.contains_key(&id) {
            return Err(RuntimeStateError::new(
                "mutex allocator selected a live identity",
            ));
        }
        Ok((id, false))
    }

    // The aggregate validates this allocation and the object table before publication.
    pub(super) fn insert_mutex(
        &mut self,
        id: RuntimeMutexId,
        name_a: Option<RuntimeMutexNameA>,
        initial_owner: bool,
        current_thread_id: RuntimeThreadId,
    ) {
        self.next_mutex_id += 1;
        if let Some(name) = name_a.as_ref() {
            self.named_mutexes.insert(name.identity(), id);
        }
        self.mutexes.insert(
            id,
            RuntimeMutex {
                id,
                name_a,
                owning_thread_id: initial_owner.then_some(current_thread_id),
                recursion_count: u32::from(initial_owner),
                abandoned: false,
            },
        );
    }

    pub(super) fn acquire_mutex(
        &mut self,
        id: RuntimeMutexId,
        current_thread_id: RuntimeThreadId,
    ) -> Result<(u32, bool), RuntimeStateError> {
        self.mutex(id)?;
        let next = self.mutexes.get_mut(&id).expect("validated mutex identity");
        let abandoned_acquisition = next.abandoned;
        match next.owning_thread_id {
            None if next.recursion_count == 0 => {
                next.owning_thread_id = Some(current_thread_id);
                next.recursion_count = 1;
                next.abandoned = false;
            }
            Some(owner) if owner == current_thread_id && next.recursion_count != 0 => {
                next.recursion_count = next
                    .recursion_count
                    .checked_add(1)
                    .ok_or_else(|| RuntimeStateError::new("mutex recursion count overflowed"))?;
            }
            Some(owner) if owner != current_thread_id && next.recursion_count != 0 => {
                return Err(RuntimeStateError::new(format!(
                    "mutex {} is owned by thread {}; blocking contention is not enabled",
                    id.0, owner.0
                )));
            }
            _ => {
                return Err(RuntimeStateError::new(
                    "mutex owner and recursion count disagree",
                ));
            }
        }
        Ok((next.recursion_count, abandoned_acquisition))
    }

    #[cfg(test)]
    pub(super) fn release_mutex(
        &mut self,
        id: RuntimeMutexId,
        current_thread_id: RuntimeThreadId,
    ) -> Result<&RuntimeMutex, RuntimeStateError> {
        let previous = self.mutex(id)?;
        if previous.owning_thread_id != Some(current_thread_id) || previous.recursion_count == 0 {
            return Err(RuntimeStateError::new(format!(
                "mutex {} is not owned by current thread {}",
                id.0, current_thread_id.0
            )));
        }
        let next = self.mutexes.get_mut(&id).expect("validated mutex identity");
        next.recursion_count -= 1;
        if next.recursion_count == 0 {
            next.owning_thread_id = None;
        }
        Ok(next)
    }

    #[cfg(test)]
    pub(super) fn mutex_ids(&self) -> Vec<RuntimeMutexId> {
        self.mutexes.keys().copied().collect()
    }

    pub(super) fn validate_mutex_retirement(
        &self,
        id: RuntimeMutexId,
    ) -> Result<(), RuntimeStateError> {
        let mutex = self.mutex(id)?;
        if let Some(name) = mutex.name_a.as_ref()
            && self.named_mutexes.get(&name.identity()) != Some(&id)
        {
            return Err(RuntimeStateError::new(
                "named mutex index does not identify its live mutex",
            ));
        }
        Ok(())
    }

    // Called only after validating all owners, with no intervening synchronization mutation.
    pub(super) fn retire_mutex(&mut self, id: RuntimeMutexId) {
        let removed = self
            .mutexes
            .remove(&id)
            .expect("validated mutex retirement");
        if let Some(name) = removed.name_a {
            self.named_mutexes.remove(&name.identity());
        }
    }

    pub(super) fn mutex(&self, id: RuntimeMutexId) -> Result<&RuntimeMutex, RuntimeStateError> {
        self.mutexes
            .get(&id)
            .ok_or_else(|| RuntimeStateError::new(format!("mutex {} is not registered", id.0)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ADDRESS: GuestAddress = GuestAddress(0x1000_4000);
    const OWNER: RuntimeThreadId = RuntimeThreadId(0x2000);
    const CONTENDER: RuntimeThreadId = RuntimeThreadId(0x3000);

    #[test]
    fn direct_updates_preserve_recursive_identity() {
        let mut synchronization = RuntimeSynchronization::default();

        assert_eq!(
            *synchronization.initialize(ADDRESS, 4000).unwrap(),
            RuntimeCriticalSection {
                address: ADDRESS,
                spin_count: 4000,
                owning_thread_id: None,
                recursion_count: 0,
            }
        );

        synchronization.enter(ADDRESS, OWNER).unwrap();
        synchronization.enter(ADDRESS, OWNER).unwrap();
        assert_eq!(
            synchronization.critical_section(ADDRESS).unwrap(),
            &RuntimeCriticalSection {
                address: ADDRESS,
                spin_count: 4000,
                owning_thread_id: Some(OWNER),
                recursion_count: 2,
            }
        );

        synchronization.leave(ADDRESS, OWNER).unwrap();
        synchronization.leave(ADDRESS, OWNER).unwrap();
        assert_eq!(
            synchronization.critical_section(ADDRESS).unwrap(),
            &RuntimeCriticalSection {
                address: ADDRESS,
                spin_count: 4000,
                owning_thread_id: None,
                recursion_count: 0,
            }
        );
    }

    #[test]
    fn rejected_operations_preserve_all_synchronization_state() {
        let mut synchronization = RuntimeSynchronization::default();
        synchronization.initialize(ADDRESS, 4000).unwrap();
        synchronization.enter(ADDRESS, OWNER).unwrap();
        let before = synchronization.clone();

        synchronization.initialize(ADDRESS, 0).unwrap_err();
        synchronization.enter(ADDRESS, CONTENDER).unwrap_err();
        synchronization.leave(ADDRESS, CONTENDER).unwrap_err();
        synchronization.delete(ADDRESS).unwrap_err();
        assert_eq!(synchronization, before);
    }

    #[test]
    fn deletion_rejects_missing_storage_and_allows_reinitialization() {
        let mut synchronization = RuntimeSynchronization::default();
        synchronization.initialize(ADDRESS, 4000).unwrap();
        assert_eq!(
            synchronization.delete(ADDRESS).unwrap(),
            RuntimeCriticalSection {
                address: ADDRESS,
                spin_count: 4000,
                owning_thread_id: None,
                recursion_count: 0,
            }
        );
        let before_duplicate = synchronization.clone();
        assert!(synchronization.delete(ADDRESS).is_err());
        assert_eq!(synchronization, before_duplicate);
        synchronization.initialize(ADDRESS, 1234).unwrap();
        assert_eq!(
            synchronization
                .critical_section(ADDRESS)
                .unwrap()
                .spin_count,
            1234
        );
    }

    #[test]
    fn waiting_is_fifo_and_only_final_recursive_leave_transfers_ownership() {
        let mut synchronization = RuntimeSynchronization::default();
        let second_address = GuestAddress(ADDRESS.0 + 24);
        let third = RuntimeThreadId(0x4000);
        synchronization.initialize(ADDRESS, 0).unwrap();
        synchronization.initialize(second_address, 0).unwrap();
        synchronization.enter(ADDRESS, OWNER).unwrap();
        synchronization.enter(ADDRESS, OWNER).unwrap();
        synchronization.enter(second_address, OWNER).unwrap();
        for thread in [CONTENDER, third] {
            synchronization.validate_wait(ADDRESS, thread).unwrap();
            synchronization.enqueue_waiter(ADDRESS, thread);
        }
        let before_rejections = synchronization.clone();
        assert!(synchronization.validate_wait(ADDRESS, OWNER).is_err());
        assert!(
            synchronization
                .validate_wait(second_address, CONTENDER)
                .is_err()
        );
        assert!(synchronization.delete(ADDRESS).is_err());
        assert!(synchronization.leave(ADDRESS, CONTENDER).is_err());
        assert_eq!(synchronization, before_rejections);

        assert_eq!(
            synchronization.validate_leave(ADDRESS, OWNER).unwrap(),
            None
        );
        synchronization.leave(ADDRESS, OWNER).unwrap();
        assert_eq!(
            synchronization.critical_section_waiters(ADDRESS),
            [CONTENDER, third]
        );
        assert_eq!(
            synchronization.validate_leave(ADDRESS, OWNER).unwrap(),
            Some(CONTENDER)
        );
        let transferred = synchronization.leave(ADDRESS, OWNER).unwrap();
        assert_eq!(transferred.owning_thread_id, Some(CONTENDER));
        assert_eq!(transferred.recursion_count, 1);
        assert_eq!(synchronization.critical_section_waiters(ADDRESS), [third]);
        synchronization.leave(ADDRESS, CONTENDER).unwrap();
        assert!(synchronization.critical_section_waiters(ADDRESS).is_empty());
        assert_eq!(
            synchronization
                .critical_section(ADDRESS)
                .unwrap()
                .owning_thread_id,
            Some(third)
        );
        synchronization.leave(ADDRESS, third).unwrap();
        synchronization.delete(ADDRESS).unwrap();
        assert_eq!(
            synchronization
                .critical_section(second_address)
                .unwrap()
                .owning_thread_id,
            Some(OWNER)
        );
    }

    #[test]
    fn bad_addresses_and_recursion_overflow_do_not_publish_changes() {
        let mut synchronization = RuntimeSynchronization::default();
        let before = synchronization.clone();
        for address in [GuestAddress(0), GuestAddress(ADDRESS.0 + 1)] {
            assert!(synchronization.initialize(address, 0).is_err());
            assert_eq!(synchronization, before);
        }
        synchronization.initialize(ADDRESS, 0).unwrap();
        synchronization.enter(ADDRESS, OWNER).unwrap();
        synchronization
            .critical_sections
            .get_mut(&ADDRESS)
            .unwrap()
            .recursion_count = u32::MAX;
        let before_overflow = synchronization.clone();
        assert!(synchronization.enter(ADDRESS, OWNER).is_err());
        assert_eq!(synchronization, before_overflow);
    }

    #[test]
    fn critical_section_capacity_is_bounded_and_failure_atomic() {
        let mut synchronization = RuntimeSynchronization::default();
        for index in 0..RUNTIME_CRITICAL_SECTION_LIMIT {
            let offset = u32::try_from(index)
                .unwrap()
                .checked_mul(u32::try_from(X86_CRITICAL_SECTION_BYTE_LENGTH).unwrap())
                .unwrap();
            let address = GuestAddress(0x2000_0000_u32.checked_add(offset).unwrap());
            synchronization.initialize(address, 0).unwrap();
        }
        assert_eq!(
            synchronization.critical_sections.len(),
            RUNTIME_CRITICAL_SECTION_LIMIT
        );

        let before = synchronization.clone();
        let overflow_address = GuestAddress(
            0x2000_0000_u32
                .checked_add(
                    u32::try_from(RUNTIME_CRITICAL_SECTION_LIMIT)
                        .unwrap()
                        .checked_mul(u32::try_from(X86_CRITICAL_SECTION_BYTE_LENGTH).unwrap())
                        .unwrap(),
                )
                .unwrap(),
        );
        synchronization.initialize(overflow_address, 0).unwrap_err();
        assert_eq!(synchronization, before);
    }

    #[test]
    fn direct_mutex_acquisition_preserves_abandonment_recursion_and_failure_atomicity() {
        let mut synchronization = RuntimeSynchronization::default();
        let id = RuntimeMutexId(1);
        synchronization.mutexes.insert(
            id,
            RuntimeMutex {
                id,
                name_a: None,
                owning_thread_id: None,
                recursion_count: 0,
                abandoned: true,
            },
        );
        assert_eq!(synchronization.acquire_mutex(id, OWNER).unwrap(), (1, true));
        assert_eq!(
            synchronization.acquire_mutex(id, OWNER).unwrap(),
            (2, false)
        );
        let before_wrong_owner = synchronization.clone();
        assert!(synchronization.acquire_mutex(id, CONTENDER).is_err());
        assert!(synchronization.release_mutex(id, CONTENDER).is_err());
        assert!(
            synchronization
                .acquire_mutex(RuntimeMutexId(2), OWNER)
                .is_err()
        );
        assert!(
            synchronization
                .release_mutex(RuntimeMutexId(2), OWNER)
                .is_err()
        );
        assert_eq!(synchronization, before_wrong_owner);
        assert_eq!(
            synchronization
                .release_mutex(id, OWNER)
                .unwrap()
                .recursion_count,
            1
        );
        let released = synchronization.release_mutex(id, OWNER).unwrap();
        assert_eq!(released.recursion_count, 0);
        assert_eq!(released.owning_thread_id, None);
        assert_eq!(
            synchronization.acquire_mutex(id, CONTENDER).unwrap(),
            (1, false)
        );

        synchronization
            .mutexes
            .get_mut(&id)
            .unwrap()
            .recursion_count = u32::MAX;
        let before_overflow = synchronization.clone();
        assert!(synchronization.acquire_mutex(id, CONTENDER).is_err());
        assert_eq!(synchronization, before_overflow);
        synchronization
            .mutexes
            .get_mut(&id)
            .unwrap()
            .owning_thread_id = None;
        let before_corruption = synchronization.clone();
        assert!(synchronization.acquire_mutex(id, OWNER).is_err());
        assert!(synchronization.release_mutex(id, OWNER).is_err());
        assert_eq!(synchronization, before_corruption);
    }

    #[test]
    fn mutex_creation_checks_capacity_allocator_and_identity_before_publication() {
        let mut synchronization = RuntimeSynchronization::default();
        for _ in 0..RUNTIME_MUTEX_LIMIT {
            let (id, existing) = synchronization.validate_mutex_creation(None).unwrap();
            assert!(!existing);
            synchronization.insert_mutex(id, None, false, OWNER);
        }
        let before_capacity = synchronization.clone();
        assert!(synchronization.validate_mutex_creation(None).is_err());
        assert_eq!(synchronization, before_capacity);

        let mut exhausted = RuntimeSynchronization {
            next_mutex_id: u64::MAX,
            ..Default::default()
        };
        let before_exhaustion = exhausted.clone();
        assert!(exhausted.validate_mutex_creation(None).is_err());
        assert_eq!(exhausted, before_exhaustion);
        exhausted.next_mutex_id = 1;
        exhausted.insert_mutex(RuntimeMutexId(1), None, false, OWNER);
        exhausted.next_mutex_id = 1;
        let before_collision = exhausted.clone();
        assert!(exhausted.validate_mutex_creation(None).is_err());
        assert_eq!(exhausted, before_collision);
    }

    #[test]
    fn named_mutex_retirement_checks_index_and_allows_recreation() {
        let mut synchronization = RuntimeSynchronization::default();
        let name = RuntimeMutexNameA::new(
            b"Vine mutex\0".to_vec(),
            "Vine mutex\0".encode_utf16().collect(),
        )
        .unwrap();
        let (id, _) = synchronization
            .validate_mutex_creation(Some(&name))
            .unwrap();
        synchronization.insert_mutex(id, Some(name.clone()), false, OWNER);
        synchronization
            .named_mutexes
            .insert(name.identity(), RuntimeMutexId(999));
        let before_mismatch = synchronization.clone();
        assert!(synchronization.validate_mutex_retirement(id).is_err());
        assert!(
            synchronization
                .validate_mutex_creation(Some(&name))
                .is_err()
        );
        assert_eq!(synchronization, before_mismatch);
        synchronization.named_mutexes.insert(name.identity(), id);
        synchronization.validate_mutex_retirement(id).unwrap();
        synchronization.retire_mutex(id);
        assert!(synchronization.named_mutexes.is_empty());
        assert!(synchronization.mutexes.is_empty());
        assert!(synchronization.validate_mutex_retirement(id).is_err());
        let (new_id, existing) = synchronization
            .validate_mutex_creation(Some(&name))
            .unwrap();
        assert_ne!(new_id, id);
        assert!(!existing);
    }

    #[test]
    fn mutex_creation_copies_names_preserves_existing_owner_and_rejects_contention() {
        let mut synchronization = RuntimeSynchronization::default();
        let name = RuntimeMutexNameA::new(
            b"Vine mutex\0".to_vec(),
            "Vine mutex\0".encode_utf16().collect(),
        )
        .unwrap();
        let before = synchronization.clone();
        let (id, already_exists) = synchronization
            .validate_mutex_creation(Some(&name))
            .unwrap();
        assert_eq!(synchronization, before);
        assert!(!already_exists);
        synchronization.insert_mutex(id, Some(name.clone()), true, OWNER);

        assert_eq!(
            synchronization
                .validate_mutex_creation(Some(&name))
                .unwrap(),
            (id, true)
        );
        let mutex = synchronization.mutex(id).unwrap();
        assert_eq!(mutex.owning_thread_id, Some(OWNER));
        assert_eq!(mutex.recursion_count, 1);

        let before_contention = synchronization.clone();
        synchronization.acquire_mutex(id, CONTENDER).unwrap_err();
        assert_eq!(synchronization, before_contention);
    }
}

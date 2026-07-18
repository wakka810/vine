use std::collections::BTreeMap;

use serde::Serialize;

use crate::memory::GuestAddress;

use super::{
    RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING, RuntimeHandle, RuntimeStateError, RuntimeThreadId,
};

pub const RUNTIME_CRITICAL_SECTION_LIMIT: usize = 32_768;
pub const RUNTIME_MUTEX_LIMIT: usize = 4096;
pub const X86_CRITICAL_SECTION_BYTE_LENGTH: usize = 24;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeCriticalSection {
    pub address: GuestAddress,
    pub spin_count: u32,
    pub owning_thread_id: Option<RuntimeThreadId>,
    pub recursion_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct RuntimeMutexId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeCreateMutexAResult {
    pub mutex_id: RuntimeMutexId,
    pub handle: RuntimeHandle,
    pub already_exists: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeMutexNameA {
    pub bytes_with_nul: Vec<u8>,
    pub utf16_units_with_nul: Vec<u16>,
    pub encoding: String,
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
            return Err(RuntimeStateError::invalid_input(
                "runtime-mutex",
                "mutex-name-invalid",
                "named mutex identity must be nonempty and singly NUL-terminated",
            ));
        }
        Ok(Self {
            bytes_with_nul,
            utf16_units_with_nul,
            encoding: RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING.to_owned(),
        })
    }

    fn identity(&self) -> Vec<u16> {
        self.utf16_units_with_nul[..self.utf16_units_with_nul.len() - 1].to_vec()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeMutex {
    pub id: RuntimeMutexId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name_a: Option<RuntimeMutexNameA>,
    pub owning_thread_id: Option<RuntimeThreadId>,
    pub recursion_count: u32,
    pub abandoned: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RuntimeSynchronization {
    critical_sections: BTreeMap<GuestAddress, RuntimeCriticalSection>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    mutexes: BTreeMap<RuntimeMutexId, RuntimeMutex>,
    #[serde(skip)]
    named_mutexes: BTreeMap<Vec<u16>, RuntimeMutexId>,
    #[serde(skip)]
    next_mutex_id: u64,
}

impl Default for RuntimeSynchronization {
    fn default() -> Self {
        Self {
            critical_sections: BTreeMap::new(),
            mutexes: BTreeMap::new(),
            named_mutexes: BTreeMap::new(),
            next_mutex_id: 1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RuntimeCriticalSectionPlan {
    address: GuestAddress,
    previous: Option<RuntimeCriticalSection>,
    next: RuntimeCriticalSection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeMutexCreatePlan {
    id: RuntimeMutexId,
    previous: Option<RuntimeMutex>,
    next: RuntimeMutex,
    name_identity: Option<Vec<u16>>,
    next_mutex_id: u64,
    already_exists: bool,
}

impl RuntimeMutexCreatePlan {
    pub(super) const fn id(&self) -> RuntimeMutexId {
        self.id
    }

    pub(super) const fn already_exists(&self) -> bool {
        self.already_exists
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeMutexTransitionPlan {
    id: RuntimeMutexId,
    previous: RuntimeMutex,
    next: RuntimeMutex,
    abandoned_acquisition: bool,
}

impl RuntimeMutexTransitionPlan {
    pub(super) const fn recursion_count(&self) -> u32 {
        self.next.recursion_count
    }

    pub(super) const fn abandoned_acquisition(&self) -> bool {
        self.abandoned_acquisition
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeMutexRetirementPlan {
    id: RuntimeMutexId,
    previous: RuntimeMutex,
}

impl RuntimeSynchronization {
    pub(super) fn plan_initialization(
        &self,
        address: GuestAddress,
        spin_count: u32,
    ) -> Result<RuntimeCriticalSectionPlan, RuntimeStateError> {
        if address.0 == 0 || !address.0.is_multiple_of(4) {
            return Err(RuntimeStateError::invalid_input(
                "runtime-critical-section",
                "critical-section-address-invalid",
                "critical section storage must be a nonzero 4-byte-aligned guest address",
            ));
        }
        if self.critical_sections.contains_key(&address) {
            return Err(RuntimeStateError::conflict(
                "runtime-critical-section",
                "critical-section-already-initialized",
                format!(
                    "critical section storage {:#010x} is already initialized",
                    address.0
                ),
            ));
        }
        if self.critical_sections.len() >= RUNTIME_CRITICAL_SECTION_LIMIT {
            return Err(RuntimeStateError::resource_limit(
                "runtime-critical-section",
                "critical-section-limit-exceeded",
                "process critical section count exceeds the runtime bound",
            ));
        }
        Ok(RuntimeCriticalSectionPlan {
            address,
            previous: None,
            next: RuntimeCriticalSection {
                address,
                spin_count,
                owning_thread_id: None,
                recursion_count: 0,
            },
        })
    }

    pub(super) fn plan_enter(
        &self,
        address: GuestAddress,
        current_thread_id: RuntimeThreadId,
    ) -> Result<RuntimeCriticalSectionPlan, RuntimeStateError> {
        let previous = *self.critical_section(address)?;
        let mut next = previous;
        match next.owning_thread_id {
            None if next.recursion_count == 0 => {
                next.owning_thread_id = Some(current_thread_id);
                next.recursion_count = 1;
            }
            Some(owner) if owner == current_thread_id && next.recursion_count != 0 => {
                next.recursion_count = next.recursion_count.checked_add(1).ok_or_else(|| {
                    RuntimeStateError::resource_limit(
                        "runtime-critical-section",
                        "critical-section-recursion-overflow",
                        "critical section recursion count overflowed",
                    )
                })?;
            }
            Some(owner) if owner != current_thread_id && next.recursion_count != 0 => {
                return Err(RuntimeStateError::conflict(
                    "runtime-critical-section",
                    "critical-section-contended",
                    format!(
                        "critical section {:#010x} is owned by thread {}",
                        address.0, owner.0
                    ),
                ));
            }
            _ => {
                return Err(RuntimeStateError::runtime_defect(
                    "runtime-critical-section",
                    "critical-section-owner-recursion-inconsistent",
                    "critical section owner and recursion count disagree",
                ));
            }
        }
        Ok(RuntimeCriticalSectionPlan {
            address,
            previous: Some(previous),
            next,
        })
    }

    pub(super) fn plan_leave(
        &self,
        address: GuestAddress,
        current_thread_id: RuntimeThreadId,
    ) -> Result<RuntimeCriticalSectionPlan, RuntimeStateError> {
        let previous = *self.critical_section(address)?;
        if previous.owning_thread_id != Some(current_thread_id) || previous.recursion_count == 0 {
            return Err(RuntimeStateError::conflict(
                "runtime-critical-section",
                "critical-section-not-owned-by-current-thread",
                format!(
                    "critical section {:#010x} is not owned by current thread {}",
                    address.0, current_thread_id.0
                ),
            ));
        }
        let mut next = previous;
        next.recursion_count -= 1;
        if next.recursion_count == 0 {
            next.owning_thread_id = None;
        }
        Ok(RuntimeCriticalSectionPlan {
            address,
            previous: Some(previous),
            next,
        })
    }

    pub(super) fn commit(&mut self, plan: RuntimeCriticalSectionPlan) -> &RuntimeCriticalSection {
        debug_assert_eq!(
            self.critical_sections.get(&plan.address).copied(),
            plan.previous,
            "critical section plan was not committed against its preflight state"
        );
        let previous = self.critical_sections.insert(plan.address, plan.next);
        debug_assert_eq!(previous, plan.previous);
        self.critical_sections
            .get(&plan.address)
            .expect("committed critical section is missing")
    }

    pub(super) fn critical_section(
        &self,
        address: GuestAddress,
    ) -> Result<&RuntimeCriticalSection, RuntimeStateError> {
        self.critical_sections.get(&address).ok_or_else(|| {
            RuntimeStateError::not_found(
                "runtime-critical-section",
                "critical-section-uninitialized",
                format!(
                    "critical section storage {:#010x} is not initialized",
                    address.0
                ),
            )
        })
    }

    pub(super) fn plan_mutex_creation(
        &self,
        name_a: Option<RuntimeMutexNameA>,
        initial_owner: bool,
        current_thread_id: RuntimeThreadId,
    ) -> Result<RuntimeMutexCreatePlan, RuntimeStateError> {
        let name_identity = name_a.as_ref().map(RuntimeMutexNameA::identity);
        if let Some(id) = name_identity
            .as_ref()
            .and_then(|identity| self.named_mutexes.get(identity))
            .copied()
        {
            let previous = self.mutexes.get(&id).cloned().ok_or_else(|| {
                RuntimeStateError::runtime_defect(
                    "runtime-mutex",
                    "named-mutex-index-dangling",
                    "named mutex index references a missing synchronization object",
                )
            })?;
            return Ok(RuntimeMutexCreatePlan {
                id,
                previous: Some(previous.clone()),
                next: previous,
                name_identity,
                next_mutex_id: self.next_mutex_id,
                already_exists: true,
            });
        }
        if self.mutexes.len() >= RUNTIME_MUTEX_LIMIT {
            return Err(RuntimeStateError::resource_limit(
                "runtime-mutex",
                "mutex-limit-exceeded",
                "process mutex count exceeds the runtime bound",
            ));
        }
        let id = RuntimeMutexId(self.next_mutex_id);
        let next_mutex_id = self.next_mutex_id.checked_add(1).ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-mutex",
                "mutex-id-space-exhausted",
                "mutex ID space is exhausted",
            )
        })?;
        if self.mutexes.contains_key(&id) {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-mutex",
                "mutex-id-collision",
                "mutex allocator selected a live identity",
            ));
        }
        let next = RuntimeMutex {
            id,
            name_a,
            owning_thread_id: initial_owner.then_some(current_thread_id),
            recursion_count: u32::from(initial_owner),
            abandoned: false,
        };
        Ok(RuntimeMutexCreatePlan {
            id,
            previous: None,
            next,
            name_identity,
            next_mutex_id,
            already_exists: false,
        })
    }

    pub(super) fn commit_mutex_creation(&mut self, plan: RuntimeMutexCreatePlan) -> &RuntimeMutex {
        debug_assert_eq!(self.mutexes.get(&plan.id), plan.previous.as_ref());
        if plan.previous.is_none() {
            self.next_mutex_id = plan.next_mutex_id;
            if let Some(identity) = plan.name_identity {
                let previous = self.named_mutexes.insert(identity, plan.id);
                debug_assert_eq!(previous, None);
            }
        }
        self.mutexes.insert(plan.id, plan.next);
        self.mutexes
            .get(&plan.id)
            .expect("committed mutex is missing")
    }

    pub(super) fn plan_mutex_wait(
        &self,
        id: RuntimeMutexId,
        current_thread_id: RuntimeThreadId,
    ) -> Result<RuntimeMutexTransitionPlan, RuntimeStateError> {
        let previous = self.mutex(id)?.clone();
        let mut next = previous.clone();
        let abandoned_acquisition = next.abandoned;
        match next.owning_thread_id {
            None if next.recursion_count == 0 => {
                next.owning_thread_id = Some(current_thread_id);
                next.recursion_count = 1;
                next.abandoned = false;
            }
            Some(owner) if owner == current_thread_id && next.recursion_count != 0 => {
                next.recursion_count = next.recursion_count.checked_add(1).ok_or_else(|| {
                    RuntimeStateError::resource_limit(
                        "runtime-mutex",
                        "mutex-recursion-overflow",
                        "mutex recursion count overflowed",
                    )
                })?;
            }
            Some(owner) if owner != current_thread_id && next.recursion_count != 0 => {
                return Err(RuntimeStateError::conflict(
                    "runtime-mutex",
                    "mutex-contended-unsupported",
                    format!(
                        "mutex {} is owned by thread {}; blocking contention is not enabled",
                        id.0, owner.0
                    ),
                ));
            }
            _ => {
                return Err(RuntimeStateError::runtime_defect(
                    "runtime-mutex",
                    "mutex-owner-recursion-inconsistent",
                    "mutex owner and recursion count disagree",
                ));
            }
        }
        Ok(RuntimeMutexTransitionPlan {
            id,
            previous,
            next,
            abandoned_acquisition,
        })
    }

    pub(super) fn plan_mutex_release(
        &self,
        id: RuntimeMutexId,
        current_thread_id: RuntimeThreadId,
    ) -> Result<RuntimeMutexTransitionPlan, RuntimeStateError> {
        let previous = self.mutex(id)?.clone();
        if previous.owning_thread_id != Some(current_thread_id) || previous.recursion_count == 0 {
            return Err(RuntimeStateError::conflict(
                "runtime-mutex",
                "mutex-not-owned-by-current-thread",
                format!(
                    "mutex {} is not owned by current thread {}",
                    id.0, current_thread_id.0
                ),
            ));
        }
        let mut next = previous.clone();
        next.recursion_count -= 1;
        if next.recursion_count == 0 {
            next.owning_thread_id = None;
        }
        Ok(RuntimeMutexTransitionPlan {
            id,
            previous,
            next,
            abandoned_acquisition: false,
        })
    }

    pub(super) fn commit_mutex_transition(
        &mut self,
        plan: RuntimeMutexTransitionPlan,
    ) -> &RuntimeMutex {
        debug_assert_eq!(self.mutexes.get(&plan.id), Some(&plan.previous));
        self.mutexes.insert(plan.id, plan.next);
        self.mutexes
            .get(&plan.id)
            .expect("transitioned mutex is missing")
    }

    pub(super) fn plan_mutex_retirement(
        &self,
        id: RuntimeMutexId,
    ) -> Result<RuntimeMutexRetirementPlan, RuntimeStateError> {
        Ok(RuntimeMutexRetirementPlan {
            id,
            previous: self.mutex(id)?.clone(),
        })
    }

    pub(super) fn commit_mutex_retirement(&mut self, plan: RuntimeMutexRetirementPlan) {
        debug_assert_eq!(self.mutexes.get(&plan.id), Some(&plan.previous));
        let removed = self.mutexes.remove(&plan.id);
        debug_assert_eq!(removed, Some(plan.previous.clone()));
        if let Some(name) = plan.previous.name_a {
            let removed_id = self.named_mutexes.remove(&name.identity());
            debug_assert_eq!(removed_id, Some(plan.id));
        }
    }

    pub(super) fn mutex(&self, id: RuntimeMutexId) -> Result<&RuntimeMutex, RuntimeStateError> {
        self.mutexes.get(&id).ok_or_else(|| {
            RuntimeStateError::not_found(
                "runtime-mutex",
                "mutex-unknown",
                format!("mutex {} is not registered", id.0),
            )
        })
    }

    pub(super) fn mutex_ids(&self) -> Vec<RuntimeMutexId> {
        self.mutexes.keys().copied().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ADDRESS: GuestAddress = GuestAddress(0x1000_4000);
    const OWNER: RuntimeThreadId = RuntimeThreadId(0x2000);
    const CONTENDER: RuntimeThreadId = RuntimeThreadId(0x3000);

    #[test]
    fn plans_do_not_mutate_and_commits_preserve_recursive_identity() {
        let mut synchronization = RuntimeSynchronization::default();

        let before_initialization = synchronization.clone();
        let initialization = synchronization.plan_initialization(ADDRESS, 4000).unwrap();
        assert_eq!(synchronization, before_initialization);
        assert_eq!(
            *synchronization.commit(initialization),
            RuntimeCriticalSection {
                address: ADDRESS,
                spin_count: 4000,
                owning_thread_id: None,
                recursion_count: 0,
            }
        );

        let before_enter = synchronization.clone();
        let enter = synchronization.plan_enter(ADDRESS, OWNER).unwrap();
        assert_eq!(synchronization, before_enter);
        synchronization.commit(enter);
        synchronization.commit(synchronization.plan_enter(ADDRESS, OWNER).unwrap());
        assert_eq!(
            synchronization.critical_section(ADDRESS).unwrap(),
            &RuntimeCriticalSection {
                address: ADDRESS,
                spin_count: 4000,
                owning_thread_id: Some(OWNER),
                recursion_count: 2,
            }
        );

        synchronization.commit(synchronization.plan_leave(ADDRESS, OWNER).unwrap());
        synchronization.commit(synchronization.plan_leave(ADDRESS, OWNER).unwrap());
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
    fn rejected_plans_preserve_all_synchronization_state() {
        let mut synchronization = RuntimeSynchronization::default();
        let initialization = synchronization.plan_initialization(ADDRESS, 4000).unwrap();
        synchronization.commit(initialization);
        let enter = synchronization.plan_enter(ADDRESS, OWNER).unwrap();
        synchronization.commit(enter);
        let before = synchronization.clone();

        assert_eq!(
            synchronization
                .plan_initialization(ADDRESS, 0)
                .unwrap_err()
                .code,
            "critical-section-already-initialized"
        );
        assert_eq!(
            synchronization
                .plan_enter(ADDRESS, CONTENDER)
                .unwrap_err()
                .code,
            "critical-section-contended"
        );
        assert_eq!(
            synchronization
                .plan_leave(ADDRESS, CONTENDER)
                .unwrap_err()
                .code,
            "critical-section-not-owned-by-current-thread"
        );
        assert_eq!(synchronization, before);
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
            let plan = synchronization.plan_initialization(address, 0).unwrap();
            synchronization.commit(plan);
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
        assert_eq!(
            synchronization
                .plan_initialization(overflow_address, 0)
                .unwrap_err()
                .code,
            "critical-section-limit-exceeded"
        );
        assert_eq!(synchronization, before);
    }

    #[test]
    fn mutex_plans_copy_names_preserve_existing_owner_and_reject_contention() {
        let mut synchronization = RuntimeSynchronization::default();
        let name = RuntimeMutexNameA::new(
            b"Vine mutex\0".to_vec(),
            "Vine mutex\0".encode_utf16().collect(),
        )
        .unwrap();
        let before = synchronization.clone();
        let creation = synchronization
            .plan_mutex_creation(Some(name.clone()), true, OWNER)
            .unwrap();
        assert_eq!(synchronization, before);
        assert!(!creation.already_exists());
        let id = creation.id();
        synchronization.commit_mutex_creation(creation);

        let duplicate = synchronization
            .plan_mutex_creation(Some(name), true, CONTENDER)
            .unwrap();
        assert!(duplicate.already_exists());
        synchronization.commit_mutex_creation(duplicate);
        let mutex = synchronization.mutex(id).unwrap();
        assert_eq!(mutex.owning_thread_id, Some(OWNER));
        assert_eq!(mutex.recursion_count, 1);

        let before_contention = synchronization.clone();
        assert_eq!(
            synchronization
                .plan_mutex_wait(id, CONTENDER)
                .unwrap_err()
                .code,
            "mutex-contended-unsupported"
        );
        assert_eq!(synchronization, before_contention);
    }
}

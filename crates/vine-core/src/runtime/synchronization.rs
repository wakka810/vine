use std::collections::BTreeMap;

use serde::Serialize;

use crate::memory::GuestAddress;

use super::{RuntimeStateError, RuntimeThreadId};

pub const RUNTIME_CRITICAL_SECTION_LIMIT: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeCriticalSection {
    pub address: GuestAddress,
    pub spin_count: u32,
    pub owning_thread_id: Option<RuntimeThreadId>,
    pub recursion_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub(super) struct RuntimeSynchronization {
    critical_sections: BTreeMap<GuestAddress, RuntimeCriticalSection>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RuntimeCriticalSectionPlan {
    address: GuestAddress,
    previous: Option<RuntimeCriticalSection>,
    next: RuntimeCriticalSection,
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
}

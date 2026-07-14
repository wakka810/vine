use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde::Serialize;

use super::{
    RuntimeObjectId, RuntimeStateError, RuntimeThreadId, RuntimeWaitCompletion,
    RuntimeWaitRegistration, RuntimeWaitRegistrationId, RuntimeWaitState,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub(super) struct RuntimeWaits {
    wait_registrations: BTreeMap<RuntimeWaitRegistrationId, RuntimeWaitRegistration>,
    object_waiters: BTreeMap<RuntimeObjectId, VecDeque<RuntimeWaitRegistrationId>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RuntimeWaitRegistrationPlan {
    registration: RuntimeWaitRegistration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RuntimeWaitWakeCandidate {
    pub(super) completion: RuntimeWaitCompletion,
    pub(super) excluded: bool,
}

impl RuntimeWaits {
    pub(super) fn registrations(
        &self,
    ) -> &BTreeMap<RuntimeWaitRegistrationId, RuntimeWaitRegistration> {
        &self.wait_registrations
    }

    pub(super) fn object_waiters(
        &self,
    ) -> &BTreeMap<RuntimeObjectId, VecDeque<RuntimeWaitRegistrationId>> {
        &self.object_waiters
    }

    pub(super) fn plan_registration(
        &self,
        registration: RuntimeWaitRegistration,
    ) -> Result<RuntimeWaitRegistrationPlan, RuntimeStateError> {
        if self.wait_registrations.contains_key(&registration.id) {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-wait",
                "wait-registration-id-collision",
                "wait registration allocator selected a live ID",
            ));
        }
        if self
            .wait_registrations
            .values()
            .any(|existing| existing.thread_id == registration.thread_id)
        {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-wait",
                "wait-registration-thread-duplicate",
                "runtime thread already owns a wait registration",
            ));
        }
        if self
            .object_waiters
            .values()
            .any(|queue| queue.contains(&registration.id))
        {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-wait",
                "wait-registration-id-index-collision",
                "wait registration ID is already present in an object queue",
            ));
        }
        Ok(RuntimeWaitRegistrationPlan { registration })
    }

    pub(super) fn commit_registration(
        &mut self,
        plan: RuntimeWaitRegistrationPlan,
    ) -> RuntimeWaitRegistration {
        let registration = plan.registration;
        let previous = self
            .wait_registrations
            .insert(registration.id, registration);
        debug_assert!(previous.is_none());
        self.object_waiters
            .entry(registration.object_id)
            .or_default()
            .push_back(registration.id);
        registration
    }

    pub(super) fn registration_for_thread(
        &self,
        thread_id: RuntimeThreadId,
    ) -> Result<Option<RuntimeWaitRegistration>, RuntimeStateError> {
        let mut registrations = self
            .wait_registrations
            .values()
            .filter(|registration| registration.thread_id == thread_id)
            .copied();
        let registration = registrations.next();
        if registrations.next().is_some() {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-wait",
                "wait-registration-thread-duplicate",
                "one runtime thread owns multiple wait registrations",
            ));
        }
        Ok(registration)
    }

    pub(super) fn has_registration_for_thread(&self, thread_id: RuntimeThreadId) -> bool {
        self.wait_registrations
            .values()
            .any(|registration| registration.thread_id == thread_id)
    }

    pub(super) fn plan_wakes(
        &self,
        wait_states: &[(RuntimeObjectId, RuntimeWaitState)],
        excluded_threads: &[RuntimeThreadId],
    ) -> Result<Vec<RuntimeWaitWakeCandidate>, RuntimeStateError> {
        let mut candidates = Vec::new();
        let mut planned_threads = BTreeSet::new();
        for &(object_id, wait_state) in wait_states {
            if !matches!(wait_state, RuntimeWaitState::Signaled { .. }) {
                continue;
            }
            let queued_ids = self
                .object_waiters
                .get(&object_id)
                .cloned()
                .unwrap_or_default();
            for registration in self
                .wait_registrations
                .values()
                .filter(|registration| registration.object_id == object_id)
            {
                if !queued_ids.contains(&registration.id) {
                    return Err(RuntimeStateError::runtime_defect(
                        "runtime-wait",
                        "wait-registration-object-index-missing",
                        "wait registration is absent from its object queue",
                    ));
                }
            }
            let mut seen_registration_ids = BTreeSet::new();
            for registration_id in queued_ids {
                if !seen_registration_ids.insert(registration_id) {
                    return Err(RuntimeStateError::runtime_defect(
                        "runtime-wait",
                        "wait-object-queue-registration-duplicate",
                        "object wait queue contains a duplicate registration",
                    ));
                }
                let registration = self
                    .wait_registrations
                    .get(&registration_id)
                    .copied()
                    .ok_or_else(|| {
                        RuntimeStateError::runtime_defect(
                            "runtime-wait",
                            "wait-object-queue-registration-missing",
                            "object wait queue references an unknown registration",
                        )
                    })?;
                if registration.object_id != object_id {
                    return Err(RuntimeStateError::runtime_defect(
                        "runtime-wait",
                        "wait-object-queue-object-mismatch",
                        "object wait queue references a registration for another object",
                    ));
                }
                let excluded = excluded_threads.contains(&registration.thread_id);
                if !excluded && !planned_threads.insert(registration.thread_id) {
                    return Err(RuntimeStateError::runtime_defect(
                        "runtime-wait",
                        "wait-wake-thread-duplicate",
                        "one thread cannot be woken by multiple object signals",
                    ));
                }
                candidates.push(RuntimeWaitWakeCandidate {
                    completion: RuntimeWaitCompletion {
                        registration,
                        wait_state,
                    },
                    excluded,
                });
            }
        }
        Ok(candidates)
    }

    pub(super) fn commit_remove(&mut self, registration: RuntimeWaitRegistration) {
        let previous = self.wait_registrations.remove(&registration.id);
        debug_assert_eq!(previous, Some(registration));
        let remove_queue = if let Some(queue) = self.object_waiters.get_mut(&registration.object_id)
        {
            let previous_length = queue.len();
            queue.retain(|&registration_id| registration_id != registration.id);
            debug_assert_eq!(queue.len() + 1, previous_length);
            queue.is_empty()
        } else {
            debug_assert!(false, "wait registration object queue disappeared");
            false
        };
        if remove_queue {
            self.object_waiters.remove(&registration.object_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::{RuntimeHandle, RuntimeObjectKind};

    fn registration(id: u64, thread_id: u32, object_id: u64) -> RuntimeWaitRegistration {
        RuntimeWaitRegistration {
            id: RuntimeWaitRegistrationId(id),
            thread_id: RuntimeThreadId(thread_id),
            object_id: RuntimeObjectId(object_id),
            handle: RuntimeHandle::from_parts(RuntimeObjectKind::Thread, 1, object_id as u16)
                .unwrap(),
        }
    }

    #[test]
    fn registration_plans_are_mutation_free_and_indexes_commit_together() {
        let mut waits = RuntimeWaits::default();
        let registration = registration(1, 0x2000, 2);
        let plan = waits.plan_registration(registration).unwrap();
        assert!(waits.registrations().is_empty());
        assert!(waits.object_waiters().is_empty());

        assert_eq!(waits.commit_registration(plan), registration);
        assert_eq!(
            waits.registration_for_thread(RuntimeThreadId(0x2000)),
            Ok(Some(registration))
        );
        assert_eq!(
            waits.object_waiters().get(&RuntimeObjectId(2)),
            Some(&VecDeque::from([RuntimeWaitRegistrationId(1)]))
        );
    }

    #[test]
    fn wake_plans_preserve_fifo_and_remove_only_during_commit() {
        let mut waits = RuntimeWaits::default();
        let first = registration(1, 0x2000, 2);
        let second = registration(2, 0x1001, 2);
        let first_plan = waits.plan_registration(first).unwrap();
        waits.commit_registration(first_plan);
        let second_plan = waits.plan_registration(second).unwrap();
        waits.commit_registration(second_plan);

        let candidates = waits
            .plan_wakes(
                &[(
                    RuntimeObjectId(2),
                    RuntimeWaitState::Signaled { exit_code: 23 },
                )],
                &[],
            )
            .unwrap();
        assert_eq!(
            candidates
                .iter()
                .map(|candidate| candidate.completion.registration.id)
                .collect::<Vec<_>>(),
            vec![RuntimeWaitRegistrationId(1), RuntimeWaitRegistrationId(2)]
        );
        assert_eq!(waits.registrations().len(), 2);
        for candidate in candidates {
            waits.commit_remove(candidate.completion.registration);
        }
        assert!(waits.registrations().is_empty());
        assert!(waits.object_waiters().is_empty());
    }

    #[test]
    fn rejected_registration_preserves_both_indexes() {
        let mut waits = RuntimeWaits::default();
        let existing = registration(1, 0x2000, 2);
        let plan = waits.plan_registration(existing).unwrap();
        waits.commit_registration(plan);
        let before = waits.clone();

        assert_eq!(
            waits
                .plan_registration(registration(1, 0x1001, 3))
                .unwrap_err()
                .code,
            "wait-registration-id-collision"
        );
        assert_eq!(
            waits
                .plan_registration(registration(2, 0x2000, 3))
                .unwrap_err()
                .code,
            "wait-registration-thread-duplicate"
        );
        assert_eq!(waits, before);
    }

    #[test]
    fn excluded_wakes_still_return_candidates_for_cross_subsystem_validation() {
        let mut waits = RuntimeWaits::default();
        let registration = registration(1, 0x2000, 2);
        let plan = waits.plan_registration(registration).unwrap();
        waits.commit_registration(plan);

        let candidates = waits
            .plan_wakes(
                &[(
                    RuntimeObjectId(2),
                    RuntimeWaitState::Signaled { exit_code: 7 },
                )],
                &[RuntimeThreadId(0x2000)],
            )
            .unwrap();
        assert_eq!(candidates.len(), 1);
        assert!(candidates[0].excluded);
        assert_eq!(candidates[0].completion.registration, registration);
        assert_eq!(waits.registrations().len(), 1);
    }
}

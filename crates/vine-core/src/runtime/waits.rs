use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde::Serialize;

use super::{
    RuntimeHandle, RuntimeMutexId, RuntimeObjectId, RuntimeStateError, RuntimeThreadId,
    RuntimeTimerClockState, RuntimeWaitState, RuntimeWaitableTimer,
};

pub const RUNTIME_WAITABLE_TIMER_PACING_PAYLOAD_VERSION: u32 = 1;
pub const RUNTIME_WAITABLE_TIMER_PACING_SERVICE: &str = "runtime.timer-delay.v1";

const fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct RuntimeWaitRegistrationId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeWaitRegistration {
    pub id: RuntimeWaitRegistrationId,
    pub thread_id: RuntimeThreadId,
    pub object_id: RuntimeObjectId,
    pub handle: RuntimeHandle,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_deadline_time_100ns: Option<u64>,
    #[serde(skip_serializing_if = "is_false")]
    pub alertable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub waitable_timer_pacing: Option<RuntimeWaitableTimerPacing>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeWaitableTimerPacing {
    pub request_id: u64,
    pub delay_milliseconds: u32,
    pub target_time_100ns: u64,
    pub timer_clock_before: RuntimeTimerClockState,
    pub timer_before: RuntimeWaitableTimer,
    pub outcome: RuntimeWaitForSingleObjectExOutcome,
}

impl RuntimeWaitableTimerPacing {
    pub fn encode_host_payload(self, registration_id: RuntimeWaitRegistrationId) -> [u8; 52] {
        let arm_generation = self.timer_before.arm.map_or(0, |arm| arm.generation);
        let outcome = match self.outcome {
            RuntimeWaitForSingleObjectExOutcome::ObjectSignaled => 0,
            RuntimeWaitForSingleObjectExOutcome::TimedOut => 1,
        };
        let words = [
            RUNTIME_WAITABLE_TIMER_PACING_PAYLOAD_VERSION,
            self.delay_milliseconds,
            registration_id.0 as u32,
            (registration_id.0 >> 32) as u32,
            self.timer_before.id.0 as u32,
            (self.timer_before.id.0 >> 32) as u32,
            self.target_time_100ns as u32,
            (self.target_time_100ns >> 32) as u32,
            self.timer_clock_before.generation as u32,
            (self.timer_clock_before.generation >> 32) as u32,
            arm_generation as u32,
            (arm_generation >> 32) as u32,
            outcome,
        ];
        let mut payload = [0; 52];
        for (index, word) in words.into_iter().enumerate() {
            payload[index * 4..index * 4 + 4].copy_from_slice(&word.to_le_bytes());
        }
        payload
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeWaitCompletion {
    pub registration: RuntimeWaitRegistration,
    pub wait_state: RuntimeWaitState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "outcome", rename_all = "kebab-case")]
pub enum RuntimeWaitDisposition {
    AlreadySignaled {
        object_id: RuntimeObjectId,
        wait_state: RuntimeWaitState,
    },
    Pending {
        registration: RuntimeWaitRegistration,
    },
    MutexAcquired {
        object_id: RuntimeObjectId,
        mutex_id: RuntimeMutexId,
        recursion_count: u32,
        abandoned: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeWaitForSingleObjectExOutcome {
    ObjectSignaled,
    TimedOut,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeWaitForSingleObjectExResult {
    pub outcome: RuntimeWaitForSingleObjectExOutcome,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registration: Option<RuntimeWaitRegistration>,
    pub object_id: RuntimeObjectId,
    pub object_wait_state: RuntimeWaitState,
    pub timer: RuntimeWaitableTimer,
    pub timer_clock_before: RuntimeTimerClockState,
    pub timer_clock_after: RuntimeTimerClockState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "disposition", rename_all = "kebab-case")]
pub enum RuntimeWaitForSingleObjectExBegin {
    Immediate {
        result: RuntimeWaitForSingleObjectExResult,
    },
    Deferred {
        registration: RuntimeWaitRegistration,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RuntimeWaits {
    wait_registrations: BTreeMap<RuntimeWaitRegistrationId, RuntimeWaitRegistration>,
    object_waiters: BTreeMap<RuntimeObjectId, VecDeque<RuntimeWaitRegistrationId>>,
    #[serde(skip)]
    next_registration_id: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RuntimeWaitRegistrationPlan {
    registration: RuntimeWaitRegistration,
    previous_next_registration_id: u64,
    next_registration_id: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RuntimeWaitWakeCandidate {
    pub(super) completion: RuntimeWaitCompletion,
    pub(super) excluded: bool,
}

impl Default for RuntimeWaits {
    fn default() -> Self {
        Self {
            wait_registrations: BTreeMap::new(),
            object_waiters: BTreeMap::new(),
            next_registration_id: 1,
        }
    }
}

impl RuntimeWaitRegistrationPlan {
    pub(super) const fn registration(&self) -> RuntimeWaitRegistration {
        self.registration
    }
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

    pub(super) const fn next_registration_id(&self) -> u64 {
        self.next_registration_id
    }

    pub(super) fn plan_registration(
        &self,
        thread_id: RuntimeThreadId,
        object_id: RuntimeObjectId,
        handle: RuntimeHandle,
    ) -> Result<RuntimeWaitRegistrationPlan, RuntimeStateError> {
        self.plan_registration_with_options(thread_id, object_id, handle, None, false, None)
    }

    pub(super) fn plan_registration_with_options(
        &self,
        thread_id: RuntimeThreadId,
        object_id: RuntimeObjectId,
        handle: RuntimeHandle,
        timeout_deadline_time_100ns: Option<u64>,
        alertable: bool,
        waitable_timer_pacing: Option<RuntimeWaitableTimerPacing>,
    ) -> Result<RuntimeWaitRegistrationPlan, RuntimeStateError> {
        let id = RuntimeWaitRegistrationId(self.next_registration_id);
        let next_registration_id = self.next_registration_id.checked_add(1).ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-wait",
                "wait-registration-id-overflow",
                "wait registration ID space is exhausted",
            )
        })?;
        let registration = RuntimeWaitRegistration {
            id,
            thread_id,
            object_id,
            handle,
            timeout_deadline_time_100ns,
            alertable,
            waitable_timer_pacing,
        };
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
        Ok(RuntimeWaitRegistrationPlan {
            registration,
            previous_next_registration_id: self.next_registration_id,
            next_registration_id,
        })
    }

    pub(super) fn preflight_registration(
        &self,
        plan: &RuntimeWaitRegistrationPlan,
    ) -> Result<(), RuntimeStateError> {
        let registration = plan.registration;
        if self.next_registration_id != plan.previous_next_registration_id
            || plan.previous_next_registration_id.checked_add(1) != Some(plan.next_registration_id)
            || self.wait_registrations.contains_key(&registration.id)
            || self
                .wait_registrations
                .values()
                .any(|existing| existing.thread_id == registration.thread_id)
            || self
                .object_waiters
                .values()
                .any(|queue| queue.contains(&registration.id))
        {
            return Err(RuntimeStateError::conflict(
                "runtime-wait",
                "wait-registration-plan-stale",
                "wait registration state changed after planning",
            ));
        }
        Ok(())
    }

    pub(super) fn commit_registration(
        &mut self,
        plan: RuntimeWaitRegistrationPlan,
    ) -> RuntimeWaitRegistration {
        debug_assert!(
            self.preflight_registration(&plan).is_ok(),
            "wait registration plan became stale"
        );
        debug_assert_eq!(
            self.next_registration_id,
            plan.previous_next_registration_id
        );
        let registration = plan.registration;
        let previous = self
            .wait_registrations
            .insert(registration.id, registration);
        debug_assert!(previous.is_none());
        self.object_waiters
            .entry(registration.object_id)
            .or_default()
            .push_back(registration.id);
        self.next_registration_id = plan.next_registration_id;
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
            timeout_deadline_time_100ns: None,
            alertable: false,
            waitable_timer_pacing: None,
        }
    }

    #[test]
    fn registration_plans_are_mutation_free_and_indexes_commit_together() {
        let mut waits = RuntimeWaits::default();
        let registration = registration(1, 0x2000, 2);
        let plan = waits
            .plan_registration(
                registration.thread_id,
                registration.object_id,
                registration.handle,
            )
            .unwrap();
        assert_eq!(plan.registration(), registration);
        assert!(waits.registrations().is_empty());
        assert!(waits.object_waiters().is_empty());
        assert_eq!(waits.next_registration_id(), 1);

        assert_eq!(waits.commit_registration(plan), registration);
        assert_eq!(waits.next_registration_id(), 2);
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
        let first_plan = waits
            .plan_registration(first.thread_id, first.object_id, first.handle)
            .unwrap();
        waits.commit_registration(first_plan);
        let second_plan = waits
            .plan_registration(second.thread_id, second.object_id, second.handle)
            .unwrap();
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
        let plan = waits
            .plan_registration(existing.thread_id, existing.object_id, existing.handle)
            .unwrap();
        waits.commit_registration(plan);
        let before = waits.clone();

        assert_eq!(
            waits
                .plan_registration(
                    RuntimeThreadId(0x2000),
                    RuntimeObjectId(3),
                    RuntimeHandle::from_parts(RuntimeObjectKind::Thread, 1, 3).unwrap(),
                )
                .unwrap_err()
                .code,
            "wait-registration-thread-duplicate"
        );
        assert_eq!(waits, before);

        waits.next_registration_id = 1;
        let collision_before = waits.clone();
        assert_eq!(
            waits
                .plan_registration(
                    RuntimeThreadId(0x1001),
                    RuntimeObjectId(3),
                    RuntimeHandle::from_parts(RuntimeObjectKind::Thread, 1, 3).unwrap(),
                )
                .unwrap_err()
                .code,
            "wait-registration-id-collision"
        );
        assert_eq!(waits, collision_before);
    }

    #[test]
    fn exhausted_registration_allocator_is_mutation_free() {
        let waits = RuntimeWaits {
            next_registration_id: u64::MAX,
            ..RuntimeWaits::default()
        };
        let before = waits.clone();

        assert_eq!(
            waits
                .plan_registration(
                    RuntimeThreadId(0x2000),
                    RuntimeObjectId(2),
                    RuntimeHandle::from_parts(RuntimeObjectKind::Thread, 1, 2).unwrap(),
                )
                .unwrap_err()
                .code,
            "wait-registration-id-overflow"
        );
        assert_eq!(waits, before);
    }

    #[test]
    fn excluded_wakes_still_return_candidates_for_cross_subsystem_validation() {
        let mut waits = RuntimeWaits::default();
        let registration = registration(1, 0x2000, 2);
        let plan = waits
            .plan_registration(
                registration.thread_id,
                registration.object_id,
                registration.handle,
            )
            .unwrap();
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

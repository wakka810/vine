use std::collections::{BTreeMap, BTreeSet, VecDeque};

use super::{
    RuntimeHandle, RuntimeMutexId, RuntimeObjectId, RuntimeStateError, RuntimeThreadId,
    RuntimeWaitState, RuntimeWaitableTimer,
};

pub const RUNTIME_WAITABLE_TIMER_PACING_SERVICE: &str = "browser.timer-delay";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct RuntimeWaitRegistrationId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeWaitRegistration {
    pub id: RuntimeWaitRegistrationId,
    pub thread_id: RuntimeThreadId,
    pub object_id: RuntimeObjectId,
    pub handle: RuntimeHandle,
    pub timeout_deadline_time_100ns: Option<u64>,
    pub alertable: bool,
    pub waitable_timer_pacing: Option<RuntimeWaitableTimerPacing>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeWaitableTimerPacing {
    pub request_id: u64,
    pub delay_milliseconds: u32,
    pub target_time_100ns: u64,
    pub started_at_time_100ns: u64,
    pub timer_before: RuntimeWaitableTimer,
    pub outcome: RuntimeWaitForSingleObjectExOutcome,
    pub signaled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeWaitCompletion {
    pub registration: RuntimeWaitRegistration,
    pub wait_state: RuntimeWaitState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeWaitForSingleObjectExOutcome {
    ObjectSignaled,
    TimedOut,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeWaitForSingleObjectExBegin {
    Immediate {
        outcome: RuntimeWaitForSingleObjectExOutcome,
    },
    Deferred {
        registration: RuntimeWaitRegistration,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeWaits {
    wait_registrations: BTreeMap<RuntimeWaitRegistrationId, RuntimeWaitRegistration>,
    object_waiters: BTreeMap<RuntimeObjectId, VecDeque<RuntimeWaitRegistrationId>>,
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

    #[cfg(test)]
    pub(crate) const fn next_registration_id(&self) -> u64 {
        self.next_registration_id
    }

    pub(super) fn register(
        &mut self,
        thread_id: RuntimeThreadId,
        object_id: RuntimeObjectId,
        handle: RuntimeHandle,
    ) -> Result<RuntimeWaitRegistration, RuntimeStateError> {
        let registration =
            self.checked_registration(thread_id, object_id, handle, None, false, None)?;
        self.publish_registration(registration);
        Ok(registration)
    }

    pub(super) fn checked_registration(
        &self,
        thread_id: RuntimeThreadId,
        object_id: RuntimeObjectId,
        handle: RuntimeHandle,
        timeout_deadline_time_100ns: Option<u64>,
        alertable: bool,
        waitable_timer_pacing: Option<RuntimeWaitableTimerPacing>,
    ) -> Result<RuntimeWaitRegistration, RuntimeStateError> {
        let id = RuntimeWaitRegistrationId(self.next_registration_id);
        self.next_registration_id
            .checked_add(1)
            .ok_or_else(|| RuntimeStateError::new("wait registration ID space is exhausted"))?;
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
            return Err(RuntimeStateError::new(
                "wait registration allocator selected a live ID",
            ));
        }
        if self
            .wait_registrations
            .values()
            .any(|existing| existing.thread_id == registration.thread_id)
        {
            return Err(RuntimeStateError::new(
                "runtime thread already owns a wait registration",
            ));
        }
        if self
            .object_waiters
            .values()
            .any(|queue| queue.contains(&registration.id))
        {
            return Err(RuntimeStateError::new(
                "wait registration ID is already present in an object queue",
            ));
        }
        Ok(registration)
    }

    pub(super) fn publish_registration(&mut self, registration: RuntimeWaitRegistration) {
        let next_id = registration
            .id
            .0
            .checked_add(1)
            .expect("validated wait registration ID");
        let previous = self
            .wait_registrations
            .insert(registration.id, registration);
        debug_assert!(previous.is_none());
        self.object_waiters
            .entry(registration.object_id)
            .or_default()
            .push_back(registration.id);
        self.next_registration_id = next_id;
    }

    pub(super) fn replace_timer_pacing(
        &mut self,
        registration: RuntimeWaitRegistration,
        pacing: RuntimeWaitableTimerPacing,
    ) {
        let live = self
            .wait_registrations
            .get_mut(&registration.id)
            .expect("validated timer wait registration");
        debug_assert_eq!(*live, registration);
        live.waitable_timer_pacing = Some(pacing);
    }

    pub(super) fn signal_timer_waiters(
        &mut self,
        object_id: RuntimeObjectId,
        deadline_time_100ns: u64,
        manual_reset: bool,
    ) {
        for registration in self.wait_registrations.values_mut() {
            if registration.object_id == object_id
                && registration
                    .timeout_deadline_time_100ns
                    .is_some_and(|timeout| timeout >= deadline_time_100ns)
                && let Some(pacing) = &mut registration.waitable_timer_pacing
                && !pacing.signaled
            {
                pacing.signaled = true;
                if !manual_reset {
                    break;
                }
            }
        }
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
            return Err(RuntimeStateError::new(
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
            let queued_ids = self.object_waiters.get(&object_id);
            for registration in self
                .wait_registrations
                .values()
                .filter(|registration| registration.object_id == object_id)
            {
                if queued_ids.is_none_or(|queue| !queue.contains(&registration.id)) {
                    return Err(RuntimeStateError::new(
                        "wait registration is absent from its object queue",
                    ));
                }
            }
            let Some(queued_ids) = queued_ids else {
                continue;
            };
            let mut seen_registration_ids = BTreeSet::new();
            for &registration_id in queued_ids {
                if !seen_registration_ids.insert(registration_id) {
                    return Err(RuntimeStateError::new(
                        "object wait queue contains a duplicate registration",
                    ));
                }
                let registration = self
                    .wait_registrations
                    .get(&registration_id)
                    .copied()
                    .ok_or_else(|| {
                        RuntimeStateError::new(
                            "object wait queue references an unknown registration",
                        )
                    })?;
                if registration.object_id != object_id {
                    return Err(RuntimeStateError::new(
                        "object wait queue references a registration for another object",
                    ));
                }
                let excluded = excluded_threads.contains(&registration.thread_id);
                if !excluded && !planned_threads.insert(registration.thread_id) {
                    return Err(RuntimeStateError::new(
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
    fn registration_publishes_both_indexes() {
        let mut waits = RuntimeWaits::default();
        let registration = registration(1, 0x2000, 2);
        assert_eq!(
            waits
                .register(
                    registration.thread_id,
                    registration.object_id,
                    registration.handle
                )
                .unwrap(),
            registration
        );
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
        waits
            .register(first.thread_id, first.object_id, first.handle)
            .unwrap();
        waits
            .register(second.thread_id, second.object_id, second.handle)
            .unwrap();

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
        waits
            .register(existing.thread_id, existing.object_id, existing.handle)
            .unwrap();
        let next_handle = RuntimeHandle::from_parts(RuntimeObjectKind::Thread, 1, 3).unwrap();
        assert!(
            waits
                .register(existing.thread_id, RuntimeObjectId(3), next_handle)
                .is_err()
        );
        assert_eq!(waits.next_registration_id(), 2);
        assert_eq!(
            waits.registration_for_thread(existing.thread_id).unwrap(),
            Some(existing)
        );
        assert_eq!(waits.registrations().len(), 1);
        assert_eq!(
            waits.object_waiters().get(&existing.object_id),
            Some(&VecDeque::from([existing.id]))
        );
        assert!(!waits.object_waiters().contains_key(&RuntimeObjectId(3)));

        waits.next_registration_id = 1;
        assert!(
            waits
                .register(RuntimeThreadId(0x1001), RuntimeObjectId(3), next_handle)
                .is_err()
        );
        assert_eq!(waits.next_registration_id(), 1);
        assert_eq!(
            waits.registration_for_thread(existing.thread_id).unwrap(),
            Some(existing)
        );
        assert_eq!(waits.registrations().len(), 1);
        assert_eq!(
            waits.object_waiters().get(&existing.object_id),
            Some(&VecDeque::from([existing.id]))
        );
        assert!(!waits.object_waiters().contains_key(&RuntimeObjectId(3)));
        waits.next_registration_id = 2;
        let retry = waits
            .register(RuntimeThreadId(0x1001), RuntimeObjectId(3), next_handle)
            .unwrap();
        assert_eq!(retry.id, RuntimeWaitRegistrationId(2));
        assert_eq!(
            waits.registration_for_thread(existing.thread_id).unwrap(),
            Some(existing)
        );
        assert_eq!(
            waits.object_waiters().get(&retry.object_id),
            Some(&VecDeque::from([retry.id]))
        );
    }

    #[test]
    fn exhausted_registration_allocator_is_mutation_free() {
        let mut waits = RuntimeWaits {
            next_registration_id: u64::MAX,
            ..RuntimeWaits::default()
        };
        assert!(
            waits
                .register(
                    RuntimeThreadId(0x2000),
                    RuntimeObjectId(2),
                    RuntimeHandle::from_parts(RuntimeObjectKind::Thread, 1, 2).unwrap()
                )
                .is_err()
        );
        assert_eq!(waits.next_registration_id(), u64::MAX);
        assert!(waits.registrations().is_empty());
        assert!(waits.object_waiters().is_empty());
    }

    #[test]
    fn excluded_wakes_still_return_candidates_for_cross_subsystem_validation() {
        let mut waits = RuntimeWaits::default();
        let registration = registration(1, 0x2000, 2);
        waits
            .register(
                registration.thread_id,
                registration.object_id,
                registration.handle,
            )
            .unwrap();

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

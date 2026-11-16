use std::collections::{BTreeMap, BTreeSet, VecDeque};

use super::{
    RuntimeHandle, RuntimeMutexId, RuntimeObjectId, RuntimeStateError, RuntimeThreadId,
    RuntimeWaitState, RuntimeWaitableTimer,
};

pub const RUNTIME_WAITABLE_TIMER_PACING_SERVICE: &str = "browser.timer-delay";
pub const RUNTIME_OBJECT_WAIT_SERVICE: &str = "browser.object-wait";

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
    pub object_wait_pacing: Option<RuntimeObjectWaitPacing>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeObjectWaitPacing {
    pub request_id: u64,
    pub delay_milliseconds: u32,
    pub started_at_time_100ns: u64,
    pub signaled: bool,
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
            object_wait_pacing: None,
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

    pub(super) fn replace_object_pacing(
        &mut self,
        registration: RuntimeWaitRegistration,
        pacing: RuntimeObjectWaitPacing,
    ) {
        let live = self
            .wait_registrations
            .get_mut(&registration.id)
            .expect("validated object wait registration");
        debug_assert_eq!(*live, registration);
        live.object_wait_pacing = Some(pacing);
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
    use crate::runtime::RuntimeObjectKind;

    #[test]
    fn duplicate_thread_wait_rejection_and_cancellation_preserve_the_other_waiter() {
        let mut waits = RuntimeWaits::default();
        let first_thread = RuntimeThreadId(7);
        let second_thread = RuntimeThreadId(8);
        let first_object = RuntimeObjectId(1);
        let second_object = RuntimeObjectId(2);
        let first_handle = RuntimeHandle::from_parts(RuntimeObjectKind::Thread, 1, 1).unwrap();
        let second_handle = RuntimeHandle::from_parts(RuntimeObjectKind::Thread, 1, 2).unwrap();
        let first = waits
            .register(first_thread, first_object, first_handle)
            .unwrap();
        let second = waits
            .register(second_thread, second_object, second_handle)
            .unwrap();
        assert!(
            waits
                .register(first_thread, second_object, second_handle)
                .is_err()
        );
        assert_eq!(waits.registration_for_thread(first_thread), Ok(Some(first)));
        assert_eq!(
            waits.registration_for_thread(second_thread),
            Ok(Some(second))
        );
        waits.commit_remove(first);
        assert_eq!(waits.registration_for_thread(first_thread), Ok(None));
        assert_eq!(
            waits.registration_for_thread(second_thread),
            Ok(Some(second))
        );
        let retry = waits
            .register(first_thread, second_object, second_handle)
            .unwrap();
        assert_eq!(waits.registration_for_thread(first_thread), Ok(Some(retry)));
        assert_eq!(
            waits.registration_for_thread(second_thread),
            Ok(Some(second))
        );
        waits.commit_remove(retry);
        waits.commit_remove(second);
        assert_eq!(waits.registration_for_thread(first_thread), Ok(None));
        assert_eq!(waits.registration_for_thread(second_thread), Ok(None));
    }
}

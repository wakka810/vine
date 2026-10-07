use std::collections::{BTreeMap, VecDeque};

use super::thread::{RuntimeThreadLifecycleState, RuntimeThreads};
use super::waits::{
    RUNTIME_WAITABLE_TIMER_PACING_SERVICE, RuntimeWaitableTimerPacing, RuntimeWaits,
};
use super::{
    RuntimeHandle, RuntimeObjectId, RuntimeStateError, RuntimeThreadId, RuntimeWaitCompletion,
    RuntimeWaitRegistration, RuntimeWaitRegistrationId, RuntimeWaitState,
};

pub const RUNTIME_API_BOUNDARY_QUANTUM: u32 = 64;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeApiBoundaryDisposition {
    #[default]
    ContinueQuantum,
    YieldCurrentThread,
    BlockCurrentThread,
    TerminateCurrentThread,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchedulerThreadState {
    Initialized,
    Suspended {
        count: u32,
    },
    Runnable,
    WaitingHost {
        request_id: u64,
    },
    WaitingObject {
        registration_id: RuntimeWaitRegistrationId,
        object_id: RuntimeObjectId,
    },
    WaitingCriticalSection {
        address: u32,
    },
    WaitingMessage,
    InCallback {
        depth: usize,
    },
    Terminating {
        exit_code: u32,
    },
    Terminated {
        exit_code: u32,
    },
}

impl SchedulerThreadState {
    pub const fn is_executable(self) -> bool {
        matches!(self, Self::Runnable | Self::InCallback { .. })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeHostRequest {
    pub request_id: u64,
    pub thread_id: RuntimeThreadId,
    pub service: String,
    pub resume_state: SchedulerThreadState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeHostCompletion {
    pub request_id: u64,
    pub thread_id: RuntimeThreadId,
    pub service: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeSchedulerEvent {
    HostCompleted(RuntimeHostCompletion),
    WaitSatisfied(RuntimeWaitCompletion),
    MessageReady(RuntimeThreadId),
}

impl RuntimeSchedulerEvent {
    pub const fn thread_id(&self) -> RuntimeThreadId {
        match self {
            Self::HostCompleted(completion) => completion.thread_id,
            Self::WaitSatisfied(completion) => completion.registration.thread_id,
            Self::MessageReady(thread_id) => *thread_id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeScheduler {
    current_thread_id: RuntimeThreadId,
    threads: BTreeMap<RuntimeThreadId, SchedulerThreadState>,
    completed_api_boundaries_in_quantum: u32,
    waits: RuntimeWaits,
    events: VecDeque<RuntimeSchedulerEvent>,
    host_requests: BTreeMap<u64, RuntimeHostRequest>,
    next_host_request_id: u64,
}

impl RuntimeScheduler {
    pub(super) fn new(initial_thread_id: RuntimeThreadId) -> Self {
        Self {
            current_thread_id: initial_thread_id,
            threads: BTreeMap::from([(initial_thread_id, SchedulerThreadState::Initialized)]),
            completed_api_boundaries_in_quantum: 0,
            waits: RuntimeWaits::default(),
            events: VecDeque::new(),
            host_requests: BTreeMap::new(),
            next_host_request_id: 1,
        }
    }

    pub fn current_thread_id(&self) -> RuntimeThreadId {
        self.current_thread_id
    }

    pub fn thread_states(&self) -> &BTreeMap<RuntimeThreadId, SchedulerThreadState> {
        &self.threads
    }

    pub fn wait_registrations(
        &self,
    ) -> &BTreeMap<RuntimeWaitRegistrationId, RuntimeWaitRegistration> {
        self.waits.registrations()
    }

    pub fn object_waiters(
        &self,
    ) -> &BTreeMap<RuntimeObjectId, VecDeque<RuntimeWaitRegistrationId>> {
        self.waits.object_waiters()
    }

    pub fn events(&self) -> &VecDeque<RuntimeSchedulerEvent> {
        &self.events
    }

    pub fn host_requests(&self) -> &BTreeMap<u64, RuntimeHostRequest> {
        &self.host_requests
    }

    #[cfg(test)]
    fn next_host_request_id(&self) -> u64 {
        self.next_host_request_id
    }

    #[cfg(test)]
    pub(crate) fn next_wait_registration_id(&self) -> u64 {
        self.waits.next_registration_id()
    }

    pub(super) fn contains_thread(&self, thread_id: RuntimeThreadId) -> bool {
        self.threads.contains_key(&thread_id)
    }

    pub(super) fn state(
        &self,
        thread_id: RuntimeThreadId,
    ) -> Result<SchedulerThreadState, RuntimeStateError> {
        self.threads.get(&thread_id).copied().ok_or_else(|| {
            RuntimeStateError::new(format!(
                "runtime thread ID {} has no scheduler state",
                thread_id.0
            ))
        })
    }

    pub(super) fn current_state(&self) -> Result<SchedulerThreadState, RuntimeStateError> {
        self.state(self.current_thread_id)
    }

    pub(super) fn validate_registration(
        &self,
        thread_id: RuntimeThreadId,
    ) -> Result<(), RuntimeStateError> {
        if self.threads.contains_key(&thread_id) {
            return Err(RuntimeStateError::new(format!(
                "runtime thread ID {} already has a scheduler lane",
                thread_id.0
            )));
        }
        Ok(())
    }

    pub(super) fn register(&mut self, thread_id: RuntimeThreadId, state: SchedulerThreadState) {
        let previous = self.threads.insert(thread_id, state);
        assert!(
            previous.is_none(),
            "scheduler registration must be validated before insertion"
        );
    }

    pub(super) fn resume_thread(
        &mut self,
        thread_id: RuntimeThreadId,
    ) -> Result<u32, RuntimeStateError> {
        match self.state(thread_id)? {
            SchedulerThreadState::Suspended { count } if count != 0 => {
                let remaining = count - 1;
                *self
                    .threads
                    .get_mut(&thread_id)
                    .expect("validated scheduler lane") = if remaining == 0 {
                    SchedulerThreadState::Runnable
                } else {
                    SchedulerThreadState::Suspended { count: remaining }
                };
                Ok(count)
            }
            SchedulerThreadState::Suspended { .. } => Err(RuntimeStateError::new(
                "suspended scheduler state contains a zero suspend count",
            )),
            SchedulerThreadState::Runnable
            | SchedulerThreadState::WaitingHost { .. }
            | SchedulerThreadState::WaitingObject { .. }
            | SchedulerThreadState::WaitingCriticalSection { .. }
            | SchedulerThreadState::WaitingMessage
            | SchedulerThreadState::InCallback { .. } => Ok(0),
            SchedulerThreadState::Initialized
            | SchedulerThreadState::Terminating { .. }
            | SchedulerThreadState::Terminated { .. } => Ok(u32::MAX),
        }
    }

    pub(super) fn start_initial_thread(&mut self, thread_id: RuntimeThreadId) {
        let state = self
            .threads
            .get_mut(&thread_id)
            .expect("validated initial scheduler lane");
        assert_eq!(*state, SchedulerThreadState::Initialized);
        *state = SchedulerThreadState::Runnable;
    }

    /// Publishes a synchronous transition after the aggregate validates the live lane.
    pub(super) fn set_validated_state(
        &mut self,
        thread_id: RuntimeThreadId,
        next: SchedulerThreadState,
    ) {
        *self
            .threads
            .get_mut(&thread_id)
            .expect("validated scheduler lane") = next;
    }

    pub(super) fn select_thread(
        &mut self,
        thread_id: RuntimeThreadId,
    ) -> Result<(), RuntimeStateError> {
        self.state(thread_id)?;
        self.current_thread_id = thread_id;
        self.completed_api_boundaries_in_quantum = 0;
        Ok(())
    }

    pub(super) fn complete_api_boundary(
        &mut self,
        disposition: RuntimeApiBoundaryDisposition,
        runnable_competitor: Option<RuntimeThreadId>,
        force_competitor: bool,
    ) -> Result<Option<RuntimeThreadId>, RuntimeStateError> {
        if disposition == RuntimeApiBoundaryDisposition::BlockCurrentThread {
            return Err(RuntimeStateError::new(
                "a blocking API boundary must select another executable thread directly",
            ));
        }
        if disposition == RuntimeApiBoundaryDisposition::TerminateCurrentThread {
            return Err(RuntimeStateError::new(
                "a terminating API boundary must commit through the thread lifecycle owner",
            ));
        }
        let current_state = self.current_state()?;
        if !current_state.is_executable() {
            return Err(RuntimeStateError::new(
                "an API execution boundary requires an executable current thread",
            ));
        }
        let next_executable = if let Some(thread_id) = runnable_competitor {
            if thread_id == self.current_thread_id || !self.state(thread_id)?.is_executable() {
                return Err(RuntimeStateError::new(
                    "an API execution boundary competitor must be another executable thread",
                ));
            }
            Some(thread_id)
        } else {
            None
        };
        let Some(next_executable) = next_executable else {
            self.completed_api_boundaries_in_quantum = 0;
            return Ok(None);
        };
        let completed = if disposition == RuntimeApiBoundaryDisposition::YieldCurrentThread {
            0
        } else {
            self.completed_api_boundaries_in_quantum
                .checked_add(1)
                .ok_or_else(|| {
                    RuntimeStateError::new("API execution boundary counter is exhausted")
                })?
        };
        let select_competitor = disposition == RuntimeApiBoundaryDisposition::YieldCurrentThread
            || force_competitor
            || completed >= RUNTIME_API_BOUNDARY_QUANTUM;
        self.completed_api_boundaries_in_quantum = if select_competitor { 0 } else { completed };
        if select_competitor {
            self.current_thread_id = next_executable;
            Ok(Some(next_executable))
        } else {
            Ok(None)
        }
    }

    pub(super) fn checked_host_request(
        &self,
        service: String,
    ) -> Result<RuntimeHostRequest, RuntimeStateError> {
        if service.is_empty() {
            return Err(RuntimeStateError::new(
                "host request requires a nonempty service identity",
            ));
        }
        let resume_state = match self.current_state()? {
            state @ (SchedulerThreadState::Runnable | SchedulerThreadState::InCallback { .. }) => {
                state
            }
            _ => {
                return Err(RuntimeStateError::new(
                    "only a runnable guest thread or active callback can begin a host request",
                ));
            }
        };
        let request_id = self.next_host_request_id;
        request_id
            .checked_add(1)
            .ok_or_else(|| RuntimeStateError::new("host request ID space is exhausted"))?;
        if self.host_requests.contains_key(&request_id) {
            return Err(RuntimeStateError::new(format!(
                "next host request ID {request_id} is already pending"
            )));
        }
        Ok(RuntimeHostRequest {
            request_id,
            thread_id: self.current_thread_id,
            service,
            resume_state,
        })
    }

    pub(super) fn publish_host_request(
        &mut self,
        request: RuntimeHostRequest,
    ) -> RuntimeHostRequest {
        self.next_host_request_id = request
            .request_id
            .checked_add(1)
            .expect("validated host request ID");
        let previous = self
            .host_requests
            .insert(request.request_id, request.clone());
        debug_assert!(previous.is_none());
        self.set_validated_state(
            request.thread_id,
            SchedulerThreadState::WaitingHost {
                request_id: request.request_id,
            },
        );
        request
    }

    pub(super) fn validate_host_completion(
        &self,
        request_id: u64,
    ) -> Result<RuntimeHostRequest, RuntimeStateError> {
        let request = self
            .host_requests
            .get(&request_id)
            .cloned()
            .ok_or_else(|| {
                RuntimeStateError::new(format!("host request ID {request_id} is not pending"))
            })?;
        if self.state(request.thread_id)? != (SchedulerThreadState::WaitingHost { request_id }) {
            return Err(RuntimeStateError::new(
                "host completion does not match its owner thread wait state",
            ));
        }
        Ok(request)
    }

    pub(super) fn commit_host_completion(
        &mut self,
        request: RuntimeHostRequest,
    ) -> RuntimeHostCompletion {
        debug_assert_eq!(self.host_requests.get(&request.request_id), Some(&request));
        debug_assert_eq!(
            self.threads.get(&request.thread_id),
            Some(&SchedulerThreadState::WaitingHost {
                request_id: request.request_id,
            })
        );
        self.set_validated_state(request.thread_id, request.resume_state);
        let removed = self.host_requests.remove(&request.request_id);
        debug_assert_eq!(removed.as_ref(), Some(&request));
        let completion = RuntimeHostCompletion {
            request_id: request.request_id,
            thread_id: request.thread_id,
            service: request.service,
        };
        self.commit_event(RuntimeSchedulerEvent::HostCompleted(completion.clone()));
        completion
    }

    pub(super) fn host_request_ids_for_thread(&self, thread_id: RuntimeThreadId) -> Vec<u64> {
        self.host_requests
            .iter()
            .filter_map(|(&id, request)| (request.thread_id == thread_id).then_some(id))
            .collect()
    }

    /// Media resumes its COM continuation directly in the same handoff, so its
    /// actual completion notification is consumed here rather than by Wasm.
    pub(super) fn commit_consumed_host_completion(
        &mut self,
        request: RuntimeHostRequest,
    ) -> RuntimeHostCompletion {
        let completion = self.commit_host_completion(request);
        let index = self.events.iter().position(|event| {
            matches!(event, RuntimeSchedulerEvent::HostCompleted(actual) if *actual == completion)
        }).expect("newly committed host completion notification");
        self.events.remove(index);
        completion
    }

    pub(super) fn cancel_host_requests(&mut self, thread_id: RuntimeThreadId, request_ids: &[u64]) {
        for request_id in request_ids {
            let removed = self.host_requests.remove(request_id);
            debug_assert_eq!(
                removed.as_ref().map(|request| request.thread_id),
                Some(thread_id)
            );
        }
    }

    pub(super) fn has_host_request_for_thread(&self, thread_id: RuntimeThreadId) -> bool {
        self.host_requests
            .values()
            .any(|request| request.thread_id == thread_id)
    }

    pub(super) fn register_wait(
        &mut self,
        thread_id: RuntimeThreadId,
        object_id: RuntimeObjectId,
        handle: RuntimeHandle,
    ) -> Result<RuntimeWaitRegistration, RuntimeStateError> {
        self.waits.register(thread_id, object_id, handle)
    }

    pub(super) fn checked_host_wait_registration(
        &self,
        thread_id: RuntimeThreadId,
        object_id: RuntimeObjectId,
        handle: RuntimeHandle,
        timeout_deadline_time_100ns: u64,
        alertable: bool,
        pacing: RuntimeWaitableTimerPacing,
    ) -> Result<RuntimeWaitRegistration, RuntimeStateError> {
        self.waits.checked_registration(
            thread_id,
            object_id,
            handle,
            Some(timeout_deadline_time_100ns),
            alertable,
            Some(pacing),
        )
    }

    pub(super) fn publish_wait_registration(&mut self, plan: RuntimeWaitRegistration) {
        self.waits.publish_registration(plan);
    }

    pub(super) fn replace_timer_wait_pacing(
        &mut self,
        registration: RuntimeWaitRegistration,
        pacing: RuntimeWaitableTimerPacing,
    ) {
        self.waits.replace_timer_pacing(registration, pacing);
    }

    pub(super) fn signal_timer_waiters(
        &mut self,
        object_id: RuntimeObjectId,
        deadline_time_100ns: u64,
        manual_reset: bool,
    ) {
        self.waits
            .signal_timer_waiters(object_id, deadline_time_100ns, manual_reset);
    }

    pub(super) fn wait_registration_for_thread(
        &self,
        thread_id: RuntimeThreadId,
    ) -> Result<Option<RuntimeWaitRegistration>, RuntimeStateError> {
        self.waits.registration_for_thread(thread_id)
    }

    pub(super) fn has_wait_registration_for_thread(&self, thread_id: RuntimeThreadId) -> bool {
        self.waits.has_registration_for_thread(thread_id)
    }

    pub(super) fn wait_registration_for_cancellation(
        &self,
        thread_id: RuntimeThreadId,
    ) -> Result<Option<RuntimeWaitRegistration>, RuntimeStateError> {
        let registration = self.waits.registration_for_thread(thread_id)?;
        match (self.state(thread_id)?, registration) {
            (
                SchedulerThreadState::WaitingObject {
                    registration_id,
                    object_id,
                },
                Some(registration),
            ) => {
                if registration.id != registration_id || registration.object_id != object_id {
                    return Err(RuntimeStateError::new(
                        "scheduler wait state does not match the thread registration",
                    ));
                }
            }
            (SchedulerThreadState::WaitingObject { .. }, None) => {
                return Err(RuntimeStateError::new(
                    "waiting thread must own exactly one wait registration",
                ));
            }
            (SchedulerThreadState::WaitingHost { request_id }, Some(registration)) => {
                let pacing = registration.waitable_timer_pacing.ok_or_else(|| {
                    RuntimeStateError::new(
                        "host-waiting thread registration has no timer pacing contract",
                    )
                })?;
                let request = self.host_requests.get(&request_id).ok_or_else(|| {
                    RuntimeStateError::new(
                        "host-waiting timer registration has no scheduler request",
                    )
                })?;
                if pacing.request_id != request_id
                    || request.thread_id != thread_id
                    || request.service != RUNTIME_WAITABLE_TIMER_PACING_SERVICE
                {
                    return Err(RuntimeStateError::new(
                        "timer pacing wait registration does not match its scheduler request",
                    ));
                }
            }
            (_, None) => return Ok(None),
            _ => {
                return Err(RuntimeStateError::new(
                    "nonwaiting thread owns a wait registration",
                ));
            }
        }
        let registration = registration.expect("validated waiting registration");
        let queue = self
            .waits
            .object_waiters()
            .get(&registration.object_id)
            .ok_or_else(|| RuntimeStateError::new("wait registration has no object queue"))?;
        if queue.iter().filter(|&&id| id == registration.id).count() != 1 {
            return Err(RuntimeStateError::new(
                "wait registration must occur exactly once in its object queue",
            ));
        }
        Ok(Some(registration))
    }

    pub(super) fn wake_object_waiters(
        &mut self,
        threads: &RuntimeThreads,
        object_id: RuntimeObjectId,
        wait_state: RuntimeWaitState,
        excluded_threads: &[RuntimeThreadId],
        maximum_wakes: usize,
    ) -> Result<usize, RuntimeStateError> {
        let mut candidates = self
            .waits
            .plan_wakes(&[(object_id, wait_state)], excluded_threads)?;
        for candidate in &candidates {
            let registration = candidate.completion.registration;
            if threads.state(registration.thread_id)?.lifecycle
                != RuntimeThreadLifecycleState::Running
                || self.state(registration.thread_id)?
                    != (SchedulerThreadState::WaitingObject {
                        registration_id: registration.id,
                        object_id: registration.object_id,
                    })
            {
                return Err(RuntimeStateError::new(
                    "wait registration does not match a running waiting thread",
                ));
            }
            if !candidate.excluded {
                threads.callback_depth(registration.thread_id)?;
            }
        }
        candidates.retain(|candidate| !candidate.excluded);
        candidates.truncate(maximum_wakes);
        let count = candidates.len();
        for candidate in candidates {
            let completion = candidate.completion;
            let depth = threads
                .callback_depth(completion.registration.thread_id)
                .expect("validated waiting thread");
            let resumed_state = if depth == 0 {
                SchedulerThreadState::Runnable
            } else {
                SchedulerThreadState::InCallback { depth }
            };
            self.waits.commit_remove(completion.registration);
            self.set_validated_state(completion.registration.thread_id, resumed_state);
            self.commit_event(RuntimeSchedulerEvent::WaitSatisfied(completion));
        }
        Ok(count)
    }

    pub(super) fn commit_remove_wait_registration(
        &mut self,
        registration: RuntimeWaitRegistration,
    ) {
        self.waits.commit_remove(registration);
    }

    pub(super) fn commit_event(&mut self, event: RuntimeSchedulerEvent) {
        self.events.push_back(event);
    }

    pub(super) fn pop_event(&mut self) -> Option<RuntimeSchedulerEvent> {
        self.events.pop_front()
    }

    #[cfg(test)]
    pub(super) fn event_count_for_thread(&self, thread_id: RuntimeThreadId) -> usize {
        self.events
            .iter()
            .filter(|event| event.thread_id() == thread_id)
            .count()
    }

    pub(super) fn has_event_for_thread(&self, thread_id: RuntimeThreadId) -> bool {
        self.events
            .iter()
            .any(|event| event.thread_id() == thread_id)
    }

    pub(super) fn commit_remove_events_for_thread(&mut self, thread_id: RuntimeThreadId) {
        self.events.retain(|event| event.thread_id() != thread_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const INITIAL: RuntimeThreadId = RuntimeThreadId(0x2000);
    const SECOND: RuntimeThreadId = RuntimeThreadId(0x1001);

    #[test]
    fn resume_thread_decrements_only_suspended_lanes_and_preserves_waits() {
        let mut scheduler = RuntimeScheduler::new(INITIAL);
        scheduler.register(SECOND, SchedulerThreadState::Suspended { count: 2 });
        assert_eq!(scheduler.resume_thread(SECOND).unwrap(), 2);
        assert_eq!(
            scheduler.state(SECOND).unwrap(),
            SchedulerThreadState::Suspended { count: 1 }
        );
        assert_eq!(scheduler.resume_thread(SECOND).unwrap(), 1);
        assert_eq!(
            scheduler.state(SECOND).unwrap(),
            SchedulerThreadState::Runnable
        );
        for state in [
            SchedulerThreadState::Runnable,
            SchedulerThreadState::WaitingHost { request_id: 7 },
            SchedulerThreadState::WaitingObject {
                registration_id: RuntimeWaitRegistrationId(1),
                object_id: RuntimeObjectId(2),
            },
            SchedulerThreadState::WaitingCriticalSection { address: 0x1000 },
            SchedulerThreadState::WaitingMessage,
            SchedulerThreadState::InCallback { depth: 2 },
        ] {
            scheduler.threads.insert(SECOND, state);
            assert_eq!(scheduler.resume_thread(SECOND).unwrap(), 0);
            assert_eq!(scheduler.state(SECOND).unwrap(), state);
            assert_eq!(scheduler.current_thread_id(), INITIAL);
        }
        let invalid_state = SchedulerThreadState::Suspended { count: 0 };
        scheduler.threads.insert(SECOND, invalid_state);
        assert!(scheduler.resume_thread(SECOND).is_err());
        assert_eq!(scheduler.state(SECOND).unwrap(), invalid_state);
        for state in [
            SchedulerThreadState::Initialized,
            SchedulerThreadState::Terminating { exit_code: 7 },
            SchedulerThreadState::Terminated { exit_code: 7 },
        ] {
            scheduler.threads.insert(SECOND, state);
            assert_eq!(scheduler.resume_thread(SECOND).unwrap(), u32::MAX);
            assert_eq!(scheduler.state(SECOND).unwrap(), state);
            assert_eq!(scheduler.current_thread_id(), INITIAL);
        }
        assert!(scheduler.resume_thread(RuntimeThreadId(0)).is_err());
        assert_eq!(
            scheduler.state(SECOND).unwrap(),
            SchedulerThreadState::Terminated { exit_code: 7 }
        );
        assert_eq!(scheduler.threads.len(), 2);
        assert_eq!(scheduler.current_thread_id(), INITIAL);
    }

    #[test]
    fn lane_registration_and_selection_validate_before_mutation() {
        let mut scheduler = RuntimeScheduler::new(INITIAL);
        scheduler.validate_registration(SECOND).unwrap();
        assert!(!scheduler.contains_thread(SECOND));
        scheduler.register(SECOND, SchedulerThreadState::Runnable);

        scheduler.completed_api_boundaries_in_quantum = 3;
        assert!(scheduler.select_thread(RuntimeThreadId(0)).is_err());
        assert_eq!(scheduler.current_thread_id(), INITIAL);
        assert_eq!(scheduler.completed_api_boundaries_in_quantum, 3);
        assert_eq!(
            scheduler.state(SECOND).unwrap(),
            SchedulerThreadState::Runnable
        );
        scheduler.select_thread(SECOND).unwrap();
        assert_eq!(scheduler.current_thread_id(), SECOND);
    }

    #[test]
    fn api_boundaries_preserve_callback_lanes_and_reject_invalid_competitors() {
        let mut scheduler = RuntimeScheduler::new(INITIAL);
        scheduler.set_validated_state(INITIAL, SchedulerThreadState::Runnable);
        assert_eq!(
            scheduler
                .complete_api_boundary(RuntimeApiBoundaryDisposition::ContinueQuantum, None, false)
                .unwrap(),
            None
        );
        scheduler.validate_registration(SECOND).unwrap();
        scheduler.register(SECOND, SchedulerThreadState::Runnable);
        for _ in 1..RUNTIME_API_BOUNDARY_QUANTUM {
            assert_eq!(
                scheduler
                    .complete_api_boundary(
                        RuntimeApiBoundaryDisposition::ContinueQuantum,
                        Some(SECOND),
                        false
                    )
                    .unwrap(),
                None
            );
        }
        let quantum_before = scheduler.completed_api_boundaries_in_quantum;
        assert!(
            scheduler
                .complete_api_boundary(
                    RuntimeApiBoundaryDisposition::YieldCurrentThread,
                    Some(INITIAL),
                    false
                )
                .is_err()
        );
        assert_eq!(scheduler.current_thread_id(), INITIAL);
        assert_eq!(
            scheduler.completed_api_boundaries_in_quantum,
            quantum_before
        );
        assert_eq!(
            scheduler.state(INITIAL).unwrap(),
            SchedulerThreadState::Runnable
        );
        assert_eq!(
            scheduler.state(SECOND).unwrap(),
            SchedulerThreadState::Runnable
        );
        assert!(
            scheduler
                .complete_api_boundary(
                    RuntimeApiBoundaryDisposition::YieldCurrentThread,
                    Some(RuntimeThreadId(99)),
                    false
                )
                .is_err()
        );
        assert_eq!(scheduler.current_thread_id(), INITIAL);
        assert_eq!(
            scheduler.completed_api_boundaries_in_quantum,
            quantum_before
        );
        assert_eq!(
            scheduler.state(INITIAL).unwrap(),
            SchedulerThreadState::Runnable
        );
        assert_eq!(
            scheduler.state(SECOND).unwrap(),
            SchedulerThreadState::Runnable
        );
        scheduler.set_validated_state(INITIAL, SchedulerThreadState::InCallback { depth: 1 });
        assert_eq!(
            scheduler
                .complete_api_boundary(
                    RuntimeApiBoundaryDisposition::YieldCurrentThread,
                    Some(SECOND),
                    false
                )
                .unwrap(),
            Some(SECOND)
        );
        assert_eq!(scheduler.completed_api_boundaries_in_quantum, 0);
        assert_eq!(
            scheduler
                .complete_api_boundary(
                    RuntimeApiBoundaryDisposition::YieldCurrentThread,
                    Some(INITIAL),
                    false
                )
                .unwrap(),
            Some(INITIAL)
        );
        assert_eq!(
            scheduler.current_state().unwrap(),
            SchedulerThreadState::InCallback { depth: 1 }
        );
    }

    #[test]
    fn host_completion_resumes_its_owner_without_selecting_it() {
        let mut scheduler = RuntimeScheduler::new(INITIAL);
        scheduler.set_validated_state(INITIAL, SchedulerThreadState::Runnable);
        scheduler.validate_registration(SECOND).unwrap();
        scheduler.register(SECOND, SchedulerThreadState::Runnable);

        let request = scheduler
            .checked_host_request("initial.pending".to_owned())
            .unwrap();
        let request = scheduler.publish_host_request(request);
        scheduler.select_thread(SECOND).unwrap();

        let completion = scheduler
            .validate_host_completion(request.request_id)
            .expect("completion is owned by the non-current initial thread");
        scheduler.commit_host_completion(completion);
        assert_eq!(scheduler.current_thread_id(), SECOND);
        assert_eq!(scheduler.state(INITIAL), Ok(SchedulerThreadState::Runnable));
    }

    #[test]
    fn event_queue_preserves_fifo_and_thread_identity() {
        let mut scheduler = RuntimeScheduler::new(INITIAL);
        scheduler.commit_event(RuntimeSchedulerEvent::MessageReady(INITIAL));
        assert_eq!(scheduler.event_count_for_thread(INITIAL), 1);
        assert!(matches!(
            scheduler.pop_event(),
            Some(RuntimeSchedulerEvent::MessageReady(_))
        ));
        assert!(scheduler.events().is_empty());
    }

    #[test]
    fn host_request_plans_do_not_mutate_and_completion_commits_one_event() {
        let mut scheduler = RuntimeScheduler::new(INITIAL);
        scheduler.set_validated_state(INITIAL, SchedulerThreadState::Runnable);

        let start = scheduler
            .checked_host_request("browser.storage.read".to_owned())
            .unwrap();
        assert_eq!(
            scheduler.current_state(),
            Ok(SchedulerThreadState::Runnable)
        );
        assert!(scheduler.host_requests().is_empty());
        assert_eq!(scheduler.next_host_request_id(), 1);
        let request = scheduler.publish_host_request(start);
        assert_eq!(request.request_id, 1);
        assert_eq!(scheduler.host_requests().get(&1), Some(&request));
        assert_eq!(scheduler.next_host_request_id(), 2);
        assert_eq!(
            scheduler.current_state(),
            Ok(SchedulerThreadState::WaitingHost { request_id: 1 })
        );

        assert!(scheduler.validate_host_completion(2).is_err());
        assert_eq!(scheduler.host_requests().len(), 1);
        assert_eq!(scheduler.host_requests().get(&1), Some(&request));
        assert_eq!(scheduler.next_host_request_id(), 2);
        assert_eq!(scheduler.current_thread_id(), INITIAL);
        assert_eq!(
            scheduler.current_state().unwrap(),
            SchedulerThreadState::WaitingHost { request_id: 1 }
        );
        assert!(scheduler.events().is_empty());

        let completion = scheduler.validate_host_completion(1).unwrap();
        assert_eq!(scheduler.host_requests().get(&1), Some(&request));
        assert!(scheduler.events().is_empty());
        let completion = scheduler.commit_host_completion(completion);
        assert_eq!(completion.request_id, 1);
        assert!(scheduler.host_requests().is_empty());
        assert_eq!(
            scheduler.current_state(),
            Ok(SchedulerThreadState::Runnable)
        );
        assert_eq!(
            scheduler.pop_event(),
            Some(RuntimeSchedulerEvent::HostCompleted(completion))
        );
    }

    #[test]
    fn consumed_media_completion_preserves_unrelated_host_and_message_events() {
        let mut scheduler = RuntimeScheduler::new(INITIAL);
        scheduler.set_validated_state(INITIAL, SchedulerThreadState::Runnable);
        scheduler.register(SECOND, SchedulerThreadState::Runnable);
        let request = scheduler
            .checked_host_request("browser.sleep".to_owned())
            .unwrap();
        let request = scheduler.publish_host_request(request);
        let sleep = scheduler.commit_host_completion(request);
        scheduler.commit_event(RuntimeSchedulerEvent::MessageReady(INITIAL));
        scheduler.select_thread(SECOND).unwrap();
        let request = scheduler
            .checked_host_request("browser.media".to_owned())
            .unwrap();
        let request = scheduler.publish_host_request(request);
        let request = scheduler
            .validate_host_completion(request.request_id)
            .unwrap();
        let media = scheduler.commit_consumed_host_completion(request);
        assert_eq!(media.thread_id, SECOND);
        assert_eq!(
            scheduler.state(SECOND).unwrap(),
            SchedulerThreadState::Runnable
        );
        assert_eq!(
            scheduler.pop_event(),
            Some(RuntimeSchedulerEvent::HostCompleted(sleep))
        );
        assert_eq!(
            scheduler.pop_event(),
            Some(RuntimeSchedulerEvent::MessageReady(INITIAL))
        );
        assert_eq!(scheduler.pop_event(), None);
    }

    #[test]
    fn host_request_cancellation_preserves_other_thread_requests() {
        let mut scheduler = RuntimeScheduler::new(INITIAL);
        scheduler.set_validated_state(INITIAL, SchedulerThreadState::Runnable);
        scheduler.validate_registration(SECOND).unwrap();
        scheduler.register(SECOND, SchedulerThreadState::Runnable);

        let initial_request = scheduler
            .checked_host_request("initial.pending".to_owned())
            .unwrap();
        let initial_request = scheduler.publish_host_request(initial_request);
        scheduler.select_thread(SECOND).unwrap();
        let second_request = scheduler
            .checked_host_request("second.pending".to_owned())
            .unwrap();
        let second_request = scheduler.publish_host_request(second_request);

        let request_ids = scheduler.host_request_ids_for_thread(INITIAL);
        assert_eq!(request_ids, vec![initial_request.request_id]);
        scheduler.cancel_host_requests(INITIAL, &request_ids);
        assert!(
            !scheduler
                .host_requests()
                .contains_key(&initial_request.request_id)
        );
        assert_eq!(
            scheduler.host_requests().get(&second_request.request_id),
            Some(&second_request)
        );
    }
}

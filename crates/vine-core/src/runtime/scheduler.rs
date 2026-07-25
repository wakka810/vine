use std::collections::{BTreeMap, VecDeque};

use serde::Serialize;

use super::waits::{RuntimeWaitRegistrationPlan, RuntimeWaitWakeCandidate, RuntimeWaits};
use super::{
    RuntimeHandle, RuntimeMessageWake, RuntimeObjectId, RuntimeStateError, RuntimeThreadId,
    RuntimeWaitCompletion, RuntimeWaitRegistration, RuntimeWaitRegistrationId, RuntimeWaitState,
};

pub const RUNTIME_API_BOUNDARY_QUANTUM: u32 = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeHostRequest {
    pub request_id: u64,
    pub thread_id: RuntimeThreadId,
    pub service: String,
    pub resume_state: SchedulerThreadState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeHostCompletion {
    pub request_id: u64,
    pub thread_id: RuntimeThreadId,
    pub service: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeThreadExecutionSelection {
    pub previous_thread_id: RuntimeThreadId,
    pub thread_id: RuntimeThreadId,
    pub state: SchedulerThreadState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "event", content = "completion", rename_all = "kebab-case")]
pub enum RuntimeSchedulerEvent {
    HostCompleted(RuntimeHostCompletion),
    WaitSatisfied(RuntimeWaitCompletion),
    MessageReady(RuntimeMessageWake),
}

impl RuntimeSchedulerEvent {
    pub const fn thread_id(&self) -> RuntimeThreadId {
        match self {
            Self::HostCompleted(completion) => completion.thread_id,
            Self::WaitSatisfied(completion) => completion.registration.thread_id,
            Self::MessageReady(wake) => wake.thread_id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeScheduler {
    current_thread_id: RuntimeThreadId,
    threads: BTreeMap<RuntimeThreadId, SchedulerThreadState>,
    #[serde(skip)]
    completed_api_boundaries_in_quantum: u32,
    #[serde(skip)]
    api_boundary_revision: u64,
    #[serde(flatten)]
    waits: RuntimeWaits,
    events: VecDeque<RuntimeSchedulerEvent>,
    #[serde(skip)]
    host_requests: BTreeMap<u64, RuntimeHostRequest>,
    #[serde(skip)]
    next_host_request_id: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RuntimeSchedulerLanePlan {
    thread_id: RuntimeThreadId,
    previous: Option<SchedulerThreadState>,
    next: SchedulerThreadState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RuntimeSchedulerImmediateWaitPlan {
    registration: RuntimeWaitRegistrationPlan,
    previous_state: SchedulerThreadState,
}

impl RuntimeSchedulerImmediateWaitPlan {
    pub(super) const fn registration(&self) -> RuntimeWaitRegistration {
        self.registration.registration()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RuntimeSchedulerSelectionPlan {
    previous: RuntimeThreadId,
    next: RuntimeThreadId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RuntimeSchedulerApiBoundaryPlan {
    previous_revision: u64,
    next_revision: u64,
    previous_thread_id: RuntimeThreadId,
    previous_thread_state: SchedulerThreadState,
    runnable_competitor: Option<RuntimeThreadId>,
    previous_completed_boundaries: u32,
    next_completed_boundaries: u32,
    selection: Option<RuntimeSchedulerSelectionPlan>,
}

impl RuntimeSchedulerSelectionPlan {
    pub(super) const fn previous(self) -> RuntimeThreadId {
        self.previous
    }

    pub(super) const fn next(self) -> RuntimeThreadId {
        self.next
    }
}

impl RuntimeSchedulerApiBoundaryPlan {
    pub(super) const fn selection(self) -> Option<RuntimeSchedulerSelectionPlan> {
        self.selection
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeHostRequestPlan {
    previous_next_request_id: u64,
    next_request_id: u64,
    request: RuntimeHostRequest,
    scheduler: RuntimeSchedulerLanePlan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeHostCompletionPlan {
    request: RuntimeHostRequest,
    completion: RuntimeHostCompletion,
    scheduler: RuntimeSchedulerLanePlan,
}

impl RuntimeHostRequestPlan {
    pub(super) fn request(&self) -> &RuntimeHostRequest {
        &self.request
    }
}

impl RuntimeHostCompletionPlan {
    pub(super) fn request(&self) -> &RuntimeHostRequest {
        &self.request
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeHostRequestCancellationPlan {
    thread_id: RuntimeThreadId,
    request_ids: Vec<u64>,
}

impl RuntimeHostRequestCancellationPlan {
    pub(super) fn request_count(&self) -> usize {
        self.request_ids.len()
    }

    pub(super) fn request_ids(&self) -> &[u64] {
        &self.request_ids
    }
}

impl RuntimeSchedulerLanePlan {
    pub(super) const fn transition_after(self, next: SchedulerThreadState) -> Self {
        Self {
            thread_id: self.thread_id,
            previous: Some(self.next),
            next,
        }
    }
}

impl RuntimeScheduler {
    pub(super) fn new(initial_thread_id: RuntimeThreadId) -> Self {
        Self {
            current_thread_id: initial_thread_id,
            threads: BTreeMap::from([(initial_thread_id, SchedulerThreadState::Initialized)]),
            completed_api_boundaries_in_quantum: 0,
            api_boundary_revision: 0,
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

    pub(super) fn next_host_request_id(&self) -> u64 {
        self.next_host_request_id
    }

    pub(super) fn next_wait_registration_id(&self) -> u64 {
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
            RuntimeStateError::runtime_defect(
                "runtime-scheduler",
                "thread-scheduler-state-missing",
                format!("runtime thread ID {} has no scheduler state", thread_id.0),
            )
        })
    }

    pub(super) fn current_state(&self) -> Result<SchedulerThreadState, RuntimeStateError> {
        self.state(self.current_thread_id)
    }

    pub(super) fn plan_registration(
        &self,
        thread_id: RuntimeThreadId,
        state: SchedulerThreadState,
    ) -> Result<RuntimeSchedulerLanePlan, RuntimeStateError> {
        if self.threads.contains_key(&thread_id) {
            return Err(RuntimeStateError::conflict(
                "runtime-scheduler",
                "thread-scheduler-state-duplicate",
                format!(
                    "runtime thread ID {} already has a scheduler lane",
                    thread_id.0
                ),
            ));
        }
        Ok(RuntimeSchedulerLanePlan {
            thread_id,
            previous: None,
            next: state,
        })
    }

    pub(super) fn plan_transition(
        &self,
        thread_id: RuntimeThreadId,
        next: SchedulerThreadState,
    ) -> Result<RuntimeSchedulerLanePlan, RuntimeStateError> {
        Ok(RuntimeSchedulerLanePlan {
            thread_id,
            previous: Some(self.state(thread_id)?),
            next,
        })
    }

    pub(super) fn plan_current_transition(
        &self,
        next: SchedulerThreadState,
    ) -> Result<RuntimeSchedulerLanePlan, RuntimeStateError> {
        self.plan_transition(self.current_thread_id, next)
    }

    pub(super) fn commit_lane(&mut self, plan: RuntimeSchedulerLanePlan) {
        debug_assert_eq!(self.threads.get(&plan.thread_id).copied(), plan.previous);
        let previous = self.threads.insert(plan.thread_id, plan.next);
        debug_assert_eq!(previous, plan.previous);
    }

    pub(super) fn preflight_lane_commit(
        &self,
        plan: &RuntimeSchedulerLanePlan,
    ) -> Result<(), RuntimeStateError> {
        if self.threads.get(&plan.thread_id).copied() != plan.previous {
            return Err(RuntimeStateError::conflict(
                "runtime-scheduler",
                "thread-scheduler-plan-stale",
                "runtime scheduler lane changed after transition preflight",
            ));
        }
        Ok(())
    }

    pub(super) fn plan_selection(
        &self,
        thread_id: RuntimeThreadId,
    ) -> Result<RuntimeSchedulerSelectionPlan, RuntimeStateError> {
        self.state(thread_id)?;
        Ok(RuntimeSchedulerSelectionPlan {
            previous: self.current_thread_id,
            next: thread_id,
        })
    }

    pub(super) fn plan_next_executable_selection(
        &self,
    ) -> Result<Option<RuntimeSchedulerSelectionPlan>, RuntimeStateError> {
        let thread_ids = self.threads.keys().copied().collect::<Vec<_>>();
        let current_index = thread_ids
            .iter()
            .position(|thread_id| *thread_id == self.current_thread_id)
            .ok_or_else(|| {
                RuntimeStateError::runtime_defect(
                    "runtime-scheduler",
                    "current-thread-scheduler-state-missing",
                    "current runtime thread has no scheduler lane",
                )
            })?;
        for offset in 1..=thread_ids.len() {
            let thread_id = thread_ids[(current_index + offset) % thread_ids.len()];
            if self.state(thread_id)?.is_executable() {
                return self.plan_selection(thread_id).map(Some);
            }
        }
        Ok(None)
    }

    fn plan_next_other_runnable_selection(
        &self,
    ) -> Result<Option<RuntimeSchedulerSelectionPlan>, RuntimeStateError> {
        let thread_ids = self.threads.keys().copied().collect::<Vec<_>>();
        let current_index = thread_ids
            .iter()
            .position(|thread_id| *thread_id == self.current_thread_id)
            .ok_or_else(|| {
                RuntimeStateError::runtime_defect(
                    "runtime-scheduler",
                    "current-thread-scheduler-state-missing",
                    "current runtime thread has no scheduler lane",
                )
            })?;
        for offset in 1..thread_ids.len() {
            let thread_id = thread_ids[(current_index + offset) % thread_ids.len()];
            if self.state(thread_id)? == SchedulerThreadState::Runnable {
                return self.plan_selection(thread_id).map(Some);
            }
        }
        Ok(None)
    }

    pub(super) fn plan_completed_api_boundary(
        &self,
    ) -> Result<RuntimeSchedulerApiBoundaryPlan, RuntimeStateError> {
        let current_state = self.current_state()?;
        let next_revision = self.api_boundary_revision.checked_add(1).ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-scheduler",
                "api-boundary-revision-exhausted",
                "API execution boundary plan revision is exhausted",
            )
        })?;
        let next_runnable = self.plan_next_other_runnable_selection()?;
        let runnable_competitor = next_runnable.map(|selection| selection.next);
        if matches!(current_state, SchedulerThreadState::InCallback { .. }) {
            return Ok(RuntimeSchedulerApiBoundaryPlan {
                previous_revision: self.api_boundary_revision,
                next_revision,
                previous_thread_id: self.current_thread_id,
                previous_thread_state: current_state,
                runnable_competitor,
                previous_completed_boundaries: self.completed_api_boundaries_in_quantum,
                next_completed_boundaries: self.completed_api_boundaries_in_quantum,
                selection: None,
            });
        }
        if current_state != SchedulerThreadState::Runnable {
            return Err(RuntimeStateError::conflict(
                "runtime-scheduler",
                "api-boundary-thread-not-executable",
                "an API execution boundary requires an executable current thread",
            ));
        }
        let Some(next_runnable) = next_runnable else {
            return Ok(RuntimeSchedulerApiBoundaryPlan {
                previous_revision: self.api_boundary_revision,
                next_revision,
                previous_thread_id: self.current_thread_id,
                previous_thread_state: current_state,
                runnable_competitor,
                previous_completed_boundaries: self.completed_api_boundaries_in_quantum,
                next_completed_boundaries: 0,
                selection: None,
            });
        };
        let completed = self
            .completed_api_boundaries_in_quantum
            .checked_add(1)
            .ok_or_else(|| {
                RuntimeStateError::resource_limit(
                    "runtime-scheduler",
                    "api-boundary-counter-overflow",
                    "API execution boundary counter is exhausted",
                )
            })?;
        let quantum_expired = completed >= RUNTIME_API_BOUNDARY_QUANTUM;
        Ok(RuntimeSchedulerApiBoundaryPlan {
            previous_revision: self.api_boundary_revision,
            next_revision,
            previous_thread_id: self.current_thread_id,
            previous_thread_state: current_state,
            runnable_competitor,
            previous_completed_boundaries: self.completed_api_boundaries_in_quantum,
            next_completed_boundaries: if quantum_expired { 0 } else { completed },
            selection: quantum_expired.then_some(next_runnable),
        })
    }

    pub(super) fn preflight_completed_api_boundary(
        &self,
        plan: &RuntimeSchedulerApiBoundaryPlan,
    ) -> Result<(), RuntimeStateError> {
        let runnable_competitor = self
            .plan_next_other_runnable_selection()?
            .map(|selection| selection.next);
        if self.api_boundary_revision != plan.previous_revision
            || self.current_thread_id != plan.previous_thread_id
            || self.current_state()? != plan.previous_thread_state
            || runnable_competitor != plan.runnable_competitor
            || self.completed_api_boundaries_in_quantum != plan.previous_completed_boundaries
            || plan.selection.is_some_and(|selection| {
                selection.previous != plan.previous_thread_id
                    || Some(selection.next) != plan.runnable_competitor
            })
        {
            return Err(RuntimeStateError::conflict(
                "runtime-scheduler",
                "api-boundary-plan-stale",
                "runtime scheduler changed after API boundary preflight",
            ));
        }
        if let Some(selection) = plan.selection
            && (selection.next == self.current_thread_id
                || self.state(selection.next)? != SchedulerThreadState::Runnable)
        {
            return Err(RuntimeStateError::conflict(
                "runtime-scheduler",
                "api-boundary-selection-stale",
                "API boundary target is no longer another executable thread",
            ));
        }
        Ok(())
    }

    pub(super) fn commit_completed_api_boundary(&mut self, plan: RuntimeSchedulerApiBoundaryPlan) {
        debug_assert_eq!(self.api_boundary_revision, plan.previous_revision);
        debug_assert_eq!(self.current_thread_id, plan.previous_thread_id);
        debug_assert_eq!(self.current_state().ok(), Some(plan.previous_thread_state));
        debug_assert_eq!(
            self.completed_api_boundaries_in_quantum,
            plan.previous_completed_boundaries
        );
        if let Some(selection) = plan.selection {
            debug_assert_eq!(self.current_thread_id, selection.previous);
            self.current_thread_id = selection.next;
        }
        self.completed_api_boundaries_in_quantum = plan.next_completed_boundaries;
        self.api_boundary_revision = plan.next_revision;
    }

    pub(super) fn commit_selection(&mut self, plan: RuntimeSchedulerSelectionPlan) {
        debug_assert_eq!(self.current_thread_id, plan.previous);
        self.current_thread_id = plan.next;
        self.completed_api_boundaries_in_quantum = 0;
    }

    pub(super) fn plan_host_request(
        &self,
        service: String,
    ) -> Result<RuntimeHostRequestPlan, RuntimeStateError> {
        if service.is_empty() {
            return Err(RuntimeStateError::invalid_input(
                "runtime-host-request",
                "host-service-empty",
                "host request requires a nonempty service identity",
            ));
        }
        let resume_state = match self.current_state()? {
            state @ (SchedulerThreadState::Runnable | SchedulerThreadState::InCallback { .. }) => {
                state
            }
            _ => {
                return Err(RuntimeStateError::conflict(
                    "runtime-host-request",
                    "thread-not-runnable",
                    "only a runnable guest thread or active callback can begin a host request",
                ));
            }
        };
        let request_id = self.next_host_request_id;
        let next_request_id = request_id.checked_add(1).ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-host-request",
                "host-request-id-overflow",
                "host request ID space is exhausted",
            )
        })?;
        if self.host_requests.contains_key(&request_id) {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-host-request",
                "host-request-id-duplicate",
                format!("next host request ID {request_id} is already pending"),
            ));
        }
        let request = RuntimeHostRequest {
            request_id,
            thread_id: self.current_thread_id,
            service,
            resume_state,
        };
        let scheduler =
            self.plan_current_transition(SchedulerThreadState::WaitingHost { request_id })?;
        Ok(RuntimeHostRequestPlan {
            previous_next_request_id: request_id,
            next_request_id,
            request,
            scheduler,
        })
    }

    pub(super) fn commit_host_request(
        &mut self,
        plan: RuntimeHostRequestPlan,
    ) -> RuntimeHostRequest {
        debug_assert_eq!(self.next_host_request_id, plan.previous_next_request_id);
        debug_assert!(!self.host_requests.contains_key(&plan.request.request_id));
        self.next_host_request_id = plan.next_request_id;
        let request = plan.request;
        let previous = self
            .host_requests
            .insert(request.request_id, request.clone());
        debug_assert!(previous.is_none());
        self.commit_lane(plan.scheduler);
        request
    }

    pub(super) fn plan_host_completion(
        &self,
        request_id: u64,
    ) -> Result<RuntimeHostCompletionPlan, RuntimeStateError> {
        let request = self
            .host_requests
            .get(&request_id)
            .cloned()
            .ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-host-request",
                    "host-request-id-unknown",
                    format!("host request ID {request_id} is not pending"),
                )
            })?;
        if self.state(request.thread_id)? != (SchedulerThreadState::WaitingHost { request_id }) {
            return Err(RuntimeStateError::conflict(
                "runtime-host-request",
                "host-completion-thread-state-mismatch",
                "host completion does not match its owner thread wait state",
            ));
        }
        let scheduler = self.plan_transition(request.thread_id, request.resume_state)?;
        let completion = RuntimeHostCompletion {
            request_id,
            thread_id: request.thread_id,
            service: request.service.clone(),
        };
        Ok(RuntimeHostCompletionPlan {
            request,
            completion,
            scheduler,
        })
    }

    pub(super) fn commit_host_completion(
        &mut self,
        plan: RuntimeHostCompletionPlan,
    ) -> RuntimeHostCompletion {
        debug_assert_eq!(
            self.host_requests.get(&plan.request.request_id),
            Some(&plan.request)
        );
        let removed = self.host_requests.remove(&plan.request.request_id);
        debug_assert_eq!(removed, Some(plan.request));
        self.commit_lane(plan.scheduler);
        let completion = plan.completion;
        self.commit_event(RuntimeSchedulerEvent::HostCompleted(completion.clone()));
        completion
    }

    pub(super) fn plan_host_request_cancellation(
        &self,
        thread_id: RuntimeThreadId,
    ) -> RuntimeHostRequestCancellationPlan {
        RuntimeHostRequestCancellationPlan {
            thread_id,
            request_ids: self
                .host_requests
                .iter()
                .filter_map(|(&request_id, request)| {
                    (request.thread_id == thread_id).then_some(request_id)
                })
                .collect(),
        }
    }

    pub(super) fn commit_host_request_cancellation(
        &mut self,
        plan: RuntimeHostRequestCancellationPlan,
    ) {
        for request_id in plan.request_ids {
            let removed = self.host_requests.remove(&request_id);
            debug_assert_eq!(
                removed.as_ref().map(|request| request.thread_id),
                Some(plan.thread_id)
            );
        }
    }

    pub(super) fn has_host_request_for_thread(&self, thread_id: RuntimeThreadId) -> bool {
        self.host_requests
            .values()
            .any(|request| request.thread_id == thread_id)
    }

    pub(super) fn plan_wait_registration(
        &self,
        thread_id: RuntimeThreadId,
        object_id: RuntimeObjectId,
        handle: RuntimeHandle,
    ) -> Result<RuntimeWaitRegistrationPlan, RuntimeStateError> {
        self.waits.plan_registration(thread_id, object_id, handle)
    }

    pub(super) fn plan_immediate_wait(
        &self,
        thread_id: RuntimeThreadId,
        object_id: RuntimeObjectId,
        handle: RuntimeHandle,
        timeout_deadline_time_100ns: u64,
        alertable: bool,
    ) -> Result<RuntimeSchedulerImmediateWaitPlan, RuntimeStateError> {
        if self.current_thread_id != thread_id {
            return Err(RuntimeStateError::conflict(
                "runtime-wait",
                "immediate-wait-thread-not-current",
                "immediate wait requires the selected runtime thread",
            ));
        }
        let previous_state = self.state(thread_id)?;
        if !previous_state.is_executable() {
            return Err(RuntimeStateError::conflict(
                "runtime-wait",
                "immediate-wait-thread-state-invalid",
                "immediate wait requires an executable scheduler lane",
            ));
        }
        let registration = self.waits.plan_registration_with_options(
            thread_id,
            object_id,
            handle,
            Some(timeout_deadline_time_100ns),
            alertable,
        )?;
        Ok(RuntimeSchedulerImmediateWaitPlan {
            registration,
            previous_state,
        })
    }

    pub(super) fn preflight_immediate_wait(
        &self,
        plan: &RuntimeSchedulerImmediateWaitPlan,
    ) -> Result<(), RuntimeStateError> {
        let registration = plan.registration();
        if self.current_thread_id != registration.thread_id
            || self.state(registration.thread_id)? != plan.previous_state
            || !plan.previous_state.is_executable()
        {
            return Err(RuntimeStateError::conflict(
                "runtime-wait",
                "immediate-wait-plan-stale",
                "scheduler selection or lane changed after immediate wait planning",
            ));
        }
        self.waits.preflight_registration(&plan.registration)
    }

    pub(super) fn commit_immediate_wait_begin(
        &mut self,
        plan: RuntimeSchedulerImmediateWaitPlan,
    ) -> RuntimeWaitRegistration {
        debug_assert!(
            self.preflight_immediate_wait(&plan).is_ok(),
            "immediate wait plan became stale"
        );
        let registration = self.waits.commit_registration(plan.registration);
        let previous = self.threads.insert(
            registration.thread_id,
            SchedulerThreadState::WaitingObject {
                registration_id: registration.id,
                object_id: registration.object_id,
            },
        );
        debug_assert_eq!(previous, Some(plan.previous_state));
        registration
    }

    pub(super) fn commit_immediate_wait_complete(
        &mut self,
        plan: RuntimeSchedulerImmediateWaitPlan,
    ) -> RuntimeWaitRegistration {
        let registration = plan.registration();
        debug_assert_eq!(
            self.threads.get(&registration.thread_id).copied(),
            Some(SchedulerThreadState::WaitingObject {
                registration_id: registration.id,
                object_id: registration.object_id,
            })
        );
        debug_assert_eq!(
            self.waits.registration_for_thread(registration.thread_id),
            Ok(Some(registration))
        );
        self.waits.commit_remove(registration);
        let previous = self
            .threads
            .insert(registration.thread_id, plan.previous_state);
        debug_assert_eq!(
            previous,
            Some(SchedulerThreadState::WaitingObject {
                registration_id: registration.id,
                object_id: registration.object_id,
            })
        );
        registration
    }

    pub(super) fn commit_wait_registration(&mut self, plan: RuntimeWaitRegistrationPlan) {
        self.waits.commit_registration(plan);
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

    pub(super) fn plan_waiter_wakes(
        &self,
        wait_states: &[(RuntimeObjectId, RuntimeWaitState)],
        excluded_threads: &[RuntimeThreadId],
    ) -> Result<Vec<RuntimeWaitWakeCandidate>, RuntimeStateError> {
        self.waits.plan_wakes(wait_states, excluded_threads)
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
    fn lane_and_selection_plans_do_not_mutate_before_commit() {
        let mut scheduler = RuntimeScheduler::new(INITIAL);
        let registration = scheduler
            .plan_registration(SECOND, SchedulerThreadState::Runnable)
            .unwrap();
        assert!(!scheduler.contains_thread(SECOND));
        scheduler.commit_lane(registration);

        let selection = scheduler.plan_selection(SECOND).unwrap();
        assert_eq!(scheduler.current_thread_id(), INITIAL);
        scheduler.commit_selection(selection);
        assert_eq!(scheduler.current_thread_id(), SECOND);
    }

    #[test]
    fn api_boundary_plan_preserves_callback_atomicity_and_rejects_stale_commit() {
        let mut scheduler = RuntimeScheduler::new(INITIAL);
        let initial_running = scheduler
            .plan_current_transition(SchedulerThreadState::Runnable)
            .unwrap();
        scheduler.commit_lane(initial_running);
        let no_competitor = scheduler.plan_completed_api_boundary().unwrap();
        let second_running = scheduler
            .plan_registration(SECOND, SchedulerThreadState::Runnable)
            .unwrap();
        scheduler.commit_lane(second_running);
        let before_runnable_set_stale = scheduler.clone();
        assert_eq!(
            scheduler
                .preflight_completed_api_boundary(&no_competitor)
                .expect_err("a plan cannot cross a runnable-set change")
                .code,
            "api-boundary-plan-stale"
        );
        assert_eq!(scheduler, before_runnable_set_stale);

        for _ in 1..RUNTIME_API_BOUNDARY_QUANTUM {
            let plan = scheduler.plan_completed_api_boundary().unwrap();
            scheduler.preflight_completed_api_boundary(&plan).unwrap();
            scheduler.commit_completed_api_boundary(plan);
        }
        assert_eq!(
            scheduler.completed_api_boundaries_in_quantum,
            RUNTIME_API_BOUNDARY_QUANTUM - 1
        );

        let enter_callback = scheduler
            .plan_current_transition(SchedulerThreadState::InCallback { depth: 1 })
            .unwrap();
        scheduler.commit_lane(enter_callback);
        let callback_boundary = scheduler.plan_completed_api_boundary().unwrap();
        assert_eq!(callback_boundary.selection(), None);
        scheduler
            .preflight_completed_api_boundary(&callback_boundary)
            .unwrap();
        scheduler.commit_completed_api_boundary(callback_boundary);
        let before_duplicate_preflight = scheduler.clone();
        assert_eq!(
            scheduler
                .preflight_completed_api_boundary(&callback_boundary)
                .expect_err("an API boundary plan is single-use")
                .code,
            "api-boundary-plan-stale"
        );
        assert_eq!(scheduler, before_duplicate_preflight);
        assert_eq!(scheduler.current_thread_id(), INITIAL);
        assert_eq!(
            scheduler.completed_api_boundaries_in_quantum,
            RUNTIME_API_BOUNDARY_QUANTUM - 1
        );

        let leave_callback = scheduler
            .plan_current_transition(SchedulerThreadState::Runnable)
            .unwrap();
        scheduler.commit_lane(leave_callback);
        let expired = scheduler.plan_completed_api_boundary().unwrap();
        assert_eq!(expired.selection().unwrap().next(), SECOND);
        assert_eq!(scheduler.current_thread_id(), INITIAL);

        let explicit_selection = scheduler.plan_selection(SECOND).unwrap();
        scheduler.commit_selection(explicit_selection);
        let before_stale_preflight = scheduler.clone();
        assert_eq!(
            scheduler
                .preflight_completed_api_boundary(&expired)
                .expect_err("a plan cannot cross an explicit selection")
                .code,
            "api-boundary-plan-stale"
        );
        assert_eq!(scheduler, before_stale_preflight);
    }

    #[test]
    fn next_executable_selection_wraps_and_skips_waiting_lanes() {
        let mut scheduler = RuntimeScheduler::new(INITIAL);
        let initial_running = scheduler
            .plan_current_transition(SchedulerThreadState::Runnable)
            .unwrap();
        scheduler.commit_lane(initial_running);
        let second_running = scheduler
            .plan_registration(SECOND, SchedulerThreadState::Runnable)
            .unwrap();
        scheduler.commit_lane(second_running);

        let request = scheduler
            .plan_host_request("initial.pending".to_owned())
            .unwrap();
        let request = scheduler.commit_host_request(request);
        let selection = scheduler
            .plan_next_executable_selection()
            .unwrap()
            .expect("second thread is executable");
        assert_eq!(selection.previous(), INITIAL);
        assert_eq!(selection.next(), SECOND);
        scheduler.commit_selection(selection);

        let completion = scheduler
            .plan_host_completion(request.request_id)
            .expect("completion is owned by the non-current initial thread");
        scheduler.commit_host_completion(completion);
        assert_eq!(scheduler.current_thread_id(), SECOND);
        assert_eq!(scheduler.state(INITIAL), Ok(SchedulerThreadState::Runnable));
        assert_eq!(
            scheduler
                .plan_next_executable_selection()
                .unwrap()
                .expect("round robin wraps to initial")
                .next(),
            INITIAL
        );
    }

    #[test]
    fn lane_plan_is_mutation_free_until_commit() {
        let mut scheduler = RuntimeScheduler::new(INITIAL);
        let running = scheduler
            .plan_current_transition(SchedulerThreadState::Runnable)
            .unwrap();
        assert_eq!(
            scheduler.current_state(),
            Ok(SchedulerThreadState::Initialized)
        );
        scheduler.commit_lane(running);
        assert_eq!(
            scheduler.current_state(),
            Ok(SchedulerThreadState::Runnable)
        );
    }

    #[test]
    fn immediate_wait_roundtrip_commits_one_registration_and_restores_the_lane() {
        let mut scheduler = RuntimeScheduler::new(INITIAL);
        let running = scheduler
            .plan_current_transition(SchedulerThreadState::Runnable)
            .unwrap();
        scheduler.commit_lane(running);
        let handle =
            RuntimeHandle::from_parts(super::super::RuntimeObjectKind::WaitableTimer, 1, 7)
                .unwrap();
        let before_plan = scheduler.clone();
        let plan = scheduler
            .plan_immediate_wait(INITIAL, RuntimeObjectId(7), handle, 100_000, true)
            .unwrap();
        let registration = plan.registration();
        assert_eq!(scheduler, before_plan);
        assert_eq!(registration.id, RuntimeWaitRegistrationId(1));
        assert_eq!(registration.thread_id, INITIAL);
        assert_eq!(registration.object_id, RuntimeObjectId(7));
        assert_eq!(registration.handle, handle);
        assert_eq!(registration.timeout_deadline_time_100ns, Some(100_000));
        assert!(registration.alertable);
        scheduler.preflight_immediate_wait(&plan).unwrap();

        assert_eq!(scheduler.commit_immediate_wait_begin(plan), registration);
        assert_eq!(
            scheduler.state(INITIAL),
            Ok(SchedulerThreadState::WaitingObject {
                registration_id: RuntimeWaitRegistrationId(1),
                object_id: RuntimeObjectId(7),
            })
        );
        assert_eq!(
            scheduler.wait_registrations().get(&registration.id),
            Some(&registration)
        );
        assert_eq!(
            serde_json::to_value(&scheduler).unwrap()["waitRegistrations"]["1"],
            serde_json::json!({
                "id": 1,
                "threadId": 8192,
                "objectId": 7,
                "handle": handle.0,
                "timeoutDeadlineTime100ns": 100_000,
                "alertable": true,
            })
        );

        assert_eq!(scheduler.commit_immediate_wait_complete(plan), registration);
        assert_eq!(scheduler.state(INITIAL), Ok(SchedulerThreadState::Runnable));
        assert!(scheduler.wait_registrations().is_empty());
        assert!(scheduler.object_waiters().is_empty());
        assert_eq!(scheduler.next_wait_registration_id(), 2);
        let before_stale = scheduler.clone();
        assert_eq!(
            scheduler.preflight_immediate_wait(&plan).unwrap_err().code,
            "wait-registration-plan-stale"
        );
        assert_eq!(scheduler, before_stale);
    }

    #[test]
    fn event_queue_preserves_fifo_and_thread_identity() {
        let mut scheduler = RuntimeScheduler::new(INITIAL);
        scheduler.commit_event(RuntimeSchedulerEvent::MessageReady(RuntimeMessageWake {
            thread_id: INITIAL,
            message_id: super::super::RuntimeMessageId(1),
            queue_depth: 1,
        }));
        assert_eq!(scheduler.event_count_for_thread(INITIAL), 1);
        assert!(matches!(
            scheduler.pop_event(),
            Some(RuntimeSchedulerEvent::MessageReady(_))
        ));
        assert!(scheduler.events().is_empty());
    }

    #[test]
    fn serialization_preserves_the_existing_scheduler_shape() {
        let scheduler = RuntimeScheduler::new(INITIAL);
        assert_eq!(
            serde_json::to_value(scheduler).unwrap(),
            serde_json::json!({
                "currentThreadId": 8192,
                "threads": {"8192": {"state": "initialized"}},
                "waitRegistrations": {},
                "objectWaiters": {},
                "events": [],
            })
        );
    }

    #[test]
    fn host_request_plans_do_not_mutate_and_completion_commits_one_event() {
        let mut scheduler = RuntimeScheduler::new(INITIAL);
        let running = scheduler
            .plan_current_transition(SchedulerThreadState::Runnable)
            .unwrap();
        scheduler.commit_lane(running);

        let start = scheduler
            .plan_host_request("browser.storage.read".to_owned())
            .unwrap();
        assert_eq!(
            scheduler.current_state(),
            Ok(SchedulerThreadState::Runnable)
        );
        assert!(scheduler.host_requests().is_empty());
        assert_eq!(scheduler.next_host_request_id(), 1);
        let request = scheduler.commit_host_request(start);
        assert_eq!(request.request_id, 1);
        assert_eq!(scheduler.host_requests().get(&1), Some(&request));
        assert_eq!(scheduler.next_host_request_id(), 2);
        assert_eq!(
            scheduler.current_state(),
            Ok(SchedulerThreadState::WaitingHost { request_id: 1 })
        );

        let before_unknown = scheduler.clone();
        assert_eq!(
            scheduler
                .plan_host_completion(2)
                .expect_err("unknown completion must not mutate")
                .code,
            "host-request-id-unknown"
        );
        assert_eq!(scheduler, before_unknown);

        let completion = scheduler.plan_host_completion(1).unwrap();
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
    fn host_request_cancellation_plan_is_thread_scoped_and_mutation_free() {
        let mut scheduler = RuntimeScheduler::new(INITIAL);
        let initial_running = scheduler
            .plan_current_transition(SchedulerThreadState::Runnable)
            .unwrap();
        scheduler.commit_lane(initial_running);
        let second_registration = scheduler
            .plan_registration(SECOND, SchedulerThreadState::Runnable)
            .unwrap();
        scheduler.commit_lane(second_registration);

        let initial_request = scheduler
            .plan_host_request("initial.pending".to_owned())
            .unwrap();
        let initial_request = scheduler.commit_host_request(initial_request);
        let select_second = scheduler.plan_selection(SECOND).unwrap();
        scheduler.commit_selection(select_second);
        let second_request = scheduler
            .plan_host_request("second.pending".to_owned())
            .unwrap();
        let second_request = scheduler.commit_host_request(second_request);

        let before_plan = scheduler.clone();
        let cancellation = scheduler.plan_host_request_cancellation(INITIAL);
        assert_eq!(cancellation.request_count(), 1);
        assert_eq!(scheduler, before_plan);
        scheduler.commit_host_request_cancellation(cancellation);
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

use std::collections::{BTreeMap, VecDeque};

use serde::Serialize;

use super::waits::{RuntimeWaitRegistrationPlan, RuntimeWaitWakeCandidate, RuntimeWaits};
use super::{
    RuntimeHostCompletion, RuntimeMessageWake, RuntimeObjectId, RuntimeStateError, RuntimeThreadId,
    RuntimeWaitCompletion, RuntimeWaitRegistration, RuntimeWaitRegistrationId, RuntimeWaitState,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum SchedulerThreadState {
    Initialized,
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
    #[serde(flatten)]
    waits: RuntimeWaits,
    events: VecDeque<RuntimeSchedulerEvent>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RuntimeSchedulerLanePlan {
    thread_id: RuntimeThreadId,
    previous: Option<SchedulerThreadState>,
    next: SchedulerThreadState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RuntimeSchedulerSelectionPlan {
    previous: RuntimeThreadId,
    next: RuntimeThreadId,
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
            waits: RuntimeWaits::default(),
            events: VecDeque::new(),
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

    pub(super) fn commit_selection(&mut self, plan: RuntimeSchedulerSelectionPlan) {
        debug_assert_eq!(self.current_thread_id, plan.previous);
        self.current_thread_id = plan.next;
    }

    pub(super) fn plan_wait_registration(
        &self,
        registration: RuntimeWaitRegistration,
    ) -> Result<RuntimeWaitRegistrationPlan, RuntimeStateError> {
        self.waits.plan_registration(registration)
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
}

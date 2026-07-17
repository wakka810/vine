use std::collections::BTreeMap;

use serde::Serialize;

use super::{RuntimeHandle, RuntimeStateError};

pub(crate) const RUNTIME_TIMER_RESOLUTION_DISTINCT_PERIOD_LIMIT: usize = 64;
pub const RUNTIME_WAITABLE_TIMER_LIMIT: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct RuntimeWaitableTimerId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeWaitableTimer {
    pub id: RuntimeWaitableTimerId,
    pub manual_reset: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeCreateWaitableTimerAResult {
    pub timer_id: RuntimeWaitableTimerId,
    pub handle: RuntimeHandle,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeTimerResolutionState {
    pub request_counts: BTreeMap<u32, u32>,
    pub total_request_count: u64,
    pub effective_period_milliseconds: Option<u32>,
    pub generation: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeTimerResolutionMutationResult {
    pub period_milliseconds: u32,
    pub request_count: u32,
    pub total_request_count: u64,
    pub effective_period_milliseconds: Option<u32>,
    pub generation: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RuntimeTimerResolutionTeardown {
    retired_distinct_periods: usize,
    retired_request_count: u64,
    generation: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeTimers {
    resolution_request_counts: BTreeMap<u32, u32>,
    resolution_generation: u64,
    waitable_timers: BTreeMap<RuntimeWaitableTimerId, RuntimeWaitableTimer>,
    next_waitable_timer_id: u64,
}

impl Default for RuntimeTimers {
    fn default() -> Self {
        Self {
            resolution_request_counts: BTreeMap::new(),
            resolution_generation: 0,
            waitable_timers: BTreeMap::new(),
            next_waitable_timer_id: 1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RuntimeWaitableTimerCreatePlan {
    expected_next_id: u64,
    next_timer: RuntimeWaitableTimer,
    next_waitable_timer_id: u64,
}

impl RuntimeWaitableTimerCreatePlan {
    pub(super) const fn id(&self) -> RuntimeWaitableTimerId {
        self.next_timer.id
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RuntimeWaitableTimerRetirementPlan {
    previous: RuntimeWaitableTimer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RuntimeTimerResolutionMutationPlan {
    expected_generation: u64,
    period_milliseconds: u32,
    expected_count: u32,
    next_count: u32,
    next_generation: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RuntimeTimerResolutionTeardownPlan {
    expected_generation: u64,
    next_generation: u64,
    retired_distinct_periods: usize,
    retired_request_count: u64,
}

impl RuntimeTimers {
    pub(super) fn plan_waitable_timer_creation(
        &self,
        manual_reset: bool,
    ) -> Result<RuntimeWaitableTimerCreatePlan, RuntimeStateError> {
        if self.waitable_timers.len() >= RUNTIME_WAITABLE_TIMER_LIMIT {
            return Err(RuntimeStateError::resource_limit(
                "runtime-waitable-timer",
                "waitable-timer-limit-exceeded",
                "process waitable timer count exceeds the runtime bound",
            ));
        }
        let id = RuntimeWaitableTimerId(self.next_waitable_timer_id);
        if id.0 == 0 {
            return Err(RuntimeStateError::resource_limit(
                "runtime-waitable-timer",
                "waitable-timer-id-space-exhausted",
                "waitable timer ID space is exhausted",
            ));
        }
        if self.waitable_timers.contains_key(&id) {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-waitable-timer",
                "waitable-timer-id-collision",
                "waitable timer allocator selected a live identity",
            ));
        }
        let next_waitable_timer_id =
            self.next_waitable_timer_id.checked_add(1).ok_or_else(|| {
                RuntimeStateError::resource_limit(
                    "runtime-waitable-timer",
                    "waitable-timer-id-space-exhausted",
                    "waitable timer ID space is exhausted",
                )
            })?;
        Ok(RuntimeWaitableTimerCreatePlan {
            expected_next_id: self.next_waitable_timer_id,
            next_timer: RuntimeWaitableTimer { id, manual_reset },
            next_waitable_timer_id,
        })
    }

    pub(super) fn preflight_waitable_timer_creation(
        &self,
        plan: &RuntimeWaitableTimerCreatePlan,
    ) -> Result<(), RuntimeStateError> {
        if self.next_waitable_timer_id != plan.expected_next_id
            || self.waitable_timers.contains_key(&plan.next_timer.id)
        {
            return Err(RuntimeStateError::conflict(
                "runtime-waitable-timer",
                "waitable-timer-creation-plan-stale",
                "waitable timer state changed after creation planning",
            ));
        }
        Ok(())
    }

    pub(super) fn commit_waitable_timer_creation(
        &mut self,
        plan: RuntimeWaitableTimerCreatePlan,
    ) -> &RuntimeWaitableTimer {
        debug_assert!(self.preflight_waitable_timer_creation(&plan).is_ok());
        self.next_waitable_timer_id = plan.next_waitable_timer_id;
        let previous = self
            .waitable_timers
            .insert(plan.next_timer.id, plan.next_timer);
        debug_assert_eq!(previous, None);
        self.waitable_timers
            .get(&plan.next_timer.id)
            .expect("committed waitable timer is missing")
    }

    pub(super) fn plan_waitable_timer_retirement(
        &self,
        id: RuntimeWaitableTimerId,
    ) -> Result<RuntimeWaitableTimerRetirementPlan, RuntimeStateError> {
        let previous = self.waitable_timer(id)?;
        Ok(RuntimeWaitableTimerRetirementPlan {
            previous: *previous,
        })
    }

    pub(super) fn commit_waitable_timer_retirement(
        &mut self,
        plan: RuntimeWaitableTimerRetirementPlan,
    ) {
        let removed = self.waitable_timers.remove(&plan.previous.id);
        debug_assert_eq!(removed, Some(plan.previous));
    }

    pub fn waitable_timer(
        &self,
        id: RuntimeWaitableTimerId,
    ) -> Result<&RuntimeWaitableTimer, RuntimeStateError> {
        self.waitable_timers.get(&id).ok_or_else(|| {
            RuntimeStateError::not_found(
                "runtime-waitable-timer",
                "waitable-timer-unknown",
                format!("waitable timer {} is not live", id.0),
            )
        })
    }

    pub(super) fn waitable_timer_ids(&self) -> Vec<RuntimeWaitableTimerId> {
        self.waitable_timers.keys().copied().collect()
    }

    pub fn waitable_timers(&self) -> &BTreeMap<RuntimeWaitableTimerId, RuntimeWaitableTimer> {
        &self.waitable_timers
    }

    pub(super) fn plan_resolution_begin(
        &self,
        period_milliseconds: u32,
    ) -> Result<RuntimeTimerResolutionMutationPlan, RuntimeStateError> {
        self.validate_period(period_milliseconds)?;
        let expected_count = self
            .resolution_request_counts
            .get(&period_milliseconds)
            .copied()
            .unwrap_or(0);
        if expected_count == 0
            && self.resolution_request_counts.len()
                >= RUNTIME_TIMER_RESOLUTION_DISTINCT_PERIOD_LIMIT
        {
            return Err(RuntimeStateError::resource_limit(
                "runtime-timers",
                "timer-resolution-distinct-period-limit-reached",
                "logical timer resolution request table is full",
            ));
        }
        let next_count = expected_count.checked_add(1).ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-timers",
                "timer-resolution-request-count-overflow",
                "logical timer resolution request count is exhausted",
            )
        })?;
        Ok(RuntimeTimerResolutionMutationPlan {
            expected_generation: self.resolution_generation,
            period_milliseconds,
            expected_count,
            next_count,
            next_generation: self.next_generation()?,
        })
    }

    pub(super) fn plan_resolution_end(
        &self,
        period_milliseconds: u32,
    ) -> Result<Option<RuntimeTimerResolutionMutationPlan>, RuntimeStateError> {
        self.validate_period(period_milliseconds)?;
        let Some(expected_count) = self
            .resolution_request_counts
            .get(&period_milliseconds)
            .copied()
        else {
            return Ok(None);
        };
        Ok(Some(RuntimeTimerResolutionMutationPlan {
            expected_generation: self.resolution_generation,
            period_milliseconds,
            expected_count,
            next_count: expected_count - 1,
            next_generation: self.next_generation()?,
        }))
    }

    pub(super) fn preflight_resolution_mutation(
        &self,
        plan: &RuntimeTimerResolutionMutationPlan,
    ) -> Result<(), RuntimeStateError> {
        let current_count = self
            .resolution_request_counts
            .get(&plan.period_milliseconds)
            .copied()
            .unwrap_or(0);
        if self.resolution_generation != plan.expected_generation
            || current_count != plan.expected_count
        {
            return Err(RuntimeStateError::conflict(
                "runtime-timers",
                "timer-resolution-mutation-plan-stale",
                "logical timer resolution requests changed after mutation planning",
            ));
        }
        Ok(())
    }

    pub(super) fn commit_resolution_mutation(
        &mut self,
        plan: RuntimeTimerResolutionMutationPlan,
    ) -> Result<RuntimeTimerResolutionMutationResult, RuntimeStateError> {
        self.preflight_resolution_mutation(&plan)?;
        if plan.next_count == 0 {
            self.resolution_request_counts
                .remove(&plan.period_milliseconds);
        } else {
            self.resolution_request_counts
                .insert(plan.period_milliseconds, plan.next_count);
        }
        self.resolution_generation = plan.next_generation;
        Ok(self.mutation_result(plan.period_milliseconds, plan.next_count))
    }

    pub(super) fn plan_process_teardown(
        &self,
    ) -> Result<Option<RuntimeTimerResolutionTeardownPlan>, RuntimeStateError> {
        if self.resolution_request_counts.is_empty() {
            return Ok(None);
        }
        Ok(Some(RuntimeTimerResolutionTeardownPlan {
            expected_generation: self.resolution_generation,
            next_generation: self.next_generation()?,
            retired_distinct_periods: self.resolution_request_counts.len(),
            retired_request_count: self.total_request_count(),
        }))
    }

    pub(super) fn preflight_process_teardown(
        &self,
        plan: &RuntimeTimerResolutionTeardownPlan,
    ) -> Result<(), RuntimeStateError> {
        if self.resolution_generation != plan.expected_generation
            || self.resolution_request_counts.len() != plan.retired_distinct_periods
            || self.total_request_count() != plan.retired_request_count
        {
            return Err(RuntimeStateError::conflict(
                "runtime-timers",
                "timer-resolution-teardown-plan-stale",
                "logical timer resolution requests changed after teardown planning",
            ));
        }
        Ok(())
    }

    pub(super) fn commit_process_teardown(
        &mut self,
        plan: RuntimeTimerResolutionTeardownPlan,
    ) -> Result<RuntimeTimerResolutionTeardown, RuntimeStateError> {
        self.preflight_process_teardown(&plan)?;
        self.resolution_request_counts.clear();
        self.resolution_generation = plan.next_generation;
        Ok(RuntimeTimerResolutionTeardown {
            retired_distinct_periods: plan.retired_distinct_periods,
            retired_request_count: plan.retired_request_count,
            generation: self.resolution_generation,
        })
    }

    pub(super) fn state(&self) -> Option<RuntimeTimerResolutionState> {
        (self.resolution_generation != 0 || !self.resolution_request_counts.is_empty()).then(|| {
            RuntimeTimerResolutionState {
                request_counts: self.resolution_request_counts.clone(),
                total_request_count: self.total_request_count(),
                effective_period_milliseconds: self
                    .resolution_request_counts
                    .first_key_value()
                    .map(|(&period, _)| period),
                generation: self.resolution_generation,
            }
        })
    }

    fn mutation_result(
        &self,
        period_milliseconds: u32,
        request_count: u32,
    ) -> RuntimeTimerResolutionMutationResult {
        RuntimeTimerResolutionMutationResult {
            period_milliseconds,
            request_count,
            total_request_count: self.total_request_count(),
            effective_period_milliseconds: self
                .resolution_request_counts
                .first_key_value()
                .map(|(&period, _)| period),
            generation: self.resolution_generation,
        }
    }

    fn validate_period(&self, period_milliseconds: u32) -> Result<(), RuntimeStateError> {
        if period_milliseconds == 0 {
            return Err(RuntimeStateError::invalid_input(
                "runtime-timers",
                "timer-resolution-period-zero",
                "logical timer resolution period must be nonzero",
            ));
        }
        Ok(())
    }

    fn next_generation(&self) -> Result<u64, RuntimeStateError> {
        self.resolution_generation.checked_add(1).ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-timers",
                "timer-resolution-generation-overflow",
                "logical timer resolution generation is exhausted",
            )
        })
    }

    fn total_request_count(&self) -> u64 {
        self.resolution_request_counts
            .values()
            .map(|&count| u64::from(count))
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn begin_end_pairing_counts_duplicates_and_rejects_stale_plans() {
        let mut timers = RuntimeTimers::default();
        let first = timers.plan_resolution_begin(2).unwrap();
        let stale = first;
        let result = timers.commit_resolution_mutation(first).unwrap();
        assert_eq!(result.request_count, 1);
        assert_eq!(result.total_request_count, 1);
        assert_eq!(result.effective_period_milliseconds, Some(2));
        assert_eq!(result.generation, 1);
        assert_eq!(
            timers.commit_resolution_mutation(stale).unwrap_err().code,
            "timer-resolution-mutation-plan-stale"
        );

        let second_period = timers.plan_resolution_begin(5).unwrap();
        timers.commit_resolution_mutation(second_period).unwrap();
        let duplicate = timers.plan_resolution_begin(2).unwrap();
        let result = timers.commit_resolution_mutation(duplicate).unwrap();
        assert_eq!(result.request_count, 2);
        assert_eq!(result.total_request_count, 3);
        assert_eq!(result.effective_period_milliseconds, Some(2));

        let before_unmatched = timers.clone();
        assert!(timers.plan_resolution_end(3).unwrap().is_none());
        assert_eq!(timers, before_unmatched);

        let end = timers.plan_resolution_end(2).unwrap().unwrap();
        assert_eq!(
            timers
                .commit_resolution_mutation(end)
                .unwrap()
                .request_count,
            1
        );
        let end = timers.plan_resolution_end(2).unwrap().unwrap();
        let result = timers.commit_resolution_mutation(end).unwrap();
        assert_eq!(result.request_count, 0);
        assert_eq!(result.total_request_count, 1);
        assert_eq!(result.effective_period_milliseconds, Some(5));
    }

    #[test]
    fn capacity_overflow_and_teardown_are_failure_atomic() {
        let mut timers = RuntimeTimers::default();
        assert_eq!(
            timers.plan_resolution_begin(0).unwrap_err().code,
            "timer-resolution-period-zero"
        );
        for period in 1..=RUNTIME_TIMER_RESOLUTION_DISTINCT_PERIOD_LIMIT as u32 {
            let plan = timers.plan_resolution_begin(period).unwrap();
            timers.commit_resolution_mutation(plan).unwrap();
        }
        let before_capacity = timers.clone();
        assert_eq!(
            timers
                .plan_resolution_begin(RUNTIME_TIMER_RESOLUTION_DISTINCT_PERIOD_LIMIT as u32 + 1,)
                .unwrap_err()
                .code,
            "timer-resolution-distinct-period-limit-reached"
        );
        assert_eq!(timers, before_capacity);

        let teardown = timers.plan_process_teardown().unwrap().unwrap();
        let stale = teardown;
        let result = timers.commit_process_teardown(teardown).unwrap();
        assert_eq!(
            result.retired_distinct_periods,
            RUNTIME_TIMER_RESOLUTION_DISTINCT_PERIOD_LIMIT
        );
        assert_eq!(
            result.retired_request_count,
            RUNTIME_TIMER_RESOLUTION_DISTINCT_PERIOD_LIMIT as u64
        );
        assert!(timers.state().unwrap().request_counts.is_empty());
        assert_eq!(
            timers.commit_process_teardown(stale).unwrap_err().code,
            "timer-resolution-teardown-plan-stale"
        );
    }

    #[test]
    fn waitable_timer_creation_retirement_and_capacity_are_planned() {
        let mut timers = RuntimeTimers::default();
        let first = timers.plan_waitable_timer_creation(true).unwrap();
        let stale = first;
        assert!(timers.waitable_timers().is_empty());
        assert_eq!(first.id(), RuntimeWaitableTimerId(1));
        let timer = *timers.commit_waitable_timer_creation(first);
        assert!(timer.manual_reset);
        assert_eq!(
            timers
                .preflight_waitable_timer_creation(&stale)
                .unwrap_err()
                .code,
            "waitable-timer-creation-plan-stale"
        );
        let retirement = timers.plan_waitable_timer_retirement(timer.id).unwrap();
        assert_eq!(timers.waitable_timer(timer.id).unwrap(), &timer);
        timers.commit_waitable_timer_retirement(retirement);
        assert_eq!(
            timers.waitable_timer(timer.id).unwrap_err().code,
            "waitable-timer-unknown"
        );

        for _ in 0..RUNTIME_WAITABLE_TIMER_LIMIT {
            let plan = timers.plan_waitable_timer_creation(false).unwrap();
            timers.commit_waitable_timer_creation(plan);
        }
        let before_capacity = timers.clone();
        assert_eq!(
            timers.plan_waitable_timer_creation(true).unwrap_err().code,
            "waitable-timer-limit-exceeded"
        );
        assert_eq!(timers, before_capacity);
    }
}

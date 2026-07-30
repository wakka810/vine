use std::collections::BTreeMap;

use serde::Serialize;

use super::{RuntimeHandle, RuntimeStateError, RuntimeThreadId, RuntimeWindowId};
use crate::memory::GuestAddress;

pub(crate) const RUNTIME_TIMER_RESOLUTION_DISTINCT_PERIOD_LIMIT: usize = 64;
pub const RUNTIME_WAITABLE_TIMER_LIMIT: usize = 4096;
pub const RUNTIME_WINDOW_TIMER_ID_NAMESPACE: &str = "vine.logical-window-timer.v1";
pub const RUNTIME_WINDOW_TIMER_LIMIT: usize = 4096;
pub const RUNTIME_USER_TIMER_MINIMUM_MILLISECONDS: u32 = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct RuntimeWaitableTimerId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeWaitableTimer {
    pub id: RuntimeWaitableTimerId,
    pub manual_reset: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arm: Option<RuntimeWaitableTimerArm>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_expiration: Option<RuntimeWaitableTimerExpiration>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeWaitableTimerArm {
    pub relative_due_time_100ns: u64,
    pub armed_at_time_100ns: u64,
    pub deadline_time_100ns: u64,
    pub period_milliseconds: u32,
    pub generation: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeWaitableTimerExpiration {
    pub arm_generation: u64,
    pub deadline_time_100ns: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeTimerClockState {
    pub current_time_100ns: u64,
    pub generation: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeCreateWaitableTimerAResult {
    pub timer_id: RuntimeWaitableTimerId,
    pub handle: RuntimeHandle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct RuntimeWindowTimerId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeWindowTimer {
    pub id: RuntimeWindowTimerId,
    pub window_id: RuntimeWindowId,
    pub window_handle: u32,
    pub timer_id: u32,
    pub owner_thread_id: RuntimeThreadId,
    pub requested_interval_milliseconds: u32,
    pub effective_interval_milliseconds: u32,
    pub callback_address: GuestAddress,
    pub registration_generation: u64,
    pub armed_at_time_100ns: u64,
    pub next_deadline_time_100ns: u64,
    pub delivery_count: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_delivery: Option<RuntimeWindowTimerDeliveryState>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeWindowTimerDeliveryState {
    pub registration_generation: u64,
    pub delivery_count: u64,
    pub scheduled_deadline_time_100ns: u64,
    pub delivered_at_time_100ns: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeWindowTimerDelivery {
    pub timer: RuntimeWindowTimer,
    pub scheduled_deadline_time_100ns: u64,
    pub delivered_at_time_100ns: u64,
    pub generation: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RuntimeWindowTimerRegistrationRequest {
    pub window_id: RuntimeWindowId,
    pub window_handle: u32,
    pub timer_id: u32,
    pub owner_thread_id: RuntimeThreadId,
    pub interval_milliseconds: u32,
    pub callback_address: GuestAddress,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeWindowTimerRegistrationResult {
    pub id: RuntimeWindowTimerId,
    pub timer_id: u32,
    pub replaced: bool,
    pub registration_generation: u64,
    pub generation: u64,
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
pub(super) struct RuntimeTimersTeardown {
    retired_distinct_periods: usize,
    retired_request_count: u64,
    retired_window_timers: usize,
    resolution_generation: u64,
    window_timer_generation: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeTimers {
    timer_clock: RuntimeTimerClockState,
    resolution_request_counts: BTreeMap<u32, u32>,
    resolution_generation: u64,
    waitable_timers: BTreeMap<RuntimeWaitableTimerId, RuntimeWaitableTimer>,
    next_waitable_timer_id: u64,
    window_timers: BTreeMap<RuntimeWindowTimerId, RuntimeWindowTimer>,
    window_timer_index: BTreeMap<(RuntimeWindowId, u32), RuntimeWindowTimerId>,
    next_window_timer_id: u64,
    window_timer_generation: u64,
}

impl Default for RuntimeTimers {
    fn default() -> Self {
        Self {
            timer_clock: RuntimeTimerClockState {
                current_time_100ns: 0,
                generation: 0,
            },
            resolution_request_counts: BTreeMap::new(),
            resolution_generation: 0,
            waitable_timers: BTreeMap::new(),
            next_waitable_timer_id: 1,
            window_timers: BTreeMap::new(),
            window_timer_index: BTreeMap::new(),
            next_window_timer_id: 1,
            window_timer_generation: 0,
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
pub(super) struct RuntimeWaitableTimerArmPlan {
    previous: RuntimeWaitableTimer,
    next: RuntimeWaitableTimer,
    previous_clock: RuntimeTimerClockState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RuntimeWaitableTimerExpirationPlan {
    previous: RuntimeWaitableTimer,
    next: RuntimeWaitableTimer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RuntimeTimerClockAdvancePlan {
    previous: RuntimeTimerClockState,
    next: RuntimeTimerClockState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RuntimeWindowTimerRegistrationPlan {
    expected_generation: u64,
    next_generation: u64,
    expected_next_id: u64,
    next_window_timer_id: u64,
    previous: Option<RuntimeWindowTimer>,
    next: RuntimeWindowTimer,
    expected_clock: RuntimeTimerClockState,
}

impl RuntimeWindowTimerRegistrationPlan {
    pub(crate) fn request_identity(
        &self,
    ) -> (RuntimeWindowId, u32, u32, RuntimeThreadId, GuestAddress) {
        (
            self.next.window_id,
            self.next.window_handle,
            self.next.timer_id,
            self.next.owner_thread_id,
            self.next.callback_address,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RuntimeWindowTimerDeliveryPlan {
    expected_generation: u64,
    next_generation: u64,
    previous_clock: RuntimeTimerClockState,
    previous: RuntimeWindowTimer,
    next: RuntimeWindowTimer,
    scheduled_deadline_time_100ns: u64,
}

impl RuntimeWindowTimerDeliveryPlan {
    pub(crate) const fn timer(&self) -> RuntimeWindowTimer {
        self.previous
    }
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
pub(super) struct RuntimeTimersTeardownPlan {
    expected_resolution_generation: u64,
    next_resolution_generation: Option<u64>,
    retired_distinct_periods: usize,
    retired_request_count: u64,
    expected_window_timer_generation: u64,
    next_window_timer_generation: Option<u64>,
    retired_window_timers: usize,
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
            next_timer: RuntimeWaitableTimer {
                id,
                manual_reset,
                arm: None,
                last_expiration: None,
            },
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

    pub(super) fn plan_waitable_timer_one_shot_arm(
        &self,
        id: RuntimeWaitableTimerId,
        relative_due_time_100ns: u64,
    ) -> Result<RuntimeWaitableTimerArmPlan, RuntimeStateError> {
        if relative_due_time_100ns == 0 {
            return Err(RuntimeStateError::invalid_input(
                "runtime-waitable-timer",
                "waitable-timer-relative-due-time-zero",
                "relative waitable timer due time must be nonzero",
            ));
        }
        let previous = *self.waitable_timer(id)?;
        let previous_generation = previous
            .arm
            .map(|arm| arm.generation)
            .or_else(|| {
                previous
                    .last_expiration
                    .map(|expiration| expiration.arm_generation)
            })
            .unwrap_or(0);
        let generation = previous_generation.checked_add(1).ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-waitable-timer",
                "waitable-timer-arm-generation-exhausted",
                "waitable timer arm generation is exhausted",
            )
        })?;
        let deadline_time_100ns = self
            .timer_clock
            .current_time_100ns
            .checked_add(relative_due_time_100ns)
            .ok_or_else(|| {
                RuntimeStateError::resource_limit(
                    "runtime-waitable-timer",
                    "waitable-timer-deadline-overflow",
                    "relative waitable timer deadline exceeds the logical timer clock",
                )
            })?;
        let mut next = previous;
        next.arm = Some(RuntimeWaitableTimerArm {
            relative_due_time_100ns,
            armed_at_time_100ns: self.timer_clock.current_time_100ns,
            deadline_time_100ns,
            period_milliseconds: 0,
            generation,
        });
        next.last_expiration = None;
        Ok(RuntimeWaitableTimerArmPlan {
            previous,
            next,
            previous_clock: self.timer_clock,
        })
    }

    pub(super) fn preflight_waitable_timer_arm(
        &self,
        plan: &RuntimeWaitableTimerArmPlan,
    ) -> Result<(), RuntimeStateError> {
        if self.waitable_timers.get(&plan.previous.id) != Some(&plan.previous)
            || self.timer_clock != plan.previous_clock
        {
            return Err(RuntimeStateError::conflict(
                "runtime-waitable-timer",
                "waitable-timer-arm-plan-stale",
                "waitable timer state changed after arm planning",
            ));
        }
        Ok(())
    }

    pub(super) fn commit_waitable_timer_arm(
        &mut self,
        plan: RuntimeWaitableTimerArmPlan,
    ) -> &RuntimeWaitableTimer {
        debug_assert!(self.preflight_waitable_timer_arm(&plan).is_ok());
        let previous = self.waitable_timers.insert(plan.next.id, plan.next);
        debug_assert_eq!(previous, Some(plan.previous));
        self.waitable_timers
            .get(&plan.next.id)
            .expect("armed waitable timer is missing")
    }

    pub(super) fn plan_waitable_timer_expiration(
        &self,
        id: RuntimeWaitableTimerId,
        expected_arm_generation: u64,
    ) -> Result<RuntimeWaitableTimerExpirationPlan, RuntimeStateError> {
        let previous = *self.waitable_timer(id)?;
        let arm = previous.arm.ok_or_else(|| {
            RuntimeStateError::conflict(
                "runtime-waitable-timer",
                "waitable-timer-expiration-unarmed",
                "waitable timer expiration requires a live arm",
            )
        })?;
        if arm.generation != expected_arm_generation {
            return Err(RuntimeStateError::conflict(
                "runtime-waitable-timer",
                "waitable-timer-expiration-generation-mismatch",
                "waitable timer expiration does not match the live arm generation",
            ));
        }
        let mut next = previous;
        next.arm = None;
        next.last_expiration = Some(RuntimeWaitableTimerExpiration {
            arm_generation: arm.generation,
            deadline_time_100ns: arm.deadline_time_100ns,
        });
        Ok(RuntimeWaitableTimerExpirationPlan { previous, next })
    }

    pub(super) fn preflight_waitable_timer_expiration(
        &self,
        plan: &RuntimeWaitableTimerExpirationPlan,
    ) -> Result<(), RuntimeStateError> {
        if self.waitable_timers.get(&plan.previous.id) != Some(&plan.previous) {
            return Err(RuntimeStateError::conflict(
                "runtime-waitable-timer",
                "waitable-timer-expiration-plan-stale",
                "waitable timer state changed after expiration planning",
            ));
        }
        Ok(())
    }

    pub(super) fn commit_waitable_timer_expiration(
        &mut self,
        plan: RuntimeWaitableTimerExpirationPlan,
    ) -> &RuntimeWaitableTimer {
        debug_assert!(
            self.preflight_waitable_timer_expiration(&plan).is_ok(),
            "waitable timer expiration plan became stale"
        );
        let previous = self.waitable_timers.insert(plan.next.id, plan.next);
        debug_assert_eq!(previous, Some(plan.previous));
        self.waitable_timers
            .get(&plan.next.id)
            .expect("expired waitable timer is missing")
    }

    pub fn timer_clock_state(&self) -> RuntimeTimerClockState {
        self.timer_clock
    }

    pub(super) fn plan_timer_clock_advance(
        &self,
        target_time_100ns: u64,
    ) -> Result<RuntimeTimerClockAdvancePlan, RuntimeStateError> {
        if target_time_100ns < self.timer_clock.current_time_100ns {
            return Err(RuntimeStateError::invalid_input(
                "runtime-timer-clock",
                "timer-clock-target-regressed",
                "logical timer clock cannot move backwards",
            ));
        }
        let generation = if target_time_100ns == self.timer_clock.current_time_100ns {
            self.timer_clock.generation
        } else {
            self.timer_clock.generation.checked_add(1).ok_or_else(|| {
                RuntimeStateError::resource_limit(
                    "runtime-timer-clock",
                    "timer-clock-generation-exhausted",
                    "logical timer clock generation is exhausted",
                )
            })?
        };
        Ok(RuntimeTimerClockAdvancePlan {
            previous: self.timer_clock,
            next: RuntimeTimerClockState {
                current_time_100ns: target_time_100ns,
                generation,
            },
        })
    }

    pub(super) fn preflight_timer_clock_advance(
        &self,
        plan: &RuntimeTimerClockAdvancePlan,
    ) -> Result<(), RuntimeStateError> {
        if self.timer_clock != plan.previous {
            return Err(RuntimeStateError::conflict(
                "runtime-timer-clock",
                "timer-clock-advance-plan-stale",
                "logical timer clock changed after advance planning",
            ));
        }
        Ok(())
    }

    pub(super) fn commit_timer_clock_advance(
        &mut self,
        plan: RuntimeTimerClockAdvancePlan,
    ) -> RuntimeTimerClockState {
        debug_assert!(
            self.preflight_timer_clock_advance(&plan).is_ok(),
            "logical timer clock advance plan became stale"
        );
        self.timer_clock = plan.next;
        self.timer_clock
    }

    pub(super) fn waitable_timer_ids(&self) -> Vec<RuntimeWaitableTimerId> {
        self.waitable_timers.keys().copied().collect()
    }

    pub fn waitable_timers(&self) -> &BTreeMap<RuntimeWaitableTimerId, RuntimeWaitableTimer> {
        &self.waitable_timers
    }

    pub(crate) fn plan_window_timer_registration(
        &self,
        request: RuntimeWindowTimerRegistrationRequest,
    ) -> Result<RuntimeWindowTimerRegistrationPlan, RuntimeStateError> {
        if request.window_id.0 == 0
            || request.window_handle == 0
            || request.timer_id == 0
            || request.owner_thread_id.0 == 0
            || request.callback_address.0 == 0
        {
            return Err(RuntimeStateError::invalid_input(
                "runtime-window-timer",
                "window-timer-registration-identity-invalid",
                "logical window timer registration requires nonzero window, timer, thread, and callback identities",
            ));
        }
        let key = (request.window_id, request.timer_id);
        let previous = self
            .window_timer_index
            .get(&key)
            .copied()
            .map(|id| {
                self.window_timers.get(&id).copied().ok_or_else(|| {
                    RuntimeStateError::runtime_defect(
                        "runtime-window-timer",
                        "window-timer-index-dangling",
                        "logical window timer index does not resolve to a live registration",
                    )
                })
            })
            .transpose()?;
        let (id, registration_generation, next_window_timer_id) = match previous {
            Some(previous) => {
                if previous.window_id != request.window_id
                    || previous.window_handle != request.window_handle
                    || previous.timer_id != request.timer_id
                    || previous.owner_thread_id != request.owner_thread_id
                {
                    return Err(RuntimeStateError::conflict(
                        "runtime-window-timer",
                        "window-timer-replacement-owner-mismatch",
                        "logical window timer replacement changed its window or owner identity",
                    ));
                }
                (
                    previous.id,
                    previous
                        .registration_generation
                        .checked_add(1)
                        .ok_or_else(|| {
                            RuntimeStateError::resource_limit(
                                "runtime-window-timer",
                                "window-timer-registration-generation-exhausted",
                                "logical window timer registration generation is exhausted",
                            )
                        })?,
                    self.next_window_timer_id,
                )
            }
            None => {
                if self.window_timers.len() >= RUNTIME_WINDOW_TIMER_LIMIT {
                    return Err(RuntimeStateError::resource_limit(
                        "runtime-window-timer",
                        "window-timer-limit-exceeded",
                        "process logical window timer count exceeds the runtime bound",
                    ));
                }
                let id = RuntimeWindowTimerId(self.next_window_timer_id);
                if id.0 == 0 || self.window_timers.contains_key(&id) {
                    return Err(RuntimeStateError::resource_limit(
                        "runtime-window-timer",
                        "window-timer-id-space-exhausted",
                        "logical window timer ID space is exhausted or collided",
                    ));
                }
                let next_id = self.next_window_timer_id.checked_add(1).ok_or_else(|| {
                    RuntimeStateError::resource_limit(
                        "runtime-window-timer",
                        "window-timer-id-space-exhausted",
                        "logical window timer ID space is exhausted",
                    )
                })?;
                (id, 1, next_id)
            }
        };
        let next_generation = self.window_timer_generation.checked_add(1).ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-window-timer",
                "window-timer-generation-exhausted",
                "logical window timer collection generation is exhausted",
            )
        })?;
        let effective_interval_milliseconds = request
            .interval_milliseconds
            .max(RUNTIME_USER_TIMER_MINIMUM_MILLISECONDS);
        let interval_100ns = u64::from(effective_interval_milliseconds)
            .checked_mul(10_000)
            .ok_or_else(|| {
                RuntimeStateError::resource_limit(
                    "runtime-window-timer",
                    "window-timer-interval-overflow",
                    "logical window timer interval exceeds the timer clock unit range",
                )
            })?;
        let next_deadline_time_100ns = self
            .timer_clock
            .current_time_100ns
            .checked_add(interval_100ns)
            .ok_or_else(|| {
                RuntimeStateError::resource_limit(
                    "runtime-window-timer",
                    "window-timer-deadline-overflow",
                    "logical window timer deadline exceeds the timer clock",
                )
            })?;
        Ok(RuntimeWindowTimerRegistrationPlan {
            expected_generation: self.window_timer_generation,
            next_generation,
            expected_next_id: self.next_window_timer_id,
            next_window_timer_id,
            previous,
            expected_clock: self.timer_clock,
            next: RuntimeWindowTimer {
                id,
                window_id: request.window_id,
                window_handle: request.window_handle,
                timer_id: request.timer_id,
                owner_thread_id: request.owner_thread_id,
                requested_interval_milliseconds: request.interval_milliseconds,
                effective_interval_milliseconds,
                callback_address: request.callback_address,
                registration_generation,
                armed_at_time_100ns: self.timer_clock.current_time_100ns,
                next_deadline_time_100ns,
                delivery_count: previous.map_or(0, |timer| timer.delivery_count),
                last_delivery: previous.and_then(|timer| timer.last_delivery),
            },
        })
    }

    pub(crate) fn preflight_window_timer_registration(
        &self,
        plan: &RuntimeWindowTimerRegistrationPlan,
    ) -> Result<(), RuntimeStateError> {
        let key = (plan.next.window_id, plan.next.timer_id);
        let current = self
            .window_timer_index
            .get(&key)
            .and_then(|id| self.window_timers.get(id))
            .copied();
        if self.window_timer_generation != plan.expected_generation
            || self.timer_clock != plan.expected_clock
            || self.next_window_timer_id != plan.expected_next_id
            || current != plan.previous
            || plan.next.id.0 == 0
            || plan.next.window_handle == 0
            || plan.next.timer_id == 0
            || plan.next.owner_thread_id.0 == 0
            || plan.next.callback_address.0 == 0
            || plan.next.effective_interval_milliseconds < RUNTIME_USER_TIMER_MINIMUM_MILLISECONDS
            || plan.next.armed_at_time_100ns != plan.expected_clock.current_time_100ns
            || plan
                .expected_clock
                .current_time_100ns
                .checked_add(u64::from(plan.next.effective_interval_milliseconds) * 10_000)
                != Some(plan.next.next_deadline_time_100ns)
            || self.window_timer_generation.checked_add(1) != Some(plan.next_generation)
            || (plan.previous.is_some()
                && (plan.next.id != plan.previous.expect("checked replacement").id
                    || plan.next_window_timer_id != plan.expected_next_id))
            || (plan.previous.is_none()
                && (self.window_timers.contains_key(&plan.next.id)
                    || plan.next.registration_generation != 1
                    || plan.expected_next_id.checked_add(1) != Some(plan.next_window_timer_id)))
        {
            return Err(RuntimeStateError::conflict(
                "runtime-window-timer",
                "window-timer-registration-plan-stale",
                "logical window timer state changed after registration planning",
            ));
        }
        Ok(())
    }

    pub(crate) fn commit_window_timer_registration(
        &mut self,
        plan: RuntimeWindowTimerRegistrationPlan,
    ) -> RuntimeWindowTimerRegistrationResult {
        debug_assert!(self.preflight_window_timer_registration(&plan).is_ok());
        let replaced = plan.previous.is_some();
        self.next_window_timer_id = plan.next_window_timer_id;
        self.window_timer_generation = plan.next_generation;
        let previous_timer = self.window_timers.insert(plan.next.id, plan.next);
        let previous_index = self
            .window_timer_index
            .insert((plan.next.window_id, plan.next.timer_id), plan.next.id);
        debug_assert_eq!(previous_timer, plan.previous);
        debug_assert_eq!(previous_index, plan.previous.map(|timer| timer.id));
        RuntimeWindowTimerRegistrationResult {
            id: plan.next.id,
            timer_id: plan.next.timer_id,
            replaced,
            registration_generation: plan.next.registration_generation,
            generation: self.window_timer_generation,
        }
    }

    pub(crate) fn plan_due_window_timer_delivery(
        &self,
        owner_thread_id: RuntimeThreadId,
    ) -> Result<Option<RuntimeWindowTimerDeliveryPlan>, RuntimeStateError> {
        if owner_thread_id.0 == 0 {
            return Err(RuntimeStateError::invalid_input(
                "runtime-window-timer",
                "window-timer-delivery-owner-invalid",
                "logical window timer delivery requires a nonzero owner thread",
            ));
        }
        let Some(previous) = self
            .window_timers
            .values()
            .filter(|timer| {
                timer.owner_thread_id == owner_thread_id
                    && timer.next_deadline_time_100ns <= self.timer_clock.current_time_100ns
            })
            .min_by_key(|timer| (timer.next_deadline_time_100ns, timer.id))
            .copied()
        else {
            return Ok(None);
        };
        let next_generation = self.window_timer_generation.checked_add(1).ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-window-timer",
                "window-timer-generation-exhausted",
                "logical window timer collection generation is exhausted",
            )
        })?;
        let delivery_count = previous.delivery_count.checked_add(1).ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-window-timer",
                "window-timer-delivery-count-exhausted",
                "logical window timer delivery count is exhausted",
            )
        })?;
        let interval_100ns = u64::from(previous.effective_interval_milliseconds)
            .checked_mul(10_000)
            .ok_or_else(|| {
                RuntimeStateError::resource_limit(
                    "runtime-window-timer",
                    "window-timer-interval-overflow",
                    "logical window timer interval exceeds the timer clock unit range",
                )
            })?;
        let next_deadline_time_100ns = self
            .timer_clock
            .current_time_100ns
            .checked_add(interval_100ns)
            .ok_or_else(|| {
                RuntimeStateError::resource_limit(
                    "runtime-window-timer",
                    "window-timer-deadline-overflow",
                    "logical window timer deadline exceeds the timer clock",
                )
            })?;
        let delivery = RuntimeWindowTimerDeliveryState {
            registration_generation: previous.registration_generation,
            delivery_count,
            scheduled_deadline_time_100ns: previous.next_deadline_time_100ns,
            delivered_at_time_100ns: self.timer_clock.current_time_100ns,
        };
        let mut next = previous;
        next.armed_at_time_100ns = self.timer_clock.current_time_100ns;
        next.next_deadline_time_100ns = next_deadline_time_100ns;
        next.delivery_count = delivery_count;
        next.last_delivery = Some(delivery);
        Ok(Some(RuntimeWindowTimerDeliveryPlan {
            expected_generation: self.window_timer_generation,
            next_generation,
            previous_clock: self.timer_clock,
            previous,
            next,
            scheduled_deadline_time_100ns: previous.next_deadline_time_100ns,
        }))
    }

    pub(crate) fn preflight_window_timer_delivery(
        &self,
        plan: &RuntimeWindowTimerDeliveryPlan,
    ) -> Result<(), RuntimeStateError> {
        if self.window_timer_generation != plan.expected_generation
            || self.window_timer_generation.checked_add(1) != Some(plan.next_generation)
            || self.timer_clock != plan.previous_clock
            || self.window_timers.get(&plan.previous.id) != Some(&plan.previous)
            || plan.previous.owner_thread_id != plan.next.owner_thread_id
            || plan.previous.registration_generation != plan.next.registration_generation
            || plan.previous.next_deadline_time_100ns != plan.scheduled_deadline_time_100ns
            || plan.scheduled_deadline_time_100ns > plan.previous_clock.current_time_100ns
            || plan.next.delivery_count != plan.previous.delivery_count.checked_add(1).unwrap_or(0)
            || plan.next.last_delivery
                != Some(RuntimeWindowTimerDeliveryState {
                    registration_generation: plan.previous.registration_generation,
                    delivery_count: plan.next.delivery_count,
                    scheduled_deadline_time_100ns: plan.scheduled_deadline_time_100ns,
                    delivered_at_time_100ns: plan.previous_clock.current_time_100ns,
                })
        {
            return Err(RuntimeStateError::conflict(
                "runtime-window-timer",
                "window-timer-delivery-plan-stale",
                "logical window timer state changed after delivery planning",
            ));
        }
        Ok(())
    }

    pub(crate) fn commit_window_timer_delivery(
        &mut self,
        plan: RuntimeWindowTimerDeliveryPlan,
    ) -> RuntimeWindowTimerDelivery {
        debug_assert!(self.preflight_window_timer_delivery(&plan).is_ok());
        self.window_timer_generation = plan.next_generation;
        let previous = self.window_timers.insert(plan.next.id, plan.next);
        debug_assert_eq!(previous, Some(plan.previous));
        RuntimeWindowTimerDelivery {
            timer: plan.next,
            scheduled_deadline_time_100ns: plan.scheduled_deadline_time_100ns,
            delivered_at_time_100ns: plan.previous_clock.current_time_100ns,
            generation: self.window_timer_generation,
        }
    }

    pub fn window_timer(
        &self,
        id: RuntimeWindowTimerId,
    ) -> Result<&RuntimeWindowTimer, RuntimeStateError> {
        self.window_timers.get(&id).ok_or_else(|| {
            RuntimeStateError::not_found(
                "runtime-window-timer",
                "window-timer-unknown",
                format!("logical window timer {} is not live", id.0),
            )
        })
    }

    pub fn window_timers(&self) -> &BTreeMap<RuntimeWindowTimerId, RuntimeWindowTimer> {
        &self.window_timers
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
    ) -> Result<Option<RuntimeTimersTeardownPlan>, RuntimeStateError> {
        if self.resolution_request_counts.is_empty() && self.window_timers.is_empty() {
            return Ok(None);
        }
        Ok(Some(RuntimeTimersTeardownPlan {
            expected_resolution_generation: self.resolution_generation,
            next_resolution_generation: (!self.resolution_request_counts.is_empty())
                .then(|| self.next_generation())
                .transpose()?,
            retired_distinct_periods: self.resolution_request_counts.len(),
            retired_request_count: self.total_request_count(),
            expected_window_timer_generation: self.window_timer_generation,
            next_window_timer_generation: (!self.window_timers.is_empty())
                .then(|| {
                    self.window_timer_generation.checked_add(1).ok_or_else(|| {
                        RuntimeStateError::resource_limit(
                            "runtime-window-timer",
                            "window-timer-generation-exhausted",
                            "logical window timer collection generation is exhausted",
                        )
                    })
                })
                .transpose()?,
            retired_window_timers: self.window_timers.len(),
        }))
    }

    pub(super) fn preflight_process_teardown(
        &self,
        plan: &RuntimeTimersTeardownPlan,
    ) -> Result<(), RuntimeStateError> {
        if self.resolution_generation != plan.expected_resolution_generation
            || self.resolution_request_counts.len() != plan.retired_distinct_periods
            || self.total_request_count() != plan.retired_request_count
            || self.window_timer_generation != plan.expected_window_timer_generation
            || self.window_timers.len() != plan.retired_window_timers
            || self.window_timer_index.len() != plan.retired_window_timers
            || plan.next_resolution_generation
                != (!self.resolution_request_counts.is_empty())
                    .then(|| self.resolution_generation.checked_add(1))
                    .flatten()
            || plan.next_window_timer_generation
                != (!self.window_timers.is_empty())
                    .then(|| self.window_timer_generation.checked_add(1))
                    .flatten()
        {
            return Err(RuntimeStateError::conflict(
                "runtime-timers",
                "timer-teardown-plan-stale",
                "logical timer state changed after process teardown planning",
            ));
        }
        Ok(())
    }

    pub(super) fn commit_process_teardown(
        &mut self,
        plan: RuntimeTimersTeardownPlan,
    ) -> Result<RuntimeTimersTeardown, RuntimeStateError> {
        self.preflight_process_teardown(&plan)?;
        self.resolution_request_counts.clear();
        self.window_timers.clear();
        self.window_timer_index.clear();
        if let Some(generation) = plan.next_resolution_generation {
            self.resolution_generation = generation;
        }
        if let Some(generation) = plan.next_window_timer_generation {
            self.window_timer_generation = generation;
        }
        Ok(RuntimeTimersTeardown {
            retired_distinct_periods: plan.retired_distinct_periods,
            retired_request_count: plan.retired_request_count,
            retired_window_timers: plan.retired_window_timers,
            resolution_generation: self.resolution_generation,
            window_timer_generation: self.window_timer_generation,
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
            "timer-teardown-plan-stale"
        );
    }

    #[test]
    fn window_timer_registration_replacement_and_teardown_preserve_identity() {
        let mut timers = RuntimeTimers::default();
        let request = RuntimeWindowTimerRegistrationRequest {
            window_id: RuntimeWindowId(7),
            window_handle: 0x4801_0001,
            timer_id: 1,
            owner_thread_id: RuntimeThreadId(9),
            interval_milliseconds: 20,
            callback_address: GuestAddress(0x1009_9600),
        };
        let plan = timers.plan_window_timer_registration(request).unwrap();
        let stale = plan;
        assert!(timers.window_timers().is_empty());
        let created = timers.commit_window_timer_registration(plan);
        assert_eq!(created.id, RuntimeWindowTimerId(1));
        assert_eq!(created.timer_id, 1);
        assert!(!created.replaced);
        assert_eq!(created.registration_generation, 1);
        let registered = *timers.window_timer(created.id).unwrap();
        assert_eq!(registered.requested_interval_milliseconds, 20);
        assert_eq!(registered.effective_interval_milliseconds, 20);
        assert_eq!(registered.callback_address, GuestAddress(0x1009_9600));
        assert_eq!(registered.armed_at_time_100ns, 0);
        assert_eq!(registered.next_deadline_time_100ns, 200_000);
        assert_eq!(registered.delivery_count, 0);
        assert_eq!(registered.last_delivery, None);
        assert_eq!(
            timers
                .preflight_window_timer_registration(&stale)
                .unwrap_err()
                .code,
            "window-timer-registration-plan-stale"
        );

        let replacement = timers
            .plan_window_timer_registration(RuntimeWindowTimerRegistrationRequest {
                interval_milliseconds: 1,
                callback_address: GuestAddress(0x1009_9700),
                ..request
            })
            .unwrap();
        let replaced = timers.commit_window_timer_registration(replacement);
        assert_eq!(replaced.id, created.id);
        assert!(replaced.replaced);
        assert_eq!(replaced.registration_generation, 2);
        let registered = timers.window_timer(created.id).unwrap();
        assert_eq!(registered.requested_interval_milliseconds, 1);
        assert_eq!(registered.effective_interval_milliseconds, 10);
        assert_eq!(registered.callback_address, GuestAddress(0x1009_9700));
        assert_eq!(registered.next_deadline_time_100ns, 100_000);
        assert_eq!(registered.delivery_count, 0);
        assert_eq!(timers.next_window_timer_id, 2);

        let before_invalid = timers.clone();
        assert_eq!(
            timers
                .plan_window_timer_registration(RuntimeWindowTimerRegistrationRequest {
                    callback_address: GuestAddress(0),
                    ..request
                })
                .unwrap_err()
                .code,
            "window-timer-registration-identity-invalid"
        );
        assert_eq!(timers, before_invalid);

        let teardown = timers.plan_process_teardown().unwrap().unwrap();
        let result = timers.commit_process_teardown(teardown).unwrap();
        assert_eq!(result.retired_window_timers, 1);
        assert!(timers.window_timers().is_empty());
        assert!(timers.window_timer_index.is_empty());
    }

    #[test]
    fn window_timer_delivery_is_due_owned_coalesced_and_generation_checked() {
        let mut timers = RuntimeTimers::default();
        let registration = timers
            .plan_window_timer_registration(RuntimeWindowTimerRegistrationRequest {
                window_id: RuntimeWindowId(7),
                window_handle: 0x4801_0001,
                timer_id: 1,
                owner_thread_id: RuntimeThreadId(9),
                interval_milliseconds: 20,
                callback_address: GuestAddress(0x1009_9600),
            })
            .unwrap();
        timers.commit_window_timer_registration(registration);
        assert!(
            timers
                .plan_due_window_timer_delivery(RuntimeThreadId(9))
                .unwrap()
                .is_none()
        );

        let advance = timers.plan_timer_clock_advance(200_000).unwrap();
        timers.commit_timer_clock_advance(advance);
        assert!(
            timers
                .plan_due_window_timer_delivery(RuntimeThreadId(10))
                .unwrap()
                .is_none()
        );
        let plan = timers
            .plan_due_window_timer_delivery(RuntimeThreadId(9))
            .unwrap()
            .expect("due owner timer");
        let stale = plan;
        let delivered = timers.commit_window_timer_delivery(plan);
        assert_eq!(delivered.scheduled_deadline_time_100ns, 200_000);
        assert_eq!(delivered.delivered_at_time_100ns, 200_000);
        assert_eq!(delivered.timer.next_deadline_time_100ns, 400_000);
        assert_eq!(delivered.timer.delivery_count, 1);
        assert_eq!(
            delivered.timer.last_delivery,
            Some(RuntimeWindowTimerDeliveryState {
                registration_generation: 1,
                delivery_count: 1,
                scheduled_deadline_time_100ns: 200_000,
                delivered_at_time_100ns: 200_000,
            })
        );
        assert!(
            timers
                .plan_due_window_timer_delivery(RuntimeThreadId(9))
                .unwrap()
                .is_none()
        );
        assert_eq!(
            timers
                .preflight_window_timer_delivery(&stale)
                .unwrap_err()
                .code,
            "window-timer-delivery-plan-stale"
        );

        let advance = timers.plan_timer_clock_advance(450_000).unwrap();
        timers.commit_timer_clock_advance(advance);
        let late = timers
            .plan_due_window_timer_delivery(RuntimeThreadId(9))
            .unwrap()
            .expect("late coalesced delivery");
        let delivered = timers.commit_window_timer_delivery(late);
        assert_eq!(delivered.scheduled_deadline_time_100ns, 400_000);
        assert_eq!(delivered.delivered_at_time_100ns, 450_000);
        assert_eq!(delivered.timer.next_deadline_time_100ns, 650_000);
        assert_eq!(delivered.timer.delivery_count, 2);
    }

    #[test]
    fn window_timer_capacity_rejection_is_failure_atomic() {
        let mut timers = RuntimeTimers::default();
        for timer_id in 1..=RUNTIME_WINDOW_TIMER_LIMIT as u32 {
            let plan = timers
                .plan_window_timer_registration(RuntimeWindowTimerRegistrationRequest {
                    window_id: RuntimeWindowId(7),
                    window_handle: 0x4801_0001,
                    timer_id,
                    owner_thread_id: RuntimeThreadId(9),
                    interval_milliseconds: 20,
                    callback_address: GuestAddress(0x1009_9600),
                })
                .unwrap();
            timers.commit_window_timer_registration(plan);
        }
        let before = timers.clone();
        assert_eq!(
            timers
                .plan_window_timer_registration(RuntimeWindowTimerRegistrationRequest {
                    window_id: RuntimeWindowId(7),
                    window_handle: 0x4801_0001,
                    timer_id: RUNTIME_WINDOW_TIMER_LIMIT as u32 + 1,
                    owner_thread_id: RuntimeThreadId(9),
                    interval_milliseconds: 20,
                    callback_address: GuestAddress(0x1009_9600),
                })
                .unwrap_err()
                .code,
            "window-timer-limit-exceeded"
        );
        assert_eq!(timers, before);
    }

    #[test]
    fn logical_clock_expiration_and_rearm_plans_reject_stale_generations() {
        let mut timers = RuntimeTimers::default();
        let creation = timers.plan_waitable_timer_creation(true).unwrap();
        let timer = *timers.commit_waitable_timer_creation(creation);

        let stale_arm = timers
            .plan_waitable_timer_one_shot_arm(timer.id, 5_000)
            .unwrap();
        let clock_advance = timers.plan_timer_clock_advance(1_000).unwrap();
        assert_eq!(
            timers.timer_clock_state(),
            RuntimeTimerClockState {
                current_time_100ns: 0,
                generation: 0,
            }
        );
        assert_eq!(
            timers.commit_timer_clock_advance(clock_advance),
            RuntimeTimerClockState {
                current_time_100ns: 1_000,
                generation: 1,
            }
        );
        assert_eq!(
            timers
                .preflight_waitable_timer_arm(&stale_arm)
                .unwrap_err()
                .code,
            "waitable-timer-arm-plan-stale"
        );
        assert_eq!(timers.waitable_timer(timer.id).unwrap().arm, None);

        let arm = timers
            .plan_waitable_timer_one_shot_arm(timer.id, 5_000)
            .unwrap();
        assert_eq!(
            timers.commit_waitable_timer_arm(arm).arm,
            Some(RuntimeWaitableTimerArm {
                relative_due_time_100ns: 5_000,
                armed_at_time_100ns: 1_000,
                deadline_time_100ns: 6_000,
                period_milliseconds: 0,
                generation: 1,
            })
        );
        let before_mismatched_expiration = timers.clone();
        assert_eq!(
            timers
                .plan_waitable_timer_expiration(timer.id, 2)
                .unwrap_err()
                .code,
            "waitable-timer-expiration-generation-mismatch"
        );
        assert_eq!(timers, before_mismatched_expiration);

        let expiration = timers.plan_waitable_timer_expiration(timer.id, 1).unwrap();
        let deadline = timers.plan_timer_clock_advance(6_000).unwrap();
        timers.commit_timer_clock_advance(deadline);
        timers
            .preflight_waitable_timer_expiration(&expiration)
            .unwrap();
        let expired = *timers.commit_waitable_timer_expiration(expiration);
        assert_eq!(expired.arm, None);
        assert_eq!(
            expired.last_expiration,
            Some(RuntimeWaitableTimerExpiration {
                arm_generation: 1,
                deadline_time_100ns: 6_000,
            })
        );

        let rearm = timers
            .plan_waitable_timer_one_shot_arm(timer.id, 1_000)
            .unwrap();
        assert_eq!(
            timers.commit_waitable_timer_arm(rearm).arm,
            Some(RuntimeWaitableTimerArm {
                relative_due_time_100ns: 1_000,
                armed_at_time_100ns: 6_000,
                deadline_time_100ns: 7_000,
                period_milliseconds: 0,
                generation: 2,
            })
        );
        let stale_clock = timers.plan_timer_clock_advance(7_000).unwrap();
        let competing_clock = timers.plan_timer_clock_advance(6_500).unwrap();
        timers.commit_timer_clock_advance(competing_clock);
        assert_eq!(
            timers
                .preflight_timer_clock_advance(&stale_clock)
                .unwrap_err()
                .code,
            "timer-clock-advance-plan-stale"
        );
        let before_regression = timers.clone();
        assert_eq!(
            timers.plan_timer_clock_advance(6_000).unwrap_err().code,
            "timer-clock-target-regressed"
        );
        assert_eq!(timers, before_regression);
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
        assert_eq!(timer.arm, None);
        let first_arm = timers
            .plan_waitable_timer_one_shot_arm(timer.id, 5_000)
            .unwrap();
        let stale_arm = first_arm;
        assert_eq!(timers.waitable_timer(timer.id).unwrap().arm, None);
        assert_eq!(
            timers.commit_waitable_timer_arm(first_arm).arm,
            Some(RuntimeWaitableTimerArm {
                relative_due_time_100ns: 5_000,
                armed_at_time_100ns: 0,
                deadline_time_100ns: 5_000,
                period_milliseconds: 0,
                generation: 1,
            })
        );
        assert_eq!(
            timers
                .preflight_waitable_timer_arm(&stale_arm)
                .unwrap_err()
                .code,
            "waitable-timer-arm-plan-stale"
        );
        let rearm = timers
            .plan_waitable_timer_one_shot_arm(timer.id, 20_000)
            .unwrap();
        assert_eq!(
            timers.commit_waitable_timer_arm(rearm).arm,
            Some(RuntimeWaitableTimerArm {
                relative_due_time_100ns: 20_000,
                armed_at_time_100ns: 0,
                deadline_time_100ns: 20_000,
                period_milliseconds: 0,
                generation: 2,
            })
        );
        let before_zero = timers.clone();
        assert_eq!(
            timers
                .plan_waitable_timer_one_shot_arm(timer.id, 0)
                .unwrap_err()
                .code,
            "waitable-timer-relative-due-time-zero"
        );
        assert_eq!(timers, before_zero);
        assert_eq!(
            timers
                .preflight_waitable_timer_creation(&stale)
                .unwrap_err()
                .code,
            "waitable-timer-creation-plan-stale"
        );
        let armed_timer = *timers.waitable_timer(timer.id).unwrap();
        let retirement = timers.plan_waitable_timer_retirement(timer.id).unwrap();
        assert_eq!(timers.waitable_timer(timer.id).unwrap(), &armed_timer);
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

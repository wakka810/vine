use crate::winmm::{TIMERR_NOCANDO, TIMERR_NOERROR};
use std::collections::{BTreeMap, BTreeSet};

use super::{RuntimeStateError, RuntimeThreadId, RuntimeWindowId};
use crate::memory::GuestAddress;

pub(crate) const RUNTIME_TIMER_RESOLUTION_DISTINCT_PERIOD_LIMIT: usize = 64;
pub const RUNTIME_WAITABLE_TIMER_LIMIT: usize = 4096;
pub const RUNTIME_WINDOW_TIMER_LIMIT: usize = 4096;
pub const RUNTIME_USER_TIMER_MINIMUM_MILLISECONDS: u32 = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct RuntimeWaitableTimerId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeWaitableTimer {
    pub id: RuntimeWaitableTimerId,
    pub manual_reset: bool,
    pub arm: Option<RuntimeWaitableTimerArm>,
    pub last_expiration: Option<RuntimeWaitableTimerExpiration>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeWaitableTimerArm {
    pub deadline_time_100ns: u64,
    pub generation: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeWaitableTimerExpiration {
    pub arm_generation: u64,
    pub deadline_time_100ns: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct RuntimeWindowTimerId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    pub(crate) message_observed: bool,
    pub delivery_count: u64,
    pub last_delivery: Option<RuntimeWindowTimerDeliveryState>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeWindowTimerDeliveryState {
    pub registration_generation: u64,
    pub delivery_count: u64,
    pub scheduled_deadline_time_100ns: u64,
    pub delivered_at_time_100ns: u64,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeTimers {
    resolution_request_counts: BTreeMap<u32, u32>,
    waitable_timers: BTreeMap<RuntimeWaitableTimerId, RuntimeWaitableTimer>,
    next_waitable_timer_id: u64,
    window_timers: BTreeMap<RuntimeWindowTimerId, RuntimeWindowTimer>,
    window_timer_index: BTreeMap<(RuntimeWindowId, u32), RuntimeWindowTimerId>,
    delivered_timer_callbacks: BTreeSet<(RuntimeWindowId, u32, u32, RuntimeThreadId, GuestAddress)>,
    next_window_timer_id: u64,
}

impl Default for RuntimeTimers {
    fn default() -> Self {
        Self {
            resolution_request_counts: BTreeMap::new(),
            waitable_timers: BTreeMap::new(),
            next_waitable_timer_id: 1,
            window_timers: BTreeMap::new(),
            window_timer_index: BTreeMap::new(),
            delivered_timer_callbacks: BTreeSet::new(),
            next_window_timer_id: 1,
        }
    }
}

impl RuntimeTimers {
    pub(super) fn retire_window(&mut self, id: RuntimeWindowId) {
        self.window_timers.retain(|_, timer| timer.window_id != id);
        self.window_timer_index
            .retain(|(window, _), _| *window != id);
        self.delivered_timer_callbacks
            .retain(|(window, ..)| *window != id);
    }
    pub(super) fn new_waitable_timer(
        &self,
        manual_reset: bool,
    ) -> Result<RuntimeWaitableTimer, RuntimeStateError> {
        if self.waitable_timers.len() >= RUNTIME_WAITABLE_TIMER_LIMIT {
            return Err(RuntimeStateError::new(
                "process waitable timer count exceeds the runtime bound",
            ));
        }
        let id = RuntimeWaitableTimerId(self.next_waitable_timer_id);
        if id.0 == 0 {
            return Err(RuntimeStateError::new(
                "waitable timer ID space is exhausted",
            ));
        }
        if self.waitable_timers.contains_key(&id) {
            return Err(RuntimeStateError::new(
                "waitable timer allocator selected a live identity",
            ));
        }
        self.next_waitable_timer_id
            .checked_add(1)
            .ok_or_else(|| RuntimeStateError::new("waitable timer ID space is exhausted"))?;
        Ok(RuntimeWaitableTimer {
            id,
            manual_reset,
            arm: None,
            last_expiration: None,
        })
    }

    // The aggregate registers the checked object/handle before publishing this
    // timer. No browser work or guest callback can intervene.
    pub(super) fn insert_waitable_timer(&mut self, timer: RuntimeWaitableTimer) {
        debug_assert_eq!(timer.id.0, self.next_waitable_timer_id);
        self.next_waitable_timer_id += 1;
        let previous = self.waitable_timers.insert(timer.id, timer);
        debug_assert!(previous.is_none());
    }

    pub(super) fn retire_waitable_timer(&mut self, id: RuntimeWaitableTimerId) {
        self.waitable_timers
            .remove(&id)
            .expect("validated timer retirement");
    }

    pub fn waitable_timer(
        &self,
        id: RuntimeWaitableTimerId,
    ) -> Result<&RuntimeWaitableTimer, RuntimeStateError> {
        self.waitable_timers
            .get(&id)
            .ok_or_else(|| RuntimeStateError::new(format!("waitable timer {} is not live", id.0)))
    }

    pub(super) fn armed_waitable_timer(
        &self,
        id: RuntimeWaitableTimerId,
        relative_due_time_100ns: u64,
        clock: u64,
    ) -> Result<RuntimeWaitableTimer, RuntimeStateError> {
        if relative_due_time_100ns == 0 {
            return Err(RuntimeStateError::new(
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
        let generation = previous_generation
            .checked_add(1)
            .ok_or_else(|| RuntimeStateError::new("waitable timer arm generation is exhausted"))?;
        let deadline_time_100ns = clock.checked_add(relative_due_time_100ns).ok_or_else(|| {
            RuntimeStateError::new(
                "relative waitable timer deadline exceeds the logical timer clock",
            )
        })?;
        let mut next = previous;
        next.arm = Some(RuntimeWaitableTimerArm {
            deadline_time_100ns,
            generation,
        });
        next.last_expiration = None;
        Ok(next)
    }

    // The aggregate has checked the deadline, generation, and wait-state
    // update before publishing either owner. This does not leave Rust.
    pub(super) fn replace_waitable_timer(&mut self, timer: RuntimeWaitableTimer) {
        let live = self
            .waitable_timers
            .get_mut(&timer.id)
            .expect("validated live timer");
        *live = timer;
    }

    pub(super) fn validate_waitable_timer_expiration(
        &self,
        id: RuntimeWaitableTimerId,
        expected_arm_generation: u64,
    ) -> Result<(), RuntimeStateError> {
        let timer = self.waitable_timer(id)?;
        if timer.id != id {
            return Err(RuntimeStateError::new(
                "waitable timer record does not match its live index",
            ));
        }
        let arm = timer.arm.ok_or_else(|| {
            RuntimeStateError::new("waitable timer expiration requires a live arm")
        })?;
        if arm.generation != expected_arm_generation {
            return Err(RuntimeStateError::new(
                "waitable timer expiration does not match the live arm generation",
            ));
        }
        Ok(())
    }

    pub(super) fn expire_waitable_timer(
        &mut self,
        id: RuntimeWaitableTimerId,
        expected_arm_generation: u64,
    ) -> Result<(), RuntimeStateError> {
        self.validate_waitable_timer_expiration(id, expected_arm_generation)?;
        let timer = self
            .waitable_timers
            .get_mut(&id)
            .expect("validated live timer");
        let arm = timer.arm.take().expect("validated live timer arm");
        timer.last_expiration = Some(RuntimeWaitableTimerExpiration {
            arm_generation: arm.generation,
            deadline_time_100ns: arm.deadline_time_100ns,
        });
        Ok(())
    }

    pub(crate) fn register_window_timer(
        &mut self,
        request: RuntimeWindowTimerRegistrationRequest,
        clock: u64,
    ) -> Result<u32, RuntimeStateError> {
        if request.window_id.0 == 0 || request.window_handle == 0 || request.owner_thread_id.0 == 0
        {
            return Err(RuntimeStateError::new(
                "logical window timer registration requires nonzero window and thread identities",
            ));
        }
        let key = (request.window_id, request.timer_id);
        let previous = self
            .window_timer_index
            .get(&key)
            .copied()
            .map(|id| {
                let timer = self.window_timers.get(&id).copied().ok_or_else(|| {
                    RuntimeStateError::new(
                        "logical window timer index does not resolve to a live registration",
                    )
                })?;
                if timer.id != id {
                    return Err(RuntimeStateError::new(
                        "logical window timer index does not match its live registration identity",
                    ));
                }
                Ok(timer)
            })
            .transpose()?;
        let (id, registration_generation, next_window_timer_id) = match previous {
            Some(previous) => {
                if previous.window_id != request.window_id
                    || previous.window_handle != request.window_handle
                    || previous.timer_id != request.timer_id
                    || previous.owner_thread_id != request.owner_thread_id
                {
                    return Err(RuntimeStateError::new(
                        "logical window timer replacement changed its window or owner identity",
                    ));
                }
                (
                    previous.id,
                    previous
                        .registration_generation
                        .checked_add(1)
                        .ok_or_else(|| {
                            RuntimeStateError::new(
                                "logical window timer registration generation is exhausted",
                            )
                        })?,
                    self.next_window_timer_id,
                )
            }
            None => {
                if self.window_timers.len() >= RUNTIME_WINDOW_TIMER_LIMIT {
                    return Err(RuntimeStateError::new(
                        "process logical window timer count exceeds the runtime bound",
                    ));
                }
                let id = RuntimeWindowTimerId(self.next_window_timer_id);
                if id.0 == 0 || self.window_timers.contains_key(&id) {
                    return Err(RuntimeStateError::new(
                        "logical window timer ID space is exhausted or collided",
                    ));
                }
                let next_id = self.next_window_timer_id.checked_add(1).ok_or_else(|| {
                    RuntimeStateError::new("logical window timer ID space is exhausted")
                })?;
                (id, 1, next_id)
            }
        };
        let effective_interval_milliseconds = request
            .interval_milliseconds
            .max(RUNTIME_USER_TIMER_MINIMUM_MILLISECONDS);
        let interval_100ns = u64::from(effective_interval_milliseconds)
            .checked_mul(10_000)
            .ok_or_else(|| {
                RuntimeStateError::new(
                    "logical window timer interval exceeds the timer clock unit range",
                )
            })?;
        let next_deadline_time_100ns = clock.checked_add(interval_100ns).ok_or_else(|| {
            RuntimeStateError::new("logical window timer deadline exceeds the timer clock")
        })?;
        let next = RuntimeWindowTimer {
            id,
            window_id: request.window_id,
            window_handle: request.window_handle,
            timer_id: request.timer_id,
            owner_thread_id: request.owner_thread_id,
            requested_interval_milliseconds: request.interval_milliseconds,
            effective_interval_milliseconds,
            callback_address: request.callback_address,
            registration_generation,
            armed_at_time_100ns: clock,
            next_deadline_time_100ns,
            message_observed: false,
            delivery_count: previous.map_or(0, |timer| timer.delivery_count),
            last_delivery: previous.and_then(|timer| timer.last_delivery),
        };
        if let Some(previous) = previous
            && previous.callback_address.0 != 0
            && previous.last_delivery.is_some()
        {
            self.delivered_timer_callbacks.insert((
                previous.window_id,
                previous.window_handle,
                previous.timer_id,
                previous.owner_thread_id,
                previous.callback_address,
            ));
        }
        self.next_window_timer_id = next_window_timer_id;
        self.window_timers.insert(id, next);
        self.window_timer_index.insert(key, id);
        // With a window, nIDEvent (including zero) is the message identity.
        // The return value is only a nonzero success token in this case.
        Ok(request.timer_id.max(1))
    }

    pub(crate) fn kill_window_timer(
        &mut self,
        window_id: RuntimeWindowId,
        timer_id: u32,
        owner: RuntimeThreadId,
    ) -> Result<bool, RuntimeStateError> {
        let key = (window_id, timer_id);
        let Some(id) = self.window_timer_index.get(&key).copied() else {
            return Ok(false);
        };
        let timer = *self
            .window_timers
            .get(&id)
            .ok_or_else(|| RuntimeStateError::new("timer index has no live registration"))?;
        if timer.id != id
            || timer.window_id != window_id
            || timer.timer_id != timer_id
            || timer.owner_thread_id != owner
        {
            return Err(RuntimeStateError::new(
                "timer removal does not match its live owner and identity",
            ));
        }
        if timer.callback_address.0 != 0 && timer.last_delivery.is_some() {
            self.delivered_timer_callbacks.insert((
                timer.window_id,
                timer.window_handle,
                timer.timer_id,
                timer.owner_thread_id,
                timer.callback_address,
            ));
        }
        self.window_timer_index.remove(&key);
        self.window_timers.remove(&id);
        Ok(true)
    }

    pub(crate) fn has_delivered_timer_callback(
        &self,
        window_id: RuntimeWindowId,
        window_handle: u32,
        timer_id: u32,
        owner_thread_id: RuntimeThreadId,
        callback_address: GuestAddress,
    ) -> bool {
        self.delivered_timer_callbacks.contains(&(
            window_id,
            window_handle,
            timer_id,
            owner_thread_id,
            callback_address,
        ))
    }

    #[cfg(test)]
    pub(crate) fn preview_due_window_timer_delivery(
        &self,
        owner_thread_id: RuntimeThreadId,
        clock: u64,
    ) -> Result<Option<RuntimeWindowTimer>, RuntimeStateError> {
        self.preview_due_window_timer_delivery_filtered(
            owner_thread_id,
            clock,
            super::RuntimeMessageFilter::default(),
        )
    }

    pub(crate) fn preview_due_window_timer_delivery_filtered(
        &self,
        owner_thread_id: RuntimeThreadId,
        clock: u64,
        filter: super::RuntimeMessageFilter,
    ) -> Result<Option<RuntimeWindowTimer>, RuntimeStateError> {
        if owner_thread_id.0 == 0 {
            return Err(RuntimeStateError::new(
                "logical window timer delivery requires a nonzero owner thread",
            ));
        }
        let Some((id, previous)) = self
            .window_timers
            .iter()
            .filter(|(_, timer)| {
                timer.owner_thread_id == owner_thread_id
                    && timer.next_deadline_time_100ns <= clock
                    && filter.matches(timer.window_handle, 0x0113)
            })
            .min_by_key(|(id, timer)| (timer.next_deadline_time_100ns, **id))
            .map(|(id, timer)| (*id, *timer))
        else {
            return Ok(None);
        };
        if id != previous.id
            || self
                .window_timer_index
                .get(&(previous.window_id, previous.timer_id))
                != Some(&id)
        {
            return Err(RuntimeStateError::new(
                "due window timer does not match its live indexes",
            ));
        }
        let delivery_count = previous.delivery_count.checked_add(1).ok_or_else(|| {
            RuntimeStateError::new("logical window timer delivery count is exhausted")
        })?;
        let interval_100ns = u64::from(previous.effective_interval_milliseconds)
            .checked_mul(10_000)
            .ok_or_else(|| {
                RuntimeStateError::new(
                    "logical window timer interval exceeds the timer clock unit range",
                )
            })?;
        let next_deadline_time_100ns = clock.checked_add(interval_100ns).ok_or_else(|| {
            RuntimeStateError::new("logical window timer deadline exceeds the timer clock")
        })?;
        let delivery = RuntimeWindowTimerDeliveryState {
            registration_generation: previous.registration_generation,
            delivery_count,
            scheduled_deadline_time_100ns: previous.next_deadline_time_100ns,
            delivered_at_time_100ns: clock,
        };
        let mut next = previous;
        next.armed_at_time_100ns = clock;
        next.next_deadline_time_100ns = next_deadline_time_100ns;
        next.message_observed = false;
        next.delivery_count = delivery_count;
        next.last_delivery = Some(delivery);
        Ok(Some(next))
    }

    pub(super) fn observe_window_timers(&mut self, owner: RuntimeThreadId, clock: u64) {
        for timer in self.window_timers.values_mut() {
            if timer.owner_thread_id == owner && timer.next_deadline_time_100ns <= clock {
                timer.message_observed = true;
            }
        }
    }

    pub(crate) fn publish_window_timer_delivery(&mut self, timer: RuntimeWindowTimer) {
        *self
            .window_timers
            .get_mut(&timer.id)
            .expect("validated due window timer") = timer;
    }

    #[cfg(test)]
    pub(crate) fn window_timer(
        &self,
        id: RuntimeWindowTimerId,
    ) -> Result<&RuntimeWindowTimer, RuntimeStateError> {
        self.window_timers.get(&id).ok_or_else(|| {
            RuntimeStateError::new(format!("logical window timer {} is not live", id.0))
        })
    }

    pub fn window_timers(&self) -> &BTreeMap<RuntimeWindowTimerId, RuntimeWindowTimer> {
        &self.window_timers
    }

    pub(super) fn begin_period(&mut self, period_milliseconds: u32) -> u32 {
        if period_milliseconds == 0 {
            return TIMERR_NOCANDO;
        }
        let count = self
            .resolution_request_counts
            .get(&period_milliseconds)
            .copied()
            .unwrap_or(0);
        if count == 0
            && self.resolution_request_counts.len()
                >= RUNTIME_TIMER_RESOLUTION_DISTINCT_PERIOD_LIMIT
        {
            return TIMERR_NOCANDO;
        }
        let Some(next) = count.checked_add(1) else {
            return TIMERR_NOCANDO;
        };
        self.resolution_request_counts
            .insert(period_milliseconds, next);
        TIMERR_NOERROR
    }

    pub(super) fn end_period(&mut self, period_milliseconds: u32) -> u32 {
        let Some(count) = self
            .resolution_request_counts
            .get(&period_milliseconds)
            .copied()
        else {
            return TIMERR_NOCANDO;
        };
        if count == 1 {
            self.resolution_request_counts.remove(&period_milliseconds);
        } else {
            self.resolution_request_counts
                .insert(period_milliseconds, count - 1);
        }
        TIMERR_NOERROR
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn begin_end_pairing_and_capacity_preserve_state() {
        let mut timers = RuntimeTimers::default();
        assert_eq!(timers.begin_period(2), TIMERR_NOERROR);
        assert_eq!(timers.begin_period(5), TIMERR_NOERROR);
        assert_eq!(timers.begin_period(2), TIMERR_NOERROR);
        assert_eq!(timers.resolution_request_counts.get(&2), Some(&2));
        assert_eq!(timers.resolution_request_counts.get(&5), Some(&1));
        assert_eq!(timers.resolution_request_counts.len(), 2);
        let before = timers.clone();
        assert_eq!(timers.end_period(3), TIMERR_NOCANDO);
        assert_eq!(timers, before);
        assert_eq!(timers.end_period(2), TIMERR_NOERROR);
        assert_eq!(timers.end_period(2), TIMERR_NOERROR);
        assert!(!timers.resolution_request_counts.contains_key(&2));
        assert_eq!(timers.resolution_request_counts.get(&5), Some(&1));
        timers.resolution_request_counts.insert(5, u32::MAX);
        let before = timers.clone();
        assert_eq!(timers.begin_period(5), TIMERR_NOCANDO);
        assert_eq!(timers, before);
        timers.resolution_request_counts.insert(5, 1);
        for period in 1..=RUNTIME_TIMER_RESOLUTION_DISTINCT_PERIOD_LIMIT as u32 {
            assert_eq!(timers.begin_period(period), TIMERR_NOERROR);
        }
        let before = timers.clone();
        assert_eq!(
            timers.begin_period(RUNTIME_TIMER_RESOLUTION_DISTINCT_PERIOD_LIMIT as u32 + 1),
            TIMERR_NOCANDO
        );
        assert_eq!(timers, before);
    }

    #[test]
    fn window_timer_registration_and_replacement_preserve_identity() {
        let mut timers = RuntimeTimers::default();
        let clock = 0;
        let request = RuntimeWindowTimerRegistrationRequest {
            window_id: RuntimeWindowId(7),
            window_handle: 0x4801_0001,
            timer_id: 1,
            owner_thread_id: RuntimeThreadId(9),
            interval_milliseconds: 20,
            callback_address: GuestAddress(0x1009_9600),
        };
        assert!(timers.window_timers().is_empty());
        let created = timers.register_window_timer(request, clock).unwrap();
        assert_eq!(created, 1);
        let id = timers.window_timer_index[&(request.window_id, created)];
        assert_eq!(id, RuntimeWindowTimerId(1));
        let registered = *timers.window_timer(id).unwrap();
        assert_eq!(registered.registration_generation, 1);
        assert_eq!(registered.requested_interval_milliseconds, 20);
        assert_eq!(registered.effective_interval_milliseconds, 20);
        assert_eq!(registered.callback_address, GuestAddress(0x1009_9600));
        assert_eq!(registered.armed_at_time_100ns, 0);
        assert_eq!(registered.next_deadline_time_100ns, 200_000);
        assert_eq!(registered.delivery_count, 0);
        assert_eq!(registered.last_delivery, None);
        let replaced = timers
            .register_window_timer(
                RuntimeWindowTimerRegistrationRequest {
                    interval_milliseconds: 1,
                    callback_address: GuestAddress(0x1009_9700),
                    ..request
                },
                clock,
            )
            .unwrap();
        assert_eq!(replaced, created);
        assert_eq!(
            timers.window_timer_index[&(request.window_id, replaced)],
            id
        );
        let registered = timers.window_timer(id).unwrap();
        assert_eq!(registered.registration_generation, 2);
        assert_eq!(registered.requested_interval_milliseconds, 1);
        assert_eq!(registered.effective_interval_milliseconds, 10);
        assert_eq!(registered.callback_address, GuestAddress(0x1009_9700));
        assert_eq!(registered.next_deadline_time_100ns, 100_000);
        assert_eq!(registered.delivery_count, 0);
        assert_eq!(timers.next_window_timer_id, 2);

        let before_invalid = timers.clone();
        timers
            .register_window_timer(
                RuntimeWindowTimerRegistrationRequest {
                    window_handle: 0,
                    ..request
                },
                clock,
            )
            .unwrap_err();
        assert_eq!(timers, before_invalid);
    }

    #[test]
    fn window_timer_delivery_is_due_owned_and_coalesced() {
        let mut timers = RuntimeTimers::default();
        let mut clock = 0;
        timers
            .register_window_timer(
                RuntimeWindowTimerRegistrationRequest {
                    window_id: RuntimeWindowId(7),
                    window_handle: 0x4801_0001,
                    timer_id: 1,
                    owner_thread_id: RuntimeThreadId(9),
                    interval_milliseconds: 20,
                    callback_address: GuestAddress(0x1009_9600),
                },
                clock,
            )
            .unwrap();
        assert!(
            timers
                .preview_due_window_timer_delivery(RuntimeThreadId(9), clock)
                .unwrap()
                .is_none()
        );

        clock = 200_000;
        assert!(
            timers
                .preview_due_window_timer_delivery(RuntimeThreadId(10), clock)
                .unwrap()
                .is_none()
        );
        let plan = timers
            .preview_due_window_timer_delivery(RuntimeThreadId(9), clock)
            .unwrap()
            .expect("due owner timer");
        timers.publish_window_timer_delivery(plan);
        let delivered = plan;
        assert_eq!(
            delivered
                .last_delivery
                .unwrap()
                .scheduled_deadline_time_100ns,
            200_000
        );
        assert_eq!(
            delivered.last_delivery.unwrap().delivered_at_time_100ns,
            200_000
        );
        assert_eq!(delivered.next_deadline_time_100ns, 400_000);
        assert_eq!(delivered.delivery_count, 1);
        assert_eq!(
            delivered.last_delivery,
            Some(RuntimeWindowTimerDeliveryState {
                registration_generation: 1,
                delivery_count: 1,
                scheduled_deadline_time_100ns: 200_000,
                delivered_at_time_100ns: 200_000,
            })
        );
        assert!(
            timers
                .preview_due_window_timer_delivery(RuntimeThreadId(9), clock)
                .unwrap()
                .is_none()
        );

        clock = 450_000;
        let late = timers
            .preview_due_window_timer_delivery(RuntimeThreadId(9), clock)
            .unwrap()
            .expect("late coalesced delivery");
        timers.publish_window_timer_delivery(late);
        let delivered = late;
        assert_eq!(
            delivered
                .last_delivery
                .unwrap()
                .scheduled_deadline_time_100ns,
            400_000
        );
        assert_eq!(
            delivered.last_delivery.unwrap().delivered_at_time_100ns,
            450_000
        );
        assert_eq!(delivered.next_deadline_time_100ns, 650_000);
        assert_eq!(delivered.delivery_count, 2);
    }

    #[test]
    fn due_timer_rejects_corrupt_live_indexes_before_mutation() {
        let mut timers = RuntimeTimers::default();
        let request = RuntimeWindowTimerRegistrationRequest {
            window_id: RuntimeWindowId(7),
            window_handle: 0x4801_0001,
            timer_id: 1,
            owner_thread_id: RuntimeThreadId(9),
            interval_milliseconds: 20,
            callback_address: GuestAddress(0x1009_9600),
        };
        timers.register_window_timer(request, 0).unwrap();
        let id = timers.window_timer_index[&(request.window_id, request.timer_id)];
        for corrupt_record in [true, false] {
            let mut invalid = timers.clone();
            if corrupt_record {
                invalid.window_timers.get_mut(&id).unwrap().id.0 += 1;
            } else {
                invalid.window_timer_index.insert(
                    (request.window_id, request.timer_id),
                    RuntimeWindowTimerId(id.0 + 1),
                );
            }
            let before = invalid.clone();
            assert!(
                invalid
                    .preview_due_window_timer_delivery(request.owner_thread_id, 200_000)
                    .is_err()
            );
            assert_eq!(invalid, before);
        }
    }

    #[test]
    fn window_timer_replacement_rejects_corrupt_live_identity_without_mutation() {
        let mut timers = RuntimeTimers::default();
        let request = RuntimeWindowTimerRegistrationRequest {
            window_id: RuntimeWindowId(7),
            window_handle: 0x4801_0001,
            timer_id: 1,
            owner_thread_id: RuntimeThreadId(9),
            interval_milliseconds: 20,
            callback_address: GuestAddress(0x1009_9600),
        };
        let clock = 0;
        timers.register_window_timer(request, clock).unwrap();
        let id = timers.window_timer_index[&(request.window_id, request.timer_id)];
        timers.window_timers.get_mut(&id).unwrap().id.0 += 1;
        let before = timers.clone();
        timers.register_window_timer(request, clock).unwrap_err();
        assert_eq!(timers, before);
    }

    #[test]
    fn window_timer_registration_overflow_preserves_existing_timer() {
        let mut timers = RuntimeTimers::default();
        let request = RuntimeWindowTimerRegistrationRequest {
            window_id: RuntimeWindowId(7),
            window_handle: 0x4801_0001,
            timer_id: 1,
            owner_thread_id: RuntimeThreadId(9),
            interval_milliseconds: 20,
            callback_address: GuestAddress(0x1009_9600),
        };
        let clock = 0;
        let created = timers.register_window_timer(request, clock).unwrap();
        let id = timers.window_timer_index[&(request.window_id, created)];
        let before = timers.clone();
        assert!(timers.register_window_timer(request, u64::MAX,).is_err());
        assert_eq!(timers, before);

        timers
            .window_timers
            .get_mut(&id)
            .unwrap()
            .registration_generation = u64::MAX;
        let before = timers.clone();
        assert!(timers.register_window_timer(request, clock).is_err());
        assert_eq!(timers, before);
    }

    #[test]
    fn window_timer_capacity_rejection_is_failure_atomic() {
        let mut timers = RuntimeTimers::default();
        let clock = 0;
        for timer_id in 1..=RUNTIME_WINDOW_TIMER_LIMIT as u32 {
            timers
                .register_window_timer(
                    RuntimeWindowTimerRegistrationRequest {
                        window_id: RuntimeWindowId(7),
                        window_handle: 0x4801_0001,
                        timer_id,
                        owner_thread_id: RuntimeThreadId(9),
                        interval_milliseconds: 20,
                        callback_address: GuestAddress(0x1009_9600),
                    },
                    clock,
                )
                .unwrap();
        }
        let before = timers.clone();
        timers
            .register_window_timer(
                RuntimeWindowTimerRegistrationRequest {
                    window_id: RuntimeWindowId(7),
                    window_handle: 0x4801_0001,
                    timer_id: RUNTIME_WINDOW_TIMER_LIMIT as u32 + 1,
                    owner_thread_id: RuntimeThreadId(9),
                    interval_milliseconds: 20,
                    callback_address: GuestAddress(0x1009_9600),
                },
                clock,
            )
            .unwrap_err();
        assert_eq!(timers, before);
    }

    #[test]
    fn expiration_rejects_unarmed_and_corrupt_timer_identity_without_mutation() {
        let mut timers = RuntimeTimers::default();
        let timer = timers.new_waitable_timer(true).unwrap();
        timers.insert_waitable_timer(timer);
        let before_unarmed = timers.clone();
        assert!(timers.expire_waitable_timer(timer.id, 1).is_err());
        assert_eq!(timers, before_unarmed);

        let clock = 0;
        let armed = timers.armed_waitable_timer(timer.id, 5_000, clock).unwrap();
        timers.replace_waitable_timer(armed);
        timers.waitable_timers.get_mut(&timer.id).unwrap().id.0 += 1;
        let before_corrupt = timers.clone();
        assert!(timers.expire_waitable_timer(timer.id, 1).is_err());
        assert_eq!(timers, before_corrupt);
    }

    #[test]
    fn expiration_checks_live_arm_generation_and_rearm_uses_current_clock() {
        let mut timers = RuntimeTimers::default();
        let timer = timers.new_waitable_timer(true).unwrap();
        timers.insert_waitable_timer(timer);

        let clock = 1_000;
        let arm = timers.armed_waitable_timer(timer.id, 5_000, clock).unwrap();
        timers.replace_waitable_timer(arm);
        assert_eq!(
            timers.waitable_timer(timer.id).unwrap().arm,
            Some(RuntimeWaitableTimerArm {
                deadline_time_100ns: 6_000,
                generation: 1,
            })
        );
        let before_mismatched_expiration = timers.clone();
        assert!(timers.expire_waitable_timer(timer.id, 2).is_err());
        assert_eq!(timers, before_mismatched_expiration);

        let clock = 6_000;
        timers.expire_waitable_timer(timer.id, 1).unwrap();
        let expired = *timers.waitable_timer(timer.id).unwrap();
        assert_eq!(expired.arm, None);
        assert_eq!(
            expired.last_expiration,
            Some(RuntimeWaitableTimerExpiration {
                arm_generation: 1,
                deadline_time_100ns: 6_000,
            })
        );

        let rearm = timers.armed_waitable_timer(timer.id, 1_000, clock).unwrap();
        timers.replace_waitable_timer(rearm);
        assert_eq!(
            timers.waitable_timer(timer.id).unwrap().arm,
            Some(RuntimeWaitableTimerArm {
                deadline_time_100ns: 7_000,
                generation: 2,
            })
        );
    }

    #[test]
    fn waitable_timer_creation_retirement_and_capacity_are_checked() {
        let mut timers = RuntimeTimers::default();
        let clock = 0;
        let before = timers.clone();
        let timer = timers.new_waitable_timer(true).unwrap();
        assert_eq!(timers, before);
        assert_eq!(timer.id, RuntimeWaitableTimerId(1));
        timers.insert_waitable_timer(timer);
        assert!(timer.manual_reset);
        assert_eq!(timer.arm, None);
        let first_arm = timers.armed_waitable_timer(timer.id, 5_000, clock).unwrap();
        assert_eq!(timers.waitable_timer(timer.id).unwrap().arm, None);
        timers.replace_waitable_timer(first_arm);
        assert_eq!(
            timers.waitable_timer(timer.id).unwrap().arm,
            Some(RuntimeWaitableTimerArm {
                deadline_time_100ns: 5_000,
                generation: 1,
            })
        );
        let rearm = timers
            .armed_waitable_timer(timer.id, 20_000, clock)
            .unwrap();
        timers.replace_waitable_timer(rearm);
        assert_eq!(
            timers.waitable_timer(timer.id).unwrap().arm,
            Some(RuntimeWaitableTimerArm {
                deadline_time_100ns: 20_000,
                generation: 2,
            })
        );
        let before_zero = timers.clone();
        timers.armed_waitable_timer(timer.id, 0, clock).unwrap_err();
        assert_eq!(timers, before_zero);
        timers.waitable_timer(timer.id).unwrap();
        timers.retire_waitable_timer(timer.id);
        timers.waitable_timer(timer.id).unwrap_err();

        for _ in 0..RUNTIME_WAITABLE_TIMER_LIMIT {
            let timer = timers.new_waitable_timer(false).unwrap();
            timers.insert_waitable_timer(timer);
        }
        let before_capacity = timers.clone();
        timers.new_waitable_timer(true).unwrap_err();
        assert_eq!(timers, before_capacity);
    }
}

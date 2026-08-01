use serde::{Deserialize, Serialize, Serializer};

use super::RuntimeStateError;

pub const RUNTIME_CLOCK_PROFILE_SCHEMA: &str = "vine.runtime-clock-profile";
pub const RUNTIME_CLOCK_PROFILE_VERSION: u32 = 1;
pub const RUNTIME_SCHEDULED_SHARED_CLOCK_PROFILE_ID: &str = "scheduled-shared-logical-deadline-v1";

const FILE_TIME_TICKS_PER_SECOND: u128 = 10_000_000;
const FILE_TIME_TICKS_PER_MILLISECOND: u64 = 10_000;
const MAX_CLOCK_PROFILE_ID_BYTES: usize = 128;
const RUNTIME_API_READ_INCREMENT_CLOCK_PROFILE_ID: &str = "api-read-increment-v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeClockProgression {
    ApiReadIncrement,
    ScheduledShared,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeClockProfile {
    pub schema: String,
    pub schema_version: u32,
    pub id: String,
    pub progression: RuntimeClockProgression,
}

impl RuntimeClockProfile {
    pub fn api_read_increment_v1() -> Self {
        Self {
            schema: RUNTIME_CLOCK_PROFILE_SCHEMA.to_owned(),
            schema_version: RUNTIME_CLOCK_PROFILE_VERSION,
            id: RUNTIME_API_READ_INCREMENT_CLOCK_PROFILE_ID.to_owned(),
            progression: RuntimeClockProgression::ApiReadIncrement,
        }
    }

    pub fn scheduled_shared_v1() -> Self {
        Self {
            schema: RUNTIME_CLOCK_PROFILE_SCHEMA.to_owned(),
            schema_version: RUNTIME_CLOCK_PROFILE_VERSION,
            id: RUNTIME_SCHEDULED_SHARED_CLOCK_PROFILE_ID.to_owned(),
            progression: RuntimeClockProgression::ScheduledShared,
        }
    }

    pub fn validate(&self) -> Result<(), RuntimeStateError> {
        if self.schema != RUNTIME_CLOCK_PROFILE_SCHEMA
            || self.schema_version != RUNTIME_CLOCK_PROFILE_VERSION
            || self.id.is_empty()
            || self.id.len() > MAX_CLOCK_PROFILE_ID_BYTES
            || !self.id.is_ascii()
            || self.id.as_bytes().contains(&0)
        {
            return Err(RuntimeStateError::invalid_input(
                "runtime-clock",
                "clock-profile-identity-invalid",
                "runtime clock profile requires the supported schema and one bounded ASCII identity",
            ));
        }
        match self.progression {
            RuntimeClockProgression::ApiReadIncrement
                if self.id != RUNTIME_API_READ_INCREMENT_CLOCK_PROFILE_ID =>
            {
                return Err(RuntimeStateError::invalid_input(
                    "runtime-clock",
                    "api-read-clock-profile-identity-invalid",
                    "API-read clock progression requires its versioned runtime profile identity",
                ));
            }
            RuntimeClockProgression::ScheduledShared
                if self.id != RUNTIME_SCHEDULED_SHARED_CLOCK_PROFILE_ID =>
            {
                return Err(RuntimeStateError::invalid_input(
                    "runtime-clock",
                    "scheduled-clock-profile-identity-invalid",
                    "scheduled shared clock progression requires its versioned runtime profile identity",
                ));
            }
            _ => {}
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeClockState {
    pub wall_clock_file_time: u64,
    pub tick_count: u32,
    pub performance_counter: u64,
    pub performance_frequency: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeTimerClockState {
    pub current_time_100ns: u64,
    pub generation: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeClock {
    epoch: RuntimeClockState,
    state: RuntimeClockState,
    timer: RuntimeTimerClockState,
    profile: RuntimeClockProfile,
    profile_revision: u64,
}

impl Serialize for RuntimeClock {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.state.serialize(serializer)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeClockProfilePlan {
    previous_profile: RuntimeClockProfile,
    previous_revision: u64,
    next_profile: RuntimeClockProfile,
    next_revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeClockAdvancePlan {
    previous_timer: RuntimeTimerClockState,
    previous_state: RuntimeClockState,
    expected_profile_revision: u64,
    next_timer: RuntimeTimerClockState,
    next_state: RuntimeClockState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RuntimeTickCountSamplePlan {
    previous: u32,
    returned: u32,
    next: u32,
    expected_timer: RuntimeTimerClockState,
    expected_profile_revision: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RuntimePerformanceCounterSamplePlan {
    previous: u64,
    returned: u64,
    next: u64,
    expected_timer: RuntimeTimerClockState,
    expected_profile_revision: u64,
}

impl RuntimeClock {
    pub(super) fn new(state: RuntimeClockState) -> Result<Self, RuntimeStateError> {
        validate_clock_state(&state)?;
        Ok(Self {
            epoch: state.clone(),
            state,
            timer: RuntimeTimerClockState {
                current_time_100ns: 0,
                generation: 0,
            },
            profile: RuntimeClockProfile::api_read_increment_v1(),
            profile_revision: 0,
        })
    }

    pub(super) fn state(&self) -> &RuntimeClockState {
        &self.state
    }

    pub(super) fn timer_state(&self) -> RuntimeTimerClockState {
        self.timer
    }

    pub(super) fn profile(&self) -> &RuntimeClockProfile {
        &self.profile
    }

    pub(super) fn uses_scheduled_shared_progression(&self) -> bool {
        self.profile.progression == RuntimeClockProgression::ScheduledShared
    }

    #[cfg(test)]
    pub(super) fn set_tick_count_for_test(&mut self, tick_count: u32) {
        self.state.tick_count = tick_count;
    }

    #[cfg(test)]
    pub(super) fn set_performance_counter_for_test(&mut self, performance_counter: u64) {
        self.state.performance_counter = performance_counter;
    }

    pub(super) fn plan_profile_configuration(
        &self,
        profile: RuntimeClockProfile,
    ) -> Result<RuntimeClockProfilePlan, RuntimeStateError> {
        profile.validate()?;
        if self.profile.progression != RuntimeClockProgression::ApiReadIncrement
            || self.profile_revision != 0
            || self.timer.current_time_100ns != 0
            || self.timer.generation != 0
            || self.state != self.epoch
        {
            return Err(RuntimeStateError::conflict(
                "runtime-clock",
                "clock-profile-already-active",
                "runtime clock profile can only be selected before clock sampling or scheduled progression",
            ));
        }
        let next_revision = self.profile_revision.checked_add(1).ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-clock",
                "clock-profile-revision-exhausted",
                "runtime clock profile revision is exhausted",
            )
        })?;
        Ok(RuntimeClockProfilePlan {
            previous_profile: self.profile.clone(),
            previous_revision: self.profile_revision,
            next_profile: profile,
            next_revision,
        })
    }

    pub(super) fn preflight_profile_configuration(
        &self,
        plan: &RuntimeClockProfilePlan,
    ) -> Result<(), RuntimeStateError> {
        if self.profile != plan.previous_profile
            || self.profile_revision != plan.previous_revision
            || self.timer.current_time_100ns != 0
            || self.timer.generation != 0
            || self.state != self.epoch
        {
            return Err(RuntimeStateError::conflict(
                "runtime-clock",
                "clock-profile-plan-stale",
                "runtime clock changed after profile selection preflight",
            ));
        }
        Ok(())
    }

    pub(super) fn commit_profile_configuration(
        &mut self,
        plan: RuntimeClockProfilePlan,
    ) -> Result<&RuntimeClockProfile, RuntimeStateError> {
        self.preflight_profile_configuration(&plan)?;
        self.profile = plan.next_profile;
        self.profile_revision = plan.next_revision;
        Ok(&self.profile)
    }

    pub(super) fn plan_tick_count_sample(
        &self,
        increment_per_read: u32,
    ) -> Result<RuntimeTickCountSamplePlan, RuntimeStateError> {
        let next = if self.uses_scheduled_shared_progression() {
            self.state.tick_count
        } else {
            if increment_per_read == 0 {
                return Err(RuntimeStateError::invalid_input(
                    "runtime-process-clock",
                    "tick-count-increment-zero",
                    "advancing tick-count sampling requires a nonzero increment",
                ));
            }
            self.state.tick_count.wrapping_add(increment_per_read)
        };
        Ok(RuntimeTickCountSamplePlan {
            previous: self.state.tick_count,
            returned: self.state.tick_count,
            next,
            expected_timer: self.timer,
            expected_profile_revision: self.profile_revision,
        })
    }

    pub(super) fn commit_tick_count_sample(
        &mut self,
        plan: RuntimeTickCountSamplePlan,
    ) -> Result<u32, RuntimeStateError> {
        if self.state.tick_count != plan.previous
            || self.timer != plan.expected_timer
            || self.profile_revision != plan.expected_profile_revision
        {
            return Err(RuntimeStateError::conflict(
                "runtime-process-clock",
                "tick-count-sample-plan-stale",
                "process clock changed after tick-count sampling preflight",
            ));
        }
        self.state.tick_count = plan.next;
        Ok(plan.returned)
    }

    pub(super) fn plan_performance_counter_sample(
        &self,
        increment_per_read: u64,
    ) -> Result<RuntimePerformanceCounterSamplePlan, RuntimeStateError> {
        let next = if self.uses_scheduled_shared_progression() {
            self.state.performance_counter
        } else {
            if increment_per_read == 0 {
                return Err(RuntimeStateError::invalid_input(
                    "runtime-process-clock",
                    "performance-counter-increment-zero",
                    "advancing performance-counter sampling requires a nonzero increment",
                ));
            }
            self.state
                .performance_counter
                .checked_add(increment_per_read)
                .filter(|next| *next <= i64::MAX as u64)
                .ok_or_else(|| {
                    RuntimeStateError::resource_limit(
                        "runtime-process-clock",
                        "performance-counter-range-exhausted",
                        "performance-counter sampling exceeded the nonnegative LARGE_INTEGER range",
                    )
                })?
        };
        Ok(RuntimePerformanceCounterSamplePlan {
            previous: self.state.performance_counter,
            returned: self.state.performance_counter,
            next,
            expected_timer: self.timer,
            expected_profile_revision: self.profile_revision,
        })
    }

    pub(super) fn commit_performance_counter_sample(
        &mut self,
        plan: RuntimePerformanceCounterSamplePlan,
    ) -> Result<u64, RuntimeStateError> {
        if self.state.performance_counter != plan.previous
            || self.timer != plan.expected_timer
            || self.profile_revision != plan.expected_profile_revision
        {
            return Err(RuntimeStateError::conflict(
                "runtime-process-clock",
                "performance-counter-sample-plan-stale",
                "process clock changed after performance-counter sampling preflight",
            ));
        }
        self.state.performance_counter = plan.next;
        Ok(plan.returned)
    }

    pub(super) fn plan_advance(
        &self,
        target_time_100ns: u64,
    ) -> Result<RuntimeClockAdvancePlan, RuntimeStateError> {
        if target_time_100ns < self.timer.current_time_100ns {
            return Err(RuntimeStateError::invalid_input(
                "runtime-timer-clock",
                "timer-clock-target-regressed",
                "logical timer clock cannot move backwards",
            ));
        }
        let generation = if target_time_100ns == self.timer.current_time_100ns {
            self.timer.generation
        } else {
            self.timer.generation.checked_add(1).ok_or_else(|| {
                RuntimeStateError::resource_limit(
                    "runtime-timer-clock",
                    "timer-clock-generation-exhausted",
                    "logical timer clock generation is exhausted",
                )
            })?
        };
        let next_state = if self.uses_scheduled_shared_progression() {
            project_scheduled_state(&self.epoch, target_time_100ns)?
        } else {
            self.state.clone()
        };
        Ok(RuntimeClockAdvancePlan {
            previous_timer: self.timer,
            previous_state: self.state.clone(),
            expected_profile_revision: self.profile_revision,
            next_timer: RuntimeTimerClockState {
                current_time_100ns: target_time_100ns,
                generation,
            },
            next_state,
        })
    }

    pub(super) fn preflight_advance(
        &self,
        plan: &RuntimeClockAdvancePlan,
    ) -> Result<(), RuntimeStateError> {
        if self.timer != plan.previous_timer
            || self.state != plan.previous_state
            || self.profile_revision != plan.expected_profile_revision
        {
            return Err(RuntimeStateError::conflict(
                "runtime-timer-clock",
                "timer-clock-advance-plan-stale",
                "logical clock changed after advance planning",
            ));
        }
        Ok(())
    }

    pub(super) fn commit_advance(
        &mut self,
        plan: RuntimeClockAdvancePlan,
    ) -> Result<RuntimeTimerClockState, RuntimeStateError> {
        self.preflight_advance(&plan)?;
        self.timer = plan.next_timer;
        self.state = plan.next_state;
        Ok(self.timer)
    }
}

fn validate_clock_state(state: &RuntimeClockState) -> Result<(), RuntimeStateError> {
    if state.performance_frequency == 0 {
        return Err(RuntimeStateError::invalid_input(
            "runtime-clock",
            "performance-frequency-zero",
            "runtime performance-counter frequency must be nonzero",
        ));
    }
    if state.performance_frequency > i64::MAX as u64 {
        return Err(RuntimeStateError::invalid_input(
            "runtime-clock",
            "performance-frequency-range-invalid",
            "runtime performance-counter frequency must fit a nonnegative LARGE_INTEGER",
        ));
    }
    if state.performance_counter > i64::MAX as u64 {
        return Err(RuntimeStateError::invalid_input(
            "runtime-clock",
            "performance-counter-range-invalid",
            "runtime performance counter must fit a nonnegative LARGE_INTEGER",
        ));
    }
    Ok(())
}

fn project_scheduled_state(
    epoch: &RuntimeClockState,
    elapsed_time_100ns: u64,
) -> Result<RuntimeClockState, RuntimeStateError> {
    let wall_clock_file_time = epoch
        .wall_clock_file_time
        .checked_add(elapsed_time_100ns)
        .ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-clock",
                "wall-clock-range-exhausted",
                "scheduled shared clock exceeded the FILETIME range",
            )
        })?;
    let elapsed_milliseconds = elapsed_time_100ns / FILE_TIME_TICKS_PER_MILLISECOND;
    let tick_count = epoch.tick_count.wrapping_add(elapsed_milliseconds as u32);
    let performance_delta = (u128::from(elapsed_time_100ns)
        * u128::from(epoch.performance_frequency))
        / FILE_TIME_TICKS_PER_SECOND;
    let performance_counter = u128::from(epoch.performance_counter)
        .checked_add(performance_delta)
        .filter(|counter| *counter <= i64::MAX as u128)
        .and_then(|counter| u64::try_from(counter).ok())
        .ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-clock",
                "performance-counter-range-exhausted",
                "scheduled shared clock exceeded the nonnegative LARGE_INTEGER range",
            )
        })?;
    Ok(RuntimeClockState {
        wall_clock_file_time,
        tick_count,
        performance_counter,
        performance_frequency: epoch.performance_frequency,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clock() -> RuntimeClock {
        RuntimeClock::new(RuntimeClockState {
            wall_clock_file_time: 134_282_016_000_000_000,
            tick_count: 3_600_000,
            performance_counter: 12_345_678_901,
            performance_frequency: 10_000_000,
        })
        .unwrap()
    }

    #[test]
    fn scheduled_profile_projects_one_elapsed_timeline_without_read_progression() {
        let mut clock = clock();
        let profile = clock
            .plan_profile_configuration(RuntimeClockProfile::scheduled_shared_v1())
            .unwrap();
        clock.commit_profile_configuration(profile).unwrap();

        for _ in 0..2 {
            let tick = clock.plan_tick_count_sample(1).unwrap();
            assert_eq!(clock.commit_tick_count_sample(tick).unwrap(), 3_600_000);
            let counter = clock.plan_performance_counter_sample(10_000).unwrap();
            assert_eq!(
                clock.commit_performance_counter_sample(counter).unwrap(),
                12_345_678_901
            );
        }
        assert_eq!(clock.timer_state().current_time_100ns, 0);

        let advance = clock.plan_advance(25_000).unwrap();
        assert_eq!(clock.timer_state().current_time_100ns, 0);
        clock.commit_advance(advance).unwrap();
        assert_eq!(clock.state().wall_clock_file_time, 134_282_016_000_025_000);
        assert_eq!(clock.state().tick_count, 3_600_002);
        assert_eq!(clock.state().performance_counter, 12_345_703_901);
        assert_eq!(clock.timer_state().current_time_100ns, 25_000);
        assert_eq!(clock.timer_state().generation, 1);
    }

    #[test]
    fn scheduled_profile_and_advance_failures_are_atomic() {
        let mut clock = clock();
        let invalid = RuntimeClockProfile {
            id: "wrong-scheduled-profile".to_owned(),
            ..RuntimeClockProfile::scheduled_shared_v1()
        };
        let before = clock.clone();
        assert_eq!(
            clock.plan_profile_configuration(invalid).unwrap_err().code,
            "scheduled-clock-profile-identity-invalid"
        );
        assert_eq!(clock, before);

        let profile = clock
            .plan_profile_configuration(RuntimeClockProfile::scheduled_shared_v1())
            .unwrap();
        clock.commit_profile_configuration(profile).unwrap();
        let advance = clock.plan_advance(1).unwrap();
        let competing = clock.plan_advance(2).unwrap();
        clock.commit_advance(competing).unwrap();
        let before_stale = clock.clone();
        assert_eq!(
            clock.commit_advance(advance).unwrap_err().code,
            "timer-clock-advance-plan-stale"
        );
        assert_eq!(clock, before_stale);
        assert_eq!(
            clock.plan_advance(1).unwrap_err().code,
            "timer-clock-target-regressed"
        );
    }

    #[test]
    fn scheduled_read_plans_reject_an_intervening_subresolution_advance() {
        let mut clock = clock();
        let profile = clock
            .plan_profile_configuration(RuntimeClockProfile::scheduled_shared_v1())
            .unwrap();
        clock.commit_profile_configuration(profile).unwrap();
        let tick = clock.plan_tick_count_sample(1).unwrap();
        let counter = clock.plan_performance_counter_sample(10_000).unwrap();
        let advance = clock.plan_advance(1).unwrap();
        clock.commit_advance(advance).unwrap();

        let before = clock.clone();
        assert_eq!(
            clock.commit_tick_count_sample(tick).unwrap_err().code,
            "tick-count-sample-plan-stale"
        );
        assert_eq!(clock, before);
        assert_eq!(
            clock
                .commit_performance_counter_sample(counter)
                .unwrap_err()
                .code,
            "performance-counter-sample-plan-stale"
        );
        assert_eq!(clock, before);
    }

    #[test]
    fn scheduled_profile_fixture_is_the_validated_public_selection() {
        let profile: RuntimeClockProfile = serde_json::from_str(include_str!(
            "../../../../fixtures/runtime-clock-profiles/scheduled-shared-logical-deadline-v1.json"
        ))
        .unwrap();
        profile.validate().unwrap();
        assert_eq!(profile, RuntimeClockProfile::scheduled_shared_v1());

        let invalid_legacy = RuntimeClockProfile {
            id: "renamed-api-read-clock".to_owned(),
            ..RuntimeClockProfile::api_read_increment_v1()
        };
        assert_eq!(
            invalid_legacy.validate().unwrap_err().code,
            "api-read-clock-profile-identity-invalid"
        );
    }
}

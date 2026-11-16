use super::RuntimeStateError;

const FILE_TIME_TICKS_PER_SECOND: u128 = 10_000_000;
const FILE_TIME_TICKS_PER_MILLISECOND: u64 = 10_000;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeClockState {
    pub wall_clock_file_time: u64,
    pub tick_count: u32,
    pub performance_counter: u64,
    pub performance_frequency: u64,
}

#[derive(Debug)]
pub(super) struct RuntimeClock {
    epoch: RuntimeClockState,
    state: RuntimeClockState,
    timer_time_100ns: u64,
    retired_instructions: u64,
    host_elapsed_time_100ns: u64,
}

impl RuntimeClock {
    pub(super) fn new(state: RuntimeClockState) -> Result<Self, RuntimeStateError> {
        validate_clock_state(&state)?;
        Ok(Self {
            epoch: state.clone(),
            state,
            timer_time_100ns: 0,
            retired_instructions: 0,
            host_elapsed_time_100ns: 0,
        })
    }

    pub(super) fn state(&self) -> &RuntimeClockState {
        &self.state
    }

    pub(super) fn timer_time_100ns(&self) -> u64 {
        self.timer_time_100ns
    }

    pub(super) fn synchronized_instruction_count(&self) -> u64 {
        self.retired_instructions
    }

    pub(super) fn host_elapsed_time_100ns(&self) -> u64 {
        self.host_elapsed_time_100ns
    }

    pub(super) fn timestamp_counter(&self) -> u64 {
        // Only real host observations drive TSC. A completed timer can project
        // the Windows timer timeline forward, but cannot invent CPU ticks.
        // Keep multiplication exact before architectural 64-bit wrap. Equal
        // coarsened browser observations intentionally return equal counters.
        ((u128::from(self.host_elapsed_time_100ns)
            * u128::from(vine_cpu_profile::TSC_FREQUENCY_HZ))
            / FILE_TIME_TICKS_PER_SECOND) as u64
    }

    pub(super) fn project_advance(
        &self,
        target_time_100ns: u64,
    ) -> Result<(u64, RuntimeClockState), RuntimeStateError> {
        if target_time_100ns < self.timer_time_100ns {
            return Err(RuntimeStateError::new(
                "logical timer clock cannot move backwards",
            ));
        }
        let next_state = project_scheduled_state(&self.epoch, target_time_100ns)?;
        Ok((target_time_100ns, next_state))
    }

    pub(super) fn advance(&mut self, target_time_100ns: u64) -> Result<u64, RuntimeStateError> {
        let (timer, state) = self.project_advance(target_time_100ns)?;
        self.timer_time_100ns = timer;
        self.state = state;
        Ok(timer)
    }

    pub(super) fn advance_scheduled_host_execution(
        &mut self,
        retired_instructions: u64,
        host_elapsed_time_100ns: u64,
    ) -> Result<u64, RuntimeStateError> {
        if retired_instructions < self.retired_instructions {
            return Err(RuntimeStateError::new(
                "retired guest instruction count cannot move backwards",
            ));
        }
        if host_elapsed_time_100ns < self.host_elapsed_time_100ns {
            return Err(RuntimeStateError::new(
                "host elapsed clock cannot move backwards",
            ));
        }
        let target_time_100ns = host_elapsed_time_100ns.max(self.timer_time_100ns);
        let timer = self.advance(target_time_100ns)?;
        self.retired_instructions = retired_instructions;
        self.host_elapsed_time_100ns = host_elapsed_time_100ns;
        Ok(timer)
    }

    pub(super) fn apply_advance(&mut self, projection: (u64, RuntimeClockState)) {
        (self.timer_time_100ns, self.state) = projection;
    }
}

fn validate_clock_state(state: &RuntimeClockState) -> Result<(), RuntimeStateError> {
    if state.performance_frequency == 0 {
        return Err(RuntimeStateError::new(
            "runtime performance-counter frequency must be nonzero",
        ));
    }
    if state.performance_frequency > i64::MAX as u64 {
        return Err(RuntimeStateError::new(
            "runtime performance-counter frequency must fit a nonnegative LARGE_INTEGER",
        ));
    }
    if state.performance_counter > i64::MAX as u64 {
        return Err(RuntimeStateError::new(
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
            RuntimeStateError::new("scheduled shared clock exceeded the FILETIME range")
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
            RuntimeStateError::new(
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
    fn timestamp_counter_uses_real_elapsed_and_does_not_increment_on_reads() {
        let mut clock = clock();
        assert_eq!(clock.timestamp_counter(), 0);
        clock.advance(20_000_000).unwrap();
        clock.advance_scheduled_host_execution(12, 1).unwrap();
        assert_eq!(clock.timestamp_counter(), 100);
        assert_eq!(clock.timer_time_100ns(), 20_000_000);
        let before = clock.state().clone();
        for retired in [12, 13, 10_000_000] {
            clock.advance_scheduled_host_execution(retired, 1).unwrap();
            assert_eq!(clock.timestamp_counter(), 100);
            assert_eq!(clock.timestamp_counter(), 100);
            assert_eq!(*clock.state(), before);
        }
        clock
            .advance_scheduled_host_execution(10_000_000, 10_000_001)
            .unwrap();
        assert_eq!(clock.timestamp_counter(), 1_000_000_100);
        assert_eq!(clock.timer_time_100ns(), 20_000_000);
    }

    #[test]
    fn timestamp_counter_wraps_without_host_clock_regression() {
        let mut clock = clock();
        for (elapsed, expected) in [(42_949_672, 4_294_967_200), (42_949_673, 4_294_967_300)] {
            clock.advance_scheduled_host_execution(0, elapsed).unwrap();
            assert_eq!(clock.timestamp_counter(), expected);
        }
        // These large host elapsed values still fit FILETIME and QPC. The TSC
        // wraps first because its rate is 100 times the 100ns transport rate.
        let before_wrap = u64::MAX / 100;
        clock
            .advance_scheduled_host_execution(0, before_wrap)
            .unwrap();
        assert_eq!(clock.timestamp_counter(), u64::MAX - 15);
        clock
            .advance_scheduled_host_execution(0, before_wrap + 1)
            .unwrap();
        assert_eq!(clock.timestamp_counter(), 84);
    }

    #[test]
    fn rejected_timestamp_observations_preserve_all_clock_fields() {
        let mut clock = clock();
        clock.advance_scheduled_host_execution(7, 100).unwrap();
        let state = clock.state().clone();
        for (retired, elapsed) in [(7, 99), (6, 101), (8, u64::MAX)] {
            assert!(
                clock
                    .advance_scheduled_host_execution(retired, elapsed)
                    .is_err()
            );
            assert_eq!(*clock.state(), state);
            assert_eq!(clock.timer_time_100ns(), 100);
            assert_eq!(clock.host_elapsed_time_100ns(), 100);
            assert_eq!(clock.synchronized_instruction_count(), 7);
            assert_eq!(clock.timestamp_counter(), 10_000);
        }
    }

    #[test]
    fn scheduled_clock_projects_one_elapsed_timeline() {
        let mut clock = clock();
        assert_eq!(clock.state().tick_count, 3_600_000);
        assert_eq!(clock.state().performance_counter, 12_345_678_901);
        assert_eq!(clock.timer_time_100ns(), 0);

        clock.advance(25_000).unwrap();
        assert_eq!(clock.state().wall_clock_file_time, 134_282_016_000_025_000);
        assert_eq!(clock.state().tick_count, 3_600_002);
        assert_eq!(clock.state().performance_counter, 12_345_703_901);
        assert_eq!(clock.timer_time_100ns(), 25_000);
    }

    #[test]
    fn scheduled_advance_is_idempotent_and_rejects_regression_before_mutation() {
        let mut clock = clock();
        clock.advance(2).unwrap();
        assert!(clock.project_advance(1).is_err());
        assert!(clock.advance(1).is_err());
        assert_eq!(clock.timer_time_100ns(), 2);
        assert_eq!(clock.state().performance_counter, 12_345_678_903);
        assert_eq!(clock.state().wall_clock_file_time, 134_282_016_000_000_002);

        clock.advance(2).unwrap();
        assert_eq!(clock.timer_time_100ns(), 2);
        assert_eq!(clock.state().performance_counter, 12_345_678_903);
        let (target, projected) = clock.project_advance(2).unwrap();
        assert_eq!(target, 2);
        assert_eq!(projected.performance_counter, 12_345_678_903);
        clock.advance(3).unwrap();
        assert_eq!(clock.timer_time_100ns(), 3);
        assert_eq!(clock.state().performance_counter, 12_345_678_904);
    }

    #[test]
    fn audio_observes_real_host_time_independently_of_timer_projection() {
        let mut clock = clock();
        clock.advance(2_000_000).unwrap();
        clock
            .advance_scheduled_host_execution(1, 1_000_000)
            .unwrap();
        assert_eq!(clock.timer_time_100ns(), 2_000_000);
        assert_eq!(clock.host_elapsed_time_100ns(), 1_000_000);
        assert!(clock.advance_scheduled_host_execution(2, 999_999).is_err());
        assert_eq!(clock.host_elapsed_time_100ns(), 1_000_000);
        assert_eq!(clock.synchronized_instruction_count(), 1);
        assert!(clock.advance_scheduled_host_execution(2, u64::MAX).is_err());
        assert_eq!(clock.host_elapsed_time_100ns(), 1_000_000);
        assert_eq!(clock.synchronized_instruction_count(), 1);
    }

    #[test]
    fn scheduled_reads_observe_subresolution_advance_without_mutating_clock() {
        let mut clock = clock();
        clock.advance(1).unwrap();
        assert_eq!(clock.state().tick_count, 3_600_000);
        assert_eq!(clock.state().performance_counter, 12_345_678_902);
    }

    #[test]
    fn scheduled_projection_checks_overflow_before_mutating() {
        let mut clock = clock();
        assert!(clock.advance(u64::MAX).is_err());
        assert_eq!(clock.timer_time_100ns(), 0);
        assert_eq!(clock.state().performance_counter, 12_345_678_901);
        assert!(
            clock
                .advance_scheduled_host_execution(100, u64::MAX)
                .is_err()
        );
        assert_eq!(clock.timer_time_100ns(), 0);
        assert_eq!(clock.state().performance_counter, 12_345_678_901);
        assert_eq!(clock.synchronized_instruction_count(), 0);
        clock.advance(1).unwrap();
        assert_eq!(clock.timer_time_100ns(), 1);
        assert_eq!(clock.state().performance_counter, 12_345_678_902);
        let epoch = RuntimeClockState {
            tick_count: u32::MAX,
            ..clock.state().clone()
        };
        assert_eq!(
            project_scheduled_state(&epoch, 10_000).unwrap().tick_count,
            0
        );
    }
}

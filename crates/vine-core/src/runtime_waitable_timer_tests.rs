use crate::runtime::{
    RuntimeHandle, RuntimeObjectKind, RuntimeWaitDisposition, RuntimeWaitForSingleObjectExBegin,
    RuntimeWaitForSingleObjectExOutcome, RuntimeWaitState, RuntimeWaitableTimerArm,
    RuntimeWaitableTimerExpiration, RuntimeWaitableTimerId, X86_STARTUP_INFO_W_BYTE_LENGTH,
};
use crate::{
    RUNTIME_WAITABLE_TIMER_PACING_SERVICE, Runtime, RuntimeBootstrap, RuntimeClockState,
    RuntimeCurrentDirectoryA, RuntimeProcessId, RuntimeProcessImagePathA, RuntimeStartupInfo,
    RuntimeThreadId, SchedulerThreadState,
};

const PROCESS_ID: u32 = 4096;
const THREAD_ID: RuntimeThreadId = RuntimeThreadId(8192);

#[test]
fn scheduled_shared_clock_accepts_authoritative_host_elapsed_time_without_cpu_inflation() {
    let mut runtime = runtime(false);
    runtime
        .start_initial_thread()
        .expect("start host-synchronized clock fixture");

    runtime
        .synchronize_scheduled_clock_for_host_execution(8_000_000, 250_000)
        .expect("commit measured host elapsed time");
    assert_eq!(runtime.timer_time_100ns(), 250_000);
    assert_eq!(runtime.clock().tick_count, 3_600_025);
    assert_eq!(runtime.synchronized_instruction_count(), 8_000_000);

    runtime
        .advance_scheduled_clock_to(400_000)
        .expect("scheduler deadline may lead the sampled host clock");
    runtime
        .synchronize_scheduled_clock_for_host_execution(16_000_000, 300_000)
        .expect("a lagging host sample must not regress the logical deadline");
    assert_eq!(runtime.timer_time_100ns(), 400_000);
    assert_eq!(runtime.clock().tick_count, 3_600_040);

    let clock_before = runtime.clock().clone();
    let execution_before = runtime.synchronized_instruction_count();
    runtime
        .synchronize_scheduled_clock_for_host_execution(15_999_999, 500_000)
        .expect_err("host cursor regressed");
    assert_eq!(runtime.clock(), &clock_before);
    assert_eq!(runtime.synchronized_instruction_count(), execution_before);
    assert_eq!(runtime.timer_time_100ns(), 400_000);
    runtime
        .synchronize_scheduled_clock_for_host_execution(24_000_000, 500_000)
        .unwrap();
    assert_eq!(runtime.timer_time_100ns(), 500_000);
}

#[test]
fn scheduled_shared_clock_advances_only_at_scheduler_commit_points() {
    let mut runtime = runtime(false);
    runtime
        .start_initial_thread()
        .expect("start scheduled-clock fixture");

    for _ in 0..2 {
        assert_eq!(runtime.sample_tick_count().unwrap(), 3_600_000);
        assert_eq!(
            runtime.sample_performance_counter().unwrap(),
            12_345_678_901
        );
    }
    assert_eq!(runtime.timer_time_100ns(), 0);
    assert_eq!(
        runtime.clock().wall_clock_file_time,
        134_282_016_000_000_000
    );

    runtime
        .advance_scheduled_clock_to(25_000)
        .expect("commit one scheduler-selected absolute time");
    assert_eq!(
        runtime.clock().wall_clock_file_time,
        134_282_016_000_025_000
    );
    assert_eq!(runtime.clock().tick_count, 3_600_002);
    assert_eq!(runtime.clock().performance_counter, 12_345_703_901);
    assert_eq!(runtime.sample_tick_count().unwrap(), 3_600_002);
    assert_eq!(
        runtime.sample_performance_counter().unwrap(),
        12_345_703_901
    );

    assert_eq!(runtime.timer_time_100ns(), 25_000);
    runtime
        .advance_scheduled_clock_to(75_000)
        .expect("advance execution to the next logical deadline");
    assert_eq!(runtime.timer_time_100ns(), 75_000);
    assert_eq!(
        runtime.clock().wall_clock_file_time,
        134_282_016_000_075_000
    );
    assert_eq!(runtime.clock().tick_count, 3_600_007);
    assert_eq!(runtime.clock().performance_counter, 12_345_753_901);

    assert_eq!(runtime.timer_time_100ns(), 75_000);

    let clock_before = runtime.clock().clone();
    runtime
        .advance_scheduled_clock_to(74_999)
        .expect_err("scheduler deadline regressed");
    assert_eq!(runtime.clock(), &clock_before);
    assert_eq!(runtime.timer_time_100ns(), 75_000);
    runtime.advance_scheduled_clock_to(100_000).unwrap();
    assert_eq!(
        runtime.clock().wall_clock_file_time,
        134_282_016_000_100_000
    );
    assert_eq!(runtime.timer_time_100ns(), 100_000);
}

#[test]
fn waitable_timer_identity_close_and_thread_wait_cancellation_preserve_process_owned_timer() {
    let mut runtime = runtime(false);
    runtime
        .create_waitable_timer_a(true)
        .expect_err("bootstrapping process cannot create timer objects");
    assert_eq!(runtime.timer_time_100ns(), 0);
    assert_eq!(runtime.current_thread_id(), THREAD_ID);
    assert_eq!(runtime.current_last_error().unwrap(), 0);
    runtime
        .start_initial_thread()
        .expect("start controlled initial thread");

    let first = runtime
        .create_waitable_timer_a(true)
        .expect("create manual-reset waitable timer");
    assert_eq!(
        runtime.waitable_timer_for_handle(first).unwrap().id,
        RuntimeWaitableTimerId(1)
    );
    assert_eq!(first, RuntimeHandle(0x5701_0001));
    assert!(
        runtime
            .waitable_timer_for_handle(first)
            .expect("timer payload")
            .manual_reset
    );
    let object = runtime
        .object_for_handle(first, RuntimeObjectKind::WaitableTimer)
        .expect("typed timer object");
    assert_eq!(object.open_handles, 1);
    assert_eq!(
        runtime
            .object_wait_state(first, RuntimeObjectKind::WaitableTimer)
            .expect("timer initial wait state"),
        RuntimeWaitState::Nonsignaled
    );
    runtime
        .set_waitable_timer(first, 5_000)
        .expect("arm reached relative one-shot timer");
    assert_eq!(
        runtime.waitable_timer_for_handle(first).unwrap().arm,
        Some(RuntimeWaitableTimerArm {
            deadline_time_100ns: 5_000,
            generation: 1,
        })
    );
    runtime
        .set_waitable_timer(first, 20_000)
        .expect("rearm one-shot timer");
    assert_eq!(
        runtime.waitable_timer_for_handle(first).unwrap().arm,
        Some(RuntimeWaitableTimerArm {
            deadline_time_100ns: 20_000,
            generation: 2,
        })
    );
    assert_eq!(
        runtime
            .object_wait_state(first, RuntimeObjectKind::WaitableTimer)
            .expect("rearmed timer wait state"),
        RuntimeWaitState::Nonsignaled
    );
    let armed = *runtime.waitable_timer_for_handle(first).unwrap();
    runtime
        .set_waitable_timer(first, 0)
        .expect_err("zero relative due time is rejected");
    assert_eq!(runtime.waitable_timer_for_handle(first).unwrap(), &armed);
    assert_eq!(
        runtime
            .object_wait_state(first, RuntimeObjectKind::WaitableTimer)
            .unwrap(),
        RuntimeWaitState::Nonsignaled
    );
    assert_eq!(runtime.timer_time_100ns(), 0);

    let timer = runtime.waitable_timer_for_handle(first).unwrap();
    assert!(timer.manual_reset);
    assert_eq!(timer.arm.as_ref().unwrap().deadline_time_100ns, 20_000);

    runtime.close_handle(first).expect("close the timer handle");
    runtime
        .waitable_timer_for_handle(first)
        .expect_err("last close retires timer payload");
    runtime
        .set_waitable_timer(first, 5_000)
        .expect_err("stale timer handle cannot be armed");
    assert!(
        runtime
            .object_for_handle(first, RuntimeObjectKind::WaitableTimer)
            .is_err()
    );
    assert_eq!(runtime.timer_time_100ns(), 0);

    let second = runtime
        .create_waitable_timer_a(false)
        .expect("create auto-reset timer in the core object model");
    assert_eq!(
        runtime.waitable_timer_for_handle(second).unwrap().id,
        RuntimeWaitableTimerId(2)
    );
    assert_eq!(second, RuntimeHandle(0x5702_0001));
    assert!(
        !runtime
            .waitable_timer_for_handle(second)
            .unwrap()
            .manual_reset
    );
    let object_id = runtime
        .object_for_handle(second, RuntimeObjectKind::WaitableTimer)
        .unwrap()
        .id;
    assert!(matches!(
        runtime
            .wait_current_thread_on(second)
            .expect("nonsignaled timer registers a wait"),
        RuntimeWaitDisposition::Pending { registration }
            if registration.object_id == object_id && registration.handle == second
    ));

    runtime
        .request_current_thread_exit(7)
        .expect("thread exit cancels its timer wait");
    assert!(runtime.scheduler().wait_registrations().is_empty());
    assert!(runtime.scheduler().object_waiters().is_empty());
    runtime
        .complete_current_thread_exit()
        .expect("complete timer waiter exit");
    assert_eq!(
        runtime.waitable_timer_for_handle(second).unwrap().id,
        RuntimeWaitableTimerId(2)
    );
}

#[test]
fn ready_timer_polls_consume_exact_arms_without_advancing_time_or_registering_waits() {
    let mut runtime = runtime(true);
    for manual_reset in [true, false] {
        let timer = runtime.create_waitable_timer_a(manual_reset).unwrap();
        let clock_before = runtime.timer_time_100ns();
        runtime.set_waitable_timer(timer, 5_000).unwrap();
        let armed = *runtime.waitable_timer_for_handle(timer).unwrap();
        assert_eq!(
            runtime
                .begin_wait_for_single_object_ex(timer, 0, true)
                .unwrap(),
            RuntimeWaitForSingleObjectExBegin::Immediate {
                outcome: RuntimeWaitForSingleObjectExOutcome::TimedOut,
            }
        );
        assert_eq!(runtime.waitable_timer_for_handle(timer).unwrap(), &armed);
        assert_eq!(runtime.timer_time_100ns(), clock_before);
        assert_eq!(
            runtime.scheduler_state().unwrap(),
            SchedulerThreadState::Runnable
        );
        assert!(runtime.scheduler().host_requests().is_empty());
        assert!(runtime.scheduler().wait_registrations().is_empty());
        assert!(runtime.scheduler().object_waiters().is_empty());

        let deadline = runtime
            .waitable_timer_for_handle(timer)
            .unwrap()
            .arm
            .unwrap()
            .deadline_time_100ns;
        runtime.advance_scheduled_clock_to(deadline).unwrap();
        let clock = runtime.timer_time_100ns();
        assert_eq!(
            runtime
                .begin_wait_for_single_object_ex(timer, 0, true)
                .unwrap(),
            RuntimeWaitForSingleObjectExBegin::Immediate {
                outcome: RuntimeWaitForSingleObjectExOutcome::ObjectSignaled,
            }
        );
        let expired = runtime.waitable_timer_for_handle(timer).unwrap();
        assert_eq!(expired.arm, None);
        assert_eq!(
            expired.last_expiration,
            Some(RuntimeWaitableTimerExpiration {
                arm_generation: 1,
                deadline_time_100ns: deadline,
            })
        );
        assert_eq!(
            runtime
                .begin_wait_for_single_object_ex(timer, 0, false)
                .unwrap(),
            RuntimeWaitForSingleObjectExBegin::Immediate {
                outcome: if manual_reset {
                    RuntimeWaitForSingleObjectExOutcome::ObjectSignaled
                } else {
                    RuntimeWaitForSingleObjectExOutcome::TimedOut
                },
            }
        );
        assert_eq!(runtime.timer_time_100ns(), clock);
        assert_eq!(
            runtime.scheduler_state().unwrap(),
            SchedulerThreadState::Runnable
        );
        assert!(runtime.scheduler().wait_registrations().is_empty());
        assert!(runtime.scheduler().object_waiters().is_empty());
    }
}

#[test]
fn paced_timer_waits_bind_host_wait_identity_and_cancel_without_partial_time() {
    let mut runtime = runtime(true);
    runtime
        .begin_host_request(RUNTIME_WAITABLE_TIMER_PACING_SERVICE)
        .expect_err("timer pacing service is reserved for authoritative waits");
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::Runnable
    );
    assert!(runtime.scheduler().host_requests().is_empty());
    assert_eq!(runtime.timer_time_100ns(), 0);

    let timer = runtime
        .create_waitable_timer_a(true)
        .expect("create paced wait timer");
    runtime
        .set_waitable_timer(timer, 5_000)
        .expect("arm paced wait timer");

    let begin = runtime
        .begin_wait_for_single_object_ex(timer, 10, true)
        .expect("begin paced timer wait");
    let RuntimeWaitForSingleObjectExBegin::Deferred { registration } = begin else {
        panic!("positive timer delay must defer");
    };
    let pacing = registration
        .waitable_timer_pacing
        .expect("deferred wait carries pacing identity");
    assert_eq!(pacing.delay_milliseconds, 1);
    assert_eq!(pacing.target_time_100ns, 5_000);
    assert_eq!(
        pacing.outcome,
        RuntimeWaitForSingleObjectExOutcome::ObjectSignaled
    );
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::WaitingHost {
            request_id: pacing.request_id,
        }
    );
    assert_eq!(runtime.timer_time_100ns(), 0);
    assert_eq!(
        runtime
            .scheduler()
            .wait_registrations()
            .get(&registration.id),
        Some(&registration)
    );

    let timer_before = *runtime.waitable_timer_for_handle(timer).unwrap();
    let object_before = runtime
        .object_for_handle(timer, RuntimeObjectKind::WaitableTimer)
        .unwrap()
        .id;
    for rejected_operation in 0..3 {
        match rejected_operation {
            0 => {
                runtime
                    .complete_host_request(pacing.request_id)
                    .expect_err("timer pacing requires its atomic completion path");
            }
            1 => {
                runtime
                    .set_waitable_timer(timer, 10_000)
                    .expect_err("rearm while paced wait is pending");
            }
            2 => {
                runtime
                    .close_handle(timer)
                    .expect_err("close while paced wait is pending");
            }
            _ => unreachable!(),
        }
        assert_eq!(runtime.timer_time_100ns(), 0);
        assert_eq!(
            runtime.waitable_timer_for_handle(timer).unwrap(),
            &timer_before
        );
        assert_eq!(
            runtime
                .object_for_handle(timer, RuntimeObjectKind::WaitableTimer)
                .unwrap()
                .id,
            object_before
        );
        assert_eq!(
            runtime.scheduler_state().unwrap(),
            SchedulerThreadState::WaitingHost {
                request_id: pacing.request_id
            }
        );
        assert_eq!(
            runtime
                .scheduler()
                .wait_registrations()
                .get(&registration.id),
            Some(&registration)
        );
        assert_eq!(runtime.scheduler().host_requests().len(), 1);
        assert!(
            runtime
                .scheduler()
                .host_requests()
                .contains_key(&pacing.request_id)
        );
    }

    runtime
        .request_current_thread_exit(7)
        .expect("cancel paced host wait");
    assert!(runtime.scheduler().host_requests().is_empty());
    assert!(runtime.scheduler().wait_registrations().is_empty());
    assert!(runtime.scheduler().object_waiters().is_empty());
    assert_eq!(runtime.timer_time_100ns(), 0);
    assert_eq!(
        runtime
            .waitable_timer_for_handle(timer)
            .expect("cancellation leaves timer authority live")
            .arm,
        pacing.timer_before.arm
    );
    runtime
        .complete_host_request(pacing.request_id)
        .expect_err("late pacing acknowledgement is stale");
}

#[test]
fn waitable_timer_rearm_keeps_held_wait_identity_and_timeout() {
    let mut runtime = runtime(true);
    let signaler = RuntimeThreadId(THREAD_ID.0 + 1);
    runtime.register_thread(signaler, 0).unwrap();
    let timer = runtime.create_waitable_timer_a(true).unwrap();
    runtime.set_waitable_timer(timer, 5_000).unwrap();
    let RuntimeWaitForSingleObjectExBegin::Deferred { registration } = runtime
        .begin_wait_for_single_object_ex(timer, 10, true)
        .unwrap()
    else {
        panic!("armed timer wait must defer");
    };
    let requests = runtime.scheduler().host_requests().clone();
    let waiters = runtime.scheduler().object_waiters().clone();
    runtime.select_thread(signaler).unwrap();
    runtime.set_waitable_timer(timer, 20_000).unwrap();
    assert_eq!(
        runtime.waitable_timer_for_handle(timer).unwrap().arm,
        Some(RuntimeWaitableTimerArm {
            deadline_time_100ns: 20_000,
            generation: 2
        })
    );
    assert_eq!(
        runtime
            .object_wait_state(timer, RuntimeObjectKind::WaitableTimer)
            .unwrap(),
        RuntimeWaitState::Nonsignaled
    );
    assert_eq!(runtime.scheduler().host_requests(), &requests);
    assert_eq!(runtime.scheduler().object_waiters(), &waiters);
    assert_eq!(
        runtime
            .scheduler()
            .wait_registrations()
            .get(&registration.id),
        Some(&registration)
    );
    assert_eq!(
        runtime.scheduler_state_for_thread(THREAD_ID).unwrap(),
        SchedulerThreadState::WaitingHost {
            request_id: registration.waitable_timer_pacing.unwrap().request_id,
        }
    );
    assert_eq!(registration.timeout_deadline_time_100ns, Some(100_000));
    assert_eq!(runtime.timer_time_100ns(), 0);
    assert_eq!(runtime.current_last_error().unwrap(), 0);
    runtime.select_thread(THREAD_ID).unwrap();
    let request_id = registration.waitable_timer_pacing.unwrap().request_id;
    let plan = runtime
        .plan_waitable_timer_pacing_completion(request_id)
        .unwrap();
    assert_eq!(plan.registration(), registration);
    let retry = plan
        .repace()
        .expect("old deadline must not signal the rearmed timer");
    assert_eq!(retry.request_id, request_id);
    assert_eq!(retry.started_at_time_100ns, 5_000);
    assert_eq!(retry.target_time_100ns, 20_000);
    assert_eq!(retry.delay_milliseconds, 2);
    runtime.commit_waitable_timer_pacing_completion(plan);
    assert_eq!(runtime.timer_time_100ns(), 5_000);
    assert_eq!(runtime.scheduler().host_requests(), &requests);
    assert_eq!(runtime.scheduler().object_waiters(), &waiters);
    let held = runtime.scheduler().wait_registrations()[&registration.id];
    assert_eq!(
        held.timeout_deadline_time_100ns,
        registration.timeout_deadline_time_100ns
    );
    assert_eq!(held.waitable_timer_pacing, Some(retry));
    assert!(runtime.scheduler().events().is_empty());
    let finish = runtime
        .plan_waitable_timer_pacing_completion(request_id)
        .unwrap();
    assert!(finish.repace().is_none());
    assert_eq!(
        finish.outcome(),
        RuntimeWaitForSingleObjectExOutcome::ObjectSignaled
    );
    runtime.commit_waitable_timer_pacing_completion(finish);
    assert_eq!(runtime.timer_time_100ns(), 20_000);
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::Runnable
    );
    assert!(runtime.scheduler().host_requests().is_empty());
    assert!(runtime.scheduler().wait_registrations().is_empty());
    assert!(runtime.scheduler().object_waiters().is_empty());
    assert_eq!(
        runtime
            .waitable_timer_for_handle(timer)
            .unwrap()
            .last_expiration,
        Some(RuntimeWaitableTimerExpiration {
            arm_generation: 2,
            deadline_time_100ns: 20_000
        })
    );
}

#[test]
fn waitable_timer_rearm_invalid_due_and_handle_preserve_held_wait() {
    let mut runtime = runtime(true);
    let signaler = RuntimeThreadId(THREAD_ID.0 + 1);
    runtime.register_thread(signaler, 0x1234_5678).unwrap();
    let timer = runtime.create_waitable_timer_a(true).unwrap();
    runtime.advance_scheduled_clock_to(3_000).unwrap();
    runtime.set_waitable_timer(timer, 5_000).unwrap();
    let RuntimeWaitForSingleObjectExBegin::Deferred { registration } = runtime
        .begin_wait_for_single_object_ex(timer, 10, true)
        .unwrap()
    else {
        panic!("armed timer wait must defer");
    };
    runtime.select_thread(signaler).unwrap();
    let foreign = runtime.create_event(false, false).unwrap();
    for (handle, due) in [(timer, 0), (timer, u64::MAX), (foreign, 5_000)] {
        let before = *runtime.waitable_timer_for_handle(timer).unwrap();
        let object = runtime
            .object_for_handle(timer, RuntimeObjectKind::WaitableTimer)
            .unwrap()
            .clone();
        let requests = runtime.scheduler().host_requests().clone();
        let waits = runtime.scheduler().wait_registrations().clone();
        let waiters = runtime.scheduler().object_waiters().clone();
        let memory = runtime.memory.current_reflection().unwrap();
        runtime.set_waitable_timer(handle, due).unwrap_err();
        assert_eq!(runtime.waitable_timer_for_handle(timer).unwrap(), &before);
        assert_eq!(
            runtime
                .object_for_handle(timer, RuntimeObjectKind::WaitableTimer)
                .unwrap(),
            &object
        );
        assert_eq!(runtime.scheduler().host_requests(), &requests);
        assert_eq!(runtime.scheduler().wait_registrations(), &waits);
        assert_eq!(runtime.scheduler().object_waiters(), &waiters);
        assert_eq!(runtime.memory.current_reflection().unwrap(), memory);
        assert_eq!(runtime.timer_time_100ns(), 3_000);
        assert_eq!(
            runtime.scheduler_state_for_thread(THREAD_ID).unwrap(),
            SchedulerThreadState::WaitingHost {
                request_id: registration.waitable_timer_pacing.unwrap().request_id,
            }
        );
        assert_eq!(runtime.current_last_error().unwrap(), 0x1234_5678);
    }
}

#[test]
fn waitable_timer_rearm_resets_prior_signal_for_manual_and_auto_reset() {
    for manual in [true, false] {
        let mut runtime = runtime(true);
        let timer = runtime.create_waitable_timer_a(manual).unwrap();
        runtime.set_waitable_timer(timer, 5_000).unwrap();
        runtime.advance_scheduled_clock_to(5_000).unwrap();
        assert_eq!(
            runtime
                .begin_wait_for_single_object_ex(timer, 0, false)
                .unwrap(),
            RuntimeWaitForSingleObjectExBegin::Immediate {
                outcome: RuntimeWaitForSingleObjectExOutcome::ObjectSignaled
            }
        );
        runtime.set_waitable_timer(timer, 20_000).unwrap();
        assert_eq!(
            runtime
                .object_wait_state(timer, RuntimeObjectKind::WaitableTimer)
                .unwrap(),
            RuntimeWaitState::Nonsignaled
        );
        assert_eq!(
            runtime
                .begin_wait_for_single_object_ex(timer, 0, false)
                .unwrap(),
            RuntimeWaitForSingleObjectExBegin::Immediate {
                outcome: RuntimeWaitForSingleObjectExOutcome::TimedOut
            }
        );
        runtime.advance_scheduled_clock_to(25_000).unwrap();
        assert_eq!(
            runtime
                .begin_wait_for_single_object_ex(timer, 0, false)
                .unwrap(),
            RuntimeWaitForSingleObjectExBegin::Immediate {
                outcome: RuntimeWaitForSingleObjectExOutcome::ObjectSignaled
            }
        );
        assert_eq!(
            runtime
                .begin_wait_for_single_object_ex(timer, 0, false)
                .unwrap(),
            RuntimeWaitForSingleObjectExBegin::Immediate {
                outcome: if manual {
                    RuntimeWaitForSingleObjectExOutcome::ObjectSignaled
                } else {
                    RuntimeWaitForSingleObjectExOutcome::TimedOut
                }
            }
        );
    }
}

#[test]
fn waitable_timer_rearm_obeys_original_timeout_and_manual_or_auto_waiters() {
    for (manual, due) in [
        (true, 20_000),
        (false, 20_000),
        (true, 150_000),
        (true, 1_000),
    ] {
        let mut runtime = runtime(true);
        let second = RuntimeThreadId(THREAD_ID.0 + 1);
        let signaler = RuntimeThreadId(THREAD_ID.0 + 2);
        runtime.register_thread(second, 0).unwrap();
        runtime.register_thread(signaler, 0).unwrap();
        let timer = runtime.create_waitable_timer_a(manual).unwrap();
        runtime.set_waitable_timer(timer, 5_000).unwrap();
        let mut registrations = Vec::new();
        for thread in [THREAD_ID, second] {
            runtime.select_thread(thread).unwrap();
            let RuntimeWaitForSingleObjectExBegin::Deferred { registration } = runtime
                .begin_wait_for_single_object_ex(timer, 10, false)
                .unwrap()
            else {
                panic!("two nonsignaled timer waiters must defer");
            };
            registrations.push(registration);
        }
        runtime.select_thread(signaler).unwrap();
        runtime.set_waitable_timer(timer, due).unwrap();
        if due > 5_000 {
            for registration in &registrations {
                runtime.select_thread(registration.thread_id).unwrap();
                let request_id = registration.waitable_timer_pacing.unwrap().request_id;
                let plan = runtime
                    .plan_waitable_timer_pacing_completion(request_id)
                    .unwrap();
                let retry = plan
                    .repace()
                    .expect("old arm deadline cannot finish future live arm");
                assert_eq!(retry.target_time_100ns, due.min(100_000));
                runtime.commit_waitable_timer_pacing_completion(plan);
                assert_eq!(
                    runtime.scheduler_state().unwrap(),
                    SchedulerThreadState::WaitingHost { request_id }
                );
                assert_eq!(
                    runtime.scheduler().wait_registrations()[&registration.id]
                        .timeout_deadline_time_100ns,
                    Some(100_000)
                );
            }
            assert!(runtime.scheduler().events().is_empty());
        }
        runtime.select_thread(THREAD_ID).unwrap();
        let first_request = registrations[0].waitable_timer_pacing.unwrap().request_id;
        let first = runtime
            .plan_waitable_timer_pacing_completion(first_request)
            .unwrap();
        assert!(first.repace().is_none());
        assert_eq!(
            first.outcome(),
            if due > 100_000 {
                RuntimeWaitForSingleObjectExOutcome::TimedOut
            } else {
                RuntimeWaitForSingleObjectExOutcome::ObjectSignaled
            }
        );
        runtime.commit_waitable_timer_pacing_completion(first);
        if manual && due == 20_000 {
            // An already satisfied manual waiter survives reset/rearm before its host ack.
            runtime.select_thread(signaler).unwrap();
            runtime.set_waitable_timer(timer, 40_000).unwrap();
        }
        runtime.select_thread(second).unwrap();
        let second_request = registrations[1].waitable_timer_pacing.unwrap().request_id;
        let second_plan = runtime
            .plan_waitable_timer_pacing_completion(second_request)
            .unwrap();
        if !manual {
            let retry = second_plan
                .repace()
                .expect("auto-reset expiration satisfies only one waiter");
            assert_eq!(retry.target_time_100ns, 100_000);
            runtime.commit_waitable_timer_pacing_completion(second_plan);
            assert_eq!(
                runtime.scheduler_state().unwrap(),
                SchedulerThreadState::WaitingHost {
                    request_id: second_request
                }
            );
            let timeout = runtime
                .plan_waitable_timer_pacing_completion(second_request)
                .unwrap();
            assert!(timeout.repace().is_none());
            assert_eq!(
                timeout.outcome(),
                RuntimeWaitForSingleObjectExOutcome::TimedOut
            );
            runtime.commit_waitable_timer_pacing_completion(timeout);
        } else {
            assert!(second_plan.repace().is_none());
            assert_eq!(
                second_plan.outcome(),
                if due > 100_000 {
                    RuntimeWaitForSingleObjectExOutcome::TimedOut
                } else {
                    RuntimeWaitForSingleObjectExOutcome::ObjectSignaled
                }
            );
            runtime.commit_waitable_timer_pacing_completion(second_plan);
        }
        assert!(runtime.scheduler().host_requests().is_empty());
        assert!(runtime.scheduler().wait_registrations().is_empty());
        assert!(runtime.scheduler().object_waiters().is_empty());
        let current_timer = runtime.waitable_timer_for_handle(timer).unwrap();
        if due > 100_000 {
            assert_eq!(current_timer.arm.unwrap().deadline_time_100ns, due);
            assert!(current_timer.last_expiration.is_none());
        } else if manual && due == 20_000 {
            assert!(current_timer.last_expiration.is_none());
        } else {
            assert_eq!(
                current_timer.last_expiration,
                Some(RuntimeWaitableTimerExpiration {
                    arm_generation: 2,
                    deadline_time_100ns: due
                })
            );
        }
        if manual && due == 20_000 {
            assert_eq!(
                current_timer.arm,
                Some(RuntimeWaitableTimerArm {
                    generation: 3,
                    deadline_time_100ns: 60_000
                })
            );
            assert_eq!(
                runtime
                    .object_wait_state(timer, RuntimeObjectKind::WaitableTimer)
                    .unwrap(),
                RuntimeWaitState::Nonsignaled
            );
        }
    }
}

#[test]
fn waitable_timer_rearm_late_ack_keeps_timeout_before_other_waiters_signal() {
    let mut runtime = runtime(true);
    let short_waiter = RuntimeThreadId(THREAD_ID.0 + 1);
    let signaler = RuntimeThreadId(THREAD_ID.0 + 2);
    runtime.register_thread(short_waiter, 0).unwrap();
    runtime.register_thread(signaler, 0).unwrap();
    let timer = runtime.create_waitable_timer_a(true).unwrap();
    runtime.set_waitable_timer(timer, 5_000).unwrap();
    let mut registrations = Vec::new();
    for (thread, timeout) in [(THREAD_ID, 10), (short_waiter, 1)] {
        runtime.select_thread(thread).unwrap();
        let RuntimeWaitForSingleObjectExBegin::Deferred { registration } = runtime
            .begin_wait_for_single_object_ex(timer, timeout, false)
            .unwrap()
        else {
            panic!("positive timer waits must defer");
        };
        registrations.push(registration);
    }
    runtime.select_thread(signaler).unwrap();
    runtime.set_waitable_timer(timer, 20_000).unwrap();
    for (registration, target) in registrations.iter().zip([20_000, 10_000]) {
        runtime.select_thread(registration.thread_id).unwrap();
        let request = registration.waitable_timer_pacing.unwrap().request_id;
        let plan = runtime
            .plan_waitable_timer_pacing_completion(request)
            .unwrap();
        assert_eq!(plan.repace().unwrap().target_time_100ns, target);
        runtime.commit_waitable_timer_pacing_completion(plan);
    }
    runtime.select_thread(THREAD_ID).unwrap();
    let long_request = registrations[0].waitable_timer_pacing.unwrap().request_id;
    let signal = runtime
        .plan_waitable_timer_pacing_completion(long_request)
        .unwrap();
    assert_eq!(
        signal.outcome(),
        RuntimeWaitForSingleObjectExOutcome::ObjectSignaled
    );
    runtime.commit_waitable_timer_pacing_completion(signal);
    assert_eq!(runtime.timer_time_100ns(), 20_000);
    runtime.select_thread(short_waiter).unwrap();
    let short_request = registrations[1].waitable_timer_pacing.unwrap().request_id;
    let late = runtime
        .plan_waitable_timer_pacing_completion(short_request)
        .unwrap();
    assert!(late.repace().is_none());
    assert_eq!(
        late.outcome(),
        RuntimeWaitForSingleObjectExOutcome::TimedOut
    );
    runtime.commit_waitable_timer_pacing_completion(late);
    assert_eq!(
        runtime
            .object_wait_state(timer, RuntimeObjectKind::WaitableTimer)
            .unwrap(),
        RuntimeWaitState::Signaled { exit_code: 0 }
    );
    assert!(runtime.scheduler().host_requests().is_empty());
    assert!(runtime.scheduler().wait_registrations().is_empty());
}

#[test]
fn waitable_timer_rearm_preserves_elapsed_arm_signal_before_pending_host_ack() {
    for manual in [true, false] {
        let mut runtime = runtime(true);
        let second = RuntimeThreadId(THREAD_ID.0 + 1);
        let signaler = RuntimeThreadId(THREAD_ID.0 + 2);
        runtime.register_thread(second, 0).unwrap();
        runtime.register_thread(signaler, 0).unwrap();
        let timer = runtime.create_waitable_timer_a(manual).unwrap();
        runtime.set_waitable_timer(timer, 5_000).unwrap();
        let mut registrations = Vec::new();
        for thread in [THREAD_ID, second] {
            runtime.select_thread(thread).unwrap();
            let RuntimeWaitForSingleObjectExBegin::Deferred { registration } = runtime
                .begin_wait_for_single_object_ex(timer, 10, false)
                .unwrap()
            else {
                panic!("two pending timer waits");
            };
            registrations.push(registration);
        }
        runtime.select_thread(signaler).unwrap();
        runtime.advance_scheduled_clock_to(10_000).unwrap();
        runtime.set_waitable_timer(timer, 40_000).unwrap();
        let new_arm = RuntimeWaitableTimerArm {
            generation: 2,
            deadline_time_100ns: 50_000,
        };
        assert_eq!(
            runtime.waitable_timer_for_handle(timer).unwrap().arm,
            Some(new_arm)
        );
        assert_eq!(
            runtime
                .object_wait_state(timer, RuntimeObjectKind::WaitableTimer)
                .unwrap(),
            RuntimeWaitState::Nonsignaled
        );
        runtime.select_thread(THREAD_ID).unwrap();
        let first = runtime
            .plan_waitable_timer_pacing_completion(
                registrations[0].waitable_timer_pacing.unwrap().request_id,
            )
            .unwrap();
        assert!(first.repace().is_none());
        assert_eq!(
            first.outcome(),
            RuntimeWaitForSingleObjectExOutcome::ObjectSignaled
        );
        runtime.commit_waitable_timer_pacing_completion(first);
        assert_eq!(
            runtime.waitable_timer_for_handle(timer).unwrap().arm,
            Some(new_arm)
        );
        assert_eq!(runtime.timer_time_100ns(), 10_000);
        runtime.select_thread(second).unwrap();
        let second_request = registrations[1].waitable_timer_pacing.unwrap().request_id;
        let second_plan = runtime
            .plan_waitable_timer_pacing_completion(second_request)
            .unwrap();
        if manual {
            assert!(second_plan.repace().is_none());
            assert_eq!(
                second_plan.outcome(),
                RuntimeWaitForSingleObjectExOutcome::ObjectSignaled
            );
            runtime.commit_waitable_timer_pacing_completion(second_plan);
            assert_eq!(
                runtime.waitable_timer_for_handle(timer).unwrap().arm,
                Some(new_arm)
            );
            assert_eq!(runtime.timer_time_100ns(), 10_000);
        } else {
            assert_eq!(second_plan.repace().unwrap().target_time_100ns, 50_000);
            runtime.commit_waitable_timer_pacing_completion(second_plan);
            let next_signal = runtime
                .plan_waitable_timer_pacing_completion(second_request)
                .unwrap();
            assert!(next_signal.repace().is_none());
            assert_eq!(
                next_signal.outcome(),
                RuntimeWaitForSingleObjectExOutcome::ObjectSignaled
            );
            runtime.commit_waitable_timer_pacing_completion(next_signal);
            assert_eq!(
                runtime
                    .waitable_timer_for_handle(timer)
                    .unwrap()
                    .last_expiration,
                Some(RuntimeWaitableTimerExpiration {
                    arm_generation: 2,
                    deadline_time_100ns: 50_000
                })
            );
        }
        assert!(runtime.scheduler().host_requests().is_empty());
        assert!(runtime.scheduler().wait_registrations().is_empty());
        assert!(runtime.scheduler().object_waiters().is_empty());
    }
}

fn runtime(started: bool) -> Runtime {
    let mut runtime = Runtime::new(
        RuntimeBootstrap {
            process_id: RuntimeProcessId(PROCESS_ID),
            initial_thread_id: THREAD_ID,
            initial_last_error: 0,
            clock: RuntimeClockState {
                wall_clock_file_time: 134_282_016_000_000_000,
                tick_count: 3_600_000,
                performance_counter: 12_345_678_901,
                performance_frequency: 10_000_000,
            },
            startup_info: RuntimeStartupInfo {
                bytes: vec![0; X86_STARTUP_INFO_W_BYTE_LENGTH],
            },
            process_image_path_a: RuntimeProcessImagePathA {
                bytes_with_nul: b"C:\\Vine\\fixture.exe\0".to_vec(),
            },
            initial_current_directory_a: RuntimeCurrentDirectoryA {
                bytes_with_nul: b"C:\\Vine\0".to_vec(),
            },
            encoded_pointer_cookie: 0xec01_9d3b,
        },
        crate::GuestMemory::new(crate::MemoryLimits::default()).unwrap(),
    )
    .expect("controlled runtime");
    if started {
        runtime
            .start_initial_thread()
            .expect("start controlled initial thread");
    }
    runtime
}

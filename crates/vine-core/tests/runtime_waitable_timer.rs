use vine_core::{
    LOGICAL_ENCODED_POINTER_NAMESPACE, ProcessAddressSpaceLayout,
    RUNTIME_WAITABLE_TIMER_OBJECT_NAMESPACE, RuntimeBootstrap, RuntimeClockState,
    RuntimeCurrentDirectoryA, RuntimeHandle, RuntimeObjectKind, RuntimeProcessId,
    RuntimeProcessImagePathA, RuntimeStartupInfo, RuntimeState, RuntimeThreadId,
    RuntimeWaitDisposition, RuntimeWaitForSingleObjectExOutcome, RuntimeWaitState,
    RuntimeWaitableTimerArm, RuntimeWaitableTimerExpiration, RuntimeWaitableTimerId,
    SchedulerThreadState, X86_STARTUP_INFO_W_BYTE_LENGTH,
};

const PROCESS_ID: u32 = 4096;
const THREAD_ID: RuntimeThreadId = RuntimeThreadId(8192);

#[test]
fn waitable_timer_identity_wait_state_close_and_process_teardown_are_atomic() {
    let mut runtime = runtime(false);
    let before_start = runtime.clone();
    assert_eq!(
        runtime
            .create_waitable_timer_a(true)
            .expect_err("bootstrapping process cannot create timer objects")
            .code,
        "waitable-timer-operation-state-invalid"
    );
    assert_eq!(runtime, before_start);
    runtime
        .start_initial_thread()
        .expect("start controlled initial thread");

    let first = runtime
        .create_waitable_timer_a(true)
        .expect("create manual-reset waitable timer");
    assert_eq!(first.timer_id, RuntimeWaitableTimerId(1));
    assert_eq!(first.handle, RuntimeHandle(0x5701_0001));
    assert!(
        runtime
            .waitable_timer_for_handle(first.handle)
            .expect("timer payload")
            .manual_reset
    );
    let object = runtime
        .object_for_handle(first.handle, RuntimeObjectKind::WaitableTimer)
        .expect("typed timer object");
    assert_eq!(object.namespace, RUNTIME_WAITABLE_TIMER_OBJECT_NAMESPACE);
    assert_eq!(object.open_handles, 1);
    assert_eq!(
        runtime
            .object_wait_state(first.handle, RuntimeObjectKind::WaitableTimer)
            .expect("timer initial wait state"),
        RuntimeWaitState::Nonsignaled
    );
    assert_eq!(
        runtime
            .set_waitable_timer(first.handle, 5_000)
            .expect("arm reached relative one-shot timer")
            .arm,
        Some(RuntimeWaitableTimerArm {
            relative_due_time_100ns: 5_000,
            armed_at_time_100ns: 0,
            deadline_time_100ns: 5_000,
            period_milliseconds: 0,
            generation: 1,
        })
    );
    assert_eq!(
        runtime
            .set_waitable_timer(first.handle, 20_000)
            .expect("rearm one-shot timer")
            .arm,
        Some(RuntimeWaitableTimerArm {
            relative_due_time_100ns: 20_000,
            armed_at_time_100ns: 0,
            deadline_time_100ns: 20_000,
            period_milliseconds: 0,
            generation: 2,
        })
    );
    assert_eq!(
        runtime
            .object_wait_state(first.handle, RuntimeObjectKind::WaitableTimer)
            .expect("rearmed timer wait state"),
        RuntimeWaitState::Nonsignaled
    );
    let before_zero_due_time = runtime.clone();
    assert_eq!(
        runtime
            .set_waitable_timer(first.handle, 0)
            .expect_err("zero relative due time is rejected")
            .code,
        "waitable-timer-relative-due-time-zero"
    );
    assert_eq!(runtime, before_zero_due_time);

    let process_json = serde_json::to_value(runtime.process()).expect("serialize process view");
    assert_eq!(process_json["waitableTimers"]["1"]["manualReset"], true);
    assert_eq!(
        process_json["waitableTimers"]["1"]["arm"],
        serde_json::json!({
            "relativeDueTime100ns": 20_000,
            "armedAtTime100ns": 0,
            "deadlineTime100ns": 20_000,
            "periodMilliseconds": 0,
            "generation": 2,
        })
    );

    runtime
        .close_handle(first.handle)
        .expect("close the timer handle");
    assert_eq!(
        runtime
            .waitable_timer_for_handle(first.handle)
            .expect_err("last close retires timer payload")
            .code,
        "runtime-handle-stale"
    );
    assert!(
        runtime
            .objects()
            .objects()
            .values()
            .all(|record| record.kind != RuntimeObjectKind::WaitableTimer)
    );
    let before_stale_arm = runtime.clone();
    assert_eq!(
        runtime
            .set_waitable_timer(first.handle, 5_000)
            .expect_err("stale timer handle cannot be armed")
            .code,
        "runtime-handle-stale"
    );
    assert_eq!(runtime, before_stale_arm);

    let second = runtime
        .create_waitable_timer_a(false)
        .expect("create auto-reset timer in the core object model");
    assert_eq!(second.timer_id, RuntimeWaitableTimerId(2));
    assert_eq!(second.handle, RuntimeHandle(0x5702_0001));
    assert!(
        !runtime
            .waitable_timer_for_handle(second.handle)
            .unwrap()
            .manual_reset
    );
    let object_id = runtime
        .object_for_handle(second.handle, RuntimeObjectKind::WaitableTimer)
        .unwrap()
        .id;
    assert!(matches!(
        runtime
            .wait_current_thread_on(second.handle)
            .expect("nonsignaled timer registers a wait"),
        RuntimeWaitDisposition::Pending { registration }
            if registration.object_id == object_id && registration.handle == second.handle
    ));

    let cancellation = runtime
        .terminate_current_process(7)
        .expect("process teardown cancels wait and retires timer");
    assert_eq!(cancellation.wait_registrations_cancelled, 1);
    assert!(
        runtime
            .objects()
            .objects()
            .values()
            .all(|record| record.kind != RuntimeObjectKind::WaitableTimer)
    );
    assert_eq!(
        runtime
            .waitable_timer_for_handle(second.handle)
            .expect_err("process teardown retires timer handle")
            .code,
        "runtime-handle-stale"
    );
    let process_json = serde_json::to_value(runtime.process()).expect("serialize terminal process");
    assert!(process_json.get("waitableTimers").is_none());
}

#[test]
fn finite_alertable_waits_advance_the_logical_timer_clock_and_consume_exact_arms() {
    let mut runtime = runtime(true);
    let manual = runtime
        .create_waitable_timer_a(true)
        .expect("create manual-reset timer");
    runtime
        .set_waitable_timer(manual.handle, 5_000)
        .expect("arm manual-reset timer");

    let satisfied = runtime
        .wait_for_single_object_ex(manual.handle, 10, true)
        .expect("timer deadline precedes wait timeout");
    assert_eq!(
        satisfied.outcome,
        RuntimeWaitForSingleObjectExOutcome::ObjectSignaled
    );
    let registration = satisfied
        .registration
        .expect("nonsignaled finite wait uses the wait queue");
    assert_eq!(registration.id.0, 1);
    assert_eq!(registration.thread_id, THREAD_ID);
    assert_eq!(registration.handle, manual.handle);
    assert_eq!(registration.timeout_deadline_time_100ns, Some(100_000));
    assert!(registration.alertable);
    assert_eq!(satisfied.timer_clock_before.current_time_100ns, 0);
    assert_eq!(satisfied.timer_clock_after.current_time_100ns, 5_000);
    assert_eq!(satisfied.timer_clock_after.generation, 1);
    assert_eq!(satisfied.timer.arm, None);
    assert_eq!(
        satisfied.timer.last_expiration,
        Some(RuntimeWaitableTimerExpiration {
            arm_generation: 1,
            deadline_time_100ns: 5_000,
        })
    );
    assert_eq!(
        satisfied.object_wait_state,
        RuntimeWaitState::Signaled { exit_code: 0 }
    );
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::Runnable
    );
    assert!(runtime.scheduler().wait_registrations().is_empty());
    assert!(runtime.scheduler().object_waiters().is_empty());
    assert_eq!(runtime.next_wait_registration_id(), 2);

    let repeated = runtime
        .wait_for_single_object_ex(manual.handle, 0, true)
        .expect("manual-reset signal remains observable");
    assert_eq!(
        repeated.outcome,
        RuntimeWaitForSingleObjectExOutcome::ObjectSignaled
    );
    assert_eq!(repeated.registration, None);
    assert_eq!(
        repeated.object_wait_state,
        RuntimeWaitState::Signaled { exit_code: 0 }
    );
    assert_eq!(repeated.timer_clock_after, satisfied.timer_clock_after);

    runtime
        .set_waitable_timer(manual.handle, 200_000)
        .expect("rearm manual timer beyond one wait timeout");
    assert_eq!(
        runtime
            .waitable_timer_for_handle(manual.handle)
            .unwrap()
            .arm,
        Some(RuntimeWaitableTimerArm {
            relative_due_time_100ns: 200_000,
            armed_at_time_100ns: 5_000,
            deadline_time_100ns: 205_000,
            period_milliseconds: 0,
            generation: 2,
        })
    );
    let timed_out = runtime
        .wait_for_single_object_ex(manual.handle, 10, false)
        .expect("timeout precedes the rearmed timer deadline");
    assert_eq!(
        timed_out.outcome,
        RuntimeWaitForSingleObjectExOutcome::TimedOut
    );
    let timeout_registration = timed_out.registration.expect("finite timeout registration");
    assert_eq!(timeout_registration.id.0, 2);
    assert_eq!(
        timeout_registration.timeout_deadline_time_100ns,
        Some(105_000)
    );
    assert!(!timeout_registration.alertable);
    assert_eq!(timed_out.timer_clock_after.current_time_100ns, 105_000);
    assert!(timed_out.timer.arm.is_some());
    assert_eq!(timed_out.object_wait_state, RuntimeWaitState::Nonsignaled);

    let second_wait = runtime
        .wait_for_single_object_ex(manual.handle, 10, true)
        .expect("second finite wait reaches the arm deadline");
    assert_eq!(
        second_wait.outcome,
        RuntimeWaitForSingleObjectExOutcome::ObjectSignaled
    );
    assert_eq!(second_wait.registration.unwrap().id.0, 3);
    assert_eq!(second_wait.timer_clock_after.current_time_100ns, 205_000);
    assert_eq!(
        second_wait.timer.last_expiration,
        Some(RuntimeWaitableTimerExpiration {
            arm_generation: 2,
            deadline_time_100ns: 205_000,
        })
    );
    assert_eq!(
        second_wait.object_wait_state,
        RuntimeWaitState::Signaled { exit_code: 0 }
    );

    let automatic = runtime
        .create_waitable_timer_a(false)
        .expect("create controlled auto-reset timer");
    runtime
        .set_waitable_timer(automatic.handle, 5_000)
        .expect("arm auto-reset timer");
    let automatic_wait = runtime
        .wait_for_single_object_ex(automatic.handle, 10, true)
        .expect("auto-reset timer satisfies one waiter");
    assert_eq!(
        automatic_wait.outcome,
        RuntimeWaitForSingleObjectExOutcome::ObjectSignaled
    );
    assert_eq!(automatic_wait.registration.unwrap().id.0, 4);
    assert_eq!(
        automatic_wait.object_wait_state,
        RuntimeWaitState::Nonsignaled
    );
    assert_eq!(automatic_wait.timer_clock_after.current_time_100ns, 210_000);

    let consumed = runtime
        .wait_for_single_object_ex(automatic.handle, 0, true)
        .expect("consumed auto-reset signal times out");
    assert_eq!(
        consumed.outcome,
        RuntimeWaitForSingleObjectExOutcome::TimedOut
    );
    assert_eq!(consumed.registration, None);
    assert_eq!(runtime.next_wait_registration_id(), 5);
    assert_eq!(consumed.timer_clock_after, automatic_wait.timer_clock_after);
}

fn runtime(started: bool) -> RuntimeState {
    let mut runtime = RuntimeState::new(RuntimeBootstrap {
        process_id: RuntimeProcessId(PROCESS_ID),
        initial_thread_id: THREAD_ID,
        initial_last_error: 0,
        address_space: ProcessAddressSpaceLayout::vine_v1(),
        clock: RuntimeClockState {
            wall_clock_file_time: 134_282_016_000_000_000,
            tick_count: 3_600_000,
            performance_counter: 12_345_678_901,
            performance_frequency: 10_000_000,
        },
        startup_info: RuntimeStartupInfo {
            profile_id: "runtime-waitable-timer-test".to_owned(),
            bytes: vec![0; X86_STARTUP_INFO_W_BYTE_LENGTH],
        },
        process_image_path_a: RuntimeProcessImagePathA {
            source_authority: "runtime-waitable-timer-fixture".to_owned(),
            bytes_with_nul: b"C:\\Vine\\fixture.exe\0".to_vec(),
            encoding: vine_core::RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING.to_owned(),
        },
        initial_current_directory_a: RuntimeCurrentDirectoryA {
            source_authority: "runtime-waitable-timer-process-bootstrap".to_owned(),
            bytes_with_nul: b"C:\\Vine\0".to_vec(),
            encoding: vine_core::RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING.to_owned(),
        },
        encoded_pointer_namespace: LOGICAL_ENCODED_POINTER_NAMESPACE.to_owned(),
        encoded_pointer_token_prefix: 0xec01_0000,
    })
    .expect("controlled runtime");
    if started {
        runtime
            .start_initial_thread()
            .expect("start controlled initial thread");
    }
    runtime
}

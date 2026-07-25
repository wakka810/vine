use vine_core::{
    LOGICAL_ENCODED_POINTER_NAMESPACE, ProcessAddressSpaceLayout,
    RUNTIME_WAITABLE_TIMER_OBJECT_NAMESPACE, RuntimeBootstrap, RuntimeClockState,
    RuntimeCurrentDirectoryA, RuntimeHandle, RuntimeObjectKind, RuntimeProcessId,
    RuntimeProcessImagePathA, RuntimeStartupInfo, RuntimeState, RuntimeThreadId,
    RuntimeWaitDisposition, RuntimeWaitState, RuntimeWaitableTimerArm, RuntimeWaitableTimerId,
    X86_STARTUP_INFO_W_BYTE_LENGTH,
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

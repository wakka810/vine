use vine_core::{
    GuestAddress, LOGICAL_ENCODED_POINTER_NAMESPACE, ProcessAddressSpaceLayout,
    RUNTIME_API_BOUNDARY_QUANTUM, RuntimeApiBoundaryDisposition, RuntimeBootstrap,
    RuntimeClockState, RuntimeHandle, RuntimeObjectKind, RuntimeProcessId,
    RuntimeProcessLifecycleState, RuntimeStartupInfo, RuntimeState, RuntimeThreadId,
    RuntimeThreadLifecycleState, RuntimeThreadPriority, RuntimeTlsValue, RuntimeWaitState,
    SchedulerThreadState, X86_STARTUP_INFO_W_BYTE_LENGTH,
};

const PROCESS_ID: u32 = 4096;
const INITIAL_THREAD_ID: RuntimeThreadId = RuntimeThreadId(8192);
const SECOND_THREAD_ID: RuntimeThreadId = RuntimeThreadId(4097);

#[test]
fn api_boundary_quantum_selects_another_runnable_thread_without_immediate_preemption() {
    let mut runtime = running_runtime();
    runtime
        .register_thread(SECOND_THREAD_ID, 0)
        .expect("register second executable thread");

    for completed in 1..RUNTIME_API_BOUNDARY_QUANTUM {
        assert_eq!(
            runtime
                .complete_api_execution_boundary()
                .expect("complete API execution boundary"),
            None,
            "boundary {completed} must preserve the current quantum"
        );
        assert_eq!(runtime.current_thread_id(), INITIAL_THREAD_ID);
    }

    let selection = runtime
        .complete_api_execution_boundary()
        .expect("expire initial API quantum")
        .expect("second runnable thread is selected");
    assert_eq!(selection.previous_thread_id, INITIAL_THREAD_ID);
    assert_eq!(selection.thread_id, SECOND_THREAD_ID);
    assert_eq!(selection.state, SchedulerThreadState::Runnable);
    assert_eq!(runtime.current_thread_id(), SECOND_THREAD_ID);

    for _ in 1..RUNTIME_API_BOUNDARY_QUANTUM {
        assert_eq!(runtime.complete_api_execution_boundary().unwrap(), None);
    }
    assert_eq!(
        runtime
            .complete_api_execution_boundary()
            .unwrap()
            .expect("round robin returns to the initial thread")
            .thread_id,
        INITIAL_THREAD_ID
    );
}

#[test]
fn api_boundary_quantum_starts_only_after_a_second_runnable_lane_exists() {
    let mut runtime = running_runtime();
    for _ in 0..(RUNTIME_API_BOUNDARY_QUANTUM * 2) {
        assert_eq!(runtime.complete_api_execution_boundary().unwrap(), None);
        assert_eq!(runtime.current_thread_id(), INITIAL_THREAD_ID);
    }

    runtime
        .register_thread(SECOND_THREAD_ID, 0)
        .expect("register a newly contending thread");
    for completed in 1..RUNTIME_API_BOUNDARY_QUANTUM {
        assert_eq!(
            runtime.complete_api_execution_boundary().unwrap(),
            None,
            "fresh contention boundary {completed} must stay in the new quantum"
        );
    }
    assert_eq!(
        runtime
            .complete_api_execution_boundary()
            .unwrap()
            .expect("fresh contention expires after a full quantum")
            .thread_id,
        SECOND_THREAD_ID
    );
}

#[test]
fn api_boundaries_preempt_for_higher_priority_and_do_not_demote_a_running_high_priority_thread() {
    let mut runtime = running_runtime();
    let second_handle = runtime
        .register_thread(SECOND_THREAD_ID, 0)
        .expect("register second executable thread");
    assert_eq!(
        runtime
            .set_thread_priority(second_handle, RuntimeThreadPriority::AboveNormal)
            .expect("raise second thread priority"),
        RuntimeThreadPriority::Normal
    );

    let selection = runtime
        .complete_api_execution_boundary()
        .expect("complete preempting boundary")
        .expect("higher-priority runnable thread preempts at an API boundary");
    assert_eq!(selection.previous_thread_id, INITIAL_THREAD_ID);
    assert_eq!(selection.thread_id, SECOND_THREAD_ID);
    assert_eq!(runtime.current_thread_id(), SECOND_THREAD_ID);

    for _ in 0..(RUNTIME_API_BOUNDARY_QUANTUM * 2) {
        assert_eq!(runtime.complete_api_execution_boundary().unwrap(), None);
        assert_eq!(runtime.current_thread_id(), SECOND_THREAD_ID);
    }

    let yielded = runtime
        .complete_api_execution_boundary_with_disposition(
            RuntimeApiBoundaryDisposition::YieldCurrentThread,
        )
        .expect("yield higher-priority thread")
        .expect("explicit yield selects the lower-priority competitor");
    assert_eq!(yielded.thread_id, INITIAL_THREAD_ID);
    assert_eq!(runtime.current_thread_id(), INITIAL_THREAD_ID);

    let resumed = runtime
        .complete_api_execution_boundary()
        .expect("complete boundary after explicit yield")
        .expect("higher-priority thread becomes current again");
    assert_eq!(resumed.thread_id, SECOND_THREAD_ID);
    assert_eq!(runtime.current_thread_id(), SECOND_THREAD_ID);
}

#[test]
fn api_boundary_yield_selects_a_runnable_competitor_without_host_wait() {
    let mut runtime = running_runtime();
    assert_eq!(
        runtime
            .complete_api_execution_boundary_with_disposition(
                RuntimeApiBoundaryDisposition::YieldCurrentThread,
            )
            .unwrap(),
        None,
        "yield without a runnable competitor returns immediately"
    );
    assert_eq!(runtime.current_thread_id(), INITIAL_THREAD_ID);

    runtime
        .register_thread(SECOND_THREAD_ID, 0)
        .expect("register second executable thread");
    assert_eq!(runtime.complete_api_execution_boundary().unwrap(), None);
    let selection = runtime
        .complete_api_execution_boundary_with_disposition(
            RuntimeApiBoundaryDisposition::YieldCurrentThread,
        )
        .expect("complete yielding API boundary")
        .expect("yield selects the runnable competitor immediately");
    assert_eq!(selection.previous_thread_id, INITIAL_THREAD_ID);
    assert_eq!(selection.thread_id, SECOND_THREAD_ID);
    assert_eq!(selection.state, SchedulerThreadState::Runnable);
    assert_eq!(runtime.current_thread_id(), SECOND_THREAD_ID);

    for completed in 1..RUNTIME_API_BOUNDARY_QUANTUM {
        assert_eq!(
            runtime.complete_api_execution_boundary().unwrap(),
            None,
            "explicit yield must reset the following quantum at boundary {completed}"
        );
        assert_eq!(runtime.current_thread_id(), SECOND_THREAD_ID);
    }
    let reset_quantum_selection = runtime
        .complete_api_execution_boundary()
        .expect("complete reset quantum")
        .expect("the reset quantum selects only at its final boundary");
    assert_eq!(reset_quantum_selection.previous_thread_id, SECOND_THREAD_ID);
    assert_eq!(reset_quantum_selection.thread_id, INITIAL_THREAD_ID);
    assert_eq!(runtime.current_thread_id(), INITIAL_THREAD_ID);
}

#[test]
fn api_boundary_round_robin_preserves_and_restores_callback_thread_lanes() {
    let mut runtime = running_runtime();
    runtime
        .register_thread(SECOND_THREAD_ID, 0)
        .expect("register second executable thread");
    let callback = runtime
        .push_callback_frame(GuestAddress(0x1000_1000), GuestAddress(0x7f01_0000))
        .expect("enter callback on initial thread");

    for completed in 1..RUNTIME_API_BOUNDARY_QUANTUM {
        assert_eq!(
            runtime.complete_api_execution_boundary().unwrap(),
            None,
            "callback boundary {completed} must preserve its current quantum"
        );
    }
    let second = runtime
        .complete_api_execution_boundary()
        .expect("expire callback thread quantum")
        .expect("select runnable worker from callback thread");
    assert_eq!(second.previous_thread_id, INITIAL_THREAD_ID);
    assert_eq!(second.thread_id, SECOND_THREAD_ID);
    assert_eq!(runtime.current_thread_id(), SECOND_THREAD_ID);
    assert_eq!(
        runtime
            .scheduler_state_for_thread(INITIAL_THREAD_ID)
            .unwrap(),
        SchedulerThreadState::InCallback { depth: 1 }
    );

    let initial = runtime
        .complete_api_execution_boundary_with_disposition(
            RuntimeApiBoundaryDisposition::YieldCurrentThread,
        )
        .expect("yield worker thread")
        .expect("restore callback thread lane");
    assert_eq!(initial.thread_id, INITIAL_THREAD_ID);
    assert_eq!(initial.state, SchedulerThreadState::InCallback { depth: 1 });
    assert_eq!(
        runtime.current_thread_state().unwrap().callback_frames,
        vec![callback]
    );
    runtime
        .pop_callback_frame(callback.id)
        .expect("retire restored callback frame");
}

#[test]
fn executable_selection_runs_another_thread_while_host_completion_remains_thread_owned() {
    let mut runtime = running_runtime();
    runtime
        .register_thread(SECOND_THREAD_ID, 0)
        .expect("register second executable thread");
    let request = runtime
        .begin_host_request("initial-thread.pending")
        .expect("suspend initial thread on host work");

    let selection = runtime
        .select_next_executable_thread()
        .expect("plan executable selection")
        .expect("second thread is executable");
    assert_eq!(selection.previous_thread_id, INITIAL_THREAD_ID);
    assert_eq!(selection.thread_id, SECOND_THREAD_ID);
    assert_eq!(selection.state, SchedulerThreadState::Runnable);
    assert_eq!(runtime.current_thread_id(), SECOND_THREAD_ID);

    let completion = runtime
        .complete_host_request(request.request_id)
        .expect("complete non-current initial-thread request");
    assert_eq!(completion.thread_id, INITIAL_THREAD_ID);
    assert_eq!(runtime.current_thread_id(), SECOND_THREAD_ID);
    assert_eq!(
        runtime
            .scheduler_state_for_thread(INITIAL_THREAD_ID)
            .unwrap(),
        SchedulerThreadState::Runnable
    );
    let wrapped = runtime
        .select_next_executable_thread()
        .expect("wrap executable selection")
        .expect("initial thread is executable again");
    assert_eq!(wrapped.thread_id, INITIAL_THREAD_ID);
    assert_eq!(runtime.current_thread_id(), INITIAL_THREAD_ID);
}

#[test]
fn thread_registry_isolates_thread_state_and_terminates_all_lanes_deterministically() {
    let mut runtime = running_runtime();
    let process_handle = runtime.process_handle().expect("process handle");
    let initial_handle = runtime
        .current_thread_handle()
        .expect("initial thread handle");
    let tls_index = runtime.allocate_tls_index().expect("controlled TLS index");
    runtime.set_current_last_error(11).unwrap();
    runtime.set_tls_value(tls_index, 0x1111).unwrap();
    let initial_frame = runtime
        .push_callback_frame(GuestAddress(0x1000_1000), GuestAddress(0x7f01_0000))
        .expect("initial thread callback");

    let second_handle = runtime
        .register_thread(SECOND_THREAD_ID, 22)
        .expect("register controlled second thread");
    assert_eq!(second_handle, RuntimeHandle(0x5401_0002));
    assert_eq!(
        runtime.thread_state(SECOND_THREAD_ID).unwrap().lifecycle,
        RuntimeThreadLifecycleState::Running
    );
    assert_eq!(
        runtime
            .scheduler_state_for_thread(SECOND_THREAD_ID)
            .unwrap(),
        SchedulerThreadState::Runnable
    );
    assert!(
        runtime
            .thread_state(SECOND_THREAD_ID)
            .unwrap()
            .tls_values
            .is_empty()
    );
    assert!(
        runtime
            .thread_state(SECOND_THREAD_ID)
            .unwrap()
            .callback_frames
            .is_empty()
    );

    let before_duplicate = runtime.clone();
    assert_eq!(
        runtime
            .register_thread(SECOND_THREAD_ID, 33)
            .expect_err("duplicate TID is rejected without allocating another handle")
            .code,
        "thread-id-duplicate"
    );
    assert_eq!(runtime, before_duplicate);

    runtime
        .select_thread(SECOND_THREAD_ID)
        .expect("select second thread");
    assert_eq!(runtime.current_thread_id(), SECOND_THREAD_ID);
    assert_eq!(runtime.current_last_error().unwrap(), 22);
    assert_eq!(
        runtime
            .tls_value(tls_index)
            .expect_err("second thread does not inherit initial TLS")
            .code,
        "tls-value-uninitialized"
    );
    runtime.set_current_last_error(33).unwrap();
    runtime.set_tls_value(tls_index, 0x2222).unwrap();
    let second_frame = runtime
        .push_callback_frame(GuestAddress(0x2000_1000), GuestAddress(0x7f01_0010))
        .expect("second thread callback");
    assert_eq!(second_frame.thread_id, SECOND_THREAD_ID);

    runtime
        .select_thread(INITIAL_THREAD_ID)
        .expect("restore initial callback lane");
    assert_eq!(runtime.current_last_error().unwrap(), 11);
    assert_eq!(
        runtime.tls_value(tls_index).unwrap(),
        &RuntimeTlsValue::Raw { value: 0x1111 }
    );
    assert_eq!(
        runtime.current_thread_state().unwrap().callback_frames,
        vec![initial_frame]
    );
    runtime
        .pop_callback_frame(initial_frame.id)
        .expect("retire initial callback");

    runtime
        .select_thread(SECOND_THREAD_ID)
        .expect("restore second callback lane");
    assert_eq!(runtime.current_last_error().unwrap(), 33);
    assert_eq!(
        runtime.tls_value(tls_index).unwrap(),
        &RuntimeTlsValue::Raw { value: 0x2222 }
    );
    assert_eq!(
        runtime.current_thread_state().unwrap().callback_frames,
        vec![second_frame]
    );
    runtime
        .pop_callback_frame(second_frame.id)
        .expect("retire second callback");
    runtime
        .request_current_thread_exit(23)
        .expect("request second thread exit");
    runtime
        .complete_current_thread_exit()
        .expect("complete second thread exit");
    assert_eq!(
        runtime.process().lifecycle(),
        RuntimeProcessLifecycleState::Running
    );
    assert_eq!(
        runtime.thread_state(SECOND_THREAD_ID).unwrap().lifecycle,
        RuntimeThreadLifecycleState::Exited { exit_code: 23 }
    );
    assert_eq!(
        runtime
            .object_wait_state(second_handle, RuntimeObjectKind::Thread)
            .unwrap(),
        RuntimeWaitState::Signaled { exit_code: 23 }
    );
    assert_eq!(
        runtime
            .object_wait_state(process_handle, RuntimeObjectKind::Process)
            .unwrap(),
        RuntimeWaitState::Nonsignaled
    );
    let before_exited_selection = runtime.clone();
    assert_eq!(
        runtime
            .select_thread(SECOND_THREAD_ID)
            .expect_err("exited thread cannot become current")
            .code,
        "thread-selection-state-invalid"
    );
    assert_eq!(runtime, before_exited_selection);
    runtime
        .select_thread(INITIAL_THREAD_ID)
        .expect("select surviving initial thread");
    assert_eq!(runtime.current_thread_handle().unwrap(), initial_handle);

    let mut process_exit = running_runtime();
    let process_handle = process_exit.process_handle().unwrap();
    let initial_handle = process_exit.current_thread_handle().unwrap();
    let second_handle = process_exit
        .register_thread(SECOND_THREAD_ID, 0)
        .expect("register process-exit second thread");
    process_exit
        .push_callback_frame(GuestAddress(0x1000_2000), GuestAddress(0x7f01_0000))
        .unwrap();
    process_exit
        .begin_host_request("initial-thread.pending")
        .unwrap();
    process_exit.select_thread(SECOND_THREAD_ID).unwrap();
    process_exit
        .push_callback_frame(GuestAddress(0x2000_2000), GuestAddress(0x7f01_0010))
        .unwrap();
    process_exit
        .begin_host_request("second-thread.pending")
        .unwrap();
    process_exit.select_thread(INITIAL_THREAD_ID).unwrap();

    let termination = process_exit
        .terminate_current_process(7)
        .expect("terminate all controlled lanes");
    assert_eq!(termination.initiating_thread_id, INITIAL_THREAD_ID);
    assert_eq!(termination.host_requests_cancelled, 2);
    assert_eq!(termination.callback_frames_cancelled, 2);
    assert_eq!(termination.scheduler_events_discarded, 0);
    assert_eq!(
        termination
            .thread_cancellations
            .iter()
            .map(|cancellation| cancellation.thread_id)
            .collect::<Vec<_>>(),
        vec![SECOND_THREAD_ID, INITIAL_THREAD_ID]
    );
    for (thread_id, handle) in [
        (SECOND_THREAD_ID, second_handle),
        (INITIAL_THREAD_ID, initial_handle),
    ] {
        assert_eq!(
            process_exit.thread_state(thread_id).unwrap().lifecycle,
            RuntimeThreadLifecycleState::Exited { exit_code: 7 }
        );
        assert_eq!(
            process_exit.scheduler_state_for_thread(thread_id).unwrap(),
            SchedulerThreadState::Terminated { exit_code: 7 }
        );
        assert_eq!(
            process_exit
                .object_wait_state(handle, RuntimeObjectKind::Thread)
                .unwrap(),
            RuntimeWaitState::Signaled { exit_code: 7 }
        );
    }
    assert_eq!(
        process_exit.process().lifecycle(),
        RuntimeProcessLifecycleState::Exited { exit_code: 7 }
    );
    assert_eq!(
        process_exit
            .object_wait_state(process_handle, RuntimeObjectKind::Process)
            .unwrap(),
        RuntimeWaitState::Signaled { exit_code: 7 }
    );
    assert!(process_exit.scheduler().host_requests().is_empty());
    assert!(process_exit.scheduler().events().is_empty());

    let mut rejected_process_exit = running_runtime();
    rejected_process_exit
        .register_thread(SECOND_THREAD_ID, 0)
        .expect("register negative-path second thread");
    rejected_process_exit
        .request_current_thread_exit(19)
        .expect("leave initial thread exit pending");
    rejected_process_exit
        .select_thread(SECOND_THREAD_ID)
        .expect("select live thread after initial exit request");
    let before_rejected_process_exit = rejected_process_exit.clone();
    assert_eq!(
        rejected_process_exit
            .terminate_current_process(7)
            .expect_err("process exit rejects a preexisting thread-exit transition")
            .code,
        "process-exit-thread-state-invalid"
    );
    assert_eq!(rejected_process_exit, before_rejected_process_exit);
}

#[test]
fn thread_resource_ownership_preserves_the_runtime_serialization_shape() {
    let mut runtime = running_runtime();
    let tls_index = runtime.allocate_tls_index().expect("allocate TLS index");
    let fls_slot = runtime
        .allocate_fls_slot(Some(GuestAddress(0x1000_2000)))
        .expect("allocate FLS slot")
        .clone();
    let callback = runtime
        .push_callback_frame(GuestAddress(0x1000_3000), GuestAddress(0x7f01_0000))
        .expect("push callback frame");

    let serialized = serde_json::to_value(&runtime).expect("serialize thread resource view");
    assert_eq!(
        serialized["process"]["allocatedTlsIndices"],
        serde_json::json!([tls_index])
    );
    assert_eq!(
        serialized["process"]["flsSlots"][fls_slot.index.to_string()]["callback"],
        serde_json::json!(fls_slot.callback)
    );
    assert_eq!(serialized["nextTlsIndex"], serde_json::json!(1));
    assert_eq!(serialized["nextFlsIndex"], serde_json::json!(1));
    assert_eq!(serialized["nextCallbackFrameId"], serde_json::json!(2));
    assert_eq!(
        serialized["threads"][INITIAL_THREAD_ID.0.to_string()]["callbackFrames"][0]["id"],
        serde_json::json!(callback.id.0)
    );
    assert!(serialized["threads"].get("allocatedTlsIndices").is_none());
    assert!(serialized["threads"].get("flsSlots").is_none());
    assert_eq!(
        runtime.allocated_tls_indices(),
        &std::collections::BTreeSet::from([0])
    );
    assert_eq!(runtime.fls_slots().get(&0), Some(&fls_slot));
}

fn running_runtime() -> RuntimeState {
    let mut runtime = RuntimeState::new(RuntimeBootstrap {
        process_id: RuntimeProcessId(PROCESS_ID),
        initial_thread_id: INITIAL_THREAD_ID,
        initial_last_error: 87,
        address_space: ProcessAddressSpaceLayout::vine_v1(),
        clock: RuntimeClockState {
            wall_clock_file_time: 134_282_016_000_000_000,
            tick_count: 3_600_000,
            performance_counter: 12_345_678_901,
            performance_frequency: 10_000_000,
        },
        startup_info: RuntimeStartupInfo {
            profile_id: "runtime-thread-registry-test".to_owned(),
            bytes: vec![0; X86_STARTUP_INFO_W_BYTE_LENGTH],
        },
        process_image_path_a: vine_core::RuntimeProcessImagePathA {
            source_authority: "runtime-thread-registry-fixture".to_owned(),
            bytes_with_nul: b"C:\\Vine\\fixture.exe\0".to_vec(),
            encoding: vine_core::RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING.to_owned(),
        },
        initial_current_directory_a: vine_core::RuntimeCurrentDirectoryA {
            source_authority: "runtime-thread-registry-process-bootstrap".to_owned(),
            bytes_with_nul: b"C:\\Vine\0".to_vec(),
            encoding: vine_core::RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING.to_owned(),
        },
        encoded_pointer_namespace: LOGICAL_ENCODED_POINTER_NAMESPACE.to_owned(),
        encoded_pointer_cookie: 0xec01_9d3b,
    })
    .expect("initialized controlled runtime");
    runtime
        .start_initial_thread()
        .expect("start controlled initial thread");
    runtime
}

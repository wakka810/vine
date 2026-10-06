use crate::{
    GuestAddress, RUNTIME_API_BOUNDARY_QUANTUM, Runtime, RuntimeApiBoundaryDisposition,
    RuntimeBootstrap, RuntimeClockState, RuntimeHandle, RuntimeObjectKind, RuntimeProcessId,
    RuntimeProcessLifecycleState, RuntimeStartupInfo, RuntimeThreadId, RuntimeThreadLifecycleState,
    RuntimeThreadPriority, RuntimeWaitState, SchedulerThreadState, X86_STARTUP_INFO_W_BYTE_LENGTH,
};

const PROCESS_ID: u32 = 4096;
const INITIAL_THREAD_ID: RuntimeThreadId = RuntimeThreadId(8192);
const SECOND_THREAD_ID: RuntimeThreadId = RuntimeThreadId(4097);

#[test]
fn executable_selection_preserves_cyclic_priority_order_with_blocked_lanes() {
    let ids = [
        RuntimeThreadId(4097),
        INITIAL_THREAD_ID,
        RuntimeThreadId(8193),
        RuntimeThreadId(9000),
    ];
    for seed in 0..32 {
        let mut runtime = running_runtime();
        let mut priorities = std::collections::BTreeMap::from([(INITIAL_THREAD_ID, 0)]);
        for (index, &id) in ids.iter().enumerate() {
            if id == INITIAL_THREAD_ID {
                continue;
            }
            let handle = runtime.register_thread(id, 0).unwrap();
            let rank = (seed + index) % 3;
            runtime
                .set_thread_priority(
                    handle,
                    match rank {
                        0 => RuntimeThreadPriority::BelowNormal,
                        1 => RuntimeThreadPriority::Normal,
                        _ => RuntimeThreadPriority::AboveNormal,
                    },
                )
                .unwrap();
            priorities.insert(id, rank as i32 - 1);
        }
        for (index, &id) in ids.iter().enumerate() {
            if seed & (1 << index) != 0 {
                runtime.select_thread(id).unwrap();
                runtime.begin_host_request("selection-order-test").unwrap();
            }
        }
        for &current in &ids {
            runtime.select_thread(current).unwrap();
            let start = ids.iter().position(|id| *id == current).unwrap();
            let mut expected = None;
            for offset in 1..=ids.len() {
                let id = ids[(start + offset) % ids.len()];
                if runtime
                    .scheduler_state_for_thread(id)
                    .unwrap()
                    .is_executable()
                    && expected.is_none_or(|previous| priorities[&id] > priorities[&previous])
                {
                    expected = Some(id);
                }
            }
            let actual = runtime.select_next_executable_thread().unwrap();
            assert_eq!(actual, expected, "seed {seed}");
        }
    }
}

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
    assert_eq!(selection, SECOND_THREAD_ID);
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::Runnable
    );
    assert_eq!(runtime.current_thread_id(), SECOND_THREAD_ID);

    for _ in 1..RUNTIME_API_BOUNDARY_QUANTUM {
        assert_eq!(runtime.complete_api_execution_boundary().unwrap(), None);
    }
    assert_eq!(
        runtime
            .complete_api_execution_boundary()
            .unwrap()
            .expect("round robin returns to the initial thread"),
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
            .expect("fresh contention expires after a full quantum"),
        SECOND_THREAD_ID
    );
}

#[test]
fn api_boundaries_preempt_for_higher_priority_and_do_not_demote_a_running_high_priority_thread() {
    let mut runtime = running_runtime();
    let second_handle = runtime
        .register_thread(SECOND_THREAD_ID, 0)
        .expect("register second executable thread");
    assert!(
        runtime
            .set_thread_priority(second_handle, RuntimeThreadPriority::AboveNormal)
            .expect("raise second thread priority")
    );

    let selection = runtime
        .complete_api_execution_boundary()
        .expect("complete preempting boundary")
        .expect("higher-priority runnable thread preempts at an API boundary");
    assert_eq!(selection, SECOND_THREAD_ID);
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
    assert_eq!(yielded, INITIAL_THREAD_ID);
    assert_eq!(runtime.current_thread_id(), INITIAL_THREAD_ID);

    let resumed = runtime
        .complete_api_execution_boundary()
        .expect("complete boundary after explicit yield")
        .expect("higher-priority thread becomes current again");
    assert_eq!(resumed, SECOND_THREAD_ID);
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
    assert_eq!(selection, SECOND_THREAD_ID);
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::Runnable
    );
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
    assert_eq!(reset_quantum_selection, INITIAL_THREAD_ID);
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
    assert_eq!(second, SECOND_THREAD_ID);
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
    assert_eq!(initial, INITIAL_THREAD_ID);
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::InCallback { depth: 1 }
    );
    assert_eq!(
        runtime.current_thread_state().unwrap().callback_frames,
        vec![callback]
    );
    runtime
        .pop_callback_frame(callback.id)
        .expect("retire restored callback frame");
}

#[test]
fn host_completions_resume_only_their_owner_when_delivered_out_of_order() {
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
    assert_eq!(selection, SECOND_THREAD_ID);
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::Runnable
    );
    assert_eq!(runtime.current_thread_id(), SECOND_THREAD_ID);

    let younger = runtime.begin_host_request("second-thread.ready").unwrap();
    let completed = runtime.complete_host_request(younger.request_id).unwrap();
    assert_eq!(completed.thread_id, SECOND_THREAD_ID);
    assert_eq!(runtime.current_thread_id(), SECOND_THREAD_ID);
    assert_eq!(
        runtime
            .scheduler_state_for_thread(INITIAL_THREAD_ID)
            .unwrap(),
        SchedulerThreadState::WaitingHost {
            request_id: request.request_id,
        }
    );
    assert_eq!(runtime.scheduler().host_requests().len(), 1);
    let events_before_duplicate = runtime.scheduler().events().clone();
    runtime
        .complete_host_request(younger.request_id)
        .expect_err("completed request cannot complete twice");
    assert_eq!(runtime.current_thread_id(), SECOND_THREAD_ID);
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::Runnable
    );
    assert_eq!(runtime.scheduler().host_requests().len(), 1);
    assert_eq!(
        runtime.scheduler().host_requests().get(&request.request_id),
        Some(&request)
    );
    assert_eq!(
        runtime
            .scheduler_state_for_thread(INITIAL_THREAD_ID)
            .unwrap(),
        SchedulerThreadState::WaitingHost {
            request_id: request.request_id
        }
    );
    assert_eq!(runtime.scheduler().events(), &events_before_duplicate);

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
    assert_eq!(wrapped, INITIAL_THREAD_ID);
    assert_eq!(runtime.current_thread_id(), INITIAL_THREAD_ID);
}

#[test]
fn thread_registry_isolates_thread_state_and_preserves_surviving_lane_on_exit() {
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

    for _ in 0..3 {
        runtime.set_current_last_error(11).unwrap();
        assert_eq!(runtime.current_last_error().unwrap(), 11);
        assert_eq!(runtime.tls_value(tls_index), Ok(0x1111));
        assert_eq!(
            runtime
                .current_thread_state()
                .unwrap()
                .callback_frames
                .as_slice(),
            &[initial_frame]
        );
        assert_eq!(
            runtime.scheduler_state().unwrap(),
            SchedulerThreadState::InCallback { depth: 1 }
        );
    }

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

    for id in [RuntimeThreadId(0), SECOND_THREAD_ID] {
        runtime
            .register_thread(id, 33)
            .expect_err("invalid or duplicate TID must not publish a thread");
        assert_eq!(runtime.scheduler().thread_states().len(), 2);
        assert_eq!(runtime.current_thread_id(), INITIAL_THREAD_ID);
        assert_eq!(runtime.current_last_error().unwrap(), 11);
        assert_eq!(runtime.tls_value(tls_index), Ok(0x1111));
        assert_eq!(
            runtime
                .current_thread_state()
                .unwrap()
                .callback_frames
                .as_slice(),
            &[initial_frame]
        );
        assert_eq!(
            runtime.thread_handle(SECOND_THREAD_ID).unwrap(),
            second_handle
        );
        let second = runtime.thread_state(SECOND_THREAD_ID).unwrap();
        assert_eq!(second.lifecycle, RuntimeThreadLifecycleState::Running);
        assert_eq!(second.last_error, 22);
        assert!(second.tls_values.is_empty());
        assert!(second.callback_frames.is_empty());
    }

    runtime
        .select_thread(SECOND_THREAD_ID)
        .expect("select second thread");
    assert_eq!(runtime.current_thread_id(), SECOND_THREAD_ID);
    assert_eq!(runtime.current_last_error().unwrap(), 22);
    assert_eq!(runtime.tls_value_or_unset(tls_index).unwrap(), None);
    assert!(runtime.tls_value(tls_index).is_err());
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
    assert_eq!(runtime.tls_value(tls_index).unwrap(), 0x1111);
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
    assert_eq!(runtime.tls_value(tls_index).unwrap(), 0x2222);
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
    runtime
        .select_thread(SECOND_THREAD_ID)
        .expect_err("exited thread cannot become executable");
    assert_eq!(runtime.current_thread_id(), SECOND_THREAD_ID);
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::Terminated { exit_code: 23 }
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
    runtime
        .select_thread(INITIAL_THREAD_ID)
        .expect("select surviving initial thread");
    assert_eq!(runtime.current_thread_handle().unwrap(), initial_handle);
}

#[test]
fn thread_resource_ownership_tracks_tls_fls_and_callbacks() {
    let mut runtime = running_runtime();
    let tls_index = runtime.allocate_tls_index().expect("allocate TLS index");
    let fls_slot = runtime
        .allocate_fls_slot(Some(GuestAddress(0x1000_2000)))
        .expect("allocate FLS slot");
    let callback = runtime
        .push_callback_frame(GuestAddress(0x1000_3000), GuestAddress(0x7f01_0000))
        .expect("push callback frame");

    assert_eq!(tls_index, 0);
    assert_eq!(callback.id.0, 1);
    assert_eq!(
        runtime.allocated_tls_indices(),
        &std::collections::BTreeSet::from([0])
    );
    assert_eq!(fls_slot, 0);
    runtime.set_fls_value(fls_slot, 42).unwrap();
    assert!(!runtime.free_fls_slot(fls_slot).unwrap());
    assert_eq!(runtime.fls_value(fls_slot).unwrap(), 42);
}

fn running_runtime() -> Runtime {
    let mut runtime = Runtime::new(
        RuntimeBootstrap {
            process_id: RuntimeProcessId(PROCESS_ID),
            initial_thread_id: INITIAL_THREAD_ID,
            initial_last_error: 87,
            clock: RuntimeClockState {
                wall_clock_file_time: 134_282_016_000_000_000,
                tick_count: 3_600_000,
                performance_counter: 12_345_678_901,
                performance_frequency: 10_000_000,
            },
            startup_info: RuntimeStartupInfo {
                bytes: vec![0; X86_STARTUP_INFO_W_BYTE_LENGTH],
            },
            process_image_path_a: crate::RuntimeProcessImagePathA {
                bytes_with_nul: b"C:\\Vine\\fixture.exe\0".to_vec(),
            },
            initial_current_directory_a: crate::RuntimeCurrentDirectoryA {
                bytes_with_nul: b"C:\\Vine\0".to_vec(),
            },
            encoded_pointer_cookie: 0xec01_9d3b,
        },
        crate::GuestMemory::new(crate::MemoryLimits::default()).unwrap(),
    )
    .expect("initialized controlled runtime");
    runtime
        .start_initial_thread()
        .expect("start controlled initial thread");
    runtime
}

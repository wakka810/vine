use vine_core::{
    LOGICAL_ENCODED_POINTER_NAMESPACE, ProcessAddressSpaceLayout, RuntimeBootstrap,
    RuntimeClockState, RuntimeHandle, RuntimeObjectKind, RuntimeProcessId, RuntimeSchedulerEvent,
    RuntimeStartupInfo, RuntimeState, RuntimeThreadId, RuntimeWaitDisposition,
    RuntimeWaitRegistrationId, RuntimeWaitState, SchedulerThreadState,
    X86_STARTUP_INFO_W_BYTE_LENGTH,
};

const PROCESS_ID: u32 = 4096;
const INITIAL_THREAD_ID: RuntimeThreadId = RuntimeThreadId(8192);
const SECOND_THREAD_ID: RuntimeThreadId = RuntimeThreadId(4097);

#[test]
fn wait_queue_wakes_on_thread_signal_and_cancels_on_process_exit() {
    let mut runtime = running_runtime();
    let second_handle = runtime
        .register_thread(SECOND_THREAD_ID, 0)
        .expect("register controlled signaler");
    let second_object_id = runtime
        .object_for_handle(second_handle, RuntimeObjectKind::Thread)
        .expect("second thread object")
        .id;

    let heap_handle = RuntimeHandle(
        runtime
            .allocate_private_heap(0, 0x1000, 0)
            .expect("nonwaitable control object")
            .handle,
    );
    let before_nonwaitable = runtime.clone();
    assert_eq!(
        runtime
            .wait_current_thread_on(heap_handle)
            .expect_err("heap object cannot enter the wait queue")
            .code,
        "runtime-object-not-waitable"
    );
    assert_eq!(runtime, before_nonwaitable);

    let registration = match runtime
        .wait_current_thread_on(second_handle)
        .expect("wait on nonsignaled second thread")
    {
        RuntimeWaitDisposition::Pending { registration } => registration,
        other => panic!("expected pending registration, got {other:?}"),
    };
    assert_eq!(registration.id, RuntimeWaitRegistrationId(1));
    assert_eq!(registration.thread_id, INITIAL_THREAD_ID);
    assert_eq!(registration.object_id, second_object_id);
    assert_eq!(registration.handle, second_handle);
    assert_eq!(
        runtime
            .scheduler_state_for_thread(INITIAL_THREAD_ID)
            .unwrap(),
        SchedulerThreadState::WaitingObject {
            registration_id: registration.id,
            object_id: second_object_id,
        }
    );
    assert_eq!(
        runtime
            .scheduler()
            .object_waiters()
            .get(&second_object_id)
            .expect("second thread wait queue")
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        vec![registration.id]
    );

    let before_duplicate = runtime.clone();
    assert_eq!(
        runtime
            .wait_current_thread_on(second_handle)
            .expect_err("waiting thread cannot register a second wait")
            .code,
        "wait-registration-thread-state-invalid"
    );
    assert_eq!(runtime, before_duplicate);

    runtime
        .select_thread(SECOND_THREAD_ID)
        .expect("select controlled signaler");
    let request = runtime
        .request_current_thread_exit(23)
        .expect("request signaler exit");
    assert_eq!(request.wait_registrations_cancelled, 0);
    assert_eq!(
        runtime
            .scheduler_state_for_thread(INITIAL_THREAD_ID)
            .unwrap(),
        SchedulerThreadState::WaitingObject {
            registration_id: registration.id,
            object_id: second_object_id,
        }
    );
    assert_eq!(
        runtime
            .object_wait_state(second_handle, RuntimeObjectKind::Thread)
            .unwrap(),
        RuntimeWaitState::Nonsignaled
    );

    runtime
        .complete_current_thread_exit()
        .expect("signal thread object and wake waiter");
    assert_eq!(
        runtime
            .object_wait_state(second_handle, RuntimeObjectKind::Thread)
            .unwrap(),
        RuntimeWaitState::Signaled { exit_code: 23 }
    );
    assert_eq!(
        runtime
            .scheduler_state_for_thread(INITIAL_THREAD_ID)
            .unwrap(),
        SchedulerThreadState::Runnable
    );
    assert!(runtime.scheduler().wait_registrations().is_empty());
    assert!(runtime.scheduler().object_waiters().is_empty());
    let completion = match runtime.pop_scheduler_event() {
        Some(RuntimeSchedulerEvent::WaitSatisfied(completion)) => completion,
        other => panic!("expected exact wait completion, got {other:?}"),
    };
    assert_eq!(completion.registration, registration);
    assert_eq!(
        completion.wait_state,
        RuntimeWaitState::Signaled { exit_code: 23 }
    );
    assert!(runtime.pop_scheduler_event().is_none());

    runtime
        .select_thread(INITIAL_THREAD_ID)
        .expect("resume awakened waiter");
    assert_eq!(
        runtime
            .wait_current_thread_on(second_handle)
            .expect("signaled object completes without queueing"),
        RuntimeWaitDisposition::AlreadySignaled {
            object_id: second_object_id,
            wait_state: RuntimeWaitState::Signaled { exit_code: 23 },
        }
    );
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::Runnable
    );
    assert!(runtime.scheduler().wait_registrations().is_empty());
    assert!(runtime.scheduler().events().is_empty());

    let mut process_exit = running_runtime();
    let second_handle = process_exit
        .register_thread(SECOND_THREAD_ID, 0)
        .expect("register process-exit signaler");
    assert!(matches!(
        process_exit.wait_current_thread_on(second_handle).unwrap(),
        RuntimeWaitDisposition::Pending { .. }
    ));
    process_exit.select_thread(SECOND_THREAD_ID).unwrap();
    let termination = process_exit
        .terminate_current_process(7)
        .expect("process exit cancels wait instead of waking it");
    assert_eq!(termination.wait_registrations_cancelled, 1);
    assert_eq!(
        termination
            .thread_cancellations
            .iter()
            .map(|cancellation| (
                cancellation.thread_id,
                cancellation.wait_registrations_cancelled,
            ))
            .collect::<Vec<_>>(),
        vec![(SECOND_THREAD_ID, 0), (INITIAL_THREAD_ID, 1)]
    );
    assert!(process_exit.scheduler().wait_registrations().is_empty());
    assert!(process_exit.scheduler().object_waiters().is_empty());
    assert!(process_exit.scheduler().events().is_empty());
}

fn running_runtime() -> RuntimeState {
    let mut runtime = RuntimeState::new(RuntimeBootstrap {
        process_id: RuntimeProcessId(PROCESS_ID),
        initial_thread_id: INITIAL_THREAD_ID,
        initial_last_error: 0,
        address_space: ProcessAddressSpaceLayout::vine_v1(),
        clock: RuntimeClockState {
            wall_clock_file_time: 134_282_016_000_000_000,
            tick_count: 3_600_000,
            performance_counter: 12_345_678_901,
        },
        startup_info: RuntimeStartupInfo {
            profile_id: "runtime-wait-queue-test".to_owned(),
            bytes: vec![0; X86_STARTUP_INFO_W_BYTE_LENGTH],
        },
        process_image_path_a: vine_core::RuntimeProcessImagePathA {
            source_authority: "runtime-wait-queue-fixture".to_owned(),
            bytes_with_nul: b"C:\\Vine\\fixture.exe\0".to_vec(),
            encoding: vine_core::RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING.to_owned(),
        },
        initial_current_directory_a: vine_core::RuntimeCurrentDirectoryA {
            source_authority: "runtime-wait-queue-process-bootstrap".to_owned(),
            bytes_with_nul: b"C:\\Vine\0".to_vec(),
            encoding: vine_core::RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING.to_owned(),
        },
        encoded_pointer_namespace: LOGICAL_ENCODED_POINTER_NAMESPACE.to_owned(),
        encoded_pointer_token_prefix: 0xec01_0000,
    })
    .expect("initialized controlled runtime");
    runtime
        .start_initial_thread()
        .expect("start controlled initial thread");
    runtime
}

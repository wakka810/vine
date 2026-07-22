use vine_core::{
    LOGICAL_ENCODED_POINTER_NAMESPACE, ProcessAddressSpaceLayout, RuntimeBootstrap,
    RuntimeClockState, RuntimeMessageId, RuntimeMessageReceiveDisposition, RuntimeMessageWake,
    RuntimeProcessId, RuntimeSchedulerEvent, RuntimeStartupInfo, RuntimeState, RuntimeThreadId,
    RuntimeThreadMessage, SchedulerThreadState, X86_STARTUP_INFO_W_BYTE_LENGTH,
};

const PROCESS_ID: u32 = 4096;
const INITIAL_THREAD_ID: RuntimeThreadId = RuntimeThreadId(8192);
const SECOND_THREAD_ID: RuntimeThreadId = RuntimeThreadId(4097);
const CONTROLLED_HWND: u32 = 0x5701_0001;

#[test]
fn message_queue_preserves_fifo_owner_and_wakes_only_on_empty_transition() {
    let mut runtime = running_runtime();
    runtime
        .register_thread(SECOND_THREAD_ID, 0)
        .expect("register controlled message producer");

    assert_eq!(
        runtime
            .receive_current_thread_message()
            .expect("empty owner queue blocks"),
        RuntimeMessageReceiveDisposition::Blocked
    );
    assert_eq!(
        runtime
            .scheduler_state_for_thread(INITIAL_THREAD_ID)
            .unwrap(),
        SchedulerThreadState::WaitingMessage
    );
    let before_duplicate_receive = runtime.clone();
    assert_eq!(
        runtime
            .receive_current_thread_message()
            .expect_err("blocked owner cannot issue another receive")
            .code,
        "message-receive-thread-state-invalid"
    );
    assert_eq!(runtime, before_duplicate_receive);

    runtime.select_thread(SECOND_THREAD_ID).unwrap();
    let first = runtime
        .post_thread_message(INITIAL_THREAD_ID, CONTROLLED_HWND, 0x0401, 0x11, 0x22)
        .expect("first post wakes empty queue owner");
    let second = runtime
        .post_thread_message(INITIAL_THREAD_ID, CONTROLLED_HWND, 0x0402, 0x33, 0x44)
        .expect("second post preserves runnable owner");
    assert_eq!(first.id, RuntimeMessageId(1));
    assert_eq!(second.id, RuntimeMessageId(2));
    assert_eq!(first.source_thread_id, SECOND_THREAD_ID);
    assert_eq!(first.target_thread_id, INITIAL_THREAD_ID);
    assert_eq!(
        runtime
            .scheduler_state_for_thread(INITIAL_THREAD_ID)
            .unwrap(),
        SchedulerThreadState::Runnable
    );
    assert_eq!(
        runtime.queued_thread_messages(INITIAL_THREAD_ID).unwrap(),
        vec![first, second]
    );

    runtime.select_thread(INITIAL_THREAD_ID).unwrap();
    assert_eq!(
        runtime.pop_scheduler_event(),
        Some(RuntimeSchedulerEvent::MessageReady(RuntimeMessageWake {
            thread_id: INITIAL_THREAD_ID,
            message_id: first.id,
            queue_depth: 1,
        }))
    );
    assert!(runtime.pop_scheduler_event().is_none());
    assert_eq!(
        runtime.receive_current_thread_message().unwrap(),
        RuntimeMessageReceiveDisposition::Message { message: first }
    );
    assert_eq!(
        runtime.receive_current_thread_message().unwrap(),
        RuntimeMessageReceiveDisposition::Message { message: second }
    );
    assert_eq!(
        runtime.receive_current_thread_message().unwrap(),
        RuntimeMessageReceiveDisposition::Blocked
    );

    runtime.select_thread(SECOND_THREAD_ID).unwrap();
    let third = runtime
        .post_thread_message(INITIAL_THREAD_ID, 0, 0x0012, 7, 0)
        .expect("third post wakes reblocked owner");
    assert_eq!(third.id, RuntimeMessageId(3));
    runtime.select_thread(INITIAL_THREAD_ID).unwrap();
    assert_eq!(
        runtime.pop_scheduler_event(),
        Some(RuntimeSchedulerEvent::MessageReady(RuntimeMessageWake {
            thread_id: INITIAL_THREAD_ID,
            message_id: third.id,
            queue_depth: 1,
        }))
    );
    assert_eq!(
        runtime.receive_current_thread_message().unwrap(),
        RuntimeMessageReceiveDisposition::Message { message: third }
    );
    assert!(
        runtime
            .queued_thread_messages(INITIAL_THREAD_ID)
            .unwrap()
            .is_empty()
    );

    runtime.select_thread(SECOND_THREAD_ID).unwrap();
    runtime.request_current_thread_exit(23).unwrap();
    runtime.complete_current_thread_exit().unwrap();
    runtime.select_thread(INITIAL_THREAD_ID).unwrap();
    let before_exited_target = runtime.clone();
    assert_eq!(
        runtime
            .post_thread_message(SECOND_THREAD_ID, 0, 1, 0, 0)
            .expect_err("message cannot target exited thread")
            .code,
        "message-post-target-state-invalid"
    );
    assert_eq!(runtime, before_exited_target);

    let mut process_exit = running_runtime();
    process_exit
        .register_thread(SECOND_THREAD_ID, 0)
        .expect("register process-exit producer");
    process_exit.select_thread(SECOND_THREAD_ID).unwrap();
    let queued = [
        RuntimeThreadMessage {
            id: RuntimeMessageId(1),
            source_thread_id: SECOND_THREAD_ID,
            target_thread_id: INITIAL_THREAD_ID,
            hwnd: 0,
            message: 0x0501,
            w_param: 1,
            l_param: 2,
        },
        RuntimeThreadMessage {
            id: RuntimeMessageId(2),
            source_thread_id: SECOND_THREAD_ID,
            target_thread_id: INITIAL_THREAD_ID,
            hwnd: 0,
            message: 0x0502,
            w_param: 3,
            l_param: 4,
        },
    ];
    for expected in queued {
        assert_eq!(
            process_exit
                .post_thread_message(
                    expected.target_thread_id,
                    expected.hwnd,
                    expected.message,
                    expected.w_param,
                    expected.l_param,
                )
                .unwrap(),
            expected
        );
    }
    let termination = process_exit.terminate_current_process(7).unwrap();
    assert_eq!(termination.messages_discarded, 2);
    assert_eq!(
        termination
            .thread_cancellations
            .iter()
            .map(|cancellation| (cancellation.thread_id, cancellation.messages_discarded))
            .collect::<Vec<_>>(),
        vec![(SECOND_THREAD_ID, 0), (INITIAL_THREAD_ID, 2)]
    );
    assert!(
        process_exit
            .queued_thread_messages(INITIAL_THREAD_ID)
            .unwrap()
            .is_empty()
    );
    assert!(process_exit.scheduler().events().is_empty());
}

#[test]
fn message_capacity_rejection_is_cross_subsystem_atomic() {
    let mut runtime = running_runtime();
    runtime
        .register_thread(SECOND_THREAD_ID, 0)
        .expect("register controlled capacity producer");
    runtime.select_thread(SECOND_THREAD_ID).unwrap();

    for message in 0..4096 {
        runtime
            .post_thread_message(INITIAL_THREAD_ID, 0, message, 0, 0)
            .expect("fill controlled message capacity");
    }
    let before_rejection = runtime.clone();
    assert_eq!(
        runtime
            .post_thread_message(INITIAL_THREAD_ID, 0, 4096, 0, 0)
            .expect_err("capacity overflow must fail before commit")
            .code,
        "message-queue-capacity-exceeded"
    );
    assert_eq!(runtime, before_rejection);
    assert_eq!(
        runtime
            .queued_thread_messages(INITIAL_THREAD_ID)
            .unwrap()
            .len(),
        4096
    );
    assert_eq!(
        runtime
            .scheduler_state_for_thread(INITIAL_THREAD_ID)
            .unwrap(),
        SchedulerThreadState::Runnable
    );
    assert!(runtime.scheduler().events().is_empty());
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
            performance_frequency: 10_000_000,
        },
        startup_info: RuntimeStartupInfo {
            profile_id: "runtime-message-queue-test".to_owned(),
            bytes: vec![0; X86_STARTUP_INFO_W_BYTE_LENGTH],
        },
        process_image_path_a: vine_core::RuntimeProcessImagePathA {
            source_authority: "runtime-message-queue-fixture".to_owned(),
            bytes_with_nul: b"C:\\Vine\\fixture.exe\0".to_vec(),
            encoding: vine_core::RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING.to_owned(),
        },
        initial_current_directory_a: vine_core::RuntimeCurrentDirectoryA {
            source_authority: "runtime-message-queue-process-bootstrap".to_owned(),
            bytes_with_nul: b"C:\\Vine\0".to_vec(),
            encoding: vine_core::RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING.to_owned(),
        },
        encoded_pointer_namespace: LOGICAL_ENCODED_POINTER_NAMESPACE.to_owned(),
        encoded_pointer_token_prefix: 0xec01_0000,
    })
    .expect("initialized controlled runtime");
    runtime.start_initial_thread().unwrap();
    runtime
}

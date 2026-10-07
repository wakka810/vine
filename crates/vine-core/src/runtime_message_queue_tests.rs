use crate::runtime::{RuntimeMessageReceiveDisposition, X86_STARTUP_INFO_W_BYTE_LENGTH};
use crate::{
    Runtime, RuntimeBootstrap, RuntimeClockState, RuntimeCursorPositionProfile,
    RuntimeInputProfile, RuntimeProcessId, RuntimeSchedulerEvent, RuntimeStartupInfo,
    RuntimeThreadId, SchedulerThreadState,
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
    assert!(runtime.receive_current_thread_message().is_err());
    assert_eq!(runtime.current_thread_id(), INITIAL_THREAD_ID);
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::WaitingMessage
    );
    assert!(
        runtime
            .queued_thread_messages(INITIAL_THREAD_ID)
            .unwrap()
            .is_empty()
    );
    assert!(runtime.scheduler().events().is_empty());

    runtime.select_thread(SECOND_THREAD_ID).unwrap();
    let first = runtime
        .post_thread_message(INITIAL_THREAD_ID, CONTROLLED_HWND, 0x0401, 0x11, 0x22)
        .expect("first post wakes empty queue owner");
    let second = runtime
        .post_thread_message(INITIAL_THREAD_ID, CONTROLLED_HWND, 0x0402, 0x33, 0x44)
        .expect("second post preserves runnable owner");
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
        Some(RuntimeSchedulerEvent::MessageReady(INITIAL_THREAD_ID))
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
    runtime.select_thread(INITIAL_THREAD_ID).unwrap();
    assert_eq!(
        runtime.pop_scheduler_event(),
        Some(RuntimeSchedulerEvent::MessageReady(INITIAL_THREAD_ID))
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
    let retired_lane = runtime
        .scheduler_state_for_thread(SECOND_THREAD_ID)
        .unwrap();
    let caller_lane = runtime.scheduler_state().unwrap();
    let last_error = runtime.current_last_error().unwrap();
    assert!(
        runtime
            .post_thread_message(SECOND_THREAD_ID, 0, 1, 0, 0)
            .is_err()
    );
    assert_eq!(
        runtime
            .scheduler_state_for_thread(SECOND_THREAD_ID)
            .unwrap(),
        retired_lane
    );
    assert_eq!(runtime.current_thread_id(), INITIAL_THREAD_ID);
    assert_eq!(runtime.scheduler_state().unwrap(), caller_lane);
    assert_eq!(runtime.current_last_error().unwrap(), last_error);
    assert!(
        runtime
            .queued_thread_messages(INITIAL_THREAD_ID)
            .unwrap()
            .is_empty()
    );
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
    let queue_before = runtime.queued_thread_messages(INITIAL_THREAD_ID).unwrap();
    let last_error = runtime.current_last_error().unwrap();
    assert!(
        runtime
            .post_thread_message(INITIAL_THREAD_ID, 0, 4096, 0, 0)
            .is_err()
    );
    assert_eq!(
        runtime.queued_thread_messages(INITIAL_THREAD_ID).unwrap(),
        queue_before
    );
    assert_eq!(runtime.current_thread_id(), SECOND_THREAD_ID);
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::Runnable
    );
    assert_eq!(runtime.current_last_error().unwrap(), last_error);
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
    runtime.select_thread(INITIAL_THREAD_ID).unwrap();
    assert_eq!(
        runtime.receive_current_thread_message().unwrap(),
        RuntimeMessageReceiveDisposition::Message {
            message: queue_before[0]
        }
    );
    runtime.select_thread(SECOND_THREAD_ID).unwrap();
    let retry = runtime
        .post_thread_message(INITIAL_THREAD_ID, 0, 4096, 0, 0)
        .unwrap();
    let queue = runtime.queued_thread_messages(INITIAL_THREAD_ID).unwrap();
    assert_eq!(&queue[..4095], &queue_before[1..]);
    assert_eq!(queue[4095], retry);
    assert!(runtime.scheduler().events().is_empty());
}

#[test]
fn queue_status_queries_current_and_new_by_mask_and_isolates_threads() {
    let mut runtime = running_runtime();
    runtime
        .register_thread(SECOND_THREAD_ID, 0x4321_1234)
        .unwrap();
    let first = runtime
        .post_thread_message(INITIAL_THREAD_ID, 0, 0x0400, 17, 23)
        .unwrap();
    runtime
        .post_thread_message(SECOND_THREAD_ID, 0, 0x8000, 31, 47)
        .unwrap();
    assert_eq!(runtime.get_current_thread_queue_status(0).unwrap(), 0);
    assert_eq!(
        runtime.get_current_thread_queue_status(0x8).unwrap(),
        0x0008_0008
    );
    assert_eq!(
        runtime.get_current_thread_queue_status(0x8).unwrap(),
        0x0008_0000
    );
    assert_eq!(
        runtime.get_current_thread_queue_status(0x100).unwrap(),
        0x0100_0100
    );
    let next = runtime
        .post_thread_message(INITIAL_THREAD_ID, 0, 0x0401, 59, 61)
        .unwrap();
    assert_eq!(
        runtime.receive_current_thread_message().unwrap(),
        RuntimeMessageReceiveDisposition::Message { message: first }
    );
    assert_eq!(
        runtime.get_current_thread_queue_status(0x108).unwrap(),
        0x0108_0000
    );
    assert_eq!(
        runtime.receive_current_thread_message().unwrap(),
        RuntimeMessageReceiveDisposition::Message { message: next }
    );
    assert_eq!(runtime.get_current_thread_queue_status(0x108).unwrap(), 0);
    runtime.select_thread(SECOND_THREAD_ID).unwrap();
    assert_eq!(
        runtime.get_current_thread_queue_status(0x108).unwrap(),
        0x0108_0108
    );
    assert_eq!(runtime.current_last_error().unwrap(), 0x4321_1234);
    assert_eq!(
        runtime
            .queued_thread_messages(SECOND_THREAD_ID)
            .unwrap()
            .len(),
        1
    );
}

fn running_runtime() -> Runtime {
    let mut runtime = Runtime::new(
        RuntimeBootstrap {
            process_id: RuntimeProcessId(PROCESS_ID),
            initial_thread_id: INITIAL_THREAD_ID,
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
    runtime.start_initial_thread().unwrap();
    runtime
        .configure_input_profile(RuntimeInputProfile {
            initial_mouse_buttons_swapped: false,
            initial_keyboard_source_focused: None,
            cursor_position: Some(RuntimeCursorPositionProfile {
                initial_screen_x: 960,
                initial_screen_y: 540,
            }),
        })
        .expect("configure controlled MSG cursor metadata");
    runtime
}

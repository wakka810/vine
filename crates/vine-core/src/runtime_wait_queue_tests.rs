use crate::runtime::{
    RuntimeHandle, RuntimeObjectKind, RuntimeWaitDisposition, RuntimeWaitRegistrationId,
    RuntimeWaitState, X86_STARTUP_INFO_W_BYTE_LENGTH,
};
use crate::{
    Runtime, RuntimeBootstrap, RuntimeClockState, RuntimeProcessId, RuntimeSchedulerEvent,
    RuntimeStartupInfo, RuntimeThreadId, SchedulerThreadState,
};

const PROCESS_ID: u32 = 4096;
const INITIAL_THREAD_ID: RuntimeThreadId = RuntimeThreadId(8192);
const SECOND_THREAD_ID: RuntimeThreadId = RuntimeThreadId(4097);
const THIRD_THREAD_ID: RuntimeThreadId = RuntimeThreadId(4098);

#[test]
fn auto_reset_event_signal_wakes_one_fifo_waiter_and_is_consumed() {
    let mut runtime = running_runtime();
    let event_handle = runtime
        .create_event(false, false)
        .expect("create unnamed auto-reset event");
    let event = runtime.event_for_handle(event_handle).unwrap();
    assert!(!event.manual_reset);
    assert_eq!(event.wait_state, RuntimeWaitState::Nonsignaled);
    runtime
        .register_thread(SECOND_THREAD_ID, 0)
        .expect("register second waiter");
    runtime
        .register_thread(THIRD_THREAD_ID, 0)
        .expect("register signaler");

    assert!(matches!(
        runtime.wait_current_thread_on(event_handle).unwrap(),
        RuntimeWaitDisposition::Pending { .. }
    ));
    runtime.select_thread(SECOND_THREAD_ID).unwrap();
    assert!(matches!(
        runtime.wait_current_thread_on(event_handle).unwrap(),
        RuntimeWaitDisposition::Pending { .. }
    ));
    runtime.select_thread(THIRD_THREAD_ID).unwrap();
    runtime
        .set_event(event_handle)
        .expect("signal first waiter");

    assert_eq!(
        runtime.event_for_handle(event_handle).unwrap().wait_state,
        RuntimeWaitState::Nonsignaled
    );
    assert_eq!(
        runtime
            .scheduler_state_for_thread(INITIAL_THREAD_ID)
            .unwrap(),
        SchedulerThreadState::Runnable
    );
    assert!(matches!(
        runtime
            .scheduler_state_for_thread(SECOND_THREAD_ID)
            .unwrap(),
        SchedulerThreadState::WaitingObject { .. }
    ));

    runtime
        .set_event(event_handle)
        .expect("signal second waiter");
    assert_eq!(
        runtime
            .scheduler_state_for_thread(SECOND_THREAD_ID)
            .unwrap(),
        SchedulerThreadState::Runnable
    );
    assert_eq!(
        runtime.event_for_handle(event_handle).unwrap().wait_state,
        RuntimeWaitState::Nonsignaled
    );
}

#[test]
fn object_wait_wake_restores_the_callback_lane() {
    let mut runtime = running_runtime();
    let event_handle = runtime
        .create_event(false, false)
        .expect("create callback wait event");
    runtime.register_thread(SECOND_THREAD_ID, 0).unwrap();
    let callback = runtime
        .push_callback_frame(
            crate::GuestAddress(0x1000_5000),
            crate::GuestAddress(0x7f01_0000),
        )
        .expect("enter callback before waiting");
    assert!(matches!(
        runtime.wait_current_thread_on(event_handle).unwrap(),
        RuntimeWaitDisposition::Pending { .. }
    ));

    runtime.select_thread(SECOND_THREAD_ID).unwrap();
    runtime.set_event(event_handle).unwrap();
    assert_eq!(
        runtime
            .scheduler_state_for_thread(INITIAL_THREAD_ID)
            .unwrap(),
        SchedulerThreadState::InCallback { depth: 1 }
    );

    runtime.select_thread(INITIAL_THREAD_ID).unwrap();
    assert_eq!(runtime.pop_callback_frame(callback.id).unwrap(), callback);
}

#[test]
fn wait_queue_wakes_on_thread_exit_signal() {
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
            .allocate_private_heap(0x1000, 0)
            .expect("nonwaitable control object"),
    );
    let heap_before = runtime
        .object_for_handle(heap_handle, RuntimeObjectKind::PrivateHeap)
        .unwrap()
        .clone();
    let last_error = runtime.current_last_error().unwrap();
    assert!(runtime.wait_current_thread_on(heap_handle).is_err());
    assert_eq!(
        runtime
            .object_for_handle(heap_handle, RuntimeObjectKind::PrivateHeap)
            .unwrap(),
        &heap_before
    );
    assert_eq!(runtime.current_thread_id(), INITIAL_THREAD_ID);
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::Runnable
    );
    assert_eq!(runtime.current_last_error().unwrap(), last_error);
    assert_eq!(runtime.next_wait_registration_id(), 1);
    assert!(runtime.scheduler().wait_registrations().is_empty());
    assert!(runtime.scheduler().object_waiters().is_empty());

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
    assert_eq!(runtime.next_wait_registration_id(), 2);
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

    assert!(runtime.wait_current_thread_on(second_handle).is_err());
    assert_eq!(runtime.current_thread_id(), INITIAL_THREAD_ID);
    assert_eq!(runtime.current_last_error().unwrap(), last_error);
    assert_eq!(runtime.next_wait_registration_id(), 2);
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::WaitingObject {
            registration_id: registration.id,
            object_id: second_object_id,
        }
    );
    assert_eq!(runtime.scheduler().wait_registrations().len(), 1);
    assert_eq!(
        runtime
            .scheduler()
            .wait_registrations()
            .get(&registration.id),
        Some(&registration)
    );
    assert_eq!(
        runtime
            .scheduler()
            .object_waiters()
            .get(&second_object_id)
            .unwrap()
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        vec![registration.id]
    );

    runtime
        .select_thread(SECOND_THREAD_ID)
        .expect("select controlled signaler");
    runtime
        .request_current_thread_exit(23)
        .expect("request signaler exit");
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
    runtime
        .start_initial_thread()
        .expect("start controlled initial thread");
    runtime
}

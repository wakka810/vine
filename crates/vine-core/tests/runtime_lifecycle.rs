use vine_core::{
    GuestAddress, LOGICAL_ENCODED_POINTER_NAMESPACE, ProcessAddressSpaceLayout, RuntimeBootstrap,
    RuntimeClockState, RuntimeObjectKind, RuntimeProcessId, RuntimeProcessLifecycleState,
    RuntimeStartupInfo, RuntimeState, RuntimeThreadId, RuntimeThreadLifecycleState,
    RuntimeWaitState, SchedulerThreadState, X86_STARTUP_INFO_W_BYTE_LENGTH,
};

const PROCESS_ID: u32 = 4096;
const THREAD_ID: u32 = 8192;

#[test]
fn process_and_thread_lifecycle_separates_exit_request_from_waitable_completion() {
    let mut runtime = initialized_runtime();
    let process_handle = runtime.process_handle().expect("bootstrap process handle");
    let thread_handle = runtime
        .current_thread_handle()
        .expect("bootstrap thread handle");
    assert_eq!(
        runtime.process().lifecycle,
        RuntimeProcessLifecycleState::Bootstrapping
    );
    assert_eq!(
        runtime.current_thread_state().unwrap().lifecycle,
        RuntimeThreadLifecycleState::Initialized
    );
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::Initialized
    );

    let before_invalid_completion = runtime.clone();
    assert_eq!(
        runtime
            .complete_current_process_exit()
            .expect_err("process cannot exit before start and request")
            .code,
        "process-exit-completion-transition-invalid"
    );
    assert_eq!(runtime, before_invalid_completion);

    runtime
        .start_initial_thread()
        .expect("start initial runtime thread");
    assert_eq!(
        runtime.process().lifecycle,
        RuntimeProcessLifecycleState::Running
    );
    assert_eq!(
        runtime.current_thread_state().unwrap().lifecycle,
        RuntimeThreadLifecycleState::Running
    );
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::Runnable
    );
    let running = runtime.clone();
    assert_eq!(
        runtime
            .start_initial_thread()
            .expect_err("initial thread cannot start twice")
            .code,
        "initial-thread-start-transition-invalid"
    );
    assert_eq!(runtime, running);

    runtime
        .push_callback_frame(GuestAddress(0x1000_1000), GuestAddress(0x7f01_0000))
        .expect("active callback before process exit");
    runtime
        .begin_host_request("browser.storage.flush")
        .expect("pending host request before process exit");
    let cancellation = runtime
        .request_current_process_exit(7)
        .expect("request process exit");
    assert_eq!(cancellation.host_requests_cancelled, 1);
    assert_eq!(cancellation.callback_frames_cancelled, 1);
    assert_eq!(cancellation.scheduler_events_discarded, 0);
    assert_eq!(
        runtime.process().lifecycle,
        RuntimeProcessLifecycleState::ExitRequested {
            exit_code: 7,
            initiating_thread_id: RuntimeThreadId(THREAD_ID),
        }
    );
    assert_eq!(
        runtime.current_thread_state().unwrap().lifecycle,
        RuntimeThreadLifecycleState::ExitRequested { exit_code: 7 }
    );
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::Terminating { exit_code: 7 }
    );
    assert_eq!(
        runtime
            .object_wait_state(process_handle, RuntimeObjectKind::Process)
            .unwrap(),
        RuntimeWaitState::Nonsignaled
    );
    assert_eq!(
        runtime
            .object_wait_state(thread_handle, RuntimeObjectKind::Thread)
            .unwrap(),
        RuntimeWaitState::Nonsignaled
    );

    let exit_requested = runtime.clone();
    assert_eq!(
        runtime
            .request_current_process_exit(7)
            .expect_err("duplicate process exit request is rejected")
            .code,
        "process-exit-request-transition-invalid"
    );
    assert_eq!(runtime, exit_requested);

    runtime
        .complete_current_process_exit()
        .expect("complete process exit");
    assert_eq!(
        runtime.process().lifecycle,
        RuntimeProcessLifecycleState::Exited { exit_code: 7 }
    );
    assert_eq!(
        runtime.current_thread_state().unwrap().lifecycle,
        RuntimeThreadLifecycleState::Exited { exit_code: 7 }
    );
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::Terminated { exit_code: 7 }
    );
    assert_eq!(
        runtime
            .object_wait_state(thread_handle, RuntimeObjectKind::Thread)
            .unwrap(),
        RuntimeWaitState::Signaled { exit_code: 7 }
    );
    assert_eq!(
        runtime
            .object_wait_state(process_handle, RuntimeObjectKind::Process)
            .unwrap(),
        RuntimeWaitState::Signaled { exit_code: 7 }
    );
    let exited = runtime.clone();
    assert_eq!(
        runtime
            .complete_current_process_exit()
            .expect_err("process exit cannot complete twice")
            .code,
        "process-exit-completion-transition-invalid"
    );
    assert_eq!(runtime, exited);

    let mut thread_only = initialized_runtime();
    thread_only.start_initial_thread().unwrap();
    let process_handle = thread_only.process_handle().unwrap();
    let thread_handle = thread_only.current_thread_handle().unwrap();
    thread_only
        .request_current_thread_exit(0xc000_013a)
        .expect("request thread-only exit");
    assert_eq!(
        thread_only.process().lifecycle,
        RuntimeProcessLifecycleState::Running
    );
    assert_eq!(
        thread_only.current_thread_state().unwrap().lifecycle,
        RuntimeThreadLifecycleState::ExitRequested {
            exit_code: 0xc000_013a,
        }
    );
    assert_eq!(
        thread_only
            .object_wait_state(thread_handle, RuntimeObjectKind::Thread)
            .unwrap(),
        RuntimeWaitState::Nonsignaled
    );
    thread_only
        .complete_current_thread_exit()
        .expect("complete thread-only exit");
    assert_eq!(
        thread_only.current_thread_state().unwrap().lifecycle,
        RuntimeThreadLifecycleState::Exited {
            exit_code: 0xc000_013a,
        }
    );
    assert_eq!(
        thread_only
            .object_wait_state(thread_handle, RuntimeObjectKind::Thread)
            .unwrap(),
        RuntimeWaitState::Signaled {
            exit_code: 0xc000_013a,
        }
    );
    assert_eq!(
        thread_only
            .object_wait_state(process_handle, RuntimeObjectKind::Process)
            .unwrap(),
        RuntimeWaitState::Nonsignaled
    );
}

fn initialized_runtime() -> RuntimeState {
    RuntimeState::new(RuntimeBootstrap {
        process_id: RuntimeProcessId(PROCESS_ID),
        initial_thread_id: RuntimeThreadId(THREAD_ID),
        initial_last_error: 87,
        address_space: ProcessAddressSpaceLayout::vine_v1(),
        clock: RuntimeClockState {
            wall_clock_file_time: 134_282_016_000_000_000,
            tick_count: 3_600_000,
            performance_counter: 12_345_678_901,
        },
        startup_info: RuntimeStartupInfo {
            profile_id: "runtime-lifecycle-test".to_owned(),
            bytes: vec![0; X86_STARTUP_INFO_W_BYTE_LENGTH],
        },
        encoded_pointer_namespace: LOGICAL_ENCODED_POINTER_NAMESPACE.to_owned(),
        encoded_pointer_token_prefix: 0xec01_0000,
    })
    .expect("initialized controlled runtime")
}

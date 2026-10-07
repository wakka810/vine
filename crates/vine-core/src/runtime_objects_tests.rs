use crate::runtime::{RuntimeObjectKind, RuntimeWaitState, X86_STARTUP_INFO_W_BYTE_LENGTH};
use crate::{
    Runtime, RuntimeBootstrap, RuntimeClockState, RuntimeProcessId, RuntimeStartupInfo,
    RuntimeThreadId,
};

const PROCESS_ID: u32 = 4096;
const THREAD_ID: u32 = 8192;

#[test]
fn closed_handles_stay_invalid_while_other_handles_keep_the_object_alive() {
    let mut runtime = controlled_runtime();
    let process = runtime.process_handle().unwrap();
    let first = runtime.current_thread_handle().unwrap();
    let second = runtime.open_current_thread_handle().unwrap();
    assert_ne!(first, second);
    runtime.close_handle(first).unwrap();
    assert!(runtime.close_handle(first).is_err());
    assert!(
        runtime
            .object_for_handle(first, RuntimeObjectKind::Thread)
            .is_err()
    );
    assert!(
        runtime
            .object_for_handle(process, RuntimeObjectKind::Thread)
            .is_err()
    );
    assert_eq!(
        runtime
            .object_wait_state(second, RuntimeObjectKind::Thread)
            .unwrap(),
        RuntimeWaitState::Nonsignaled
    );

    let later = runtime.open_current_thread_handle().unwrap();
    assert_ne!(later, first);
    assert!(
        runtime
            .object_for_handle(first, RuntimeObjectKind::Thread)
            .is_err()
    );
    runtime.request_current_thread_exit(23).unwrap();
    runtime.complete_current_thread_exit().unwrap();
    for handle in [second, later] {
        assert_eq!(
            runtime
                .object_wait_state(handle, RuntimeObjectKind::Thread)
                .unwrap(),
            RuntimeWaitState::Signaled { exit_code: 23 }
        );
        runtime.close_handle(handle).unwrap();
    }
    let reopened = runtime.open_current_thread_handle().unwrap();
    assert_eq!(
        runtime
            .object_wait_state(reopened, RuntimeObjectKind::Thread)
            .unwrap(),
        RuntimeWaitState::Signaled { exit_code: 23 }
    );
    assert_eq!(
        runtime
            .object_wait_state(process, RuntimeObjectKind::Process)
            .unwrap(),
        RuntimeWaitState::Nonsignaled
    );
}

fn controlled_runtime() -> Runtime {
    let mut runtime = Runtime::new(
        RuntimeBootstrap {
            process_id: RuntimeProcessId(PROCESS_ID),
            initial_thread_id: RuntimeThreadId(THREAD_ID),
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
    .expect("controlled runtime");
    runtime
        .start_initial_thread()
        .expect("start controlled initial thread");
    runtime
}

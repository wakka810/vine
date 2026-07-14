use vine_core::{
    LOGICAL_ENCODED_POINTER_NAMESPACE, ProcessAddressSpaceLayout, RuntimeBootstrap,
    RuntimeClockState, RuntimeCurrentDirectoryA, RuntimeMutexId, RuntimeMutexNameA,
    RuntimeObjectKind, RuntimeProcessId, RuntimeProcessImagePathA, RuntimeStartupInfo,
    RuntimeState, RuntimeThreadId, RuntimeWaitDisposition, X86_STARTUP_INFO_W_BYTE_LENGTH,
};

const PROCESS_ID: u32 = 4096;
const THREAD_ID: RuntimeThreadId = RuntimeThreadId(8192);
const CONTENDER_THREAD_ID: RuntimeThreadId = RuntimeThreadId(4097);

#[test]
fn named_mutex_identity_owner_handles_and_process_teardown_are_atomic() {
    let mut runtime = running_runtime();
    let mut caller_bytes = b"Vine named mutex\0".to_vec();
    let first = runtime
        .create_mutex_a(Some(mutex_name(&caller_bytes)), true)
        .expect("create initially owned named mutex");
    assert_eq!(first.handle.0, 0x5801_0001);
    assert_eq!(first.mutex_id, RuntimeMutexId(1));
    assert!(!first.already_exists);
    caller_bytes[0] = b'X';
    let mutex = runtime
        .mutex_for_handle(first.handle)
        .expect("first mutex handle");
    assert_eq!(
        mutex.name_a.as_ref().unwrap().bytes_with_nul,
        b"Vine named mutex\0"
    );
    assert_eq!(mutex.owning_thread_id, Some(THREAD_ID));
    assert_eq!(mutex.recursion_count, 1);

    let duplicate = runtime
        .create_mutex_a(Some(mutex_name(b"Vine named mutex\0")), true)
        .expect("open existing named mutex");
    assert_eq!(duplicate.handle.0, 0x5801_0002);
    assert_eq!(duplicate.mutex_id, first.mutex_id);
    assert!(duplicate.already_exists);
    assert_eq!(
        runtime
            .mutex_for_handle(duplicate.handle)
            .unwrap()
            .recursion_count,
        1,
        "initial-owner is ignored when the named object already exists"
    );

    runtime
        .register_thread(CONTENDER_THREAD_ID, 77)
        .expect("register controlled contender");
    runtime
        .select_thread(CONTENDER_THREAD_ID)
        .expect("select controlled contender");
    let before_contention = runtime.clone();
    assert_eq!(
        runtime
            .wait_current_thread_on(duplicate.handle)
            .expect_err("cross-thread mutex contention must fail closed")
            .code,
        "mutex-contended-unsupported"
    );
    assert_eq!(runtime, before_contention);
    runtime
        .select_thread(THREAD_ID)
        .expect("restore initial thread");

    assert_eq!(
        runtime
            .wait_current_thread_on(duplicate.handle)
            .expect("recursive mutex wait"),
        RuntimeWaitDisposition::MutexAcquired {
            object_id: runtime
                .object_for_handle(duplicate.handle, RuntimeObjectKind::Mutex)
                .unwrap()
                .id,
            mutex_id: first.mutex_id,
            recursion_count: 2,
            abandoned: false,
        }
    );
    runtime
        .release_mutex(duplicate.handle)
        .expect("release recursive acquisition");
    runtime
        .release_mutex(first.handle)
        .expect("release initial ownership");
    let before_unowned_release = runtime.clone();
    assert_eq!(
        runtime
            .release_mutex(first.handle)
            .expect_err("unowned mutex release must fail")
            .code,
        "mutex-not-owned-by-current-thread"
    );
    assert_eq!(runtime, before_unowned_release);

    runtime
        .wait_current_thread_on(duplicate.handle)
        .expect("acquire fully released mutex");
    runtime
        .release_mutex(duplicate.handle)
        .expect("release acquired mutex");
    runtime
        .close_handle(first.handle)
        .expect("close first handle");
    assert_eq!(
        runtime
            .mutex_for_handle(duplicate.handle)
            .expect("remaining handle keeps mutex alive")
            .id,
        first.mutex_id
    );
    runtime
        .close_handle(duplicate.handle)
        .expect("close last handle");
    assert!(
        runtime
            .objects()
            .objects()
            .values()
            .all(|record| record.kind != RuntimeObjectKind::Mutex)
    );

    let recreated = runtime
        .create_mutex_a(Some(mutex_name(b"Vine named mutex\0")), false)
        .expect("recreate retired named mutex");
    assert_eq!(recreated.mutex_id, RuntimeMutexId(2));
    assert!(!recreated.already_exists);
    assert_eq!(recreated.handle.0, 0x5802_0001);
    assert_eq!(
        runtime
            .mutex_for_handle(recreated.handle)
            .unwrap()
            .owning_thread_id,
        None
    );

    runtime
        .terminate_current_process(7)
        .expect("process teardown closes process-owned mutex handles");
    assert!(
        runtime
            .objects()
            .objects()
            .values()
            .all(|record| record.kind != RuntimeObjectKind::Mutex)
    );
    assert_eq!(
        runtime
            .mutex_for_handle(recreated.handle)
            .expect_err("teardown retires the live mutex handle")
            .code,
        "runtime-handle-stale"
    );
}

fn mutex_name(bytes_with_nul: &[u8]) -> RuntimeMutexNameA {
    RuntimeMutexNameA::new(
        bytes_with_nul.to_vec(),
        bytes_with_nul.iter().map(|byte| u16::from(*byte)).collect(),
    )
    .expect("valid ASCII mutex name")
}

fn running_runtime() -> RuntimeState {
    let mut runtime = RuntimeState::new(RuntimeBootstrap {
        process_id: RuntimeProcessId(PROCESS_ID),
        initial_thread_id: THREAD_ID,
        initial_last_error: 0,
        address_space: ProcessAddressSpaceLayout::vine_v1(),
        clock: RuntimeClockState {
            wall_clock_file_time: 134_282_016_000_000_000,
            tick_count: 3_600_000,
            performance_counter: 12_345_678_901,
        },
        startup_info: RuntimeStartupInfo {
            profile_id: "runtime-mutex-test".to_owned(),
            bytes: vec![0; X86_STARTUP_INFO_W_BYTE_LENGTH],
        },
        process_image_path_a: RuntimeProcessImagePathA {
            source_authority: "runtime-mutex-fixture".to_owned(),
            bytes_with_nul: b"C:\\Vine\\fixture.exe\0".to_vec(),
            encoding: vine_core::RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING.to_owned(),
        },
        initial_current_directory_a: RuntimeCurrentDirectoryA {
            source_authority: "runtime-mutex-process-bootstrap".to_owned(),
            bytes_with_nul: b"C:\\Vine\0".to_vec(),
            encoding: vine_core::RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING.to_owned(),
        },
        encoded_pointer_namespace: LOGICAL_ENCODED_POINTER_NAMESPACE.to_owned(),
        encoded_pointer_token_prefix: 0xec01_0000,
    })
    .expect("controlled runtime");
    runtime
        .start_initial_thread()
        .expect("start controlled initial thread");
    runtime
}

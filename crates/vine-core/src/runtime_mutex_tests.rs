use crate::runtime::{
    RuntimeMutexId, RuntimeMutexName, RuntimeObjectKind, RuntimeWaitDisposition,
    X86_STARTUP_INFO_W_BYTE_LENGTH,
};
use crate::{
    Runtime, RuntimeBootstrap, RuntimeClockState, RuntimeCurrentDirectoryA, RuntimeProcessId,
    RuntimeProcessImagePathA, RuntimeStartupInfo, RuntimeThreadId,
};

const PROCESS_ID: u32 = 4096;
const THREAD_ID: RuntimeThreadId = RuntimeThreadId(8192);
const CONTENDER_THREAD_ID: RuntimeThreadId = RuntimeThreadId(4097);

#[test]
fn named_mutex_identity_owner_and_handle_lifetime_are_atomic() {
    let mut runtime = running_runtime();
    let mut caller_bytes = b"Vine named mutex\0".to_vec();
    let (first, already_exists) = runtime
        .create_mutex(Some(mutex_name(&caller_bytes)), true)
        .expect("create initially owned named mutex");
    assert_eq!(first.0, 0x5801_0001);
    assert!(!already_exists);
    caller_bytes[0] = b'X';
    let mutex = runtime.mutex_for_handle(first).expect("first mutex handle");
    let first_id = mutex.id;
    assert_eq!(first_id, RuntimeMutexId(1));
    assert_eq!(
        mutex.name.as_ref().unwrap().utf16_units_with_nul,
        "Vine named mutex\0".encode_utf16().collect::<Vec<_>>()
    );
    assert_eq!(mutex.owning_thread_id, Some(THREAD_ID));
    assert_eq!(mutex.recursion_count, 1);

    let (duplicate, already_exists) = runtime
        .create_mutex(Some(mutex_name(b"Vine named mutex\0")), true)
        .expect("open existing named mutex");
    assert_eq!(duplicate.0, 0x5801_0002);
    assert_eq!(runtime.mutex_for_handle(duplicate).unwrap().id, first_id);
    assert!(already_exists);
    assert_eq!(
        runtime.mutex_for_handle(duplicate).unwrap().recursion_count,
        1,
        "initial-owner is ignored when the named object already exists"
    );

    runtime
        .register_thread(CONTENDER_THREAD_ID, 77)
        .expect("register controlled contender");
    runtime
        .select_thread(CONTENDER_THREAD_ID)
        .expect("select controlled contender");
    for rejected_operation in 0..2 {
        if rejected_operation == 0 {
            runtime
                .wait_current_thread_on(duplicate)
                .expect_err("cross-thread mutex contention is not implemented");
        } else {
            runtime
                .release_mutex(duplicate)
                .expect_err("a non-owner cannot release the mutex");
        }
        for handle in [first, duplicate] {
            let mutex = runtime.mutex_for_handle(handle).unwrap();
            assert_eq!(mutex.id, first_id);
            assert_eq!(mutex.owning_thread_id, Some(THREAD_ID));
            assert_eq!(mutex.recursion_count, 1);
            assert!(!mutex.abandoned);
            assert_eq!(
                runtime
                    .object_for_handle(handle, RuntimeObjectKind::Mutex)
                    .unwrap()
                    .open_handles,
                2
            );
        }
        assert_eq!(runtime.current_thread_id(), CONTENDER_THREAD_ID);
        assert_eq!(runtime.current_last_error().unwrap(), 77);
        assert_eq!(
            runtime.scheduler_state().unwrap(),
            crate::SchedulerThreadState::Runnable
        );
        assert!(runtime.scheduler().wait_registrations().is_empty());
        assert!(runtime.scheduler().object_waiters().is_empty());
    }
    runtime
        .select_thread(THREAD_ID)
        .expect("restore initial thread");

    assert_eq!(
        runtime
            .wait_current_thread_on(duplicate)
            .expect("recursive mutex wait"),
        RuntimeWaitDisposition::MutexAcquired {
            object_id: runtime
                .object_for_handle(duplicate, RuntimeObjectKind::Mutex)
                .unwrap()
                .id,
            mutex_id: first_id,
            recursion_count: 2,
            abandoned: false,
        }
    );
    runtime
        .release_mutex(duplicate)
        .expect("release recursive acquisition");
    runtime
        .release_mutex(first)
        .expect("release initial ownership");
    runtime
        .release_mutex(first)
        .expect_err("unowned mutex release must fail");
    let released = runtime.mutex_for_handle(first).unwrap();
    assert_eq!(released.id, first_id);
    assert_eq!(released.owning_thread_id, None);
    assert_eq!(released.recursion_count, 0);
    assert!(!released.abandoned);
    assert_eq!(runtime.current_thread_id(), THREAD_ID);
    assert_eq!(runtime.current_last_error().unwrap(), 0);

    runtime.select_thread(CONTENDER_THREAD_ID).unwrap();
    runtime
        .wait_current_thread_on(duplicate)
        .expect("the contender can acquire a fully released mutex");
    let acquired = runtime.mutex_for_handle(first).unwrap();
    assert_eq!(acquired.id, first_id);
    assert_eq!(acquired.owning_thread_id, Some(CONTENDER_THREAD_ID));
    assert_eq!(acquired.recursion_count, 1);
    assert!(!acquired.abandoned);
    runtime
        .release_mutex(duplicate)
        .expect("release acquired mutex");
    runtime.select_thread(THREAD_ID).unwrap();
    runtime.close_handle(first).expect("close first handle");
    assert_eq!(
        runtime
            .mutex_for_handle(duplicate)
            .expect("remaining handle keeps mutex alive")
            .id,
        first_id
    );
    runtime.close_handle(duplicate).expect("close last handle");
    assert!(runtime.mutex_for_handle(duplicate).is_err());

    let (recreated, already_exists) = runtime
        .create_mutex(Some(mutex_name(b"Vine named mutex\0")), false)
        .expect("recreate retired named mutex");
    assert_eq!(
        runtime.mutex_for_handle(recreated).unwrap().id,
        RuntimeMutexId(2)
    );
    assert!(!already_exists);
    assert_eq!(recreated.0, 0x5802_0001);
    assert_eq!(
        runtime
            .mutex_for_handle(recreated)
            .unwrap()
            .owning_thread_id,
        None
    );

    let (owned, _) = runtime
        .create_mutex(None, true)
        .expect("create owned unnamed mutex");
    runtime.register_thread(CONTENDER_THREAD_ID, 55).ok();
    runtime.select_thread(CONTENDER_THREAD_ID).unwrap();
    assert_eq!(runtime.try_release_mutex(owned).unwrap(), Err(288));
    runtime.select_thread(THREAD_ID).unwrap();
    assert_eq!(
        runtime
            .try_release_mutex(crate::runtime::RuntimeHandle(0xdead_beef))
            .unwrap(),
        Err(6)
    );
    assert_eq!(runtime.try_release_mutex(owned).unwrap(), Ok(()));
    assert_eq!(runtime.try_release_mutex(owned).unwrap(), Err(288));
    runtime.close_handle(owned).unwrap();
}

fn mutex_name(bytes_with_nul: &[u8]) -> RuntimeMutexName {
    RuntimeMutexName::new(bytes_with_nul.iter().map(|byte| u16::from(*byte)).collect())
        .expect("valid ASCII mutex name")
}

fn running_runtime() -> Runtime {
    let mut runtime = Runtime::new(
        RuntimeBootstrap {
            process_id: RuntimeProcessId(PROCESS_ID),
            initial_thread_id: THREAD_ID,
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
            process_image_path_a: RuntimeProcessImagePathA {
                bytes_with_nul: b"C:\\Vine\\fixture.exe\0".to_vec(),
            },
            initial_current_directory_a: RuntimeCurrentDirectoryA {
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

use crate::{
    GuestAddress, Runtime, RuntimeBootstrap, RuntimeClockState, RuntimeProcessId,
    RuntimeStartupInfo, RuntimeThreadId, SchedulerThreadState, X86_STARTUP_INFO_W_BYTE_LENGTH,
};

const PROCESS_ID: RuntimeProcessId = RuntimeProcessId(4096);
const INITIAL_THREAD_ID: RuntimeThreadId = RuntimeThreadId(8192);
const SECOND_THREAD_ID: RuntimeThreadId = RuntimeThreadId(4097);
const CRITICAL_SECTION: GuestAddress = GuestAddress(0x1000_4000);

#[test]
fn synchronization_failures_preserve_state_and_wait_transfers_ownership() {
    let mut runtime = initialized_runtime();

    let initial_lane = runtime.scheduler_state().unwrap();
    let initial_last_error = runtime.current_last_error().unwrap();
    assert!(
        runtime
            .initialize_critical_section(CRITICAL_SECTION, 4000)
            .is_err()
    );
    assert!(runtime.critical_section(CRITICAL_SECTION).is_err());
    assert_eq!(runtime.current_thread_id(), INITIAL_THREAD_ID);
    assert_eq!(runtime.scheduler_state().unwrap(), initial_lane);
    assert_eq!(runtime.current_last_error().unwrap(), initial_last_error);
    assert!(
        runtime
            .critical_section_waiters(CRITICAL_SECTION)
            .is_empty()
    );

    runtime.start_initial_thread().unwrap();
    assert!(
        runtime
            .initialize_critical_section(CRITICAL_SECTION, 0x0100_0000)
            .is_err()
    );
    assert!(runtime.critical_section(CRITICAL_SECTION).is_err());
    assert_eq!(runtime.current_thread_id(), INITIAL_THREAD_ID);
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::Runnable
    );
    assert_eq!(runtime.current_last_error().unwrap(), initial_last_error);
    assert!(
        runtime
            .critical_section_waiters(CRITICAL_SECTION)
            .is_empty()
    );

    runtime
        .initialize_critical_section(CRITICAL_SECTION, 4000)
        .expect("initialize process-owned synchronization state");
    runtime
        .enter_critical_section(CRITICAL_SECTION)
        .expect("initial owner enters critical section");
    runtime
        .register_thread(SECOND_THREAD_ID, 0)
        .expect("register contending thread");
    runtime
        .select_thread(SECOND_THREAD_ID)
        .expect("select contending thread");
    let section_before = *runtime.critical_section(CRITICAL_SECTION).unwrap();
    let contender_last_error = runtime.current_last_error().unwrap();
    assert!(runtime.enter_critical_section(CRITICAL_SECTION).is_err());
    assert_eq!(
        runtime.critical_section(CRITICAL_SECTION).unwrap(),
        &section_before
    );
    assert_eq!(runtime.current_thread_id(), SECOND_THREAD_ID);
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::Runnable
    );
    assert_eq!(runtime.current_last_error().unwrap(), contender_last_error);
    assert!(
        runtime
            .critical_section_waiters(CRITICAL_SECTION)
            .is_empty()
    );

    runtime
        .wait_current_thread_for_critical_section(CRITICAL_SECTION)
        .expect("contending thread blocks on the owned critical section");
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::WaitingCriticalSection {
            address: CRITICAL_SECTION.0,
        }
    );
    assert_eq!(
        runtime.critical_section_waiters(CRITICAL_SECTION),
        vec![SECOND_THREAD_ID]
    );

    runtime.select_thread(INITIAL_THREAD_ID).unwrap();
    let transferred = runtime
        .leave_critical_section(CRITICAL_SECTION)
        .expect("owner transfers critical section to its oldest waiter");
    assert_eq!(transferred.owning_thread_id, Some(SECOND_THREAD_ID));
    assert_eq!(transferred.recursion_count, 1);
    assert_eq!(
        runtime
            .scheduler_state_for_thread(SECOND_THREAD_ID)
            .unwrap(),
        SchedulerThreadState::Runnable
    );
    assert!(
        runtime
            .critical_section_waiters(CRITICAL_SECTION)
            .is_empty()
    );

    runtime.select_thread(SECOND_THREAD_ID).unwrap();
    let released = runtime
        .leave_critical_section(CRITICAL_SECTION)
        .expect("awakened owner releases critical section");
    assert_eq!(released.owning_thread_id, None);
    assert_eq!(released.recursion_count, 0);

    let retired = runtime
        .delete_critical_section(CRITICAL_SECTION)
        .expect("delete released critical section");
    assert_eq!(retired.address, CRITICAL_SECTION);
    assert!(runtime.critical_section(CRITICAL_SECTION).is_err());
    runtime
        .initialize_critical_section(CRITICAL_SECTION, 0)
        .expect("deleted storage can be initialized again");
}

#[test]
fn last_executable_critical_section_waiter_can_block_for_a_host_waiting_owner() {
    let mut runtime = initialized_runtime();
    runtime.start_initial_thread().unwrap();
    runtime
        .initialize_critical_section(CRITICAL_SECTION, 0)
        .unwrap();
    runtime.enter_critical_section(CRITICAL_SECTION).unwrap();
    runtime
        .begin_host_request("runtime.owner-delay")
        .expect("owner enters an authoritative host wait");
    runtime.register_thread(SECOND_THREAD_ID, 0).unwrap();
    runtime.select_thread(SECOND_THREAD_ID).unwrap();

    runtime
        .wait_current_thread_for_critical_section(CRITICAL_SECTION)
        .expect("the final executable lane may wait for a host-waiting owner");
    assert_eq!(
        runtime
            .scheduler_state_for_thread(SECOND_THREAD_ID)
            .unwrap(),
        SchedulerThreadState::WaitingCriticalSection {
            address: CRITICAL_SECTION.0,
        }
    );
    assert!(runtime.select_next_executable_thread().unwrap().is_none());

    let completion = runtime.complete_host_request(1).unwrap();
    assert_eq!(completion.thread_id, INITIAL_THREAD_ID);
    runtime.select_thread(INITIAL_THREAD_ID).unwrap();
    runtime.leave_critical_section(CRITICAL_SECTION).unwrap();
    assert_eq!(
        runtime
            .scheduler_state_for_thread(SECOND_THREAD_ID)
            .unwrap(),
        SchedulerThreadState::Runnable
    );

    let mut deadlocked = initialized_runtime();
    deadlocked.start_initial_thread().unwrap();
    deadlocked
        .initialize_critical_section(CRITICAL_SECTION, 0)
        .unwrap();
    deadlocked.enter_critical_section(CRITICAL_SECTION).unwrap();
    deadlocked.register_thread(SECOND_THREAD_ID, 0).unwrap();
    deadlocked
        .receive_current_thread_message()
        .expect("owner blocks without an external host completion");
    deadlocked.select_thread(SECOND_THREAD_ID).unwrap();
    deadlocked
        .wait_current_thread_for_critical_section(CRITICAL_SECTION)
        .unwrap_err();
}

#[test]
fn critical_section_wake_restores_the_callback_lane() {
    let mut runtime = initialized_runtime();
    runtime.start_initial_thread().unwrap();
    runtime
        .initialize_critical_section(CRITICAL_SECTION, 0)
        .unwrap();
    runtime.enter_critical_section(CRITICAL_SECTION).unwrap();
    runtime.register_thread(SECOND_THREAD_ID, 0).unwrap();
    runtime.select_thread(SECOND_THREAD_ID).unwrap();
    let callback = runtime
        .push_callback_frame(GuestAddress(0x1000_5000), GuestAddress(0x7f01_0000))
        .expect("enter callback on the contending thread");
    assert_eq!(
        runtime.scheduler_state().unwrap(),
        SchedulerThreadState::InCallback { depth: 1 }
    );

    runtime
        .wait_current_thread_for_critical_section(CRITICAL_SECTION)
        .expect("callback thread waits for the owned critical section");
    runtime.select_thread(INITIAL_THREAD_ID).unwrap();
    runtime.leave_critical_section(CRITICAL_SECTION).unwrap();
    assert_eq!(
        runtime
            .scheduler_state_for_thread(SECOND_THREAD_ID)
            .unwrap(),
        SchedulerThreadState::InCallback { depth: 1 }
    );

    runtime.select_thread(SECOND_THREAD_ID).unwrap();
    assert_eq!(runtime.pop_callback_frame(callback.id).unwrap(), callback);
    runtime.leave_critical_section(CRITICAL_SECTION).unwrap();
}

fn initialized_runtime() -> Runtime {
    Runtime::new(
        RuntimeBootstrap {
            process_id: PROCESS_ID,
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
    .expect("initialize controlled synchronization runtime")
}

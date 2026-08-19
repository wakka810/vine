use vine_core::{
    GuestAddress, LOGICAL_ENCODED_POINTER_NAMESPACE, ProcessAddressSpaceLayout, RuntimeBootstrap,
    RuntimeClockState, RuntimeProcessId, RuntimeStartupInfo, RuntimeState, RuntimeThreadId,
    SchedulerThreadState, X86_STARTUP_INFO_W_BYTE_LENGTH,
};

const PROCESS_ID: RuntimeProcessId = RuntimeProcessId(4096);
const INITIAL_THREAD_ID: RuntimeThreadId = RuntimeThreadId(8192);
const SECOND_THREAD_ID: RuntimeThreadId = RuntimeThreadId(4097);
const CRITICAL_SECTION: GuestAddress = GuestAddress(0x1000_4000);

#[test]
fn synchronization_mutates_only_after_runtime_lifecycle_and_subsystem_preflight() {
    let mut runtime = initialized_runtime();

    let before_start = runtime.clone();
    assert_eq!(
        runtime
            .initialize_critical_section(CRITICAL_SECTION, 4000)
            .expect_err("bootstrapping process cannot initialize synchronization state")
            .code,
        "critical-section-lifecycle-state-invalid"
    );
    assert_eq!(runtime, before_start);

    runtime.start_initial_thread().unwrap();
    let before_invalid_spin = runtime.clone();
    assert_eq!(
        runtime
            .initialize_critical_section(CRITICAL_SECTION, 0x0100_0000)
            .expect_err("invalid Win32 spin count must fail after a mutation-free plan")
            .code,
        "critical-section-spin-count-invalid"
    );
    assert_eq!(runtime, before_invalid_spin);

    runtime
        .initialize_critical_section(CRITICAL_SECTION, 4000)
        .expect("initialize process-owned synchronization state");
    let process_json = serde_json::to_value(runtime.process()).unwrap();
    assert!(process_json.get("criticalSections").is_some());
    assert!(process_json.get("synchronization").is_none());

    runtime
        .enter_critical_section(CRITICAL_SECTION)
        .expect("initial owner enters critical section");
    runtime
        .register_thread(SECOND_THREAD_ID, 0)
        .expect("register contending thread");
    runtime
        .select_thread(SECOND_THREAD_ID)
        .expect("select contending thread");
    let before_contention = runtime.clone();
    assert_eq!(
        runtime
            .enter_critical_section(CRITICAL_SECTION)
            .expect_err("contention remains an unsupported mutation-free result")
            .code,
        "critical-section-contended"
    );
    assert_eq!(runtime, before_contention);

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
    assert_eq!(
        runtime.critical_section(CRITICAL_SECTION).unwrap_err().code,
        "critical-section-uninitialized"
    );
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
        .begin_host_request("runtime.owner-delay.v1")
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
    assert_eq!(
        deadlocked
            .wait_current_thread_for_critical_section(CRITICAL_SECTION)
            .unwrap_err()
            .code,
        "critical-section-no-executable-owner"
    );
}

fn initialized_runtime() -> RuntimeState {
    RuntimeState::new(RuntimeBootstrap {
        process_id: PROCESS_ID,
        initial_thread_id: INITIAL_THREAD_ID,
        initial_last_error: 87,
        address_space: ProcessAddressSpaceLayout::vine_v1(),
        clock: RuntimeClockState {
            wall_clock_file_time: 134_282_016_000_000_000,
            tick_count: 3_600_000,
            performance_counter: 12_345_678_901,
            performance_frequency: 10_000_000,
        },
        startup_info: RuntimeStartupInfo {
            profile_id: "runtime-synchronization-test".to_owned(),
            bytes: vec![0; X86_STARTUP_INFO_W_BYTE_LENGTH],
        },
        process_image_path_a: vine_core::RuntimeProcessImagePathA {
            source_authority: "runtime-synchronization-fixture".to_owned(),
            bytes_with_nul: b"C:\\Vine\\fixture.exe\0".to_vec(),
            encoding: vine_core::RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING.to_owned(),
        },
        initial_current_directory_a: vine_core::RuntimeCurrentDirectoryA {
            source_authority: "runtime-synchronization-process-bootstrap".to_owned(),
            bytes_with_nul: b"C:\\Vine\0".to_vec(),
            encoding: vine_core::RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING.to_owned(),
        },
        encoded_pointer_namespace: LOGICAL_ENCODED_POINTER_NAMESPACE.to_owned(),
        encoded_pointer_cookie: 0xec01_9d3b,
    })
    .expect("initialize controlled synchronization runtime")
}

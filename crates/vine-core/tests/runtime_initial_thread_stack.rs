use vine_core::{
    GUEST_PAGE_SIZE, GuestAddress, GuestMemory, MappingOperation, MemoryLimits,
    ProcessAddressAllocationClassification, ProcessAddressAllocationLifetime,
    ProcessAddressAllocationPurpose, ProcessAddressAllocationReleasePolicy,
    ProcessAddressSpaceLayout, RuntimeBootstrap, RuntimeClockState, RuntimeCurrentDirectoryA,
    RuntimeProcessId, RuntimeProcessImagePathA, RuntimeStartupInfo, RuntimeState, RuntimeThreadId,
    install_initial_thread_stack, install_runtime_return_sentinels,
};

const STACK_TOP: u32 = 0x0070_0000;
const STACK_RESERVE: u64 = 0x0010_0000;
const STACK_COMMIT: u64 = GUEST_PAGE_SIZE;
const INITIAL_GUARD: GuestAddress = GuestAddress(0x006f_e000);

#[test]
fn initial_thread_stack_reserves_registers_and_grows_one_guard_page_atomically() {
    let (mut memory, mut runtime) = prepared_runtime_stack(STACK_RESERVE, STACK_COMMIT);
    let thread_id = runtime.current_thread_id();
    let stack = runtime.thread_stack(thread_id).unwrap().clone();
    assert_eq!(stack.reserved_range.start(), GuestAddress(0x0060_0000));
    assert_eq!(stack.reserved_range.byte_length(), STACK_RESERVE);
    assert_eq!(stack.committed_range.start(), GuestAddress(0x006f_f000));
    assert_eq!(stack.committed_range.byte_length(), STACK_COMMIT);
    assert_eq!(
        stack.guard_page.map(|range| range.start()),
        Some(INITIAL_GUARD)
    );
    assert_eq!(stack.committed_pages, 1);
    assert_eq!(stack.stack_top, GuestAddress(STACK_TOP));

    let allocation = runtime
        .process_address_space_allocator()
        .allocation(stack.address_allocation_id)
        .unwrap();
    assert_eq!(
        allocation.purpose,
        ProcessAddressAllocationPurpose::ThreadStack
    );
    assert_eq!(allocation.range.base, stack.reserved_range.start());
    assert_eq!(allocation.range.byte_length, STACK_RESERVE);
    assert_eq!(
        allocation.usable_byte_length,
        STACK_RESERVE - GUEST_PAGE_SIZE
    );
    assert_eq!(allocation.guard_pages, 1);
    assert_eq!(
        allocation.classification,
        ProcessAddressAllocationClassification::Stack
    );
    assert_eq!(
        allocation.lifetime,
        ProcessAddressAllocationLifetime::Thread
    );
    assert_eq!(
        allocation.release_policy,
        ProcessAddressAllocationReleasePolicy::Explicit
    );
    assert!(!allocation.executable);

    assert_rejected_without_mutation(
        &mut memory,
        &mut runtime,
        GuestAddress(0x0050_0001),
        4,
        "thread-stack-fault-outside-guard",
    );
    assert_rejected_without_mutation(
        &mut memory,
        &mut runtime,
        GuestAddress(INITIAL_GUARD.0 - GUEST_PAGE_SIZE as u32),
        4,
        "thread-stack-fault-outside-guard",
    );
    assert_rejected_without_mutation(
        &mut memory,
        &mut runtime,
        INITIAL_GUARD,
        5,
        "thread-stack-protection-fault",
    );

    let growth = runtime
        .grow_initial_thread_stack(&mut memory, INITIAL_GUARD, 4)
        .unwrap();
    assert_eq!(growth.thread_id, thread_id);
    assert_eq!(growth.address_allocation_id, stack.address_allocation_id);
    assert_eq!(growth.memory_owner, stack.memory_owner);
    assert_eq!(growth.committed_page.start(), INITIAL_GUARD);
    assert_eq!(growth.committed_page.byte_length(), GUEST_PAGE_SIZE);
    assert_eq!(growth.previous_guard_page, growth.committed_page);
    assert_eq!(
        growth.next_guard_page.map(|range| range.start()),
        Some(GuestAddress(0x006f_d000))
    );
    assert_eq!(growth.committed_pages, 2);

    let grown = runtime.thread_stack(thread_id).unwrap();
    assert_eq!(grown.committed_range.start(), INITIAL_GUARD);
    assert_eq!(grown.committed_range.byte_length(), 2 * GUEST_PAGE_SIZE);
    assert_eq!(grown.guard_page, growth.next_guard_page);
    assert_eq!(grown.committed_pages, 2);
    let delta = memory.pending_delta().unwrap();
    assert_eq!(delta.mapping_operations.len(), 1);
    assert_eq!(
        delta.mapping_operations[0],
        MappingOperation::Commit {
            owner: stack.memory_owner,
            range: growth.committed_page,
            protection: vine_core::PageProtection {
                read: true,
                write: true,
                execute: false,
            },
        }
    );
    assert_eq!(delta.dirty_pages.len(), 1);
    let page = &delta.dirty_pages[0];
    assert_eq!(page.address, INITIAL_GUARD);
    assert_eq!(page.owner, stack.memory_owner);
    assert!(page.owner_sealed);
    assert_eq!(
        page.protection,
        vine_core::PageProtection {
            read: true,
            write: true,
            execute: false
        }
    );
    assert!(page.bytes.iter().all(|byte| *byte == 0));

    assert_rejected_without_mutation(
        &mut memory,
        &mut runtime,
        INITIAL_GUARD,
        4,
        "thread-stack-fault-outside-guard",
    );
    memory.acknowledge_delta(delta.generation).unwrap();

    let (mut exhausted_memory, mut exhausted_runtime) =
        prepared_runtime_stack(2 * GUEST_PAGE_SIZE, GUEST_PAGE_SIZE);
    let exhausted_growth = exhausted_runtime
        .grow_initial_thread_stack(
            &mut exhausted_memory,
            GuestAddress(INITIAL_GUARD.0 + 0xd94),
            6,
        )
        .unwrap();
    assert_eq!(exhausted_growth.next_guard_page, None);
    let exhausted_delta = exhausted_memory.pending_delta().unwrap();
    exhausted_memory
        .acknowledge_delta(exhausted_delta.generation)
        .unwrap();
    assert_rejected_without_mutation(
        &mut exhausted_memory,
        &mut exhausted_runtime,
        GuestAddress(0x006f_d000),
        4,
        "thread-stack-reserve-exhausted",
    );
}

fn prepared_runtime_stack(reserve_size: u64, commit_size: u64) -> (GuestMemory, RuntimeState) {
    let mut memory = GuestMemory::new(MemoryLimits::default()).unwrap();
    let sentinels = install_runtime_return_sentinels(&mut memory).unwrap();
    let stack = install_initial_thread_stack(
        &mut memory,
        &sentinels.process_entry_return,
        reserve_size,
        commit_size,
    )
    .unwrap();
    let mut runtime = RuntimeState::new(runtime_bootstrap()).unwrap();
    runtime.register_initial_thread_stack(&stack).unwrap();
    runtime.start_initial_thread().unwrap();
    let reflection = memory.pending_reflection().unwrap();
    memory
        .acknowledge_delta(reflection.invalidation.generation)
        .unwrap();
    (memory, runtime)
}

fn assert_rejected_without_mutation(
    memory: &mut GuestMemory,
    runtime: &mut RuntimeState,
    address: GuestAddress,
    error_code: u32,
    expected_code: &str,
) {
    let runtime_before = runtime.clone();
    let stats_before = memory.stats();
    let content_before = memory.current_reflection().unwrap().content_sha256;
    let error = runtime
        .grow_initial_thread_stack(memory, address, error_code)
        .unwrap_err();
    assert_eq!(error.code, expected_code);
    assert_eq!(*runtime, runtime_before);
    assert_eq!(memory.stats(), stats_before);
    assert_eq!(
        memory.current_reflection().unwrap().content_sha256,
        content_before
    );
}

fn runtime_bootstrap() -> RuntimeBootstrap {
    let mut startup_info = vec![0; 68];
    startup_info[..4].copy_from_slice(&68_u32.to_le_bytes());
    RuntimeBootstrap {
        process_id: RuntimeProcessId(4096),
        initial_thread_id: RuntimeThreadId(8192),
        initial_last_error: 87,
        address_space: ProcessAddressSpaceLayout::vine_v1(),
        clock: RuntimeClockState {
            wall_clock_file_time: 134_282_016_000_000_000,
            tick_count: 3_600_000,
            performance_counter: 12_345_678_901,
        },
        startup_info: RuntimeStartupInfo {
            profile_id: "initial-thread-stack-test-v1".to_owned(),
            bytes: startup_info,
        },
        process_image_path_a: RuntimeProcessImagePathA {
            source_authority: "initial-thread-stack-test".to_owned(),
            bytes_with_nul: b"C:\\Vine\\stack-test.exe\0".to_vec(),
            encoding: vine_core::RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING.to_owned(),
        },
        initial_current_directory_a: RuntimeCurrentDirectoryA {
            source_authority: "initial-thread-stack-process-bootstrap".to_owned(),
            bytes_with_nul: b"C:\\Vine\0".to_vec(),
            encoding: vine_core::RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING.to_owned(),
        },
        encoded_pointer_namespace: vine_core::LOGICAL_ENCODED_POINTER_NAMESPACE.to_owned(),
        encoded_pointer_token_prefix: 0xec01_0000,
    }
}

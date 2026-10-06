use crate::{
    GUEST_PAGE_SIZE, GuestAddress, GuestMemory, MappingOperation, MemoryLimits,
    ProcessAddressAllocationClassification, ProcessAddressAllocationPurpose,
    ProcessAddressAllocationReleasePolicy, Runtime, RuntimeBootstrap, RuntimeClockState,
    RuntimeCurrentDirectoryA, RuntimeProcessId, RuntimeProcessImagePathA, RuntimeStartupInfo,
    RuntimeThreadId, install_initial_thread_stack, install_runtime_return_sentinels,
};

const STACK_TOP: u32 = 0x0070_0000;
const STACK_RESERVE: u64 = 0x0010_0000;
const STACK_COMMIT: u64 = GUEST_PAGE_SIZE;
const INITIAL_GUARD: GuestAddress = GuestAddress(0x006f_e000);

#[test]
fn initial_thread_stack_reserves_registers_and_grows_one_guard_page_atomically() {
    let mut runtime = prepared_runtime_stack(STACK_RESERVE, STACK_COMMIT);
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
    assert_eq!(stack.committed_range.page_count(), 1);
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
        allocation.release_policy,
        ProcessAddressAllocationReleasePolicy::Explicit
    );
    assert!(!allocation.executable);

    assert_rejected_without_mutation(&mut runtime, GuestAddress(0x0050_0001), 4);
    assert_rejected_without_mutation(
        &mut runtime,
        GuestAddress(INITIAL_GUARD.0 - GUEST_PAGE_SIZE as u32),
        4,
    );
    assert_rejected_without_mutation(&mut runtime, INITIAL_GUARD, 5);

    runtime.service_page_fault(INITIAL_GUARD, 4).unwrap();

    let grown = runtime.thread_stack(thread_id).unwrap();
    assert_eq!(grown.committed_range.start(), INITIAL_GUARD);
    assert_eq!(grown.committed_range.byte_length(), 2 * GUEST_PAGE_SIZE);
    assert_eq!(grown.address_allocation_id, stack.address_allocation_id);
    assert_eq!(grown.memory_owner, stack.memory_owner);
    assert_eq!(
        grown.guard_page.map(|range| range.start()),
        Some(GuestAddress(0x006f_d000))
    );
    assert_eq!(grown.committed_range.page_count(), 2);
    let delta = runtime.memory.pending_delta().unwrap();
    assert_eq!(delta.mapping_operations.len(), 1);
    assert_eq!(
        delta.mapping_operations[0],
        MappingOperation::Commit {
            address_allocation_id: None,
            owner: stack.memory_owner,
            range: stack.guard_page.unwrap(),
            protection: crate::PageProtection {
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
        crate::PageProtection {
            read: true,
            write: true,
            execute: false
        }
    );
    assert!(page.bytes.iter().all(|byte| *byte == 0));

    assert_rejected_without_mutation(&mut runtime, INITIAL_GUARD, 4);
    runtime.memory.acknowledge_delta(delta.generation).unwrap();

    let mut exhausted_runtime = prepared_runtime_stack(2 * GUEST_PAGE_SIZE, GUEST_PAGE_SIZE);
    exhausted_runtime
        .service_page_fault(GuestAddress(INITIAL_GUARD.0 + 0xd94), 6)
        .unwrap();
    assert_eq!(
        exhausted_runtime
            .thread_stack(exhausted_runtime.current_thread_id())
            .unwrap()
            .guard_page,
        None
    );
    let exhausted_delta = exhausted_runtime.memory.pending_delta().unwrap();
    exhausted_runtime
        .memory
        .acknowledge_delta(exhausted_delta.generation)
        .unwrap();
    assert_rejected_without_mutation(&mut exhausted_runtime, GuestAddress(0x006f_d000), 4);
}

#[test]
fn stack_growth_rejects_physical_page_exhaustion_without_publishing_a_new_guard_or_range() {
    let mut runtime = prepared_runtime_stack_with_limits(
        STACK_RESERVE,
        STACK_COMMIT,
        MemoryLimits {
            max_physical_pages: 2,
            ..MemoryLimits::default()
        },
    );
    assert_eq!(runtime.memory.stats().committed_pages, 2);
    runtime.set_current_last_error(0x4321).unwrap();
    assert_rejected_without_mutation(&mut runtime, INITIAL_GUARD, 4);
    assert_eq!(runtime.current_last_error().unwrap(), 0x4321);
    assert_eq!(
        runtime
            .thread_stack(runtime.current_thread_id())
            .unwrap()
            .committed_range
            .byte_length(),
        STACK_COMMIT
    );
    assert_eq!(
        runtime
            .thread_stack(runtime.current_thread_id())
            .unwrap()
            .guard_page
            .unwrap()
            .start(),
        INITIAL_GUARD
    );
    assert!(runtime.memory.pending_delta().is_err());
}

#[test]
fn stack_registration_rejects_unknown_duplicate_and_overlapping_owners_without_mutation() {
    let memory = GuestMemory::new(MemoryLimits::default()).unwrap();
    let mut runtime = Runtime::new(runtime_bootstrap(), memory).unwrap();
    let callback = runtime
        .process_address_space_allocator()
        .allocation_for_purpose(&ProcessAddressAllocationPurpose::CallbackReturns)
        .unwrap()
        .clone();
    let sentinels = install_runtime_return_sentinels(&mut runtime.memory, &callback).unwrap();
    let stack = install_initial_thread_stack(
        &mut runtime.memory,
        &sentinels.process_entry_return,
        STACK_RESERVE,
        STACK_COMMIT,
    )
    .unwrap();
    let memory_before = runtime.memory.current_reflection().unwrap();
    assert!(
        runtime
            .register_thread_stack(RuntimeThreadId(0xffff), &stack)
            .is_err()
    );
    assert!(runtime.thread_state(RuntimeThreadId(0xffff)).is_err());
    assert!(runtime.thread_stack(runtime.current_thread_id()).is_err());
    assert_eq!(runtime.memory.current_reflection().unwrap(), memory_before);

    let mut missing_owner_stack = stack.clone();
    missing_owner_stack.owner = crate::MemoryOwnerId(u32::MAX);
    assert!(
        runtime
            .register_initial_thread_stack(&missing_owner_stack)
            .is_err()
    );
    assert!(runtime.thread_stack(runtime.current_thread_id()).is_err());
    assert_eq!(runtime.memory.current_reflection().unwrap(), memory_before);
    missing_owner_stack.owner = sentinels.process_entry_return.owner;
    assert!(
        runtime
            .register_initial_thread_stack(&missing_owner_stack)
            .is_err()
    );
    assert!(runtime.thread_stack(runtime.current_thread_id()).is_err());
    assert_eq!(runtime.memory.current_reflection().unwrap(), memory_before);
    runtime.register_initial_thread_stack(&stack).unwrap();
    let registered = runtime
        .thread_stack(runtime.current_thread_id())
        .unwrap()
        .clone();
    assert!(runtime.register_initial_thread_stack(&stack).is_err());
    assert_eq!(
        runtime.thread_stack(runtime.current_thread_id()).unwrap(),
        &registered
    );
    assert_eq!(runtime.memory.current_reflection().unwrap(), memory_before);

    runtime.start_initial_thread().unwrap();
    let second = RuntimeThreadId(8193);
    runtime.register_thread(second, 23).unwrap();
    assert!(runtime.register_thread_stack(second, &stack).is_err());
    assert_eq!(
        runtime.thread_stack(runtime.current_thread_id()).unwrap(),
        &registered
    );
    assert!(runtime.thread_state(second).is_ok());
    assert_eq!(runtime.memory.current_reflection().unwrap(), memory_before);
    assert!(runtime.thread_stack(second).is_err());
}

fn prepared_runtime_stack(reserve_size: u64, commit_size: u64) -> Runtime {
    prepared_runtime_stack_with_limits(reserve_size, commit_size, MemoryLimits::default())
}

fn prepared_runtime_stack_with_limits(
    reserve_size: u64,
    commit_size: u64,
    limits: MemoryLimits,
) -> Runtime {
    let memory = GuestMemory::new(limits).unwrap();
    let mut runtime = Runtime::new(runtime_bootstrap(), memory).unwrap();
    let callback_allocation = runtime
        .process_address_space_allocator()
        .allocation_for_purpose(&ProcessAddressAllocationPurpose::CallbackReturns)
        .unwrap()
        .clone();
    let sentinels =
        install_runtime_return_sentinels(&mut runtime.memory, &callback_allocation).unwrap();
    let stack = install_initial_thread_stack(
        &mut runtime.memory,
        &sentinels.process_entry_return,
        reserve_size,
        commit_size,
    )
    .unwrap();
    runtime.register_initial_thread_stack(&stack).unwrap();
    runtime.start_initial_thread().unwrap();
    let reflection = runtime.memory.pending_reflection().unwrap();
    runtime
        .memory
        .acknowledge_delta(reflection.invalidation.generation)
        .unwrap();
    runtime
}

fn assert_rejected_without_mutation(runtime: &mut Runtime, address: GuestAddress, error_code: u32) {
    let thread_id = runtime.current_thread_id();
    let stack_before = runtime.thread_stack(thread_id).unwrap().clone();
    let scheduler_before = runtime.scheduler_state().unwrap();
    let last_error_before = runtime.current_last_error().unwrap();
    let stats_before = runtime.memory.stats();
    let reflection_before = runtime.memory.current_reflection().unwrap();
    runtime
        .service_page_fault(address, error_code)
        .expect_err("invalid or exhausted stack growth must fail without mutation");
    assert_eq!(runtime.current_thread_id(), thread_id);
    assert_eq!(runtime.thread_stack(thread_id).unwrap(), &stack_before);
    assert_eq!(runtime.scheduler_state().unwrap(), scheduler_before);
    assert_eq!(runtime.current_last_error().unwrap(), last_error_before);
    assert_eq!(runtime.memory.stats(), stats_before);
    assert_eq!(
        runtime.memory.current_reflection().unwrap(),
        reflection_before
    );
}

fn runtime_bootstrap() -> RuntimeBootstrap {
    let mut startup_info = vec![0; 68];
    startup_info[..4].copy_from_slice(&68_u32.to_le_bytes());
    RuntimeBootstrap {
        process_id: RuntimeProcessId(4096),
        initial_thread_id: RuntimeThreadId(8192),
        initial_last_error: 87,
        clock: RuntimeClockState {
            wall_clock_file_time: 134_282_016_000_000_000,
            tick_count: 3_600_000,
            performance_counter: 12_345_678_901,
            performance_frequency: 10_000_000,
        },
        startup_info: RuntimeStartupInfo {
            bytes: startup_info,
        },
        process_image_path_a: RuntimeProcessImagePathA {
            bytes_with_nul: b"C:\\Vine\\stack-test.exe\0".to_vec(),
        },
        initial_current_directory_a: RuntimeCurrentDirectoryA {
            bytes_with_nul: b"C:\\Vine\0".to_vec(),
        },
        encoded_pointer_cookie: 0xec01_9d3b,
    }
}

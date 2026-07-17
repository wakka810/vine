use vine_core::{
    ExecutionSession, GUEST_PAGE_SIZE, GuestAddress, GuestMemory,
    LOGICAL_ENCODED_POINTER_NAMESPACE, MappingOperation, MemoryLimits,
    ProcessAddressAllocationPurpose, ProcessAddressSpaceLayout, RUNTIME_PRIVATE_HEAP_ARENA_STRIDE,
    RUNTIME_PRIVATE_HEAP_LARGE_SLOT_BASE, RUNTIME_PRIVATE_HEAP_LARGE_SLOT_SIZE,
    RUNTIME_PRIVATE_HEAP_UNINITIALIZED_BYTE, RuntimeBootstrap, RuntimeClockState,
    RuntimeHeapAllocationId, RuntimeHeapInitialization, RuntimePageFaultResolution,
    RuntimeProcessId, RuntimeStartupInfo, RuntimeState, RuntimeThreadId,
    X86_STARTUP_INFO_W_BYTE_LENGTH,
};

const PROCESS_ID: u32 = 4096;
const THREAD_ID: u32 = 8192;

#[test]
fn large_heap_reserves_lazily_materializes_pages_and_reuses_its_permanent_slot() {
    let mut memory = GuestMemory::new(MemoryLimits {
        max_physical_pages: 2,
        ..MemoryLimits::default()
    })
    .unwrap();
    let mut runtime = running_runtime();
    let handle = runtime.allocate_private_heap(0, 0x1000, 0).unwrap().handle;
    let small = ExecutionSession::new(&mut memory, &mut runtime)
        .allocate_private_heap_block(handle, 0x20, RuntimeHeapInitialization::Zeroed)
        .unwrap();
    assert_eq!(small.id, RuntimeHeapAllocationId(1));
    acknowledge_reflection(&mut memory);

    assert_rejected_without_mutation(
        &mut memory,
        &mut runtime,
        |memory, runtime| {
            ExecutionSession::new(memory, runtime)
                .allocate_private_heap_block(
                    handle,
                    32 * 1024 * 1024,
                    RuntimeHeapInitialization::DeterministicUninitialized,
                )
                .map(|_| ())
        },
        "heap-backing-capacity-exceeded",
    );

    let physical_before_large = memory.stats().physical_pages;
    let large = ExecutionSession::new(&mut memory, &mut runtime)
        .allocate_private_heap_block(
            handle,
            RUNTIME_PRIVATE_HEAP_LARGE_SLOT_SIZE,
            RuntimeHeapInitialization::DeterministicUninitialized,
        )
        .unwrap();
    assert_eq!(large.id, RuntimeHeapAllocationId(2));
    assert_eq!(large.address, RUNTIME_PRIVATE_HEAP_LARGE_SLOT_BASE);
    assert_eq!(large.byte_length, RUNTIME_PRIVATE_HEAP_LARGE_SLOT_SIZE);
    assert_eq!(memory.stats().physical_pages, physical_before_large);
    let reserve_delta = memory.pending_delta().unwrap();
    assert_eq!(reserve_delta.mapping_operations.len(), 1);
    assert!(matches!(
        reserve_delta.mapping_operations[0],
        MappingOperation::Reserve { range, .. }
            if range.start() == RUNTIME_PRIVATE_HEAP_LARGE_SLOT_BASE
                && range.byte_length() == u64::from(RUNTIME_PRIVATE_HEAP_LARGE_SLOT_SIZE)
    ));
    assert!(reserve_delta.dirty_pages.is_empty());
    memory.acknowledge_delta(reserve_delta.generation).unwrap();

    let large_backing = runtime.large_heap_allocation_backing().unwrap();
    let large_address_allocation_id = large_backing.address_allocation_id;
    assert!(large_backing.materialized_pages.is_empty());
    let address_allocation = runtime
        .process_address_space_allocator()
        .allocation(large_address_allocation_id)
        .unwrap();
    assert_eq!(
        address_allocation.purpose,
        ProcessAddressAllocationPurpose::PrivateHeapLargeSlot
    );

    assert_rejected_without_mutation(
        &mut memory,
        &mut runtime,
        |memory, runtime| {
            ExecutionSession::new(memory, runtime)
                .allocate_private_heap_block(
                    handle,
                    RUNTIME_PRIVATE_HEAP_LARGE_SLOT_SIZE,
                    RuntimeHeapInitialization::Zeroed,
                )
                .map(|_| ())
        },
        "large-heap-slot-in-use",
    );

    let fault_address = GuestAddress(RUNTIME_PRIVATE_HEAP_LARGE_SLOT_BASE.0 + 0x123);
    let resolution = runtime
        .service_page_fault(&mut memory, fault_address, 6)
        .unwrap();
    let materialization = match resolution {
        RuntimePageFaultResolution::PrivateHeapPageMaterialization { materialization } => {
            materialization
        }
        other => panic!("unexpected page-fault resolution: {other:?}"),
    };
    assert_eq!(materialization.handle, handle);
    assert_eq!(materialization.allocation, large);
    assert_eq!(materialization.fault_address, fault_address);
    assert_eq!(materialization.page_fault_error_code, 6);
    assert_eq!(
        materialization.committed_page.start(),
        RUNTIME_PRIVATE_HEAP_LARGE_SLOT_BASE
    );
    assert_eq!(
        materialization.committed_page.byte_length(),
        GUEST_PAGE_SIZE
    );
    assert_eq!(materialization.materialized_pages, 1);
    let materialize_delta = memory.pending_delta().unwrap();
    assert_eq!(materialize_delta.mapping_operations.len(), 1);
    assert!(matches!(
        materialize_delta.mapping_operations[0],
        MappingOperation::Commit { range, owner, .. }
            if range == materialization.committed_page && owner == materialization.memory_owner
    ));
    assert_eq!(materialize_delta.dirty_pages.len(), 1);
    assert!(
        materialize_delta.dirty_pages[0]
            .bytes
            .iter()
            .all(|byte| *byte == RUNTIME_PRIVATE_HEAP_UNINITIALIZED_BYTE)
    );
    memory
        .acknowledge_delta(materialize_delta.generation)
        .unwrap();

    assert_rejected_without_mutation(
        &mut memory,
        &mut runtime,
        |memory, runtime| {
            runtime
                .service_page_fault(memory, fault_address, 6)
                .map(|_| ())
        },
        "large-heap-page-already-materialized",
    );
    assert_rejected_without_mutation(
        &mut memory,
        &mut runtime,
        |memory, runtime| {
            runtime
                .service_page_fault(
                    memory,
                    GuestAddress(RUNTIME_PRIVATE_HEAP_LARGE_SLOT_BASE.0 - 1),
                    4,
                )
                .map(|_| ())
        },
        "guest-page-fault-unhandled",
    );
    assert_rejected_without_mutation(
        &mut memory,
        &mut runtime,
        |memory, runtime| {
            runtime
                .service_page_fault(
                    memory,
                    GuestAddress(RUNTIME_PRIVATE_HEAP_LARGE_SLOT_BASE.0 + GUEST_PAGE_SIZE as u32),
                    7,
                )
                .map(|_| ())
        },
        "large-heap-protection-fault",
    );
    assert_rejected_without_mutation(
        &mut memory,
        &mut runtime,
        |memory, runtime| {
            runtime
                .service_page_fault(
                    memory,
                    GuestAddress(RUNTIME_PRIVATE_HEAP_LARGE_SLOT_BASE.0 + GUEST_PAGE_SIZE as u32),
                    6,
                )
                .map(|_| ())
        },
        "physical-page-quota-exceeded",
    );

    let freed = ExecutionSession::new(&mut memory, &mut runtime)
        .free_private_heap_block(handle, large.address)
        .unwrap();
    assert_eq!(freed, large);
    let release_delta = memory.pending_delta().unwrap();
    assert!(matches!(
        release_delta.mapping_operations.as_slice(),
        [MappingOperation::Release { range, .. }]
            if range.start() == RUNTIME_PRIVATE_HEAP_LARGE_SLOT_BASE
    ));
    memory.acknowledge_delta(release_delta.generation).unwrap();
    assert!(runtime.large_heap_allocation_backing().is_none());
    assert_eq!(memory.stats().physical_pages, physical_before_large);
    assert!(
        runtime
            .process_address_space_allocator()
            .allocation(large_address_allocation_id)
            .is_ok(),
        "the permanent large-slot address record must survive HeapFree"
    );
    assert_rejected_without_mutation(
        &mut memory,
        &mut runtime,
        |memory, runtime| {
            runtime
                .service_page_fault(memory, fault_address, 6)
                .map(|_| ())
        },
        "guest-page-fault-unhandled",
    );

    let reused = ExecutionSession::new(&mut memory, &mut runtime)
        .allocate_private_heap_block(
            handle,
            RUNTIME_PRIVATE_HEAP_LARGE_SLOT_SIZE,
            RuntimeHeapInitialization::Zeroed,
        )
        .unwrap();
    assert_eq!(reused.id, RuntimeHeapAllocationId(3));
    assert_eq!(reused.address, large.address);
    acknowledge_delta(&mut memory);
    let zero_page = GuestAddress(reused.address.0 + GUEST_PAGE_SIZE as u32 + 0x88);
    runtime
        .service_page_fault(&mut memory, zero_page, 4)
        .unwrap();
    let zero_delta = memory.pending_delta().unwrap();
    assert_eq!(zero_delta.dirty_pages.len(), 1);
    assert!(
        zero_delta.dirty_pages[0]
            .bytes
            .iter()
            .all(|byte| *byte == 0)
    );
    memory.acknowledge_delta(zero_delta.generation).unwrap();

    runtime.request_current_process_exit(0).unwrap();
    let teardown = ExecutionSession::new(&mut memory, &mut runtime)
        .teardown_process_heaps()
        .unwrap();
    assert_eq!(teardown.handles, vec![handle]);
    assert_eq!(teardown.allocations_discarded, 2);
    assert_eq!(
        teardown.reserved_bytes_released,
        u64::from(RUNTIME_PRIVATE_HEAP_ARENA_STRIDE)
            + u64::from(RUNTIME_PRIVATE_HEAP_LARGE_SLOT_SIZE)
    );
    assert_eq!(teardown.committed_bytes_released, 2 * GUEST_PAGE_SIZE);
    acknowledge_delta(&mut memory);
    assert!(runtime.large_heap_allocation_backing().is_none());
    assert_eq!(memory.stats().owners, 0);
    assert_eq!(memory.stats().regions, 0);
    assert_eq!(memory.stats().physical_pages, 0);
}

fn assert_rejected_without_mutation<F>(
    memory: &mut GuestMemory,
    runtime: &mut RuntimeState,
    operation: F,
    expected_code: &str,
) where
    F: FnOnce(&mut GuestMemory, &mut RuntimeState) -> Result<(), vine_core::RuntimeStateError>,
{
    let runtime_before = runtime.clone();
    let memory_before = memory.stats();
    let content_before = memory.current_reflection().unwrap().content_sha256;
    let error = operation(memory, runtime).unwrap_err();
    assert_eq!(error.code, expected_code);
    assert_eq!(*runtime, runtime_before);
    assert_eq!(memory.stats(), memory_before);
    assert_eq!(
        memory.current_reflection().unwrap().content_sha256,
        content_before
    );
}

fn acknowledge_reflection(memory: &mut GuestMemory) {
    let reflection = memory.pending_reflection().unwrap();
    memory
        .acknowledge_delta(reflection.invalidation.generation)
        .unwrap();
}

fn acknowledge_delta(memory: &mut GuestMemory) {
    let delta = memory.pending_delta().unwrap();
    memory.acknowledge_delta(delta.generation).unwrap();
}

fn running_runtime() -> RuntimeState {
    let mut runtime = RuntimeState::new(RuntimeBootstrap {
        process_id: RuntimeProcessId(PROCESS_ID),
        initial_thread_id: RuntimeThreadId(THREAD_ID),
        initial_last_error: 0,
        address_space: ProcessAddressSpaceLayout::vine_v1(),
        clock: RuntimeClockState {
            wall_clock_file_time: 134_282_016_000_000_000,
            tick_count: 3_600_000,
            performance_counter: 12_345_678_901,
        },
        startup_info: RuntimeStartupInfo {
            profile_id: "runtime-large-heap-materialization-test".to_owned(),
            bytes: vec![0; X86_STARTUP_INFO_W_BYTE_LENGTH],
        },
        process_image_path_a: vine_core::RuntimeProcessImagePathA {
            source_authority: "runtime-large-heap-materialization-fixture".to_owned(),
            bytes_with_nul: b"C:\\Vine\\fixture.exe\0".to_vec(),
            encoding: vine_core::RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING.to_owned(),
        },
        initial_current_directory_a: vine_core::RuntimeCurrentDirectoryA {
            source_authority: "runtime-large-heap-materialization-bootstrap".to_owned(),
            bytes_with_nul: b"C:\\Vine\0".to_vec(),
            encoding: vine_core::RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING.to_owned(),
        },
        encoded_pointer_namespace: LOGICAL_ENCODED_POINTER_NAMESPACE.to_owned(),
        encoded_pointer_token_prefix: 0xec01_0000,
    })
    .unwrap();
    runtime.start_initial_thread().unwrap();
    runtime
}

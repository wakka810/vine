use crate::{
    GUEST_PAGE_SIZE, GuestAddress, GuestMemory, MappingOperation, MemoryLimits,
    ProcessAddressAllocationPurpose, RUNTIME_PRIVATE_HEAP_UNINITIALIZED_BYTE, Runtime,
    RuntimeBootstrap, RuntimeClockState, RuntimeHeapAllocationId, RuntimeHeapInitialization,
    RuntimeProcessId, RuntimeStartupInfo, RuntimeThreadId, X86_STARTUP_INFO_W_BYTE_LENGTH,
};

const PROCESS_ID: u32 = 4096;
const THREAD_ID: u32 = 8192;
const REACHED_FIRST_LARGE_BYTES: u32 = 47_354_272;
const REACHED_VARIABLE_LARGE_BYTES: u32 = 61_219_868;

#[test]
fn large_heap_reserves_lazily_materializes_pages_and_retires_its_allocation() {
    large_heap_materialization_case(true);
    large_heap_materialization_case(false);
}

fn large_heap_materialization_case(poison: bool) {
    let memory = GuestMemory::new(MemoryLimits {
        max_physical_pages: 3,
        ..MemoryLimits::default()
    })
    .unwrap();
    let mut runtime = running_runtime(memory);
    runtime.set_heap_poisoning_enabled(poison);
    let handle = runtime.allocate_private_heap(0x1000, 0).unwrap();
    let small = runtime
        .allocate_private_heap_block(handle, 0x20, RuntimeHeapInitialization::Zeroed)
        .unwrap()
        .unwrap();
    assert_eq!(small.id, RuntimeHeapAllocationId(1));
    acknowledge_reflection(&mut runtime.memory);

    assert_rejected_without_mutation(
        &mut runtime,
        handle,
        |runtime| {
            runtime
                .allocate_private_heap_block(
                    handle,
                    u32::MAX,
                    RuntimeHeapInitialization::DeterministicUninitialized,
                )
                .unwrap()
                .is_none()
        },
        "heap-backing-capacity-exceeded",
    );

    let physical_before_large = runtime.memory.stats().physical_pages;
    let large = runtime
        .allocate_private_heap_block(
            handle,
            REACHED_FIRST_LARGE_BYTES,
            RuntimeHeapInitialization::DeterministicUninitialized,
        )
        .unwrap()
        .unwrap();
    assert_eq!(large.id, RuntimeHeapAllocationId(2));
    assert!(large.address.0.is_multiple_of(65_536));
    assert_eq!(large.byte_length, REACHED_FIRST_LARGE_BYTES);
    assert_eq!(large.reserved_byte_length, REACHED_FIRST_LARGE_BYTES);
    assert_eq!(runtime.memory.stats().physical_pages, physical_before_large);
    let reserve_delta = runtime.memory.pending_delta().unwrap();
    assert_eq!(reserve_delta.mapping_operations.len(), 1);
    assert!(matches!(
        reserve_delta.mapping_operations[0],
        MappingOperation::Reserve { range, .. }
            if range.start() == large.address
                && range.byte_length() == u64::from(REACHED_FIRST_LARGE_BYTES).next_multiple_of(GUEST_PAGE_SIZE)
    ));
    assert!(reserve_delta.dirty_pages.is_empty());
    runtime
        .memory
        .acknowledge_delta(reserve_delta.generation)
        .unwrap();

    let large_backing = &runtime.large_heap_allocation_backings()[&large.address.0];
    let large_address_allocation_id = large_backing.address_allocation_id;
    assert!(large_backing.materialized_pages.is_empty());
    let address_allocation = runtime
        .process_address_space_allocator()
        .allocation(large_address_allocation_id)
        .unwrap();
    assert_eq!(
        address_allocation.purpose,
        ProcessAddressAllocationPurpose::PrivateHeapLargeAllocation
    );

    let second_large = runtime
        .allocate_private_heap_block(
            handle,
            REACHED_VARIABLE_LARGE_BYTES,
            RuntimeHeapInitialization::DeterministicUninitialized,
        )
        .unwrap()
        .unwrap();
    assert_eq!(second_large.id, RuntimeHeapAllocationId(3));
    assert!(second_large.address.0.is_multiple_of(65_536));
    assert_ne!(second_large.address, large.address);
    assert_eq!(second_large.byte_length, REACHED_VARIABLE_LARGE_BYTES);
    assert_eq!(second_large.reserved_byte_length, 61_219_872);
    let second_reserve_delta = runtime.memory.pending_delta().unwrap();
    assert!(matches!(
        second_reserve_delta.mapping_operations.as_slice(),
        [MappingOperation::Reserve { range, .. }]
            if range.start() == second_large.address
                && range.byte_length() == u64::from(REACHED_VARIABLE_LARGE_BYTES).next_multiple_of(GUEST_PAGE_SIZE)
    ));
    runtime
        .memory
        .acknowledge_delta(second_reserve_delta.generation)
        .unwrap();
    assert_eq!(runtime.large_heap_allocation_backings().len(), 2);

    let fault_address = GuestAddress(large.address.0 + 0x123);
    runtime.service_page_fault(fault_address, 6).unwrap();
    let backing = &runtime.large_heap_allocation_backings()[&large.address.0];
    assert_eq!(backing.handle, handle);
    assert_eq!(backing.allocation, large);
    assert!(backing.materialized_pages.contains(&large.address.0));
    let memory_owner = backing.memory_owner;
    let materialize_delta = runtime.memory.pending_delta().unwrap();
    assert_eq!(materialize_delta.mapping_operations.len(), 1);
    assert!(matches!(
        materialize_delta.mapping_operations[0],
        MappingOperation::Commit { range, owner, .. }
            if range.start() == large.address && range.byte_length() == GUEST_PAGE_SIZE && owner == memory_owner
    ));
    assert_eq!(materialize_delta.dirty_pages.len(), 1);
    assert!(
        materialize_delta.dirty_pages[0]
            .bytes
            .iter()
            .all(|byte| *byte
                == if poison {
                    RUNTIME_PRIVATE_HEAP_UNINITIALIZED_BYTE
                } else {
                    0
                })
    );
    runtime
        .memory
        .acknowledge_delta(materialize_delta.generation)
        .unwrap();

    assert_rejected_without_mutation(
        &mut runtime,
        handle,
        |runtime| runtime.service_page_fault(fault_address, 6).is_err(),
        "large-heap-page-already-materialized",
    );
    assert_rejected_without_mutation(
        &mut runtime,
        handle,
        |runtime| {
            runtime
                .service_page_fault(GuestAddress(large.address.0 - 1), 4)
                .is_err()
        },
        "guest-page-fault-unhandled",
    );
    assert_rejected_without_mutation(
        &mut runtime,
        handle,
        |runtime| {
            runtime
                .service_page_fault(GuestAddress(large.address.0 + REACHED_FIRST_LARGE_BYTES), 6)
                .is_err()
        },
        "guest-page-fault-unhandled",
    );
    assert_rejected_without_mutation(
        &mut runtime,
        handle,
        |runtime| {
            runtime
                .service_page_fault(GuestAddress(large.address.0 + GUEST_PAGE_SIZE as u32), 7)
                .is_err()
        },
        "large-heap-protection-fault",
    );
    let second_fault_address = GuestAddress(second_large.address.0 + 0x456);
    runtime.service_page_fault(second_fault_address, 6).unwrap();
    let backing = runtime
        .large_heap_allocation_backings()
        .values()
        .find(|backing| backing.allocation == second_large)
        .unwrap();
    assert!(backing.materialized_pages.contains(&second_large.address.0));
    acknowledge_delta(&mut runtime.memory);
    assert_rejected_without_mutation(
        &mut runtime,
        handle,
        |runtime| {
            runtime
                .service_page_fault(GuestAddress(large.address.0 + GUEST_PAGE_SIZE as u32), 6)
                .is_err()
        },
        "physical-page-quota-exceeded",
    );

    runtime
        .free_private_heap_block(handle, large.address)
        .unwrap();
    assert!(
        runtime
            .private_heap_allocation_option(handle, large.address)
            .unwrap()
            .is_none()
    );
    let release_delta = runtime.memory.pending_delta().unwrap();
    assert!(matches!(
        release_delta.mapping_operations.as_slice(),
        [MappingOperation::Release { range, .. }]
            if range.start() == large.address
    ));
    runtime
        .memory
        .acknowledge_delta(release_delta.generation)
        .unwrap();
    assert_eq!(runtime.large_heap_allocation_backings().len(), 1);
    assert_eq!(
        runtime.large_heap_allocation_backing().unwrap().allocation,
        second_large
    );
    assert_eq!(
        runtime.memory.stats().physical_pages,
        physical_before_large + 1
    );
    assert!(
        runtime
            .process_address_space_allocator()
            .allocation(large_address_allocation_id)
            .is_err(),
        "HeapFree must retire the large allocation address authority"
    );
    assert_rejected_without_mutation(
        &mut runtime,
        handle,
        |runtime| runtime.service_page_fault(fault_address, 6).is_err(),
        "guest-page-fault-unhandled",
    );

    let reused = runtime
        .allocate_private_heap_block(
            handle,
            REACHED_FIRST_LARGE_BYTES,
            RuntimeHeapInitialization::Zeroed,
        )
        .unwrap()
        .unwrap();
    assert_eq!(reused.id, RuntimeHeapAllocationId(4));
    assert_eq!(reused.address, large.address);
    acknowledge_delta(&mut runtime.memory);
    let zero_page = GuestAddress(reused.address.0 + GUEST_PAGE_SIZE as u32 + 0x88);
    runtime.service_page_fault(zero_page, 4).unwrap();
    let zero_delta = runtime.memory.pending_delta().unwrap();
    assert_eq!(zero_delta.dirty_pages.len(), 1);
    assert!(
        zero_delta.dirty_pages[0]
            .bytes
            .iter()
            .all(|byte| *byte == 0)
    );
    runtime
        .memory
        .acknowledge_delta(zero_delta.generation)
        .unwrap();
}

#[test]
fn lazy_heap_pages_materialize_in_sparse_order_without_eager_allocation() {
    let memory = GuestMemory::new(MemoryLimits {
        max_physical_pages: 3,
        ..MemoryLimits::default()
    })
    .unwrap();
    let mut runtime = running_runtime(memory);
    let handle = runtime.allocate_private_heap(0, 0).unwrap();
    let allocation = runtime
        .allocate_private_heap_block(
            handle,
            crate::RUNTIME_PRIVATE_HEAP_ARENA_STRIDE + 0x3000,
            RuntimeHeapInitialization::Zeroed,
        )
        .unwrap()
        .unwrap();
    assert_eq!(runtime.memory.stats().physical_pages, 0);
    acknowledge_reflection(&mut runtime.memory);
    for (index, offset) in [0x2000, 0, 0x1000].into_iter().enumerate() {
        let address = GuestAddress(allocation.address.0 + offset);
        runtime
            .materialize_private_heap_page(GuestAddress(address.0 + 0x123), 6)
            .unwrap();
        assert_eq!(
            runtime
                .large_heap_allocation_backing()
                .unwrap()
                .materialized_pages
                .len(),
            index + 1
        );
        acknowledge_delta(&mut runtime.memory);
        assert_eq!(
            runtime
                .memory
                .read(address, GUEST_PAGE_SIZE as usize)
                .unwrap(),
            vec![0; GUEST_PAGE_SIZE as usize]
        );
    }
    assert_eq!(runtime.memory.stats().physical_pages, 3);
    assert_rejected_without_mutation(
        &mut runtime,
        handle,
        |runtime| {
            runtime
                .materialize_private_heap_page(allocation.address, 6)
                .is_err()
        },
        "duplicate sparse page",
    );
}

#[test]
fn three_large_allocations_use_disjoint_live_ranges_and_reject_retired_cpu_owners() {
    let mut runtime = running_runtime(
        GuestMemory::new(MemoryLimits {
            max_physical_pages: 9,
            ..MemoryLimits::default()
        })
        .unwrap(),
    );
    let handle = runtime.allocate_private_heap(0, 0).unwrap();
    let lengths = [64 * 1024 * 1024, 64 * 1024 * 1024 + 8193, 33_177_600];
    let allocations = lengths.map(|length| {
        runtime
            .allocate_private_heap_block(handle, length, RuntimeHeapInitialization::Zeroed)
            .unwrap()
            .unwrap()
    });
    assert_eq!(runtime.heap(handle).unwrap().live_allocations, 3);
    assert_eq!(runtime.memory.stats().physical_pages, 0);
    acknowledge_reflection(&mut runtime.memory);
    for (index, allocation) in allocations.iter().enumerate() {
        let backing = &runtime.large_heap_allocation_backings()[&allocation.address.0];
        let range = backing.range;
        assert_eq!(range.start(), allocation.address);
        assert_eq!(
            range.byte_length(),
            u64::from(allocation.reserved_byte_length).next_multiple_of(GUEST_PAGE_SIZE)
        );
        for other in &allocations {
            if other.address != allocation.address {
                let other_range = runtime.large_heap_allocation_backings()[&other.address.0].range;
                assert!(
                    range.end() <= u64::from(other_range.start().0)
                        || other_range.end() <= u64::from(range.start().0)
                );
            }
        }
        for offset in [0, allocation.byte_length / 2, allocation.byte_length - 1] {
            let address = GuestAddress(allocation.address.0 + offset);
            runtime.service_page_fault(address, 6).unwrap();
            acknowledge_delta(&mut runtime.memory);
            assert_eq!(runtime.memory.read(address, 1).unwrap(), [0]);
            runtime.memory.write(address, &[index as u8 + 1]).unwrap();
            acknowledge_delta(&mut runtime.memory);
        }
        assert_rejected_without_mutation(
            &mut runtime,
            handle,
            |runtime| {
                runtime
                    .service_page_fault(
                        GuestAddress(allocation.address.0 + allocation.byte_length),
                        6,
                    )
                    .is_err()
            },
            "large allocation exact end is not owned",
        );
    }
    assert_eq!(runtime.memory.stats().physical_pages, 9);
    for (index, allocation) in allocations.iter().enumerate() {
        for offset in [0, allocation.byte_length / 2, allocation.byte_length - 1] {
            assert_eq!(
                runtime
                    .memory
                    .read(GuestAddress(allocation.address.0 + offset), 1)
                    .unwrap(),
                [index as u8 + 1]
            );
        }
    }

    let retired = allocations[1];
    let backing = &runtime.large_heap_allocation_backings()[&retired.address.0];
    let retired_owner = backing.memory_owner;
    let retired_record = backing.address_allocation_id;
    runtime
        .free_private_heap_block(handle, retired.address)
        .unwrap();
    acknowledge_delta(&mut runtime.memory);
    assert_eq!(runtime.memory.stats().physical_pages, 6);
    assert!(
        runtime
            .process_address_space_allocator()
            .allocation(retired_record)
            .is_err()
    );
    assert!(
        runtime
            .memory
            .committed_page(retired.address)
            .unwrap()
            .is_none()
    );
    let reused = runtime
        .allocate_private_heap_block(
            handle,
            retired.byte_length,
            RuntimeHeapInitialization::Zeroed,
        )
        .unwrap()
        .unwrap();
    assert_eq!(reused.address, retired.address);
    let reused_backing = &runtime.large_heap_allocation_backings()[&reused.address.0];
    assert_ne!(reused_backing.memory_owner, retired_owner);
    assert_ne!(reused_backing.address_allocation_id, retired_record);
    acknowledge_delta(&mut runtime.memory);
    runtime.service_page_fault(reused.address, 6).unwrap();
    acknowledge_delta(&mut runtime.memory);
    let before = runtime.memory.current_reflection().unwrap();
    let writeback = crate::memory::CpuWritebackBatch {
        base_generation: runtime.memory.stats().generation,
        pages: vec![crate::memory::CpuWritebackPage {
            address: reused.address,
            owner: retired_owner,
            bytes: vec![0xff; GUEST_PAGE_SIZE as usize],
        }],
    }
    .encode_binary()
    .unwrap();
    assert!(
        runtime
            .memory
            .reconcile_cpu_writeback_binary(&writeback.control, &writeback.page_bytes)
            .is_err()
    );
    assert_eq!(runtime.memory.current_reflection().unwrap(), before);
    assert_eq!(runtime.memory.read(reused.address, 1).unwrap(), [0]);
}

#[test]
fn large_allocation_owner_region_and_address_exhaustion_preserve_publication() {
    for (max_owners, max_regions) in [(1, 2), (2, 1)] {
        let mut runtime = running_runtime(
            GuestMemory::new(MemoryLimits {
                max_owners,
                max_regions,
                ..MemoryLimits::default()
            })
            .unwrap(),
        );
        let handle = runtime.allocate_private_heap(0, 0).unwrap();
        let first = runtime
            .allocate_private_heap_block(handle, 33_177_600, RuntimeHeapInitialization::Zeroed)
            .unwrap()
            .unwrap();
        acknowledge_reflection(&mut runtime.memory);
        let allocator_before = runtime.process_address_space_allocator().clone();
        assert_rejected_without_mutation(
            &mut runtime,
            handle,
            |runtime| {
                runtime
                    .allocate_private_heap_block(
                        handle,
                        33_177_600,
                        RuntimeHeapInitialization::Zeroed,
                    )
                    .unwrap()
                    .is_none()
            },
            "large allocation owner or region exhausted",
        );
        assert_eq!(runtime.process_address_space_allocator(), &allocator_before);
        runtime
            .free_private_heap_block(handle, first.address)
            .unwrap();
        acknowledge_delta(&mut runtime.memory);
        let next = runtime
            .allocate_private_heap_block(handle, 33_177_600, RuntimeHeapInitialization::Zeroed)
            .unwrap()
            .unwrap();
        assert_eq!(next.id, RuntimeHeapAllocationId(first.id.0 + 1));
    }
    let mut runtime = running_runtime(GuestMemory::new(MemoryLimits::default()).unwrap());
    let handle = runtime.allocate_private_heap(0, 0).unwrap();
    let allocator_before = runtime.process_address_space_allocator().clone();
    assert_rejected_without_mutation(
        &mut runtime,
        handle,
        |runtime| {
            runtime
                .allocate_private_heap_block(handle, 0x7fff_fff8, RuntimeHeapInitialization::Zeroed)
                .unwrap()
                .is_none()
        },
        "large allocation has no contiguous guest address range",
    );
    assert_eq!(runtime.process_address_space_allocator(), &allocator_before);
    let first = runtime
        .allocate_private_heap_block(handle, 33_177_600, RuntimeHeapInitialization::Zeroed)
        .unwrap()
        .unwrap();
    assert_eq!(first.id, RuntimeHeapAllocationId(1));
}

fn assert_rejected_without_mutation<F>(
    runtime: &mut Runtime,
    handle: u32,
    operation: F,
    failure_context: &str,
) where
    F: FnOnce(&mut Runtime) -> bool,
{
    let heap_before = runtime.heap(handle).unwrap().live_allocations;
    let owner_before = runtime
        .heap_backing(handle)
        .unwrap()
        .map(|backing| backing.memory_owner);
    let large_count = runtime.large_heap_allocation_backings().len();
    let first_large_allocation = runtime
        .large_heap_allocation_backing()
        .map(|backing| backing.allocation);
    let last_error = runtime.current_last_error().unwrap();
    let memory_before = runtime.memory.stats();
    let reflection_before = runtime.memory.current_reflection().unwrap();
    assert!(operation(runtime), "{failure_context}");
    assert_eq!(runtime.heap(handle).unwrap().live_allocations, heap_before);
    assert_eq!(
        runtime
            .heap_backing(handle)
            .unwrap()
            .map(|backing| backing.memory_owner),
        owner_before
    );
    assert_eq!(runtime.current_last_error().unwrap(), last_error);
    assert!(runtime.heap_extension_backing(handle).unwrap().is_none());
    assert_eq!(runtime.large_heap_allocation_backings().len(), large_count);
    assert_eq!(
        runtime
            .large_heap_allocation_backing()
            .map(|backing| backing.allocation),
        first_large_allocation
    );
    assert_eq!(runtime.memory.stats(), memory_before);
    assert_eq!(
        runtime.memory.current_reflection().unwrap(),
        reflection_before
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

fn running_runtime(memory: GuestMemory) -> Runtime {
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
        memory,
    )
    .unwrap();
    runtime.start_initial_thread().unwrap();
    runtime
}

#[test]
fn lazy_heap_fault_rejects_missing_memory_owner_without_publication() {
    let memory = GuestMemory::new(MemoryLimits::default()).unwrap();
    let mut runtime = running_runtime(memory);
    let handle = runtime.allocate_private_heap(0x1000, 0).unwrap();
    let allocation = runtime
        .allocate_private_heap_block(
            handle,
            crate::RUNTIME_PRIVATE_HEAP_ARENA_STRIDE + 0x3000,
            RuntimeHeapInitialization::Zeroed,
        )
        .unwrap()
        .unwrap();
    let owner = runtime
        .large_heap_allocation_backing()
        .unwrap()
        .memory_owner;
    runtime
        .memory
        .release_region(owner, allocation.address)
        .unwrap();
    runtime.memory.destroy_owner(owner).unwrap();
    acknowledge_reflection(&mut runtime.memory);

    let before = runtime.memory.synchronization_state();
    let live_allocations = runtime.heap(handle).unwrap().live_allocations;
    let last_error = runtime.current_last_error().unwrap();
    assert!(
        runtime
            .materialize_private_heap_page(allocation.address, 6)
            .is_err()
    );
    assert_eq!(runtime.memory.synchronization_state(), before);
    assert!(!runtime.memory.has_pending_delta());
    assert_eq!(
        runtime.heap(handle).unwrap().live_allocations,
        live_allocations
    );
    assert_eq!(runtime.current_last_error().unwrap(), last_error);
    assert!(
        runtime
            .large_heap_allocation_backing()
            .unwrap()
            .materialized_pages
            .is_empty()
    );
}

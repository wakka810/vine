use vine_core::{
    ExecutionSession, GuestAddress, GuestMemory, LOGICAL_ENCODED_POINTER_NAMESPACE, MemoryLimits,
    MemoryOwnerKind, MemoryRegionKind, ProcessAddressAllocationPurpose, ProcessAddressSpaceLayout,
    RUNTIME_PRIVATE_HEAP_ALIGNMENT, RUNTIME_PRIVATE_HEAP_ARENA_BASE,
    RUNTIME_PRIVATE_HEAP_ARENA_STRIDE, RUNTIME_PRIVATE_HEAP_BACKING_LIMIT,
    RUNTIME_PRIVATE_HEAP_FREED_BYTE, RUNTIME_PRIVATE_HEAP_UNINITIALIZED_BYTE, RuntimeBootstrap,
    RuntimeClockState, RuntimeHeapAllocationId, RuntimeHeapInitialization, RuntimeProcessId,
    RuntimeProcessLifecycleState, RuntimeStartupInfo, RuntimeState, RuntimeThreadId,
    X86_STARTUP_INFO_W_BYTE_LENGTH,
};

const PROCESS_ID: u32 = 4096;
const THREAD_ID: u32 = 8192;
const REACHED_SAVE_COMPRESSION_BYTES: u32 = 7_786_068;
const REACHED_SCENARIO_BUFFER_BYTES: u32 = 61_219_868;

#[test]
fn heap_poisoning_can_be_disabled_without_skipping_zeroing_copy_or_validation() {
    let mut memory = GuestMemory::new(MemoryLimits::default()).unwrap();
    let mut runtime = running_runtime();
    runtime.set_heap_poisoning_enabled(false);
    let handle = runtime.allocate_private_heap(0, 0x1000, 0).unwrap().handle;
    let initial = ExecutionSession::new(&mut memory, &mut runtime)
        .allocate_private_heap_block(
            handle,
            32,
            RuntimeHeapInitialization::DeterministicUninitialized,
        )
        .unwrap();
    acknowledge_initial_reflection(&mut memory);
    memory.write(initial.address, &[0x5a; 32]).unwrap();
    acknowledge_delta(&mut memory);
    let unchanged = memory.stats();
    ExecutionSession::new(&mut memory, &mut runtime)
        .free_private_heap_block(handle, initial.address)
        .unwrap();
    assert_eq!(memory.stats(), unchanged);
    assert_eq!(memory.read(initial.address, 32).unwrap(), [0x5a; 32]);
    let reused = ExecutionSession::new(&mut memory, &mut runtime)
        .allocate_private_heap_block(
            handle,
            32,
            RuntimeHeapInitialization::DeterministicUninitialized,
        )
        .unwrap();
    assert_eq!(reused.address, initial.address);
    assert_eq!(memory.stats(), unchanged);
    assert_eq!(memory.read(reused.address, 32).unwrap(), [0x5a; 32]);

    let grown = ExecutionSession::new(&mut memory, &mut runtime)
        .reallocate_private_heap_block(
            handle,
            reused.address,
            64,
            RuntimeHeapInitialization::Zeroed,
        )
        .unwrap();
    acknowledge_delta(&mut memory);
    assert_eq!(memory.read(grown.address, 32).unwrap(), [0x5a; 32]);
    assert_eq!(
        memory.read(GuestAddress(grown.address.0 + 32), 32).unwrap(),
        [0; 32]
    );

    let backing = runtime.heap_backing(handle).unwrap().unwrap().clone();
    let page = vine_core::GuestRange::new(backing.range.start(), 4096).unwrap();
    memory
        .protect(
            backing.memory_owner,
            page,
            vine_core::PageProtection {
                read: true,
                write: false,
                execute: false,
            },
        )
        .unwrap();
    acknowledge_delta(&mut memory);
    let memory_before = memory.stats();
    let runtime_before = runtime.clone();
    assert!(
        ExecutionSession::new(&mut memory, &mut runtime)
            .free_private_heap_block(handle, grown.address)
            .is_err()
    );
    assert!(
        ExecutionSession::new(&mut memory, &mut runtime)
            .allocate_private_heap_block(
                handle,
                32,
                RuntimeHeapInitialization::DeterministicUninitialized
            )
            .is_err()
    );
    assert_eq!(memory.stats(), memory_before);
    assert_eq!(runtime, runtime_before);
    assert_eq!(memory.read(grown.address, 32).unwrap(), [0x5a; 32]);

    memory
        .protect(
            backing.memory_owner,
            page,
            vine_core::PageProtection {
                read: true,
                write: true,
                execute: false,
            },
        )
        .unwrap();
    acknowledge_delta(&mut memory);
    runtime.set_heap_poisoning_enabled(true);
    ExecutionSession::new(&mut memory, &mut runtime)
        .free_private_heap_block(handle, grown.address)
        .unwrap();
    acknowledge_delta(&mut memory);
    assert_eq!(
        memory.read(grown.address, 64).unwrap(),
        [RUNTIME_PRIVATE_HEAP_FREED_BYTE; 64]
    );
    let before = runtime.clone();
    assert!(
        ExecutionSession::new(&mut memory, &mut runtime)
            .free_private_heap_block(handle, grown.address)
            .is_err()
    );
    assert_eq!(runtime, before);
}

#[test]
fn heap_slot_allocator_ownership_preserves_runtime_serialization_shape() {
    let mut runtime = running_runtime();
    let initial = serde_json::to_value(&runtime).expect("serialize initial heap allocator view");
    assert_eq!(initial["nextPrivateHeapSlot"], serde_json::json!(1));
    assert!(initial["process"].get("nextPrivateHeapSlot").is_none());

    let heap = runtime
        .allocate_private_heap(0, 0x1000, 0)
        .expect("allocate first private heap")
        .clone();
    let allocated = serde_json::to_value(&runtime).expect("serialize allocated heap view");
    assert_eq!(heap.handle, 0x6801_0001);
    assert_eq!(allocated["nextPrivateHeapSlot"], serde_json::json!(2));
    let heap_key = heap.handle.to_string();
    assert_eq!(
        allocated["process"]["heaps"][&heap_key]["handle"],
        serde_json::json!(heap.handle)
    );
    assert!(allocated["process"].get("nextPrivateHeapSlot").is_none());
}

#[test]
fn growable_heap_uses_multiple_bounded_segments_and_tears_them_down_atomically() {
    let mut memory = GuestMemory::new(MemoryLimits {
        max_physical_pages: 12_288,
        ..MemoryLimits::default()
    })
    .unwrap();
    let mut runtime = running_runtime();
    let handle = runtime.allocate_private_heap(0, 0, 0).unwrap().handle;
    let first_length = RUNTIME_PRIVATE_HEAP_ARENA_STRIDE - 0x1000;
    let first = ExecutionSession::new(&mut memory, &mut runtime)
        .allocate_private_heap_block(
            handle,
            first_length,
            RuntimeHeapInitialization::DeterministicUninitialized,
        )
        .unwrap();
    let reflection = memory.pending_reflection().unwrap();
    memory
        .acknowledge_delta(reflection.invalidation.generation)
        .unwrap();
    assert_eq!(first.address, RUNTIME_PRIVATE_HEAP_ARENA_BASE);
    assert!(runtime.heap_extension_backing(handle).unwrap().is_none());

    let extension = ExecutionSession::new(&mut memory, &mut runtime)
        .allocate_private_heap_block(handle, first_length, RuntimeHeapInitialization::Zeroed)
        .unwrap();
    acknowledge_delta(&mut memory);
    let extension_slot_base = RUNTIME_PRIVATE_HEAP_ARENA_BASE.0
        + u32::from(RUNTIME_PRIVATE_HEAP_BACKING_LIMIT - 1) * RUNTIME_PRIVATE_HEAP_ARENA_STRIDE;
    assert_eq!(extension.address, GuestAddress(extension_slot_base));
    let extension_backing = runtime.heap_extension_backing(handle).unwrap().unwrap();
    let extension_allocation_id = extension_backing.address_allocation_id;
    assert_eq!(extension_backing.range.start(), extension.address);
    assert_eq!(extension_backing.committed_bytes, first_length);
    assert_eq!(runtime.heap(handle).unwrap().live_allocations, 2);

    let third = ExecutionSession::new(&mut memory, &mut runtime)
        .allocate_private_heap_block(handle, 0x406000, RuntimeHeapInitialization::Zeroed)
        .unwrap();
    acknowledge_delta(&mut memory);
    let third_slot = RUNTIME_PRIVATE_HEAP_BACKING_LIMIT - 1;
    let third_slot_base = RUNTIME_PRIVATE_HEAP_ARENA_BASE.0
        + u32::from(third_slot - 1) * RUNTIME_PRIVATE_HEAP_ARENA_STRIDE;
    assert_eq!(third.address, GuestAddress(third_slot_base));
    assert_eq!(
        runtime
            .private_heap_allocation(handle, third.address)
            .unwrap(),
        third
    );
    let third_allocation_id = runtime
        .process_address_space_allocator()
        .allocation_for_purpose(&ProcessAddressAllocationPurpose::PrivateHeapSlot {
            slot: third_slot,
        })
        .unwrap()
        .id;
    assert_eq!(runtime.heap(handle).unwrap().live_allocations, 3);

    let freed = ExecutionSession::new(&mut memory, &mut runtime)
        .free_private_heap_block(handle, third.address)
        .unwrap();
    acknowledge_delta(&mut memory);
    assert_eq!(freed, third);
    let reused = ExecutionSession::new(&mut memory, &mut runtime)
        .allocate_private_heap_block(handle, 0x406000, RuntimeHeapInitialization::Zeroed)
        .unwrap();
    acknowledge_delta(&mut memory);
    assert_eq!(reused.address, third.address);

    let primary_allocation_id = runtime
        .heap_backing(handle)
        .unwrap()
        .unwrap()
        .address_allocation_id;
    let teardown = ExecutionSession::new(&mut memory, &mut runtime)
        .destroy_private_heap(handle)
        .unwrap();
    acknowledge_delta(&mut memory);
    assert_eq!(
        teardown.address_allocation_ids,
        vec![
            primary_allocation_id,
            third_allocation_id,
            extension_allocation_id
        ]
    );
    assert_eq!(teardown.allocations_discarded, 3);
    assert_eq!(
        teardown.reserved_bytes_released,
        u64::from(RUNTIME_PRIVATE_HEAP_ARENA_STRIDE) * 3
    );
    assert_eq!(memory.stats().owners, 0);
    assert_eq!(memory.stats().regions, 0);
}

#[test]
fn growable_heap_keeps_save_compression_capacity_while_the_large_slot_is_live() {
    let mut memory = GuestMemory::new(MemoryLimits {
        max_physical_pages: 8_192,
        ..MemoryLimits::default()
    })
    .unwrap();
    let mut runtime = running_runtime();
    let handle = runtime.allocate_private_heap(0, 0, 0).unwrap().handle;
    for _ in 1..32 {
        runtime.allocate_private_heap(0, 0, 0).unwrap();
    }

    ExecutionSession::new(&mut memory, &mut runtime)
        .allocate_private_heap_block(
            handle,
            RUNTIME_PRIVATE_HEAP_ARENA_STRIDE,
            RuntimeHeapInitialization::DeterministicUninitialized,
        )
        .unwrap();
    acknowledge_initial_reflection(&mut memory);
    let scenario_buffer = ExecutionSession::new(&mut memory, &mut runtime)
        .allocate_private_heap_block(
            handle,
            REACHED_SCENARIO_BUFFER_BYTES,
            RuntimeHeapInitialization::DeterministicUninitialized,
        )
        .unwrap();
    assert_eq!(
        scenario_buffer.address,
        vine_core::RUNTIME_PRIVATE_HEAP_LARGE_SLOT_BASE
    );
    acknowledge_delta(&mut memory);

    let save_compression = ExecutionSession::new(&mut memory, &mut runtime)
        .allocate_private_heap_block(
            handle,
            REACHED_SAVE_COMPRESSION_BYTES,
            RuntimeHeapInitialization::DeterministicUninitialized,
        )
        .unwrap();
    let final_ordinary_slot = RUNTIME_PRIVATE_HEAP_ARENA_BASE.0
        + u32::from(RUNTIME_PRIVATE_HEAP_BACKING_LIMIT - 1) * RUNTIME_PRIVATE_HEAP_ARENA_STRIDE;
    assert_eq!(save_compression.address, GuestAddress(final_ordinary_slot));
    assert_eq!(runtime.heap(handle).unwrap().live_allocations, 3);
}

#[test]
fn extension_slot_exhaustion_and_fixed_heap_growth_rejection_are_atomic() {
    let mut memory = GuestMemory::new(MemoryLimits {
        max_physical_pages: 8_192,
        ..MemoryLimits::default()
    })
    .unwrap();
    let mut runtime = running_runtime();
    let handle = runtime.allocate_private_heap(0, 0, 0).unwrap().handle;
    for _ in 1..RUNTIME_PRIVATE_HEAP_BACKING_LIMIT {
        runtime.allocate_private_heap(0, 0, 0).unwrap();
    }
    ExecutionSession::new(&mut memory, &mut runtime)
        .allocate_private_heap_block(
            handle,
            RUNTIME_PRIVATE_HEAP_ARENA_STRIDE,
            RuntimeHeapInitialization::DeterministicUninitialized,
        )
        .unwrap();
    acknowledge_initial_reflection(&mut memory);
    let runtime_before_exhaustion = runtime.clone();
    let memory_before_exhaustion = memory.stats();
    let error = ExecutionSession::new(&mut memory, &mut runtime)
        .allocate_private_heap_block(handle, 8, RuntimeHeapInitialization::Zeroed)
        .unwrap_err();
    assert_eq!(error.code, "heap-extension-backing-slot-exhausted");
    assert_eq!(runtime, runtime_before_exhaustion);
    assert_eq!(memory.stats(), memory_before_exhaustion);

    let mut fixed_memory = GuestMemory::new(MemoryLimits {
        max_physical_pages: 16,
        ..MemoryLimits::default()
    })
    .unwrap();
    let mut fixed_runtime = running_runtime();
    let fixed_handle = fixed_runtime
        .allocate_private_heap(0, 0, 0x1000)
        .unwrap()
        .handle;
    ExecutionSession::new(&mut fixed_memory, &mut fixed_runtime)
        .allocate_private_heap_block(
            fixed_handle,
            0x1000,
            RuntimeHeapInitialization::DeterministicUninitialized,
        )
        .unwrap();
    acknowledge_initial_reflection(&mut fixed_memory);
    let runtime_before_fixed_growth = fixed_runtime.clone();
    let memory_before_fixed_growth = fixed_memory.stats();
    let error = ExecutionSession::new(&mut fixed_memory, &mut fixed_runtime)
        .allocate_private_heap_block(fixed_handle, 8, RuntimeHeapInitialization::Zeroed)
        .unwrap_err();
    assert_eq!(error.code, "heap-backing-capacity-exceeded");
    assert_eq!(fixed_runtime, runtime_before_fixed_growth);
    assert_eq!(fixed_memory.stats(), memory_before_fixed_growth);
}

#[test]
fn private_heap_backing_is_atomic_reusable_and_process_owned() {
    let mut memory = GuestMemory::new(MemoryLimits {
        max_physical_pages: 16,
        ..MemoryLimits::default()
    })
    .unwrap();
    let mut runtime = running_runtime();
    let handle = runtime.allocate_private_heap(0, 0x1000, 0).unwrap().handle;
    assert_eq!(handle, 0x6801_0001);

    let collision_owner = memory.create_owner(MemoryOwnerKind::Runtime).unwrap();
    memory
        .reserve_fixed(
            collision_owner,
            MemoryRegionKind::Runtime,
            RUNTIME_PRIVATE_HEAP_ARENA_BASE,
            0x1000,
        )
        .unwrap();
    let runtime_before_collision = runtime.clone();
    let memory_before_collision = memory.stats();
    let error = ExecutionSession::new(&mut memory, &mut runtime)
        .allocate_private_heap_block(handle, 0x214, RuntimeHeapInitialization::Zeroed)
        .unwrap_err();
    assert_eq!(error.code, "memory-region-overlap");
    assert_eq!(runtime, runtime_before_collision);
    assert_eq!(memory.stats(), memory_before_collision);
    memory
        .release_region(collision_owner, RUNTIME_PRIVATE_HEAP_ARENA_BASE)
        .unwrap();
    memory.destroy_owner(collision_owner).unwrap();

    let zeroed = ExecutionSession::new(&mut memory, &mut runtime)
        .allocate_private_heap_block(handle, 0x214, RuntimeHeapInitialization::Zeroed)
        .unwrap();
    assert_eq!(zeroed.id, RuntimeHeapAllocationId(1));
    assert_eq!(zeroed.address, RUNTIME_PRIVATE_HEAP_ARENA_BASE);
    assert_eq!(zeroed.address.0 % RUNTIME_PRIVATE_HEAP_ALIGNMENT, 0);
    assert_eq!(zeroed.reserved_byte_length, 0x218);
    let backing = runtime.heap_backing(handle).unwrap().unwrap();
    let backing_allocation_id = backing.address_allocation_id;
    let address_allocation = runtime
        .process_address_space_allocator()
        .allocation(backing.address_allocation_id)
        .unwrap();
    assert_eq!(
        address_allocation.purpose,
        ProcessAddressAllocationPurpose::PrivateHeapSlot { slot: 1 }
    );
    assert!(
        address_allocation
            .range
            .contains(vine_core::RuntimeAddressRange {
                base: backing.range.start(),
                byte_length: backing.range.byte_length(),
            })
            .unwrap()
    );
    assert_eq!(backing.range.start(), RUNTIME_PRIVATE_HEAP_ARENA_BASE);
    assert_eq!(
        backing.range.byte_length(),
        u64::from(RUNTIME_PRIVATE_HEAP_ARENA_STRIDE)
    );
    assert_eq!(backing.committed_bytes, 0x1000);
    assert_eq!(backing.high_water_mark, 0x218);
    assert_eq!(runtime.heap(handle).unwrap().live_allocations, 1);
    let reflection = memory.pending_reflection().unwrap();
    assert_eq!(reflection.pages.len(), 1);
    assert_eq!(reflection.pages[0].owner, backing.memory_owner);
    memory
        .acknowledge_delta(reflection.invalidation.generation)
        .unwrap();
    assert_eq!(
        memory
            .read(zeroed.address, zeroed.reserved_byte_length as usize)
            .unwrap(),
        vec![0; zeroed.reserved_byte_length as usize]
    );

    let uninitialized = ExecutionSession::new(&mut memory, &mut runtime)
        .allocate_private_heap_block(
            handle,
            0x800,
            RuntimeHeapInitialization::DeterministicUninitialized,
        )
        .unwrap();
    assert_eq!(uninitialized.id, RuntimeHeapAllocationId(2));
    assert_eq!(
        uninitialized.address,
        GuestAddress(RUNTIME_PRIVATE_HEAP_ARENA_BASE.0 + 0x218)
    );
    acknowledge_delta(&mut memory);
    assert_eq!(
        memory
            .read(
                uninitialized.address,
                uninitialized.reserved_byte_length as usize,
            )
            .unwrap(),
        vec![RUNTIME_PRIVATE_HEAP_UNINITIALIZED_BYTE; uninitialized.reserved_byte_length as usize]
    );

    let freed = ExecutionSession::new(&mut memory, &mut runtime)
        .free_private_heap_block(handle, zeroed.address)
        .unwrap();
    acknowledge_delta(&mut memory);
    assert_eq!(freed, zeroed);
    assert_eq!(runtime.heap(handle).unwrap().live_allocations, 1);
    assert_eq!(
        memory
            .read(zeroed.address, zeroed.reserved_byte_length as usize)
            .unwrap(),
        vec![RUNTIME_PRIVATE_HEAP_FREED_BYTE; zeroed.reserved_byte_length as usize]
    );
    let state_after_free = runtime.clone();
    assert_eq!(
        ExecutionSession::new(&mut memory, &mut runtime)
            .free_private_heap_block(handle, zeroed.address)
            .unwrap_err()
            .code,
        "heap-allocation-double-free"
    );
    assert_eq!(
        ExecutionSession::new(&mut memory, &mut runtime)
            .free_private_heap_block(handle, GuestAddress(zeroed.address.0 + 4))
            .unwrap_err()
            .code,
        "heap-allocation-pointer-invalid"
    );
    assert_eq!(runtime, state_after_free);

    let reused = ExecutionSession::new(&mut memory, &mut runtime)
        .allocate_private_heap_block(handle, 0x100, RuntimeHeapInitialization::Zeroed)
        .unwrap();
    acknowledge_delta(&mut memory);
    assert_eq!(reused.id, RuntimeHeapAllocationId(3));
    assert_eq!(reused.address, zeroed.address);
    assert_eq!(
        memory
            .read(reused.address, reused.reserved_byte_length as usize)
            .unwrap(),
        vec![0; reused.reserved_byte_length as usize]
    );
    assert_eq!(
        ExecutionSession::new(&mut memory, &mut runtime)
            .free_private_heap_block(handle, GuestAddress(reused.address.0 + 4))
            .unwrap_err()
            .code,
        "heap-allocation-pointer-invalid"
    );

    let teardown = ExecutionSession::new(&mut memory, &mut runtime)
        .destroy_private_heap(handle)
        .unwrap();
    acknowledge_delta(&mut memory);
    assert_eq!(teardown.handles, vec![handle]);
    assert_eq!(teardown.address_allocation_ids, vec![backing_allocation_id]);
    assert_eq!(teardown.allocations_discarded, 2);
    assert_eq!(
        teardown.reserved_bytes_released,
        u64::from(RUNTIME_PRIVATE_HEAP_ARENA_STRIDE)
    );
    assert_eq!(teardown.committed_bytes_released, 0x1000);
    assert_eq!(
        runtime.heap(handle).unwrap_err().code,
        "runtime-handle-stale"
    );
    assert_eq!(memory.stats().owners, 0);
    assert_eq!(memory.stats().regions, 0);
    assert_eq!(memory.stats().committed_pages, 0);

    let process_heap = runtime.allocate_private_heap(0, 0, 0).unwrap().handle;
    let process_allocation = ExecutionSession::new(&mut memory, &mut runtime)
        .allocate_private_heap_block(
            process_heap,
            0x80,
            RuntimeHeapInitialization::DeterministicUninitialized,
        )
        .unwrap();
    assert_eq!(process_heap, 0x6801_0002);
    assert_eq!(
        process_allocation.address,
        GuestAddress(RUNTIME_PRIVATE_HEAP_ARENA_BASE.0 + RUNTIME_PRIVATE_HEAP_ARENA_STRIDE)
    );
    let metadata_only_heap = runtime.allocate_private_heap(0, 0x1000, 0).unwrap().handle;
    assert_eq!(metadata_only_heap, 0x6801_0003);
    assert!(runtime.heap_backing(metadata_only_heap).unwrap().is_none());
    let before_direct_termination = runtime.clone();
    assert_eq!(
        runtime.terminate_current_process(7).unwrap_err().code,
        "process-heaps-live-at-direct-termination"
    );
    assert_eq!(runtime, before_direct_termination);
    runtime.request_current_process_exit(7).unwrap();
    assert_eq!(
        runtime.complete_current_process_exit().unwrap_err().code,
        "process-heaps-live-at-exit-completion"
    );
    let process_teardown = ExecutionSession::new(&mut memory, &mut runtime)
        .teardown_process_heaps()
        .unwrap();
    acknowledge_delta(&mut memory);
    assert_eq!(
        process_teardown.handles,
        vec![process_heap, metadata_only_heap]
    );
    assert_eq!(process_teardown.allocations_discarded, 1);
    runtime.complete_current_process_exit().unwrap();
    assert_eq!(
        runtime.process().lifecycle(),
        RuntimeProcessLifecycleState::Exited { exit_code: 7 }
    );
    assert!(runtime.heaps().is_empty());
    assert_eq!(memory.stats().owners, 0);
    assert_eq!(memory.stats().regions, 0);
    assert_eq!(memory.stats().physical_pages, 0);
    assert_eq!(memory.stats().generation, 13);
    assert_eq!(memory.stats().acknowledged_generation, 13);
}

fn acknowledge_delta(memory: &mut GuestMemory) {
    let delta = memory.pending_delta().unwrap();
    memory.acknowledge_delta(delta.generation).unwrap();
}

fn acknowledge_initial_reflection(memory: &mut GuestMemory) {
    let reflection = memory.pending_reflection().unwrap();
    memory
        .acknowledge_delta(reflection.invalidation.generation)
        .unwrap();
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
            performance_frequency: 10_000_000,
        },
        startup_info: RuntimeStartupInfo {
            profile_id: "runtime-heap-backing-test".to_owned(),
            bytes: vec![0; X86_STARTUP_INFO_W_BYTE_LENGTH],
        },
        process_image_path_a: vine_core::RuntimeProcessImagePathA {
            source_authority: "runtime-heap-backing-fixture".to_owned(),
            bytes_with_nul: b"C:\\Vine\\fixture.exe\0".to_vec(),
            encoding: vine_core::RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING.to_owned(),
        },
        initial_current_directory_a: vine_core::RuntimeCurrentDirectoryA {
            source_authority: "runtime-heap-backing-process-bootstrap".to_owned(),
            bytes_with_nul: b"C:\\Vine\0".to_vec(),
            encoding: vine_core::RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING.to_owned(),
        },
        encoded_pointer_namespace: LOGICAL_ENCODED_POINTER_NAMESPACE.to_owned(),
        encoded_pointer_cookie: 0xec01_9d3b,
    })
    .unwrap();
    runtime.start_initial_thread().unwrap();
    runtime
}

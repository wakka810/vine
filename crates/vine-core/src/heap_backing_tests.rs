use crate::runtime::{RuntimeHeapInitialization, X86_STARTUP_INFO_W_BYTE_LENGTH};
use crate::{
    GuestAddress, GuestMemory, GuestRange, MemoryLimits, PageProtection, Runtime, RuntimeBootstrap,
    RuntimeClockState, RuntimeProcessId, RuntimeStartupInfo, RuntimeThreadId,
};

const PROCESS_ID: u32 = 4096;
const THREAD_ID: u32 = 8192;

#[test]
fn heap_destroy_failure_preserves_mappings_handle_and_bytes() {
    let mut runtime = running_runtime(GuestMemory::new(MemoryLimits::default()).unwrap());
    let heap = runtime.allocate_private_heap(0, 0).unwrap();
    let allocation = runtime
        .allocate_private_heap_block(heap, 32, RuntimeHeapInitialization::Zeroed)
        .unwrap()
        .unwrap();
    let owner = runtime.heap_backing(heap).unwrap().unwrap().memory_owner;
    // An unexpected second region prevents retiring the owner after the known
    // heap region has been removed in the transaction.
    let extra = GuestAddress(0x6f00_0000);
    runtime
        .memory
        .reserve_fixed(
            owner,
            crate::memory::MemoryRegionKind::Anonymous,
            extra,
            4096,
        )
        .unwrap();
    acknowledge(&mut runtime.memory);
    runtime
        .memory
        .write(allocation.address, &[0xa5; 32])
        .unwrap();
    acknowledge(&mut runtime.memory);
    let before = runtime.memory.stats();
    let addresses = runtime.process_address_space_allocator().clone();
    assert!(runtime.destroy_private_heap(heap).is_err());
    assert_eq!(runtime.memory.stats(), before);
    assert_eq!(runtime.process_address_space_allocator(), &addresses);
    assert_eq!(
        runtime.memory.read(allocation.address, 32).unwrap(),
        [0xa5; 32]
    );
    assert!(runtime.heap(heap).is_ok());
    runtime.memory.release_region(owner, extra).unwrap();
    acknowledge(&mut runtime.memory);
    assert!(runtime.destroy_private_heap(heap).unwrap());
}

#[test]
fn heap_destroy_releases_primary_extension_and_large_storage_without_touching_other_heaps() {
    let mut runtime = running_runtime(GuestMemory::new(MemoryLimits::default()).unwrap());
    let heap = runtime.allocate_private_heap(0, 0).unwrap();
    let other = runtime.process_heap().unwrap();
    let keep = runtime
        .allocate_private_heap_block(other, 16, RuntimeHeapInitialization::Zeroed)
        .unwrap()
        .unwrap();
    let before = runtime.memory.stats();
    let mut addresses = Vec::new();
    for size in [0x0100_0000, 32, 0x0101_0000] {
        let allocation = runtime
            .allocate_private_heap_block(heap, size, RuntimeHeapInitialization::Zeroed)
            .unwrap()
            .unwrap();
        addresses.push(allocation.address);
    }
    acknowledge(&mut runtime.memory);
    runtime.memory.write(keep.address, &[0x5a; 16]).unwrap();
    acknowledge(&mut runtime.memory);
    assert!(runtime.destroy_private_heap(heap).unwrap());
    acknowledge(&mut runtime.memory);
    for address in addresses {
        assert!(runtime.memory.read(address, 1).is_err());
        assert!(
            runtime
                .private_heap_allocation_option(heap, address)
                .is_err()
        );
    }
    assert_eq!(runtime.memory.read(keep.address, 16).unwrap(), [0x5a; 16]);
    assert_eq!(runtime.memory.stats().physical_pages, before.physical_pages);
    assert!(!runtime.destroy_private_heap(heap).unwrap());
    assert!(!runtime.destroy_private_heap(other).unwrap());
    let mut retired = vec![heap];
    for _ in 0..40 {
        let next = runtime.allocate_private_heap(0, 0).unwrap();
        assert_ne!(next, 0);
        assert!(!retired.contains(&next));
        let allocation = runtime
            .allocate_private_heap_block(next, 8, RuntimeHeapInitialization::Zeroed)
            .unwrap()
            .unwrap();
        acknowledge(&mut runtime.memory);
        assert_eq!(runtime.memory.read(allocation.address, 8).unwrap(), [0; 8]);
        for old in &retired {
            assert!(!runtime.destroy_private_heap(*old).unwrap());
        }
        assert!(runtime.destroy_private_heap(next).unwrap());
        acknowledge(&mut runtime.memory);
        retired.push(next);
    }
}

#[test]
fn realloc_preserves_other_blocks_and_leaves_coalesced_space_usable() {
    let mut runtime = running_runtime(GuestMemory::new(MemoryLimits::default()).unwrap());
    let heap = runtime.allocate_private_heap(0, 4096).unwrap();
    let first = runtime
        .allocate_private_heap_block(heap, 32, RuntimeHeapInitialization::Zeroed)
        .unwrap()
        .unwrap();
    let source = runtime
        .allocate_private_heap_block(heap, 32, RuntimeHeapInitialization::Zeroed)
        .unwrap()
        .unwrap();
    let last = runtime
        .allocate_private_heap_block(heap, 4032, RuntimeHeapInitialization::Zeroed)
        .unwrap()
        .unwrap();
    acknowledge(&mut runtime.memory);
    runtime.memory.write(source.address, &[0x5a; 32]).unwrap();
    runtime.memory.write(last.address, &[0x77; 32]).unwrap();
    acknowledge(&mut runtime.memory);
    runtime
        .free_private_heap_block(heap, first.address)
        .unwrap();
    acknowledge(&mut runtime.memory);
    let target = runtime
        .reallocate_private_heap_block(heap, source.address, 16, RuntimeHeapInitialization::Zeroed)
        .unwrap()
        .unwrap();
    acknowledge(&mut runtime.memory);
    assert_eq!(runtime.memory.read(target.address, 16).unwrap(), [0x5a; 16]);
    assert_eq!(runtime.memory.read(last.address, 32).unwrap(), [0x77; 32]);
    let remaining = runtime
        .allocate_private_heap_block(heap, 48, RuntimeHeapInitialization::Zeroed)
        .unwrap()
        .unwrap();
    acknowledge(&mut runtime.memory);
    assert_eq!(runtime.memory.read(remaining.address, 48).unwrap(), [0; 48]);
    assert_eq!(runtime.memory.read(target.address, 16).unwrap(), [0x5a; 16]);
    assert_eq!(runtime.memory.read(last.address, 32).unwrap(), [0x77; 32]);
}

#[test]
fn rejected_realloc_preserves_source_bytes_and_allows_retry() {
    let mut runtime = running_runtime(GuestMemory::new(MemoryLimits::default()).unwrap());
    let heap = runtime.allocate_private_heap(0, 0).unwrap();
    let source = runtime
        .allocate_private_heap_block(heap, 4096, RuntimeHeapInitialization::Zeroed)
        .unwrap()
        .unwrap();
    acknowledge(&mut runtime.memory);
    runtime.memory.write(source.address, &[0x5a; 4096]).unwrap();
    acknowledge(&mut runtime.memory);
    let owner = runtime.heap_backing(heap).unwrap().unwrap().memory_owner;
    let page = GuestRange::new(source.address, 4096).unwrap();
    runtime
        .memory
        .protect(owner, page, protection(false))
        .unwrap();
    acknowledge(&mut runtime.memory);
    let before = runtime.memory.current_reflection().unwrap();
    assert!(
        runtime
            .reallocate_private_heap_block(
                heap,
                source.address,
                8192,
                RuntimeHeapInitialization::Zeroed
            )
            .is_err()
    );
    assert_eq!(runtime.memory.current_reflection().unwrap(), before);
    assert_eq!(
        runtime.memory.read(source.address, 4096).unwrap(),
        [0x5a; 4096]
    );
    runtime
        .memory
        .protect(owner, page, protection(true))
        .unwrap();
    acknowledge(&mut runtime.memory);
    let grown = runtime
        .reallocate_private_heap_block(
            heap,
            source.address,
            8192,
            RuntimeHeapInitialization::Zeroed,
        )
        .unwrap()
        .unwrap();
    acknowledge(&mut runtime.memory);
    assert_eq!(
        runtime.memory.read(grown.address, 4096).unwrap(),
        [0x5a; 4096]
    );
    assert_eq!(
        runtime
            .memory
            .read(GuestAddress(grown.address.0 + 4096), 4096)
            .unwrap(),
        [0; 4096]
    );
}

#[test]
fn heap_preserves_zeroing_copy_and_protection_checks() {
    let mut runtime = running_runtime(GuestMemory::new(MemoryLimits::default()).unwrap());
    let heap = runtime.allocate_private_heap(0x1000, 0).unwrap();
    let source = runtime
        .allocate_private_heap_block(heap, 32, RuntimeHeapInitialization::Uninitialized)
        .unwrap()
        .unwrap();
    acknowledge(&mut runtime.memory);
    runtime.memory.write(source.address, &[0x5a; 32]).unwrap();
    acknowledge(&mut runtime.memory);
    let grown = runtime
        .reallocate_private_heap_block(heap, source.address, 64, RuntimeHeapInitialization::Zeroed)
        .unwrap()
        .unwrap();
    acknowledge(&mut runtime.memory);
    assert_eq!(runtime.memory.read(grown.address, 32).unwrap(), [0x5a; 32]);
    assert_eq!(
        runtime
            .memory
            .read(GuestAddress(grown.address.0 + 32), 32)
            .unwrap(),
        [0; 32]
    );
    let owner = runtime.heap_backing(heap).unwrap().unwrap().memory_owner;
    let page = GuestRange::new(GuestAddress(grown.address.0 & !0xfff), 4096).unwrap();
    acknowledge(&mut runtime.memory);
    runtime
        .memory
        .protect(owner, page, protection(false))
        .unwrap();
    acknowledge(&mut runtime.memory);
    let before = runtime.memory.current_reflection().unwrap();
    assert!(
        runtime
            .free_private_heap_block(heap, grown.address)
            .is_err()
    );
    assert!(
        runtime
            .allocate_private_heap_block(heap, 32, RuntimeHeapInitialization::Uninitialized)
            .is_err()
    );
    assert_eq!(runtime.memory.current_reflection().unwrap(), before);
    runtime
        .memory
        .protect(owner, page, protection(true))
        .unwrap();
    acknowledge(&mut runtime.memory);
    runtime
        .free_private_heap_block(heap, grown.address)
        .unwrap();
    acknowledge(&mut runtime.memory);
    assert!(
        runtime
            .free_private_heap_block(heap, grown.address)
            .is_err()
    );
    let next = runtime
        .allocate_private_heap_block(heap, 64, RuntimeHeapInitialization::Zeroed)
        .unwrap()
        .unwrap();
    acknowledge(&mut runtime.memory);
    assert_eq!(runtime.memory.read(next.address, 64).unwrap(), [0; 64]);
}

#[test]
fn uninitialized_alloc_free_and_reuse_preserve_bytes_without_memory_deltas() {
    let mut runtime = running_runtime(GuestMemory::new(MemoryLimits::default()).unwrap());
    let heap = runtime.allocate_private_heap(4096, 0).unwrap();
    let seed = runtime
        .allocate_private_heap_block(heap, 32, RuntimeHeapInitialization::Uninitialized)
        .unwrap()
        .unwrap();
    acknowledge(&mut runtime.memory);
    assert_eq!(runtime.memory.read(seed.address, 4096).unwrap(), [0; 4096]);
    runtime.memory.write(seed.address, &[0x5a; 4096]).unwrap();
    acknowledge(&mut runtime.memory);
    let generation = runtime.memory.stats().generation;
    let neighbor = runtime
        .allocate_private_heap_block(heap, 32, RuntimeHeapInitialization::Uninitialized)
        .unwrap()
        .unwrap();
    assert!(!runtime.memory.has_pending_delta());
    assert_eq!(
        runtime.memory.read(neighbor.address, 32).unwrap(),
        [0x5a; 32]
    );
    runtime.free_private_heap_block(heap, seed.address).unwrap();
    assert!(!runtime.memory.has_pending_delta());
    assert_eq!(runtime.memory.read(seed.address, 32).unwrap(), [0x5a; 32]);
    let reused = runtime
        .allocate_private_heap_block(heap, 32, RuntimeHeapInitialization::Uninitialized)
        .unwrap()
        .unwrap();
    assert_eq!(reused.address, seed.address);
    assert_ne!(reused.id, seed.id);
    assert_eq!(runtime.memory.stats().generation, generation);
    assert!(!runtime.memory.has_pending_delta());
    assert_eq!(
        runtime.memory.read(seed.address, 4096).unwrap(),
        [0x5a; 4096]
    );
}

#[test]
fn growable_heap_allocations_remain_disjoint_across_segments_and_reuse() {
    let mut runtime = running_runtime(
        GuestMemory::new(MemoryLimits {
            max_physical_pages: 12_288,
            ..MemoryLimits::default()
        })
        .unwrap(),
    );
    let heap = runtime.allocate_private_heap(0, 0).unwrap();
    let sizes = [16 * 1024 * 1024 - 4096, 16 * 1024 * 1024 - 4096, 0x406000];
    let blocks = sizes.map(|size| {
        runtime
            .allocate_private_heap_block(heap, size, RuntimeHeapInitialization::Zeroed)
            .unwrap()
            .unwrap()
    });
    acknowledge(&mut runtime.memory);
    for (index, block) in blocks.iter().enumerate() {
        runtime
            .memory
            .write(block.address, &[index as u8 + 1; 32])
            .unwrap();
    }
    acknowledge(&mut runtime.memory);
    for (index, block) in blocks.iter().enumerate() {
        assert_eq!(
            runtime.memory.read(block.address, 32).unwrap(),
            [index as u8 + 1; 32]
        );
    }
    runtime
        .free_private_heap_block(heap, blocks[2].address)
        .unwrap();
    acknowledge(&mut runtime.memory);
    let next = runtime
        .allocate_private_heap_block(heap, sizes[2], RuntimeHeapInitialization::Zeroed)
        .unwrap()
        .unwrap();
    acknowledge(&mut runtime.memory);
    assert_eq!(runtime.memory.read(next.address, 32).unwrap(), [0; 32]);
    assert_eq!(runtime.memory.read(blocks[0].address, 32).unwrap(), [1; 32]);
    assert_eq!(runtime.memory.read(blocks[1].address, 32).unwrap(), [2; 32]);
}

#[test]
fn a_live_large_allocation_keeps_ordinary_growth_available() {
    let mut runtime = running_runtime(
        GuestMemory::new(MemoryLimits {
            max_physical_pages: 8192,
            ..MemoryLimits::default()
        })
        .unwrap(),
    );
    let heap = runtime.allocate_private_heap(0, 0).unwrap();
    for _ in 1..32 {
        runtime.allocate_private_heap(0, 0).unwrap();
    }
    runtime
        .allocate_private_heap_block(heap, 16 * 1024 * 1024, RuntimeHeapInitialization::Zeroed)
        .unwrap()
        .unwrap();
    acknowledge(&mut runtime.memory);
    let large = runtime
        .allocate_private_heap_block(heap, 64 * 1024 * 1024, RuntimeHeapInitialization::Zeroed)
        .unwrap()
        .unwrap();
    acknowledge(&mut runtime.memory);
    let ordinary = runtime
        .allocate_private_heap_block(heap, 8 * 1024 * 1024, RuntimeHeapInitialization::Zeroed)
        .unwrap()
        .unwrap();
    acknowledge(&mut runtime.memory);
    runtime.service_page_fault(large.address, 6).unwrap();
    acknowledge(&mut runtime.memory);
    runtime.memory.write(large.address, &[0x77; 32]).unwrap();
    runtime.memory.write(ordinary.address, &[0x5a; 32]).unwrap();
    acknowledge(&mut runtime.memory);
    assert_eq!(runtime.memory.read(large.address, 32).unwrap(), [0x77; 32]);
    assert_eq!(
        runtime.memory.read(ordinary.address, 32).unwrap(),
        [0x5a; 32]
    );
}

#[test]
fn fixed_heap_exhaustion_preserves_live_bytes_and_can_recover_after_free() {
    let mut runtime = running_runtime(GuestMemory::new(MemoryLimits::default()).unwrap());
    let heap = runtime.allocate_private_heap(0, 4096).unwrap();
    let block = runtime
        .allocate_private_heap_block(heap, 4096, RuntimeHeapInitialization::Zeroed)
        .unwrap()
        .unwrap();
    acknowledge(&mut runtime.memory);
    runtime.memory.write(block.address, &[0x5a; 32]).unwrap();
    acknowledge(&mut runtime.memory);
    let before = runtime.memory.current_reflection().unwrap();
    assert!(
        runtime
            .allocate_private_heap_block(heap, 8, RuntimeHeapInitialization::Zeroed)
            .unwrap()
            .is_none()
    );
    assert_eq!(runtime.memory.current_reflection().unwrap(), before);
    assert_eq!(runtime.memory.read(block.address, 32).unwrap(), [0x5a; 32]);
    runtime
        .free_private_heap_block(heap, block.address)
        .unwrap();
    acknowledge(&mut runtime.memory);
    let next = runtime
        .allocate_private_heap_block(heap, 4096, RuntimeHeapInitialization::Zeroed)
        .unwrap()
        .unwrap();
    acknowledge(&mut runtime.memory);
    assert_eq!(runtime.memory.read(next.address, 32).unwrap(), [0; 32]);
}

#[test]
fn heap_rejects_interior_double_and_foreign_frees_without_changing_live_bytes() {
    let mut runtime = running_runtime(GuestMemory::new(MemoryLimits::default()).unwrap());
    let heap = runtime.allocate_private_heap(0x1000, 0).unwrap();
    let other = runtime.allocate_private_heap(0x1000, 0).unwrap();
    let block = runtime
        .allocate_private_heap_block(heap, 0x214, RuntimeHeapInitialization::Zeroed)
        .unwrap()
        .unwrap();
    let survivor = runtime
        .allocate_private_heap_block(heap, 0x800, RuntimeHeapInitialization::Zeroed)
        .unwrap()
        .unwrap();
    acknowledge(&mut runtime.memory);
    assert!(block.address.0.is_multiple_of(8));
    assert_eq!(
        runtime.memory.read(block.address, 0x214).unwrap(),
        vec![0; 0x214]
    );
    runtime.memory.write(survivor.address, &[0x5a; 32]).unwrap();
    acknowledge(&mut runtime.memory);
    let before = runtime.memory.current_reflection().unwrap();
    assert!(
        runtime
            .free_private_heap_block(heap, GuestAddress(block.address.0 + 4))
            .is_err()
    );
    assert!(
        runtime
            .free_private_heap_block(other, block.address)
            .is_err()
    );
    assert_eq!(runtime.memory.current_reflection().unwrap(), before);
    runtime
        .free_private_heap_block(heap, block.address)
        .unwrap();
    acknowledge(&mut runtime.memory);
    let before = runtime.memory.current_reflection().unwrap();
    assert!(
        runtime
            .free_private_heap_block(heap, block.address)
            .is_err()
    );
    assert_eq!(runtime.memory.current_reflection().unwrap(), before);
    assert_eq!(
        runtime.memory.read(survivor.address, 32).unwrap(),
        [0x5a; 32]
    );
    let next = runtime
        .allocate_private_heap_block(heap, 0x100, RuntimeHeapInitialization::Zeroed)
        .unwrap()
        .unwrap();
    acknowledge(&mut runtime.memory);
    assert_eq!(
        runtime.memory.read(next.address, 0x100).unwrap(),
        vec![0; 0x100]
    );
}

fn protection(write: bool) -> PageProtection {
    PageProtection {
        read: true,
        write,
        execute: false,
    }
}

fn acknowledge(memory: &mut GuestMemory) {
    if memory.has_pending_delta() {
        let generation = if memory.stats().acknowledged_generation == 0 {
            memory.pending_reflection().unwrap().invalidation.generation
        } else {
            memory.pending_delta().unwrap().generation
        };
        memory.acknowledge_delta(generation).unwrap();
    }
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

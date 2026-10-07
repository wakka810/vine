use crate::memory::{CpuWritebackBatch, CpuWritebackPage};
use crate::runtime::{RuntimeHeapInitialization, X86_STARTUP_INFO_W_BYTE_LENGTH};
use crate::{
    GUEST_PAGE_SIZE, GuestAddress, GuestMemory, MemoryLimits, Runtime, RuntimeBootstrap,
    RuntimeClockState, RuntimeProcessId, RuntimeStartupInfo, RuntimeThreadId,
};

const PROCESS_ID: u32 = 4096;
const THREAD_ID: u32 = 8192;
const LARGE_BYTES: u32 = 64 * 1024 * 1024;

#[test]
fn lazy_heap_faults_preserve_boundaries_quota_and_released_ranges() {
    let mut runtime = running_runtime(
        GuestMemory::new(MemoryLimits {
            max_physical_pages: 1,
            ..MemoryLimits::default()
        })
        .unwrap(),
    );
    let heap = runtime.allocate_private_heap(0, 0).unwrap();
    let block = runtime
        .allocate_private_heap_block(heap, LARGE_BYTES, RuntimeHeapInitialization::Uninitialized)
        .unwrap()
        .unwrap();
    assert_eq!(runtime.memory.stats().physical_pages, 0);
    acknowledge(&mut runtime.memory);
    let before = runtime.memory.current_reflection().unwrap();
    for (address, fault) in [
        (GuestAddress(block.address.0 - 1), 4),
        (GuestAddress(block.address.0 + LARGE_BYTES), 6),
        (block.address, 7),
    ] {
        assert!(runtime.service_page_fault(address, fault).is_err());
        assert_eq!(runtime.memory.current_reflection().unwrap(), before);
    }
    runtime
        .service_page_fault(GuestAddress(block.address.0 + 0x123), 6)
        .unwrap();
    acknowledge(&mut runtime.memory);
    assert_eq!(runtime.memory.read(block.address, 4096).unwrap(), [0; 4096]);
    runtime.memory.write(block.address, &[0x5a; 32]).unwrap();
    acknowledge(&mut runtime.memory);
    let before = runtime.memory.current_reflection().unwrap();
    for address in [block.address, GuestAddress(block.address.0 + 4096)] {
        assert!(runtime.service_page_fault(address, 6).is_err());
        assert_eq!(runtime.memory.current_reflection().unwrap(), before);
    }
    assert_eq!(runtime.memory.read(block.address, 32).unwrap(), [0x5a; 32]);
    runtime
        .free_private_heap_block(heap, block.address)
        .unwrap();
    acknowledge(&mut runtime.memory);
    assert_eq!(runtime.memory.stats().physical_pages, 0);
    assert!(runtime.memory.read(block.address, 1).is_err());
    let before = runtime.memory.current_reflection().unwrap();
    assert!(runtime.service_page_fault(block.address, 6).is_err());
    assert_eq!(runtime.memory.current_reflection().unwrap(), before);
    let next = runtime
        .allocate_private_heap_block(heap, LARGE_BYTES, RuntimeHeapInitialization::Zeroed)
        .unwrap()
        .unwrap();
    acknowledge(&mut runtime.memory);
    runtime.service_page_fault(next.address, 4).unwrap();
    acknowledge(&mut runtime.memory);
    assert_eq!(runtime.memory.read(next.address, 32).unwrap(), [0; 32]);
}

#[test]
fn lazy_heap_pages_can_materialize_in_sparse_order() {
    let mut runtime = running_runtime(
        GuestMemory::new(MemoryLimits {
            max_physical_pages: 3,
            ..MemoryLimits::default()
        })
        .unwrap(),
    );
    let heap = runtime.allocate_private_heap(0, 0).unwrap();
    let block = runtime
        .allocate_private_heap_block(heap, LARGE_BYTES, RuntimeHeapInitialization::Zeroed)
        .unwrap()
        .unwrap();
    acknowledge(&mut runtime.memory);
    for (index, offset) in [8192, 0, 4096].into_iter().enumerate() {
        let address = GuestAddress(block.address.0 + offset);
        runtime
            .service_page_fault(GuestAddress(address.0 + 0x123), 6)
            .unwrap();
        acknowledge(&mut runtime.memory);
        assert_eq!(runtime.memory.read(address, 4096).unwrap(), [0; 4096]);
        runtime.memory.write(address, &[index as u8 + 1]).unwrap();
        acknowledge(&mut runtime.memory);
    }
    assert_eq!(runtime.memory.stats().physical_pages, 3);
    for (index, offset) in [8192, 0, 4096].into_iter().enumerate() {
        assert_eq!(
            runtime
                .memory
                .read(GuestAddress(block.address.0 + offset), 1)
                .unwrap(),
            [index as u8 + 1]
        );
    }
}

#[test]
fn large_live_ranges_remain_disjoint_and_reject_retired_cpu_owners() {
    let mut runtime = running_runtime(
        GuestMemory::new(MemoryLimits {
            max_physical_pages: 9,
            ..MemoryLimits::default()
        })
        .unwrap(),
    );
    let heap = runtime.allocate_private_heap(0, 0).unwrap();
    let blocks = [LARGE_BYTES, LARGE_BYTES + 8193, 33_177_600].map(|size| {
        runtime
            .allocate_private_heap_block(heap, size, RuntimeHeapInitialization::Zeroed)
            .unwrap()
            .unwrap()
    });
    assert_eq!(runtime.memory.stats().physical_pages, 0);
    acknowledge(&mut runtime.memory);
    for (index, block) in blocks.iter().enumerate() {
        for offset in [0, block.byte_length / 2, block.byte_length - 1] {
            let address = GuestAddress(block.address.0 + offset);
            runtime.service_page_fault(address, 6).unwrap();
            acknowledge(&mut runtime.memory);
            runtime.memory.write(address, &[index as u8 + 1]).unwrap();
            acknowledge(&mut runtime.memory);
        }
        let before = runtime.memory.current_reflection().unwrap();
        assert!(
            runtime
                .service_page_fault(GuestAddress(block.address.0 + block.byte_length), 6)
                .is_err()
        );
        assert_eq!(runtime.memory.current_reflection().unwrap(), before);
    }
    for (index, block) in blocks.iter().enumerate() {
        for offset in [0, block.byte_length / 2, block.byte_length - 1] {
            assert_eq!(
                runtime
                    .memory
                    .read(GuestAddress(block.address.0 + offset), 1)
                    .unwrap(),
                [index as u8 + 1]
            );
        }
    }
    let retired = blocks[1];
    let retired_owner = runtime
        .memory
        .committed_page(retired.address)
        .unwrap()
        .unwrap()
        .0;
    runtime
        .free_private_heap_block(heap, retired.address)
        .unwrap();
    acknowledge(&mut runtime.memory);
    assert_eq!(runtime.memory.stats().physical_pages, 6);
    assert!(runtime.memory.read(retired.address, 1).is_err());
    let reused = runtime
        .allocate_private_heap_block(heap, retired.byte_length, RuntimeHeapInitialization::Zeroed)
        .unwrap()
        .unwrap();
    assert_eq!(reused.address, retired.address);
    acknowledge(&mut runtime.memory);
    runtime.service_page_fault(reused.address, 6).unwrap();
    acknowledge(&mut runtime.memory);
    let before = runtime.memory.current_reflection().unwrap();
    let writeback = CpuWritebackBatch {
        base_generation: runtime.memory.stats().generation,
        pages: vec![CpuWritebackPage {
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
    assert_eq!(runtime.memory.read(blocks[0].address, 1).unwrap(), [1]);
    assert_eq!(runtime.memory.read(blocks[2].address, 1).unwrap(), [3]);
}

#[test]
fn large_heap_owner_region_and_address_exhaustion_preserve_bytes_and_allow_retry() {
    for (max_owners, max_regions) in [(1, 2), (2, 1)] {
        let mut runtime = running_runtime(
            GuestMemory::new(MemoryLimits {
                max_owners,
                max_regions,
                ..MemoryLimits::default()
            })
            .unwrap(),
        );
        let heap = runtime.allocate_private_heap(0, 0).unwrap();
        let block = runtime
            .allocate_private_heap_block(heap, LARGE_BYTES, RuntimeHeapInitialization::Zeroed)
            .unwrap()
            .unwrap();
        acknowledge(&mut runtime.memory);
        runtime.service_page_fault(block.address, 6).unwrap();
        acknowledge(&mut runtime.memory);
        runtime.memory.write(block.address, &[0x5a; 32]).unwrap();
        acknowledge(&mut runtime.memory);
        let before = runtime.memory.current_reflection().unwrap();
        assert!(
            runtime
                .allocate_private_heap_block(heap, LARGE_BYTES, RuntimeHeapInitialization::Zeroed)
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
            .allocate_private_heap_block(heap, LARGE_BYTES, RuntimeHeapInitialization::Zeroed)
            .unwrap()
            .unwrap();
        acknowledge(&mut runtime.memory);
        runtime.service_page_fault(next.address, 6).unwrap();
        acknowledge(&mut runtime.memory);
        assert_eq!(runtime.memory.read(next.address, 32).unwrap(), [0; 32]);
    }
    let mut runtime = running_runtime(GuestMemory::new(MemoryLimits::default()).unwrap());
    let heap = runtime.allocate_private_heap(0, 0).unwrap();
    acknowledge(&mut runtime.memory);
    let before = runtime.memory.current_reflection().unwrap();
    for size in [u32::MAX, 0x7fff_fff8] {
        assert!(
            runtime
                .allocate_private_heap_block(heap, size, RuntimeHeapInitialization::Zeroed)
                .unwrap()
                .is_none()
        );
        assert_eq!(runtime.memory.current_reflection().unwrap(), before);
    }
    let next = runtime
        .allocate_private_heap_block(heap, LARGE_BYTES, RuntimeHeapInitialization::Zeroed)
        .unwrap()
        .unwrap();
    acknowledge(&mut runtime.memory);
    runtime.service_page_fault(next.address, 6).unwrap();
    acknowledge(&mut runtime.memory);
    assert_eq!(runtime.memory.read(next.address, 32).unwrap(), [0; 32]);
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

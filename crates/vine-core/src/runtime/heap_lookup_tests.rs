use super::*;

fn fixture() -> (RuntimeHeaps, u32, GuestMemory, ProcessAddressSpaceAllocator) {
    let mut heaps = RuntimeHeaps::default();
    let handle = heaps
        .create_private(&mut RuntimeObjectTable::default(), 0, 0)
        .unwrap();
    (
        heaps,
        handle,
        GuestMemory::new(crate::memory::MemoryLimits::default()).unwrap(),
        ProcessAddressSpaceAllocator::new().unwrap(),
    )
}

fn allocate(
    heaps: &mut RuntimeHeaps,
    handle: u32,
    memory: &mut GuestMemory,
    allocator: &mut ProcessAddressSpaceAllocator,
    bytes: u32,
) -> RuntimeHeapAllocation {
    let allocation = heaps
        .allocate(
            memory,
            allocator,
            handle,
            bytes,
            RuntimeHeapInitialization::Zeroed,
            None,
        )
        .unwrap()
        .unwrap();
    acknowledge(memory);
    allocation
}

fn acknowledge(memory: &mut GuestMemory) {
    let stats = memory.stats();
    if stats.generation == stats.acknowledged_generation {
        return;
    }
    let generation = if stats.acknowledged_generation == 0 {
        memory.pending_reflection().unwrap().invalidation.generation
    } else {
        memory.pending_delta().unwrap().generation
    };
    memory.acknowledge_delta(generation).unwrap();
}

#[test]
fn lookup_reallocation_preserves_bytes_and_rejects_interior_pointers() {
    let (mut heaps, handle, mut memory, mut allocator) = fixture();
    let original = allocate(&mut heaps, handle, &mut memory, &mut allocator, 32);
    memory.write(original.address, &[0x37; 32]).unwrap();
    acknowledge(&mut memory);
    let grown = heaps
        .allocate(
            &mut memory,
            &mut allocator,
            handle,
            65,
            RuntimeHeapInitialization::Zeroed,
            Some(original.address),
        )
        .unwrap()
        .unwrap();
    acknowledge(&mut memory);
    let mut expected = vec![0x37; 32];
    expected.resize(65, 0);
    assert_eq!(memory.read(grown.address, 65).unwrap(), expected);
    assert_eq!(
        heaps.allocation_option(handle, grown.address).unwrap(),
        Some(grown)
    );

    let interior = GuestAddress(grown.address.0 + 1);
    assert_eq!(heaps.allocation_option(handle, interior).unwrap(), None);
    let generation = memory.stats().generation;
    assert!(
        heaps
            .free(&mut memory, &mut allocator, handle, interior)
            .is_err()
    );
    assert!(
        heaps
            .allocate(
                &mut memory,
                &mut allocator,
                handle,
                128,
                RuntimeHeapInitialization::Zeroed,
                Some(interior),
            )
            .is_err()
    );
    assert_eq!(memory.stats().generation, generation);
    assert_eq!(memory.read(grown.address, 65).unwrap(), expected);
    assert_eq!(
        heaps.allocation_option(handle, grown.address).unwrap(),
        Some(grown)
    );
    heaps
        .free(&mut memory, &mut allocator, handle, grown.address)
        .unwrap();
}

#[test]
fn guest_overwrites_and_wrong_heap_do_not_change_allocation_authority() {
    let (mut heaps, handle, mut memory, mut allocator) = fixture();
    let first = allocate(&mut heaps, handle, &mut memory, &mut allocator, 32);
    let second = allocate(&mut heaps, handle, &mut memory, &mut allocator, 32);
    let other_handle = heaps
        .create_private(&mut RuntimeObjectTable::default(), 0, 0)
        .unwrap();
    for allocation in [first, second] {
        memory.write(allocation.address, &[0xff; 32]).unwrap();
        acknowledge(&mut memory);
        assert_eq!(
            heaps.allocation_option(handle, allocation.address).unwrap(),
            Some(allocation)
        );
        assert_eq!(
            heaps
                .allocation_option(other_handle, allocation.address)
                .unwrap(),
            None
        );
        let generation = memory.stats().generation;
        assert!(
            heaps
                .free(
                    &mut memory,
                    &mut allocator,
                    other_handle,
                    allocation.address,
                )
                .is_err()
        );
        assert_eq!(memory.stats().generation, generation);
        assert_eq!(memory.read(allocation.address, 32).unwrap(), vec![0xff; 32]);
    }
    heaps
        .free(&mut memory, &mut allocator, handle, first.address)
        .unwrap();
    assert_eq!(
        heaps.allocation_option(handle, first.address).unwrap(),
        None
    );
    assert!(
        heaps
            .free(&mut memory, &mut allocator, handle, first.address)
            .is_err()
    );
    let replacement = allocate(&mut heaps, handle, &mut memory, &mut allocator, 32);
    assert_eq!(memory.read(replacement.address, 32).unwrap(), vec![0; 32]);
    assert_eq!(memory.read(second.address, 32).unwrap(), vec![0xff; 32]);
    assert_eq!(
        heaps
            .allocation_option(handle, replacement.address)
            .unwrap(),
        Some(replacement)
    );
}

#[test]
fn lookup_finds_fragmented_extended_and_large_allocations() {
    let (mut heaps, handle, mut memory, mut allocator) = fixture();
    let mut live: Vec<_> = (0..3)
        .map(|_| {
            allocate(
                &mut heaps,
                handle,
                &mut memory,
                &mut allocator,
                RUNTIME_PRIVATE_HEAP_ARENA_STRIDE / 2,
            )
        })
        .collect();
    let retired = live.remove(0);
    heaps
        .free(&mut memory, &mut allocator, handle, retired.address)
        .unwrap();
    for bytes in [17, 41, 65, RUNTIME_PRIVATE_HEAP_ARENA_STRIDE + 8193] {
        live.push(allocate(
            &mut heaps,
            handle,
            &mut memory,
            &mut allocator,
            bytes,
        ));
    }
    for allocation in &live {
        assert_eq!(
            heaps.allocation_option(handle, allocation.address).unwrap(),
            Some(*allocation)
        );
        assert_eq!(
            heaps
                .allocation_option(handle, GuestAddress(allocation.address.0 + 1))
                .unwrap(),
            None
        );
    }
    for allocation in live {
        heaps
            .free(&mut memory, &mut allocator, handle, allocation.address)
            .unwrap();
        assert_eq!(
            heaps.allocation_option(handle, allocation.address).unwrap(),
            None
        );
    }
}

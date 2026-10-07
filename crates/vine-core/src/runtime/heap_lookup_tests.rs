use super::*;

const HANDLE: u32 = 0x6801_0001;

fn fixture() -> (RuntimeHeaps, GuestMemory, ProcessAddressSpaceAllocator) {
    let mut heaps = RuntimeHeaps::default();
    heaps
        .create_private(&mut RuntimeObjectTable::default(), 0, 0)
        .unwrap();
    (
        heaps,
        GuestMemory::new(crate::memory::MemoryLimits::default()).unwrap(),
        ProcessAddressSpaceAllocator::new().unwrap(),
    )
}

fn allocate(
    heaps: &mut RuntimeHeaps,
    memory: &mut GuestMemory,
    allocator: &mut ProcessAddressSpaceAllocator,
    bytes: u32,
) -> RuntimeHeapAllocation {
    heaps
        .allocate(
            memory,
            allocator,
            HANDLE,
            bytes,
            RuntimeHeapInitialization::Zeroed,
            None,
        )
        .unwrap()
        .unwrap()
}

// Keep the original release lookup as an independent full-state oracle and
// diagnostic comparator. It deliberately audits every allocation and free block.
fn audited_lookup(
    heaps: &RuntimeHeaps,
    handle: u32,
    address: GuestAddress,
) -> Result<Option<RuntimeHeapAllocation>, RuntimeStateError> {
    let heap = heaps.heap(handle)?;
    heaps.validate_allocation_count_metadata(heap)?;
    heaps.validate_allocation_id_uniqueness(heap)?;
    let next_id = heaps.next_allocation_id(handle)?;
    for backing in heaps
        .backing(handle)
        .into_iter()
        .chain(heaps.extension_backings(handle).map(|(_, backing)| backing))
    {
        validate_runtime_heap_backing_metadata(heap, backing, next_id)?;
        validate_runtime_heap_backing_contents(backing, next_id)?;
    }
    for (_, backing) in heaps.large_backings_for_handle(handle) {
        validate_large_heap_backing(heap, backing, next_id)?;
    }
    if let Some(backing) = heaps.backing(handle)
        && let Some(allocation) = backing.allocations.get(&address.0)
    {
        return Ok(Some(*allocation));
    }
    for (_, backing) in heaps.extension_backings(handle) {
        if let Some(allocation) = backing.allocations.get(&address.0) {
            return Ok(Some(*allocation));
        }
    }
    for (_, backing) in heaps.large_backings_for_handle(handle) {
        if backing.allocation.address == address {
            return Ok(Some(backing.allocation));
        }
    }
    Ok(None)
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
fn lookup_matches_full_audit_through_randomized_alloc_free_realloc() {
    for seed in [1_u64, 0x5a17, 0xface_cafe, u32::MAX as u64] {
        let (mut heaps, mut memory, mut allocator) = fixture();
        let mut random = seed;
        let mut live: Vec<(RuntimeHeapAllocation, Vec<u8>)> = Vec::new();
        let mut next_id = 1;
        for step in 0..400 {
            random ^= random << 13;
            random ^= random >> 7;
            random ^= random << 17;
            let operation = if live.is_empty() { 0 } else { random % 4 };
            if operation == 0 && live.len() < 96 {
                let bytes = 1 + ((random >> 16) % 151) as u32;
                let allocation = allocate(&mut heaps, &mut memory, &mut allocator, bytes);
                acknowledge(&mut memory);
                assert_eq!(allocation.id, RuntimeHeapAllocationId(next_id));
                next_id += 1;
                assert_eq!(
                    memory.read(allocation.address, bytes as usize).unwrap(),
                    vec![0; bytes as usize]
                );
                let payload = vec![(random >> 32) as u8; bytes as usize];
                memory.write(allocation.address, &payload).unwrap();
                acknowledge(&mut memory);
                live.push((allocation, payload));
            } else if operation == 1 {
                let index = (random as usize / 4) % live.len();
                let (allocation, _) = live.swap_remove(index);
                heaps
                    .free(&mut memory, &mut allocator, HANDLE, allocation.address)
                    .unwrap();
                acknowledge(&mut memory);
                assert_eq!(
                    heaps.allocation_option(HANDLE, allocation.address).unwrap(),
                    None
                );
                assert_eq!(
                    memory
                        .read(allocation.address, allocation.reserved_byte_length as usize)
                        .unwrap(),
                    vec![RUNTIME_PRIVATE_HEAP_FREED_BYTE; allocation.reserved_byte_length as usize]
                );
            } else if operation == 2 {
                let index = (random as usize / 4) % live.len();
                let (previous, previous_bytes) = &live[index];
                let bytes = 1 + ((random >> 16) % 193) as u32;
                let allocation = heaps
                    .allocate(
                        &mut memory,
                        &mut allocator,
                        HANDLE,
                        bytes,
                        RuntimeHeapInitialization::Zeroed,
                        Some(previous.address),
                    )
                    .unwrap()
                    .unwrap();
                acknowledge(&mut memory);
                assert_eq!(allocation.id, RuntimeHeapAllocationId(next_id));
                next_id += 1;
                let mut expected = previous_bytes.clone();
                expected.resize(bytes as usize, 0);
                assert_eq!(
                    memory.read(allocation.address, bytes as usize).unwrap(),
                    expected
                );
                live[index] = (allocation, expected);
            } else if !live.is_empty() {
                let allocation = live[(random as usize / 4) % live.len()].0;
                let interior = GuestAddress(allocation.address.0 + 1);
                let before = format!("{heaps:?}");
                let memory_before = memory.stats();
                let allocator_before = allocator.clone();
                assert_eq!(heaps.allocation_option(HANDLE, interior).unwrap(), None);
                assert!(
                    heaps
                        .free(&mut memory, &mut allocator, HANDLE, interior)
                        .is_err()
                );
                assert!(
                    heaps
                        .allocate(
                            &mut memory,
                            &mut allocator,
                            HANDLE,
                            32,
                            RuntimeHeapInitialization::Zeroed,
                            Some(interior)
                        )
                        .is_err()
                );
                assert_eq!(format!("{heaps:?}"), before);
                assert_eq!(memory.stats(), memory_before);
                assert_eq!(allocator, allocator_before);
            }
            assert_eq!(
                heaps.heap(HANDLE).unwrap().live_allocations as usize,
                live.len()
            );
            // This unconditional oracle retains the complete partition and ID audit
            // even when the production path is compiled with --release.
            assert_eq!(
                audited_lookup(&heaps, HANDLE, GuestAddress(0)).unwrap(),
                None
            );
            for (allocation, expected_bytes) in &live {
                assert_eq!(
                    heaps.allocation_option(HANDLE, allocation.address).unwrap(),
                    Some(*allocation),
                    "seed {seed}, step {step}"
                );
                assert_eq!(
                    memory
                        .read(allocation.address, expected_bytes.len())
                        .unwrap(),
                    *expected_bytes
                );
            }
        }
    }
}

#[test]
fn lookup_guest_byte_overwrites_do_not_change_host_allocation_authority() {
    let (mut heaps, mut memory, mut allocator) = fixture();
    let first = allocate(&mut heaps, &mut memory, &mut allocator, 32);
    let second = allocate(&mut heaps, &mut memory, &mut allocator, 32);
    let other_handle = heaps
        .create_private(&mut RuntimeObjectTable::default(), 0, 0)
        .unwrap();
    memory.write(first.address, &[0xff; 64]).unwrap();
    for allocation in [first, second] {
        assert_eq!(
            heaps.allocation_option(HANDLE, allocation.address).unwrap(),
            Some(allocation)
        );
        assert_eq!(
            audited_lookup(&heaps, HANDLE, allocation.address).unwrap(),
            Some(allocation)
        );
        assert_eq!(
            heaps
                .allocation_option(other_handle, allocation.address)
                .unwrap(),
            None
        );
        let before = format!("{heaps:?}");
        let memory_before = memory.stats();
        assert!(
            heaps
                .free(
                    &mut memory,
                    &mut allocator,
                    other_handle,
                    allocation.address
                )
                .is_err()
        );
        assert_eq!(format!("{heaps:?}"), before);
        assert_eq!(memory.stats(), memory_before);
    }
    heaps
        .free(&mut memory, &mut allocator, HANDLE, first.address)
        .unwrap();
    let before = format!("{heaps:?}");
    let memory_before = memory.stats();
    assert_eq!(
        heaps.allocation_option(HANDLE, first.address).unwrap(),
        None
    );
    assert!(
        heaps
            .free(&mut memory, &mut allocator, HANDLE, first.address)
            .is_err()
    );
    assert_eq!(format!("{heaps:?}"), before);
    assert_eq!(memory.stats(), memory_before);
    let reused = allocate(&mut heaps, &mut memory, &mut allocator, 32);
    assert_eq!(reused.address, first.address);
    assert_ne!(reused.id, first.id);
    assert_eq!(
        heaps.allocation_option(HANDLE, reused.address).unwrap(),
        Some(reused)
    );
}

#[test]
fn lookup_full_audit_agrees_for_fragmented_extensions_and_lazy_large_allocations() {
    let (mut heaps, mut memory, mut allocator) = fixture();
    let mut live = Vec::new();
    for _ in 0..3 {
        live.push(allocate(
            &mut heaps,
            &mut memory,
            &mut allocator,
            RUNTIME_PRIVATE_HEAP_ARENA_STRIDE / 2,
        ));
    }
    assert_eq!(heaps.extension_backings(HANDLE).count(), 1);
    let retired = live.remove(0);
    heaps
        .free(&mut memory, &mut allocator, HANDLE, retired.address)
        .unwrap();
    for bytes in [17, 41, 65] {
        live.push(allocate(&mut heaps, &mut memory, &mut allocator, bytes));
    }
    let pages_before = memory.stats().physical_pages;
    let large = allocate(
        &mut heaps,
        &mut memory,
        &mut allocator,
        RUNTIME_PRIVATE_HEAP_ARENA_STRIDE + 8193,
    );
    assert_eq!(memory.stats().physical_pages, pages_before);
    live.push(large);
    for allocation in live {
        for address in [
            allocation.address,
            GuestAddress(allocation.address.0 + 1),
            GuestAddress(allocation.address.0 - 1),
        ] {
            assert_eq!(
                heaps.allocation_option(HANDLE, address).unwrap(),
                audited_lookup(&heaps, HANDLE, address).unwrap()
            );
        }
    }
    heaps
        .free(&mut memory, &mut allocator, HANDLE, large.address)
        .unwrap();
    assert_eq!(
        heaps.allocation_option(HANDLE, large.address).unwrap(),
        None
    );
    assert_eq!(audited_lookup(&heaps, HANDLE, large.address).unwrap(), None);
}

fn assert_corrupt_target_rejected_without_mutation(
    heaps: &mut RuntimeHeaps,
    memory: &mut GuestMemory,
    allocator: &mut ProcessAddressSpaceAllocator,
    address: GuestAddress,
) {
    let before = format!("{heaps:?}");
    let memory_before = memory.stats();
    let allocator_before = allocator.clone();
    assert!(heaps.allocation_option(HANDLE, address).is_err());
    assert!(heaps.free(memory, allocator, HANDLE, address).is_err());
    assert!(
        heaps
            .allocate(
                memory,
                allocator,
                HANDLE,
                32,
                RuntimeHeapInitialization::Zeroed,
                Some(address)
            )
            .is_err()
    );
    assert_eq!(format!("{heaps:?}"), before);
    assert_eq!(memory.stats(), memory_before);
    assert_eq!(*allocator, allocator_before);
}

#[test]
fn lookup_rejects_selected_host_record_corruption_even_in_release() {
    let (mut heaps, mut memory, mut allocator) = fixture();
    let original = allocate(&mut heaps, &mut memory, &mut allocator, 64);
    let variants = [
        RuntimeHeapAllocation {
            id: RuntimeHeapAllocationId(0),
            ..original
        },
        RuntimeHeapAllocation {
            id: RuntimeHeapAllocationId(2),
            ..original
        },
        RuntimeHeapAllocation {
            address: GuestAddress(original.address.0 + 8),
            ..original
        },
        RuntimeHeapAllocation {
            byte_length: 0,
            ..original
        },
        RuntimeHeapAllocation {
            byte_length: 65,
            ..original
        },
        RuntimeHeapAllocation {
            reserved_byte_length: 63,
            ..original
        },
        RuntimeHeapAllocation {
            reserved_byte_length: 72,
            ..original
        },
        RuntimeHeapAllocation {
            reserved_byte_length: u32::MAX,
            ..original
        },
    ];
    // These mutations are deliberately injected host bugs, never guest bytes.
    // Check the selected record independently of debug-only complete audits.
    for variant in variants {
        heaps
            .backings
            .get_mut(&HANDLE)
            .unwrap()
            .allocations
            .insert(original.address.0, variant);
        assert_corrupt_target_rejected_without_mutation(
            &mut heaps,
            &mut memory,
            &mut allocator,
            original.address,
        );
    }
    let shifted = RuntimeHeapAllocation {
        address: GuestAddress(original.address.0 + 1),
        ..original
    };
    let backing = heaps.backings.get_mut(&HANDLE).unwrap();
    backing.allocations.remove(&original.address.0);
    backing.allocations.insert(shifted.address.0, shifted);
    assert_corrupt_target_rejected_without_mutation(
        &mut heaps,
        &mut memory,
        &mut allocator,
        shifted.address,
    );
}

#[test]
fn lookup_rejects_overlapping_free_neighbors_even_in_release() {
    let (mut heaps, mut memory, mut allocator) = fixture();
    let first = allocate(&mut heaps, &mut memory, &mut allocator, 64);
    let selected = allocate(&mut heaps, &mut memory, &mut allocator, 64);
    let last = allocate(&mut heaps, &mut memory, &mut allocator, 64);
    heaps
        .free(&mut memory, &mut allocator, HANDLE, first.address)
        .unwrap();
    heaps
        .free(&mut memory, &mut allocator, HANDLE, last.address)
        .unwrap();
    for free_blocks in [
        BTreeMap::from([(0, 72), (128, 64)]),
        BTreeMap::from([(0, 64), (120, 64)]),
        BTreeMap::from([(0, u32::MAX), (128, 64)]),
    ] {
        heaps.backings.get_mut(&HANDLE).unwrap().free_blocks = free_blocks;
        assert_corrupt_target_rejected_without_mutation(
            &mut heaps,
            &mut memory,
            &mut allocator,
            selected.address,
        );
    }
}

#[test]
fn lookup_rejects_large_backing_under_wrong_live_map_key() {
    let (mut heaps, mut memory, mut allocator) = fixture();
    let allocation = allocate(
        &mut heaps,
        &mut memory,
        &mut allocator,
        RUNTIME_PRIVATE_HEAP_ARENA_STRIDE + 1,
    );
    let backing = heaps.large_backings.remove(&allocation.address.0).unwrap();
    heaps
        .large_backings
        .insert(allocation.address.0 + 65_536, backing);
    assert_corrupt_target_rejected_without_mutation(
        &mut heaps,
        &mut memory,
        &mut allocator,
        allocation.address,
    );
}

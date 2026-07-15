use vine_core::{
    GuestAddress, GuestMemory, GuestRange, MEMORY_DELTA_BINARY_VERSION, MappingOperation,
    MemoryLimits, MemoryOwnerKind, MemoryRegionKind, PageProtection,
};

const ADDRESS_ALLOCATION_ID: u64 = 0x1_0000_0001;
const BASE: GuestAddress = GuestAddress(0x3100_0000);

#[test]
fn address_allocation_identity_follows_every_mapping_mutation_and_binary_transport() {
    let mut memory = GuestMemory::new(MemoryLimits::default()).expect("memory");
    let baseline_owner = memory
        .create_owner(MemoryOwnerKind::Anonymous)
        .expect("baseline owner");
    let baseline = memory
        .reserve_fixed(
            baseline_owner,
            MemoryRegionKind::Anonymous,
            GuestAddress(0x3000_0000),
            0x1000,
        )
        .expect("baseline reservation");
    memory
        .commit(baseline_owner, baseline, read_write())
        .expect("baseline commit");
    let reflection = memory.pending_reflection().expect("initial reflection");
    memory
        .acknowledge_delta(reflection.invalidation.generation)
        .expect("acknowledge reflection");

    let owner = memory
        .create_owner(MemoryOwnerKind::Anonymous)
        .expect("owner");
    let range = memory
        .reserve_fixed_for_address_allocation(
            ADDRESS_ALLOCATION_ID,
            owner,
            MemoryRegionKind::Anonymous,
            BASE,
            0x2000,
        )
        .expect("allocation-backed reservation");
    let first_page = GuestRange::new(BASE, 0x1000).expect("first page");
    memory
        .commit(owner, first_page, read_write())
        .expect("commit");
    memory
        .protect(owner, first_page, read_only())
        .expect("protect");
    memory.decommit(owner, first_page).expect("decommit");
    memory
        .commit(owner, first_page, read_write())
        .expect("recommit");
    memory.release_region(owner, BASE).expect("release");

    let delta = memory.pending_delta().expect("pending mapping delta");
    assert_eq!(
        delta
            .mapping_operations
            .iter()
            .map(mapping_address_allocation_id)
            .collect::<Vec<_>>(),
        vec![Some(ADDRESS_ALLOCATION_ID); 6]
    );
    assert!(matches!(
        delta.mapping_operations.as_slice(),
        [
            MappingOperation::Reserve { .. },
            MappingOperation::Commit { .. },
            MappingOperation::Protect { .. },
            MappingOperation::Decommit { .. },
            MappingOperation::Commit { .. },
            MappingOperation::Release { .. },
        ]
    ));
    assert_eq!(range.start(), BASE);

    let encoded = delta.encode_binary().expect("binary delta");
    let control = encoded.control;
    assert_eq!(u32_at(&control, 8), MEMORY_DELTA_BINARY_VERSION);
    assert_eq!(u32_at(&control, 36), 40);
    assert_eq!(u32_at(&control, 32), 6);
    for index in 0..6 {
        let descriptor = 56 + index * 40;
        assert_eq!(u64_at(&control, descriptor + 28), ADDRESS_ALLOCATION_ID);
        assert_eq!(u32_at(&control, descriptor + 36), 0);
    }
}

fn mapping_address_allocation_id(operation: &MappingOperation) -> Option<u64> {
    match operation {
        MappingOperation::Reserve {
            address_allocation_id,
            ..
        }
        | MappingOperation::Commit {
            address_allocation_id,
            ..
        }
        | MappingOperation::Protect {
            address_allocation_id,
            ..
        }
        | MappingOperation::Decommit {
            address_allocation_id,
            ..
        }
        | MappingOperation::Release {
            address_allocation_id,
            ..
        } => *address_allocation_id,
    }
}

fn read_write() -> PageProtection {
    PageProtection {
        read: true,
        write: true,
        execute: false,
    }
}

fn read_only() -> PageProtection {
    PageProtection {
        read: true,
        write: false,
        execute: false,
    }
}

fn u32_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("u32 field"))
}

fn u64_at(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().expect("u64 field"))
}

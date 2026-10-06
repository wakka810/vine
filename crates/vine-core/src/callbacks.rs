use crate::memory::{
    GUEST_PAGE_SIZE, GuestAddress, GuestMemory, GuestMemoryError, GuestRange, MemoryOwnerId,
    MemoryOwnerKind, MemoryRegionKind, PageProtection,
};
use crate::runtime::{
    ProcessAddressAllocationClassification, ProcessAddressAllocationPurpose,
    ProcessAddressAllocationRecord, ProcessAddressAllocationReleasePolicy,
    ProcessAddressMappingRequirements, RuntimeAddressRange,
    validate_process_address_mapping_allocation,
};

pub const CALLBACK_RETURN_ARENA_BASE: GuestAddress = GuestAddress(0x7f01_0000);
pub const CALLBACK_RETURN_ARENA_SIZE: u64 = 0x1_0000;
pub const PROCESS_ENTRY_RETURN_SENTINEL_ADDRESS: GuestAddress = GuestAddress(0x7f01_0010);
// Keep callback scratch outside the ordinary PE image range (0x00400000 + SizeOfImage).
// The previous low arena could alias real executable pages and corrupt callback frames.
pub const CALLBACK_SCRATCH_ARENA_BASE: GuestAddress = GuestAddress(0x7fc4_0000);
pub const CALLBACK_SCRATCH_ARENA_SIZE: u64 = 0x4_0000;
pub const CALLBACK_SCRATCH_SLOT_SIZE: u32 = GUEST_PAGE_SIZE as u32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessEntryReturnSentinel {
    pub address: GuestAddress,
    pub owner: MemoryOwnerId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeReturnSentinels {
    pub callback_return: GuestAddress,
    pub process_entry_return: ProcessEntryReturnSentinel,
}

pub fn install_runtime_return_sentinels(
    memory: &mut GuestMemory,
    allocation: &ProcessAddressAllocationRecord,
) -> Result<RuntimeReturnSentinels, GuestMemoryError> {
    validate_process_address_mapping_allocation(
        allocation,
        &ProcessAddressMappingRequirements {
            purpose: ProcessAddressAllocationPurpose::CallbackReturns,

            range: RuntimeAddressRange {
                base: CALLBACK_RETURN_ARENA_BASE,
                byte_length: CALLBACK_RETURN_ARENA_SIZE,
            },
            classification: ProcessAddressAllocationClassification::RuntimeControl,

            release_policy: ProcessAddressAllocationReleasePolicy::Permanent,
            executable: true,
        },
    )?;
    memory.preflight_growth(1, 1, 1)?;
    memory.preflight_fixed_reservation(CALLBACK_RETURN_ARENA_BASE, CALLBACK_RETURN_ARENA_SIZE)?;
    if PROCESS_ENTRY_RETURN_SENTINEL_ADDRESS.0 < CALLBACK_RETURN_ARENA_BASE.0
        || u64::from(PROCESS_ENTRY_RETURN_SENTINEL_ADDRESS.0)
            >= u64::from(CALLBACK_RETURN_ARENA_BASE.0) + GUEST_PAGE_SIZE
    {
        return Err(GuestMemoryError::runtime_defect(
            "process-entry return sentinel does not fit the committed runtime control page",
        ));
    }
    let mut transaction = memory.begin_transaction();
    let owner = transaction.create_owner(MemoryOwnerKind::Runtime)?;
    transaction.reserve_fixed_for_address_allocation(
        allocation.id.0,
        owner,
        MemoryRegionKind::Runtime,
        CALLBACK_RETURN_ARENA_BASE,
        CALLBACK_RETURN_ARENA_SIZE,
    )?;
    let committed_range = GuestRange::new(CALLBACK_RETURN_ARENA_BASE, GUEST_PAGE_SIZE)?;
    transaction.commit(
        owner,
        committed_range,
        PageProtection {
            read: true,
            write: false,
            execute: true,
        },
    )?;
    fill_halt_page(&mut transaction, owner)?;
    transaction.write_initialization(owner, CALLBACK_RETURN_ARENA_BASE, &[0x0f, 0x0b])?;
    transaction.write_initialization(
        owner,
        PROCESS_ENTRY_RETURN_SENTINEL_ADDRESS,
        &[0x0f, 0x0b],
    )?;
    transaction.seal_owner(owner)?;

    let process_entry_return = ProcessEntryReturnSentinel {
        address: PROCESS_ENTRY_RETURN_SENTINEL_ADDRESS,
        owner,
    };
    let sentinels = RuntimeReturnSentinels {
        callback_return: CALLBACK_RETURN_ARENA_BASE,
        process_entry_return,
    };
    transaction.commit_transaction();
    Ok(sentinels)
}

fn fill_halt_page(memory: &mut GuestMemory, owner: MemoryOwnerId) -> Result<(), GuestMemoryError> {
    let page_size = GUEST_PAGE_SIZE as usize;
    let chunk_length = memory.limits().max_access_bytes.min(page_size);
    let halt_bytes = vec![0xf4; chunk_length];
    let mut offset = 0usize;
    while offset < page_size {
        let byte_length = (page_size - offset).min(chunk_length);
        let address = add_offset(CALLBACK_RETURN_ARENA_BASE, offset)?;
        memory.write_initialization(owner, address, &halt_bytes[..byte_length])?;
        offset += byte_length;
    }
    Ok(())
}

fn add_offset(base: GuestAddress, offset: usize) -> Result<GuestAddress, GuestMemoryError> {
    let offset = u32::try_from(offset).map_err(|_| {
        GuestMemoryError::runtime_defect("callback return arena offset does not fit 32 bits")
    })?;
    let address = base.0.checked_add(offset).ok_or_else(|| {
        GuestMemoryError::runtime_defect("callback return arena address overflows 32 bits")
    })?;
    Ok(GuestAddress(address))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::MemoryLimits;
    use crate::runtime::ProcessAddressSpaceAllocator;

    #[test]
    fn sentinel_initialization_failure_rolls_back_committed_pages_and_owner() {
        let allocator = ProcessAddressSpaceAllocator::new().unwrap();
        let allocation = allocator
            .allocation_for_purpose(&ProcessAddressAllocationPurpose::CallbackReturns)
            .unwrap();
        let mut memory = GuestMemory::new(MemoryLimits {
            max_access_bytes: 1,
            ..MemoryLimits::default()
        })
        .unwrap();
        let before = memory.stats();
        // Page filling succeeds in one-byte chunks, but the two-byte sentinel
        // instruction cannot be initialized under this access bound.
        assert!(install_runtime_return_sentinels(&mut memory, allocation).is_err());
        assert_eq!(memory.stats(), before);
        assert_eq!(
            memory.create_owner(MemoryOwnerKind::Runtime).unwrap(),
            MemoryOwnerId(1)
        );
    }

    #[test]
    fn mapping_record_mismatches_are_rejected_before_memory_mutation() {
        let allocator = ProcessAddressSpaceAllocator::new().expect("allocator");
        let allocation = allocator
            .allocation_for_purpose(&ProcessAddressAllocationPurpose::CallbackReturns)
            .expect("callback allocation")
            .clone();

        for mismatched in [
            {
                let mut value = allocation.clone();
                value.purpose = ProcessAddressAllocationPurpose::DynamicExports;
                value
            },
            {
                let mut value = allocation.clone();
                value.range.byte_length = GUEST_PAGE_SIZE;
                value
            },
            {
                let mut value = allocation.clone();
                value.release_policy = ProcessAddressAllocationReleasePolicy::Explicit;
                value
            },
        ] {
            let mut memory = GuestMemory::new(MemoryLimits::default()).expect("memory");
            let before = memory.stats();
            assert!(install_runtime_return_sentinels(&mut memory, &mismatched).is_err());
            assert_eq!(memory.stats(), before);
            let sentinels = install_runtime_return_sentinels(&mut memory, &allocation).unwrap();
            assert_eq!(sentinels.process_entry_return.owner, MemoryOwnerId(1));
            assert!(
                memory
                    .owner_is_sealed(sentinels.process_entry_return.owner)
                    .unwrap()
            );
            let reflection = memory.pending_reflection().unwrap();
            memory
                .acknowledge_delta(reflection.invalidation.generation)
                .unwrap();
            assert_eq!(
                memory.fetch(sentinels.callback_return, 2).unwrap(),
                [0x0f, 0x0b]
            );
            assert_eq!(
                memory
                    .fetch(sentinels.process_entry_return.address, 2)
                    .unwrap(),
                [0x0f, 0x0b]
            );
        }
    }
}

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::memory::{
    GUEST_PAGE_SIZE, GuestAddress, GuestMemory, GuestMemoryError, GuestRange, MemoryInvalidation,
    MemoryOwnerId, MemoryOwnerKind, MemoryRegionKind, PageProtection,
};
use crate::runtime::{
    ProcessAddressAllocationClassification, ProcessAddressAllocationLifetime,
    ProcessAddressAllocationPurpose, ProcessAddressAllocationRecord,
    ProcessAddressAllocationReleasePolicy, ProcessAddressMappingRequirements, RuntimeAddressRange,
    validate_process_address_mapping_allocation_for_memory,
};

pub const CALLBACK_RETURN_ARENA_BASE: GuestAddress = GuestAddress(0x7f01_0000);
pub const CALLBACK_RETURN_ARENA_SIZE: u64 = 0x1_0000;
pub const CALLBACK_RETURN_SLOT_SIZE: u32 = 0x10;
pub const PROCESS_ENTRY_RETURN_SENTINEL_ADDRESS: GuestAddress = GuestAddress(0x7f01_0010);
pub const CALLBACK_SCRATCH_ARENA_BASE: GuestAddress = GuestAddress(0x0050_1000);
pub const CALLBACK_SCRATCH_ARENA_SIZE: u64 = 0x4_0000;
pub const CALLBACK_SCRATCH_SLOT_SIZE: u32 = GUEST_PAGE_SIZE as u32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct CallbackReturnSentinelId(pub u32);

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CallbackReturnSentinel {
    pub id: CallbackReturnSentinelId,
    pub address: GuestAddress,
    pub owner: MemoryOwnerId,
    pub arena_range: GuestRange,
    pub committed_pages: usize,
    pub slot_size: u32,
    pub content_sha256: String,
    pub owner_sealed: bool,
    pub invalidation: MemoryInvalidation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct ProcessEntryReturnSentinelId(pub u32);

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessEntryReturnSentinel {
    pub id: ProcessEntryReturnSentinelId,
    pub address: GuestAddress,
    pub owner: MemoryOwnerId,
    pub arena_range: GuestRange,
    pub committed_pages: usize,
    pub slot_size: u32,
    pub content_sha256: String,
    pub owner_sealed: bool,
    pub invalidation: MemoryInvalidation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeReturnSentinels {
    pub callback_return: CallbackReturnSentinel,
    pub process_entry_return: ProcessEntryReturnSentinel,
}

pub fn install_callback_return_sentinel(
    memory: &mut GuestMemory,
    allocation: &ProcessAddressAllocationRecord,
) -> Result<CallbackReturnSentinel, GuestMemoryError> {
    let (callback_return, process_entry_return) =
        install_return_sentinels(memory, allocation, false)?;
    debug_assert!(process_entry_return.is_none());
    Ok(callback_return)
}

pub fn install_runtime_return_sentinels(
    memory: &mut GuestMemory,
    allocation: &ProcessAddressAllocationRecord,
) -> Result<RuntimeReturnSentinels, GuestMemoryError> {
    let (callback_return, process_entry_return) =
        install_return_sentinels(memory, allocation, true)?;
    let process_entry_return = process_entry_return.ok_or_else(|| {
        GuestMemoryError::runtime_defect(
            "process-entry-return",
            "process-entry-return-sentinel-missing",
            "combined runtime return installation omitted the process-entry sentinel",
        )
    })?;
    Ok(RuntimeReturnSentinels {
        callback_return,
        process_entry_return,
    })
}

fn install_return_sentinels(
    memory: &mut GuestMemory,
    allocation: &ProcessAddressAllocationRecord,
    include_process_entry_return: bool,
) -> Result<(CallbackReturnSentinel, Option<ProcessEntryReturnSentinel>), GuestMemoryError> {
    validate_process_address_mapping_allocation_for_memory(
        allocation,
        &ProcessAddressMappingRequirements {
            purpose: ProcessAddressAllocationPurpose::CallbackReturns,
            owner: "runtime-callback-returns".to_owned(),
            range: RuntimeAddressRange {
                base: CALLBACK_RETURN_ARENA_BASE,
                byte_length: CALLBACK_RETURN_ARENA_SIZE,
            },
            classification: ProcessAddressAllocationClassification::RuntimeControl,
            lifetime: ProcessAddressAllocationLifetime::Process,
            release_policy: ProcessAddressAllocationReleasePolicy::Permanent,
            executable: true,
        },
    )?;
    memory.preflight_growth(1, 1, 1, "callback-return")?;
    memory.preflight_fixed_reservation(
        CALLBACK_RETURN_ARENA_BASE,
        CALLBACK_RETURN_ARENA_SIZE,
        "callback-return",
    )?;
    if include_process_entry_return
        && (PROCESS_ENTRY_RETURN_SENTINEL_ADDRESS.0 < CALLBACK_RETURN_ARENA_BASE.0
            || u64::from(PROCESS_ENTRY_RETURN_SENTINEL_ADDRESS.0)
                >= u64::from(CALLBACK_RETURN_ARENA_BASE.0) + GUEST_PAGE_SIZE)
    {
        return Err(GuestMemoryError::runtime_defect(
            "process-entry-return",
            "process-entry-return-sentinel-outside-committed-page",
            "process-entry return sentinel does not fit the committed runtime control page",
        ));
    }
    let mut transaction = memory.begin_transaction();
    let sentinels = install_return_sentinels_transaction(
        &mut transaction,
        allocation,
        include_process_entry_return,
    )?;
    transaction.commit_transaction();
    Ok(sentinels)
}

fn install_return_sentinels_transaction(
    memory: &mut GuestMemory,
    allocation: &ProcessAddressAllocationRecord,
    include_process_entry_return: bool,
) -> Result<(CallbackReturnSentinel, Option<ProcessEntryReturnSentinel>), GuestMemoryError> {
    let owner = memory.create_owner(MemoryOwnerKind::Runtime)?;
    let arena_range = match memory.reserve_fixed_for_address_allocation(
        allocation.id.0,
        owner,
        MemoryRegionKind::Runtime,
        CALLBACK_RETURN_ARENA_BASE,
        CALLBACK_RETURN_ARENA_SIZE,
    ) {
        Ok(range) => range,
        Err(error) => {
            memory.destroy_owner(owner)?;
            return Err(error);
        }
    };
    let committed_range = GuestRange::new(CALLBACK_RETURN_ARENA_BASE, GUEST_PAGE_SIZE)?;
    memory.commit(
        owner,
        committed_range,
        PageProtection {
            read: true,
            write: false,
            execute: true,
        },
    )?;
    fill_halt_page(memory, owner)?;
    memory.write_initialization(owner, CALLBACK_RETURN_ARENA_BASE, &[0x0f, 0x0b])?;
    if include_process_entry_return {
        memory.write_initialization(owner, PROCESS_ENTRY_RETURN_SENTINEL_ADDRESS, &[0x0f, 0x0b])?;
    }
    let content_sha256 = hash_initialization_page(memory, owner)?;
    let invalidation = memory.seal_owner(owner)?;

    let callback_return = CallbackReturnSentinel {
        id: CallbackReturnSentinelId(1),
        address: CALLBACK_RETURN_ARENA_BASE,
        owner,
        arena_range,
        committed_pages: 1,
        slot_size: CALLBACK_RETURN_SLOT_SIZE,
        content_sha256: content_sha256.clone(),
        owner_sealed: true,
        invalidation,
    };
    let process_entry_return = include_process_entry_return.then_some(ProcessEntryReturnSentinel {
        id: ProcessEntryReturnSentinelId(2),
        address: PROCESS_ENTRY_RETURN_SENTINEL_ADDRESS,
        owner,
        arena_range,
        committed_pages: 1,
        slot_size: CALLBACK_RETURN_SLOT_SIZE,
        content_sha256,
        owner_sealed: true,
        invalidation,
    });
    Ok((callback_return, process_entry_return))
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

fn hash_initialization_page(
    memory: &GuestMemory,
    owner: MemoryOwnerId,
) -> Result<String, GuestMemoryError> {
    let page_size = GUEST_PAGE_SIZE as usize;
    let chunk_length = memory.limits().max_access_bytes.min(page_size);
    let mut hasher = Sha256::new();
    let mut offset = 0usize;
    while offset < page_size {
        let byte_length = (page_size - offset).min(chunk_length);
        let address = add_offset(CALLBACK_RETURN_ARENA_BASE, offset)?;
        hasher.update(memory.read_initialization(owner, address, byte_length)?);
        offset += byte_length;
    }
    Ok(hex_digest(&hasher.finalize()))
}

fn add_offset(base: GuestAddress, offset: usize) -> Result<GuestAddress, GuestMemoryError> {
    let offset = u32::try_from(offset).map_err(|_| {
        GuestMemoryError::runtime_defect(
            "callback-return",
            "callback-return-offset-not-representable",
            "callback return arena offset does not fit 32 bits",
        )
    })?;
    let address = base.0.checked_add(offset).ok_or_else(|| {
        GuestMemoryError::runtime_defect(
            "callback-return",
            "callback-return-address-overflow",
            "callback return arena address overflows 32 bits",
        )
    })?;
    Ok(GuestAddress(address))
}

fn hex_digest(digest: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut value = String::with_capacity(digest.len() * 2);
    for byte in digest {
        value.push(char::from(HEX[usize::from(byte >> 4)]));
        value.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::MemoryLimits;
    use crate::runtime::{ProcessAddressSpaceAllocator, ProcessAddressSpaceLayout};

    #[test]
    fn mapping_record_mismatches_are_rejected_before_memory_mutation() {
        let allocator = ProcessAddressSpaceAllocator::vine_v1(ProcessAddressSpaceLayout::vine_v1())
            .expect("allocator");
        let allocation = allocator
            .allocation_for_purpose(&ProcessAddressAllocationPurpose::CallbackReturns)
            .expect("callback allocation")
            .clone();

        for (mismatched, expected_code) in [
            (
                {
                    let mut value = allocation.clone();
                    value.owner = "wrong-owner".to_owned();
                    value
                },
                "address-allocation-mapping-owner-mismatch",
            ),
            (
                {
                    let mut value = allocation.clone();
                    value.range.byte_length = GUEST_PAGE_SIZE;
                    value
                },
                "address-allocation-mapping-range-mismatch",
            ),
            (
                {
                    let mut value = allocation.clone();
                    value.lifetime = ProcessAddressAllocationLifetime::Thread;
                    value
                },
                "address-allocation-mapping-lifetime-mismatch",
            ),
        ] {
            let mut memory = GuestMemory::new(MemoryLimits::default()).expect("memory");
            let before = memory.stats();
            let error = install_runtime_return_sentinels(&mut memory, &mismatched)
                .expect_err("mismatched allocation must fail");
            assert_eq!(error.code, expected_code);
            assert_eq!(memory.stats(), before);
        }
    }
}

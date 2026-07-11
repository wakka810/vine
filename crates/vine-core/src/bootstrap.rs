use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::callbacks::{ProcessEntryReturnSentinel, ProcessEntryReturnSentinelId};
use crate::memory::{
    GUEST_PAGE_SIZE, GuestAddress, GuestMemory, GuestMemoryError, GuestRange, MemoryInvalidation,
    MemoryOwnerId, MemoryOwnerKind, MemoryRegionKind, PageProtection,
};

pub const CONTROLLED_INITIAL_STACK_BASE: GuestAddress = GuestAddress(0x006f_f000);
pub const CONTROLLED_INITIAL_STACK_TOP: GuestAddress = GuestAddress(0x0070_0000);
pub const CONTROLLED_INITIAL_STACK_SIZE: u64 = GUEST_PAGE_SIZE;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InitialThreadStack {
    pub owner: MemoryOwnerId,
    pub stack_range: GuestRange,
    pub committed_pages: usize,
    pub stack_top: GuestAddress,
    pub initial_esp: GuestAddress,
    pub entry_return_sentinel_id: ProcessEntryReturnSentinelId,
    pub entry_return_address: GuestAddress,
    pub content_sha256: String,
    pub owner_sealed: bool,
    pub invalidation: MemoryInvalidation,
}

pub fn install_controlled_initial_stack(
    memory: &mut GuestMemory,
    entry_return: &ProcessEntryReturnSentinel,
) -> Result<InitialThreadStack, GuestMemoryError> {
    if !entry_return.owner_sealed {
        return Err(GuestMemoryError::runtime_defect(
            "process-bootstrap",
            "entry-return-sentinel-owner-unsealed",
            "initial stack requires a sealed process-entry return sentinel",
        ));
    }
    memory.preflight_growth(1, 1, 1, "process-bootstrap")?;
    memory.preflight_fixed_reservation(
        CONTROLLED_INITIAL_STACK_BASE,
        CONTROLLED_INITIAL_STACK_SIZE,
        "process-bootstrap",
    )?;
    let initial_esp = GuestAddress(CONTROLLED_INITIAL_STACK_TOP.0.checked_sub(4).ok_or_else(
        || {
            GuestMemoryError::runtime_defect(
                "process-bootstrap",
                "initial-stack-pointer-underflow",
                "controlled initial stack cannot hold its entry return address",
            )
        },
    )?);

    let mut transaction = memory.begin_transaction();
    let stack =
        install_controlled_initial_stack_transaction(&mut transaction, entry_return, initial_esp)?;
    transaction.commit_transaction();
    Ok(stack)
}

fn install_controlled_initial_stack_transaction(
    memory: &mut GuestMemory,
    entry_return: &ProcessEntryReturnSentinel,
    initial_esp: GuestAddress,
) -> Result<InitialThreadStack, GuestMemoryError> {
    let owner = memory.create_owner(MemoryOwnerKind::Anonymous)?;
    let stack_range = match memory.reserve_fixed(
        owner,
        MemoryRegionKind::Stack,
        CONTROLLED_INITIAL_STACK_BASE,
        CONTROLLED_INITIAL_STACK_SIZE,
    ) {
        Ok(range) => range,
        Err(error) => {
            memory.destroy_owner(owner)?;
            return Err(error);
        }
    };
    memory.commit(
        owner,
        stack_range,
        PageProtection {
            read: true,
            write: true,
            execute: false,
        },
    )?;
    memory.write_initialization(owner, initial_esp, &entry_return.address.0.to_le_bytes())?;
    let content_sha256 = hash_initialization_page(memory, owner)?;
    let invalidation = memory.seal_owner(owner)?;

    Ok(InitialThreadStack {
        owner,
        stack_range,
        committed_pages: 1,
        stack_top: CONTROLLED_INITIAL_STACK_TOP,
        initial_esp,
        entry_return_sentinel_id: entry_return.id,
        entry_return_address: entry_return.address,
        content_sha256,
        owner_sealed: true,
        invalidation,
    })
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
        let address = add_offset(CONTROLLED_INITIAL_STACK_BASE, offset)?;
        hasher.update(memory.read_initialization(owner, address, byte_length)?);
        offset += byte_length;
    }
    Ok(hex_digest(&hasher.finalize()))
}

fn add_offset(base: GuestAddress, offset: usize) -> Result<GuestAddress, GuestMemoryError> {
    let offset = u32::try_from(offset).map_err(|_| {
        GuestMemoryError::runtime_defect(
            "process-bootstrap",
            "initial-stack-offset-not-representable",
            "controlled initial stack offset does not fit 32 bits",
        )
    })?;
    let address = base.0.checked_add(offset).ok_or_else(|| {
        GuestMemoryError::runtime_defect(
            "process-bootstrap",
            "initial-stack-address-overflow",
            "controlled initial stack address overflows 32 bits",
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

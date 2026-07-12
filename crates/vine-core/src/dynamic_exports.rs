use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::memory::{
    GUEST_PAGE_SIZE, GuestAddress, GuestMemory, GuestMemoryError, GuestRange, MemoryInvalidation,
    MemoryOwnerId, MemoryOwnerKind, MemoryRegionKind, PageProtection,
};

pub const DYNAMIC_EXPORT_ARENA_BASE: GuestAddress = GuestAddress(0x7f02_0000);
pub const DYNAMIC_EXPORT_ARENA_SIZE: u64 = 0x1_0000;
pub const DYNAMIC_EXPORT_SLOT_SIZE: u32 = 0x10;
pub const DYNAMIC_EXPORT_ENDPOINT_NAMESPACE: &str = "vine.dynamic-export-endpoint.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct DynamicExportThunkId(pub u32);

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DynamicExportThunk {
    pub namespace: &'static str,
    pub id: DynamicExportThunkId,
    pub address: GuestAddress,
    pub owner: MemoryOwnerId,
    pub arena_range: GuestRange,
    pub committed_pages: usize,
    pub slot_size: u32,
    pub module: String,
    pub name: String,
    pub lifetime: &'static str,
    pub content_sha256: String,
    pub owner_sealed: bool,
    pub invalidation: MemoryInvalidation,
}

pub fn install_dynamic_export_thunk(
    memory: &mut GuestMemory,
    module: &str,
    name: &str,
) -> Result<DynamicExportThunk, GuestMemoryError> {
    validate_identity(module, name)?;
    memory.preflight_growth(1, 1, 1, "dynamic-export")?;
    memory.preflight_fixed_reservation(
        DYNAMIC_EXPORT_ARENA_BASE,
        DYNAMIC_EXPORT_ARENA_SIZE,
        "dynamic-export",
    )?;

    let mut transaction = memory.begin_transaction();
    let owner = transaction.create_owner(MemoryOwnerKind::Runtime)?;
    let arena_range = transaction.reserve_fixed(
        owner,
        MemoryRegionKind::Runtime,
        DYNAMIC_EXPORT_ARENA_BASE,
        DYNAMIC_EXPORT_ARENA_SIZE,
    )?;
    transaction.commit(
        owner,
        GuestRange::new(DYNAMIC_EXPORT_ARENA_BASE, GUEST_PAGE_SIZE)?,
        PageProtection {
            read: true,
            write: false,
            execute: true,
        },
    )?;
    fill_halt_page(&mut transaction, owner)?;
    transaction.write_initialization(owner, DYNAMIC_EXPORT_ARENA_BASE, &[0x0f, 0x0b])?;
    let content_sha256 = hash_initialization_page(&transaction, owner)?;
    let invalidation = transaction.seal_owner(owner)?;
    let thunk = DynamicExportThunk {
        namespace: DYNAMIC_EXPORT_ENDPOINT_NAMESPACE,
        id: DynamicExportThunkId(1),
        address: DYNAMIC_EXPORT_ARENA_BASE,
        owner,
        arena_range,
        committed_pages: 1,
        slot_size: DYNAMIC_EXPORT_SLOT_SIZE,
        module: module.to_owned(),
        name: name.to_owned(),
        lifetime: "process-runtime",
        content_sha256,
        owner_sealed: true,
        invalidation,
    };
    transaction.commit_transaction();
    Ok(thunk)
}

fn validate_identity(module: &str, name: &str) -> Result<(), GuestMemoryError> {
    for (label, value) in [("module", module), ("name", name)] {
        if value.is_empty() || !value.is_ascii() || value.as_bytes().contains(&0) {
            return Err(GuestMemoryError::invalid_request(
                "dynamic-export",
                "dynamic-export-identity-invalid",
                format!("dynamic export {label} must be nonempty NUL-free ASCII"),
            ));
        }
    }
    Ok(())
}

fn fill_halt_page(memory: &mut GuestMemory, owner: MemoryOwnerId) -> Result<(), GuestMemoryError> {
    let page_size = GUEST_PAGE_SIZE as usize;
    let chunk_length = memory.limits().max_access_bytes.min(page_size);
    let halt_bytes = vec![0xf4; chunk_length];
    let mut offset = 0usize;
    while offset < page_size {
        let byte_length = (page_size - offset).min(chunk_length);
        let address = add_offset(offset)?;
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
        let address = add_offset(offset)?;
        hasher.update(memory.read_initialization(owner, address, byte_length)?);
        offset += byte_length;
    }
    Ok(hex_digest(&hasher.finalize()))
}

fn add_offset(offset: usize) -> Result<GuestAddress, GuestMemoryError> {
    let offset = u32::try_from(offset).map_err(|_| {
        GuestMemoryError::runtime_defect(
            "dynamic-export",
            "dynamic-export-offset-not-representable",
            "dynamic export arena offset does not fit 32 bits",
        )
    })?;
    let address = DYNAMIC_EXPORT_ARENA_BASE
        .0
        .checked_add(offset)
        .ok_or_else(|| {
            GuestMemoryError::runtime_defect(
                "dynamic-export",
                "dynamic-export-address-overflow",
                "dynamic export arena address overflows 32 bits",
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

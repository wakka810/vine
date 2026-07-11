use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::memory::{
    GUEST_PAGE_SIZE, GuestAddress, GuestMemory, GuestMemoryError, GuestRange, MemoryInvalidation,
    MemoryOwnerId, MemoryOwnerKind, MemoryRegionKind, PageProtection,
};

pub const CONTROLLED_TEB_BASE: GuestAddress = GuestAddress(0x7ffd_e000);
pub const CONTROLLED_PEB_BASE: GuestAddress = GuestAddress(0x7ffd_f000);
pub const CONTROLLED_ENVIRONMENT_PAGE_SIZE: u64 = GUEST_PAGE_SIZE;
pub const TEB_SELF_POINTER_OFFSET: u32 = 0x18;
pub const TEB_PEB_POINTER_OFFSET: u32 = 0x30;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InitialProcessEnvironment {
    pub teb_owner: MemoryOwnerId,
    pub peb_owner: MemoryOwnerId,
    pub teb_region_kind: MemoryRegionKind,
    pub peb_region_kind: MemoryRegionKind,
    pub teb_range: GuestRange,
    pub peb_range: GuestRange,
    pub committed_pages: usize,
    pub teb_address: GuestAddress,
    pub peb_address: GuestAddress,
    pub teb_self_pointer_offset: u32,
    pub teb_peb_pointer_offset: u32,
    pub teb_content_sha256: String,
    pub peb_content_sha256: String,
    pub owners_sealed: bool,
    pub invalidation: MemoryInvalidation,
}

pub fn install_controlled_process_environment(
    memory: &mut GuestMemory,
) -> Result<InitialProcessEnvironment, GuestMemoryError> {
    let (teb_owner, teb_range) = reserve_environment_page(
        memory,
        MemoryRegionKind::ThreadEnvironmentBlock,
        CONTROLLED_TEB_BASE,
    )?;
    let (peb_owner, peb_range) = reserve_environment_page(
        memory,
        MemoryRegionKind::ProcessEnvironmentBlock,
        CONTROLLED_PEB_BASE,
    )?;
    let protection = PageProtection {
        read: true,
        write: true,
        execute: false,
    };
    memory.commit(teb_owner, teb_range, protection)?;
    memory.commit(peb_owner, peb_range, protection)?;
    zero_environment_page(memory, teb_owner, CONTROLLED_TEB_BASE)?;
    zero_environment_page(memory, peb_owner, CONTROLLED_PEB_BASE)?;

    let teb_self_address = add_offset(CONTROLLED_TEB_BASE, TEB_SELF_POINTER_OFFSET)?;
    let teb_peb_address = add_offset(CONTROLLED_TEB_BASE, TEB_PEB_POINTER_OFFSET)?;
    memory.write_initialization(
        teb_owner,
        teb_self_address,
        &CONTROLLED_TEB_BASE.0.to_le_bytes(),
    )?;
    memory.write_initialization(
        teb_owner,
        teb_peb_address,
        &CONTROLLED_PEB_BASE.0.to_le_bytes(),
    )?;

    let teb_content_sha256 = hash_environment_page(memory, teb_owner, CONTROLLED_TEB_BASE)?;
    let peb_content_sha256 = hash_environment_page(memory, peb_owner, CONTROLLED_PEB_BASE)?;
    memory.seal_owner(peb_owner)?;
    let invalidation = memory.seal_owner(teb_owner)?;

    Ok(InitialProcessEnvironment {
        teb_owner,
        peb_owner,
        teb_region_kind: MemoryRegionKind::ThreadEnvironmentBlock,
        peb_region_kind: MemoryRegionKind::ProcessEnvironmentBlock,
        teb_range,
        peb_range,
        committed_pages: 2,
        teb_address: CONTROLLED_TEB_BASE,
        peb_address: CONTROLLED_PEB_BASE,
        teb_self_pointer_offset: TEB_SELF_POINTER_OFFSET,
        teb_peb_pointer_offset: TEB_PEB_POINTER_OFFSET,
        teb_content_sha256,
        peb_content_sha256,
        owners_sealed: true,
        invalidation,
    })
}

fn reserve_environment_page(
    memory: &mut GuestMemory,
    kind: MemoryRegionKind,
    address: GuestAddress,
) -> Result<(MemoryOwnerId, GuestRange), GuestMemoryError> {
    let owner = memory.create_owner(MemoryOwnerKind::Anonymous)?;
    match memory.reserve_fixed(owner, kind, address, CONTROLLED_ENVIRONMENT_PAGE_SIZE) {
        Ok(range) => Ok((owner, range)),
        Err(error) => {
            memory.destroy_owner(owner)?;
            Err(error)
        }
    }
}

fn zero_environment_page(
    memory: &mut GuestMemory,
    owner: MemoryOwnerId,
    base: GuestAddress,
) -> Result<(), GuestMemoryError> {
    let page_size = GUEST_PAGE_SIZE as usize;
    let chunk_length = memory.limits().max_access_bytes.min(page_size);
    let mut offset = 0usize;
    while offset < page_size {
        let byte_length = (page_size - offset).min(chunk_length);
        let address = add_offset_usize(base, offset)?;
        memory.zero_initialization(owner, address, byte_length)?;
        offset += byte_length;
    }
    Ok(())
}

fn hash_environment_page(
    memory: &GuestMemory,
    owner: MemoryOwnerId,
    base: GuestAddress,
) -> Result<String, GuestMemoryError> {
    let page_size = GUEST_PAGE_SIZE as usize;
    let chunk_length = memory.limits().max_access_bytes.min(page_size);
    let mut hasher = Sha256::new();
    let mut offset = 0usize;
    while offset < page_size {
        let byte_length = (page_size - offset).min(chunk_length);
        let address = add_offset_usize(base, offset)?;
        hasher.update(memory.read_initialization(owner, address, byte_length)?);
        offset += byte_length;
    }
    Ok(hex_digest(&hasher.finalize()))
}

fn add_offset(base: GuestAddress, offset: u32) -> Result<GuestAddress, GuestMemoryError> {
    let address = base.0.checked_add(offset).ok_or_else(|| {
        GuestMemoryError::runtime_defect(
            "process-bootstrap",
            "environment-field-address-overflow",
            "controlled process environment field address overflows 32 bits",
        )
    })?;
    Ok(GuestAddress(address))
}

fn add_offset_usize(base: GuestAddress, offset: usize) -> Result<GuestAddress, GuestMemoryError> {
    let offset = u32::try_from(offset).map_err(|_| {
        GuestMemoryError::runtime_defect(
            "process-bootstrap",
            "environment-page-offset-not-representable",
            "controlled process environment page offset does not fit 32 bits",
        )
    })?;
    add_offset(base, offset)
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

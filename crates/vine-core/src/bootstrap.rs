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
    pub committed_range: GuestRange,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub guard_page: Option<GuestRange>,
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
    install_initial_thread_stack(
        memory,
        entry_return,
        CONTROLLED_INITIAL_STACK_SIZE,
        CONTROLLED_INITIAL_STACK_SIZE,
    )
}

pub fn install_initial_thread_stack(
    memory: &mut GuestMemory,
    entry_return: &ProcessEntryReturnSentinel,
    reserve_size: u64,
    commit_size: u64,
) -> Result<InitialThreadStack, GuestMemoryError> {
    install_thread_stack_at(
        memory,
        entry_return,
        CONTROLLED_INITIAL_STACK_TOP,
        reserve_size,
        commit_size,
    )
}

pub fn install_thread_stack_at(
    memory: &mut GuestMemory,
    entry_return: &ProcessEntryReturnSentinel,
    stack_top: GuestAddress,
    reserve_size: u64,
    commit_size: u64,
) -> Result<InitialThreadStack, GuestMemoryError> {
    if !entry_return.owner_sealed {
        return Err(GuestMemoryError::runtime_defect(
            "process-bootstrap",
            "entry-return-sentinel-owner-unsealed",
            "initial stack requires a sealed process-entry return sentinel",
        ));
    }
    if stack_top.0 == 0
        || !stack_top.0.is_multiple_of(GUEST_PAGE_SIZE as u32)
        || reserve_size == 0
        || commit_size == 0
        || commit_size > reserve_size
        || !reserve_size.is_multiple_of(GUEST_PAGE_SIZE)
        || !commit_size.is_multiple_of(GUEST_PAGE_SIZE)
    {
        return Err(GuestMemoryError::invalid_request(
            "process-bootstrap",
            "initial-stack-layout-invalid",
            "thread stack top, reserve, and commit must be page aligned and nonzero with commit no larger than reserve",
        ));
    }
    let reserve_size_u32 = u32::try_from(reserve_size).map_err(|_| {
        GuestMemoryError::invalid_request(
            "process-bootstrap",
            "initial-stack-reserve-not-representable",
            "initial stack reserve size does not fit the x86 address space",
        )
    })?;
    let commit_size_u32 = u32::try_from(commit_size).map_err(|_| {
        GuestMemoryError::invalid_request(
            "process-bootstrap",
            "initial-stack-commit-not-representable",
            "initial stack commit size does not fit the x86 address space",
        )
    })?;
    let stack_base = GuestAddress(stack_top.0.checked_sub(reserve_size_u32).ok_or_else(|| {
        GuestMemoryError::invalid_request(
            "process-bootstrap",
            "initial-stack-reserve-underflow",
            "initial stack reserve extends below the x86 address-space origin",
        )
    })?);
    let committed_base =
        GuestAddress(stack_top.0.checked_sub(commit_size_u32).ok_or_else(|| {
            GuestMemoryError::invalid_request(
                "process-bootstrap",
                "initial-stack-commit-underflow",
                "initial stack commit extends below the x86 address-space origin",
            )
        })?);
    let committed_pages = usize::try_from(commit_size / GUEST_PAGE_SIZE).map_err(|_| {
        GuestMemoryError::invalid_request(
            "process-bootstrap",
            "initial-stack-page-count-not-representable",
            "initial stack commit page count does not fit host accounting",
        )
    })?;
    memory.preflight_growth(1, 1, committed_pages, "process-bootstrap")?;
    memory.preflight_fixed_reservation(stack_base, reserve_size, "process-bootstrap")?;
    let initial_esp = GuestAddress(stack_top.0.checked_sub(4).ok_or_else(|| {
        GuestMemoryError::runtime_defect(
            "process-bootstrap",
            "initial-stack-pointer-underflow",
            "controlled initial stack cannot hold its entry return address",
        )
    })?);

    let plan = InitialThreadStackPlan {
        stack_base,
        stack_top,
        reserve_size,
        committed_base,
        commit_size,
        committed_pages,
        initial_esp,
    };
    let mut transaction = memory.begin_transaction();
    let stack = install_initial_thread_stack_transaction(&mut transaction, entry_return, plan)?;
    transaction.commit_transaction();
    Ok(stack)
}

#[derive(Debug, Clone, Copy)]
struct InitialThreadStackPlan {
    stack_base: GuestAddress,
    stack_top: GuestAddress,
    reserve_size: u64,
    committed_base: GuestAddress,
    commit_size: u64,
    committed_pages: usize,
    initial_esp: GuestAddress,
}

fn install_initial_thread_stack_transaction(
    memory: &mut GuestMemory,
    entry_return: &ProcessEntryReturnSentinel,
    plan: InitialThreadStackPlan,
) -> Result<InitialThreadStack, GuestMemoryError> {
    let owner = memory.create_owner(MemoryOwnerKind::Anonymous)?;
    let stack_range = match memory.reserve_fixed(
        owner,
        MemoryRegionKind::Stack,
        plan.stack_base,
        plan.reserve_size,
    ) {
        Ok(range) => range,
        Err(error) => {
            memory.destroy_owner(owner)?;
            return Err(error);
        }
    };
    let committed_range = GuestRange::new(plan.committed_base, plan.commit_size)?;
    memory.commit(
        owner,
        committed_range,
        PageProtection {
            read: true,
            write: true,
            execute: false,
        },
    )?;
    memory.write_initialization(
        owner,
        plan.initial_esp,
        &entry_return.address.0.to_le_bytes(),
    )?;
    let content_sha256 = hash_initialization_range(memory, owner, committed_range)?;
    let invalidation = memory.seal_owner(owner)?;
    let guard_page = (plan.committed_base.0 > plan.stack_base.0)
        .then(|| {
            plan.committed_base
                .0
                .checked_sub(GUEST_PAGE_SIZE as u32)
                .map(GuestAddress)
                .ok_or_else(|| {
                    GuestMemoryError::runtime_defect(
                        "process-bootstrap",
                        "initial-stack-guard-underflow",
                        "initial stack growth guard underflowed the x86 address space",
                    )
                })
        })
        .transpose()?
        .map(|base| GuestRange::new(base, GUEST_PAGE_SIZE))
        .transpose()?;

    Ok(InitialThreadStack {
        owner,
        stack_range,
        committed_range,
        guard_page,
        committed_pages: plan.committed_pages,
        stack_top: plan.stack_top,
        initial_esp: plan.initial_esp,
        entry_return_sentinel_id: entry_return.id,
        entry_return_address: entry_return.address,
        content_sha256,
        owner_sealed: true,
        invalidation,
    })
}

fn hash_initialization_range(
    memory: &GuestMemory,
    owner: MemoryOwnerId,
    range: GuestRange,
) -> Result<String, GuestMemoryError> {
    let byte_length = usize::try_from(range.byte_length()).map_err(|_| {
        GuestMemoryError::runtime_defect(
            "process-bootstrap",
            "initial-stack-hash-size-not-representable",
            "initial stack committed size does not fit host accounting",
        )
    })?;
    let chunk_length = memory.limits().max_access_bytes.min(byte_length);
    let mut hasher = Sha256::new();
    let mut offset = 0usize;
    while offset < byte_length {
        let chunk_byte_length = (byte_length - offset).min(chunk_length);
        let address = add_offset(range.start(), offset)?;
        hasher.update(memory.read_initialization(owner, address, chunk_byte_length)?);
        offset += chunk_byte_length;
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

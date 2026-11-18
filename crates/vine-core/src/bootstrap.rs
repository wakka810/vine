use crate::callbacks::ProcessEntryReturnSentinel;
use crate::memory::{
    GUEST_PAGE_SIZE, GuestAddress, GuestMemory, GuestMemoryError, GuestRange, MemoryOwnerId,
    MemoryOwnerKind, MemoryRegionKind, PageProtection,
};

pub(crate) const INITIAL_THREAD_STACK_TOP: GuestAddress = GuestAddress(0x0070_0000);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitialThreadStack {
    pub owner: MemoryOwnerId,
    pub stack_range: GuestRange,
    pub committed_range: GuestRange,
    pub guard_page: Option<GuestRange>,
    pub initial_esp: GuestAddress,
    pub entry_return_address: GuestAddress,
}

pub fn install_initial_thread_stack(
    memory: &mut GuestMemory,
    entry_return: &ProcessEntryReturnSentinel,
    reserve_size: u64,
    commit_size: u64,
) -> Result<InitialThreadStack, GuestMemoryError> {
    install_initial_thread_stack_at(
        memory,
        entry_return,
        reserve_size,
        commit_size,
        INITIAL_THREAD_STACK_TOP,
    )
}

pub(crate) fn install_initial_thread_stack_at(
    memory: &mut GuestMemory,
    entry_return: &ProcessEntryReturnSentinel,
    reserve_size: u64,
    commit_size: u64,
    stack_top: GuestAddress,
) -> Result<InitialThreadStack, GuestMemoryError> {
    if !memory.owner_is_sealed(entry_return.owner)? {
        return Err(GuestMemoryError::runtime_defect(
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
            "thread stack top, reserve, and commit must be page aligned and nonzero with commit no larger than reserve",
        ));
    }
    let reserve_size_u32 = u32::try_from(reserve_size).map_err(|_| {
        GuestMemoryError::invalid_request(
            "initial stack reserve size does not fit the x86 address space",
        )
    })?;
    let commit_size_u32 = u32::try_from(commit_size).map_err(|_| {
        GuestMemoryError::invalid_request(
            "initial stack commit size does not fit the x86 address space",
        )
    })?;
    let stack_base = GuestAddress(stack_top.0.checked_sub(reserve_size_u32).ok_or_else(|| {
        GuestMemoryError::invalid_request(
            "initial stack reserve extends below the x86 address-space origin",
        )
    })?);
    let committed_base =
        GuestAddress(stack_top.0.checked_sub(commit_size_u32).ok_or_else(|| {
            GuestMemoryError::invalid_request(
                "initial stack commit extends below the x86 address-space origin",
            )
        })?);
    let committed_pages = usize::try_from(commit_size / GUEST_PAGE_SIZE).map_err(|_| {
        GuestMemoryError::invalid_request(
            "initial stack commit page count does not fit host accounting",
        )
    })?;
    memory.preflight_growth(1, 1, committed_pages)?;
    memory.preflight_fixed_reservation(stack_base, reserve_size)?;
    let initial_esp = GuestAddress(stack_top.0.checked_sub(4).ok_or_else(|| {
        GuestMemoryError::runtime_defect(
            "controlled initial stack cannot hold its entry return address",
        )
    })?);

    let committed_range = GuestRange::new(committed_base, commit_size)?;
    let guard_page = (committed_base.0 > stack_base.0)
        .then(|| {
            committed_base
                .0
                .checked_sub(GUEST_PAGE_SIZE as u32)
                .map(GuestAddress)
                .ok_or_else(|| {
                    GuestMemoryError::runtime_defect(
                        "initial stack growth guard underflowed the x86 address space",
                    )
                })
        })
        .transpose()?
        .map(|base| GuestRange::new(base, GUEST_PAGE_SIZE))
        .transpose()?;

    let mut transaction = memory.begin_transaction();
    let owner = transaction.create_owner(MemoryOwnerKind::Anonymous)?;
    let stack_range =
        transaction.reserve_fixed(owner, MemoryRegionKind::Stack, stack_base, reserve_size)?;
    transaction.commit(
        owner,
        committed_range,
        PageProtection {
            read: true,
            write: true,
            execute: false,
        },
    )?;
    transaction.write_initialization(owner, initial_esp, &entry_return.address.0.to_le_bytes())?;
    transaction.seal_owner(owner)?;

    let stack = InitialThreadStack {
        owner,
        stack_range,
        committed_range,
        guard_page,
        initial_esp,
        entry_return_address: entry_return.address,
    };
    transaction.commit_transaction();
    Ok(stack)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::callbacks::install_runtime_return_sentinels;
    use crate::memory::MemoryLimits;
    use crate::runtime::{ProcessAddressAllocationPurpose, ProcessAddressSpaceAllocator};

    #[test]
    fn initial_stack_requires_live_sealed_sentinel_owner_before_allocation() {
        let mut memory = GuestMemory::new(MemoryLimits::default()).unwrap();
        let allocator = ProcessAddressSpaceAllocator::new().unwrap();
        let allocation = allocator
            .allocation_for_purpose(&ProcessAddressAllocationPurpose::CallbackReturns)
            .unwrap();
        let sentinels = install_runtime_return_sentinels(&mut memory, allocation).unwrap();
        let unsealed_owner = memory.create_owner(MemoryOwnerKind::Runtime).unwrap();
        let wrong_owner = ProcessEntryReturnSentinel {
            owner: unsealed_owner,
            address: sentinels.process_entry_return.address,
        };
        let before = memory.stats();
        assert!(
            install_initial_thread_stack(
                &mut memory,
                &wrong_owner,
                2 * GUEST_PAGE_SIZE,
                GUEST_PAGE_SIZE
            )
            .is_err()
        );
        assert_eq!(memory.stats(), before);
        let stack = install_initial_thread_stack(
            &mut memory,
            &sentinels.process_entry_return,
            2 * GUEST_PAGE_SIZE,
            GUEST_PAGE_SIZE,
        )
        .unwrap();
        assert_eq!(
            stack.entry_return_address,
            sentinels.process_entry_return.address
        );
        assert!(memory.owner_is_sealed(stack.owner).unwrap());
    }
}

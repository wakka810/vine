use crate::memory::{
    GuestAddress, GuestMemory, GuestRange, MemoryOwnerId, MemoryOwnerKind, MemoryRegionKind,
    PageProtection,
};
use crate::runtime::{
    RUNTIME_PRIVATE_HEAP_FREED_BYTE, RuntimeHeapAllocation, RuntimeHeapAllocationPlan,
    RuntimeHeapBackingPlan, RuntimeHeapDestructionPlan, RuntimeHeapFreeBackingPlan,
    RuntimeHeapInitialization, RuntimeHeapTeardown, RuntimeState, RuntimeStateError,
};

pub(crate) fn allocate_private_heap_block(
    memory: &mut GuestMemory,
    runtime: &mut RuntimeState,
    handle: u32,
    byte_length: u32,
    initialization: RuntimeHeapInitialization,
) -> Result<RuntimeHeapAllocation, RuntimeStateError> {
    let plan = runtime.plan_private_heap_allocation(handle, byte_length, initialization)?;
    let mut transaction = memory.begin_transaction();
    let memory_owner = prepare_allocation_memory(&mut transaction, &plan)?;
    if plan.backing.requires_eager_initialization() {
        fill_allocation(
            &mut transaction,
            plan.allocation.address,
            plan.allocation.reserved_byte_length,
            initialization.fill_byte(),
        )?;
    }
    let allocation = runtime.commit_private_heap_allocation(
        plan,
        plan.backing.creates_memory_owner().then_some(memory_owner),
    )?;
    transaction.commit_transaction();
    Ok(allocation)
}

pub(crate) fn free_private_heap_block(
    memory: &mut GuestMemory,
    runtime: &mut RuntimeState,
    handle: u32,
    address: GuestAddress,
) -> Result<RuntimeHeapAllocation, RuntimeStateError> {
    let plan = runtime.plan_private_heap_free(handle, address)?;
    let mut transaction = memory.begin_transaction();
    match &plan.backing {
        RuntimeHeapFreeBackingPlan::Small { .. } => fill_allocation(
            &mut transaction,
            plan.allocation.address,
            plan.allocation.reserved_byte_length,
            RUNTIME_PRIVATE_HEAP_FREED_BYTE,
        )?,
        RuntimeHeapFreeBackingPlan::Large { backing } => {
            transaction
                .release_region(backing.memory_owner, backing.range.start())
                .map_err(RuntimeStateError::from_memory)?;
            transaction
                .destroy_owner(backing.memory_owner)
                .map_err(RuntimeStateError::from_memory)?;
        }
    }
    let allocation = runtime.commit_private_heap_free(plan)?;
    transaction.commit_transaction();
    Ok(allocation)
}

pub(crate) fn destroy_private_heap(
    memory: &mut GuestMemory,
    runtime: &mut RuntimeState,
    handle: u32,
) -> Result<RuntimeHeapTeardown, RuntimeStateError> {
    let plan = runtime.plan_private_heap_destruction(handle)?;
    destroy_planned_heaps(memory, runtime, &[plan])
}

pub(crate) fn teardown_process_heaps(
    memory: &mut GuestMemory,
    runtime: &mut RuntimeState,
) -> Result<RuntimeHeapTeardown, RuntimeStateError> {
    let plans = runtime.plan_process_heap_teardown()?;
    destroy_planned_heaps(memory, runtime, &plans)
}

fn prepare_allocation_memory(
    memory: &mut GuestMemory,
    plan: &RuntimeHeapAllocationPlan,
) -> Result<MemoryOwnerId, RuntimeStateError> {
    match plan.backing {
        RuntimeHeapBackingPlan::New {
            address_allocation_id,
            range,
            initial_committed_bytes,
        } => {
            if plan.next_committed_bytes < initial_committed_bytes {
                return Err(RuntimeStateError::runtime_defect(
                    "runtime-heap-backing",
                    "heap-initial-commit-plan-invalid",
                    "private heap allocation plan commits less than the initial heap size",
                ));
            }
            let owner = memory
                .create_owner(MemoryOwnerKind::Anonymous)
                .map_err(RuntimeStateError::from_memory)?;
            memory
                .reserve_fixed_for_address_allocation(
                    address_allocation_id.0,
                    owner,
                    MemoryRegionKind::Heap,
                    range.start(),
                    range.byte_length(),
                )
                .map_err(RuntimeStateError::from_memory)?;
            if plan.next_committed_bytes != 0 {
                let commit_range =
                    GuestRange::new(range.start(), u64::from(plan.next_committed_bytes))
                        .map_err(RuntimeStateError::from_memory)?;
                memory
                    .commit(owner, commit_range, heap_page_protection())
                    .map_err(RuntimeStateError::from_memory)?;
            }
            Ok(owner)
        }
        RuntimeHeapBackingPlan::Existing {
            memory_owner,
            address_allocation_id: _,
            range,
            committed_bytes,
        } => {
            if plan.next_committed_bytes < committed_bytes {
                return Err(RuntimeStateError::runtime_defect(
                    "runtime-heap-backing",
                    "heap-commit-plan-regressed",
                    "private heap allocation plan regressed committed backing bytes",
                ));
            }
            let additional_bytes = plan.next_committed_bytes - committed_bytes;
            if additional_bytes != 0 {
                let start = range
                    .start()
                    .0
                    .checked_add(committed_bytes)
                    .map(GuestAddress)
                    .ok_or_else(|| {
                        RuntimeStateError::runtime_defect(
                            "runtime-heap-backing",
                            "heap-commit-address-overflow",
                            "private heap commit tail overflowed x86 accounting",
                        )
                    })?;
                let commit_range = GuestRange::new(start, u64::from(additional_bytes))
                    .map_err(RuntimeStateError::from_memory)?;
                memory
                    .commit(memory_owner, commit_range, heap_page_protection())
                    .map_err(RuntimeStateError::from_memory)?;
            }
            Ok(memory_owner)
        }
        RuntimeHeapBackingPlan::LargeNew {
            address_allocation_id,
            range,
        } => {
            if plan.next_committed_bytes != 0 {
                return Err(RuntimeStateError::runtime_defect(
                    "runtime-heap-backing",
                    "large-heap-eager-commit-plan-invalid",
                    "large private heap allocation must reserve without eager commit",
                ));
            }
            let owner = memory
                .create_owner(MemoryOwnerKind::Anonymous)
                .map_err(RuntimeStateError::from_memory)?;
            memory
                .reserve_fixed_for_address_allocation(
                    address_allocation_id.0,
                    owner,
                    MemoryRegionKind::Heap,
                    range.start(),
                    range.byte_length(),
                )
                .map_err(RuntimeStateError::from_memory)?;
            Ok(owner)
        }
    }
}

fn fill_allocation(
    memory: &mut GuestMemory,
    address: GuestAddress,
    byte_length: u32,
    fill_byte: u8,
) -> Result<(), RuntimeStateError> {
    let byte_length = usize::try_from(byte_length).map_err(|_| {
        RuntimeStateError::runtime_defect(
            "runtime-heap-backing",
            "heap-allocation-size-not-representable",
            "private heap allocation size does not fit host accounting",
        )
    })?;
    let bytes = vec![fill_byte; byte_length];
    memory
        .write(address, &bytes)
        .map_err(RuntimeStateError::from_memory)?;
    Ok(())
}

fn destroy_planned_heaps(
    memory: &mut GuestMemory,
    runtime: &mut RuntimeState,
    plans: &[RuntimeHeapDestructionPlan],
) -> Result<RuntimeHeapTeardown, RuntimeStateError> {
    let mut transaction = memory.begin_transaction();
    for plan in plans {
        if let Some((_address_allocation_id, owner, range, _)) = plan.small_backing {
            transaction
                .release_region(owner, range.start())
                .map_err(RuntimeStateError::from_memory)?;
            transaction
                .destroy_owner(owner)
                .map_err(RuntimeStateError::from_memory)?;
        }
        if let Some(backing) = plan.large_backing.as_ref() {
            transaction
                .release_region(backing.memory_owner, backing.range.start())
                .map_err(RuntimeStateError::from_memory)?;
            transaction
                .destroy_owner(backing.memory_owner)
                .map_err(RuntimeStateError::from_memory)?;
        }
    }
    let teardown = runtime.commit_private_heap_destruction(plans)?;
    transaction.commit_transaction();
    Ok(teardown)
}

const fn heap_page_protection() -> PageProtection {
    PageProtection {
        read: true,
        write: true,
        execute: false,
    }
}

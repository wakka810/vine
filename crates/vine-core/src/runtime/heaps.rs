use std::collections::BTreeMap;

use serde::Serialize;

use super::{
    LOGICAL_PRIVATE_HEAP_NAMESPACE, ProcessAddressAllocationClassification,
    ProcessAddressAllocationPurpose, ProcessAddressSpaceAllocator, RUNTIME_PRIVATE_HEAP_ALIGNMENT,
    RUNTIME_PRIVATE_HEAP_ARENA_STRIDE, RUNTIME_PRIVATE_HEAP_BACKING_LIMIT,
    RUNTIME_PRIVATE_HEAP_MAX_LIVE_ALLOCATIONS, RuntimeAddressRange, RuntimeHandle, RuntimeHeap,
    RuntimeHeapAllocation, RuntimeHeapAllocationId, RuntimeHeapAllocationPlan, RuntimeHeapBacking,
    RuntimeHeapBackingPlan, RuntimeHeapDestructionPlan, RuntimeHeapFreePlan,
    RuntimeHeapInitialization, RuntimeHeapTeardown, RuntimeStateError,
};
use crate::memory::{GUEST_PAGE_SIZE, GuestAddress, GuestRange, MemoryOwnerId};

const LOGICAL_PRIVATE_HEAP_HANDLE_PREFIX: u32 = 0x6801_0000;
const RUNTIME_PRIVATE_HEAP_OWNER: &str = "runtime-process-private-heaps";
const RUNTIME_PRIVATE_HEAP_VARIANT: &str = "heap-create-metadata-v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RuntimeHeaps {
    heaps: BTreeMap<u32, RuntimeHeap>,
    #[serde(skip)]
    backings: BTreeMap<u32, RuntimeHeapBacking>,
    #[serde(skip)]
    next_private_heap_slot: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeHeapCreationPlan {
    heap: RuntimeHeap,
    slot_allocator: Option<RuntimePrivateHeapSlotPlan>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RuntimePrivateHeapSlotPlan {
    previous_next_slot: u16,
    next_slot: u16,
}

impl Default for RuntimeHeaps {
    fn default() -> Self {
        Self {
            heaps: BTreeMap::new(),
            backings: BTreeMap::new(),
            next_private_heap_slot: 1,
        }
    }
}

impl RuntimeHeapCreationPlan {
    pub(super) const fn handle(&self) -> u32 {
        self.heap.handle
    }
}

impl RuntimeHeaps {
    pub(super) fn heaps(&self) -> &BTreeMap<u32, RuntimeHeap> {
        &self.heaps
    }

    pub(super) fn is_empty(&self) -> bool {
        self.heaps.is_empty()
    }

    pub(super) fn heap(&self, handle: u32) -> Result<&RuntimeHeap, RuntimeStateError> {
        self.heaps.get(&handle).ok_or_else(|| {
            RuntimeStateError::not_found(
                "runtime-heap",
                "heap-handle-unknown",
                format!("logical heap handle {handle:#010x} is not live"),
            )
        })
    }

    pub(super) fn backing(&self, handle: u32) -> Option<&RuntimeHeapBacking> {
        self.backings.get(&handle)
    }

    pub(super) fn handles(&self) -> impl Iterator<Item = u32> + '_ {
        self.heaps.keys().copied()
    }

    pub(super) const fn next_private_heap_slot(&self) -> u16 {
        self.next_private_heap_slot
    }

    pub(super) fn plan_creation(
        &self,
        heap: RuntimeHeap,
    ) -> Result<RuntimeHeapCreationPlan, RuntimeStateError> {
        self.validate_creation(&heap)?;
        Ok(RuntimeHeapCreationPlan {
            heap,
            slot_allocator: None,
        })
    }

    pub(super) fn plan_private_creation(
        &self,
        creation_flags: u32,
        initial_size: u32,
        maximum_size: u32,
    ) -> Result<RuntimeHeapCreationPlan, RuntimeStateError> {
        let previous_next_slot = self.next_private_heap_slot;
        let mut candidate_slot = previous_next_slot;
        loop {
            if candidate_slot == 0 {
                return Err(RuntimeStateError::resource_limit(
                    "runtime-heap",
                    "private-heap-handle-space-exhausted",
                    "logical private heap handle space is exhausted",
                ));
            }
            let next_slot = candidate_slot.checked_add(1).unwrap_or(0);
            let handle = LOGICAL_PRIVATE_HEAP_HANDLE_PREFIX | u32::from(candidate_slot);
            if self.heaps.contains_key(&handle) {
                candidate_slot = next_slot;
                continue;
            }
            let heap = RuntimeHeap {
                owner: RUNTIME_PRIVATE_HEAP_OWNER.to_owned(),
                variant_id: RUNTIME_PRIVATE_HEAP_VARIANT.to_owned(),
                namespace: LOGICAL_PRIVATE_HEAP_NAMESPACE.to_owned(),
                handle,
                creation_flags,
                initial_size,
                maximum_size,
                growable: maximum_size == 0,
                live_allocations: 0,
            };
            self.validate_creation(&heap)?;
            return Ok(RuntimeHeapCreationPlan {
                heap,
                slot_allocator: Some(RuntimePrivateHeapSlotPlan {
                    previous_next_slot,
                    next_slot,
                }),
            });
        }
    }

    fn validate_creation(&self, heap: &RuntimeHeap) -> Result<(), RuntimeStateError> {
        if heap.owner.is_empty()
            || heap.variant_id.is_empty()
            || heap.namespace != LOGICAL_PRIVATE_HEAP_NAMESPACE
            || heap.handle == 0
        {
            return Err(RuntimeStateError::invalid_input(
                "runtime-heap",
                "heap-identity-invalid",
                "runtime heap requires controlled owner, variant, namespace, and nonzero handle",
            ));
        }
        if self.heaps.contains_key(&heap.handle) {
            return Err(RuntimeStateError::conflict(
                "runtime-heap",
                "heap-handle-duplicate",
                "runtime heap handle is already live",
            ));
        }
        if self.backings.contains_key(&heap.handle) {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-heap-backing",
                "heap-backing-without-heap",
                "private heap backing exists without logical heap metadata",
            ));
        }
        Ok(())
    }

    pub(super) fn commit_creation(&mut self, plan: RuntimeHeapCreationPlan) {
        let RuntimeHeapCreationPlan {
            heap,
            slot_allocator,
        } = plan;
        let handle = heap.handle;
        debug_assert!(!self.heaps.contains_key(&handle));
        if let Some(slot_allocator) = slot_allocator {
            debug_assert_eq!(
                self.next_private_heap_slot,
                slot_allocator.previous_next_slot
            );
        }
        let previous = self.heaps.insert(handle, heap);
        debug_assert!(previous.is_none());
        if let Some(slot_allocator) = slot_allocator {
            self.next_private_heap_slot = slot_allocator.next_slot;
        }
    }

    pub(super) fn allocation(
        &self,
        handle: u32,
        address: GuestAddress,
    ) -> Result<RuntimeHeapAllocation, RuntimeStateError> {
        let heap = self.heap(handle)?;
        let backing = self.backing(handle).ok_or_else(|| {
            RuntimeStateError::not_found(
                "runtime-heap-backing",
                "heap-allocation-pointer-invalid",
                "private heap has no backing containing the requested pointer",
            )
        })?;
        validate_runtime_heap_backing(heap, backing)?;
        backing.allocations.get(&address.0).copied().ok_or_else(|| {
            RuntimeStateError::not_found(
                "runtime-heap-backing",
                "heap-allocation-pointer-invalid",
                "private heap pointer does not identify a live allocation",
            )
        })
    }

    pub(super) fn plan_allocation(
        &self,
        allocator: &ProcessAddressSpaceAllocator,
        handle: u32,
        byte_length: u32,
        initialization: RuntimeHeapInitialization,
    ) -> Result<RuntimeHeapAllocationPlan, RuntimeStateError> {
        if byte_length == 0 {
            return Err(RuntimeStateError::invalid_input(
                "runtime-heap-backing",
                "heap-allocation-size-zero",
                "private heap backing does not accept zero-byte allocations",
            ));
        }
        let heap = self.heap(handle)?;
        let reserved_byte_length = align_runtime_heap_value(
            byte_length,
            RUNTIME_PRIVATE_HEAP_ALIGNMENT,
            "heap-allocation-size-overflow",
        )?;
        if usize::try_from(heap.live_allocations).map_err(|_| {
            RuntimeStateError::runtime_defect(
                "runtime-heap-backing",
                "heap-live-allocation-count-not-representable",
                "heap live allocation count does not fit host accounting",
            )
        })? >= RUNTIME_PRIVATE_HEAP_MAX_LIVE_ALLOCATIONS
        {
            return Err(RuntimeStateError::resource_limit(
                "runtime-heap-backing",
                "heap-live-allocation-limit-exceeded",
                format!(
                    "private heap allocation limit {} was reached",
                    RUNTIME_PRIVATE_HEAP_MAX_LIVE_ALLOCATIONS
                ),
            ));
        }

        let (
            backing_plan,
            allocation_id,
            allocation_offset,
            source_free_block,
            previous_high_water_mark,
            next_committed_bytes,
        ) = if let Some(backing) = self.backing(handle) {
            validate_heap_backing_address_allocation(allocator, handle, backing)?;
            validate_runtime_heap_backing(heap, backing)?;
            let allocation_id = backing.next_allocation_id;
            allocation_id.0.checked_add(1).ok_or_else(|| {
                RuntimeStateError::resource_limit(
                    "runtime-heap-backing",
                    "heap-allocation-id-space-exhausted",
                    "private heap allocation identifier space is exhausted",
                )
            })?;
            let source_free_block = backing
                .free_blocks
                .iter()
                .find(|(_, available)| **available >= reserved_byte_length)
                .map(|(offset, available)| (*offset, *available));
            let allocation_offset = source_free_block
                .map(|(offset, _)| offset)
                .unwrap_or(backing.high_water_mark);
            let allocation_end = allocation_offset
                .checked_add(reserved_byte_length)
                .ok_or_else(|| {
                    RuntimeStateError::resource_limit(
                        "runtime-heap-backing",
                        "heap-allocation-range-overflow",
                        "private heap allocation range overflowed x86 accounting",
                    )
                })?;
            let capacity = u32::try_from(backing.range.byte_length()).map_err(|_| {
                RuntimeStateError::runtime_defect(
                    "runtime-heap-backing",
                    "heap-backing-capacity-not-representable",
                    "private heap backing capacity does not fit x86 accounting",
                )
            })?;
            if allocation_end > capacity {
                return Err(RuntimeStateError::resource_limit(
                    "runtime-heap-backing",
                    "heap-backing-capacity-exceeded",
                    format!(
                        "allocation needs {allocation_end:#x} bytes in a {capacity:#x}-byte heap backing"
                    ),
                ));
            }
            let required_commit = align_runtime_heap_value(
                allocation_end,
                GUEST_PAGE_SIZE as u32,
                "heap-commit-size-overflow",
            )?;
            (
                RuntimeHeapBackingPlan::Existing {
                    memory_owner: backing.memory_owner,
                    address_allocation_id: backing.address_allocation_id,
                    range: backing.range,
                    committed_bytes: backing.committed_bytes,
                },
                allocation_id,
                allocation_offset,
                source_free_block,
                backing.high_water_mark,
                backing.committed_bytes.max(required_commit),
            )
        } else {
            if heap.live_allocations != 0 {
                return Err(RuntimeStateError::runtime_defect(
                    "runtime-heap-backing",
                    "heap-allocation-count-without-backing",
                    "logical heap has live allocations but no guest-memory backing",
                ));
            }
            let slot = RuntimeHandle(handle).slot();
            if slot == 0 || slot > RUNTIME_PRIVATE_HEAP_BACKING_LIMIT {
                return Err(RuntimeStateError::resource_limit(
                    "runtime-heap-backing",
                    "heap-backing-slot-limit-exceeded",
                    format!(
                        "heap handle slot {slot} exceeds the {}-backing controlled arena",
                        RUNTIME_PRIVATE_HEAP_BACKING_LIMIT
                    ),
                ));
            }
            let address_allocation = allocator.allocation_for_purpose(
                &ProcessAddressAllocationPurpose::PrivateHeapSlot { slot },
            )?;
            let base = address_allocation.range.base;
            if address_allocation.usable_byte_length != u64::from(RUNTIME_PRIVATE_HEAP_ARENA_STRIDE)
                || address_allocation.classification != ProcessAddressAllocationClassification::Heap
            {
                return Err(RuntimeStateError::runtime_defect(
                    "runtime-heap-backing",
                    "heap-address-allocation-invalid",
                    "private heap slot allocation record does not match v1 heap policy",
                ));
            }
            let capacity = if heap.maximum_size == 0 {
                RUNTIME_PRIVATE_HEAP_ARENA_STRIDE
            } else {
                align_runtime_heap_value(
                    heap.maximum_size,
                    GUEST_PAGE_SIZE as u32,
                    "heap-maximum-size-overflow",
                )?
            };
            if capacity > RUNTIME_PRIVATE_HEAP_ARENA_STRIDE {
                return Err(RuntimeStateError::resource_limit(
                    "runtime-heap-backing",
                    "heap-backing-capacity-policy-exceeded",
                    format!(
                        "heap requests {capacity:#x} bytes; the controlled per-heap limit is {:#x}",
                        RUNTIME_PRIVATE_HEAP_ARENA_STRIDE
                    ),
                ));
            }
            let initial_committed_bytes = if heap.initial_size == 0 {
                0
            } else {
                align_runtime_heap_value(
                    heap.initial_size,
                    GUEST_PAGE_SIZE as u32,
                    "heap-initial-size-overflow",
                )?
            };
            if initial_committed_bytes > capacity {
                return Err(RuntimeStateError::invalid_input(
                    "runtime-heap-backing",
                    "heap-initial-size-exceeds-maximum",
                    "private heap initial size exceeds its bounded maximum size",
                ));
            }
            if reserved_byte_length > capacity {
                return Err(RuntimeStateError::resource_limit(
                    "runtime-heap-backing",
                    "heap-backing-capacity-exceeded",
                    "private heap allocation exceeds the controlled backing capacity",
                ));
            }
            let range = GuestRange::new(base, u64::from(capacity))
                .map_err(RuntimeStateError::from_memory)?;
            let required_commit = align_runtime_heap_value(
                reserved_byte_length,
                GUEST_PAGE_SIZE as u32,
                "heap-commit-size-overflow",
            )?;
            (
                RuntimeHeapBackingPlan::New {
                    address_allocation_id: address_allocation.id,
                    range,
                    initial_committed_bytes,
                },
                RuntimeHeapAllocationId(1),
                0,
                None,
                0,
                initial_committed_bytes.max(required_commit),
            )
        };

        let address = backing_plan
            .range()
            .start()
            .0
            .checked_add(allocation_offset)
            .map(GuestAddress)
            .ok_or_else(|| {
                RuntimeStateError::resource_limit(
                    "runtime-heap-backing",
                    "heap-allocation-address-overflow",
                    "private heap allocation address overflowed x86 accounting",
                )
            })?;
        Ok(RuntimeHeapAllocationPlan {
            handle,
            allocation: RuntimeHeapAllocation {
                id: allocation_id,
                address,
                byte_length,
                reserved_byte_length,
                initialization,
            },
            allocation_offset,
            backing: backing_plan,
            source_free_block,
            previous_high_water_mark,
            next_committed_bytes,
        })
    }

    pub(super) fn commit_allocation(
        &mut self,
        plan: RuntimeHeapAllocationPlan,
        new_memory_owner: Option<MemoryOwnerId>,
    ) -> Result<RuntimeHeapAllocation, RuntimeStateError> {
        let mut next_heap = self.heap(plan.handle)?.clone();
        next_heap.live_allocations =
            next_heap.live_allocations.checked_add(1).ok_or_else(|| {
                RuntimeStateError::resource_limit(
                    "runtime-heap-backing",
                    "heap-live-allocation-count-overflow",
                    "private heap live allocation count overflowed",
                )
            })?;
        let allocation_end = plan
            .allocation_offset
            .checked_add(plan.allocation.reserved_byte_length)
            .ok_or_else(|| {
                RuntimeStateError::runtime_defect(
                    "runtime-heap-backing",
                    "planned-heap-allocation-range-overflow",
                    "planned private heap allocation range overflowed",
                )
            })?;
        let next_allocation_id =
            RuntimeHeapAllocationId(plan.allocation.id.0.checked_add(1).ok_or_else(|| {
                RuntimeStateError::resource_limit(
                    "runtime-heap-backing",
                    "heap-allocation-id-space-exhausted",
                    "private heap allocation identifier space is exhausted",
                )
            })?);

        match plan.backing {
            RuntimeHeapBackingPlan::New {
                address_allocation_id,
                range,
                initial_committed_bytes: _,
            } => {
                if self.backings.contains_key(&plan.handle) {
                    return Err(RuntimeStateError::conflict(
                        "runtime-heap-backing",
                        "heap-backing-plan-stale",
                        "private heap backing appeared after allocation planning",
                    ));
                }
                let memory_owner = new_memory_owner.ok_or_else(|| {
                    RuntimeStateError::runtime_defect(
                        "runtime-heap-backing",
                        "heap-backing-memory-owner-missing",
                        "new private heap backing has no guest-memory owner",
                    )
                })?;
                let mut allocations = BTreeMap::new();
                allocations.insert(plan.allocation.address.0, plan.allocation);
                self.backings.insert(
                    plan.handle,
                    RuntimeHeapBacking {
                        memory_owner,
                        address_allocation_id,
                        range,
                        alignment: RUNTIME_PRIVATE_HEAP_ALIGNMENT,
                        committed_bytes: plan.next_committed_bytes,
                        high_water_mark: allocation_end,
                        next_allocation_id,
                        allocations,
                        free_blocks: BTreeMap::new(),
                    },
                );
            }
            RuntimeHeapBackingPlan::Existing {
                memory_owner,
                address_allocation_id,
                range,
                committed_bytes,
            } => {
                if new_memory_owner.is_some() {
                    return Err(RuntimeStateError::runtime_defect(
                        "runtime-heap-backing",
                        "heap-backing-memory-owner-unexpected",
                        "existing private heap allocation supplied a new memory owner",
                    ));
                }
                let backing = self.backings.get(&plan.handle).ok_or_else(|| {
                    RuntimeStateError::conflict(
                        "runtime-heap-backing",
                        "heap-backing-plan-stale",
                        "private heap backing disappeared after allocation planning",
                    )
                })?;
                if backing.memory_owner != memory_owner
                    || backing.address_allocation_id != address_allocation_id
                    || backing.range != range
                    || backing.committed_bytes != committed_bytes
                    || backing.high_water_mark != plan.previous_high_water_mark
                    || backing.next_allocation_id != plan.allocation.id
                    || backing.allocations.contains_key(&plan.allocation.address.0)
                    || plan.source_free_block.is_some_and(|block| {
                        backing.free_blocks.get(&block.0).copied() != Some(block.1)
                    })
                {
                    return Err(RuntimeStateError::conflict(
                        "runtime-heap-backing",
                        "heap-allocation-plan-stale",
                        "private heap allocation metadata changed after planning",
                    ));
                }
                let backing = self.backings.get_mut(&plan.handle).ok_or_else(|| {
                    RuntimeStateError::runtime_defect(
                        "runtime-heap-backing",
                        "preflighted-heap-backing-missing",
                        "preflighted private heap backing disappeared",
                    )
                })?;
                if let Some((free_offset, free_length)) = plan.source_free_block {
                    backing.free_blocks.remove(&free_offset);
                    let remaining = free_length - plan.allocation.reserved_byte_length;
                    if remaining != 0 {
                        backing.free_blocks.insert(allocation_end, remaining);
                    }
                } else {
                    backing.high_water_mark = allocation_end;
                }
                backing.committed_bytes = plan.next_committed_bytes;
                backing.next_allocation_id = next_allocation_id;
                backing
                    .allocations
                    .insert(plan.allocation.address.0, plan.allocation);
            }
        }
        self.heaps.insert(plan.handle, next_heap);
        Ok(plan.allocation)
    }

    pub(super) fn plan_free(
        &self,
        handle: u32,
        address: GuestAddress,
    ) -> Result<RuntimeHeapFreePlan, RuntimeStateError> {
        let heap = self.heap(handle)?;
        let backing = self.backing(handle).ok_or_else(|| {
            RuntimeStateError::not_found(
                "runtime-heap-backing",
                "heap-allocation-pointer-invalid",
                "private heap has no backing containing the requested pointer",
            )
        })?;
        validate_runtime_heap_backing(heap, backing)?;
        let allocation = match backing.allocations.get(&address.0).copied() {
            Some(allocation) => allocation,
            None => {
                let offset = address.0.checked_sub(backing.range.start().0);
                return Err(
                    if offset.is_some_and(|offset| backing.free_blocks.contains_key(&offset)) {
                        RuntimeStateError::conflict(
                            "runtime-heap-backing",
                            "heap-allocation-double-free",
                            "private heap pointer belongs to an already freed block",
                        )
                    } else {
                        RuntimeStateError::not_found(
                            "runtime-heap-backing",
                            "heap-allocation-pointer-invalid",
                            "private heap pointer does not identify a live allocation",
                        )
                    },
                );
            }
        };
        let offset = allocation
            .address
            .0
            .checked_sub(backing.range.start().0)
            .ok_or_else(|| {
                RuntimeStateError::runtime_defect(
                    "runtime-heap-backing",
                    "heap-allocation-outside-backing",
                    "live private heap allocation precedes its backing range",
                )
            })?;
        let allocation_end = offset
            .checked_add(allocation.reserved_byte_length)
            .ok_or_else(|| {
                RuntimeStateError::runtime_defect(
                    "runtime-heap-backing",
                    "heap-allocation-range-overflow",
                    "live private heap allocation range overflowed",
                )
            })?;
        let predecessor = backing
            .free_blocks
            .range(..offset)
            .next_back()
            .map(|(start, length)| (*start, *length));
        let successor = backing
            .free_blocks
            .range(offset..)
            .next()
            .map(|(start, length)| (*start, *length));
        if predecessor
            .is_some_and(|(start, length)| start.checked_add(length).is_none_or(|end| end > offset))
            || successor.is_some_and(|(start, _)| start < allocation_end)
        {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-heap-backing",
                "heap-free-block-overlap",
                "private heap free blocks overlap a live allocation",
            ));
        }
        let merge_predecessor =
            predecessor.filter(|(start, length)| start.checked_add(*length) == Some(offset));
        let merge_successor = successor.filter(|(start, _)| *start == allocation_end);
        let merged_start = merge_predecessor.map(|(start, _)| start).unwrap_or(offset);
        let merged_length = allocation
            .reserved_byte_length
            .checked_add(merge_predecessor.map(|(_, length)| length).unwrap_or(0))
            .and_then(|length| {
                length.checked_add(merge_successor.map(|(_, length)| length).unwrap_or(0))
            })
            .ok_or_else(|| {
                RuntimeStateError::runtime_defect(
                    "runtime-heap-backing",
                    "heap-free-block-size-overflow",
                    "coalesced private heap free block overflowed x86 accounting",
                )
            })?;
        Ok(RuntimeHeapFreePlan {
            handle,
            allocation,
            memory_owner: backing.memory_owner,
            merged_free_block: (merged_start, merged_length),
            predecessor: merge_predecessor,
            successor: merge_successor,
        })
    }

    pub(super) fn commit_free(
        &mut self,
        plan: RuntimeHeapFreePlan,
    ) -> Result<RuntimeHeapAllocation, RuntimeStateError> {
        let mut next_heap = self.heap(plan.handle)?.clone();
        if next_heap.live_allocations == 0 {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-heap-backing",
                "heap-live-allocation-count-underflow",
                "private heap free would underflow its live allocation count",
            ));
        }
        let backing = self.backings.get(&plan.handle).ok_or_else(|| {
            RuntimeStateError::conflict(
                "runtime-heap-backing",
                "heap-free-plan-stale",
                "private heap backing disappeared after free planning",
            )
        })?;
        if backing.memory_owner != plan.memory_owner
            || backing.allocations.get(&plan.allocation.address.0) != Some(&plan.allocation)
            || plan
                .predecessor
                .is_some_and(|block| backing.free_blocks.get(&block.0).copied() != Some(block.1))
            || plan
                .successor
                .is_some_and(|block| backing.free_blocks.get(&block.0).copied() != Some(block.1))
        {
            return Err(RuntimeStateError::conflict(
                "runtime-heap-backing",
                "heap-free-plan-stale",
                "private heap allocation metadata changed after free planning",
            ));
        }
        let backing = self.backings.get_mut(&plan.handle).ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-heap-backing",
                "preflighted-heap-backing-missing",
                "preflighted private heap backing disappeared before free commit",
            )
        })?;
        backing.allocations.remove(&plan.allocation.address.0);
        if let Some((offset, _)) = plan.predecessor {
            backing.free_blocks.remove(&offset);
        }
        if let Some((offset, _)) = plan.successor {
            backing.free_blocks.remove(&offset);
        }
        backing
            .free_blocks
            .insert(plan.merged_free_block.0, plan.merged_free_block.1);
        next_heap.live_allocations -= 1;
        self.heaps.insert(plan.handle, next_heap);
        Ok(plan.allocation)
    }

    pub(super) fn plan_destruction(
        &self,
        allocator: &ProcessAddressSpaceAllocator,
        handle: u32,
    ) -> Result<RuntimeHeapDestructionPlan, RuntimeStateError> {
        let heap = self.heap(handle)?;
        let backing = self.backing(handle);
        if let Some(backing) = backing {
            validate_heap_backing_address_allocation(allocator, handle, backing)?;
            validate_runtime_heap_backing(heap, backing)?;
        } else if heap.live_allocations != 0 {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-heap-backing",
                "heap-allocation-count-without-backing",
                "logical heap has live allocations but no guest-memory backing",
            ));
        }
        Ok(RuntimeHeapDestructionPlan {
            handle,
            backing: backing.map(|backing| {
                (
                    backing.address_allocation_id,
                    backing.memory_owner,
                    backing.range,
                    backing.committed_bytes,
                )
            }),
            live_allocations: backing.map_or(0, |backing| backing.allocations.len()),
        })
    }

    pub(super) fn plan_all_destructions(
        &self,
        allocator: &ProcessAddressSpaceAllocator,
    ) -> Result<Vec<RuntimeHeapDestructionPlan>, RuntimeStateError> {
        self.handles()
            .map(|handle| self.plan_destruction(allocator, handle))
            .collect()
    }

    pub(super) fn preflight_destruction(
        &self,
        plans: &[RuntimeHeapDestructionPlan],
    ) -> Result<RuntimeHeapTeardown, RuntimeStateError> {
        let mut handles = Vec::with_capacity(plans.len());
        let mut address_allocation_ids = Vec::with_capacity(plans.len());
        let mut allocations_discarded = 0usize;
        let mut reserved_bytes_released = 0u64;
        let mut committed_bytes_released = 0u64;
        for plan in plans {
            let heap = self.heap(plan.handle)?;
            let backing = self.backings.get(&plan.handle);
            let actual_backing = backing.map(|backing| {
                (
                    backing.address_allocation_id,
                    backing.memory_owner,
                    backing.range,
                    backing.committed_bytes,
                )
            });
            if actual_backing != plan.backing
                || backing.map_or(0, |backing| backing.allocations.len()) != plan.live_allocations
                || usize::try_from(heap.live_allocations).ok() != Some(plan.live_allocations)
            {
                return Err(RuntimeStateError::conflict(
                    "runtime-heap-backing",
                    "heap-destruction-plan-stale",
                    "private heap metadata changed after destruction planning",
                ));
            }
            handles.push(plan.handle);
            allocations_discarded = allocations_discarded
                .checked_add(plan.live_allocations)
                .ok_or_else(|| {
                    RuntimeStateError::resource_limit(
                        "runtime-heap-backing",
                        "heap-teardown-allocation-count-overflow",
                        "private heap teardown allocation count overflowed host accounting",
                    )
                })?;
            if let Some((address_allocation_id, _, range, committed_bytes)) = plan.backing {
                address_allocation_ids.push(address_allocation_id);
                reserved_bytes_released = reserved_bytes_released
                    .checked_add(range.byte_length())
                    .ok_or_else(|| {
                        RuntimeStateError::resource_limit(
                            "runtime-heap-backing",
                            "heap-teardown-reserved-byte-count-overflow",
                            "private heap teardown reserved bytes overflowed host accounting",
                        )
                    })?;
                committed_bytes_released = committed_bytes_released
                    .checked_add(u64::from(committed_bytes))
                    .ok_or_else(|| {
                        RuntimeStateError::resource_limit(
                            "runtime-heap-backing",
                            "heap-teardown-committed-byte-count-overflow",
                            "private heap teardown committed bytes overflowed host accounting",
                        )
                    })?;
            }
        }
        Ok(RuntimeHeapTeardown {
            handles,
            address_allocation_ids,
            allocations_discarded,
            reserved_bytes_released,
            committed_bytes_released,
        })
    }

    pub(super) fn commit_destruction(&mut self, teardown: &RuntimeHeapTeardown) {
        for handle in &teardown.handles {
            self.backings.remove(handle);
            let previous = self.heaps.remove(handle);
            debug_assert!(previous.is_some());
        }
    }
}

fn align_runtime_heap_value(
    value: u32,
    alignment: u32,
    overflow_code: &'static str,
) -> Result<u32, RuntimeStateError> {
    if alignment == 0 || !alignment.is_power_of_two() {
        return Err(RuntimeStateError::runtime_defect(
            "runtime-heap-backing",
            "heap-alignment-policy-invalid",
            "private heap alignment must be a nonzero power of two",
        ));
    }
    value
        .checked_add(alignment - 1)
        .map(|rounded| rounded & !(alignment - 1))
        .ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-heap-backing",
                overflow_code,
                "private heap value overflowed while applying alignment",
            )
        })
}

fn validate_heap_backing_address_allocation(
    allocator: &ProcessAddressSpaceAllocator,
    handle: u32,
    backing: &RuntimeHeapBacking,
) -> Result<(), RuntimeStateError> {
    let slot = RuntimeHandle(handle).slot();
    let allocation = allocator.allocation(backing.address_allocation_id)?;
    let backing_range = RuntimeAddressRange {
        base: backing.range.start(),
        byte_length: backing.range.byte_length(),
    };
    if allocation.purpose != (ProcessAddressAllocationPurpose::PrivateHeapSlot { slot })
        || allocation.classification != ProcessAddressAllocationClassification::Heap
        || !allocation.range.contains(backing_range)?
    {
        return Err(RuntimeStateError::runtime_defect(
            "runtime-heap-backing",
            "heap-address-allocation-mismatch",
            "private heap backing is not contained by its allocation record",
        ));
    }
    Ok(())
}

fn validate_runtime_heap_backing(
    heap: &RuntimeHeap,
    backing: &RuntimeHeapBacking,
) -> Result<(), RuntimeStateError> {
    let capacity = u32::try_from(backing.range.byte_length()).map_err(|_| {
        RuntimeStateError::runtime_defect(
            "runtime-heap-backing",
            "heap-backing-capacity-not-representable",
            "private heap backing capacity does not fit x86 accounting",
        )
    })?;
    if backing.memory_owner.0 == 0
        || backing.alignment != RUNTIME_PRIVATE_HEAP_ALIGNMENT
        || !backing
            .range
            .start()
            .0
            .is_multiple_of(GUEST_PAGE_SIZE as u32)
        || capacity == 0
        || capacity % GUEST_PAGE_SIZE as u32 != 0
        || capacity > RUNTIME_PRIVATE_HEAP_ARENA_STRIDE
        || !backing
            .committed_bytes
            .is_multiple_of(GUEST_PAGE_SIZE as u32)
        || backing.committed_bytes > capacity
        || backing.high_water_mark > backing.committed_bytes
        || backing.next_allocation_id.0 == 0
    {
        return Err(RuntimeStateError::runtime_defect(
            "runtime-heap-backing",
            "heap-backing-metadata-invalid",
            "private heap backing violates its controlled address or size policy",
        ));
    }
    if backing.allocations.len() != heap.live_allocations as usize
        || backing.allocations.len() > RUNTIME_PRIVATE_HEAP_MAX_LIVE_ALLOCATIONS
    {
        return Err(RuntimeStateError::runtime_defect(
            "runtime-heap-backing",
            "heap-live-allocation-count-mismatch",
            "logical heap and backing allocation counts disagree",
        ));
    }

    let mut intervals = Vec::with_capacity(backing.allocations.len() + backing.free_blocks.len());
    let mut allocation_ids = std::collections::BTreeSet::new();
    for (&address, allocation) in &backing.allocations {
        let offset = address
            .checked_sub(backing.range.start().0)
            .ok_or_else(|| {
                RuntimeStateError::runtime_defect(
                    "runtime-heap-backing",
                    "heap-allocation-outside-backing",
                    "private heap allocation precedes its backing range",
                )
            })?;
        let end = offset
            .checked_add(allocation.reserved_byte_length)
            .ok_or_else(|| {
                RuntimeStateError::runtime_defect(
                    "runtime-heap-backing",
                    "heap-allocation-range-overflow",
                    "private heap allocation range overflowed",
                )
            })?;
        if allocation.address.0 != address
            || allocation.id.0 == 0
            || allocation.id.0 >= backing.next_allocation_id.0
            || !allocation_ids.insert(allocation.id)
            || allocation.byte_length == 0
            || allocation.byte_length > allocation.reserved_byte_length
            || allocation.reserved_byte_length % backing.alignment != 0
            || offset % backing.alignment != 0
            || end > backing.high_water_mark
        {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-heap-backing",
                "heap-allocation-metadata-invalid",
                "private heap allocation violates identity, alignment, or range invariants",
            ));
        }
        intervals.push((offset, end));
    }
    for (&offset, &length) in &backing.free_blocks {
        let end = offset.checked_add(length).ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-heap-backing",
                "heap-free-block-range-overflow",
                "private heap free block range overflowed",
            )
        })?;
        if length == 0
            || offset % backing.alignment != 0
            || length % backing.alignment != 0
            || end > backing.high_water_mark
        {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-heap-backing",
                "heap-free-block-metadata-invalid",
                "private heap free block violates alignment or range invariants",
            ));
        }
        intervals.push((offset, end));
    }
    intervals.sort_unstable_by_key(|interval| interval.0);
    let mut cursor = 0u32;
    for (start, end) in intervals {
        if start != cursor || end <= start {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-heap-backing",
                "heap-backing-partition-invalid",
                "private heap live and free blocks do not partition the used arena",
            ));
        }
        cursor = end;
    }
    if cursor != backing.high_water_mark {
        return Err(RuntimeStateError::runtime_defect(
            "runtime-heap-backing",
            "heap-backing-high-water-mark-invalid",
            "private heap partition does not end at its high-water mark",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::{GuestAddress, GuestRange, MemoryOwnerId};
    use crate::runtime::{
        LOGICAL_PRIVATE_HEAP_NAMESPACE, ProcessAddressAllocationId, RuntimeHeapInitialization,
    };

    const HANDLE: u32 = 0x6801_0001;

    fn heap() -> RuntimeHeap {
        RuntimeHeap {
            owner: "runtime-process-private-heaps".to_owned(),
            variant_id: "heap-create-metadata-v1".to_owned(),
            namespace: LOGICAL_PRIVATE_HEAP_NAMESPACE.to_owned(),
            handle: HANDLE,
            creation_flags: 0,
            initial_size: 0,
            maximum_size: 0,
            growable: true,
            live_allocations: 0,
        }
    }

    #[test]
    fn creation_plan_is_mutation_free_and_serialization_shape_is_stable() {
        let mut heaps = RuntimeHeaps::default();
        let plan = heaps.plan_creation(heap()).unwrap();
        assert!(heaps.is_empty());
        assert_eq!(heaps.next_private_heap_slot(), 1);
        heaps.commit_creation(plan);
        assert_eq!(heaps.heap(HANDLE).unwrap().handle, HANDLE);
        assert_eq!(heaps.next_private_heap_slot(), 1);
        let serialized = serde_json::to_value(&heaps).unwrap();
        assert!(serialized.get("heaps").is_some());
        assert!(serialized.get("backings").is_none());
        assert!(serialized.get("nextPrivateHeapSlot").is_none());
    }

    #[test]
    fn automatic_creation_plan_commits_heap_and_slot_allocator_together() {
        let mut heaps = RuntimeHeaps::default();
        let before = heaps.clone();
        let plan = heaps.plan_private_creation(2, 0x1000, 0x2000).unwrap();

        assert_eq!(plan.handle(), HANDLE);
        assert_eq!(heaps, before, "planning must not consume a heap slot");

        heaps.commit_creation(plan);
        let created = heaps.heap(HANDLE).unwrap();
        assert_eq!(created.creation_flags, 2);
        assert_eq!(created.initial_size, 0x1000);
        assert_eq!(created.maximum_size, 0x2000);
        assert!(!created.growable);
        assert_eq!(heaps.next_private_heap_slot(), 2);
    }

    #[test]
    fn automatic_creation_skips_live_slots_without_advancing_on_explicit_creation() {
        let mut heaps = RuntimeHeaps::default();
        let explicit = heaps.plan_creation(heap()).unwrap();
        heaps.commit_creation(explicit);
        assert_eq!(heaps.next_private_heap_slot(), 1);

        let automatic = heaps.plan_private_creation(0, 0, 0).unwrap();
        assert_eq!(automatic.handle(), LOGICAL_PRIVATE_HEAP_HANDLE_PREFIX | 2);
        assert_eq!(heaps.next_private_heap_slot(), 1);

        heaps.commit_creation(automatic);
        assert_eq!(heaps.next_private_heap_slot(), 3);
    }

    #[test]
    fn exhausted_automatic_slot_allocator_rejects_without_mutation() {
        let mut heaps = RuntimeHeaps {
            next_private_heap_slot: u16::MAX,
            ..RuntimeHeaps::default()
        };
        let final_plan = heaps.plan_private_creation(0, 0, 0).unwrap();
        assert_eq!(
            final_plan.handle(),
            LOGICAL_PRIVATE_HEAP_HANDLE_PREFIX | u32::from(u16::MAX)
        );
        assert_eq!(heaps.next_private_heap_slot(), u16::MAX);
        heaps.commit_creation(final_plan);
        assert_eq!(heaps.next_private_heap_slot(), 0);

        let before = heaps.clone();
        assert_eq!(
            heaps.plan_private_creation(0, 0, 0).unwrap_err().code,
            "private-heap-handle-space-exhausted"
        );
        assert_eq!(heaps, before);
    }

    #[test]
    fn allocation_commit_owns_logical_and_backing_records_together() {
        let mut heaps = RuntimeHeaps::default();
        let creation = heaps.plan_creation(heap()).unwrap();
        heaps.commit_creation(creation);
        let range = GuestRange::new(GuestAddress(0x2000_0000), 0x1000).unwrap();
        let allocation = RuntimeHeapAllocation {
            id: RuntimeHeapAllocationId(1),
            address: range.start(),
            byte_length: 0x20,
            reserved_byte_length: 0x20,
            initialization: RuntimeHeapInitialization::Zeroed,
        };
        let plan = RuntimeHeapAllocationPlan {
            handle: HANDLE,
            allocation,
            allocation_offset: 0,
            backing: RuntimeHeapBackingPlan::New {
                address_allocation_id: ProcessAddressAllocationId(1),
                range,
                initial_committed_bytes: 0,
            },
            source_free_block: None,
            previous_high_water_mark: 0,
            next_committed_bytes: 0x1000,
        };
        assert_eq!(
            heaps.commit_allocation(plan, Some(MemoryOwnerId(1))),
            Ok(allocation)
        );
        assert_eq!(heaps.heap(HANDLE).unwrap().live_allocations, 1);
        assert_eq!(heaps.backing(HANDLE).unwrap().allocations.len(), 1);
    }

    #[test]
    fn stale_or_duplicate_plans_preserve_all_heap_collections() {
        let mut heaps = RuntimeHeaps::default();
        let creation = heaps.plan_creation(heap()).unwrap();
        heaps.commit_creation(creation);
        let before = heaps.clone();
        assert_eq!(
            heaps.plan_creation(heap()).unwrap_err().code,
            "heap-handle-duplicate"
        );
        assert_eq!(heaps, before);
    }
}

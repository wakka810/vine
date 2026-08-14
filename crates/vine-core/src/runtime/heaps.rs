use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use super::{
    ProcessAddressAllocationClassification, ProcessAddressAllocationPurpose,
    ProcessAddressSpaceAllocator, RuntimeAddressRange, RuntimeHandle, RuntimeStateError,
};
use crate::memory::{GUEST_PAGE_SIZE, GuestAddress, GuestRange, MemoryOwnerId};

pub const LOGICAL_PRIVATE_HEAP_NAMESPACE: &str = "vine.logical-private-heap.v1";
pub const RUNTIME_PRIVATE_HEAP_ARENA_BASE: GuestAddress = GuestAddress(0x2000_0000);
pub const RUNTIME_PRIVATE_HEAP_ARENA_STRIDE: u32 = 0x0100_0000;
pub const RUNTIME_PRIVATE_HEAP_BACKING_LIMIT: u16 = 32;
pub const RUNTIME_PRIVATE_HEAP_MAX_ORDINARY_SEGMENTS: usize =
    RUNTIME_PRIVATE_HEAP_BACKING_LIMIT as usize;
pub const RUNTIME_PRIVATE_HEAP_LARGE_SLOT_BASE: GuestAddress = GuestAddress(0x4000_0000);
pub const RUNTIME_PRIVATE_HEAP_LARGE_SLOT_SIZE: u32 = 0x0400_0000;
pub const RUNTIME_PRIVATE_HEAP_ALIGNMENT: u32 = 8;
pub const RUNTIME_PRIVATE_HEAP_UNINITIALIZED_BYTE: u8 = 0xa5;
pub const RUNTIME_PRIVATE_HEAP_FREED_BYTE: u8 = 0xdd;
pub const RUNTIME_PRIVATE_HEAP_MAX_LIVE_ALLOCATIONS: usize = 65_536;

const LOGICAL_PRIVATE_HEAP_HANDLE_PREFIX: u32 = 0x6801_0000;
const RUNTIME_PRIVATE_HEAP_OWNER: &str = "runtime-process-private-heaps";
const RUNTIME_PRIVATE_HEAP_VARIANT: &str = "heap-create-metadata-v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeHeap {
    pub owner: String,
    pub variant_id: String,
    pub namespace: String,
    pub handle: u32,
    pub creation_flags: u32,
    pub initial_size: u32,
    pub maximum_size: u32,
    pub growable: bool,
    pub live_allocations: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct RuntimeHeapAllocationId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeHeapInitialization {
    Zeroed,
    DeterministicUninitialized,
}

impl RuntimeHeapInitialization {
    pub const fn fill_byte(self) -> u8 {
        match self {
            Self::Zeroed => 0,
            Self::DeterministicUninitialized => RUNTIME_PRIVATE_HEAP_UNINITIALIZED_BYTE,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeHeapAllocation {
    pub id: RuntimeHeapAllocationId,
    pub address: GuestAddress,
    pub byte_length: u32,
    pub reserved_byte_length: u32,
    pub initialization: RuntimeHeapInitialization,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeHeapBacking {
    pub memory_owner: MemoryOwnerId,
    pub address_allocation_id: super::ProcessAddressAllocationId,
    pub range: GuestRange,
    pub alignment: u32,
    pub committed_bytes: u32,
    pub high_water_mark: u32,
    pub allocations: BTreeMap<u32, RuntimeHeapAllocation>,
    pub free_blocks: BTreeMap<u32, u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeLargeHeapAllocationBacking {
    pub handle: u32,
    pub memory_owner: MemoryOwnerId,
    pub address_allocation_id: super::ProcessAddressAllocationId,
    pub range: GuestRange,
    pub allocation: RuntimeHeapAllocation,
    pub materialized_pages: BTreeSet<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeHeapPageMaterialization {
    pub handle: u32,
    pub allocation: RuntimeHeapAllocation,
    pub memory_owner: MemoryOwnerId,
    pub address_allocation_id: super::ProcessAddressAllocationId,
    pub fault_address: GuestAddress,
    pub page_fault_error_code: u32,
    pub committed_page: GuestRange,
    pub materialized_pages: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeHeapTeardown {
    pub handles: Vec<u32>,
    pub address_allocation_ids: Vec<super::ProcessAddressAllocationId>,
    pub allocations_discarded: usize,
    pub reserved_bytes_released: u64,
    pub committed_bytes_released: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RuntimeHeapBackingKind {
    Primary,
    Extension { slot: u16 },
}

impl RuntimeHeapBackingKind {
    const fn slot(self, handle: u32) -> u16 {
        match self {
            Self::Primary => RuntimeHandle(handle).slot(),
            Self::Extension { slot } => slot,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum RuntimeHeapBackingPlan {
    New {
        kind: RuntimeHeapBackingKind,
        address_allocation_id: super::ProcessAddressAllocationId,
        range: GuestRange,
        initial_committed_bytes: u32,
    },
    Existing {
        kind: RuntimeHeapBackingKind,
        memory_owner: MemoryOwnerId,
        address_allocation_id: super::ProcessAddressAllocationId,
        range: GuestRange,
        committed_bytes: u32,
    },
    LargeNew {
        address_allocation_id: super::ProcessAddressAllocationId,
        range: GuestRange,
    },
}

type RuntimeExistingHeapAllocationPlan =
    (RuntimeHeapBackingPlan, u32, Option<(u32, u32)>, u32, u32);

impl RuntimeHeapBackingPlan {
    pub(crate) const fn range(self) -> GuestRange {
        match self {
            Self::New { range, .. }
            | Self::Existing { range, .. }
            | Self::LargeNew { range, .. } => range,
        }
    }

    pub(crate) const fn requires_eager_initialization(self) -> bool {
        !matches!(self, Self::LargeNew { .. })
    }

    pub(crate) const fn creates_memory_owner(self) -> bool {
        matches!(self, Self::New { .. } | Self::LargeNew { .. })
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct RuntimeHeapAllocationPlan {
    pub handle: u32,
    pub allocation: RuntimeHeapAllocation,
    pub allocation_offset: u32,
    pub backing: RuntimeHeapBackingPlan,
    pub source_free_block: Option<(u32, u32)>,
    pub previous_high_water_mark: u32,
    pub next_committed_bytes: u32,
}

#[derive(Debug, Clone)]
pub(crate) struct RuntimeHeapFreePlan {
    pub handle: u32,
    pub allocation: RuntimeHeapAllocation,
    pub backing: RuntimeHeapFreeBackingPlan,
}

#[derive(Debug, Clone)]
pub(crate) enum RuntimeHeapFreeBackingPlan {
    Small {
        kind: RuntimeHeapBackingKind,
        memory_owner: MemoryOwnerId,
        merged_free_block: (u32, u32),
        predecessor: Option<(u32, u32)>,
        successor: Option<(u32, u32)>,
    },
    Large {
        backing: RuntimeLargeHeapAllocationBacking,
    },
}

#[derive(Debug, Clone)]
pub(crate) struct RuntimeHeapDestructionPlan {
    pub handle: u32,
    pub small_backings: Vec<(
        super::ProcessAddressAllocationId,
        MemoryOwnerId,
        GuestRange,
        u32,
    )>,
    pub large_backing: Option<RuntimeLargeHeapAllocationBacking>,
    pub live_allocations: usize,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct RuntimeHeapPageMaterializationPlan {
    pub handle: u32,
    pub allocation: RuntimeHeapAllocation,
    pub memory_owner: MemoryOwnerId,
    pub address_allocation_id: super::ProcessAddressAllocationId,
    pub fault_address: GuestAddress,
    pub page_fault_error_code: u32,
    pub committed_page: GuestRange,
    previous_materialized_pages: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RuntimeHeaps {
    heaps: BTreeMap<u32, RuntimeHeap>,
    #[serde(skip)]
    backings: BTreeMap<u32, RuntimeHeapBacking>,
    #[serde(skip)]
    extension_backings: BTreeMap<u32, BTreeMap<u16, RuntimeHeapBacking>>,
    #[serde(skip)]
    large_backing: Option<RuntimeLargeHeapAllocationBacking>,
    #[serde(skip)]
    next_allocation_ids: BTreeMap<u32, RuntimeHeapAllocationId>,
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
            extension_backings: BTreeMap::new(),
            large_backing: None,
            next_allocation_ids: BTreeMap::new(),
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

    pub(super) fn extension_backing(&self, handle: u32) -> Option<&RuntimeHeapBacking> {
        self.extension_backings
            .get(&handle)
            .and_then(|backings| backings.values().next_back())
    }

    fn extension_backings(
        &self,
        handle: u32,
    ) -> impl DoubleEndedIterator<Item = (u16, &RuntimeHeapBacking)> {
        self.extension_backings
            .get(&handle)
            .into_iter()
            .flat_map(|backings| backings.iter().map(|(&slot, backing)| (slot, backing)))
    }

    fn backing_for_kind(
        &self,
        handle: u32,
        kind: RuntimeHeapBackingKind,
    ) -> Option<&RuntimeHeapBacking> {
        match kind {
            RuntimeHeapBackingKind::Primary => self.backings.get(&handle),
            RuntimeHeapBackingKind::Extension { slot } => self
                .extension_backings
                .get(&handle)
                .and_then(|backings| backings.get(&slot)),
        }
    }

    fn backing_for_kind_mut(
        &mut self,
        handle: u32,
        kind: RuntimeHeapBackingKind,
    ) -> Option<&mut RuntimeHeapBacking> {
        match kind {
            RuntimeHeapBackingKind::Primary => self.backings.get_mut(&handle),
            RuntimeHeapBackingKind::Extension { slot } => self
                .extension_backings
                .get_mut(&handle)
                .and_then(|backings| backings.get_mut(&slot)),
        }
    }

    pub(super) fn large_backing(&self) -> Option<&RuntimeLargeHeapAllocationBacking> {
        self.large_backing.as_ref()
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
            if candidate_slot == 0 || candidate_slot > RUNTIME_PRIVATE_HEAP_BACKING_LIMIT {
                return Err(RuntimeStateError::resource_limit(
                    "runtime-heap",
                    "private-heap-handle-space-exhausted",
                    "logical private heap handle and backing slot space is exhausted",
                ));
            }
            let next_slot = if candidate_slot == RUNTIME_PRIVATE_HEAP_BACKING_LIMIT {
                0
            } else {
                candidate_slot + 1
            };
            let handle = LOGICAL_PRIVATE_HEAP_HANDLE_PREFIX | u32::from(candidate_slot);
            if self.heaps.contains_key(&handle) || self.extension_slot_claimed(candidate_slot) {
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
        if self.extension_backings.contains_key(&heap.handle) {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-heap-backing",
                "heap-extension-backing-without-heap",
                "private heap extension backing exists without logical heap metadata",
            ));
        }
        let slot = RuntimeHandle(heap.handle).slot();
        if slot == 0
            || slot > RUNTIME_PRIVATE_HEAP_BACKING_LIMIT
            || self.extension_slot_claimed(slot)
        {
            return Err(RuntimeStateError::resource_limit(
                "runtime-heap",
                "private-heap-handle-slot-unavailable",
                "logical private heap handle does not own an available controlled backing slot",
            ));
        }
        if self.next_allocation_ids.contains_key(&heap.handle)
            || self
                .large_backing
                .as_ref()
                .is_some_and(|backing| backing.handle == heap.handle)
        {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-heap-backing",
                "heap-allocation-state-without-heap",
                "private heap allocation state exists without logical heap metadata",
            ));
        }
        Ok(())
    }

    fn extension_slot_claimed(&self, slot: u16) -> bool {
        self.extension_backings
            .values()
            .any(|backings| backings.contains_key(&slot))
    }

    fn slot_claimed(&self, slot: u16) -> bool {
        let handle = LOGICAL_PRIVATE_HEAP_HANDLE_PREFIX | u32::from(slot);
        self.heaps.contains_key(&handle) || self.extension_slot_claimed(slot)
    }

    fn available_extension_slot(&self) -> Option<u16> {
        (1..=RUNTIME_PRIVATE_HEAP_BACKING_LIMIT)
            .rev()
            .find(|slot| !self.slot_claimed(*slot))
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
        let previous_allocation_id = self
            .next_allocation_ids
            .insert(handle, RuntimeHeapAllocationId(1));
        debug_assert!(previous_allocation_id.is_none());
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
        self.validate_allocation_count(heap)?;
        let next_allocation_id = self.next_allocation_id(handle)?;
        let ordinary_backing = self.backing(handle);
        if let Some(backing) = ordinary_backing {
            validate_runtime_heap_backing(heap, backing, next_allocation_id)?;
        }
        for (_, backing) in self.extension_backings(handle) {
            validate_runtime_heap_backing(heap, backing, next_allocation_id)?;
        }
        let large_backing = self
            .large_backing
            .as_ref()
            .filter(|backing| backing.handle == handle);
        if let Some(backing) = large_backing {
            validate_large_heap_backing(heap, backing, next_allocation_id)?;
        }
        if let Some(backing) = ordinary_backing
            && let Some(allocation) = backing.allocations.get(&address.0)
        {
            return Ok(*allocation);
        }
        for (_, backing) in self.extension_backings(handle) {
            if let Some(allocation) = backing.allocations.get(&address.0) {
                return Ok(*allocation);
            }
        }
        if let Some(backing) = large_backing
            && backing.allocation.address == address
        {
            return Ok(backing.allocation);
        }
        Err(RuntimeStateError::not_found(
            "runtime-heap-backing",
            "heap-allocation-pointer-invalid",
            "private heap pointer does not identify a live allocation",
        ))
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
        self.validate_allocation_mutation_state(heap)?;
        let next_allocation_id = self.next_allocation_id(handle)?;
        if let Some(backing) = self.backing(handle) {
            validate_heap_backing_address_allocation(
                allocator,
                RuntimeHandle(handle).slot(),
                backing,
            )?;
            validate_runtime_heap_backing_mutation_state(heap, backing, next_allocation_id)?;
        }
        for (slot, backing) in self.extension_backings(handle) {
            if runtime_heap_backing_slot(backing) != Some(slot) {
                return Err(RuntimeStateError::runtime_defect(
                    "runtime-heap-backing",
                    "heap-extension-backing-slot-invalid",
                    "private heap extension backing does not match its controlled slot identity",
                ));
            }
            if slot == RuntimeHandle(handle).slot() {
                return Err(RuntimeStateError::runtime_defect(
                    "runtime-heap-backing",
                    "heap-extension-backing-slot-alias",
                    "private heap extension backing aliases its primary slot",
                ));
            }
            validate_heap_backing_address_allocation(allocator, slot, backing)?;
            validate_runtime_heap_backing_mutation_state(heap, backing, next_allocation_id)?;
        }
        if let Some(backing) = self
            .large_backing
            .as_ref()
            .filter(|backing| backing.handle == handle)
        {
            validate_large_heap_backing_address_allocation(allocator, backing)?;
            validate_large_heap_backing(heap, backing, next_allocation_id)?;
        }
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
        if byte_length == RUNTIME_PRIVATE_HEAP_LARGE_SLOT_SIZE {
            return self.plan_large_allocation(allocator, heap, next_allocation_id, initialization);
        }
        if byte_length > RUNTIME_PRIVATE_HEAP_ARENA_STRIDE {
            return Err(RuntimeStateError::resource_limit(
                "runtime-heap-backing",
                "heap-backing-capacity-exceeded",
                format!(
                    "allocation requests {byte_length:#x} bytes; only the {:#x}-byte large-slot variant exceeds the {:#x}-byte ordinary heap capacity",
                    RUNTIME_PRIVATE_HEAP_LARGE_SLOT_SIZE, RUNTIME_PRIVATE_HEAP_ARENA_STRIDE
                ),
            ));
        }

        next_allocation_id.0.checked_add(1).ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-heap-backing",
                "heap-allocation-id-space-exhausted",
                "private heap allocation identifier space is exhausted",
            )
        })?;
        let mut existing_plan = self
            .backing(handle)
            .map(|backing| {
                plan_existing_heap_backing_allocation(
                    RuntimeHeapBackingKind::Primary,
                    backing,
                    reserved_byte_length,
                )
            })
            .transpose()?
            .flatten();
        if existing_plan.is_none() {
            for (slot, backing) in self.extension_backings(handle) {
                if let Some(plan) = plan_existing_heap_backing_allocation(
                    RuntimeHeapBackingKind::Extension { slot },
                    backing,
                    reserved_byte_length,
                )? {
                    existing_plan = Some(plan);
                    break;
                }
            }
        }
        let (
            backing_plan,
            allocation_offset,
            source_free_block,
            previous_high_water_mark,
            next_committed_bytes,
        ) = if let Some(plan) = existing_plan {
            plan
        } else if self.backing(handle).is_none() {
            let large_live_allocations = usize::from(
                self.large_backing
                    .as_ref()
                    .is_some_and(|backing| backing.handle == handle),
            );
            if usize::try_from(heap.live_allocations).ok() != Some(large_live_allocations) {
                return Err(RuntimeStateError::runtime_defect(
                    "runtime-heap-backing",
                    "heap-allocation-count-without-backing",
                    "logical heap live allocation count does not match its large-only backing state",
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
                    kind: RuntimeHeapBackingKind::Primary,
                    address_allocation_id: address_allocation.id,
                    range,
                    initial_committed_bytes,
                },
                0,
                None,
                0,
                initial_committed_bytes.max(required_commit),
            )
        } else if heap.growable {
            let ordinary_segment_count = 1usize
                .checked_add(self.extension_backings(handle).count())
                .ok_or_else(|| {
                    RuntimeStateError::runtime_defect(
                        "runtime-heap-backing",
                        "heap-segment-count-overflow",
                        "private heap ordinary segment count overflowed host accounting",
                    )
                })?;
            if ordinary_segment_count >= RUNTIME_PRIVATE_HEAP_MAX_ORDINARY_SEGMENTS {
                return Err(RuntimeStateError::resource_limit(
                    "runtime-heap-backing",
                    "heap-backing-capacity-exceeded",
                    format!(
                        "allocation requires another ordinary backing after the bounded {}-segment heap capacity",
                        RUNTIME_PRIVATE_HEAP_MAX_ORDINARY_SEGMENTS
                    ),
                ));
            }
            let slot = self.available_extension_slot().ok_or_else(|| {
                RuntimeStateError::resource_limit(
                    "runtime-heap-backing",
                    "heap-extension-backing-slot-exhausted",
                    "no controlled private heap slot remains for a growable heap extension",
                )
            })?;
            let address_allocation = allocator.allocation_for_purpose(
                &ProcessAddressAllocationPurpose::PrivateHeapSlot { slot },
            )?;
            if address_allocation.usable_byte_length != u64::from(RUNTIME_PRIVATE_HEAP_ARENA_STRIDE)
                || address_allocation.classification != ProcessAddressAllocationClassification::Heap
            {
                return Err(RuntimeStateError::runtime_defect(
                    "runtime-heap-backing",
                    "heap-extension-address-allocation-invalid",
                    "private heap extension slot does not match the controlled address policy",
                ));
            }
            let range = GuestRange::new(
                address_allocation.range.base,
                u64::from(RUNTIME_PRIVATE_HEAP_ARENA_STRIDE),
            )
            .map_err(RuntimeStateError::from_memory)?;
            let required_commit = align_runtime_heap_value(
                reserved_byte_length,
                GUEST_PAGE_SIZE as u32,
                "heap-commit-size-overflow",
            )?;
            (
                RuntimeHeapBackingPlan::New {
                    kind: RuntimeHeapBackingKind::Extension { slot },
                    address_allocation_id: address_allocation.id,
                    range,
                    initial_committed_bytes: 0,
                },
                0,
                None,
                0,
                required_commit,
            )
        } else {
            return Err(RuntimeStateError::resource_limit(
                "runtime-heap-backing",
                "heap-backing-capacity-exceeded",
                "allocation exceeds the fixed private heap backing capacity",
            ));
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
                id: next_allocation_id,
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

    fn plan_large_allocation(
        &self,
        allocator: &ProcessAddressSpaceAllocator,
        heap: &RuntimeHeap,
        allocation_id: RuntimeHeapAllocationId,
        initialization: RuntimeHeapInitialization,
    ) -> Result<RuntimeHeapAllocationPlan, RuntimeStateError> {
        allocation_id.0.checked_add(1).ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-heap-backing",
                "heap-allocation-id-space-exhausted",
                "private heap allocation identifier space is exhausted",
            )
        })?;
        if self.large_backing.is_some() {
            return Err(RuntimeStateError::resource_limit(
                "runtime-heap-backing",
                "large-heap-slot-in-use",
                "the controlled process-wide large heap slot already has a live allocation",
            ));
        }
        if heap.maximum_size != 0 && heap.maximum_size < RUNTIME_PRIVATE_HEAP_LARGE_SLOT_SIZE {
            return Err(RuntimeStateError::resource_limit(
                "runtime-heap-backing",
                "large-heap-allocation-exceeds-maximum",
                "large heap allocation exceeds the logical heap maximum size",
            ));
        }
        let address_allocation = allocator
            .allocation_for_purpose(&ProcessAddressAllocationPurpose::PrivateHeapLargeSlot)?;
        if address_allocation.range.base != RUNTIME_PRIVATE_HEAP_LARGE_SLOT_BASE
            || address_allocation.usable_byte_length
                != u64::from(RUNTIME_PRIVATE_HEAP_LARGE_SLOT_SIZE)
            || address_allocation.classification != ProcessAddressAllocationClassification::Heap
        {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-heap-backing",
                "large-heap-address-allocation-invalid",
                "large private heap slot allocation record does not match the controlled policy",
            ));
        }
        let range = GuestRange::new(
            RUNTIME_PRIVATE_HEAP_LARGE_SLOT_BASE,
            u64::from(RUNTIME_PRIVATE_HEAP_LARGE_SLOT_SIZE),
        )
        .map_err(RuntimeStateError::from_memory)?;
        Ok(RuntimeHeapAllocationPlan {
            handle: heap.handle,
            allocation: RuntimeHeapAllocation {
                id: allocation_id,
                address: RUNTIME_PRIVATE_HEAP_LARGE_SLOT_BASE,
                byte_length: RUNTIME_PRIVATE_HEAP_LARGE_SLOT_SIZE,
                reserved_byte_length: RUNTIME_PRIVATE_HEAP_LARGE_SLOT_SIZE,
                initialization,
            },
            allocation_offset: 0,
            backing: RuntimeHeapBackingPlan::LargeNew {
                address_allocation_id: address_allocation.id,
                range,
            },
            source_free_block: None,
            previous_high_water_mark: 0,
            next_committed_bytes: 0,
        })
    }

    fn next_allocation_id(
        &self,
        handle: u32,
    ) -> Result<RuntimeHeapAllocationId, RuntimeStateError> {
        self.next_allocation_ids
            .get(&handle)
            .copied()
            .ok_or_else(|| {
                RuntimeStateError::runtime_defect(
                    "runtime-heap-backing",
                    "heap-allocation-id-state-missing",
                    "logical heap has no allocation identifier state",
                )
            })
    }

    pub(super) fn commit_allocation(
        &mut self,
        plan: RuntimeHeapAllocationPlan,
        new_memory_owner: Option<MemoryOwnerId>,
    ) -> Result<RuntimeHeapAllocation, RuntimeStateError> {
        let mut next_heap = self.heap(plan.handle)?.clone();
        self.validate_allocation_mutation_state(&next_heap)?;
        if self.next_allocation_id(plan.handle)? != plan.allocation.id {
            return Err(RuntimeStateError::conflict(
                "runtime-heap-backing",
                "heap-allocation-plan-stale",
                "private heap allocation identifier changed after planning",
            ));
        }
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
                kind,
                address_allocation_id,
                range,
                initial_committed_bytes: _,
            } => {
                if self.backing_for_kind(plan.handle, kind).is_some() {
                    return Err(RuntimeStateError::conflict(
                        "runtime-heap-backing",
                        "heap-backing-plan-stale",
                        "private heap backing appeared after allocation planning",
                    ));
                }
                let slot = kind.slot(plan.handle);
                if runtime_heap_range_slot(range) != Some(slot) {
                    return Err(RuntimeStateError::runtime_defect(
                        "runtime-heap-backing",
                        "heap-backing-slot-invalid",
                        "planned private heap backing does not match its controlled slot identity",
                    ));
                }
                if let RuntimeHeapBackingKind::Extension { slot } = kind {
                    if slot == RuntimeHandle(plan.handle).slot() {
                        return Err(RuntimeStateError::runtime_defect(
                            "runtime-heap-backing",
                            "heap-extension-backing-slot-alias",
                            "planned private heap extension aliases its primary slot",
                        ));
                    }
                    if self.slot_claimed(slot) {
                        return Err(RuntimeStateError::conflict(
                            "runtime-heap-backing",
                            "heap-extension-backing-plan-stale",
                            "planned private heap extension slot became occupied",
                        ));
                    }
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
                let backing = RuntimeHeapBacking {
                    memory_owner,
                    address_allocation_id,
                    range,
                    alignment: RUNTIME_PRIVATE_HEAP_ALIGNMENT,
                    committed_bytes: plan.next_committed_bytes,
                    high_water_mark: allocation_end,
                    allocations,
                    free_blocks: BTreeMap::new(),
                };
                match kind {
                    RuntimeHeapBackingKind::Primary => {
                        self.backings.insert(plan.handle, backing);
                    }
                    RuntimeHeapBackingKind::Extension { slot } => {
                        self.extension_backings
                            .entry(plan.handle)
                            .or_default()
                            .insert(slot, backing);
                    }
                }
            }
            RuntimeHeapBackingPlan::Existing {
                kind,
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
                let backing = self.backing_for_kind(plan.handle, kind).ok_or_else(|| {
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
                let backing = self
                    .backing_for_kind_mut(plan.handle, kind)
                    .ok_or_else(|| {
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
                backing
                    .allocations
                    .insert(plan.allocation.address.0, plan.allocation);
            }
            RuntimeHeapBackingPlan::LargeNew {
                address_allocation_id,
                range,
            } => {
                if self.large_backing.is_some() {
                    return Err(RuntimeStateError::conflict(
                        "runtime-heap-backing",
                        "large-heap-allocation-plan-stale",
                        "the process-wide large heap slot became occupied after planning",
                    ));
                }
                let memory_owner = new_memory_owner.ok_or_else(|| {
                    RuntimeStateError::runtime_defect(
                        "runtime-heap-backing",
                        "large-heap-memory-owner-missing",
                        "new large private heap allocation has no guest-memory owner",
                    )
                })?;
                self.large_backing = Some(RuntimeLargeHeapAllocationBacking {
                    handle: plan.handle,
                    memory_owner,
                    address_allocation_id,
                    range,
                    allocation: plan.allocation,
                    materialized_pages: BTreeSet::new(),
                });
            }
        }
        self.next_allocation_ids
            .insert(plan.handle, next_allocation_id);
        self.heaps.insert(plan.handle, next_heap);
        Ok(plan.allocation)
    }

    pub(super) fn plan_free(
        &self,
        handle: u32,
        address: GuestAddress,
    ) -> Result<RuntimeHeapFreePlan, RuntimeStateError> {
        let heap = self.heap(handle)?;
        self.validate_allocation_mutation_state(heap)?;
        let next_allocation_id = self.next_allocation_id(handle)?;
        let ordinary_backing = self.backing(handle);
        if let Some(backing) = ordinary_backing {
            validate_runtime_heap_backing_mutation_state(heap, backing, next_allocation_id)?;
        }
        for (_, backing) in self.extension_backings(handle) {
            validate_runtime_heap_backing_mutation_state(heap, backing, next_allocation_id)?;
        }
        let large_backing = self
            .large_backing
            .as_ref()
            .filter(|backing| backing.handle == handle);
        if let Some(backing) = large_backing {
            validate_large_heap_backing(heap, backing, next_allocation_id)?;
        }
        if let Some(backing) = ordinary_backing
            && let Some(plan) =
                plan_small_heap_free(handle, address, RuntimeHeapBackingKind::Primary, backing)?
        {
            return Ok(plan);
        }
        for (slot, backing) in self.extension_backings(handle) {
            if let Some(plan) = plan_small_heap_free(
                handle,
                address,
                RuntimeHeapBackingKind::Extension { slot },
                backing,
            )? {
                return Ok(plan);
            }
        }
        if let Some(backing) = large_backing.filter(|backing| backing.allocation.address == address)
        {
            return Ok(RuntimeHeapFreePlan {
                handle,
                allocation: backing.allocation,
                backing: RuntimeHeapFreeBackingPlan::Large {
                    backing: backing.clone(),
                },
            });
        }
        Err(RuntimeStateError::not_found(
            "runtime-heap-backing",
            "heap-allocation-pointer-invalid",
            "private heap pointer does not identify a live allocation",
        ))
    }

    pub(super) fn commit_free(
        &mut self,
        plan: RuntimeHeapFreePlan,
    ) -> Result<RuntimeHeapAllocation, RuntimeStateError> {
        let mut next_heap = self.heap(plan.handle)?.clone();
        self.validate_allocation_mutation_state(&next_heap)?;
        if next_heap.live_allocations == 0 {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-heap-backing",
                "heap-live-allocation-count-underflow",
                "private heap free would underflow its live allocation count",
            ));
        }
        match &plan.backing {
            RuntimeHeapFreeBackingPlan::Small {
                kind,
                memory_owner,
                merged_free_block,
                predecessor,
                successor,
            } => {
                let backing = self.backing_for_kind(plan.handle, *kind).ok_or_else(|| {
                    RuntimeStateError::conflict(
                        "runtime-heap-backing",
                        "heap-free-plan-stale",
                        "private heap backing disappeared after free planning",
                    )
                })?;
                if backing.memory_owner != *memory_owner
                    || backing.allocations.get(&plan.allocation.address.0) != Some(&plan.allocation)
                    || predecessor.is_some_and(|block| {
                        backing.free_blocks.get(&block.0).copied() != Some(block.1)
                    })
                    || successor.is_some_and(|block| {
                        backing.free_blocks.get(&block.0).copied() != Some(block.1)
                    })
                {
                    return Err(RuntimeStateError::conflict(
                        "runtime-heap-backing",
                        "heap-free-plan-stale",
                        "private heap allocation metadata changed after free planning",
                    ));
                }
                let backing = self
                    .backing_for_kind_mut(plan.handle, *kind)
                    .ok_or_else(|| {
                        RuntimeStateError::runtime_defect(
                            "runtime-heap-backing",
                            "preflighted-heap-backing-missing",
                            "preflighted private heap backing disappeared before free commit",
                        )
                    })?;
                backing.allocations.remove(&plan.allocation.address.0);
                if let Some((offset, _)) = predecessor {
                    backing.free_blocks.remove(offset);
                }
                if let Some((offset, _)) = successor {
                    backing.free_blocks.remove(offset);
                }
                backing
                    .free_blocks
                    .insert(merged_free_block.0, merged_free_block.1);
            }
            RuntimeHeapFreeBackingPlan::Large { backing } => {
                if self.large_backing.as_ref() != Some(backing) {
                    return Err(RuntimeStateError::conflict(
                        "runtime-heap-backing",
                        "large-heap-free-plan-stale",
                        "large private heap allocation changed after free planning",
                    ));
                }
                self.large_backing = None;
            }
        }
        next_heap.live_allocations -= 1;
        self.heaps.insert(plan.handle, next_heap);
        Ok(plan.allocation)
    }

    pub(super) fn large_allocation_contains(&self, address: GuestAddress) -> bool {
        self.large_backing.as_ref().is_some_and(|backing| {
            backing
                .allocation
                .address
                .0
                .checked_add(backing.allocation.byte_length)
                .is_some_and(|end| address.0 >= backing.allocation.address.0 && address.0 < end)
        })
    }

    pub(super) fn plan_page_materialization(
        &self,
        allocator: &ProcessAddressSpaceAllocator,
        fault_address: GuestAddress,
        page_fault_error_code: u32,
    ) -> Result<RuntimeHeapPageMaterializationPlan, RuntimeStateError> {
        let backing = self.large_backing.as_ref().ok_or_else(|| {
            RuntimeStateError::not_found(
                "runtime-heap-backing",
                "large-heap-allocation-not-live",
                "no large private heap allocation owns the fault address",
            )
        })?;
        let heap = self.heap(backing.handle)?;
        self.validate_allocation_count(heap)?;
        let next_allocation_id = self.next_allocation_id(backing.handle)?;
        if let Some(ordinary_backing) = self.backing(backing.handle) {
            validate_heap_backing_address_allocation(
                allocator,
                RuntimeHandle(backing.handle).slot(),
                ordinary_backing,
            )?;
            validate_runtime_heap_backing(heap, ordinary_backing, next_allocation_id)?;
        }
        for (slot, extension_backing) in self.extension_backings(backing.handle) {
            if runtime_heap_backing_slot(extension_backing) != Some(slot) {
                return Err(RuntimeStateError::runtime_defect(
                    "runtime-heap-backing",
                    "heap-extension-backing-slot-invalid",
                    "private heap extension backing does not match its controlled slot identity",
                ));
            }
            validate_heap_backing_address_allocation(allocator, slot, extension_backing)?;
            validate_runtime_heap_backing(heap, extension_backing, next_allocation_id)?;
        }
        validate_large_heap_backing(heap, backing, next_allocation_id)?;
        validate_large_heap_backing_address_allocation(allocator, backing)?;
        if !self.large_allocation_contains(fault_address) {
            return Err(RuntimeStateError::invalid_input(
                "runtime-heap-backing",
                "large-heap-fault-outside-allocation",
                "page fault does not fall within the live large private heap allocation",
            ));
        }
        let page_base = fault_address.0 & !(GUEST_PAGE_SIZE as u32 - 1);
        if backing.materialized_pages.contains(&page_base) {
            return Err(RuntimeStateError::conflict(
                "runtime-heap-backing",
                "large-heap-page-already-materialized",
                "large private heap page is already materialized",
            ));
        }
        let committed_page = GuestRange::new(GuestAddress(page_base), GUEST_PAGE_SIZE)
            .map_err(RuntimeStateError::from_memory)?;
        Ok(RuntimeHeapPageMaterializationPlan {
            handle: backing.handle,
            allocation: backing.allocation,
            memory_owner: backing.memory_owner,
            address_allocation_id: backing.address_allocation_id,
            fault_address,
            page_fault_error_code,
            committed_page,
            previous_materialized_pages: backing.materialized_pages.len(),
        })
    }

    pub(super) fn commit_page_materialization(
        &mut self,
        plan: RuntimeHeapPageMaterializationPlan,
    ) -> Result<RuntimeHeapPageMaterialization, RuntimeStateError> {
        let backing = self.large_backing.as_ref().ok_or_else(|| {
            RuntimeStateError::conflict(
                "runtime-heap-backing",
                "large-heap-materialization-plan-stale",
                "large private heap allocation disappeared after page planning",
            )
        })?;
        if backing.handle != plan.handle
            || backing.allocation != plan.allocation
            || backing.memory_owner != plan.memory_owner
            || backing.address_allocation_id != plan.address_allocation_id
            || backing.materialized_pages.len() != plan.previous_materialized_pages
            || backing
                .materialized_pages
                .contains(&plan.committed_page.start().0)
        {
            return Err(RuntimeStateError::conflict(
                "runtime-heap-backing",
                "large-heap-materialization-plan-stale",
                "large private heap allocation changed after page planning",
            ));
        }
        let backing = self
            .large_backing
            .as_mut()
            .expect("preflighted large backing");
        backing
            .materialized_pages
            .insert(plan.committed_page.start().0);
        Ok(RuntimeHeapPageMaterialization {
            handle: plan.handle,
            allocation: plan.allocation,
            memory_owner: plan.memory_owner,
            address_allocation_id: plan.address_allocation_id,
            fault_address: plan.fault_address,
            page_fault_error_code: plan.page_fault_error_code,
            committed_page: plan.committed_page,
            materialized_pages: backing.materialized_pages.len(),
        })
    }

    pub(super) fn plan_destruction(
        &self,
        allocator: &ProcessAddressSpaceAllocator,
        handle: u32,
    ) -> Result<RuntimeHeapDestructionPlan, RuntimeStateError> {
        let heap = self.heap(handle)?;
        self.validate_allocation_count(heap)?;
        let next_allocation_id = self.next_allocation_id(handle)?;
        let primary_backing = self.backing(handle);
        if let Some(backing) = primary_backing {
            validate_heap_backing_address_allocation(
                allocator,
                RuntimeHandle(handle).slot(),
                backing,
            )?;
            validate_runtime_heap_backing(heap, backing, next_allocation_id)?;
        }
        for (slot, backing) in self.extension_backings(handle) {
            if runtime_heap_backing_slot(backing) != Some(slot) {
                return Err(RuntimeStateError::runtime_defect(
                    "runtime-heap-backing",
                    "heap-extension-backing-slot-invalid",
                    "private heap extension backing does not match its controlled slot identity",
                ));
            }
            validate_heap_backing_address_allocation(allocator, slot, backing)?;
            validate_runtime_heap_backing(heap, backing, next_allocation_id)?;
        }
        let large_backing = self
            .large_backing
            .as_ref()
            .filter(|backing| backing.handle == handle);
        if let Some(backing) = large_backing {
            validate_large_heap_backing_address_allocation(allocator, backing)?;
            validate_large_heap_backing(heap, backing, next_allocation_id)?;
        }
        let live_allocations = primary_backing.map_or(0, |backing| backing.allocations.len())
            + self
                .extension_backings(handle)
                .map(|(_, backing)| backing.allocations.len())
                .sum::<usize>()
            + usize::from(large_backing.is_some());
        if usize::try_from(heap.live_allocations).ok() != Some(live_allocations) {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-heap-backing",
                "heap-live-allocation-count-mismatch",
                "logical heap and ordinary/large backing allocation counts disagree",
            ));
        }
        Ok(RuntimeHeapDestructionPlan {
            handle,
            small_backings: primary_backing
                .into_iter()
                .chain(self.extension_backings(handle).map(|(_, backing)| backing))
                .map(|backing| {
                    (
                        backing.address_allocation_id,
                        backing.memory_owner,
                        backing.range,
                        backing.committed_bytes,
                    )
                })
                .collect(),
            large_backing: large_backing.cloned(),
            live_allocations,
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
        let mut address_allocation_ids = Vec::new();
        let mut allocations_discarded = 0usize;
        let mut reserved_bytes_released = 0u64;
        let mut committed_bytes_released = 0u64;
        for plan in plans {
            let heap = self.heap(plan.handle)?;
            let actual_small_backings = self
                .backings
                .get(&plan.handle)
                .into_iter()
                .chain(
                    self.extension_backings(plan.handle)
                        .map(|(_, backing)| backing),
                )
                .map(|backing| {
                    (
                        backing.address_allocation_id,
                        backing.memory_owner,
                        backing.range,
                        backing.committed_bytes,
                    )
                })
                .collect::<Vec<_>>();
            let actual_large_backing = self
                .large_backing
                .as_ref()
                .filter(|backing| backing.handle == plan.handle);
            let actual_live_allocations = self
                .backings
                .get(&plan.handle)
                .into_iter()
                .chain(
                    self.extension_backings(plan.handle)
                        .map(|(_, backing)| backing),
                )
                .map(|backing| backing.allocations.len())
                .sum::<usize>()
                + usize::from(actual_large_backing.is_some());
            if actual_small_backings != plan.small_backings
                || actual_large_backing != plan.large_backing.as_ref()
                || actual_live_allocations != plan.live_allocations
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
            for (address_allocation_id, _, range, committed_bytes) in &plan.small_backings {
                address_allocation_ids.push(*address_allocation_id);
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
                    .checked_add(u64::from(*committed_bytes))
                    .ok_or_else(|| {
                        RuntimeStateError::resource_limit(
                            "runtime-heap-backing",
                            "heap-teardown-committed-byte-count-overflow",
                            "private heap teardown committed bytes overflowed host accounting",
                        )
                    })?;
            }
            if let Some(backing) = plan.large_backing.as_ref() {
                address_allocation_ids.push(backing.address_allocation_id);
                reserved_bytes_released = reserved_bytes_released
                    .checked_add(backing.range.byte_length())
                    .ok_or_else(|| {
                        RuntimeStateError::resource_limit(
                            "runtime-heap-backing",
                            "heap-teardown-reserved-byte-count-overflow",
                            "private heap teardown reserved bytes overflowed host accounting",
                        )
                    })?;
                let committed_bytes = u64::try_from(backing.materialized_pages.len())
                    .ok()
                    .and_then(|pages| pages.checked_mul(GUEST_PAGE_SIZE))
                    .ok_or_else(|| {
                        RuntimeStateError::resource_limit(
                            "runtime-heap-backing",
                            "heap-teardown-committed-byte-count-overflow",
                            "large private heap committed bytes overflowed host accounting",
                        )
                    })?;
                committed_bytes_released = committed_bytes_released
                    .checked_add(committed_bytes)
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
            self.extension_backings.remove(handle);
            if self
                .large_backing
                .as_ref()
                .is_some_and(|backing| backing.handle == *handle)
            {
                self.large_backing = None;
            }
            self.next_allocation_ids.remove(handle);
            let previous = self.heaps.remove(handle);
            debug_assert!(previous.is_some());
        }
    }

    fn validate_allocation_count(&self, heap: &RuntimeHeap) -> Result<(), RuntimeStateError> {
        self.validate_allocation_count_metadata(heap)?;
        self.validate_allocation_id_uniqueness(heap)
    }

    fn validate_allocation_mutation_state(
        &self,
        heap: &RuntimeHeap,
    ) -> Result<(), RuntimeStateError> {
        self.validate_allocation_count_metadata(heap)?;
        #[cfg(debug_assertions)]
        self.validate_allocation_id_uniqueness(heap)?;
        Ok(())
    }

    fn validate_allocation_count_metadata(
        &self,
        heap: &RuntimeHeap,
    ) -> Result<(), RuntimeStateError> {
        let primary_backing = self.backings.get(&heap.handle);
        let extension_backings = self.extension_backings.get(&heap.handle);
        if primary_backing.is_none()
            && extension_backings.is_some_and(|backings| !backings.is_empty())
        {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-heap-backing",
                "heap-extension-backing-without-primary",
                "private heap extension backing exists without its primary backing",
            ));
        }
        let ordinary_segment_count = usize::from(primary_backing.is_some())
            .checked_add(extension_backings.map_or(0, BTreeMap::len))
            .ok_or_else(|| {
                RuntimeStateError::runtime_defect(
                    "runtime-heap-backing",
                    "heap-segment-count-overflow",
                    "private heap ordinary segment count overflowed host accounting",
                )
            })?;
        if ordinary_segment_count > RUNTIME_PRIVATE_HEAP_MAX_ORDINARY_SEGMENTS {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-heap-backing",
                "heap-segment-count-invalid",
                "private heap owns more ordinary segments than the controlled process arena",
            ));
        }
        let ordinary_allocations = self
            .backings
            .get(&heap.handle)
            .map_or(0, |backing| backing.allocations.len())
            + self
                .extension_backings
                .get(&heap.handle)
                .map_or(0, |backings| {
                    backings
                        .values()
                        .map(|backing| backing.allocations.len())
                        .sum()
                });
        let large_allocations = usize::from(
            self.large_backing
                .as_ref()
                .is_some_and(|backing| backing.handle == heap.handle),
        );
        if usize::try_from(heap.live_allocations).ok()
            != ordinary_allocations.checked_add(large_allocations)
        {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-heap-backing",
                "heap-live-allocation-count-mismatch",
                "logical heap and ordinary/large backing allocation counts disagree",
            ));
        }
        Ok(())
    }

    fn validate_allocation_id_uniqueness(
        &self,
        heap: &RuntimeHeap,
    ) -> Result<(), RuntimeStateError> {
        let primary_backing = self.backings.get(&heap.handle);
        let extension_backings = self.extension_backings.get(&heap.handle);
        let mut allocation_ids = BTreeSet::new();
        for backing in primary_backing.into_iter().chain(
            extension_backings
                .into_iter()
                .flat_map(|backings| backings.values()),
        ) {
            if backing
                .allocations
                .values()
                .any(|allocation| !allocation_ids.insert(allocation.id))
            {
                return Err(RuntimeStateError::runtime_defect(
                    "runtime-heap-backing",
                    "heap-allocation-id-duplicate-across-segments",
                    "private heap allocation identity is duplicated across ordinary segments",
                ));
            }
        }
        Ok(())
    }
}

fn plan_existing_heap_backing_allocation(
    kind: RuntimeHeapBackingKind,
    backing: &RuntimeHeapBacking,
    reserved_byte_length: u32,
) -> Result<Option<RuntimeExistingHeapAllocationPlan>, RuntimeStateError> {
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
        return Ok(None);
    }
    let required_commit = align_runtime_heap_value(
        allocation_end,
        GUEST_PAGE_SIZE as u32,
        "heap-commit-size-overflow",
    )?;
    Ok(Some((
        RuntimeHeapBackingPlan::Existing {
            kind,
            memory_owner: backing.memory_owner,
            address_allocation_id: backing.address_allocation_id,
            range: backing.range,
            committed_bytes: backing.committed_bytes,
        },
        allocation_offset,
        source_free_block,
        backing.high_water_mark,
        backing.committed_bytes.max(required_commit),
    )))
}

fn plan_small_heap_free(
    handle: u32,
    address: GuestAddress,
    kind: RuntimeHeapBackingKind,
    backing: &RuntimeHeapBacking,
) -> Result<Option<RuntimeHeapFreePlan>, RuntimeStateError> {
    if let Some(allocation) = backing.allocations.get(&address.0).copied() {
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
        return Ok(Some(RuntimeHeapFreePlan {
            handle,
            allocation,
            backing: RuntimeHeapFreeBackingPlan::Small {
                kind,
                memory_owner: backing.memory_owner,
                merged_free_block: (merged_start, merged_length),
                predecessor: merge_predecessor,
                successor: merge_successor,
            },
        }));
    }
    let offset = address.0.checked_sub(backing.range.start().0);
    if offset.is_some_and(|offset| backing.free_blocks.contains_key(&offset)) {
        return Err(RuntimeStateError::conflict(
            "runtime-heap-backing",
            "heap-allocation-double-free",
            "private heap pointer belongs to an already freed block",
        ));
    }
    Ok(None)
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
    slot: u16,
    backing: &RuntimeHeapBacking,
) -> Result<(), RuntimeStateError> {
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

fn runtime_heap_backing_slot(backing: &RuntimeHeapBacking) -> Option<u16> {
    runtime_heap_range_slot(backing.range)
}

fn runtime_heap_range_slot(range: GuestRange) -> Option<u16> {
    let offset = range
        .start()
        .0
        .checked_sub(RUNTIME_PRIVATE_HEAP_ARENA_BASE.0)?;
    if !offset.is_multiple_of(RUNTIME_PRIVATE_HEAP_ARENA_STRIDE) {
        return None;
    }
    let slot = offset
        .checked_div(RUNTIME_PRIVATE_HEAP_ARENA_STRIDE)?
        .checked_add(1)?;
    let slot = u16::try_from(slot).ok()?;
    (slot <= RUNTIME_PRIVATE_HEAP_BACKING_LIMIT).then_some(slot)
}

fn validate_runtime_heap_backing(
    heap: &RuntimeHeap,
    backing: &RuntimeHeapBacking,
    next_allocation_id: RuntimeHeapAllocationId,
) -> Result<(), RuntimeStateError> {
    validate_runtime_heap_backing_metadata(heap, backing, next_allocation_id)?;
    validate_runtime_heap_backing_contents(backing, next_allocation_id)
}

fn validate_runtime_heap_backing_mutation_state(
    heap: &RuntimeHeap,
    backing: &RuntimeHeapBacking,
    next_allocation_id: RuntimeHeapAllocationId,
) -> Result<(), RuntimeStateError> {
    validate_runtime_heap_backing_metadata(heap, backing, next_allocation_id)?;
    #[cfg(debug_assertions)]
    validate_runtime_heap_backing_contents(backing, next_allocation_id)?;
    Ok(())
}

fn validate_runtime_heap_backing_metadata(
    heap: &RuntimeHeap,
    backing: &RuntimeHeapBacking,
    next_allocation_id: RuntimeHeapAllocationId,
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
        || next_allocation_id.0 == 0
    {
        return Err(RuntimeStateError::runtime_defect(
            "runtime-heap-backing",
            "heap-backing-metadata-invalid",
            "private heap backing violates its controlled address or size policy",
        ));
    }
    if backing.allocations.len() > heap.live_allocations as usize
        || backing.allocations.len() > RUNTIME_PRIVATE_HEAP_MAX_LIVE_ALLOCATIONS
    {
        return Err(RuntimeStateError::runtime_defect(
            "runtime-heap-backing",
            "heap-live-allocation-count-mismatch",
            "logical heap and backing allocation counts disagree",
        ));
    }
    Ok(())
}

fn validate_runtime_heap_backing_contents(
    backing: &RuntimeHeapBacking,
    next_allocation_id: RuntimeHeapAllocationId,
) -> Result<(), RuntimeStateError> {
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
            || allocation.id.0 >= next_allocation_id.0
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

fn validate_large_heap_backing_address_allocation(
    allocator: &ProcessAddressSpaceAllocator,
    backing: &RuntimeLargeHeapAllocationBacking,
) -> Result<(), RuntimeStateError> {
    let allocation = allocator.allocation(backing.address_allocation_id)?;
    if allocation.purpose != ProcessAddressAllocationPurpose::PrivateHeapLargeSlot
        || allocation.classification != ProcessAddressAllocationClassification::Heap
        || allocation.range.base != RUNTIME_PRIVATE_HEAP_LARGE_SLOT_BASE
        || allocation.range.byte_length != u64::from(RUNTIME_PRIVATE_HEAP_LARGE_SLOT_SIZE)
        || backing.range.start() != allocation.range.base
        || backing.range.byte_length() != allocation.range.byte_length
    {
        return Err(RuntimeStateError::runtime_defect(
            "runtime-heap-backing",
            "large-heap-address-allocation-mismatch",
            "large private heap backing does not match its permanent allocation record",
        ));
    }
    Ok(())
}

fn validate_large_heap_backing(
    heap: &RuntimeHeap,
    backing: &RuntimeLargeHeapAllocationBacking,
    next_allocation_id: RuntimeHeapAllocationId,
) -> Result<(), RuntimeStateError> {
    let allocation = backing.allocation;
    let pages_fit = backing.materialized_pages.iter().all(|page_base| {
        page_base.is_multiple_of(GUEST_PAGE_SIZE as u32)
            && *page_base >= RUNTIME_PRIVATE_HEAP_LARGE_SLOT_BASE.0
            && page_base
                .checked_add(GUEST_PAGE_SIZE as u32)
                .is_some_and(|end| {
                    end <= RUNTIME_PRIVATE_HEAP_LARGE_SLOT_BASE.0
                        + RUNTIME_PRIVATE_HEAP_LARGE_SLOT_SIZE
                })
    });
    if backing.handle != heap.handle
        || backing.memory_owner.0 == 0
        || backing.range.start() != RUNTIME_PRIVATE_HEAP_LARGE_SLOT_BASE
        || backing.range.byte_length() != u64::from(RUNTIME_PRIVATE_HEAP_LARGE_SLOT_SIZE)
        || allocation.address != RUNTIME_PRIVATE_HEAP_LARGE_SLOT_BASE
        || allocation.byte_length != RUNTIME_PRIVATE_HEAP_LARGE_SLOT_SIZE
        || allocation.reserved_byte_length != RUNTIME_PRIVATE_HEAP_LARGE_SLOT_SIZE
        || allocation.id.0 == 0
        || allocation.id.0 >= next_allocation_id.0
        || heap.live_allocations == 0
        || backing.materialized_pages.len()
            > RUNTIME_PRIVATE_HEAP_LARGE_SLOT_SIZE as usize / GUEST_PAGE_SIZE as usize
        || !pages_fit
    {
        return Err(RuntimeStateError::runtime_defect(
            "runtime-heap-backing",
            "large-heap-backing-metadata-invalid",
            "large private heap backing violates identity, range, or materialization invariants",
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
            next_private_heap_slot: RUNTIME_PRIVATE_HEAP_BACKING_LIMIT,
            ..RuntimeHeaps::default()
        };
        let final_plan = heaps.plan_private_creation(0, 0, 0).unwrap();
        assert_eq!(
            final_plan.handle(),
            LOGICAL_PRIVATE_HEAP_HANDLE_PREFIX | u32::from(RUNTIME_PRIVATE_HEAP_BACKING_LIMIT)
        );
        assert_eq!(
            heaps.next_private_heap_slot(),
            RUNTIME_PRIVATE_HEAP_BACKING_LIMIT
        );
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
                kind: RuntimeHeapBackingKind::Primary,
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

    #[test]
    fn stale_extension_slot_plan_preserves_heap_and_backing_authority() {
        let mut heaps = RuntimeHeaps::default();
        let creation = heaps.plan_creation(heap()).unwrap();
        heaps.commit_creation(creation);
        let primary_range = GuestRange::new(RUNTIME_PRIVATE_HEAP_ARENA_BASE, 0x1000).unwrap();
        let primary_allocation = RuntimeHeapAllocation {
            id: RuntimeHeapAllocationId(1),
            address: primary_range.start(),
            byte_length: 0x20,
            reserved_byte_length: 0x20,
            initialization: RuntimeHeapInitialization::Zeroed,
        };
        heaps
            .commit_allocation(
                RuntimeHeapAllocationPlan {
                    handle: HANDLE,
                    allocation: primary_allocation,
                    allocation_offset: 0,
                    backing: RuntimeHeapBackingPlan::New {
                        kind: RuntimeHeapBackingKind::Primary,
                        address_allocation_id: ProcessAddressAllocationId(1),
                        range: primary_range,
                        initial_committed_bytes: 0,
                    },
                    source_free_block: None,
                    previous_high_water_mark: 0,
                    next_committed_bytes: 0x1000,
                },
                Some(MemoryOwnerId(1)),
            )
            .unwrap();

        let extension_base = GuestAddress(
            RUNTIME_PRIVATE_HEAP_ARENA_BASE.0
                + u32::from(RUNTIME_PRIVATE_HEAP_BACKING_LIMIT - 1)
                    * RUNTIME_PRIVATE_HEAP_ARENA_STRIDE,
        );
        let extension_range = GuestRange::new(extension_base, 0x1000).unwrap();
        let extension_plan = RuntimeHeapAllocationPlan {
            handle: HANDLE,
            allocation: RuntimeHeapAllocation {
                id: RuntimeHeapAllocationId(2),
                address: extension_base,
                byte_length: 0x20,
                reserved_byte_length: 0x20,
                initialization: RuntimeHeapInitialization::Zeroed,
            },
            allocation_offset: 0,
            backing: RuntimeHeapBackingPlan::New {
                kind: RuntimeHeapBackingKind::Extension {
                    slot: RUNTIME_PRIVATE_HEAP_BACKING_LIMIT,
                },
                address_allocation_id: ProcessAddressAllocationId(32),
                range: extension_range,
                initial_committed_bytes: 0,
            },
            source_free_block: None,
            previous_high_water_mark: 0,
            next_committed_bytes: 0x1000,
        };
        let mut conflicting = heap();
        conflicting.handle =
            LOGICAL_PRIVATE_HEAP_HANDLE_PREFIX | u32::from(RUNTIME_PRIVATE_HEAP_BACKING_LIMIT);
        let conflicting_creation = heaps.plan_creation(conflicting).unwrap();
        heaps.commit_creation(conflicting_creation);

        let before = heaps.clone();
        let error = heaps
            .commit_allocation(extension_plan, Some(MemoryOwnerId(2)))
            .unwrap_err();
        assert_eq!(error.code, "heap-extension-backing-plan-stale");
        assert_eq!(heaps, before);
    }

    #[test]
    fn ordinary_and_large_backing_counts_are_one_heap_invariant() {
        let mut heaps = RuntimeHeaps::default();
        let creation = heaps.plan_creation(heap()).unwrap();
        heaps.commit_creation(creation);
        heaps.heaps.get_mut(&HANDLE).unwrap().live_allocations = 1;

        let before = heaps.clone();
        assert_eq!(
            heaps
                .allocation(HANDLE, GuestAddress(0x2000_0000))
                .unwrap_err()
                .code,
            "heap-live-allocation-count-mismatch"
        );
        assert_eq!(heaps, before);
    }
}

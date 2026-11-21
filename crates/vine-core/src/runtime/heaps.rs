use std::collections::{BTreeMap, BTreeSet};

use super::objects::{RuntimeObjectKey, RuntimeObjectTable};
use super::{
    ProcessAddressAllocationClassification, ProcessAddressAllocationPurpose,
    ProcessAddressAllocationReleasePolicy, ProcessAddressAllocationRequest,
    ProcessAddressSpaceAllocator, RuntimeAddressRange, RuntimeHandle, RuntimeStateError,
};
use crate::memory::{GUEST_PAGE_SIZE, GuestAddress, GuestMemory, GuestRange, MemoryOwnerId};

pub const RUNTIME_PRIVATE_HEAP_ARENA_BASE: GuestAddress = GuestAddress(0x2000_0000);
pub const RUNTIME_PRIVATE_HEAP_ARENA_STRIDE: u32 = 0x0100_0000;
pub const RUNTIME_PRIVATE_HEAP_BACKING_LIMIT: u16 = 36;
pub const RUNTIME_PRIVATE_HEAP_MAX_ORDINARY_SEGMENTS: usize =
    RUNTIME_PRIVATE_HEAP_BACKING_LIMIT as usize;
pub const RUNTIME_PRIVATE_HEAP_ALIGNMENT: u32 = 8;
// Bound process-wide allocation metadata independently of committed guest RAM.
// Archive indexes and script strings can exceed 64K small blocks in one heap.
pub const RUNTIME_PRIVATE_HEAP_MAX_LIVE_ALLOCATIONS: usize = 1_048_576;

const LOGICAL_PRIVATE_HEAP_HANDLE_PREFIX: u32 = 0x6801_0000;
#[derive(Debug)]
pub struct RuntimeHeap {
    pub handle: u32,
    pub initial_size: u32,
    pub maximum_size: u32,
    pub live_allocations: u32,
    pub serialized: bool,
    pub low_fragmentation: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct RuntimeHeapAllocationId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeHeapInitialization {
    Zeroed,
    Uninitialized,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeHeapAllocation {
    pub id: RuntimeHeapAllocationId,
    pub address: GuestAddress,
    pub byte_length: u32,
    pub reserved_byte_length: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeLargeHeapAllocationBacking {
    pub handle: u32,
    pub memory_owner: MemoryOwnerId,
    pub address_allocation_id: super::ProcessAddressAllocationId,
    pub range: GuestRange,
    pub allocation: RuntimeHeapAllocation,
    pub materialized_pages: BTreeSet<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RuntimeHeapBackingKind {
    Primary,
    Extension { slot: u16 },
}

#[derive(Debug, Clone, Copy)]
enum HeapAllocationStorage {
    Small(RuntimeHeapBackingKind),
    Large(u32),
}

type HeapPlacement = (u32, Option<(u32, u32)>, u32);

#[derive(Debug)]
pub(super) struct RuntimeHeaps {
    heaps: BTreeMap<u32, RuntimeHeap>,
    backings: BTreeMap<u32, RuntimeHeapBacking>,
    extension_backings: BTreeMap<u32, BTreeMap<u16, RuntimeHeapBacking>>,
    large_backings: BTreeMap<u32, RuntimeLargeHeapAllocationBacking>,
    next_allocation_ids: BTreeMap<u32, RuntimeHeapAllocationId>,
    next_private_heap_slot: u16,
    process_heap: Option<u32>,
    task_heap: Option<u32>,
}

impl Default for RuntimeHeaps {
    fn default() -> Self {
        Self {
            heaps: BTreeMap::new(),
            backings: BTreeMap::new(),
            extension_backings: BTreeMap::new(),
            large_backings: BTreeMap::new(),
            next_allocation_ids: BTreeMap::new(),
            next_private_heap_slot: 1,
            process_heap: None,
            task_heap: None,
        }
    }
}

impl RuntimeHeaps {
    pub(super) fn largest_committed_free_block(
        &self,
        handle: u32,
    ) -> Result<u32, RuntimeStateError> {
        self.heap(handle)?;
        let mut largest = 0;
        for backing in self
            .backing(handle)
            .into_iter()
            .chain(self.extension_backings(handle).map(|(_, backing)| backing))
        {
            let tail = backing
                .committed_bytes
                .checked_sub(backing.high_water_mark)
                .ok_or_else(|| {
                    RuntimeStateError::new("heap high water mark exceeds committed storage")
                })?;
            largest = largest.max(tail);
            for (&offset, &length) in &backing.free_blocks {
                let end = offset
                    .checked_add(length)
                    .ok_or_else(|| RuntimeStateError::new("free heap block overflowed"))?;
                if end > backing.committed_bytes {
                    return Err(RuntimeStateError::new(
                        "free heap block exceeds committed storage",
                    ));
                }
                let length = if end == backing.high_water_mark {
                    length + tail
                } else {
                    length
                };
                largest = largest.max(length);
            }
        }
        // Adjacent spans are already coalesced by every successful HeapFree.
        Ok(largest)
    }

    pub(super) fn configure_serialization(
        &mut self,
        handle: u32,
        serialized: bool,
    ) -> Result<(), RuntimeStateError> {
        self.heaps
            .get_mut(&handle)
            .ok_or_else(|| RuntimeStateError::new("heap serialization requires a live heap"))?
            .serialized = serialized;
        Ok(())
    }

    pub(super) fn enable_low_fragmentation(
        &mut self,
        handle: u32,
    ) -> Result<bool, RuntimeStateError> {
        let heap = self
            .heaps
            .get_mut(&handle)
            .ok_or_else(|| RuntimeStateError::new("heap compatibility requires a live heap"))?;
        if !heap.serialized || heap.maximum_size != 0 {
            return Ok(false);
        }
        heap.low_fragmentation = true;
        Ok(true)
    }

    pub(super) fn process_heap(
        &mut self,
        objects: &mut RuntimeObjectTable,
    ) -> Result<u32, RuntimeStateError> {
        if let Some(handle) = self.process_heap {
            return Ok(handle);
        }
        let handle = self.create_private(objects, 0, 0)?;
        if handle != 0 {
            self.process_heap = Some(handle);
        }
        Ok(handle)
    }

    #[cfg(test)]
    pub(super) fn task_heap_handle(&self) -> Option<u32> {
        self.task_heap
    }

    pub(super) fn allocate_task(
        &mut self,
        memory: &mut GuestMemory,
        allocator: &mut ProcessAddressSpaceAllocator,
        byte_length: u32,
    ) -> Result<Option<RuntimeHeapAllocation>, RuntimeStateError> {
        let byte_length = byte_length.max(1);
        if let Some(handle) = self.task_heap {
            return self.allocate(
                memory,
                allocator,
                handle,
                byte_length,
                RuntimeHeapInitialization::Uninitialized,
                None,
            );
        }
        let mut slot = self.next_private_heap_slot;
        while slot != 0 && slot <= RUNTIME_PRIVATE_HEAP_BACKING_LIMIT {
            let handle = LOGICAL_PRIVATE_HEAP_HANDLE_PREFIX | u32::from(slot);
            let next_slot = if slot == RUNTIME_PRIVATE_HEAP_BACKING_LIMIT {
                0
            } else {
                slot + 1
            };
            if self.slot_claimed(slot) {
                slot = next_slot;
                continue;
            }
            let heap = RuntimeHeap {
                handle,
                initial_size: 0,
                maximum_size: 0,
                live_allocations: 0,
                serialized: true,
                low_fragmentation: false,
            };
            self.validate_creation(&heap)?;
            // This internal heap has no published Win32 object or handle. Its
            // first allocation uses the same memory transaction as HeapAlloc;
            // failed allocation leaves no heap, slot, or allocation identity.
            self.heaps.insert(handle, heap);
            self.next_allocation_ids
                .insert(handle, RuntimeHeapAllocationId(1));
            let result = self.allocate(
                memory,
                allocator,
                handle,
                byte_length,
                RuntimeHeapInitialization::Uninitialized,
                None,
            );
            if matches!(result, Ok(Some(_))) {
                self.task_heap = Some(handle);
                self.next_private_heap_slot = next_slot;
            } else {
                self.heaps.remove(&handle);
                self.next_allocation_ids.remove(&handle);
            }
            return result;
        }
        Ok(None)
    }

    pub(super) fn free_task(
        &mut self,
        memory: &mut GuestMemory,
        allocator: &mut ProcessAddressSpaceAllocator,
        address: GuestAddress,
    ) -> Result<(), RuntimeStateError> {
        if address.0 == 0 {
            return Ok(());
        }
        let handle = self.task_heap.ok_or_else(|| {
            RuntimeStateError::new("COM task memory pointer has no live task allocator")
        })?;
        self.free(memory, allocator, handle, address)
    }

    #[cfg(test)]
    pub(super) fn is_empty(&self) -> bool {
        self.heaps.is_empty()
    }

    pub(super) fn heap(&self, handle: u32) -> Result<&RuntimeHeap, RuntimeStateError> {
        self.heaps.get(&handle).ok_or_else(|| {
            RuntimeStateError::new(format!("logical heap handle {handle:#010x} is not live"))
        })
    }

    pub(super) fn backing(&self, handle: u32) -> Option<&RuntimeHeapBacking> {
        self.backings.get(&handle)
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

    fn large_backings_for_handle(
        &self,
        handle: u32,
    ) -> impl DoubleEndedIterator<Item = (u32, &RuntimeLargeHeapAllocationBacking)> {
        self.large_backings
            .iter()
            .filter(move |(_, backing)| backing.handle == handle)
            .map(|(&base, backing)| (base, backing))
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

    fn create(
        &mut self,
        objects: &mut RuntimeObjectTable,
        heap: RuntimeHeap,
    ) -> Result<u32, RuntimeStateError> {
        self.validate_creation(&heap)?;
        let handle = heap.handle;
        objects.register_object_with_handle(
            RuntimeObjectKey::PrivateHeap { handle },
            RuntimeHandle(handle),
        )?;
        self.heaps.insert(handle, heap);
        self.next_allocation_ids
            .insert(handle, RuntimeHeapAllocationId(1));
        Ok(handle)
    }

    pub(super) fn create_private(
        &mut self,
        objects: &mut RuntimeObjectTable,
        initial_size: u32,
        maximum_size: u32,
    ) -> Result<u32, RuntimeStateError> {
        let mut candidate_slot = self.next_private_heap_slot;
        loop {
            if candidate_slot == 0 || candidate_slot > RUNTIME_PRIVATE_HEAP_BACKING_LIMIT {
                return Ok(0);
            }
            let next_slot = if candidate_slot == RUNTIME_PRIVATE_HEAP_BACKING_LIMIT {
                0
            } else {
                candidate_slot + 1
            };
            if self.slot_claimed(candidate_slot) {
                candidate_slot = next_slot;
                continue;
            }
            let Some(handle) = objects
                .available_handle_in_slot(super::RuntimeObjectKind::PrivateHeap, candidate_slot)?
            else {
                candidate_slot = next_slot;
                continue;
            };
            let handle = handle.0;
            let heap = RuntimeHeap {
                handle,
                initial_size,
                maximum_size,
                live_allocations: 0,
                serialized: true,
                low_fragmentation: false,
            };
            let handle = self.create(objects, heap)?;
            self.next_private_heap_slot = next_slot;
            return Ok(handle);
        }
    }

    fn validate_creation(&self, heap: &RuntimeHeap) -> Result<(), RuntimeStateError> {
        if heap.handle == 0 {
            return Err(RuntimeStateError::new(
                "runtime heap requires a nonzero handle",
            ));
        }
        if self.heaps.contains_key(&heap.handle) {
            return Err(RuntimeStateError::new(
                "runtime heap handle is already live",
            ));
        }
        if self.backings.contains_key(&heap.handle) {
            return Err(RuntimeStateError::new(
                "private heap backing exists without logical heap metadata",
            ));
        }
        if self.extension_backings.contains_key(&heap.handle) {
            return Err(RuntimeStateError::new(
                "private heap extension backing exists without logical heap metadata",
            ));
        }
        let slot = RuntimeHandle(heap.handle).slot();
        if slot == 0
            || slot > RUNTIME_PRIVATE_HEAP_BACKING_LIMIT
            || self.extension_slot_claimed(slot)
        {
            return Err(RuntimeStateError::new(
                "logical private heap handle does not own an available controlled backing slot",
            ));
        }
        if self.next_allocation_ids.contains_key(&heap.handle)
            || self
                .large_backings
                .values()
                .any(|backing| backing.handle == heap.handle)
        {
            return Err(RuntimeStateError::new(
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
        self.heaps
            .keys()
            .any(|handle| RuntimeHandle(*handle).slot() == slot)
            || self.extension_slot_claimed(slot)
    }

    pub(super) fn destroy_private(
        &mut self,
        memory: &mut GuestMemory,
        allocator: &mut ProcessAddressSpaceAllocator,
        objects: &mut RuntimeObjectTable,
        handle: u32,
    ) -> Result<bool, RuntimeStateError> {
        let heap = self.heap(handle)?;
        if self.process_heap == Some(handle) || self.task_heap == Some(handle) {
            return Ok(false);
        }
        self.validate_allocation_mutation_state(heap)?;
        let next_id = self.next_allocation_id(handle)?;
        let mut regions = Vec::new();
        let mut releases = Vec::new();
        for (slot, backing) in self
            .backing(handle)
            .map(|backing| (RuntimeHandle(handle).slot(), backing))
            .into_iter()
            .chain(self.extension_backings(handle))
        {
            validate_runtime_heap_backing_mutation_state(heap, backing, next_id)?;
            validate_heap_backing_address_allocation(allocator, slot, backing)?;
            regions.push((backing.memory_owner, backing.range.start()));
        }
        for (_, backing) in self.large_backings_for_handle(handle) {
            validate_large_heap_backing(heap, backing, next_id)?;
            validate_large_heap_backing_address_allocation(allocator, backing)?;
            regions.push((backing.memory_owner, backing.range.start()));
            releases.push(backing.address_allocation_id);
        }
        allocator.validate_releases(&releases)?;
        let mut retired_objects = objects.clone();
        retired_objects.retire_private_heap(RuntimeHandle(handle))?;
        let mut transaction = memory.begin_transaction();
        for (owner, base) in regions {
            transaction
                .release_region(owner, base)
                .map_err(RuntimeStateError::from_memory)?;
            transaction
                .destroy_owner(owner)
                .map_err(RuntimeStateError::from_memory)?;
        }
        self.heaps.remove(&handle);
        self.backings.remove(&handle);
        self.extension_backings.remove(&handle);
        self.large_backings
            .retain(|_, backing| backing.handle != handle);
        self.next_allocation_ids.remove(&handle);
        self.next_private_heap_slot = 1;
        // Ordinary arena reservations remain available for another heap; the
        // mappings and owners are gone. Large reservations are process allocations.
        allocator.remove_validated_allocations(releases);
        *objects = retired_objects;
        transaction.commit_transaction();
        Ok(true)
    }

    fn available_extension_slot(&self) -> Option<u16> {
        (1..=RUNTIME_PRIVATE_HEAP_BACKING_LIMIT)
            .rev()
            .find(|slot| !self.slot_claimed(*slot))
    }

    #[cfg(test)]
    pub(super) fn allocation(
        &self,
        handle: u32,
        address: GuestAddress,
    ) -> Result<RuntimeHeapAllocation, RuntimeStateError> {
        self.allocation_option(handle, address)?.ok_or_else(|| {
            RuntimeStateError::new("private heap pointer does not identify a live allocation")
        })
    }

    pub(super) fn allocation_option(
        &self,
        handle: u32,
        address: GuestAddress,
    ) -> Result<Option<RuntimeHeapAllocation>, RuntimeStateError> {
        let heap = self.heap(handle)?;
        // Heap maps and IDs are private Rust state, never guest heap headers.
        // Creation/allocation/free preserve their partition and unique IDs before
        // publishing. Like mutations, queries retain metadata checks in release
        // and full internal audits in debug; the selected record is checked below.
        self.validate_allocation_mutation_state(heap)?;
        let next_allocation_id = self.next_allocation_id(handle)?;
        if let Some(backing) = self.backing(handle) {
            validate_runtime_heap_backing_mutation_state(heap, backing, next_allocation_id)?;
        }
        for (_, backing) in self.extension_backings(handle) {
            validate_runtime_heap_backing_mutation_state(heap, backing, next_allocation_id)?;
        }
        for (_, backing) in self.large_backings_for_handle(handle) {
            validate_large_heap_backing(heap, backing, next_allocation_id)?;
        }
        Ok(self
            .find_allocation_option(handle, address)?
            .map(|(allocation, _)| allocation))
    }

    fn next_allocation_id(
        &self,
        handle: u32,
    ) -> Result<RuntimeHeapAllocationId, RuntimeStateError> {
        self.next_allocation_ids
            .get(&handle)
            .copied()
            .ok_or_else(|| {
                RuntimeStateError::new("logical heap has no allocation identifier state")
            })
    }

    pub(super) fn allocate(
        &mut self,
        memory: &mut GuestMemory,
        allocator: &mut ProcessAddressSpaceAllocator,
        handle: u32,
        byte_length: u32,
        initialization: RuntimeHeapInitialization,
        replacement: Option<GuestAddress>,
    ) -> Result<Option<RuntimeHeapAllocation>, RuntimeStateError> {
        if byte_length == 0 {
            return Err(RuntimeStateError::new(
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
                return Err(RuntimeStateError::new(
                    "private heap extension backing does not match its controlled slot identity",
                ));
            }
            if slot == RuntimeHandle(handle).slot() {
                return Err(RuntimeStateError::new(
                    "private heap extension backing aliases its primary slot",
                ));
            }
            validate_heap_backing_address_allocation(allocator, slot, backing)?;
            validate_runtime_heap_backing_mutation_state(heap, backing, next_allocation_id)?;
        }
        for (_, backing) in self.large_backings_for_handle(handle) {
            validate_large_heap_backing_address_allocation(allocator, backing)?;
            validate_large_heap_backing(heap, backing, next_allocation_id)?;
        }
        let replacement = replacement
            .map(|address| {
                let (allocation, storage) = self.find_allocation(handle, address)?;
                self.validate_free_allocation(handle, allocation, storage)?;
                Ok::<_, RuntimeStateError>((allocation, storage))
            })
            .transpose()?;
        if matches!(replacement, Some((_, HeapAllocationStorage::Large(_)))) {
            return Err(RuntimeStateError::new(
                "large heap reallocation requires moving lazy guest pages",
            ));
        }
        let copy_bytes = if let Some((allocation, _)) = replacement {
            let length = allocation.byte_length.min(byte_length) as usize;
            if length > memory.limits().max_access_bytes {
                return Ok(None);
            }
            memory
                .read(allocation.address, length)
                .map_err(RuntimeStateError::from_memory)?
        } else {
            Vec::new()
        };
        let live_allocations: usize = self
            .heaps
            .values()
            .map(|heap| heap.live_allocations as usize)
            .sum();
        if (replacement.is_none() && live_allocations >= RUNTIME_PRIVATE_HEAP_MAX_LIVE_ALLOCATIONS)
            || next_allocation_id.0.checked_add(1).is_none()
        {
            return Ok(None);
        }
        // LFH is an allocation policy, not a different memory owner. Bucket
        // small requests so nearby sizes reuse the same coalescing free spans;
        // HeapSize and realloc copying still use the requested byte length.
        let alignment = if heap.low_fragmentation && byte_length <= 16_384 {
            (byte_length.next_power_of_two() / 32).max(RUNTIME_PRIVATE_HEAP_ALIGNMENT)
        } else {
            RUNTIME_PRIVATE_HEAP_ALIGNMENT
        };
        let Some(reserved_byte_length) = byte_length
            .checked_add(alignment - 1)
            .map(|rounded| rounded & !(alignment - 1))
        else {
            return Ok(None);
        };
        if byte_length > RUNTIME_PRIVATE_HEAP_ARENA_STRIDE {
            if replacement.is_some() {
                return Err(RuntimeStateError::new(
                    "large heap reallocation requires moving lazy guest pages",
                ));
            }
            // Lazy pages receive fresh zeroed frames when committed, in either mode.
            return self.allocate_large(memory, allocator, handle, byte_length, next_allocation_id);
        }
        let mut existing_space = self
            .backing(handle)
            .map(|backing| find_existing_heap_space(backing, reserved_byte_length))
            .transpose()?
            .flatten()
            .map(|space| (RuntimeHeapBackingKind::Primary, space));
        if existing_space.is_none() {
            for (slot, backing) in self.extension_backings(handle) {
                if let Some(space) = find_existing_heap_space(backing, reserved_byte_length)? {
                    existing_space = Some((RuntimeHeapBackingKind::Extension { slot }, space));
                    break;
                }
            }
        }
        let (
            storage,
            previous_owner,
            range,
            address_allocation_id,
            committed_bytes,
            next_committed_bytes,
            allocation_offset,
            source_free_block,
        ) = if let Some((kind, (offset, free_block, required_commit))) = existing_space {
            let backing = self
                .backing_for_kind(handle, kind)
                .expect("selected live heap backing");
            (
                HeapAllocationStorage::Small(kind),
                Some(backing.memory_owner),
                backing.range,
                backing.address_allocation_id,
                backing.committed_bytes,
                required_commit,
                offset,
                free_block,
            )
        } else {
            let (kind, slot, capacity, initial_commit) = if self.backing(handle).is_none() {
                let slot = RuntimeHandle(handle).slot();
                if slot == 0 || slot > RUNTIME_PRIVATE_HEAP_BACKING_LIMIT {
                    return Err(RuntimeStateError::new(
                        "live heap does not identify a controlled backing slot",
                    ));
                }
                if heap.maximum_size > RUNTIME_PRIVATE_HEAP_ARENA_STRIDE
                    || heap
                        .initial_size
                        .checked_add(GUEST_PAGE_SIZE as u32 - 1)
                        .is_none()
                {
                    return Ok(None);
                }
                let capacity = if heap.maximum_size == 0 {
                    RUNTIME_PRIVATE_HEAP_ARENA_STRIDE
                } else {
                    align_runtime_heap_value(heap.maximum_size, GUEST_PAGE_SIZE as u32)?
                };
                let initial_commit = if heap.initial_size == 0 {
                    0
                } else {
                    align_runtime_heap_value(heap.initial_size, GUEST_PAGE_SIZE as u32)?
                };
                if initial_commit > capacity {
                    return Err(RuntimeStateError::new(
                        "private heap initial size exceeds its bounded maximum size",
                    ));
                }
                (
                    RuntimeHeapBackingKind::Primary,
                    slot,
                    capacity,
                    initial_commit,
                )
            } else {
                if heap.maximum_size != 0
                    || 1 + self.extension_backings(handle).count()
                        >= RUNTIME_PRIVATE_HEAP_MAX_ORDINARY_SEGMENTS
                {
                    return Ok(None);
                }
                let Some(slot) = self.available_extension_slot() else {
                    return Ok(None);
                };
                (
                    RuntimeHeapBackingKind::Extension { slot },
                    slot,
                    RUNTIME_PRIVATE_HEAP_ARENA_STRIDE,
                    0,
                )
            };
            let address_allocation = allocator.allocation_for_purpose(
                &ProcessAddressAllocationPurpose::PrivateHeapSlot { slot },
            )?;
            if address_allocation.usable_byte_length != u64::from(RUNTIME_PRIVATE_HEAP_ARENA_STRIDE)
                || address_allocation.classification != ProcessAddressAllocationClassification::Heap
            {
                return Err(RuntimeStateError::new(
                    "private heap slot allocation record does not match its bounded backing",
                ));
            }
            if reserved_byte_length > capacity {
                return Ok(None);
            }
            let range = GuestRange::new(address_allocation.range.base, u64::from(capacity))
                .map_err(RuntimeStateError::from_memory)?;
            let required_commit =
                align_runtime_heap_value(reserved_byte_length, GUEST_PAGE_SIZE as u32)?;
            (
                HeapAllocationStorage::Small(kind),
                None,
                range,
                address_allocation.id,
                0,
                initial_commit.max(required_commit),
                0,
                None,
            )
        };
        let eager = matches!(storage, HeapAllocationStorage::Small(_));
        if eager && reserved_byte_length as usize > memory.limits().max_access_bytes {
            return Ok(None);
        }
        let allocation = RuntimeHeapAllocation {
            id: next_allocation_id,
            address: range
                .start()
                .0
                .checked_add(allocation_offset)
                .map(GuestAddress)
                .ok_or_else(|| {
                    RuntimeStateError::new(
                        "private heap allocation address overflowed x86 accounting",
                    )
                })?,
            byte_length,
            reserved_byte_length,
        };
        let additional_pages =
            ((next_committed_bytes - committed_bytes) / GUEST_PAGE_SIZE as u32) as usize;
        let new_backing = previous_owner.is_none();
        if memory
            .preflight_growth(
                usize::from(new_backing),
                usize::from(new_backing),
                additional_pages,
            )
            .is_err()
        {
            return Ok(None);
        }
        let mut transaction = memory.begin_transaction();
        let owner = if let Some(owner) = previous_owner {
            owner
        } else {
            let owner = transaction
                .create_owner(crate::memory::MemoryOwnerKind::Anonymous)
                .map_err(RuntimeStateError::from_memory)?;
            transaction
                .reserve_fixed_for_address_allocation(
                    address_allocation_id.0,
                    owner,
                    crate::memory::MemoryRegionKind::Heap,
                    range.start(),
                    range.byte_length(),
                )
                .map_err(RuntimeStateError::from_memory)?;
            owner
        };
        if next_committed_bytes != committed_bytes {
            let start = range
                .start()
                .0
                .checked_add(committed_bytes)
                .map(GuestAddress)
                .ok_or_else(|| {
                    RuntimeStateError::new("private heap commit tail overflowed x86 accounting")
                })?;
            let commit_range =
                GuestRange::new(start, u64::from(next_committed_bytes - committed_bytes))
                    .map_err(RuntimeStateError::from_memory)?;
            transaction
                .commit(
                    owner,
                    commit_range,
                    crate::memory::PageProtection {
                        read: true,
                        write: true,
                        execute: false,
                    },
                )
                .map_err(RuntimeStateError::from_memory)?;
        }
        if eager {
            crate::heap::initialize_allocation(
                &mut transaction,
                allocation.address,
                reserved_byte_length,
                initialization,
            )?;
            if replacement.is_some() {
                transaction
                    .write(allocation.address, &copy_bytes)
                    .map_err(RuntimeStateError::from_memory)?;
            }
        }
        if let Some((previous, previous_storage)) = replacement {
            self.free_allocation_memory(&mut transaction, previous, previous_storage)?;
        }
        let allocation_end = allocation_offset + reserved_byte_length;
        match storage {
            HeapAllocationStorage::Small(kind) => {
                if new_backing {
                    let backing = RuntimeHeapBacking {
                        memory_owner: owner,
                        address_allocation_id,
                        range,
                        alignment: RUNTIME_PRIVATE_HEAP_ALIGNMENT,
                        committed_bytes: next_committed_bytes,
                        high_water_mark: allocation_end,
                        allocations: BTreeMap::from([(allocation.address.0, allocation)]),
                        free_blocks: BTreeMap::new(),
                    };
                    match kind {
                        RuntimeHeapBackingKind::Primary => {
                            self.backings.insert(handle, backing);
                        }
                        RuntimeHeapBackingKind::Extension { slot } => {
                            self.extension_backings
                                .entry(handle)
                                .or_default()
                                .insert(slot, backing);
                        }
                    }
                } else {
                    let backing = self
                        .backing_for_kind_mut(handle, kind)
                        .expect("selected live heap backing");
                    if let Some((offset, length)) = source_free_block {
                        backing.free_blocks.remove(&offset);
                        let remaining = length - reserved_byte_length;
                        if remaining != 0 {
                            backing.free_blocks.insert(allocation_end, remaining);
                        }
                    } else {
                        backing.high_water_mark = allocation_end;
                    }
                    backing.committed_bytes = next_committed_bytes;
                    backing.allocations.insert(allocation.address.0, allocation);
                }
            }
            HeapAllocationStorage::Large(_) => unreachable!("large allocations publish separately"),
        }
        self.next_allocation_ids
            .insert(handle, RuntimeHeapAllocationId(next_allocation_id.0 + 1));
        self.heaps
            .get_mut(&handle)
            .expect("live heap")
            .live_allocations += 1;
        if let Some((previous, previous_storage)) = replacement {
            self.remove_allocation(handle, previous, previous_storage);
        }
        transaction.commit_transaction();
        Ok(Some(allocation))
    }

    fn allocate_large(
        &mut self,
        memory: &mut GuestMemory,
        allocator: &mut ProcessAddressSpaceAllocator,
        handle: u32,
        byte_length: u32,
        next_allocation_id: RuntimeHeapAllocationId,
    ) -> Result<Option<RuntimeHeapAllocation>, RuntimeStateError> {
        let heap = self.heap(handle)?;
        if heap.maximum_size != 0 && heap.maximum_size < byte_length {
            return Ok(None);
        }
        let reserved_byte_length =
            align_runtime_heap_value(byte_length, RUNTIME_PRIVATE_HEAP_ALIGNMENT)?;
        let page_mask = GUEST_PAGE_SIZE - 1;
        let range_bytes = (u64::from(reserved_byte_length) + page_mask) & !page_mask;
        if memory.preflight_growth(1, 1, 0).is_err() {
            return Ok(None);
        }
        allocator.try_reserve_top_down_with(
            ProcessAddressAllocationRequest {
                purpose: ProcessAddressAllocationPurpose::PrivateHeapLargeAllocation,
                preferred_address: None,
                search_range: RuntimeAddressRange::new(GuestAddress(0x0001_0000), 0x7ffe_0000)?,
                reserve_size: range_bytes,
                alignment: 65_536,
                guard_pages: 0,
                classification: ProcessAddressAllocationClassification::Heap,
                release_policy: ProcessAddressAllocationReleasePolicy::Explicit,
                executable: false,
            },
            |address_allocation| {
                let range = GuestRange::new(address_allocation.range.base, range_bytes)
                    .map_err(RuntimeStateError::from_memory)?;
                let mut transaction = memory.begin_transaction();
                let owner = transaction
                    .create_owner(crate::memory::MemoryOwnerKind::Anonymous)
                    .map_err(RuntimeStateError::from_memory)?;
                transaction
                    .reserve_fixed_for_address_allocation(
                        address_allocation.id.0,
                        owner,
                        crate::memory::MemoryRegionKind::Heap,
                        range.start(),
                        range.byte_length(),
                    )
                    .map_err(RuntimeStateError::from_memory)?;
                let allocation = RuntimeHeapAllocation {
                    id: next_allocation_id,
                    address: range.start(),
                    byte_length,
                    reserved_byte_length,
                };
                transaction.commit_transaction();
                self.large_backings.insert(
                    allocation.address.0,
                    RuntimeLargeHeapAllocationBacking {
                        handle,
                        memory_owner: owner,
                        address_allocation_id: address_allocation.id,
                        range,
                        allocation,
                        materialized_pages: BTreeSet::new(),
                    },
                );
                self.next_allocation_ids
                    .insert(handle, RuntimeHeapAllocationId(next_allocation_id.0 + 1));
                self.heaps
                    .get_mut(&handle)
                    .expect("live heap")
                    .live_allocations += 1;
                Ok(Some(allocation))
            },
        )
    }

    fn find_allocation(
        &self,
        handle: u32,
        address: GuestAddress,
    ) -> Result<(RuntimeHeapAllocation, HeapAllocationStorage), RuntimeStateError> {
        self.find_allocation_option(handle, address)?
            .ok_or_else(|| {
                RuntimeStateError::new("private heap pointer does not identify a live allocation")
            })
    }

    fn find_allocation_option(
        &self,
        handle: u32,
        address: GuestAddress,
    ) -> Result<Option<(RuntimeHeapAllocation, HeapAllocationStorage)>, RuntimeStateError> {
        let next_allocation_id = self.next_allocation_id(handle)?;
        if let Some(backing) = self.backing(handle)
            && let Some(allocation) = backing.allocations.get(&address.0)
        {
            validate_runtime_heap_allocation(backing, address, *allocation, next_allocation_id)?;
            return Ok(Some((
                *allocation,
                HeapAllocationStorage::Small(RuntimeHeapBackingKind::Primary),
            )));
        }
        for (slot, backing) in self.extension_backings(handle) {
            if let Some(allocation) = backing.allocations.get(&address.0) {
                validate_runtime_heap_allocation(
                    backing,
                    address,
                    *allocation,
                    next_allocation_id,
                )?;
                return Ok(Some((
                    *allocation,
                    HeapAllocationStorage::Small(RuntimeHeapBackingKind::Extension { slot }),
                )));
            }
        }
        for (base, backing) in self.large_backings_for_handle(handle) {
            if backing.allocation.address == address {
                if base != address.0 {
                    return Err(RuntimeStateError::new(
                        "large heap backing address differs from its live table key",
                    ));
                }
                return Ok(Some((
                    backing.allocation,
                    HeapAllocationStorage::Large(base),
                )));
            }
        }
        Ok(None)
    }

    fn validate_free_allocation(
        &self,
        handle: u32,
        allocation: RuntimeHeapAllocation,
        storage: HeapAllocationStorage,
    ) -> Result<(), RuntimeStateError> {
        if let HeapAllocationStorage::Small(kind) = storage {
            validate_small_heap_free(
                self.backing_for_kind(handle, kind).expect("live backing"),
                allocation,
            )?;
        }
        Ok(())
    }

    fn free_allocation_memory(
        &self,
        memory: &mut GuestMemory,
        allocation: RuntimeHeapAllocation,
        storage: HeapAllocationStorage,
    ) -> Result<(), RuntimeStateError> {
        match storage {
            HeapAllocationStorage::Small(_) => memory
                .validate_host_write(allocation.address, allocation.reserved_byte_length as usize)
                .map_err(RuntimeStateError::from_memory),
            HeapAllocationStorage::Large(base) => {
                let backing = &self.large_backings[&base];
                memory
                    .release_region(backing.memory_owner, backing.range.start())
                    .map_err(RuntimeStateError::from_memory)?;
                memory
                    .destroy_owner(backing.memory_owner)
                    .map_err(RuntimeStateError::from_memory)?;
                Ok(())
            }
        }
    }

    fn remove_allocation(
        &mut self,
        handle: u32,
        allocation: RuntimeHeapAllocation,
        storage: HeapAllocationStorage,
    ) {
        match storage {
            HeapAllocationStorage::Small(kind) => {
                let backing = self
                    .backing_for_kind_mut(handle, kind)
                    .expect("live backing");
                let offset = allocation.address.0 - backing.range.start().0;
                let end = offset + allocation.reserved_byte_length;
                let predecessor = backing
                    .free_blocks
                    .range(..offset)
                    .next_back()
                    .map(|(start, length)| (*start, *length))
                    .filter(|(start, length)| *start + *length == offset);
                let successor = backing
                    .free_blocks
                    .get_key_value(&end)
                    .map(|(start, length)| (*start, *length));
                let merged_start = predecessor.map_or(offset, |(start, _)| start);
                let merged_length = allocation.reserved_byte_length
                    + predecessor.map_or(0, |(_, length)| length)
                    + successor.map_or(0, |(_, length)| length);
                backing.allocations.remove(&allocation.address.0);
                if let Some((start, _)) = predecessor {
                    backing.free_blocks.remove(&start);
                }
                if let Some((start, _)) = successor {
                    backing.free_blocks.remove(&start);
                }
                backing.free_blocks.insert(merged_start, merged_length);
            }
            HeapAllocationStorage::Large(base) => {
                self.large_backings.remove(&base);
            }
        }
        self.heaps
            .get_mut(&handle)
            .expect("live heap")
            .live_allocations -= 1;
    }

    pub(super) fn free(
        &mut self,
        memory: &mut GuestMemory,
        allocator: &mut ProcessAddressSpaceAllocator,
        handle: u32,
        address: GuestAddress,
    ) -> Result<(), RuntimeStateError> {
        let heap = self.heap(handle)?;
        self.validate_allocation_mutation_state(heap)?;
        let next_allocation_id = self.next_allocation_id(handle)?;
        if let Some(backing) = self.backing(handle) {
            validate_runtime_heap_backing_mutation_state(heap, backing, next_allocation_id)?;
        }
        for (_, backing) in self.extension_backings(handle) {
            validate_runtime_heap_backing_mutation_state(heap, backing, next_allocation_id)?;
        }
        for (_, backing) in self.large_backings_for_handle(handle) {
            validate_large_heap_backing_address_allocation(allocator, backing)?;
            validate_large_heap_backing(heap, backing, next_allocation_id)?;
        }
        let (allocation, storage) = self.find_allocation(handle, address)?;
        self.validate_free_allocation(handle, allocation, storage)?;
        let release = match storage {
            HeapAllocationStorage::Small(_) => None,
            HeapAllocationStorage::Large(base) => {
                let id = self.large_backings[&base].address_allocation_id;
                allocator.validate_releases(&[id])?;
                Some(id)
            }
        };
        if release.is_some() {
            let mut transaction = memory.begin_transaction();
            self.free_allocation_memory(&mut transaction, allocation, storage)?;
            transaction.commit_transaction();
        } else {
            self.free_allocation_memory(memory, allocation, storage)?;
        }
        self.remove_allocation(handle, allocation, storage);
        if let Some(id) = release {
            allocator.remove_validated_allocations(vec![id]);
        }
        Ok(())
    }

    pub(super) fn large_allocation_contains(&self, address: GuestAddress) -> bool {
        self.large_backings.values().any(|backing| {
            backing
                .allocation
                .address
                .0
                .checked_add(backing.allocation.byte_length)
                .is_some_and(|end| address.0 >= backing.allocation.address.0 && address.0 < end)
        })
    }

    pub(super) fn preview_page_materialization(
        &self,
        allocator: &ProcessAddressSpaceAllocator,
        fault_address: GuestAddress,
    ) -> Result<(&RuntimeLargeHeapAllocationBacking, GuestRange), RuntimeStateError> {
        let (base, backing) = self
            .large_backings
            .iter()
            .find(|(_, backing)| {
                backing
                    .allocation
                    .address
                    .0
                    .checked_add(backing.allocation.byte_length)
                    .is_some_and(|end| {
                        fault_address.0 >= backing.allocation.address.0 && fault_address.0 < end
                    })
            })
            .ok_or_else(|| {
                RuntimeStateError::new("no large private heap allocation owns the fault address")
            })?;
        if *base != backing.allocation.address.0 {
            return Err(RuntimeStateError::new(
                "large heap backing address differs from its live table key",
            ));
        }
        let heap = self.heap(backing.handle)?;
        self.validate_allocation_mutation_state(heap)?;
        let next_allocation_id = self.next_allocation_id(backing.handle)?;
        if let Some(ordinary_backing) = self.backing(backing.handle) {
            validate_heap_backing_address_allocation(
                allocator,
                RuntimeHandle(backing.handle).slot(),
                ordinary_backing,
            )?;
            validate_runtime_heap_backing_mutation_state(
                heap,
                ordinary_backing,
                next_allocation_id,
            )?;
        }
        for (slot, extension_backing) in self.extension_backings(backing.handle) {
            if runtime_heap_backing_slot(extension_backing) != Some(slot) {
                return Err(RuntimeStateError::new(
                    "private heap extension backing does not match its controlled slot identity",
                ));
            }
            validate_heap_backing_address_allocation(allocator, slot, extension_backing)?;
            validate_runtime_heap_backing_mutation_state(
                heap,
                extension_backing,
                next_allocation_id,
            )?;
        }
        validate_large_heap_backing(heap, backing, next_allocation_id)?;
        validate_large_heap_backing_address_allocation(allocator, backing)?;
        let page_base = fault_address.0 & !(GUEST_PAGE_SIZE as u32 - 1);
        if backing.materialized_pages.contains(&page_base) {
            return Err(RuntimeStateError::new(
                "large private heap page is already materialized",
            ));
        }
        let committed_page = GuestRange::new(GuestAddress(page_base), GUEST_PAGE_SIZE)
            .map_err(RuntimeStateError::from_memory)?;
        Ok((backing, committed_page))
    }

    pub(super) fn record_materialized_page(&mut self, base: u32, page: GuestRange) {
        let backing = self
            .large_backings
            .get_mut(&base)
            .expect("validated large heap backing");
        backing.materialized_pages.insert(page.start().0);
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
            return Err(RuntimeStateError::new(
                "private heap extension backing exists without its primary backing",
            ));
        }
        let ordinary_segment_count = usize::from(primary_backing.is_some())
            .checked_add(extension_backings.map_or(0, BTreeMap::len))
            .ok_or_else(|| {
                RuntimeStateError::new(
                    "private heap ordinary segment count overflowed host accounting",
                )
            })?;
        if ordinary_segment_count > RUNTIME_PRIVATE_HEAP_MAX_ORDINARY_SEGMENTS {
            return Err(RuntimeStateError::new(
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
        let large_allocations = self.large_backings_for_handle(heap.handle).count();
        if usize::try_from(heap.live_allocations).ok()
            != ordinary_allocations.checked_add(large_allocations)
        {
            return Err(RuntimeStateError::new(
                "logical heap and ordinary/large backing allocation counts disagree",
            ));
        }
        Ok(())
    }

    #[cfg(any(test, debug_assertions))]
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
                return Err(RuntimeStateError::new(
                    "private heap allocation identity is duplicated across ordinary segments",
                ));
            }
        }
        if self
            .large_backings_for_handle(heap.handle)
            .any(|(_, backing)| !allocation_ids.insert(backing.allocation.id))
        {
            return Err(RuntimeStateError::new(
                "private heap allocation identity is duplicated across ordinary and large segments",
            ));
        }
        Ok(())
    }
}

fn find_existing_heap_space(
    backing: &RuntimeHeapBacking,
    reserved_byte_length: u32,
) -> Result<Option<HeapPlacement>, RuntimeStateError> {
    let free_block = backing
        .free_blocks
        .iter()
        .find(|(_, available)| **available >= reserved_byte_length)
        .map(|(offset, available)| (*offset, *available));
    let offset = free_block.map_or(backing.high_water_mark, |(offset, _)| offset);
    let end = offset.checked_add(reserved_byte_length).ok_or_else(|| {
        RuntimeStateError::new("private heap allocation range overflowed x86 accounting")
    })?;
    let capacity = u32::try_from(backing.range.byte_length()).map_err(|_| {
        RuntimeStateError::new("private heap backing capacity does not fit x86 accounting")
    })?;
    if end > capacity {
        return Ok(None);
    }
    let required_commit = align_runtime_heap_value(end, GUEST_PAGE_SIZE as u32)?;
    Ok(Some((
        offset,
        free_block,
        backing.committed_bytes.max(required_commit),
    )))
}

// Exact map membership is resolved before this check. Bounds/identity checks
// concern the selected host record; free-neighbor checks keep free/realloc safe
// without rebuilding the complete live/free partition on a read-only lookup.
fn validate_runtime_heap_allocation(
    backing: &RuntimeHeapBacking,
    address: GuestAddress,
    allocation: RuntimeHeapAllocation,
    next_allocation_id: RuntimeHeapAllocationId,
) -> Result<(), RuntimeStateError> {
    let offset = address
        .0
        .checked_sub(backing.range.start().0)
        .ok_or_else(|| {
            RuntimeStateError::new("private heap allocation precedes its backing range")
        })?;
    let end = offset
        .checked_add(allocation.reserved_byte_length)
        .ok_or_else(|| RuntimeStateError::new("private heap allocation range overflowed"))?;
    if allocation.address != address
        || allocation.id.0 == 0
        || allocation.id.0 >= next_allocation_id.0
        || allocation.byte_length == 0
        || allocation.byte_length > allocation.reserved_byte_length
        || !allocation
            .reserved_byte_length
            .is_multiple_of(backing.alignment)
        || !offset.is_multiple_of(backing.alignment)
        || end > backing.high_water_mark
    {
        return Err(RuntimeStateError::new(
            "private heap allocation violates identity, alignment, or range invariants",
        ));
    }
    validate_small_heap_free(backing, allocation)
}

fn validate_small_heap_free(
    backing: &RuntimeHeapBacking,
    allocation: RuntimeHeapAllocation,
) -> Result<(), RuntimeStateError> {
    let offset = allocation
        .address
        .0
        .checked_sub(backing.range.start().0)
        .ok_or_else(|| {
            RuntimeStateError::new("live private heap allocation precedes its backing range")
        })?;
    let end = offset
        .checked_add(allocation.reserved_byte_length)
        .ok_or_else(|| RuntimeStateError::new("live private heap allocation range overflowed"))?;
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
        || successor.is_some_and(|(start, _)| start < end)
    {
        return Err(RuntimeStateError::new(
            "private heap free blocks overlap a live allocation",
        ));
    }
    let predecessor =
        predecessor.filter(|(start, length)| start.checked_add(*length) == Some(offset));
    let successor = successor.filter(|(start, _)| *start == end);
    allocation
        .reserved_byte_length
        .checked_add(predecessor.map_or(0, |(_, length)| length))
        .and_then(|length| length.checked_add(successor.map_or(0, |(_, length)| length)))
        .ok_or_else(|| {
            RuntimeStateError::new("coalesced private heap free block overflowed x86 accounting")
        })?;
    Ok(())
}

fn align_runtime_heap_value(value: u32, alignment: u32) -> Result<u32, RuntimeStateError> {
    if alignment == 0 || !alignment.is_power_of_two() {
        return Err(RuntimeStateError::new(
            "private heap alignment must be a nonzero power of two",
        ));
    }
    value
        .checked_add(alignment - 1)
        .map(|rounded| rounded & !(alignment - 1))
        .ok_or_else(|| {
            RuntimeStateError::new("private heap value overflowed while applying alignment")
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
        return Err(RuntimeStateError::new(
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
        RuntimeStateError::new("private heap backing capacity does not fit x86 accounting")
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
        return Err(RuntimeStateError::new(
            "private heap backing violates its controlled address or size policy",
        ));
    }
    if backing.allocations.len() > heap.live_allocations as usize
        || backing.allocations.len() > RUNTIME_PRIVATE_HEAP_MAX_LIVE_ALLOCATIONS
    {
        return Err(RuntimeStateError::new(
            "logical heap and backing allocation counts disagree",
        ));
    }
    Ok(())
}

#[cfg(any(test, debug_assertions))]
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
                RuntimeStateError::new("private heap allocation precedes its backing range")
            })?;
        let end = offset
            .checked_add(allocation.reserved_byte_length)
            .ok_or_else(|| RuntimeStateError::new("private heap allocation range overflowed"))?;
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
            return Err(RuntimeStateError::new(
                "private heap allocation violates identity, alignment, or range invariants",
            ));
        }
        intervals.push((offset, end));
    }
    for (&offset, &length) in &backing.free_blocks {
        let end = offset
            .checked_add(length)
            .ok_or_else(|| RuntimeStateError::new("private heap free block range overflowed"))?;
        if length == 0
            || offset % backing.alignment != 0
            || length % backing.alignment != 0
            || end > backing.high_water_mark
        {
            return Err(RuntimeStateError::new(
                "private heap free block violates alignment or range invariants",
            ));
        }
        intervals.push((offset, end));
    }
    intervals.sort_unstable_by_key(|interval| interval.0);
    let mut cursor = 0u32;
    for (start, end) in intervals {
        if start != cursor || end <= start {
            return Err(RuntimeStateError::new(
                "private heap live and free blocks do not partition the used arena",
            ));
        }
        cursor = end;
    }
    if cursor != backing.high_water_mark {
        return Err(RuntimeStateError::new(
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
    if allocation.purpose != ProcessAddressAllocationPurpose::PrivateHeapLargeAllocation
        || allocation.classification != ProcessAddressAllocationClassification::Heap
        || allocation.release_policy != ProcessAddressAllocationReleasePolicy::Explicit
        || allocation.executable
        || allocation.guard_pages != 0
        || allocation.alignment != 65_536
        || allocation.usable_byte_length != backing.range.byte_length()
        || backing.range.start() != allocation.range.base
        || backing.range.byte_length() != allocation.range.byte_length
    {
        return Err(RuntimeStateError::new(
            "large private heap backing does not match its live allocation record",
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
    let expected_base = backing.range.start();
    let expected_end = backing.range.end();
    let page_mask = GUEST_PAGE_SIZE - 1;
    let expected_range_bytes =
        (u64::from(allocation.reserved_byte_length) + page_mask) & !page_mask;
    let page_fits = |page_base: &u32| {
        page_base.is_multiple_of(GUEST_PAGE_SIZE as u32)
            && *page_base >= expected_base.0
            && u64::from(*page_base) + GUEST_PAGE_SIZE <= expected_end
    };
    // Page insertion follows checked live-backing validation and a memory transaction.
    // Ordered endpoints prove the range without an O(n) rescan on every new page.
    // Keep the complete internal-state audit in debug builds.
    let pages_fit = backing.materialized_pages.first().is_none_or(page_fits)
        && backing.materialized_pages.last().is_none_or(page_fits)
        && (!cfg!(debug_assertions) || backing.materialized_pages.iter().all(page_fits));
    if backing.handle != heap.handle
        || backing.memory_owner.0 == 0
        || !expected_base.0.is_multiple_of(65_536)
        || !backing.range.byte_length().is_multiple_of(GUEST_PAGE_SIZE)
        || backing.range.byte_length() != expected_range_bytes
        || allocation.address != expected_base
        || allocation.byte_length <= RUNTIME_PRIVATE_HEAP_ARENA_STRIDE
        || allocation.byte_length > allocation.reserved_byte_length
        || !allocation
            .reserved_byte_length
            .is_multiple_of(RUNTIME_PRIVATE_HEAP_ALIGNMENT)
        || allocation.id.0 == 0
        || allocation.id.0 >= next_allocation_id.0
        || heap.live_allocations == 0
        || backing.materialized_pages.len()
            > backing.range.byte_length() as usize / GUEST_PAGE_SIZE as usize
        || !pages_fit
    {
        return Err(RuntimeStateError::new(
            "large private heap backing violates identity, range, or materialization invariants",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::RuntimeObjectKind;

    #[test]
    fn private_heap_capacity_keeps_every_previously_created_heap_live() {
        let mut heaps = RuntimeHeaps::default();
        let mut objects = RuntimeObjectTable::default();
        let mut handles = Vec::new();
        for _ in 0..256 {
            let handle = heaps.create_private(&mut objects, 0, 0).unwrap();
            if handle == 0 {
                break;
            }
            assert!(!handles.contains(&handle));
            handles.push(handle);
        }
        assert!(!handles.is_empty());
        assert_eq!(heaps.create_private(&mut objects, 0, 0).unwrap(), 0);
        for handle in handles {
            assert_eq!(heaps.heap(handle).unwrap().handle, handle);
            assert!(
                objects
                    .object_for_handle(RuntimeHandle(handle), RuntimeObjectKind::PrivateHeap)
                    .is_ok()
            );
        }
    }

    #[test]
    fn invalid_or_duplicate_heap_creation_preserves_the_live_heap() {
        let mut heaps = RuntimeHeaps::default();
        let mut objects = RuntimeObjectTable::default();
        let handle = heaps.create_private(&mut objects, 4096, 8192).unwrap();
        for rejected in [0, u32::MAX, handle] {
            assert!(
                heaps
                    .create(
                        &mut objects,
                        RuntimeHeap {
                            handle: rejected,
                            initial_size: 0,
                            maximum_size: 0,
                            live_allocations: 0,
                            serialized: true,
                            low_fragmentation: false,
                        }
                    )
                    .is_err()
            );
            let live = heaps.heap(handle).unwrap();
            assert_eq!(live.initial_size, 4096);
            assert_eq!(live.maximum_size, 8192);
            assert!(
                objects
                    .object_for_handle(RuntimeHandle(handle), RuntimeObjectKind::PrivateHeap)
                    .is_ok()
            );
        }
        let next = heaps.create_private(&mut objects, 0, 0).unwrap();
        assert_ne!(next, 0);
        assert_ne!(next, handle);
    }
}

#[cfg(test)]
#[path = "heap_lookup_tests.rs"]
mod lookup_tests;

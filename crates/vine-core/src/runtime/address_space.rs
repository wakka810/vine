use std::collections::BTreeMap;

use crate::callbacks::{
    CALLBACK_RETURN_ARENA_BASE, CALLBACK_RETURN_ARENA_SIZE, CALLBACK_SCRATCH_ARENA_BASE,
    CALLBACK_SCRATCH_ARENA_SIZE,
};
use crate::dynamic_exports::{DYNAMIC_EXPORT_ARENA_BASE, DYNAMIC_EXPORT_ARENA_SIZE};
use crate::imports::{IMPORT_THUNK_ARENA_BASE, IMPORT_THUNK_ARENA_SIZE};
use crate::memory::{GUEST_PAGE_SIZE, GuestAddress, GuestMemory, GuestMemoryError};

use super::{
    RUNTIME_AUDIO_RESOURCE_ARENA_BASE, RUNTIME_AUDIO_RESOURCE_ARENA_SIZE,
    RUNTIME_GRAPHICS_RESOURCE_ARENA_BASE, RUNTIME_GRAPHICS_RESOURCE_ARENA_SIZE,
    RUNTIME_PRIVATE_HEAP_ARENA_BASE, RUNTIME_PRIVATE_HEAP_ARENA_STRIDE,
    RUNTIME_PRIVATE_HEAP_BACKING_LIMIT, RuntimeStateError,
};

const X86_ADDRESS_SPACE_END: u64 = 0x1_0000_0000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeAddressRange {
    pub base: GuestAddress,
    pub byte_length: u64,
}

impl RuntimeAddressRange {
    pub fn new(base: GuestAddress, byte_length: u64) -> Result<Self, RuntimeStateError> {
        let range = Self { base, byte_length };
        range.end_exclusive()?;
        Ok(range)
    }

    pub fn end_exclusive(self) -> Result<u64, RuntimeStateError> {
        if self.byte_length == 0 {
            return Err(address_error("process address range must be nonempty"));
        }
        u64::from(self.base.0)
            .checked_add(self.byte_length)
            .filter(|end| *end <= X86_ADDRESS_SPACE_END)
            .ok_or_else(|| address_error("process address range crosses the x86 address limit"))
    }

    pub fn contains(self, other: Self) -> Result<bool, RuntimeStateError> {
        Ok(u64::from(self.base.0) <= u64::from(other.base.0)
            && other.end_exclusive()? <= self.end_exclusive()?)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProcessAddressAllocationId(pub u64);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ProcessAddressAllocationPurpose {
    ImportThunks,
    CallbackReturns,
    CallbackScratch,
    DynamicExports,
    PrivateHeapBand,
    PrivateHeapSlot { slot: u16 },
    PrivateHeapLargeAllocation,
    GuestImage,
    ThreadStack,
    ThreadEnvironment,
    ProcessEnvironment,
    ComEndpoint,
    GraphicsResourceArena,
    GraphicsResource,
    GdiResource,
    AudioResourceArena,
    AudioResource,
    MediaResource,
    VirtualMemory,
    MappedFile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessAddressAllocationClassification {
    RuntimeControl,
    Image,
    Stack,
    Heap,
    ProcessEnvironment,
    Com,
    Graphics,
    Audio,
    VirtualMemory,
    MappedFile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessAddressAllocationReleasePolicy {
    Permanent,
    Explicit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessAddressAllocationRecord {
    pub id: ProcessAddressAllocationId,
    pub parent_id: Option<ProcessAddressAllocationId>,
    pub purpose: ProcessAddressAllocationPurpose,
    pub range: RuntimeAddressRange,
    pub usable_byte_length: u64,
    pub alignment: u32,
    pub guard_pages: u32,
    pub classification: ProcessAddressAllocationClassification,

    pub release_policy: ProcessAddressAllocationReleasePolicy,
    pub executable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessAddressAllocationRequest {
    pub purpose: ProcessAddressAllocationPurpose,
    pub preferred_address: Option<GuestAddress>,
    pub search_range: RuntimeAddressRange,
    pub reserve_size: u64,
    pub alignment: u32,
    pub guard_pages: u32,
    pub classification: ProcessAddressAllocationClassification,

    pub release_policy: ProcessAddressAllocationReleasePolicy,
    pub executable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessAddressMappingRequirements {
    pub purpose: ProcessAddressAllocationPurpose,
    pub range: RuntimeAddressRange,
    pub classification: ProcessAddressAllocationClassification,

    pub release_policy: ProcessAddressAllocationReleasePolicy,
    pub executable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessAddressSpaceAllocator {
    allocations: BTreeMap<ProcessAddressAllocationId, ProcessAddressAllocationRecord>,
    next_id: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProcessAddressSearchDirection {
    LowToHigh,
    HighToLow,
}

impl ProcessAddressSpaceAllocator {
    pub fn new() -> Result<Self, RuntimeStateError> {
        let mut allocator = Self {
            allocations: BTreeMap::new(),
            next_id: 1,
        };
        for (purpose, range) in [
            (
                ProcessAddressAllocationPurpose::ImportThunks,
                RuntimeAddressRange::new(IMPORT_THUNK_ARENA_BASE, IMPORT_THUNK_ARENA_SIZE)?,
            ),
            (
                ProcessAddressAllocationPurpose::CallbackReturns,
                RuntimeAddressRange::new(CALLBACK_RETURN_ARENA_BASE, CALLBACK_RETURN_ARENA_SIZE)?,
            ),
            (
                ProcessAddressAllocationPurpose::DynamicExports,
                RuntimeAddressRange::new(DYNAMIC_EXPORT_ARENA_BASE, DYNAMIC_EXPORT_ARENA_SIZE)?,
            ),
        ] {
            allocator.reserve_fixed(
                None,
                fixed_request(
                    purpose,
                    range,
                    ProcessAddressAllocationClassification::RuntimeControl,
                    true,
                ),
            )?;
        }

        let small_heap_band_size = u64::from(RUNTIME_PRIVATE_HEAP_ARENA_STRIDE)
            .checked_mul(u64::from(RUNTIME_PRIVATE_HEAP_BACKING_LIMIT))
            .ok_or_else(|| address_error("private heap band size overflowed"))?;
        let heap_band_range =
            RuntimeAddressRange::new(RUNTIME_PRIVATE_HEAP_ARENA_BASE, small_heap_band_size)?;
        let heap_band = allocator.reserve_fixed(
            None,
            fixed_request(
                ProcessAddressAllocationPurpose::PrivateHeapBand,
                heap_band_range,
                ProcessAddressAllocationClassification::Heap,
                false,
            ),
        )?;
        for slot in 1..=RUNTIME_PRIVATE_HEAP_BACKING_LIMIT {
            let offset = u32::from(slot - 1)
                .checked_mul(RUNTIME_PRIVATE_HEAP_ARENA_STRIDE)
                .ok_or_else(|| address_error("private heap slot offset overflowed"))?;
            let base = RUNTIME_PRIVATE_HEAP_ARENA_BASE
                .0
                .checked_add(offset)
                .map(GuestAddress)
                .ok_or_else(|| address_error("private heap slot base overflowed"))?;
            let range =
                RuntimeAddressRange::new(base, u64::from(RUNTIME_PRIVATE_HEAP_ARENA_STRIDE))?;
            allocator.reserve_fixed(
                Some(heap_band.id),
                fixed_request(
                    ProcessAddressAllocationPurpose::PrivateHeapSlot { slot },
                    range,
                    ProcessAddressAllocationClassification::Heap,
                    false,
                ),
            )?;
        }
        allocator.reserve_fixed(
            None,
            fixed_request(
                ProcessAddressAllocationPurpose::CallbackScratch,
                RuntimeAddressRange::new(CALLBACK_SCRATCH_ARENA_BASE, CALLBACK_SCRATCH_ARENA_SIZE)?,
                ProcessAddressAllocationClassification::RuntimeControl,
                false,
            ),
        )?;
        allocator.reserve_fixed(
            None,
            fixed_request(
                ProcessAddressAllocationPurpose::GraphicsResourceArena,
                RuntimeAddressRange::new(
                    RUNTIME_GRAPHICS_RESOURCE_ARENA_BASE,
                    RUNTIME_GRAPHICS_RESOURCE_ARENA_SIZE,
                )?,
                ProcessAddressAllocationClassification::Graphics,
                false,
            ),
        )?;
        allocator.reserve_fixed(
            None,
            fixed_request(
                ProcessAddressAllocationPurpose::AudioResourceArena,
                RuntimeAddressRange::new(
                    RUNTIME_AUDIO_RESOURCE_ARENA_BASE,
                    RUNTIME_AUDIO_RESOURCE_ARENA_SIZE,
                )?,
                ProcessAddressAllocationClassification::Audio,
                false,
            ),
        )?;
        Ok(allocator)
    }

    pub(crate) fn for_memory(memory: &GuestMemory) -> Result<Self, RuntimeStateError> {
        let mut allocator = Self::new()?;
        for range in memory.image_ranges() {
            let range = range.map_err(RuntimeStateError::from_memory)?;
            allocator.reserve_fixed(
                None,
                fixed_request(
                    ProcessAddressAllocationPurpose::GuestImage,
                    RuntimeAddressRange::new(range.start(), range.byte_length())?,
                    ProcessAddressAllocationClassification::Image,
                    true,
                ),
            )?;
        }
        Ok(allocator)
    }

    pub fn reserve(
        &mut self,
        request: ProcessAddressAllocationRequest,
    ) -> Result<ProcessAddressAllocationRecord, RuntimeStateError> {
        self.reserve_internal(None, request, false)
    }

    pub(crate) fn reserve_with<T>(
        &mut self,
        request: ProcessAddressAllocationRequest,
        initialize: impl FnOnce(&ProcessAddressAllocationRecord) -> Result<T, RuntimeStateError>,
    ) -> Result<T, RuntimeStateError> {
        let (allocation, next_id) = self.build_reservation(
            None,
            request,
            false,
            ProcessAddressSearchDirection::LowToHigh,
        )?;
        let result = initialize(&allocation)?;
        self.allocations.insert(allocation.id, allocation);
        self.next_id = next_id;
        Ok(result)
    }

    pub(crate) fn try_reserve_top_down_with<T>(
        &mut self,
        request: ProcessAddressAllocationRequest,
        initialize: impl FnOnce(&ProcessAddressAllocationRecord) -> Result<Option<T>, RuntimeStateError>,
    ) -> Result<Option<T>, RuntimeStateError> {
        let Some((allocation, next_id)) = self.try_build_reservation_with_pending(
            None,
            request,
            false,
            ProcessAddressSearchDirection::HighToLow,
            self.next_id,
            &[],
        )?
        else {
            return Ok(None);
        };
        let Some(result) = initialize(&allocation)? else {
            return Ok(None);
        };
        self.allocations.insert(allocation.id, allocation);
        self.next_id = next_id;
        Ok(Some(result))
    }

    pub(crate) fn reserve_batch_with<T>(
        &mut self,
        requests: Vec<(
            Option<ProcessAddressAllocationId>,
            ProcessAddressAllocationRequest,
        )>,
        initialize: impl FnOnce(&[ProcessAddressAllocationRecord]) -> Result<T, RuntimeStateError>,
    ) -> Result<T, RuntimeStateError> {
        self.try_reserve_batch_with(requests, |allocations| initialize(allocations).map(Some))?
            .ok_or_else(|| {
                RuntimeStateError::new(
                    "process address reservation has no fitting range or allocation ID",
                )
            })
    }

    pub(crate) fn try_reserve_batch_with<T>(
        &mut self,
        requests: Vec<(
            Option<ProcessAddressAllocationId>,
            ProcessAddressAllocationRequest,
        )>,
        initialize: impl FnOnce(
            &[ProcessAddressAllocationRecord],
        ) -> Result<Option<T>, RuntimeStateError>,
    ) -> Result<Option<T>, RuntimeStateError> {
        if requests.is_empty() {
            return Err(RuntimeStateError::new(
                "process address reservation batch must contain at least one request",
            ));
        }
        let mut allocations = Vec::with_capacity(requests.len());
        let mut next_id = self.next_id;
        for (parent_id, request) in requests {
            let Some((allocation, following_id)) = self.try_build_reservation_with_pending(
                parent_id,
                request,
                false,
                ProcessAddressSearchDirection::LowToHigh,
                next_id,
                &allocations,
            )?
            else {
                return Ok(None);
            };
            allocations.push(allocation);
            next_id = following_id;
        }
        let Some(result) = initialize(&allocations)? else {
            return Ok(None);
        };
        for allocation in allocations {
            self.allocations.insert(allocation.id, allocation);
        }
        self.next_id = next_id;
        Ok(Some(result))
    }

    pub(crate) fn validate_releases(
        &self,
        ids: &[ProcessAddressAllocationId],
    ) -> Result<Vec<&ProcessAddressAllocationRecord>, RuntimeStateError> {
        let requested = ids
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        if requested.len() != ids.len() {
            return Err(RuntimeStateError::new(
                "one process address allocation cannot be released twice in one batch",
            ));
        }
        let mut allocations = Vec::with_capacity(ids.len());
        for id in ids {
            let record = self.allocation(*id)?;
            if record.release_policy != ProcessAddressAllocationReleasePolicy::Explicit {
                return Err(RuntimeStateError::new(
                    "permanent process address allocation cannot be released",
                ));
            }
            if self.allocations.iter().any(|(child_id, allocation)| {
                allocation.parent_id == Some(*id) && !requested.contains(child_id)
            }) {
                return Err(RuntimeStateError::new(
                    "process address allocation cannot be released while children are live",
                ));
            }
            allocations.push(record);
        }
        Ok(allocations)
    }

    pub(crate) fn remove_validated_allocations(&mut self, ids: Vec<ProcessAddressAllocationId>) {
        for id in ids {
            self.allocations.remove(&id);
        }
    }

    #[cfg(test)]
    fn reserve_within(
        &mut self,
        parent_id: ProcessAddressAllocationId,
        request: ProcessAddressAllocationRequest,
    ) -> Result<ProcessAddressAllocationRecord, RuntimeStateError> {
        self.reserve_internal(Some(parent_id), request, false)
    }

    pub fn release(
        &mut self,
        id: ProcessAddressAllocationId,
    ) -> Result<ProcessAddressAllocationRecord, RuntimeStateError> {
        let record = self.allocation(id)?;
        if record.release_policy != ProcessAddressAllocationReleasePolicy::Explicit {
            return Err(RuntimeStateError::new(
                "permanent process address allocation cannot be released",
            ));
        }
        if self
            .allocations
            .values()
            .any(|allocation| allocation.parent_id == Some(id))
        {
            return Err(RuntimeStateError::new(
                "process address allocation cannot be released while children are live",
            ));
        }
        let record = self
            .allocations
            .remove(&id)
            .expect("validated process address allocation");
        Ok(record)
    }

    pub fn allocation(
        &self,
        id: ProcessAddressAllocationId,
    ) -> Result<&ProcessAddressAllocationRecord, RuntimeStateError> {
        let allocation = self.allocations.get(&id).ok_or_else(|| {
            RuntimeStateError::new(format!("process address allocation ID {} is unknown", id.0))
        })?;
        if allocation.id != id {
            return Err(RuntimeStateError::new(
                "process address allocation record disagrees with its live index",
            ));
        }
        Ok(allocation)
    }

    pub fn allocation_for_purpose(
        &self,
        purpose: &ProcessAddressAllocationPurpose,
    ) -> Result<&ProcessAddressAllocationRecord, RuntimeStateError> {
        let mut matches = self
            .allocations
            .values()
            .filter(|allocation| &allocation.purpose == purpose);
        let allocation = matches.next().ok_or_else(|| {
            RuntimeStateError::new("process address allocation purpose is not registered")
        })?;
        if matches.next().is_some() {
            return Err(RuntimeStateError::new(
                "process address allocation purpose identifies multiple records",
            ));
        }
        Ok(allocation)
    }

    pub(crate) fn records(&self) -> impl ExactSizeIterator<Item = &ProcessAddressAllocationRecord> {
        self.allocations.values()
    }

    fn reserve_fixed(
        &mut self,
        parent_id: Option<ProcessAddressAllocationId>,
        request: ProcessAddressAllocationRequest,
    ) -> Result<ProcessAddressAllocationRecord, RuntimeStateError> {
        self.reserve_internal(parent_id, request, true)
    }

    fn reserve_internal(
        &mut self,
        parent_id: Option<ProcessAddressAllocationId>,
        request: ProcessAddressAllocationRequest,
        preferred_required: bool,
    ) -> Result<ProcessAddressAllocationRecord, RuntimeStateError> {
        let (record, next_id) = self.build_reservation(
            parent_id,
            request,
            preferred_required,
            ProcessAddressSearchDirection::LowToHigh,
        )?;
        self.allocations.insert(record.id, record.clone());
        self.next_id = next_id;
        Ok(record)
    }

    fn build_reservation(
        &self,
        parent_id: Option<ProcessAddressAllocationId>,
        request: ProcessAddressAllocationRequest,
        preferred_required: bool,
        direction: ProcessAddressSearchDirection,
    ) -> Result<(ProcessAddressAllocationRecord, u64), RuntimeStateError> {
        self.try_build_reservation_with_pending(
            parent_id,
            request,
            preferred_required,
            direction,
            self.next_id,
            &[],
        )?
        .ok_or_else(|| {
            RuntimeStateError::new(
                "process address reservation has no fitting range or allocation ID",
            )
        })
    }

    fn try_build_reservation_with_pending(
        &self,
        parent_id: Option<ProcessAddressAllocationId>,
        request: ProcessAddressAllocationRequest,
        preferred_required: bool,
        direction: ProcessAddressSearchDirection,
        next_id: u64,
        pending: &[ProcessAddressAllocationRecord],
    ) -> Result<Option<(ProcessAddressAllocationRecord, u64)>, RuntimeStateError> {
        validate_request(&request)?;
        let aligned_reserve_size = align_up(request.reserve_size, GUEST_PAGE_SIZE)
            .ok_or_else(|| address_error("address allocation size overflowed page alignment"))?;
        let total_size = request
            .guard_pages
            .checked_mul(GUEST_PAGE_SIZE as u32)
            .map(u64::from)
            .and_then(|guards| aligned_reserve_size.checked_add(guards))
            .ok_or_else(|| address_error("address allocation size and guards overflowed"))?;
        let search_range =
            RuntimeAddressRange::new(request.search_range.base, request.search_range.byte_length)?;
        if let Some(parent_id) = parent_id {
            let parent = self
                .allocations
                .get(&parent_id)
                .or_else(|| pending.iter().find(|allocation| allocation.id == parent_id))
                .ok_or_else(|| {
                    RuntimeStateError::new(format!(
                        "process address allocation ID {} is unknown",
                        parent_id.0
                    ))
                })?;
            if !parent.range.contains(search_range)? {
                return Err(address_error(
                    "child allocation search range leaves its parent allocation",
                ));
            }
        }
        let preferred = request
            .preferred_address
            .map(|base| {
                if u64::from(base.0) % u64::from(request.alignment) != 0 {
                    return Err(address_error(
                        "preferred address does not satisfy allocation alignment",
                    ));
                }
                Ok(base)
            })
            .transpose()?;
        let available_preferred = preferred.filter(|base| {
            self.range_is_available_with_pending(
                parent_id,
                *base,
                total_size,
                search_range,
                pending,
            )
        });
        if preferred_required && preferred.is_some() && available_preferred.is_none() {
            return Err(RuntimeStateError::new(
                "fixed process address allocation overlaps an existing allocation",
            ));
        }
        let Some(base) = available_preferred.or_else(|| {
            (!preferred_required)
                .then(|| match direction {
                    ProcessAddressSearchDirection::LowToHigh => self.first_fit_with_pending(
                        parent_id,
                        search_range,
                        total_size,
                        request.alignment,
                        pending,
                    ),
                    ProcessAddressSearchDirection::HighToLow => self.last_fit_with_pending(
                        parent_id,
                        search_range,
                        total_size,
                        request.alignment,
                        pending,
                    ),
                })
                .flatten()
        }) else {
            return Ok(None);
        };
        let id = ProcessAddressAllocationId(next_id);
        let Some(next_id) = next_id.checked_add(1) else {
            return Ok(None);
        };
        let record = ProcessAddressAllocationRecord {
            id,
            parent_id,
            purpose: request.purpose,
            range: RuntimeAddressRange::new(base, total_size)?,
            usable_byte_length: request.reserve_size,
            alignment: request.alignment,
            guard_pages: request.guard_pages,
            classification: request.classification,

            release_policy: request.release_policy,
            executable: request.executable,
        };
        Ok(Some((record, next_id)))
    }

    fn first_fit_with_pending(
        &self,
        parent_id: Option<ProcessAddressAllocationId>,
        search: RuntimeAddressRange,
        byte_length: u64,
        alignment: u32,
        pending: &[ProcessAddressAllocationRecord],
    ) -> Option<GuestAddress> {
        let search_end = search.end_exclusive().ok()?;
        let mut candidate = align_up(u64::from(search.base.0), u64::from(alignment))?;
        let mut siblings: Vec<_> = self
            .allocations
            .values()
            .chain(pending.iter())
            .filter(|allocation| allocation.parent_id == parent_id)
            .collect();
        siblings.sort_by_key(|allocation| allocation.range.base.0);
        for allocation in siblings {
            let allocation_start = u64::from(allocation.range.base.0);
            let allocation_end = allocation.range.end_exclusive().ok()?;
            if allocation_end <= candidate {
                continue;
            }
            if candidate.checked_add(byte_length)? <= allocation_start {
                break;
            }
            candidate = align_up(allocation_end, u64::from(alignment))?;
        }
        if candidate.checked_add(byte_length)? > search_end {
            return None;
        }
        Some(GuestAddress(u32::try_from(candidate).ok()?))
    }

    fn last_fit_with_pending(
        &self,
        parent_id: Option<ProcessAddressAllocationId>,
        search: RuntimeAddressRange,
        byte_length: u64,
        alignment: u32,
        pending: &[ProcessAddressAllocationRecord],
    ) -> Option<GuestAddress> {
        let search_start = u64::from(search.base.0);
        let search_end = search.end_exclusive().ok()?;
        let mut candidate = align_down(search_end.checked_sub(byte_length)?, u64::from(alignment))?;
        let mut siblings: Vec<_> = self
            .allocations
            .values()
            .chain(pending.iter())
            .filter(|allocation| allocation.parent_id == parent_id)
            .collect();
        siblings.sort_by_key(|allocation| std::cmp::Reverse(allocation.range.base.0));
        for allocation in siblings {
            let allocation_start = u64::from(allocation.range.base.0);
            let allocation_end = allocation.range.end_exclusive().ok()?;
            let candidate_end = candidate.checked_add(byte_length)?;
            if allocation_end <= candidate || allocation_start >= candidate_end {
                continue;
            }
            candidate = align_down(
                allocation_start.checked_sub(byte_length)?,
                u64::from(alignment),
            )?;
        }
        if candidate < search_start {
            return None;
        }
        let base = GuestAddress(u32::try_from(candidate).ok()?);
        self.range_is_available_with_pending(parent_id, base, byte_length, search, pending)
            .then_some(base)
    }

    fn range_is_available_with_pending(
        &self,
        parent_id: Option<ProcessAddressAllocationId>,
        base: GuestAddress,
        byte_length: u64,
        search: RuntimeAddressRange,
        pending: &[ProcessAddressAllocationRecord],
    ) -> bool {
        let Ok(candidate) = RuntimeAddressRange::new(base, byte_length) else {
            return false;
        };
        if !search.contains(candidate).unwrap_or(false) {
            return false;
        }
        let candidate_start = u64::from(base.0);
        let Ok(candidate_end) = candidate.end_exclusive() else {
            return false;
        };
        self.allocations
            .values()
            .chain(pending.iter())
            .filter(|allocation| allocation.parent_id == parent_id)
            .all(|allocation| {
                let allocation_start = u64::from(allocation.range.base.0);
                let allocation_end = allocation.range.end_exclusive().unwrap_or(0);
                candidate_start >= allocation_end || allocation_start >= candidate_end
            })
    }
}

pub fn validate_process_address_mapping_allocation(
    allocation: &ProcessAddressAllocationRecord,
    requirements: &ProcessAddressMappingRequirements,
) -> Result<(), GuestMemoryError> {
    if allocation.id.0 == 0 {
        return Err(GuestMemoryError::invalid_request(
            "a mapping allocation record requires a nonzero identity",
        ));
    }
    if allocation.purpose != requirements.purpose {
        return Err(GuestMemoryError::conflict(
            "the mapping plan received an allocation record for a different purpose",
        ));
    }
    if allocation.range != requirements.range {
        return Err(GuestMemoryError::conflict(
            "the mapping plan range does not exactly match its allocation record",
        ));
    }
    if allocation.classification != requirements.classification {
        return Err(GuestMemoryError::conflict(
            "the mapping plan classification does not match the allocation record",
        ));
    }
    if allocation.release_policy != requirements.release_policy {
        return Err(GuestMemoryError::conflict(
            "the mapping plan release policy does not match the allocation record",
        ));
    }
    if allocation.executable != requirements.executable {
        return Err(GuestMemoryError::conflict(
            "the mapping plan executable policy does not match the allocation record",
        ));
    }
    Ok(())
}

fn fixed_request(
    purpose: ProcessAddressAllocationPurpose,
    range: RuntimeAddressRange,
    classification: ProcessAddressAllocationClassification,
    executable: bool,
) -> ProcessAddressAllocationRequest {
    ProcessAddressAllocationRequest {
        purpose,

        preferred_address: Some(range.base),
        search_range: range,
        reserve_size: range.byte_length,
        alignment: GUEST_PAGE_SIZE as u32,
        guard_pages: 0,
        classification,

        release_policy: ProcessAddressAllocationReleasePolicy::Permanent,
        executable,
    }
}

fn validate_request(request: &ProcessAddressAllocationRequest) -> Result<(), RuntimeStateError> {
    if request.reserve_size == 0 {
        return Err(address_error(
            "address allocation reserve size must be nonzero",
        ));
    }
    if request.alignment < GUEST_PAGE_SIZE as u32 || !request.alignment.is_power_of_two() {
        return Err(address_error(
            "address allocation alignment must be a page-sized or larger power of two",
        ));
    }
    request.search_range.end_exclusive()?;
    Ok(())
}

fn align_up(value: u64, alignment: u64) -> Option<u64> {
    value
        .checked_add(alignment.checked_sub(1)?)
        .map(|rounded| rounded & !(alignment - 1))
}

fn align_down(value: u64, alignment: u64) -> Option<u64> {
    alignment
        .is_power_of_two()
        .then_some(value & !(alignment - 1))
}

fn address_error(message: impl Into<String>) -> RuntimeStateError {
    RuntimeStateError::new(message)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn search_request(
        purpose: ProcessAddressAllocationPurpose,
        classification: ProcessAddressAllocationClassification,
    ) -> ProcessAddressAllocationRequest {
        ProcessAddressAllocationRequest {
            purpose,

            preferred_address: None,
            search_range: RuntimeAddressRange {
                base: GuestAddress(0x0100_0000),
                byte_length: 0x1000_0000,
            },
            reserve_size: 0x20_000,
            alignment: 0x1_0000,
            guard_pages: 1,
            classification,
            release_policy: ProcessAddressAllocationReleasePolicy::Explicit,
            executable: false,
        }
    }

    #[test]
    fn fixed_address_layout_registers_control_arenas_and_heap_slots() {
        let allocator = ProcessAddressSpaceAllocator::new().unwrap();
        let import = allocator
            .allocation_for_purpose(&ProcessAddressAllocationPurpose::ImportThunks)
            .unwrap();
        assert_eq!(import.range.base, IMPORT_THUNK_ARENA_BASE);
        let callback_scratch = allocator
            .allocation_for_purpose(&ProcessAddressAllocationPurpose::CallbackScratch)
            .unwrap();
        assert_eq!(callback_scratch.range.base, CALLBACK_SCRATCH_ARENA_BASE);
        assert_eq!(
            callback_scratch.range.byte_length,
            CALLBACK_SCRATCH_ARENA_SIZE
        );
        assert_eq!(
            callback_scratch.classification,
            ProcessAddressAllocationClassification::RuntimeControl
        );
        assert_eq!(
            callback_scratch.release_policy,
            ProcessAddressAllocationReleasePolicy::Permanent
        );
        let first_heap = allocator
            .allocation_for_purpose(&ProcessAddressAllocationPurpose::PrivateHeapSlot { slot: 1 })
            .unwrap();
        assert_eq!(first_heap.range.base, RUNTIME_PRIVATE_HEAP_ARENA_BASE);
        assert!(first_heap.parent_id.is_some());
        assert!(!allocator.records().any(|allocation| {
            allocation.purpose == ProcessAddressAllocationPurpose::PrivateHeapLargeAllocation
        }));
        let graphics = allocator
            .allocation_for_purpose(&ProcessAddressAllocationPurpose::GraphicsResourceArena)
            .unwrap();
        assert_eq!(graphics.range.base, RUNTIME_GRAPHICS_RESOURCE_ARENA_BASE);
        assert_eq!(
            graphics.range.byte_length,
            RUNTIME_GRAPHICS_RESOURCE_ARENA_SIZE
        );
        assert_eq!(
            graphics.release_policy,
            ProcessAddressAllocationReleasePolicy::Permanent
        );
        let audio = allocator
            .allocation_for_purpose(&ProcessAddressAllocationPurpose::AudioResourceArena)
            .unwrap();
        assert_eq!(audio.range.base, RUNTIME_AUDIO_RESOURCE_ARENA_BASE);
        assert_eq!(audio.range.byte_length, RUNTIME_AUDIO_RESOURCE_ARENA_SIZE);
        assert_eq!(
            audio.classification,
            ProcessAddressAllocationClassification::Audio
        );
        assert_eq!(
            audio.release_policy,
            ProcessAddressAllocationReleasePolicy::Permanent
        );
    }

    #[test]
    fn batch_reservation_assigns_distinct_child_ranges() {
        let mut allocator = ProcessAddressSpaceAllocator::new().unwrap();
        let arena = allocator
            .allocation_for_purpose(&ProcessAddressAllocationPurpose::GraphicsResourceArena)
            .unwrap()
            .clone();
        let request = || ProcessAddressAllocationRequest {
            purpose: ProcessAddressAllocationPurpose::GraphicsResource,

            preferred_address: None,
            search_range: arena.range,
            reserve_size: 112,
            alignment: GUEST_PAGE_SIZE as u32,
            guard_pages: 0,
            classification: ProcessAddressAllocationClassification::Graphics,

            release_policy: ProcessAddressAllocationReleasePolicy::Explicit,
            executable: false,
        };
        let records = allocator
            .reserve_batch_with(
                vec![(Some(arena.id), request()), (Some(arena.id), request())],
                |allocations| Ok(allocations.to_vec()),
            )
            .unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].range.base, RUNTIME_GRAPHICS_RESOURCE_ARENA_BASE);
        assert_eq!(
            records[1].range.base,
            GuestAddress(RUNTIME_GRAPHICS_RESOURCE_ARENA_BASE.0 + GUEST_PAGE_SIZE as u32)
        );
        assert_eq!(records[0].usable_byte_length, 112);
        assert_eq!(records[1].usable_byte_length, 112);
        let ids = records
            .iter()
            .map(|allocation| allocation.id)
            .collect::<Vec<_>>();
        assert!(ids.iter().all(|id| allocator.allocation(*id).is_ok()));
    }

    #[test]
    fn search_reserves_dll_stacks_and_com_without_fixed_result_addresses() {
        let mut allocator = ProcessAddressSpaceAllocator::new().unwrap();
        let image = allocator
            .reserve(search_request(
                ProcessAddressAllocationPurpose::GuestImage,
                ProcessAddressAllocationClassification::Image,
            ))
            .unwrap();
        let stack_one = allocator
            .reserve(search_request(
                ProcessAddressAllocationPurpose::ThreadStack,
                ProcessAddressAllocationClassification::Stack,
            ))
            .unwrap();
        let stack_two = allocator
            .reserve(search_request(
                ProcessAddressAllocationPurpose::ThreadStack,
                ProcessAddressAllocationClassification::Stack,
            ))
            .unwrap();
        let com = allocator
            .reserve(search_request(
                ProcessAddressAllocationPurpose::ComEndpoint,
                ProcessAddressAllocationClassification::Com,
            ))
            .unwrap();
        for record in [&image, &stack_one, &stack_two, &com] {
            assert_eq!(record.range.base.0 % record.alignment, 0);
            assert_eq!(record.guard_pages, 1);
        }
        assert!(image.range.end_exclusive().unwrap() <= u64::from(stack_one.range.base.0));
        assert!(stack_one.range.end_exclusive().unwrap() <= u64::from(stack_two.range.base.0));
        assert!(stack_two.range.end_exclusive().unwrap() <= u64::from(com.range.base.0));
    }

    #[test]
    fn top_down_reservation_selects_highest_aligned_gap() {
        let mut allocator = ProcessAddressSpaceAllocator::new().unwrap();
        let request = ProcessAddressAllocationRequest {
            purpose: ProcessAddressAllocationPurpose::VirtualMemory,

            preferred_address: None,
            search_range: RuntimeAddressRange {
                base: GuestAddress(0x0001_0000),
                byte_length: 0x7ffe_0000,
            },
            reserve_size: 0x0600_0000,
            alignment: 0x1_0000,
            guard_pages: 0,
            classification: ProcessAddressAllocationClassification::VirtualMemory,

            release_policy: ProcessAddressAllocationReleasePolicy::Explicit,
            executable: false,
        };
        let first = allocator
            .try_reserve_top_down_with(request.clone(), |allocation| Ok(Some(allocation.clone())))
            .unwrap()
            .unwrap();
        assert_eq!(first.range.base, GuestAddress(0x7900_0000));
        assert_eq!(allocator.allocation(first.id).unwrap(), &first);
        let second = allocator
            .try_reserve_top_down_with(request, |allocation| Ok(Some(allocation.clone())))
            .unwrap()
            .unwrap();
        assert_eq!(second.range.base, GuestAddress(0x7300_0000));
    }

    #[test]
    fn optional_reservations_preserve_authority_on_exhaustion_and_reject_malformed_requests() {
        let mut allocator = ProcessAddressSpaceAllocator::new().unwrap();
        let mut request = search_request(
            ProcessAddressAllocationPurpose::MappedFile,
            ProcessAddressAllocationClassification::MappedFile,
        );
        request.search_range = RuntimeAddressRange {
            base: GuestAddress(0x0100_0000),
            byte_length: GUEST_PAGE_SIZE,
        };
        request.reserve_size = GUEST_PAGE_SIZE;
        request.alignment = GUEST_PAGE_SIZE as u32;
        request.guard_pages = 0;
        let before = allocator.clone();
        assert!(
            allocator
                .try_reserve_batch_with(
                    vec![(None, request.clone()), (None, request.clone())],
                    |_| Ok(Some(()))
                )
                .unwrap()
                .is_none()
        );
        assert_eq!(allocator, before);
        allocator.next_id = u64::MAX;
        let exhausted = allocator.clone();
        assert!(
            allocator
                .try_reserve_batch_with(vec![(None, request.clone())], |_| Ok(Some(())))
                .unwrap()
                .is_none()
        );
        assert!(
            allocator
                .try_reserve_top_down_with(request.clone(), |_| Ok(Some(())))
                .unwrap()
                .is_none()
        );
        assert_eq!(allocator, exhausted);
        request.alignment = 3;
        assert!(
            allocator
                .try_reserve_batch_with(vec![(None, request.clone())], |_| Ok(Some(())))
                .is_err()
        );
        assert!(
            allocator
                .try_reserve_top_down_with(request, |_| Ok(Some(())))
                .is_err()
        );
        assert_eq!(allocator, exhausted);
        allocator.next_id = before.next_id;
        allocator
            .try_reserve_batch_with(
                vec![(
                    None,
                    search_request(
                        ProcessAddressAllocationPurpose::MappedFile,
                        ProcessAddressAllocationClassification::MappedFile,
                    ),
                )],
                |_| Ok(Some(())),
            )
            .unwrap()
            .unwrap();
        assert_eq!(allocator.records().len(), before.records().len() + 1);
    }

    #[test]
    fn invalid_or_exhausted_requests_do_not_mutate_registry() {
        let mut allocator = ProcessAddressSpaceAllocator::new().unwrap();
        let before = allocator.clone();
        let mut request = search_request(
            ProcessAddressAllocationPurpose::MappedFile,
            ProcessAddressAllocationClassification::MappedFile,
        );
        request.alignment = 3;
        allocator.reserve(request).unwrap_err();
        assert_eq!(allocator, before);

        let mut overflow = search_request(
            ProcessAddressAllocationPurpose::MappedFile,
            ProcessAddressAllocationClassification::MappedFile,
        );
        overflow.search_range = RuntimeAddressRange {
            base: GuestAddress(0xffff_f000),
            byte_length: 0x2000,
        };
        allocator.reserve(overflow).unwrap_err();
        assert_eq!(allocator, before);

        let mut one_page = search_request(
            ProcessAddressAllocationPurpose::MappedFile,
            ProcessAddressAllocationClassification::MappedFile,
        );
        one_page.search_range = RuntimeAddressRange {
            base: GuestAddress(0x0100_0000),
            byte_length: 0x1000,
        };
        one_page.reserve_size = 0x1000;
        one_page.alignment = 0x1000;
        one_page.guard_pages = 0;
        allocator.reserve(one_page.clone()).unwrap();
        let after_fill = allocator.clone();

        allocator.reserve(one_page).unwrap_err();
        assert_eq!(allocator, after_fill);
    }

    #[test]
    fn failed_initialization_leaves_ranges_and_ids_available_for_retry() {
        let mut allocator = ProcessAddressSpaceAllocator::new().unwrap();
        let request = search_request(
            ProcessAddressAllocationPurpose::MappedFile,
            ProcessAddressAllocationClassification::MappedFile,
        );
        let before = allocator.clone();
        let mut rejected_id = None;
        assert!(
            allocator
                .reserve_with(request.clone(), |allocation| {
                    rejected_id = Some(allocation.id);
                    Err::<(), _>(RuntimeStateError::new("memory initialization fault"))
                })
                .is_err()
        );
        assert_eq!(allocator, before);
        assert!(
            allocator
                .try_reserve_top_down_with(request.clone(), |_| Ok(None::<()>))
                .unwrap()
                .is_none()
        );
        assert_eq!(allocator, before);
        let requests = vec![(None, request.clone()), (None, request.clone())];
        assert!(
            allocator
                .try_reserve_batch_with(requests.clone(), |_| Ok(None::<()>))
                .unwrap()
                .is_none()
        );
        assert_eq!(allocator, before);
        assert!(
            allocator
                .try_reserve_batch_with(requests, |_| {
                    Err::<Option<()>, _>(RuntimeStateError::new("output write fault"))
                })
                .is_err()
        );
        assert_eq!(allocator, before);
        let record = allocator
            .reserve_with(request, |allocation| Ok(allocation.clone()))
            .unwrap();
        assert_eq!(Some(record.id), rejected_id);
        assert_eq!(allocator.allocation(record.id).unwrap(), &record);
    }

    #[test]
    fn fixed_overlap_and_permanent_release_are_rejected_without_mutation() {
        let mut allocator = ProcessAddressSpaceAllocator::new().unwrap();
        let before = allocator.clone();
        allocator
            .reserve_fixed(
                None,
                fixed_request(
                    ProcessAddressAllocationPurpose::CallbackReturns,
                    RuntimeAddressRange::new(
                        GuestAddress(IMPORT_THUNK_ARENA_BASE.0 + 0xf000),
                        CALLBACK_RETURN_ARENA_SIZE,
                    )
                    .unwrap(),
                    ProcessAddressAllocationClassification::RuntimeControl,
                    true,
                ),
            )
            .unwrap_err();
        assert_eq!(allocator, before);
        let import = allocator
            .allocation_for_purpose(&ProcessAddressAllocationPurpose::ImportThunks)
            .unwrap()
            .id;
        let before = allocator.clone();
        allocator.release(import).unwrap_err();
        assert_eq!(allocator, before);
    }

    #[test]
    fn direct_release_validates_children_and_exhaustion_before_removing_records() {
        let mut allocator = ProcessAddressSpaceAllocator::new().unwrap();
        let request = search_request(
            ProcessAddressAllocationPurpose::MappedFile,
            ProcessAddressAllocationClassification::MappedFile,
        );
        let parent = allocator.reserve(request.clone()).unwrap();
        let mut child_request = request.clone();

        child_request.search_range = parent.range;
        child_request.reserve_size = GUEST_PAGE_SIZE;
        child_request.alignment = GUEST_PAGE_SIZE as u32;
        child_request.guard_pages = 0;
        let child = allocator.reserve_within(parent.id, child_request).unwrap();
        let before_parent_release = allocator.clone();
        allocator.release(parent.id).unwrap_err();
        assert_eq!(allocator, before_parent_release);

        assert_eq!(allocator.release(child.id).unwrap(), child);
        assert_eq!(allocator.release(parent.id).unwrap(), parent);
        let before_repeated = allocator.clone();
        allocator.release(parent.id).unwrap_err();
        assert_eq!(allocator, before_repeated);
        let reused = allocator.reserve(request).unwrap();
        assert_eq!(reused.range, parent.range);
        assert_ne!(reused.id, parent.id);
    }

    #[test]
    fn batch_release_validates_identity_policy_and_children_before_mutation() {
        let mut allocator = ProcessAddressSpaceAllocator::new().unwrap();
        let request = search_request(
            ProcessAddressAllocationPurpose::MappedFile,
            ProcessAddressAllocationClassification::MappedFile,
        );
        let parent = allocator.reserve(request.clone()).unwrap();
        let mut child_request = request.clone();

        child_request.search_range = parent.range;
        child_request.reserve_size = GUEST_PAGE_SIZE;
        child_request.alignment = GUEST_PAGE_SIZE as u32;
        child_request.guard_pages = 0;
        let child = allocator.reserve_within(parent.id, child_request).unwrap();
        for ids in [
            vec![parent.id],
            vec![child.id, child.id],
            vec![child.id, ProcessAddressAllocationId(u64::MAX)],
        ] {
            let before = allocator.clone();
            assert!(allocator.validate_releases(&ids).is_err());
            assert_eq!(allocator, before);
        }
        for corrupt_identity in [true, false] {
            let mut rejected = allocator.clone();
            if corrupt_identity {
                rejected.allocations.get_mut(&child.id).unwrap().id.0 += 1;
            } else {
                rejected
                    .allocations
                    .get_mut(&child.id)
                    .unwrap()
                    .release_policy = ProcessAddressAllocationReleasePolicy::Permanent
            }
            let before = rejected.clone();
            assert!(rejected.validate_releases(&[child.id, parent.id]).is_err());
            assert_eq!(rejected, before);
        }
        let before_empty = allocator.clone();
        let empty = allocator.validate_releases(&[]).unwrap();
        allocator.remove_validated_allocations(empty.iter().map(|record| record.id).collect());
        assert_eq!(allocator, before_empty);
        let before_validation = allocator.clone();
        let releases = allocator.validate_releases(&[parent.id, child.id]).unwrap();
        assert_eq!(allocator, before_validation);
        allocator.remove_validated_allocations(releases.iter().map(|record| record.id).collect());
        assert!(allocator.allocation(child.id).is_err());
        assert!(allocator.allocation(parent.id).is_err());
        let reused = allocator.reserve(request).unwrap();
        assert_eq!(reused.range, parent.range);
        assert_ne!(reused.id, parent.id);
    }

    #[test]
    fn checked_reservations_publish_and_release_live_ids() {
        let mut allocator = ProcessAddressSpaceAllocator::new().unwrap();
        let first = allocator
            .reserve_with(
                search_request(
                    ProcessAddressAllocationPurpose::MappedFile,
                    ProcessAddressAllocationClassification::MappedFile,
                ),
                |allocation| Ok(allocation.clone()),
            )
            .unwrap();
        assert_eq!(allocator.allocation(first.id).unwrap(), &first);

        let second = allocator
            .reserve(search_request(
                ProcessAddressAllocationPurpose::MappedFile,
                ProcessAddressAllocationClassification::MappedFile,
            ))
            .unwrap();
        let release = allocator.validate_releases(&[first.id]).unwrap();
        allocator.remove_validated_allocations(release.iter().map(|record| record.id).collect());
        allocator.allocation(first.id).unwrap_err();
        assert_eq!(allocator.allocation(second.id).unwrap(), &second);
    }
}

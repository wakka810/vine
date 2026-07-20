use std::collections::BTreeMap;

use serde::Serialize;

use crate::callbacks::{
    CALLBACK_RETURN_ARENA_BASE, CALLBACK_RETURN_ARENA_SIZE, CALLBACK_SCRATCH_ARENA_BASE,
    CALLBACK_SCRATCH_ARENA_SIZE,
};
use crate::dynamic_exports::{DYNAMIC_EXPORT_ARENA_BASE, DYNAMIC_EXPORT_ARENA_SIZE};
use crate::imports::{IMPORT_THUNK_ARENA_BASE, IMPORT_THUNK_ARENA_SIZE};
use crate::memory::{GUEST_PAGE_SIZE, GuestAddress, GuestMemoryError};

use super::{
    RUNTIME_PRIVATE_HEAP_ARENA_BASE, RUNTIME_PRIVATE_HEAP_ARENA_STRIDE,
    RUNTIME_PRIVATE_HEAP_BACKING_LIMIT, RUNTIME_PRIVATE_HEAP_LARGE_SLOT_BASE,
    RUNTIME_PRIVATE_HEAP_LARGE_SLOT_SIZE, RuntimeStateError,
};

const X86_ADDRESS_SPACE_END: u64 = 0x1_0000_0000;
const MAX_ADDRESS_OWNER_BYTES: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
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
            return Err(address_error(
                "address-space-range-empty",
                "process address range must be nonempty",
            ));
        }
        u64::from(self.base.0)
            .checked_add(self.byte_length)
            .filter(|end| *end <= X86_ADDRESS_SPACE_END)
            .ok_or_else(|| {
                address_error(
                    "address-space-range-overflow",
                    "process address range crosses the x86 address limit",
                )
            })
    }

    pub fn contains(self, other: Self) -> Result<bool, RuntimeStateError> {
        Ok(u64::from(self.base.0) <= u64::from(other.base.0)
            && other.end_exclusive()? <= self.end_exclusive()?)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessAddressSpaceLayout {
    pub import_thunks: RuntimeAddressRange,
    pub callback_returns: RuntimeAddressRange,
    pub dynamic_exports: RuntimeAddressRange,
    pub callback_scratch: RuntimeAddressRange,
}

impl ProcessAddressSpaceLayout {
    pub fn vine_v1() -> Self {
        Self {
            import_thunks: RuntimeAddressRange {
                base: IMPORT_THUNK_ARENA_BASE,
                byte_length: IMPORT_THUNK_ARENA_SIZE,
            },
            callback_returns: RuntimeAddressRange {
                base: CALLBACK_RETURN_ARENA_BASE,
                byte_length: CALLBACK_RETURN_ARENA_SIZE,
            },
            dynamic_exports: RuntimeAddressRange {
                base: DYNAMIC_EXPORT_ARENA_BASE,
                byte_length: DYNAMIC_EXPORT_ARENA_SIZE,
            },
            callback_scratch: RuntimeAddressRange {
                base: CALLBACK_SCRATCH_ARENA_BASE,
                byte_length: CALLBACK_SCRATCH_ARENA_SIZE,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct ProcessAddressAllocationId(pub u64);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ProcessAddressAllocationPurpose {
    ImportThunks,
    CallbackReturns,
    CallbackScratch,
    DynamicExports,
    PrivateHeapBand,
    PrivateHeapSlot { slot: u16 },
    PrivateHeapLargeSlot,
    GuestImage,
    ThreadStack,
    ProcessEnvironment,
    ComEndpoint,
    MappedFile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProcessAddressAllocationClassification {
    RuntimeControl,
    Image,
    Stack,
    Heap,
    ProcessEnvironment,
    Com,
    MappedFile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProcessAddressAllocationLifetime {
    Process,
    Module,
    Thread,
    Object,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProcessAddressAllocationReleasePolicy {
    Permanent,
    Explicit,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessAddressAllocationRecord {
    pub id: ProcessAddressAllocationId,
    pub parent_id: Option<ProcessAddressAllocationId>,
    pub purpose: ProcessAddressAllocationPurpose,
    pub owner: String,
    pub range: RuntimeAddressRange,
    pub usable_byte_length: u64,
    pub alignment: u32,
    pub guard_pages: u32,
    pub classification: ProcessAddressAllocationClassification,
    pub lifetime: ProcessAddressAllocationLifetime,
    pub release_policy: ProcessAddressAllocationReleasePolicy,
    pub executable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessAddressAllocationRequest {
    pub purpose: ProcessAddressAllocationPurpose,
    pub owner: String,
    pub preferred_address: Option<GuestAddress>,
    pub search_range: RuntimeAddressRange,
    pub reserve_size: u64,
    pub alignment: u32,
    pub guard_pages: u32,
    pub classification: ProcessAddressAllocationClassification,
    pub lifetime: ProcessAddressAllocationLifetime,
    pub release_policy: ProcessAddressAllocationReleasePolicy,
    pub executable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessAddressMappingRequirements {
    pub purpose: ProcessAddressAllocationPurpose,
    pub owner: String,
    pub range: RuntimeAddressRange,
    pub classification: ProcessAddressAllocationClassification,
    pub lifetime: ProcessAddressAllocationLifetime,
    pub release_policy: ProcessAddressAllocationReleasePolicy,
    pub executable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessAddressSpaceAllocator {
    allocations: BTreeMap<ProcessAddressAllocationId, ProcessAddressAllocationRecord>,
    next_id: u64,
    #[serde(skip)]
    mutation_generation: u64,
}

#[derive(Debug, Clone)]
pub(crate) struct ProcessAddressReservationPlan {
    expected_generation: u64,
    next_generation: u64,
    expected_next_id: u64,
    next_id: u64,
    pub(crate) allocation: ProcessAddressAllocationRecord,
}

#[derive(Debug, Clone)]
pub(crate) struct ProcessAddressReleasePlan {
    expected_generation: u64,
    next_generation: u64,
    allocations: Vec<ProcessAddressAllocationRecord>,
}

impl ProcessAddressReleasePlan {
    pub(crate) fn allocations(&self) -> &[ProcessAddressAllocationRecord] {
        &self.allocations
    }
}

impl ProcessAddressSpaceAllocator {
    pub fn vine_v1(layout: ProcessAddressSpaceLayout) -> Result<Self, RuntimeStateError> {
        let mut allocator = Self {
            allocations: BTreeMap::new(),
            next_id: 1,
            mutation_generation: 0,
        };
        for (purpose, owner, range) in [
            (
                ProcessAddressAllocationPurpose::ImportThunks,
                "runtime-import-thunks",
                layout.import_thunks,
            ),
            (
                ProcessAddressAllocationPurpose::CallbackReturns,
                "runtime-callback-returns",
                layout.callback_returns,
            ),
            (
                ProcessAddressAllocationPurpose::DynamicExports,
                "runtime-dynamic-exports",
                layout.dynamic_exports,
            ),
        ] {
            allocator.reserve_fixed(
                None,
                fixed_request(
                    purpose,
                    owner,
                    range,
                    ProcessAddressAllocationClassification::RuntimeControl,
                    true,
                ),
            )?;
        }

        let small_heap_band_size = u64::from(RUNTIME_PRIVATE_HEAP_ARENA_STRIDE)
            .checked_mul(u64::from(RUNTIME_PRIVATE_HEAP_BACKING_LIMIT))
            .ok_or_else(|| {
                address_error(
                    "address-space-heap-band-overflow",
                    "private heap band size overflowed",
                )
            })?;
        let heap_band_size = small_heap_band_size
            .checked_add(u64::from(RUNTIME_PRIVATE_HEAP_LARGE_SLOT_SIZE))
            .ok_or_else(|| {
                address_error(
                    "address-space-heap-band-overflow",
                    "private heap band size overflowed while adding the large slot",
                )
            })?;
        let heap_band_range =
            RuntimeAddressRange::new(RUNTIME_PRIVATE_HEAP_ARENA_BASE, heap_band_size)?;
        let heap_band = allocator.reserve_fixed(
            None,
            fixed_request(
                ProcessAddressAllocationPurpose::PrivateHeapBand,
                "runtime-private-heap-band",
                heap_band_range,
                ProcessAddressAllocationClassification::Heap,
                false,
            ),
        )?;
        for slot in 1..=RUNTIME_PRIVATE_HEAP_BACKING_LIMIT {
            let offset = u32::from(slot - 1)
                .checked_mul(RUNTIME_PRIVATE_HEAP_ARENA_STRIDE)
                .ok_or_else(|| {
                    address_error(
                        "address-space-heap-slot-overflow",
                        "private heap slot offset overflowed",
                    )
                })?;
            let base = RUNTIME_PRIVATE_HEAP_ARENA_BASE
                .0
                .checked_add(offset)
                .map(GuestAddress)
                .ok_or_else(|| {
                    address_error(
                        "address-space-heap-slot-overflow",
                        "private heap slot base overflowed",
                    )
                })?;
            let range =
                RuntimeAddressRange::new(base, u64::from(RUNTIME_PRIVATE_HEAP_ARENA_STRIDE))?;
            allocator.reserve_fixed(
                Some(heap_band.id),
                fixed_request(
                    ProcessAddressAllocationPurpose::PrivateHeapSlot { slot },
                    &format!("runtime-private-heap-slot-{slot}"),
                    range,
                    ProcessAddressAllocationClassification::Heap,
                    false,
                ),
            )?;
        }
        allocator.reserve_fixed(
            Some(heap_band.id),
            fixed_request(
                ProcessAddressAllocationPurpose::PrivateHeapLargeSlot,
                "runtime-private-heap-large-slot",
                RuntimeAddressRange::new(
                    RUNTIME_PRIVATE_HEAP_LARGE_SLOT_BASE,
                    u64::from(RUNTIME_PRIVATE_HEAP_LARGE_SLOT_SIZE),
                )?,
                ProcessAddressAllocationClassification::Heap,
                false,
            ),
        )?;
        allocator.reserve_fixed(
            None,
            fixed_request(
                ProcessAddressAllocationPurpose::CallbackScratch,
                "runtime-callback-scratch",
                layout.callback_scratch,
                ProcessAddressAllocationClassification::RuntimeControl,
                false,
            ),
        )?;
        Ok(allocator)
    }

    pub fn reserve(
        &mut self,
        request: ProcessAddressAllocationRequest,
    ) -> Result<ProcessAddressAllocationRecord, RuntimeStateError> {
        self.reserve_internal(None, request, false)
    }

    pub(crate) fn plan_reservation(
        &self,
        request: ProcessAddressAllocationRequest,
    ) -> Result<ProcessAddressReservationPlan, RuntimeStateError> {
        let (allocation, next_id) = self.build_reservation(None, request, false)?;
        Ok(ProcessAddressReservationPlan {
            expected_generation: self.mutation_generation,
            next_generation: self.next_mutation_generation()?,
            expected_next_id: self.next_id,
            next_id,
            allocation,
        })
    }

    pub(crate) fn preflight_reservation_commit(
        &self,
        plan: &ProcessAddressReservationPlan,
    ) -> Result<(), RuntimeStateError> {
        if self.mutation_generation != plan.expected_generation
            || self.next_id != plan.expected_next_id
        {
            return Err(RuntimeStateError::conflict(
                "runtime-address-space",
                "address-allocation-reservation-plan-stale",
                "process address allocations changed after reservation planning",
            ));
        }
        if self.allocations.contains_key(&plan.allocation.id) {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-address-space",
                "address-allocation-reservation-plan-invalid",
                "planned process address allocation identity is already live",
            ));
        }
        Ok(())
    }

    pub(crate) fn commit_reservation(&mut self, plan: ProcessAddressReservationPlan) {
        debug_assert_eq!(self.mutation_generation, plan.expected_generation);
        debug_assert_eq!(self.next_id, plan.expected_next_id);
        let replaced = self.allocations.insert(plan.allocation.id, plan.allocation);
        debug_assert!(replaced.is_none());
        self.next_id = plan.next_id;
        self.mutation_generation = plan.next_generation;
    }

    pub(crate) fn plan_releases(
        &self,
        ids: &[ProcessAddressAllocationId],
    ) -> Result<ProcessAddressReleasePlan, RuntimeStateError> {
        let requested = ids
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        if requested.len() != ids.len() {
            return Err(RuntimeStateError::invalid_input(
                "runtime-address-space",
                "address-allocation-release-duplicate",
                "one process address allocation cannot be released twice in one plan",
            ));
        }
        let mut allocations = Vec::with_capacity(ids.len());
        for id in ids {
            let record = self.allocation(*id)?.clone();
            if record.release_policy != ProcessAddressAllocationReleasePolicy::Explicit {
                return Err(RuntimeStateError::conflict(
                    "runtime-address-space",
                    "address-allocation-release-policy",
                    "permanent process address allocation cannot be released",
                ));
            }
            if self.allocations.values().any(|allocation| {
                allocation.parent_id == Some(*id) && !requested.contains(&allocation.id)
            }) {
                return Err(RuntimeStateError::conflict(
                    "runtime-address-space",
                    "address-allocation-has-children",
                    "process address allocation cannot be released while children are live",
                ));
            }
            allocations.push(record);
        }
        Ok(ProcessAddressReleasePlan {
            expected_generation: self.mutation_generation,
            next_generation: if allocations.is_empty() {
                self.mutation_generation
            } else {
                self.next_mutation_generation()?
            },
            allocations,
        })
    }

    pub(crate) fn preflight_releases(
        &self,
        plan: &ProcessAddressReleasePlan,
    ) -> Result<(), RuntimeStateError> {
        if self.mutation_generation != plan.expected_generation {
            return Err(RuntimeStateError::conflict(
                "runtime-address-space",
                "address-allocation-release-plan-stale",
                "process address allocations changed after release planning",
            ));
        }
        for expected in &plan.allocations {
            if self.allocations.get(&expected.id) != Some(expected) {
                return Err(RuntimeStateError::conflict(
                    "runtime-address-space",
                    "address-allocation-release-plan-stale",
                    "a planned process address allocation changed before release",
                ));
            }
        }
        Ok(())
    }

    pub(crate) fn commit_releases(&mut self, plan: ProcessAddressReleasePlan) {
        debug_assert_eq!(self.mutation_generation, plan.expected_generation);
        for expected in plan.allocations {
            let removed = self.allocations.remove(&expected.id);
            debug_assert_eq!(removed.as_ref(), Some(&expected));
        }
        self.mutation_generation = plan.next_generation;
    }

    pub fn reserve_within(
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
        let plan = self.plan_releases(&[id])?;
        let record = plan.allocations[0].clone();
        self.commit_releases(plan);
        Ok(record)
    }

    pub fn allocation(
        &self,
        id: ProcessAddressAllocationId,
    ) -> Result<&ProcessAddressAllocationRecord, RuntimeStateError> {
        self.allocations.get(&id).ok_or_else(|| {
            RuntimeStateError::not_found(
                "runtime-address-space",
                "address-allocation-id-unknown",
                format!("process address allocation ID {} is unknown", id.0),
            )
        })
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
            RuntimeStateError::not_found(
                "runtime-address-space",
                "address-allocation-purpose-unknown",
                "process address allocation purpose is not registered",
            )
        })?;
        if matches.next().is_some() {
            return Err(RuntimeStateError::conflict(
                "runtime-address-space",
                "address-allocation-purpose-ambiguous",
                "process address allocation purpose identifies multiple records",
            ));
        }
        Ok(allocation)
    }

    pub fn records(&self) -> Vec<ProcessAddressAllocationRecord> {
        self.allocations.values().cloned().collect()
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
        let (record, next_id) = self.build_reservation(parent_id, request, preferred_required)?;
        let next_generation = self.next_mutation_generation()?;
        self.allocations.insert(record.id, record.clone());
        self.next_id = next_id;
        self.mutation_generation = next_generation;
        Ok(record)
    }

    fn build_reservation(
        &self,
        parent_id: Option<ProcessAddressAllocationId>,
        request: ProcessAddressAllocationRequest,
        preferred_required: bool,
    ) -> Result<(ProcessAddressAllocationRecord, u64), RuntimeStateError> {
        validate_request(&request)?;
        let aligned_reserve_size =
            align_up(request.reserve_size, GUEST_PAGE_SIZE).ok_or_else(|| {
                address_error(
                    "address-allocation-size-overflow",
                    "address allocation size overflowed page alignment",
                )
            })?;
        let total_size = request
            .guard_pages
            .checked_mul(GUEST_PAGE_SIZE as u32)
            .map(u64::from)
            .and_then(|guards| aligned_reserve_size.checked_add(guards))
            .ok_or_else(|| {
                address_error(
                    "address-allocation-size-overflow",
                    "address allocation size and guards overflowed",
                )
            })?;
        let search_range =
            RuntimeAddressRange::new(request.search_range.base, request.search_range.byte_length)?;
        if let Some(parent_id) = parent_id {
            let parent = self.allocation(parent_id)?;
            if !parent.range.contains(search_range)? {
                return Err(address_error(
                    "address-allocation-parent-range",
                    "child allocation search range leaves its parent allocation",
                ));
            }
        }
        let preferred = request
            .preferred_address
            .map(|base| {
                if u64::from(base.0) % u64::from(request.alignment) != 0 {
                    return Err(address_error(
                        "address-allocation-preferred-alignment",
                        "preferred address does not satisfy allocation alignment",
                    ));
                }
                Ok(base)
            })
            .transpose()?;
        let available_preferred = preferred
            .filter(|base| self.range_is_available(parent_id, *base, total_size, search_range));
        if preferred_required && preferred.is_some() && available_preferred.is_none() {
            return Err(RuntimeStateError::conflict(
                "runtime-address-space",
                "address-allocation-overlap",
                "fixed process address allocation overlaps an existing allocation",
            ));
        }
        let base = available_preferred
            .or_else(|| {
                (!preferred_required)
                    .then(|| self.first_fit(parent_id, search_range, total_size, request.alignment))
                    .flatten()
            })
            .ok_or_else(|| {
                RuntimeStateError::resource_limit(
                    "runtime-address-space",
                    "address-allocation-exhausted",
                    "no aligned process address range satisfies the request",
                )
            })?;
        let id = ProcessAddressAllocationId(self.next_id);
        let next_id = self.next_id.checked_add(1).ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-address-space",
                "address-allocation-id-exhausted",
                "process address allocation ID space is exhausted",
            )
        })?;
        let record = ProcessAddressAllocationRecord {
            id,
            parent_id,
            purpose: request.purpose,
            owner: request.owner,
            range: RuntimeAddressRange::new(base, total_size)?,
            usable_byte_length: request.reserve_size,
            alignment: request.alignment,
            guard_pages: request.guard_pages,
            classification: request.classification,
            lifetime: request.lifetime,
            release_policy: request.release_policy,
            executable: request.executable,
        };
        Ok((record, next_id))
    }

    fn next_mutation_generation(&self) -> Result<u64, RuntimeStateError> {
        self.mutation_generation.checked_add(1).ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-address-space",
                "address-allocation-mutation-generation-exhausted",
                "process address allocation mutation generation is exhausted",
            )
        })
    }

    fn first_fit(
        &self,
        parent_id: Option<ProcessAddressAllocationId>,
        search: RuntimeAddressRange,
        byte_length: u64,
        alignment: u32,
    ) -> Option<GuestAddress> {
        let search_end = search.end_exclusive().ok()?;
        let mut candidate = align_up(u64::from(search.base.0), u64::from(alignment))?;
        let mut siblings: Vec<_> = self
            .allocations
            .values()
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

    fn range_is_available(
        &self,
        parent_id: Option<ProcessAddressAllocationId>,
        base: GuestAddress,
        byte_length: u64,
        search: RuntimeAddressRange,
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
) -> Result<(), RuntimeStateError> {
    if allocation.id.0 == 0 {
        return Err(RuntimeStateError::invalid_input(
            "runtime-address-space",
            "address-allocation-mapping-id-zero",
            "a mapping allocation record requires a nonzero identity",
        ));
    }
    if allocation.purpose != requirements.purpose {
        return Err(RuntimeStateError::conflict(
            "runtime-address-space",
            "address-allocation-mapping-purpose-mismatch",
            "the mapping plan received an allocation record for a different purpose",
        ));
    }
    if allocation.owner != requirements.owner {
        return Err(RuntimeStateError::conflict(
            "runtime-address-space",
            "address-allocation-mapping-owner-mismatch",
            "the mapping plan owner does not match the allocation record owner",
        ));
    }
    if allocation.range != requirements.range {
        return Err(RuntimeStateError::conflict(
            "runtime-address-space",
            "address-allocation-mapping-range-mismatch",
            "the mapping plan range does not exactly match its allocation record",
        ));
    }
    if allocation.classification != requirements.classification {
        return Err(RuntimeStateError::conflict(
            "runtime-address-space",
            "address-allocation-mapping-classification-mismatch",
            "the mapping plan classification does not match the allocation record",
        ));
    }
    if allocation.lifetime != requirements.lifetime {
        return Err(RuntimeStateError::conflict(
            "runtime-address-space",
            "address-allocation-mapping-lifetime-mismatch",
            "the mapping plan lifetime does not match the allocation record",
        ));
    }
    if allocation.release_policy != requirements.release_policy {
        return Err(RuntimeStateError::conflict(
            "runtime-address-space",
            "address-allocation-mapping-release-policy-mismatch",
            "the mapping plan release policy does not match the allocation record",
        ));
    }
    if allocation.executable != requirements.executable {
        return Err(RuntimeStateError::conflict(
            "runtime-address-space",
            "address-allocation-mapping-executable-policy-mismatch",
            "the mapping plan executable policy does not match the allocation record",
        ));
    }
    Ok(())
}

pub(crate) fn validate_process_address_mapping_allocation_for_memory(
    allocation: &ProcessAddressAllocationRecord,
    requirements: &ProcessAddressMappingRequirements,
) -> Result<(), GuestMemoryError> {
    validate_process_address_mapping_allocation(allocation, requirements).map_err(|error| {
        match error.class {
            super::RuntimeStateErrorClass::InvalidInput
            | super::RuntimeStateErrorClass::NotFound => {
                GuestMemoryError::invalid_request(error.stage, error.code, error.message)
            }
            super::RuntimeStateErrorClass::Conflict => {
                GuestMemoryError::conflict(error.stage, error.code, error.message)
            }
            super::RuntimeStateErrorClass::ResourceLimit => {
                GuestMemoryError::resource_limit(error.stage, error.code, error.message)
            }
            super::RuntimeStateErrorClass::RuntimeDefect => {
                GuestMemoryError::runtime_defect(error.stage, error.code, error.message)
            }
        }
    })
}

fn fixed_request(
    purpose: ProcessAddressAllocationPurpose,
    owner: &str,
    range: RuntimeAddressRange,
    classification: ProcessAddressAllocationClassification,
    executable: bool,
) -> ProcessAddressAllocationRequest {
    ProcessAddressAllocationRequest {
        purpose,
        owner: owner.to_owned(),
        preferred_address: Some(range.base),
        search_range: range,
        reserve_size: range.byte_length,
        alignment: GUEST_PAGE_SIZE as u32,
        guard_pages: 0,
        classification,
        lifetime: ProcessAddressAllocationLifetime::Process,
        release_policy: ProcessAddressAllocationReleasePolicy::Permanent,
        executable,
    }
}

fn validate_request(request: &ProcessAddressAllocationRequest) -> Result<(), RuntimeStateError> {
    if request.owner.is_empty()
        || request.owner.len() > MAX_ADDRESS_OWNER_BYTES
        || !request.owner.is_ascii()
        || request.owner.as_bytes().contains(&0)
    {
        return Err(address_error(
            "address-allocation-owner-invalid",
            "address allocation owner must be bounded nonempty ASCII",
        ));
    }
    if request.reserve_size == 0 {
        return Err(address_error(
            "address-allocation-size-zero",
            "address allocation reserve size must be nonzero",
        ));
    }
    if request.alignment < GUEST_PAGE_SIZE as u32 || !request.alignment.is_power_of_two() {
        return Err(address_error(
            "address-allocation-alignment-invalid",
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

fn address_error(code: &'static str, message: impl Into<String>) -> RuntimeStateError {
    RuntimeStateError::invalid_input("runtime-address-space", code, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn search_request(
        purpose: ProcessAddressAllocationPurpose,
        owner: &str,
        classification: ProcessAddressAllocationClassification,
        lifetime: ProcessAddressAllocationLifetime,
    ) -> ProcessAddressAllocationRequest {
        ProcessAddressAllocationRequest {
            purpose,
            owner: owner.to_owned(),
            preferred_address: None,
            search_range: RuntimeAddressRange {
                base: GuestAddress(0x0100_0000),
                byte_length: 0x1000_0000,
            },
            reserve_size: 0x20_000,
            alignment: 0x1_0000,
            guard_pages: 1,
            classification,
            lifetime,
            release_policy: ProcessAddressAllocationReleasePolicy::Explicit,
            executable: false,
        }
    }

    #[test]
    fn vine_layout_registers_control_arenas_and_heap_slots() {
        let allocator =
            ProcessAddressSpaceAllocator::vine_v1(ProcessAddressSpaceLayout::vine_v1()).unwrap();
        assert_eq!(allocator.records().len(), 38);
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
        let large_heap = allocator
            .allocation_for_purpose(&ProcessAddressAllocationPurpose::PrivateHeapLargeSlot)
            .unwrap();
        assert_eq!(large_heap.range.base, RUNTIME_PRIVATE_HEAP_LARGE_SLOT_BASE);
        assert_eq!(
            large_heap.range.byte_length,
            u64::from(RUNTIME_PRIVATE_HEAP_LARGE_SLOT_SIZE)
        );
        assert_eq!(large_heap.parent_id, first_heap.parent_id);
    }

    #[test]
    fn search_reserves_dll_stacks_and_com_without_fixed_result_addresses() {
        let mut allocator =
            ProcessAddressSpaceAllocator::vine_v1(ProcessAddressSpaceLayout::vine_v1()).unwrap();
        let image = allocator
            .reserve(search_request(
                ProcessAddressAllocationPurpose::GuestImage,
                "test-dll",
                ProcessAddressAllocationClassification::Image,
                ProcessAddressAllocationLifetime::Module,
            ))
            .unwrap();
        let stack_one = allocator
            .reserve(search_request(
                ProcessAddressAllocationPurpose::ThreadStack,
                "test-stack-1",
                ProcessAddressAllocationClassification::Stack,
                ProcessAddressAllocationLifetime::Thread,
            ))
            .unwrap();
        let stack_two = allocator
            .reserve(search_request(
                ProcessAddressAllocationPurpose::ThreadStack,
                "test-stack-2",
                ProcessAddressAllocationClassification::Stack,
                ProcessAddressAllocationLifetime::Thread,
            ))
            .unwrap();
        let com = allocator
            .reserve(search_request(
                ProcessAddressAllocationPurpose::ComEndpoint,
                "test-com",
                ProcessAddressAllocationClassification::Com,
                ProcessAddressAllocationLifetime::Object,
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
    fn invalid_or_exhausted_requests_do_not_mutate_registry() {
        let mut allocator =
            ProcessAddressSpaceAllocator::vine_v1(ProcessAddressSpaceLayout::vine_v1()).unwrap();
        let before = allocator.clone();
        let mut request = search_request(
            ProcessAddressAllocationPurpose::MappedFile,
            "bad-alignment",
            ProcessAddressAllocationClassification::MappedFile,
            ProcessAddressAllocationLifetime::Object,
        );
        request.alignment = 3;
        assert_eq!(
            allocator.reserve(request).unwrap_err().code,
            "address-allocation-alignment-invalid"
        );
        assert_eq!(allocator, before);

        let mut overflow = search_request(
            ProcessAddressAllocationPurpose::MappedFile,
            "overflow",
            ProcessAddressAllocationClassification::MappedFile,
            ProcessAddressAllocationLifetime::Object,
        );
        overflow.search_range = RuntimeAddressRange {
            base: GuestAddress(0xffff_f000),
            byte_length: 0x2000,
        };
        assert_eq!(
            allocator.reserve(overflow).unwrap_err().code,
            "address-space-range-overflow"
        );
        assert_eq!(allocator, before);

        let mut one_page = search_request(
            ProcessAddressAllocationPurpose::MappedFile,
            "one-page",
            ProcessAddressAllocationClassification::MappedFile,
            ProcessAddressAllocationLifetime::Object,
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
        one_page.owner = "exhausted".to_owned();
        assert_eq!(
            allocator.reserve(one_page).unwrap_err().code,
            "address-allocation-exhausted"
        );
        assert_eq!(allocator, after_fill);
    }

    #[test]
    fn fixed_overlap_and_permanent_release_are_rejected_without_mutation() {
        let layout = ProcessAddressSpaceLayout {
            import_thunks: RuntimeAddressRange {
                base: GuestAddress(0x7f00_0000),
                byte_length: 0x1_0000,
            },
            callback_returns: RuntimeAddressRange {
                base: GuestAddress(0x7f00_f000),
                byte_length: 0x1_0000,
            },
            dynamic_exports: RuntimeAddressRange {
                base: GuestAddress(0x7f02_0000),
                byte_length: 0x1_0000,
            },
            callback_scratch: RuntimeAddressRange {
                base: GuestAddress(0x0050_1000),
                byte_length: 0x1_0000,
            },
        };
        assert_eq!(
            ProcessAddressSpaceAllocator::vine_v1(layout)
                .unwrap_err()
                .code,
            "address-allocation-overlap"
        );

        let mut allocator =
            ProcessAddressSpaceAllocator::vine_v1(ProcessAddressSpaceLayout::vine_v1()).unwrap();
        let import = allocator
            .allocation_for_purpose(&ProcessAddressAllocationPurpose::ImportThunks)
            .unwrap()
            .id;
        let before = allocator.clone();
        assert_eq!(
            allocator.release(import).unwrap_err().code,
            "address-allocation-release-policy"
        );
        assert_eq!(allocator, before);
    }

    #[test]
    fn reservation_and_release_plans_are_mutation_free_and_reject_stale_commits() {
        let mut allocator =
            ProcessAddressSpaceAllocator::vine_v1(ProcessAddressSpaceLayout::vine_v1()).unwrap();
        let first_plan = allocator
            .plan_reservation(search_request(
                ProcessAddressAllocationPurpose::MappedFile,
                "planned-first",
                ProcessAddressAllocationClassification::MappedFile,
                ProcessAddressAllocationLifetime::Object,
            ))
            .unwrap();
        let first = first_plan.allocation.clone();
        assert_eq!(
            allocator.allocation(first.id).unwrap_err().code,
            "address-allocation-id-unknown"
        );
        allocator.preflight_reservation_commit(&first_plan).unwrap();
        allocator.commit_reservation(first_plan);
        assert_eq!(allocator.allocation(first.id).unwrap(), &first);

        let stale_reservation = allocator
            .plan_reservation(search_request(
                ProcessAddressAllocationPurpose::MappedFile,
                "planned-stale",
                ProcessAddressAllocationClassification::MappedFile,
                ProcessAddressAllocationLifetime::Object,
            ))
            .unwrap();
        let stale_release = allocator.plan_releases(&[first.id]).unwrap();
        let second = allocator
            .reserve(search_request(
                ProcessAddressAllocationPurpose::MappedFile,
                "committed-second",
                ProcessAddressAllocationClassification::MappedFile,
                ProcessAddressAllocationLifetime::Object,
            ))
            .unwrap();
        let before_stale = allocator.clone();
        assert_eq!(
            allocator
                .preflight_reservation_commit(&stale_reservation)
                .unwrap_err()
                .code,
            "address-allocation-reservation-plan-stale"
        );
        assert_eq!(
            allocator
                .preflight_releases(&stale_release)
                .unwrap_err()
                .code,
            "address-allocation-release-plan-stale"
        );
        assert_eq!(allocator, before_stale);

        let release = allocator.plan_releases(&[first.id]).unwrap();
        allocator.preflight_releases(&release).unwrap();
        allocator.commit_releases(release);
        assert_eq!(
            allocator.allocation(first.id).unwrap_err().code,
            "address-allocation-id-unknown"
        );
        assert_eq!(allocator.allocation(second.id).unwrap(), &second);
    }
}

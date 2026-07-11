use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::ops::{Deref, DerefMut, Range};

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::{MappingProtection, ModuleIdentity, Pe32LoadPlan};

pub const GUEST_PAGE_SIZE: u64 = 4_096;
const GUEST_PAGE_SIZE_USIZE: usize = GUEST_PAGE_SIZE as usize;
const GUEST_ADDRESS_SPACE_SIZE: u64 = 1_u64 << 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct GuestAddress(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuestRange {
    start: GuestAddress,
    byte_length: u64,
}

impl GuestRange {
    pub fn new(start: GuestAddress, byte_length: u64) -> Result<Self, GuestMemoryError> {
        if byte_length == 0 {
            return Err(GuestMemoryError::invalid_request(
                "guest-range",
                "zero-length-range",
                "guest range must contain at least one byte",
            ));
        }
        let end = u64::from(start.0).checked_add(byte_length).ok_or_else(|| {
            GuestMemoryError::invalid_request(
                "guest-range",
                "guest-range-overflow",
                "guest range end overflowed 64-bit accounting",
            )
        })?;
        if end > GUEST_ADDRESS_SPACE_SIZE {
            return Err(GuestMemoryError::invalid_request(
                "guest-range",
                "guest-range-out-of-bounds",
                "guest range extends beyond the 4 GiB virtual address space",
            ));
        }
        Ok(Self { start, byte_length })
    }

    pub fn end(self) -> u64 {
        u64::from(self.start.0) + self.byte_length
    }

    pub fn start(self) -> GuestAddress {
        self.start
    }

    pub fn byte_length(self) -> u64 {
        self.byte_length
    }

    pub fn page_count(self) -> usize {
        let first_page = u64::from(self.start.0) / GUEST_PAGE_SIZE;
        let last_page = (self.end() - 1) / GUEST_PAGE_SIZE;
        (last_page - first_page + 1) as usize
    }

    fn is_page_aligned(self) -> bool {
        u64::from(self.start.0).is_multiple_of(GUEST_PAGE_SIZE)
            && self.byte_length.is_multiple_of(GUEST_PAGE_SIZE)
    }

    fn as_u64_range(self) -> Range<u64> {
        u64::from(self.start.0)..self.end()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryLimits {
    pub max_physical_pages: usize,
    pub max_regions: usize,
    pub max_owners: usize,
    pub max_access_bytes: usize,
}

impl Default for MemoryLimits {
    fn default() -> Self {
        Self {
            max_physical_pages: 65_536,
            max_regions: 16_384,
            max_owners: 4_096,
            max_access_bytes: 16 * 1024 * 1024,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct MemoryOwnerId(pub u32);

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum MemoryOwnerKind {
    Image { source_sha256: String },
    Anonymous,
    Runtime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum MemoryRegionKind {
    Image,
    Stack,
    ThreadEnvironmentBlock,
    ProcessEnvironmentBlock,
    ProcessParameters,
    CommandLine,
    Heap,
    Anonymous,
    Runtime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageProtection {
    pub read: bool,
    pub write: bool,
    pub execute: bool,
}

impl From<MappingProtection> for PageProtection {
    fn from(protection: MappingProtection) -> Self {
        Self {
            read: protection.read,
            write: protection.write,
            execute: protection.execute,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum MemoryAccess {
    Read,
    Write,
    Execute,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum MemoryFaultReason {
    Unmapped,
    ReadDenied,
    WriteDenied,
    ExecuteDenied,
    OwnerUnsealed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryFault {
    pub address: GuestAddress,
    pub access: MemoryAccess,
    pub reason: MemoryFaultReason,
    pub page_fault_error_code: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryInvalidation {
    pub generation: u64,
    pub data_tlb: bool,
    pub instruction_fetch: bool,
    pub generated_code: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheEffects {
    pub flush_tlb: bool,
    pub invalidate_instruction_fetch: bool,
    pub invalidate_generated_code: bool,
}

impl CacheEffects {
    fn merge(self, other: Self) -> Self {
        Self {
            flush_tlb: self.flush_tlb || other.flush_tlb,
            invalidate_instruction_fetch: self.invalidate_instruction_fetch
                || other.invalidate_instruction_fetch,
            invalidate_generated_code: self.invalidate_generated_code
                || other.invalidate_generated_code,
        }
    }
}

impl From<MemoryInvalidation> for CacheEffects {
    fn from(value: MemoryInvalidation) -> Self {
        Self {
            flush_tlb: value.data_tlb,
            invalidate_instruction_fetch: value.instruction_fetch,
            invalidate_generated_code: value.generated_code,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "operation", rename_all = "kebab-case")]
pub enum MappingOperation {
    Reserve {
        owner: MemoryOwnerId,
        kind: MemoryRegionKind,
        range: GuestRange,
    },
    Commit {
        owner: MemoryOwnerId,
        range: GuestRange,
        protection: PageProtection,
    },
    Protect {
        owner: MemoryOwnerId,
        range: GuestRange,
        protection: PageProtection,
    },
    Decommit {
        owner: MemoryOwnerId,
        range: GuestRange,
    },
    Release {
        owner: MemoryOwnerId,
        range: GuestRange,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirtyPage {
    pub address: GuestAddress,
    pub owner: MemoryOwnerId,
    pub owner_sealed: bool,
    pub protection: PageProtection,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryDelta {
    pub from_generation: u64,
    pub generation: u64,
    pub page_size: u64,
    pub mapping_operations: Vec<MappingOperation>,
    pub dirty_pages: Vec<DirtyPage>,
    pub cache_effects: CacheEffects,
    pub transferred_bytes: usize,
    pub content_sha256: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuestWriteResult {
    pub bytes_written: usize,
    pub dirty_pages: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invalidation: Option<MemoryInvalidation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuestPageReconciliation {
    pub generation: u64,
    pub address: GuestAddress,
    pub owner: MemoryOwnerId,
    pub bytes_reconciled: usize,
    pub changed_bytes: usize,
    pub content_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageMemoryLayout {
    pub source: ModuleIdentity,
    pub owner: MemoryOwnerId,
    pub image_range: GuestRange,
    pub reserved_pages: usize,
    pub committed_pages: usize,
    pub physical_pages: usize,
    pub readable_pages: usize,
    pub writable_pages: usize,
    pub executable_pages: usize,
    pub owner_sealed: bool,
    pub invalidation: MemoryInvalidation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryStats {
    pub owners: usize,
    pub image_owners: usize,
    pub sealed_owners: usize,
    pub regions: usize,
    pub image_regions: usize,
    pub reserved_pages: usize,
    pub committed_pages: usize,
    pub physical_pages: usize,
    pub dirty_pages: usize,
    pub generation: u64,
    pub acknowledged_generation: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pending_invalidation: Option<MemoryInvalidation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryReflectionPage {
    pub address: GuestAddress,
    pub owner: MemoryOwnerId,
    pub owner_sealed: bool,
    pub protection: PageProtection,
    pub dirty: bool,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryReflection {
    pub invalidation: MemoryInvalidation,
    pub page_size: u64,
    pub pages: Vec<MemoryReflectionPage>,
    pub content_sha256: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum GuestMemoryErrorClass {
    InvalidRequest,
    ResourceLimit,
    Conflict,
    SynchronizationRequired,
    Fault,
    RuntimeDefect,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuestMemoryError {
    pub class: GuestMemoryErrorClass,
    pub stage: &'static str,
    pub code: &'static str,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fault: Option<MemoryFault>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pending_invalidation: Option<MemoryInvalidation>,
}

impl GuestMemoryError {
    pub(crate) fn invalid_request(
        stage: &'static str,
        code: &'static str,
        message: impl Into<String>,
    ) -> Self {
        Self::new(GuestMemoryErrorClass::InvalidRequest, stage, code, message)
    }

    pub(crate) fn resource_limit(
        stage: &'static str,
        code: &'static str,
        message: impl Into<String>,
    ) -> Self {
        Self::new(GuestMemoryErrorClass::ResourceLimit, stage, code, message)
    }

    pub(crate) fn conflict(
        stage: &'static str,
        code: &'static str,
        message: impl Into<String>,
    ) -> Self {
        Self::new(GuestMemoryErrorClass::Conflict, stage, code, message)
    }

    fn synchronization(
        code: &'static str,
        message: impl Into<String>,
        pending: MemoryInvalidation,
    ) -> Self {
        let mut error = Self::new(
            GuestMemoryErrorClass::SynchronizationRequired,
            "memory-invalidation",
            code,
            message,
        );
        error.pending_invalidation = Some(pending);
        error
    }

    fn fault(fault: MemoryFault) -> Self {
        let mut error = Self::new(
            GuestMemoryErrorClass::Fault,
            "guest-access",
            "guest-page-fault",
            format!(
                "{:?} at {:#010x} failed: {:?}, PFEC {:#x}",
                fault.access, fault.address.0, fault.reason, fault.page_fault_error_code
            ),
        );
        error.fault = Some(fault);
        error
    }

    pub(crate) fn runtime_defect(
        stage: &'static str,
        code: &'static str,
        message: impl Into<String>,
    ) -> Self {
        Self::new(GuestMemoryErrorClass::RuntimeDefect, stage, code, message)
    }

    fn new(
        class: GuestMemoryErrorClass,
        stage: &'static str,
        code: &'static str,
        message: impl Into<String>,
    ) -> Self {
        Self {
            class,
            stage,
            code,
            message: message.into(),
            fault: None,
            pending_invalidation: None,
        }
    }
}

impl fmt::Display for GuestMemoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} / {}: {}",
            self.stage, self.code, self.message
        )
    }
}

impl std::error::Error for GuestMemoryError {}

#[derive(Debug, Clone)]
struct OwnerState {
    kind: MemoryOwnerKind,
    sealed: bool,
}

#[derive(Debug, Clone)]
struct MemoryRegion {
    range: GuestRange,
    owner: MemoryOwnerId,
    kind: MemoryRegionKind,
}

#[derive(Debug, Clone, Copy)]
struct FrameId(usize);

#[derive(Debug, Clone)]
struct PageState {
    owner: MemoryOwnerId,
    frame: FrameId,
    protection: PageProtection,
    dirty_generation: Option<u64>,
}

#[derive(Debug, Clone)]
struct PhysicalFrames {
    limit: usize,
    frames: Vec<Option<Box<[u8; GUEST_PAGE_SIZE_USIZE]>>>,
    free: Vec<usize>,
    in_use: usize,
}

#[derive(Debug, Clone)]
struct MemoryChange {
    generation: u64,
    mapping_operations: Vec<MappingOperation>,
    cache_effects: CacheEffects,
}

impl PhysicalFrames {
    fn new(limit: usize) -> Self {
        Self {
            limit,
            frames: Vec::new(),
            free: Vec::new(),
            in_use: 0,
        }
    }

    fn available(&self) -> usize {
        self.limit.saturating_sub(self.in_use)
    }

    fn allocate_zeroed(&mut self) -> Result<FrameId, GuestMemoryError> {
        if self.in_use >= self.limit {
            return Err(GuestMemoryError::resource_limit(
                "physical-allocator",
                "physical-page-quota-exceeded",
                format!("physical page quota {} was reached", self.limit),
            ));
        }
        let bytes = Box::new([0; GUEST_PAGE_SIZE_USIZE]);
        let index = if let Some(index) = self.free.pop() {
            let slot = self.frames.get_mut(index).ok_or_else(|| {
                GuestMemoryError::runtime_defect(
                    "physical-allocator",
                    "free-frame-index-invalid",
                    "free frame index is outside the allocator table",
                )
            })?;
            if slot.is_some() {
                return Err(GuestMemoryError::runtime_defect(
                    "physical-allocator",
                    "free-frame-still-owned",
                    "free frame list points to an allocated frame",
                ));
            }
            *slot = Some(bytes);
            index
        } else {
            self.frames.push(Some(bytes));
            self.frames.len() - 1
        };
        self.in_use += 1;
        Ok(FrameId(index))
    }

    fn release(&mut self, frame: FrameId) -> Result<(), GuestMemoryError> {
        let slot = self.frames.get_mut(frame.0).ok_or_else(|| {
            GuestMemoryError::runtime_defect(
                "physical-allocator",
                "released-frame-index-invalid",
                "released frame index is outside the allocator table",
            )
        })?;
        if slot.take().is_none() {
            return Err(GuestMemoryError::runtime_defect(
                "physical-allocator",
                "physical-frame-double-release",
                "physical frame was released more than once",
            ));
        }
        self.free.push(frame.0);
        self.in_use = self.in_use.checked_sub(1).ok_or_else(|| {
            GuestMemoryError::runtime_defect(
                "physical-allocator",
                "physical-page-accounting-underflow",
                "physical page usage underflowed during release",
            )
        })?;
        Ok(())
    }

    fn bytes(&self, frame: FrameId) -> Result<&[u8; GUEST_PAGE_SIZE_USIZE], GuestMemoryError> {
        self.frames
            .get(frame.0)
            .and_then(Option::as_deref)
            .ok_or_else(|| {
                GuestMemoryError::runtime_defect(
                    "physical-allocator",
                    "physical-frame-not-allocated",
                    "page table references a physical frame that is not allocated",
                )
            })
    }

    fn bytes_mut(
        &mut self,
        frame: FrameId,
    ) -> Result<&mut [u8; GUEST_PAGE_SIZE_USIZE], GuestMemoryError> {
        self.frames
            .get_mut(frame.0)
            .and_then(Option::as_deref_mut)
            .ok_or_else(|| {
                GuestMemoryError::runtime_defect(
                    "physical-allocator",
                    "physical-frame-not-allocated",
                    "page table references a physical frame that is not allocated",
                )
            })
    }
}

#[derive(Debug, Clone, Copy)]
struct AccessSegment {
    virtual_page: u32,
    page_offset: usize,
    byte_length: usize,
    fault_address: GuestAddress,
}

#[derive(Debug, Clone)]
pub struct GuestMemory {
    limits: MemoryLimits,
    owners: BTreeMap<MemoryOwnerId, OwnerState>,
    regions: BTreeMap<u32, MemoryRegion>,
    pages: BTreeMap<u32, PageState>,
    frames: PhysicalFrames,
    next_owner: u32,
    generation: u64,
    acknowledged_generation: u64,
    pending_changes: Vec<MemoryChange>,
    offered_generations: BTreeSet<u64>,
    offered_full_reflections: BTreeSet<u64>,
    initial_reflection_acknowledged: bool,
}

pub struct GuestMemoryTransaction<'a> {
    memory: &'a mut GuestMemory,
    rollback: Option<Box<GuestMemory>>,
}

impl Deref for GuestMemoryTransaction<'_> {
    type Target = GuestMemory;

    fn deref(&self) -> &Self::Target {
        self.memory
    }
}

impl DerefMut for GuestMemoryTransaction<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.memory
    }
}

impl GuestMemoryTransaction<'_> {
    pub fn commit_transaction(mut self) {
        self.rollback = None;
    }
}

impl Drop for GuestMemoryTransaction<'_> {
    fn drop(&mut self) {
        if let Some(rollback) = self.rollback.take() {
            *self.memory = *rollback;
        }
    }
}

impl GuestMemory {
    pub fn new(limits: MemoryLimits) -> Result<Self, GuestMemoryError> {
        if limits.max_physical_pages == 0
            || limits.max_regions == 0
            || limits.max_owners == 0
            || limits.max_access_bytes == 0
        {
            return Err(GuestMemoryError::invalid_request(
                "guest-memory",
                "zero-memory-limit",
                "all guest-memory limits must be greater than zero",
            ));
        }
        Ok(Self {
            limits,
            owners: BTreeMap::new(),
            regions: BTreeMap::new(),
            pages: BTreeMap::new(),
            frames: PhysicalFrames::new(limits.max_physical_pages),
            next_owner: 1,
            generation: 0,
            acknowledged_generation: 0,
            pending_changes: Vec::new(),
            offered_generations: BTreeSet::new(),
            offered_full_reflections: BTreeSet::new(),
            initial_reflection_acknowledged: false,
        })
    }

    pub fn begin_transaction(&mut self) -> GuestMemoryTransaction<'_> {
        let rollback = Box::new(self.clone());
        GuestMemoryTransaction {
            memory: self,
            rollback: Some(rollback),
        }
    }

    pub fn limits(&self) -> MemoryLimits {
        self.limits
    }

    pub(crate) fn owner_is_sealed(&self, owner: MemoryOwnerId) -> Result<bool, GuestMemoryError> {
        Ok(self.require_owner(owner)?.sealed)
    }

    pub(crate) fn preflight_growth(
        &self,
        additional_owners: usize,
        additional_regions: usize,
        additional_physical_pages: usize,
        stage: &'static str,
    ) -> Result<(), GuestMemoryError> {
        let owners = self
            .owners
            .len()
            .checked_add(additional_owners)
            .ok_or_else(|| {
                GuestMemoryError::resource_limit(
                    stage,
                    "memory-owner-count-overflow",
                    "required memory owner count overflowed host accounting",
                )
            })?;
        if owners > self.limits.max_owners {
            return Err(GuestMemoryError::resource_limit(
                stage,
                "memory-owner-limit-exceeded",
                format!(
                    "operation needs {additional_owners} additional owners; {} of {} are live",
                    self.owners.len(),
                    self.limits.max_owners
                ),
            ));
        }
        let regions = self
            .regions
            .len()
            .checked_add(additional_regions)
            .ok_or_else(|| {
                GuestMemoryError::resource_limit(
                    stage,
                    "memory-region-count-overflow",
                    "required memory region count overflowed host accounting",
                )
            })?;
        if regions > self.limits.max_regions {
            return Err(GuestMemoryError::resource_limit(
                stage,
                "memory-region-limit-exceeded",
                format!(
                    "operation needs {additional_regions} additional regions; {} of {} are live",
                    self.regions.len(),
                    self.limits.max_regions
                ),
            ));
        }
        if additional_physical_pages > self.frames.available() {
            return Err(GuestMemoryError::resource_limit(
                stage,
                "physical-page-quota-exceeded",
                format!(
                    "operation needs {additional_physical_pages} physical pages; {} remain under quota",
                    self.frames.available()
                ),
            ));
        }
        Ok(())
    }

    pub(crate) fn preflight_fixed_reservation(
        &self,
        start: GuestAddress,
        byte_length: u64,
        stage: &'static str,
    ) -> Result<GuestRange, GuestMemoryError> {
        let range = GuestRange::new(start, byte_length)?;
        require_page_aligned(range, "reserve-range-not-page-aligned")?;
        if self
            .regions
            .values()
            .any(|region| ranges_overlap(region.range, range))
        {
            return Err(GuestMemoryError::conflict(
                stage,
                "memory-region-overlap",
                format!(
                    "requested range [{:#010x}, {:#010x}) overlaps an existing region",
                    range.start().0,
                    range.end()
                ),
            ));
        }
        Ok(range)
    }

    pub fn create_owner(
        &mut self,
        kind: MemoryOwnerKind,
    ) -> Result<MemoryOwnerId, GuestMemoryError> {
        if self.owners.len() >= self.limits.max_owners {
            return Err(GuestMemoryError::resource_limit(
                "memory-owner",
                "memory-owner-limit-exceeded",
                format!("memory owner limit {} was reached", self.limits.max_owners),
            ));
        }
        let owner = MemoryOwnerId(self.next_owner);
        self.next_owner = self.next_owner.checked_add(1).ok_or_else(|| {
            GuestMemoryError::resource_limit(
                "memory-owner",
                "memory-owner-id-exhausted",
                "memory owner identifier space was exhausted",
            )
        })?;
        self.owners.insert(
            owner,
            OwnerState {
                kind,
                sealed: false,
            },
        );
        Ok(owner)
    }

    pub fn reserve_fixed(
        &mut self,
        owner: MemoryOwnerId,
        kind: MemoryRegionKind,
        start: GuestAddress,
        byte_length: u64,
    ) -> Result<GuestRange, GuestMemoryError> {
        self.require_owner(owner)?;
        if self.regions.len() >= self.limits.max_regions {
            return Err(GuestMemoryError::resource_limit(
                "region-table",
                "memory-region-limit-exceeded",
                format!(
                    "memory region limit {} was reached",
                    self.limits.max_regions
                ),
            ));
        }
        let range = GuestRange::new(start, byte_length)?;
        require_page_aligned(range, "reserve-range-not-page-aligned")?;
        if self
            .regions
            .values()
            .any(|region| ranges_overlap(region.range, range))
        {
            return Err(GuestMemoryError::conflict(
                "region-table",
                "memory-region-overlap",
                "reserved guest range overlaps an existing region",
            ));
        }
        self.ensure_generation_available()?;
        self.regions
            .insert(start.0, MemoryRegion { range, owner, kind });
        self.record_change(
            vec![MappingOperation::Reserve { owner, kind, range }],
            CacheEffects::default(),
        )?;
        Ok(range)
    }

    pub fn destroy_owner(
        &mut self,
        owner: MemoryOwnerId,
    ) -> Result<MemoryOwnerKind, GuestMemoryError> {
        self.require_owner(owner)?;
        if self.regions.values().any(|region| region.owner == owner)
            || self.pages.values().any(|page| page.owner == owner)
        {
            return Err(GuestMemoryError::conflict(
                "memory-owner",
                "memory-owner-still-has-mappings",
                "memory owner cannot be destroyed while regions or pages remain live",
            ));
        }
        self.owners
            .remove(&owner)
            .map(|state| state.kind)
            .ok_or_else(|| {
                GuestMemoryError::runtime_defect(
                    "memory-owner",
                    "preflighted-owner-missing",
                    "owner liveness preflight succeeded but owner disappeared",
                )
            })
    }

    pub fn commit(
        &mut self,
        owner: MemoryOwnerId,
        range: GuestRange,
        protection: PageProtection,
    ) -> Result<MemoryInvalidation, GuestMemoryError> {
        self.require_owner(owner)?;
        require_page_aligned(range, "commit-range-not-page-aligned")?;
        self.require_region_contains(owner, range)?;
        let pages = virtual_pages(range)?;
        if pages.clone().any(|page| self.pages.contains_key(&page)) {
            return Err(GuestMemoryError::conflict(
                "page-table",
                "page-already-committed",
                "commit range contains an already committed page",
            ));
        }
        let page_count = range.page_count();
        if page_count > self.frames.available() {
            return Err(GuestMemoryError::resource_limit(
                "physical-allocator",
                "physical-page-quota-exceeded",
                format!(
                    "commit requires {page_count} pages; {} remain under quota",
                    self.frames.available()
                ),
            ));
        }
        let generation = self.next_generation()?;

        let mut allocated = Vec::with_capacity(page_count);
        for virtual_page in pages {
            match self.frames.allocate_zeroed() {
                Ok(frame) => allocated.push((virtual_page, frame)),
                Err(error) => {
                    for (_, frame) in allocated {
                        self.frames.release(frame)?;
                    }
                    return Err(error);
                }
            }
        }
        for (virtual_page, frame) in allocated {
            self.pages.insert(
                virtual_page,
                PageState {
                    owner,
                    frame,
                    protection,
                    dirty_generation: Some(generation),
                },
            );
        }
        self.record_change(
            vec![MappingOperation::Commit {
                owner,
                range,
                protection,
            }],
            CacheEffects {
                flush_tlb: true,
                invalidate_instruction_fetch: true,
                invalidate_generated_code: true,
            },
        )
    }

    pub fn protect(
        &mut self,
        owner: MemoryOwnerId,
        range: GuestRange,
        protection: PageProtection,
    ) -> Result<MemoryInvalidation, GuestMemoryError> {
        self.require_owner(owner)?;
        require_page_aligned(range, "protect-range-not-page-aligned")?;
        self.require_region_contains(owner, range)?;
        let pages: Vec<u32> = virtual_pages(range)?.collect();
        self.require_committed_owner(owner, &pages)?;
        self.ensure_generation_available()?;
        for virtual_page in pages {
            let page = self.pages.get_mut(&virtual_page).ok_or_else(|| {
                GuestMemoryError::runtime_defect(
                    "page-table",
                    "preflighted-page-missing",
                    "protect preflight succeeded but page disappeared",
                )
            })?;
            page.protection = protection;
        }
        self.record_change(
            vec![MappingOperation::Protect {
                owner,
                range,
                protection,
            }],
            CacheEffects {
                flush_tlb: true,
                invalidate_instruction_fetch: true,
                invalidate_generated_code: true,
            },
        )
    }

    pub fn decommit(
        &mut self,
        owner: MemoryOwnerId,
        range: GuestRange,
    ) -> Result<MemoryInvalidation, GuestMemoryError> {
        self.require_owner(owner)?;
        require_page_aligned(range, "decommit-range-not-page-aligned")?;
        self.require_region_contains(owner, range)?;
        let pages: Vec<u32> = virtual_pages(range)?.collect();
        self.require_committed_owner(owner, &pages)?;
        self.ensure_generation_available()?;
        for virtual_page in pages {
            let page = self.pages.remove(&virtual_page).ok_or_else(|| {
                GuestMemoryError::runtime_defect(
                    "page-table",
                    "preflighted-page-missing",
                    "decommit preflight succeeded but page disappeared",
                )
            })?;
            self.frames.release(page.frame)?;
        }
        self.record_change(
            vec![MappingOperation::Decommit { owner, range }],
            CacheEffects {
                flush_tlb: true,
                invalidate_instruction_fetch: true,
                invalidate_generated_code: true,
            },
        )
    }

    pub fn release_region(
        &mut self,
        owner: MemoryOwnerId,
        start: GuestAddress,
    ) -> Result<Option<MemoryInvalidation>, GuestMemoryError> {
        self.require_owner(owner)?;
        let region = self.regions.get(&start.0).ok_or_else(|| {
            GuestMemoryError::invalid_request(
                "region-table",
                "memory-region-not-found",
                "release requires the exact base of an existing region",
            )
        })?;
        if region.owner != owner {
            return Err(GuestMemoryError::conflict(
                "region-table",
                "memory-region-owner-mismatch",
                "memory region belongs to a different owner",
            ));
        }
        let range = region.range;
        let pages: Vec<u32> = virtual_pages(range)?
            .filter(|page| self.pages.contains_key(page))
            .collect();
        self.require_committed_owner(owner, &pages)?;
        self.ensure_generation_available()?;
        for virtual_page in &pages {
            let page = self.pages.remove(virtual_page).ok_or_else(|| {
                GuestMemoryError::runtime_defect(
                    "page-table",
                    "preflighted-page-missing",
                    "release preflight succeeded but page disappeared",
                )
            })?;
            self.frames.release(page.frame)?;
        }
        self.regions.remove(&start.0);
        self.record_change(
            vec![MappingOperation::Release { owner, range }],
            CacheEffects {
                flush_tlb: !pages.is_empty(),
                invalidate_instruction_fetch: !pages.is_empty(),
                invalidate_generated_code: !pages.is_empty(),
            },
        )
        .map(Some)
    }

    pub fn prepare_image_layout(
        &mut self,
        source: &ModuleIdentity,
        plan: &Pe32LoadPlan,
    ) -> Result<ImageMemoryLayout, GuestMemoryError> {
        self.prepare_image_layout_at(source, plan, GuestAddress(plan.image.preferred_image_base))
    }

    pub fn prepare_image_layout_at(
        &mut self,
        source: &ModuleIdentity,
        plan: &Pe32LoadPlan,
        load_base: GuestAddress,
    ) -> Result<ImageMemoryLayout, GuestMemoryError> {
        let mut transaction = self.begin_transaction();
        let layout = transaction.prepare_image_layout_at_transaction(source, plan, load_base)?;
        transaction.commit_transaction();
        Ok(layout)
    }

    fn prepare_image_layout_at_transaction(
        &mut self,
        source: &ModuleIdentity,
        plan: &Pe32LoadPlan,
        load_base: GuestAddress,
    ) -> Result<ImageMemoryLayout, GuestMemoryError> {
        if !load_base.0.is_multiple_of(0x1_0000) {
            return Err(GuestMemoryError::invalid_request(
                "image-layout",
                "image-load-base-not-allocation-aligned",
                "image load base must be aligned to the 64 KiB Windows allocation granularity",
            ));
        }
        let preferred_end = u64::from(plan.image.preferred_image_base)
            .checked_add(u64::from(plan.image.size_of_image))
            .ok_or_else(|| {
                GuestMemoryError::invalid_request(
                    "image-layout",
                    "preferred-image-end-overflow",
                    "preferred image base plus SizeOfImage overflowed 64-bit accounting",
                )
            })?;
        if preferred_end != plan.image.image_end {
            return Err(GuestMemoryError::invalid_request(
                "image-layout",
                "image-end-mismatch",
                "preferred image base plus SizeOfImage does not match the load plan image end",
            ));
        }
        let image_range = GuestRange::new(load_base, u64::from(plan.image.size_of_image))?;
        require_page_aligned(image_range, "image-range-not-page-aligned")?;

        let mut page_specs = BTreeMap::new();
        insert_plan_mapping(
            &mut page_specs,
            image_range,
            load_base.0,
            plan.header_mapping.destination_rva.0,
            plan.header_mapping.mapped_size,
            plan.header_mapping.final_protection.into(),
            "header",
        )?;
        for section in &plan.sections {
            insert_plan_mapping(
                &mut page_specs,
                image_range,
                load_base.0,
                section.destination_rva.0,
                section.mapped_size,
                section.final_protection.into(),
                &section.name,
            )?;
        }
        if page_specs.len() > self.frames.available() {
            return Err(GuestMemoryError::resource_limit(
                "image-layout",
                "image-physical-page-quota-exceeded",
                format!(
                    "image needs {} committed pages; {} remain under quota",
                    page_specs.len(),
                    self.frames.available()
                ),
            ));
        }

        self.ensure_generation_available()?;

        let owner = self.create_owner(MemoryOwnerKind::Image {
            source_sha256: source.sha256.clone(),
        })?;
        if let Err(error) = self.reserve_fixed(
            owner,
            MemoryRegionKind::Image,
            image_range.start,
            image_range.byte_length,
        ) {
            self.owners.remove(&owner);
            return Err(error);
        }

        let mut allocated = Vec::with_capacity(page_specs.len());
        for (virtual_page, protection) in page_specs {
            match self.frames.allocate_zeroed() {
                Ok(frame) => allocated.push((virtual_page, protection, frame)),
                Err(error) => {
                    for (_, _, frame) in allocated {
                        self.frames.release(frame)?;
                    }
                    self.regions.remove(&image_range.start.0);
                    self.owners.remove(&owner);
                    return Err(error);
                }
            }
        }

        let committed_pages = allocated.len();
        let generation = self.next_generation()?;
        let mut mapping_operations = Vec::with_capacity(committed_pages);
        let mut readable_pages = 0;
        let mut writable_pages = 0;
        let mut executable_pages = 0;
        for (virtual_page, protection, frame) in allocated {
            readable_pages += usize::from(protection.read);
            writable_pages += usize::from(protection.write);
            executable_pages += usize::from(protection.execute);
            self.pages.insert(
                virtual_page,
                PageState {
                    owner,
                    frame,
                    protection,
                    dirty_generation: Some(generation),
                },
            );
            mapping_operations.push(MappingOperation::Commit {
                owner,
                range: GuestRange::new(
                    GuestAddress(
                        virtual_page
                            .checked_mul(GUEST_PAGE_SIZE as u32)
                            .ok_or_else(|| {
                                GuestMemoryError::runtime_defect(
                                    "image-layout",
                                    "virtual-page-address-overflow",
                                    "planned image virtual page does not fit a guest address",
                                )
                            })?,
                    ),
                    GUEST_PAGE_SIZE,
                )?,
                protection,
            });
        }
        let invalidation = self.record_change(
            mapping_operations,
            CacheEffects {
                flush_tlb: true,
                invalidate_instruction_fetch: true,
                invalidate_generated_code: true,
            },
        )?;
        Ok(ImageMemoryLayout {
            source: source.clone(),
            owner,
            image_range,
            reserved_pages: image_range.page_count(),
            committed_pages,
            physical_pages: committed_pages,
            readable_pages,
            writable_pages,
            executable_pages,
            owner_sealed: false,
            invalidation,
        })
    }

    pub fn read_initialization(
        &self,
        owner: MemoryOwnerId,
        address: GuestAddress,
        byte_length: usize,
    ) -> Result<Vec<u8>, GuestMemoryError> {
        let owner_state = self.require_owner(owner)?;
        if owner_state.sealed {
            return Err(GuestMemoryError::conflict(
                "image-initialization",
                "memory-owner-already-sealed",
                "owner initialization reads are forbidden after sealing",
            ));
        }
        let segments = self.preflight_owner_access(owner, address, byte_length)?;
        self.read_segments(&segments, byte_length)
    }

    pub fn write_initialization(
        &mut self,
        owner: MemoryOwnerId,
        address: GuestAddress,
        bytes: &[u8],
    ) -> Result<GuestWriteResult, GuestMemoryError> {
        let owner_state = self.require_owner(owner)?;
        if owner_state.sealed {
            return Err(GuestMemoryError::conflict(
                "image-initialization",
                "memory-owner-already-sealed",
                "owner initialization writes are forbidden after sealing",
            ));
        }
        let segments = self.preflight_owner_access(owner, address, bytes.len())?;
        let touches_executable = self.segments_touch_executable(&segments);
        let invalidation = if self.initial_reflection_acknowledged {
            let generation = self.next_generation()?;
            self.write_segments(&segments, bytes, generation)?;
            Some(self.record_change(
                Vec::new(),
                CacheEffects {
                    flush_tlb: false,
                    invalidate_instruction_fetch: touches_executable,
                    invalidate_generated_code: touches_executable,
                },
            )?)
        } else {
            self.write_segments(&segments, bytes, self.generation)?;
            None
        };
        let dirty_pages = unique_segment_pages(&segments);
        Ok(GuestWriteResult {
            bytes_written: bytes.len(),
            dirty_pages,
            invalidation,
        })
    }

    pub fn zero_initialization(
        &mut self,
        owner: MemoryOwnerId,
        address: GuestAddress,
        byte_length: usize,
    ) -> Result<GuestWriteResult, GuestMemoryError> {
        let owner_state = self.require_owner(owner)?;
        if owner_state.sealed {
            return Err(GuestMemoryError::conflict(
                "image-initialization",
                "memory-owner-already-sealed",
                "owner initialization zero-fill is forbidden after sealing",
            ));
        }
        let segments = self.preflight_owner_access(owner, address, byte_length)?;
        let touches_executable = self.segments_touch_executable(&segments);
        let generation = if self.initial_reflection_acknowledged {
            self.next_generation()?
        } else {
            self.generation
        };
        for segment in &segments {
            let page = self.pages.get_mut(&segment.virtual_page).ok_or_else(|| {
                GuestMemoryError::runtime_defect(
                    "page-table",
                    "preflighted-page-missing",
                    "zero-fill preflight succeeded but page disappeared",
                )
            })?;
            let bytes = self.frames.bytes_mut(page.frame)?;
            let end = segment
                .page_offset
                .checked_add(segment.byte_length)
                .ok_or_else(segment_bounds_error)?;
            let destination = bytes
                .get_mut(segment.page_offset..end)
                .ok_or_else(segment_bounds_error)?;
            destination.fill(0);
            page.dirty_generation = Some(generation);
        }
        let invalidation = if self.initial_reflection_acknowledged {
            Some(self.record_change(
                Vec::new(),
                CacheEffects {
                    flush_tlb: false,
                    invalidate_instruction_fetch: touches_executable,
                    invalidate_generated_code: touches_executable,
                },
            )?)
        } else {
            None
        };
        Ok(GuestWriteResult {
            bytes_written: byte_length,
            dirty_pages: unique_segment_pages(&segments),
            invalidation,
        })
    }

    pub fn seal_owner(
        &mut self,
        owner: MemoryOwnerId,
    ) -> Result<MemoryInvalidation, GuestMemoryError> {
        if self.require_owner(owner)?.sealed {
            return Err(GuestMemoryError::conflict(
                "memory-owner",
                "memory-owner-already-sealed",
                "memory owner was sealed more than once",
            ));
        }
        self.ensure_generation_available()?;
        let has_executable_pages = self
            .pages
            .values()
            .any(|page| page.owner == owner && page.protection.execute);
        let state = self.owners.get_mut(&owner).ok_or_else(|| {
            GuestMemoryError::runtime_defect(
                "memory-owner",
                "preflighted-owner-missing",
                "owner liveness preflight succeeded but owner disappeared",
            )
        })?;
        state.sealed = true;
        self.record_change(
            Vec::new(),
            CacheEffects {
                flush_tlb: false,
                invalidate_instruction_fetch: has_executable_pages,
                invalidate_generated_code: has_executable_pages,
            },
        )
    }

    pub fn acknowledge_invalidation(&mut self, generation: u64) -> Result<(), GuestMemoryError> {
        self.acknowledge_delta(generation)
    }

    pub fn acknowledge_delta(&mut self, generation: u64) -> Result<(), GuestMemoryError> {
        if generation > self.generation {
            return Err(GuestMemoryError::invalid_request(
                "memory-delta",
                "memory-delta-acknowledgement-future",
                format!(
                    "acknowledged generation {generation}, current generation is {}",
                    self.generation
                ),
            ));
        }
        if generation <= self.acknowledged_generation {
            let code = if generation == self.acknowledged_generation {
                "memory-delta-acknowledgement-duplicate"
            } else {
                "memory-delta-acknowledgement-stale"
            };
            return Err(GuestMemoryError::invalid_request(
                "memory-delta",
                code,
                format!(
                    "acknowledged generation {generation}, last acknowledged generation is {}",
                    self.acknowledged_generation
                ),
            ));
        }
        if !self.offered_generations.contains(&generation) {
            return Err(GuestMemoryError::invalid_request(
                "memory-delta",
                "memory-delta-generation-not-offered",
                "memory generation must be obtained from a reflection or delta before acknowledgement",
            ));
        }
        if self.pending_changes.is_empty() {
            return Err(GuestMemoryError::invalid_request(
                "memory-delta",
                "no-memory-delta-pending",
                "there is no pending memory delta to acknowledge",
            ));
        }

        for page in self.pages.values_mut() {
            if page
                .dirty_generation
                .is_some_and(|dirty_generation| dirty_generation <= generation)
            {
                page.dirty_generation = None;
            }
        }
        self.pending_changes
            .retain(|change| change.generation > generation);
        self.acknowledged_generation = generation;
        if self.offered_full_reflections.contains(&generation) {
            self.initial_reflection_acknowledged = true;
        }
        self.offered_generations
            .retain(|offered| *offered > generation);
        self.offered_full_reflections
            .retain(|offered| *offered > generation);
        Ok(())
    }

    pub fn pending_delta(&mut self) -> Result<MemoryDelta, GuestMemoryError> {
        if !self.initial_reflection_acknowledged {
            return Err(GuestMemoryError::invalid_request(
                "memory-delta",
                "initial-memory-reflection-required",
                "a full memory reflection must be acknowledged before incremental deltas",
            ));
        }
        let invalidation = self.pending_invalidation().ok_or_else(|| {
            GuestMemoryError::invalid_request(
                "memory-delta",
                "no-memory-delta-pending",
                "there is no pending memory delta to transfer",
            )
        })?;

        let mapping_operations = self
            .pending_changes
            .iter()
            .flat_map(|change| change.mapping_operations.iter().cloned())
            .collect::<Vec<_>>();
        let dirty_pages = self.collect_dirty_pages()?;
        let transferred_bytes = dirty_pages.iter().try_fold(0usize, |total, page| {
            total.checked_add(page.bytes.len()).ok_or_else(|| {
                GuestMemoryError::runtime_defect(
                    "memory-delta",
                    "memory-delta-byte-count-overflow",
                    "dirty page transfer size overflowed host accounting",
                )
            })
        })?;
        let content_sha256 = hash_delta_content(&mapping_operations, &dirty_pages);
        self.offered_generations.insert(invalidation.generation);
        Ok(MemoryDelta {
            from_generation: self.acknowledged_generation,
            generation: invalidation.generation,
            page_size: GUEST_PAGE_SIZE,
            mapping_operations,
            dirty_pages,
            cache_effects: invalidation.into(),
            transferred_bytes,
            content_sha256,
        })
    }

    fn collect_dirty_pages(&self) -> Result<Vec<DirtyPage>, GuestMemoryError> {
        let mut dirty_pages = Vec::new();
        for (&virtual_page, page) in &self.pages {
            if page.dirty_generation.is_none() {
                continue;
            }
            let address = virtual_page_address(virtual_page, "memory-delta")?;
            let owner = self.require_owner(page.owner)?;
            dirty_pages.push(DirtyPage {
                address,
                owner: page.owner,
                owner_sealed: owner.sealed,
                protection: page.protection,
                bytes: self.frames.bytes(page.frame)?.to_vec(),
            });
        }
        Ok(dirty_pages)
    }

    pub fn read(
        &self,
        address: GuestAddress,
        byte_length: usize,
    ) -> Result<Vec<u8>, GuestMemoryError> {
        let segments = self.preflight_guest_access(address, byte_length, MemoryAccess::Read)?;
        self.read_segments(&segments, byte_length)
    }

    pub fn fetch(
        &self,
        address: GuestAddress,
        byte_length: usize,
    ) -> Result<Vec<u8>, GuestMemoryError> {
        let segments = self.preflight_guest_access(address, byte_length, MemoryAccess::Execute)?;
        self.read_segments(&segments, byte_length)
    }

    pub fn write(
        &mut self,
        address: GuestAddress,
        bytes: &[u8],
    ) -> Result<GuestWriteResult, GuestMemoryError> {
        let segments = self.preflight_guest_access_without_synchronization(
            address,
            bytes.len(),
            MemoryAccess::Write,
        )?;
        let touches_executable = self.segments_touch_executable(&segments);
        let generation = self.next_generation()?;
        self.write_segments(&segments, bytes, generation)?;
        let invalidation = Some(self.record_change(
            Vec::new(),
            CacheEffects {
                flush_tlb: false,
                invalidate_instruction_fetch: touches_executable,
                invalidate_generated_code: touches_executable,
            },
        )?);
        Ok(GuestWriteResult {
            bytes_written: bytes.len(),
            dirty_pages: unique_segment_pages(&segments),
            invalidation,
        })
    }

    pub fn reconcile_reflected_page(
        &mut self,
        acknowledged_generation: u64,
        owner: MemoryOwnerId,
        address: GuestAddress,
        bytes: &[u8],
    ) -> Result<GuestPageReconciliation, GuestMemoryError> {
        self.ensure_synchronized()?;
        if !self.initial_reflection_acknowledged {
            return Err(GuestMemoryError::invalid_request(
                "memory-reconciliation",
                "initial-memory-reflection-required",
                "a full memory reflection must be acknowledged before CPU page reconciliation",
            ));
        }
        if acknowledged_generation != self.acknowledged_generation {
            return Err(GuestMemoryError::invalid_request(
                "memory-reconciliation",
                "reconciliation-generation-mismatch",
                "CPU page reconciliation does not match the acknowledged memory generation",
            ));
        }
        if !address.0.is_multiple_of(GUEST_PAGE_SIZE as u32) {
            return Err(GuestMemoryError::invalid_request(
                "memory-reconciliation",
                "reconciliation-address-not-page-aligned",
                "CPU page reconciliation address must be guest-page aligned",
            ));
        }
        if bytes.len() != GUEST_PAGE_SIZE_USIZE {
            return Err(GuestMemoryError::invalid_request(
                "memory-reconciliation",
                "reconciliation-page-size-mismatch",
                "CPU page reconciliation must provide exactly one guest page",
            ));
        }

        let segments = self.preflight_guest_access_without_synchronization(
            address,
            bytes.len(),
            MemoryAccess::Write,
        )?;
        let [segment] = segments.as_slice() else {
            return Err(GuestMemoryError::runtime_defect(
                "memory-reconciliation",
                "reconciliation-segment-count-invalid",
                "one aligned guest page did not produce exactly one access segment",
            ));
        };
        let page = self.pages.get(&segment.virtual_page).ok_or_else(|| {
            GuestMemoryError::runtime_defect(
                "memory-reconciliation",
                "preflighted-page-missing",
                "CPU page reconciliation preflight succeeded but the page disappeared",
            )
        })?;
        if page.owner != owner {
            return Err(GuestMemoryError::conflict(
                "memory-reconciliation",
                "reconciliation-page-owner-mismatch",
                "CPU page reconciliation names a different owner than the mapped page",
            ));
        }
        if page.dirty_generation.is_some() {
            return Err(GuestMemoryError::runtime_defect(
                "memory-reconciliation",
                "reconciliation-page-already-dirty",
                "a synchronized page retained dirty state before CPU reconciliation",
            ));
        }
        let frame = page.frame;
        let changed_bytes = self
            .frames
            .bytes(frame)?
            .iter()
            .zip(bytes)
            .filter(|(current, incoming)| current != incoming)
            .count();
        self.frames.bytes_mut(frame)?.copy_from_slice(bytes);
        let mut hasher = Sha256::new();
        hasher.update(bytes);

        Ok(GuestPageReconciliation {
            generation: acknowledged_generation,
            address,
            owner,
            bytes_reconciled: bytes.len(),
            changed_bytes,
            content_sha256: hex_digest(&hasher.finalize()),
        })
    }

    pub fn stats(&self) -> MemoryStats {
        let reserved_pages = self
            .regions
            .values()
            .map(|region| region.range.page_count())
            .sum();
        MemoryStats {
            owners: self.owners.len(),
            image_owners: self
                .owners
                .values()
                .filter(|owner| matches!(owner.kind, MemoryOwnerKind::Image { .. }))
                .count(),
            sealed_owners: self.owners.values().filter(|owner| owner.sealed).count(),
            regions: self.regions.len(),
            image_regions: self
                .regions
                .values()
                .filter(|region| region.kind == MemoryRegionKind::Image)
                .count(),
            reserved_pages,
            committed_pages: self.pages.len(),
            physical_pages: self.frames.in_use,
            dirty_pages: self
                .pages
                .values()
                .filter(|page| page.dirty_generation.is_some())
                .count(),
            generation: self.generation,
            acknowledged_generation: self.acknowledged_generation,
            pending_invalidation: self.pending_invalidation(),
        }
    }

    pub fn pending_reflection(&mut self) -> Result<MemoryReflection, GuestMemoryError> {
        if self.initial_reflection_acknowledged {
            return Err(GuestMemoryError::conflict(
                "memory-reflection",
                "full-memory-reflection-already-acknowledged",
                "incremental memory deltas must be used after the initial full reflection",
            ));
        }
        let invalidation = self.pending_invalidation().ok_or_else(|| {
            GuestMemoryError::invalid_request(
                "memory-reflection",
                "no-memory-delta-pending",
                "memory reflection requires a pending memory generation",
            )
        })?;
        let mut pages = Vec::with_capacity(self.pages.len());
        let mut hasher = Sha256::new();
        for (&virtual_page, page) in &self.pages {
            let address = virtual_page_address(virtual_page, "memory-reflection")?;
            let owner = self.require_owner(page.owner)?;
            if page.protection.execute && !owner.sealed {
                return Err(GuestMemoryError::conflict(
                    "memory-reflection",
                    "executable-owner-unsealed",
                    "executable pages cannot be reflected before their owner is sealed",
                ));
            }
            let bytes = self.frames.bytes(page.frame)?.to_vec();
            let protection_bits = u8::from(page.protection.read)
                | u8::from(page.protection.write) << 1
                | u8::from(page.protection.execute) << 2;
            hasher.update(address.0.to_le_bytes());
            hasher.update(page.owner.0.to_le_bytes());
            hasher.update([
                protection_bits,
                u8::from(owner.sealed),
                u8::from(page.dirty_generation.is_some()),
            ]);
            hasher.update(&bytes);
            pages.push(MemoryReflectionPage {
                address,
                owner: page.owner,
                owner_sealed: owner.sealed,
                protection: page.protection,
                dirty: page.dirty_generation.is_some(),
                bytes,
            });
        }
        self.offered_generations.insert(invalidation.generation);
        self.offered_full_reflections
            .insert(invalidation.generation);
        Ok(MemoryReflection {
            invalidation,
            page_size: GUEST_PAGE_SIZE,
            pages,
            content_sha256: hex_digest(&hasher.finalize()),
        })
    }

    fn require_owner(&self, owner: MemoryOwnerId) -> Result<&OwnerState, GuestMemoryError> {
        self.owners.get(&owner).ok_or_else(|| {
            GuestMemoryError::invalid_request(
                "memory-owner",
                "memory-owner-not-found",
                "memory owner identifier is not live",
            )
        })
    }

    fn require_region_contains(
        &self,
        owner: MemoryOwnerId,
        range: GuestRange,
    ) -> Result<(), GuestMemoryError> {
        let found = self
            .regions
            .values()
            .find(|region| region.owner == owner && contains_range(region.range, range));
        if found.is_none() {
            return Err(GuestMemoryError::invalid_request(
                "region-table",
                "range-not-reserved-by-owner",
                "guest range is not fully contained in a region reserved by this owner",
            ));
        }
        Ok(())
    }

    fn require_committed_owner(
        &self,
        owner: MemoryOwnerId,
        pages: &[u32],
    ) -> Result<(), GuestMemoryError> {
        for virtual_page in pages {
            let page = self.pages.get(virtual_page).ok_or_else(|| {
                GuestMemoryError::invalid_request(
                    "page-table",
                    "page-not-committed",
                    format!("virtual page {virtual_page:#x} is not committed"),
                )
            })?;
            if page.owner != owner {
                return Err(GuestMemoryError::conflict(
                    "page-table",
                    "committed-page-owner-mismatch",
                    "committed page belongs to a different owner",
                ));
            }
        }
        Ok(())
    }

    fn ensure_synchronized(&self) -> Result<(), GuestMemoryError> {
        if let Some(pending) = self.pending_invalidation() {
            return Err(GuestMemoryError::synchronization(
                "memory-invalidation-unacknowledged",
                "guest access is blocked until the pending memory delta is applied and acknowledged",
                pending,
            ));
        }
        Ok(())
    }

    fn preflight_guest_access(
        &self,
        address: GuestAddress,
        byte_length: usize,
        access: MemoryAccess,
    ) -> Result<Vec<AccessSegment>, GuestMemoryError> {
        self.ensure_synchronized()?;
        self.preflight_guest_access_without_synchronization(address, byte_length, access)
    }

    fn preflight_guest_access_without_synchronization(
        &self,
        address: GuestAddress,
        byte_length: usize,
        access: MemoryAccess,
    ) -> Result<Vec<AccessSegment>, GuestMemoryError> {
        let segments = access_segments(address, byte_length, self.limits.max_access_bytes)?;
        for segment in &segments {
            let Some(page) = self.pages.get(&segment.virtual_page) else {
                return Err(GuestMemoryError::fault(memory_fault(
                    segment.fault_address,
                    access,
                    MemoryFaultReason::Unmapped,
                )));
            };
            let denied = match access {
                MemoryAccess::Read if !page.protection.read => Some(MemoryFaultReason::ReadDenied),
                MemoryAccess::Write if !page.protection.write => {
                    Some(MemoryFaultReason::WriteDenied)
                }
                MemoryAccess::Execute if !page.protection.execute => {
                    Some(MemoryFaultReason::ExecuteDenied)
                }
                MemoryAccess::Execute
                    if !self
                        .owners
                        .get(&page.owner)
                        .is_some_and(|owner| owner.sealed) =>
                {
                    Some(MemoryFaultReason::OwnerUnsealed)
                }
                _ => None,
            };
            if let Some(reason) = denied {
                return Err(GuestMemoryError::fault(memory_fault(
                    segment.fault_address,
                    access,
                    reason,
                )));
            }
        }
        Ok(segments)
    }

    fn preflight_owner_access(
        &self,
        owner: MemoryOwnerId,
        address: GuestAddress,
        byte_length: usize,
    ) -> Result<Vec<AccessSegment>, GuestMemoryError> {
        let segments = access_segments(address, byte_length, self.limits.max_access_bytes)?;
        for segment in &segments {
            let page = self.pages.get(&segment.virtual_page).ok_or_else(|| {
                GuestMemoryError::invalid_request(
                    "image-initialization",
                    "initialization-page-not-committed",
                    format!(
                        "initialization address {:#010x} is not committed",
                        segment.fault_address.0
                    ),
                )
            })?;
            if page.owner != owner {
                return Err(GuestMemoryError::conflict(
                    "image-initialization",
                    "initialization-page-owner-mismatch",
                    "initialization range crosses into a different memory owner",
                ));
            }
        }
        Ok(segments)
    }

    fn read_segments(
        &self,
        segments: &[AccessSegment],
        byte_length: usize,
    ) -> Result<Vec<u8>, GuestMemoryError> {
        let mut output = Vec::with_capacity(byte_length);
        for segment in segments {
            let page = self.pages.get(&segment.virtual_page).ok_or_else(|| {
                GuestMemoryError::runtime_defect(
                    "page-table",
                    "preflighted-page-missing",
                    "access preflight succeeded but page disappeared",
                )
            })?;
            let bytes = self.frames.bytes(page.frame)?;
            let end = segment
                .page_offset
                .checked_add(segment.byte_length)
                .ok_or_else(segment_bounds_error)?;
            let source = bytes
                .get(segment.page_offset..end)
                .ok_or_else(segment_bounds_error)?;
            output.extend_from_slice(source);
        }
        Ok(output)
    }

    fn write_segments(
        &mut self,
        segments: &[AccessSegment],
        input: &[u8],
        dirty_generation: u64,
    ) -> Result<(), GuestMemoryError> {
        let mut input_offset = 0usize;
        for segment in segments {
            let page = self.pages.get_mut(&segment.virtual_page).ok_or_else(|| {
                GuestMemoryError::runtime_defect(
                    "page-table",
                    "preflighted-page-missing",
                    "write preflight succeeded but page disappeared",
                )
            })?;
            let bytes = self.frames.bytes_mut(page.frame)?;
            let page_end = segment
                .page_offset
                .checked_add(segment.byte_length)
                .ok_or_else(segment_bounds_error)?;
            let input_end = input_offset
                .checked_add(segment.byte_length)
                .ok_or_else(segment_bounds_error)?;
            let destination = bytes
                .get_mut(segment.page_offset..page_end)
                .ok_or_else(segment_bounds_error)?;
            let source = input
                .get(input_offset..input_end)
                .ok_or_else(segment_bounds_error)?;
            destination.copy_from_slice(source);
            page.dirty_generation = Some(dirty_generation);
            input_offset = input_end;
        }
        Ok(())
    }

    fn segments_touch_executable(&self, segments: &[AccessSegment]) -> bool {
        segments.iter().any(|segment| {
            self.pages
                .get(&segment.virtual_page)
                .is_some_and(|page| page.protection.execute)
        })
    }

    fn next_generation(&self) -> Result<u64, GuestMemoryError> {
        self.generation.checked_add(1).ok_or_else(|| {
            GuestMemoryError::runtime_defect(
                "memory-delta",
                "memory-generation-overflow",
                "memory generation counter overflowed",
            )
        })
    }

    fn ensure_generation_available(&self) -> Result<(), GuestMemoryError> {
        self.next_generation().map(|_| ())
    }

    fn record_change(
        &mut self,
        mapping_operations: Vec<MappingOperation>,
        cache_effects: CacheEffects,
    ) -> Result<MemoryInvalidation, GuestMemoryError> {
        self.generation = self.next_generation()?;
        self.pending_changes.push(MemoryChange {
            generation: self.generation,
            mapping_operations,
            cache_effects,
        });
        self.pending_invalidation().ok_or_else(|| {
            GuestMemoryError::runtime_defect(
                "memory-delta",
                "recorded-memory-delta-missing",
                "recorded memory change did not produce a pending delta",
            )
        })
    }

    fn pending_invalidation(&self) -> Option<MemoryInvalidation> {
        let latest = self.pending_changes.last()?;
        let effects = self
            .pending_changes
            .iter()
            .fold(CacheEffects::default(), |effects, change| {
                effects.merge(change.cache_effects)
            });
        let invalidation = MemoryInvalidation {
            generation: latest.generation,
            data_tlb: effects.flush_tlb,
            instruction_fetch: effects.invalidate_instruction_fetch,
            generated_code: effects.invalidate_generated_code,
        };
        Some(invalidation)
    }
}

fn virtual_page_address(
    virtual_page: u32,
    stage: &'static str,
) -> Result<GuestAddress, GuestMemoryError> {
    virtual_page
        .checked_mul(GUEST_PAGE_SIZE as u32)
        .map(GuestAddress)
        .ok_or_else(|| {
            GuestMemoryError::runtime_defect(
                stage,
                "virtual-page-address-overflow",
                "committed virtual page does not fit the 32-bit guest address space",
            )
        })
}

fn hash_delta_content(
    mapping_operations: &[MappingOperation],
    dirty_pages: &[DirtyPage],
) -> String {
    let mut hasher = Sha256::new();
    for operation in mapping_operations {
        match operation {
            MappingOperation::Reserve { owner, kind, range } => {
                hasher.update([0]);
                hash_mapping_fields(&mut hasher, *owner, *range);
                hasher.update([*kind as u8]);
            }
            MappingOperation::Commit {
                owner,
                range,
                protection,
            } => {
                hasher.update([1]);
                hash_mapping_fields(&mut hasher, *owner, *range);
                hash_protection(&mut hasher, *protection);
            }
            MappingOperation::Protect {
                owner,
                range,
                protection,
            } => {
                hasher.update([2]);
                hash_mapping_fields(&mut hasher, *owner, *range);
                hash_protection(&mut hasher, *protection);
            }
            MappingOperation::Decommit { owner, range } => {
                hasher.update([3]);
                hash_mapping_fields(&mut hasher, *owner, *range);
            }
            MappingOperation::Release { owner, range } => {
                hasher.update([4]);
                hash_mapping_fields(&mut hasher, *owner, *range);
            }
        }
    }
    for page in dirty_pages {
        hasher.update(page.address.0.to_le_bytes());
        hasher.update(page.owner.0.to_le_bytes());
        hasher.update([u8::from(page.owner_sealed)]);
        hash_protection(&mut hasher, page.protection);
        hasher.update(&page.bytes);
    }
    hex_digest(&hasher.finalize())
}

fn hash_mapping_fields(hasher: &mut Sha256, owner: MemoryOwnerId, range: GuestRange) {
    hasher.update(owner.0.to_le_bytes());
    hasher.update(range.start().0.to_le_bytes());
    hasher.update(range.byte_length().to_le_bytes());
}

fn hash_protection(hasher: &mut Sha256, protection: PageProtection) {
    hasher.update([
        u8::from(protection.read),
        u8::from(protection.write),
        u8::from(protection.execute),
    ]);
}

fn require_page_aligned(range: GuestRange, code: &'static str) -> Result<(), GuestMemoryError> {
    if !range.is_page_aligned() {
        return Err(GuestMemoryError::invalid_request(
            "guest-range",
            code,
            "guest range start and length must be 4 KiB aligned",
        ));
    }
    Ok(())
}

fn virtual_pages(range: GuestRange) -> Result<Range<u32>, GuestMemoryError> {
    require_page_aligned(range, "page-range-not-aligned")?;
    let start = u64::from(range.start.0) / GUEST_PAGE_SIZE;
    let end = range.end() / GUEST_PAGE_SIZE;
    let start = u32::try_from(start).map_err(|_| {
        GuestMemoryError::runtime_defect(
            "guest-range",
            "virtual-page-start-not-representable",
            "virtual page start does not fit the page index type",
        )
    })?;
    let end = u32::try_from(end).map_err(|_| {
        GuestMemoryError::runtime_defect(
            "guest-range",
            "virtual-page-end-not-representable",
            "virtual page end does not fit the page index type",
        )
    })?;
    Ok(start..end)
}

fn insert_plan_mapping(
    pages: &mut BTreeMap<u32, PageProtection>,
    image_range: GuestRange,
    image_base: u32,
    rva: u32,
    mapped_size: u32,
    protection: PageProtection,
    label: &str,
) -> Result<(), GuestMemoryError> {
    if mapped_size == 0 {
        return Ok(());
    }
    let address = image_base.checked_add(rva).ok_or_else(|| {
        GuestMemoryError::invalid_request(
            "image-layout",
            "image-mapping-address-overflow",
            format!("{label} guest address overflows 32 bits"),
        )
    })?;
    let range = GuestRange::new(GuestAddress(address), u64::from(mapped_size))?;
    require_page_aligned(range, "image-mapping-not-page-aligned")?;
    if !contains_range(image_range, range) {
        return Err(GuestMemoryError::invalid_request(
            "image-layout",
            "image-mapping-outside-reservation",
            format!("{label} mapping extends outside SizeOfImage"),
        ));
    }
    for virtual_page in virtual_pages(range)? {
        if pages.insert(virtual_page, protection).is_some() {
            return Err(GuestMemoryError::conflict(
                "image-layout",
                "image-page-mapping-overlap",
                format!("{label} overlaps another image page mapping"),
            ));
        }
    }
    Ok(())
}

fn access_segments(
    address: GuestAddress,
    byte_length: usize,
    max_access_bytes: usize,
) -> Result<Vec<AccessSegment>, GuestMemoryError> {
    if byte_length == 0 {
        return Err(GuestMemoryError::invalid_request(
            "guest-access",
            "zero-length-access",
            "guest access must contain at least one byte",
        ));
    }
    if byte_length > max_access_bytes {
        return Err(GuestMemoryError::resource_limit(
            "guest-access",
            "guest-access-limit-exceeded",
            format!("guest access requests {byte_length} bytes; limit is {max_access_bytes}"),
        ));
    }
    let byte_length_u64 = u64::try_from(byte_length).map_err(|_| {
        GuestMemoryError::resource_limit(
            "guest-access",
            "guest-access-length-not-representable",
            "guest access length does not fit 64-bit range accounting",
        )
    })?;
    let range = GuestRange::new(address, byte_length_u64)?;
    let mut current = u64::from(address.0);
    let mut segments = Vec::new();
    while current < range.end() {
        let virtual_page = u32::try_from(current / GUEST_PAGE_SIZE).map_err(|_| {
            GuestMemoryError::runtime_defect(
                "guest-access",
                "virtual-page-not-representable",
                "guest access page does not fit the page index type",
            )
        })?;
        let page_offset = usize::try_from(current % GUEST_PAGE_SIZE).map_err(|_| {
            GuestMemoryError::runtime_defect(
                "guest-access",
                "page-offset-not-representable",
                "guest access offset does not fit the host index type",
            )
        })?;
        let available = GUEST_PAGE_SIZE_USIZE - page_offset;
        let remaining = usize::try_from(range.end() - current).map_err(|_| {
            GuestMemoryError::resource_limit(
                "guest-access",
                "guest-access-remaining-not-representable",
                "guest access remainder does not fit the host index type",
            )
        })?;
        let segment_length = available.min(remaining);
        let fault_address = GuestAddress(u32::try_from(current).map_err(|_| {
            GuestMemoryError::runtime_defect(
                "guest-access",
                "fault-address-not-representable",
                "guest access address does not fit 32 bits",
            )
        })?);
        segments.push(AccessSegment {
            virtual_page,
            page_offset,
            byte_length: segment_length,
            fault_address,
        });
        current += u64::try_from(segment_length).map_err(|_| {
            GuestMemoryError::runtime_defect(
                "guest-access",
                "segment-length-not-representable",
                "guest access segment length does not fit 64 bits",
            )
        })?;
    }
    Ok(segments)
}

fn memory_fault(
    address: GuestAddress,
    access: MemoryAccess,
    reason: MemoryFaultReason,
) -> MemoryFault {
    let present = !matches!(reason, MemoryFaultReason::Unmapped);
    let mut error_code = 4;
    if present {
        error_code |= 1;
    }
    if access == MemoryAccess::Write {
        error_code |= 2;
    }
    if access == MemoryAccess::Execute {
        error_code |= 16;
    }
    MemoryFault {
        address,
        access,
        reason,
        page_fault_error_code: error_code,
    }
}

fn ranges_overlap(left: GuestRange, right: GuestRange) -> bool {
    let left = left.as_u64_range();
    let right = right.as_u64_range();
    left.start < right.end && right.start < left.end
}

fn contains_range(container: GuestRange, value: GuestRange) -> bool {
    let container = container.as_u64_range();
    let value = value.as_u64_range();
    value.start >= container.start && value.end <= container.end
}

fn unique_segment_pages(segments: &[AccessSegment]) -> usize {
    segments
        .iter()
        .map(|segment| segment.virtual_page)
        .collect::<std::collections::BTreeSet<_>>()
        .len()
}

fn segment_bounds_error() -> GuestMemoryError {
    GuestMemoryError::runtime_defect(
        "guest-access",
        "preflighted-segment-out-of-bounds",
        "access segment escaped its preflighted page or input bounds",
    )
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

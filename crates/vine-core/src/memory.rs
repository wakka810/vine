use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::ops::{Deref, DerefMut, Range};

use crate::{MappingProtection, ModuleIdentity, Pe32LoadPlan};

pub const GUEST_PAGE_SIZE: u64 = 4_096;
const GUEST_PAGE_SIZE_USIZE: usize = GUEST_PAGE_SIZE as usize;
const GUEST_ADDRESS_SPACE_SIZE: u64 = 1_u64 << 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct GuestAddress(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GuestRange {
    start: GuestAddress,
    byte_length: u64,
}

impl GuestRange {
    pub fn new(start: GuestAddress, byte_length: u64) -> Result<Self, GuestMemoryError> {
        if byte_length == 0 {
            return Err(GuestMemoryError::invalid_request(
                "guest range must contain at least one byte",
            ));
        }
        let end = u64::from(start.0).checked_add(byte_length).ok_or_else(|| {
            GuestMemoryError::invalid_request("guest range end overflowed 64-bit accounting")
        })?;
        if end > GUEST_ADDRESS_SPACE_SIZE {
            return Err(GuestMemoryError::invalid_request(
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct MemoryOwnerId(pub u32);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemoryOwnerKind {
    Image { source_sha256: String },
    Anonymous,
    Runtime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    Surface,
    GraphicsResource,
    AudioResource,
    GdiResource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryAccess {
    Read,
    Write,
    Execute,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryFaultReason {
    Unmapped,
    ReadDenied,
    WriteDenied,
    ExecuteDenied,
    OwnerUnsealed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryFault {
    pub address: GuestAddress,
    pub access: MemoryAccess,
    pub reason: MemoryFaultReason,
    pub page_fault_error_code: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryInvalidation {
    pub generation: u64,
    pub data_tlb: bool,
    pub instruction_fetch: bool,
    pub generated_code: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum MappingOperation {
    Reserve {
        address_allocation_id: Option<u64>,
        owner: MemoryOwnerId,
        kind: MemoryRegionKind,
        range: GuestRange,
    },
    Commit {
        address_allocation_id: Option<u64>,
        owner: MemoryOwnerId,
        range: GuestRange,
        protection: PageProtection,
    },
    Protect {
        address_allocation_id: Option<u64>,
        owner: MemoryOwnerId,
        range: GuestRange,
        protection: PageProtection,
    },
    Decommit {
        address_allocation_id: Option<u64>,
        owner: MemoryOwnerId,
        range: GuestRange,
    },
    Release {
        address_allocation_id: Option<u64>,
        owner: MemoryOwnerId,
        range: GuestRange,
    },
}

#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DirtyPage {
    pub address: GuestAddress,
    pub owner: MemoryOwnerId,
    pub owner_sealed: bool,
    pub protection: PageProtection,
    pub bytes: Vec<u8>,
}

#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MemoryDelta {
    pub from_generation: u64,
    pub generation: u64,
    pub page_size: u64,
    pub mapping_operations: Vec<MappingOperation>,
    pub dirty_pages: Vec<DirtyPage>,
    pub cache_effects: CacheEffects,
}

const MEMORY_DELTA_CONTROL_MAGIC: &[u8; 8] = b"VINEDELT";
const MEMORY_DELTA_CONTROL_HEADER_BYTES: usize = 52;
const MEMORY_DELTA_MAPPING_DESCRIPTOR_BYTES: usize = 40;
const MEMORY_DELTA_PAGE_DESCRIPTOR_BYTES: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryDeltaBuffers {
    pub control: Vec<u8>,
    pub page_bytes: Vec<u8>,
}

#[cfg(test)]
impl MemoryDelta {
    pub(crate) fn encode_binary(&self) -> Result<MemoryDeltaBuffers, GuestMemoryError> {
        if self.page_size != GUEST_PAGE_SIZE {
            return Err(GuestMemoryError::invalid_request(
                "binary memory delta requires 4 KiB pages",
            ));
        }
        let mut buffers = MemoryDeltaBuffers::new(
            self.from_generation,
            self.generation,
            &self.mapping_operations,
            self.cache_effects,
            self.dirty_pages.len(),
            0,
        )?;
        for page in &self.dirty_pages {
            buffers.push_page(
                page.address,
                page.owner,
                page_protection_flags(page.protection) | u32::from(page.owner_sealed) << 3,
                &page.bytes,
            )?;
        }
        Ok(buffers)
    }
}

impl MemoryDeltaBuffers {
    fn new(
        from_generation: u64,
        generation: u64,
        mapping_operations: &[MappingOperation],
        cache_effects: CacheEffects,
        page_count: usize,
        shared_guest_base: u32,
    ) -> Result<Self, GuestMemoryError> {
        let mapping_count = u32::try_from(mapping_operations.len()).map_err(|_| {
            GuestMemoryError::resource_limit(
                "memory delta mapping count does not fit the binary control format",
            )
        })?;
        let dirty_count = u32::try_from(page_count).map_err(|_| {
            GuestMemoryError::resource_limit(
                "memory delta page count does not fit the binary control format",
            )
        })?;
        let mapping_bytes = mapping_operations
            .len()
            .checked_mul(MEMORY_DELTA_MAPPING_DESCRIPTOR_BYTES)
            .ok_or_else(|| {
                GuestMemoryError::resource_limit(
                    "memory delta mapping size overflowed host accounting",
                )
            })?;
        let page_descriptor_bytes = page_count
            .checked_mul(MEMORY_DELTA_PAGE_DESCRIPTOR_BYTES)
            .ok_or_else(|| {
                GuestMemoryError::resource_limit(
                    "memory delta page descriptor size overflowed host accounting",
                )
            })?;
        let payload_bytes = page_count
            .checked_mul(GUEST_PAGE_SIZE_USIZE)
            .ok_or_else(|| {
                GuestMemoryError::resource_limit(
                    "memory delta payload size overflowed host accounting",
                )
            })?;
        let control_bytes = MEMORY_DELTA_CONTROL_HEADER_BYTES
            .checked_add(mapping_bytes)
            .and_then(|bytes| bytes.checked_add(page_descriptor_bytes))
            .ok_or_else(|| {
                GuestMemoryError::resource_limit(
                    "memory delta control size overflowed host accounting",
                )
            })?;
        let mut control = Vec::with_capacity(control_bytes);
        control.extend_from_slice(MEMORY_DELTA_CONTROL_MAGIC);
        control.extend_from_slice(&(GUEST_PAGE_SIZE as u32).to_le_bytes());
        control.extend_from_slice(&from_generation.to_le_bytes());
        control.extend_from_slice(&generation.to_le_bytes());
        control.extend_from_slice(&mapping_count.to_le_bytes());
        control.extend_from_slice(&(MEMORY_DELTA_MAPPING_DESCRIPTOR_BYTES as u32).to_le_bytes());
        control.extend_from_slice(&dirty_count.to_le_bytes());
        control.extend_from_slice(&(MEMORY_DELTA_PAGE_DESCRIPTOR_BYTES as u32).to_le_bytes());
        let cache_flags = u32::from(cache_effects.flush_tlb)
            | u32::from(cache_effects.invalidate_instruction_fetch) << 1
            | u32::from(cache_effects.invalidate_generated_code) << 2;
        control.extend_from_slice(&cache_flags.to_le_bytes());
        control.extend_from_slice(&shared_guest_base.to_le_bytes());

        for operation in mapping_operations {
            let (operation_code, address_allocation_id, owner, kind, range, protection) =
                match operation {
                    MappingOperation::Reserve {
                        address_allocation_id,
                        owner,
                        kind,
                        range,
                    } => (
                        1u32,
                        *address_allocation_id,
                        *owner,
                        Some(*kind),
                        *range,
                        None,
                    ),
                    MappingOperation::Commit {
                        address_allocation_id,
                        owner,
                        range,
                        protection,
                    } => (
                        2,
                        *address_allocation_id,
                        *owner,
                        None,
                        *range,
                        Some(*protection),
                    ),
                    MappingOperation::Protect {
                        address_allocation_id,
                        owner,
                        range,
                        protection,
                    } => (
                        3,
                        *address_allocation_id,
                        *owner,
                        None,
                        *range,
                        Some(*protection),
                    ),
                    MappingOperation::Decommit {
                        address_allocation_id,
                        owner,
                        range,
                    } => (4, *address_allocation_id, *owner, None, *range, None),
                    MappingOperation::Release {
                        address_allocation_id,
                        owner,
                        range,
                    } => (5, *address_allocation_id, *owner, None, *range, None),
                };
            let protection_flags = protection.map_or(0, page_protection_flags);
            control.extend_from_slice(&operation_code.to_le_bytes());
            control.extend_from_slice(&owner.0.to_le_bytes());
            control.extend_from_slice(&kind.map_or(0, memory_region_kind_code).to_le_bytes());
            control.extend_from_slice(&protection_flags.to_le_bytes());
            control.extend_from_slice(&range.start().0.to_le_bytes());
            control.extend_from_slice(&range.byte_length().to_le_bytes());
            control.extend_from_slice(&address_allocation_id.unwrap_or(0).to_le_bytes());
            control.extend_from_slice(&0u32.to_le_bytes());
        }

        Ok(Self {
            control,
            page_bytes: Vec::with_capacity(if shared_guest_base == 0 {
                payload_bytes
            } else {
                0
            }),
        })
    }

    fn push_page(
        &mut self,
        address: GuestAddress,
        owner: MemoryOwnerId,
        flags: u32,
        bytes: &[u8],
    ) -> Result<(), GuestMemoryError> {
        if bytes.len() != GUEST_PAGE_SIZE_USIZE {
            return Err(GuestMemoryError::runtime_defect(
                "memory delta contains a page with a non-4-KiB payload",
            ));
        }
        self.control.extend_from_slice(&address.0.to_le_bytes());
        self.control.extend_from_slice(&owner.0.to_le_bytes());
        self.control.extend_from_slice(&flags.to_le_bytes());
        self.control.extend_from_slice(&0u32.to_le_bytes());
        if self.control[48..52] == [0; 4] {
            self.page_bytes.extend_from_slice(bytes);
        }
        Ok(())
    }
}

const CPU_WRITEBACK_CONTROL_MAGIC: &[u8; 8] = b"VINEWBCK";
const CPU_WRITEBACK_CONTROL_HEADER_BYTES: usize = 28;
const CPU_WRITEBACK_DESCRIPTOR_BYTES: usize = 8;

#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CpuWritebackPage {
    pub address: GuestAddress,
    pub owner: MemoryOwnerId,
    pub bytes: Vec<u8>,
}

#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CpuWritebackBatch {
    pub base_generation: u64,
    pub pages: Vec<CpuWritebackPage>,
}

#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CpuWritebackBuffers {
    pub control: Vec<u8>,
    pub page_bytes: Vec<u8>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct CpuWritebackPageView<'a> {
    pub address: GuestAddress,
    pub owner: MemoryOwnerId,
    pub bytes: &'a [u8],
}

#[derive(Debug)]
struct CpuWritebackBatchView<'a> {
    base_generation: u64,
    pages: Vec<CpuWritebackPageView<'a>>,
}

fn cpu_writeback_pages_equal(
    left: &[u8; GUEST_PAGE_SIZE_USIZE],
    right: &[u8; GUEST_PAGE_SIZE_USIZE],
) -> bool {
    left.as_chunks::<8>()
        .0
        .iter()
        .zip(right.as_chunks::<8>().0)
        .all(|(left, right)| u64::from_ne_bytes(*left) == u64::from_ne_bytes(*right))
}

#[cfg(test)]
impl CpuWritebackBatch {
    pub fn encode_binary(&self) -> Result<CpuWritebackBuffers, GuestMemoryError> {
        let page_count = u32::try_from(self.pages.len()).map_err(|_| {
            GuestMemoryError::resource_limit(
                "CPU writeback page count does not fit the binary control format",
            )
        })?;
        let control_bytes = CPU_WRITEBACK_CONTROL_HEADER_BYTES
            .checked_add(
                self.pages
                    .len()
                    .checked_mul(CPU_WRITEBACK_DESCRIPTOR_BYTES)
                    .ok_or_else(|| {
                        GuestMemoryError::resource_limit(
                            "CPU writeback control size overflowed host accounting",
                        )
                    })?,
            )
            .ok_or_else(|| {
                GuestMemoryError::resource_limit(
                    "CPU writeback control size overflowed host accounting",
                )
            })?;
        let payload_bytes = self
            .pages
            .len()
            .checked_mul(GUEST_PAGE_SIZE_USIZE)
            .ok_or_else(|| {
                GuestMemoryError::resource_limit(
                    "CPU writeback page payload size overflowed host accounting",
                )
            })?;
        let mut control = Vec::with_capacity(control_bytes);
        control.extend_from_slice(CPU_WRITEBACK_CONTROL_MAGIC);
        control.extend_from_slice(&(GUEST_PAGE_SIZE as u32).to_le_bytes());
        control.extend_from_slice(&self.base_generation.to_le_bytes());
        control.extend_from_slice(&page_count.to_le_bytes());
        control.extend_from_slice(&(CPU_WRITEBACK_DESCRIPTOR_BYTES as u32).to_le_bytes());
        let mut page_bytes = Vec::with_capacity(payload_bytes);
        for page in &self.pages {
            if !page.address.0.is_multiple_of(GUEST_PAGE_SIZE as u32) {
                return Err(GuestMemoryError::invalid_request(
                    "CPU writeback page address must be guest-page aligned",
                ));
            }
            if page.bytes.len() != GUEST_PAGE_SIZE_USIZE {
                return Err(GuestMemoryError::invalid_request(
                    "CPU writeback page payload must contain exactly one guest page",
                ));
            }
            control.extend_from_slice(&page.address.0.to_le_bytes());
            control.extend_from_slice(&page.owner.0.to_le_bytes());
            page_bytes.extend_from_slice(&page.bytes);
        }
        Ok(CpuWritebackBuffers {
            control,
            page_bytes,
        })
    }
}

fn decode_cpu_writeback_view<'a>(
    control: &[u8],
    page_bytes: &'a [u8],
    max_pages: usize,
) -> Result<CpuWritebackBatchView<'a>, GuestMemoryError> {
    if control.len() < CPU_WRITEBACK_CONTROL_HEADER_BYTES {
        return Err(GuestMemoryError::invalid_request(
            "CPU writeback control header is truncated",
        ));
    }
    if control.get(..8) != Some(CPU_WRITEBACK_CONTROL_MAGIC.as_slice()) {
        return Err(GuestMemoryError::invalid_request(
            "CPU writeback control magic is invalid",
        ));
    }
    let page_size = read_binary_u32(control, 8)?;
    if page_size != GUEST_PAGE_SIZE as u32 {
        return Err(GuestMemoryError::invalid_request(
            "CPU writeback binary page size is not 4 KiB",
        ));
    }
    let base_generation = read_binary_u64(control, 12)?;
    let page_count = read_binary_u32(control, 20)? as usize;
    if page_count > max_pages {
        return Err(GuestMemoryError::resource_limit(format!(
            "CPU writeback contains {page_count} pages; limit is {max_pages}"
        )));
    }
    let descriptor_bytes = read_binary_u32(control, 24)? as usize;
    if descriptor_bytes != CPU_WRITEBACK_DESCRIPTOR_BYTES {
        return Err(GuestMemoryError::invalid_request(
            "CPU writeback descriptor size is unsupported",
        ));
    }
    let expected_control = CPU_WRITEBACK_CONTROL_HEADER_BYTES
        .checked_add(page_count.checked_mul(descriptor_bytes).ok_or_else(|| {
            GuestMemoryError::resource_limit(
                "CPU writeback descriptor size overflowed host accounting",
            )
        })?)
        .ok_or_else(|| {
            GuestMemoryError::resource_limit(
                "CPU writeback control size overflowed host accounting",
            )
        })?;
    if control.len() != expected_control {
        return Err(GuestMemoryError::invalid_request(
            "CPU writeback control length does not match its page count",
        ));
    }
    let expected_payload = page_count
        .checked_mul(GUEST_PAGE_SIZE_USIZE)
        .ok_or_else(|| {
            GuestMemoryError::resource_limit(
                "CPU writeback page payload size overflowed host accounting",
            )
        })?;
    if page_bytes.len() != expected_payload {
        return Err(GuestMemoryError::invalid_request(
            "CPU writeback page payload length does not match its page count",
        ));
    }

    let mut pages = Vec::with_capacity(page_count);
    for index in 0..page_count {
        let descriptor = CPU_WRITEBACK_CONTROL_HEADER_BYTES + index * descriptor_bytes;
        let address = GuestAddress(read_binary_u32(control, descriptor)?);
        let owner = MemoryOwnerId(read_binary_u32(control, descriptor + 4)?);
        let payload = index * GUEST_PAGE_SIZE_USIZE;
        pages.push(CpuWritebackPageView {
            address,
            owner,
            bytes: &page_bytes[payload..payload + GUEST_PAGE_SIZE_USIZE],
        });
    }
    Ok(CpuWritebackBatchView {
        base_generation,
        pages,
    })
}

#[derive(Debug)]
pub struct ImageMemoryLayout {
    pub owner: MemoryOwnerId,
    pub image_range: GuestRange,
    pub invalidation: MemoryInvalidation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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
    pub pending_invalidation: Option<MemoryInvalidation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemorySynchronizationState {
    pub generation: u64,
    pub acknowledged_generation: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryReflectionPage {
    pub address: GuestAddress,
    pub owner: MemoryOwnerId,
    pub owner_sealed: bool,
    pub protection: PageProtection,
    pub dirty: bool,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryRegionInfo {
    pub address_allocation_id: Option<u64>,
    pub owner: MemoryOwnerId,
    pub kind: MemoryRegionKind,
    pub range: GuestRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryReflection {
    pub invalidation: MemoryInvalidation,
    pub page_size: u64,
    pub pages: Vec<MemoryReflectionPage>,
}

const MEMORY_REFLECTION_CONTROL_MAGIC: &[u8; 8] = b"VINEREFL";
const MEMORY_REFLECTION_CONTROL_HEADER_BYTES: usize = 36;
const MEMORY_REFLECTION_DESCRIPTOR_BYTES: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryReflectionBuffers {
    pub control: Vec<u8>,
    pub page_bytes: Vec<u8>,
}

impl MemoryReflection {
    pub fn encode_binary(&self) -> Result<MemoryReflectionBuffers, GuestMemoryError> {
        if self.page_size != GUEST_PAGE_SIZE {
            return Err(GuestMemoryError::invalid_request(
                "binary memory reflection requires 4 KiB pages",
            ));
        }
        let page_count = u32::try_from(self.pages.len()).map_err(|_| {
            GuestMemoryError::resource_limit(
                "memory reflection page count does not fit the binary control format",
            )
        })?;
        let descriptor_bytes = self
            .pages
            .len()
            .checked_mul(MEMORY_REFLECTION_DESCRIPTOR_BYTES)
            .ok_or_else(|| {
                GuestMemoryError::resource_limit(
                    "memory reflection descriptor size overflowed host accounting",
                )
            })?;
        let payload_bytes = self
            .pages
            .len()
            .checked_mul(GUEST_PAGE_SIZE_USIZE)
            .ok_or_else(|| {
                GuestMemoryError::resource_limit(
                    "memory reflection payload size overflowed host accounting",
                )
            })?;
        let control_bytes = MEMORY_REFLECTION_CONTROL_HEADER_BYTES
            .checked_add(descriptor_bytes)
            .ok_or_else(|| {
                GuestMemoryError::resource_limit(
                    "memory reflection control size overflowed host accounting",
                )
            })?;
        let mut control = Vec::with_capacity(control_bytes);
        control.extend_from_slice(MEMORY_REFLECTION_CONTROL_MAGIC);
        control.extend_from_slice(&(GUEST_PAGE_SIZE as u32).to_le_bytes());
        control.extend_from_slice(&self.invalidation.generation.to_le_bytes());
        control.extend_from_slice(&page_count.to_le_bytes());
        control.extend_from_slice(&(MEMORY_REFLECTION_DESCRIPTOR_BYTES as u32).to_le_bytes());
        let cache_flags = u32::from(self.invalidation.data_tlb)
            | u32::from(self.invalidation.instruction_fetch) << 1
            | u32::from(self.invalidation.generated_code) << 2;
        control.extend_from_slice(&cache_flags.to_le_bytes());
        control.extend_from_slice(&0u32.to_le_bytes());
        let mut page_bytes = Vec::with_capacity(payload_bytes);
        for page in &self.pages {
            if page.bytes.len() != GUEST_PAGE_SIZE_USIZE {
                return Err(GuestMemoryError::runtime_defect(
                    "memory reflection contains a page with a non-4-KiB payload",
                ));
            }
            let flags = u32::from(page.protection.read)
                | u32::from(page.protection.write) << 1
                | u32::from(page.protection.execute) << 2
                | u32::from(page.owner_sealed) << 3
                | u32::from(page.dirty) << 4;
            control.extend_from_slice(&page.address.0.to_le_bytes());
            control.extend_from_slice(&page.owner.0.to_le_bytes());
            control.extend_from_slice(&flags.to_le_bytes());
            control.extend_from_slice(&0u32.to_le_bytes());
            page_bytes.extend_from_slice(&page.bytes);
        }
        Ok(MemoryReflectionBuffers {
            control,
            page_bytes,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GuestMemoryErrorClass {
    InvalidRequest,
    ResourceLimit,
    Conflict,
    SynchronizationRequired,
    Fault,
    RuntimeDefect,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuestMemoryError {
    pub(crate) class: GuestMemoryErrorClass,
    pub message: String,
    pub fault: Option<MemoryFault>,
    pub pending_invalidation: Option<MemoryInvalidation>,
}

impl GuestMemoryError {
    pub(crate) fn invalid_request(message: impl Into<String>) -> Self {
        Self::new(GuestMemoryErrorClass::InvalidRequest, message)
    }

    pub(crate) fn resource_limit(message: impl Into<String>) -> Self {
        Self::new(GuestMemoryErrorClass::ResourceLimit, message)
    }

    pub(crate) fn conflict(message: impl Into<String>) -> Self {
        Self::new(GuestMemoryErrorClass::Conflict, message)
    }

    fn synchronization(message: impl Into<String>, pending: MemoryInvalidation) -> Self {
        let mut error = Self::new(GuestMemoryErrorClass::SynchronizationRequired, message);
        error.pending_invalidation = Some(pending);
        error
    }

    fn fault(fault: MemoryFault) -> Self {
        let mut error = Self::new(
            GuestMemoryErrorClass::Fault,
            format!(
                "{:?} at {:#010x} failed: {:?}, PFEC {:#x}",
                fault.access, fault.address.0, fault.reason, fault.page_fault_error_code
            ),
        );
        error.fault = Some(fault);
        error
    }

    pub(crate) fn runtime_defect(message: impl Into<String>) -> Self {
        Self::new(GuestMemoryErrorClass::RuntimeDefect, message)
    }

    fn new(class: GuestMemoryErrorClass, message: impl Into<String>) -> Self {
        Self {
            class,
            message: message.into(),
            fault: None,
            pending_invalidation: None,
        }
    }
}

impl fmt::Display for GuestMemoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
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
    address_allocation_id: Option<u64>,
    range: GuestRange,
    owner: MemoryOwnerId,
    kind: MemoryRegionKind,
    initial_protection: Option<PageProtection>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FrameId(usize);

#[derive(Debug, Clone)]
struct PageState {
    owner: MemoryOwnerId,
    frame: FrameId,
    protection: PageProtection,
    dirty_generation: Option<u64>,
}

#[derive(Debug)]
struct PhysicalFrames {
    limit: usize,
    frames: Vec<Option<FrameBytes>>,
    free: BTreeSet<usize>,
    in_use: usize,
    #[cfg(feature = "shared-guest-memory")]
    arena: Option<vine_shared_memory::Arena>,
}

#[derive(Debug)]
enum FrameBytes {
    Owned(Box<[u8; GUEST_PAGE_SIZE_USIZE]>),
    #[cfg(feature = "shared-guest-memory")]
    Shared(vine_shared_memory::Page),
    #[cfg(feature = "shared-guest-memory")]
    Released(u32, Box<[u8; GUEST_PAGE_SIZE_USIZE]>),
}
impl Deref for FrameBytes {
    type Target = [u8; GUEST_PAGE_SIZE_USIZE];
    fn deref(&self) -> &Self::Target {
        match self {
            Self::Owned(bytes) => bytes,
            #[cfg(feature = "shared-guest-memory")]
            Self::Shared(page) => page,
            #[cfg(feature = "shared-guest-memory")]
            Self::Released(_, bytes) => bytes,
        }
    }
}
impl DerefMut for FrameBytes {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            Self::Owned(bytes) => bytes,
            #[cfg(feature = "shared-guest-memory")]
            Self::Shared(page) => page,
            #[cfg(feature = "shared-guest-memory")]
            Self::Released(_, bytes) => bytes,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FrameAllocationSource {
    Reused,
    Appended,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FrameAllocation {
    frame: FrameId,
    source: FrameAllocationSource,
}

#[derive(Debug)]
enum FrameUndo {
    Allocation(FrameAllocation),
    Release { frame: FrameId, bytes: FrameBytes },
    Bytes { frame: FrameId, image: usize },
}

#[derive(Debug)]
struct MemoryTransactionJournal {
    baseline_next_owner: u32,
    baseline_generation: u64,
    baseline_acknowledged_generation: u64,
    baseline_pending_changes: usize,
    owners: BTreeMap<MemoryOwnerId, Option<OwnerState>>,
    regions: BTreeMap<u32, Option<MemoryRegion>>,
    pages: BTreeMap<u32, Option<PageState>>,
    frame_bytes_recorded: BTreeSet<usize>,
    allocated_frames: BTreeSet<usize>,
    frame_undo: Vec<FrameUndo>,
    frame_images: Vec<[u8; GUEST_PAGE_SIZE_USIZE]>,
    offered_generations: BTreeSet<u64>,
    offered_full_reflections: BTreeSet<u64>,
}

impl MemoryTransactionJournal {
    fn new(memory: &GuestMemory) -> Self {
        Self {
            baseline_next_owner: memory.next_owner,
            baseline_generation: memory.generation,
            baseline_acknowledged_generation: memory.acknowledged_generation,
            baseline_pending_changes: memory.pending_changes.len(),
            owners: BTreeMap::new(),
            regions: BTreeMap::new(),
            pages: BTreeMap::new(),
            frame_bytes_recorded: BTreeSet::new(),
            allocated_frames: BTreeSet::new(),
            frame_undo: Vec::new(),
            frame_images: Vec::new(),
            offered_generations: BTreeSet::new(),
            offered_full_reflections: BTreeSet::new(),
        }
    }
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
            free: BTreeSet::new(),
            in_use: 0,
            #[cfg(feature = "shared-guest-memory")]
            arena: None,
        }
    }

    fn available(&self) -> usize {
        self.limit.saturating_sub(self.in_use)
    }

    fn allocate_zeroed(&mut self, _virtual_page: u32) -> Result<FrameAllocation, GuestMemoryError> {
        if self.in_use >= self.limit {
            return Err(GuestMemoryError::resource_limit(format!(
                "physical page quota {} was reached",
                self.limit
            )));
        }
        #[cfg(feature = "shared-guest-memory")]
        let bytes = match self.arena.as_mut() {
            Some(arena) => FrameBytes::Shared(
                arena
                    .allocate(_virtual_page * 4096)
                    .map_err(GuestMemoryError::resource_limit)?,
            ),
            None => FrameBytes::Owned(Box::new([0; GUEST_PAGE_SIZE_USIZE])),
        };
        #[cfg(not(feature = "shared-guest-memory"))]
        let bytes = FrameBytes::Owned(Box::new([0; GUEST_PAGE_SIZE_USIZE]));
        let (index, source) = if let Some(index) = self.free.pop_first() {
            let slot = self.frames.get_mut(index).ok_or_else(|| {
                GuestMemoryError::runtime_defect("free frame index is outside the allocator table")
            })?;
            if slot.is_some() {
                return Err(GuestMemoryError::runtime_defect(
                    "free frame list points to an allocated frame",
                ));
            }
            *slot = Some(bytes);
            (index, FrameAllocationSource::Reused)
        } else {
            self.frames.push(Some(bytes));
            (self.frames.len() - 1, FrameAllocationSource::Appended)
        };
        self.in_use += 1;
        Ok(FrameAllocation {
            frame: FrameId(index),
            source,
        })
    }

    fn release(&mut self, frame: FrameId, _retain: bool) -> Result<FrameBytes, GuestMemoryError> {
        let slot = self.frames.get_mut(frame.0).ok_or_else(|| {
            GuestMemoryError::runtime_defect("released frame index is outside the allocator table")
        })?;
        let bytes = slot.take().ok_or_else(|| {
            GuestMemoryError::runtime_defect("physical frame was released more than once")
        })?;
        // A released linear page can be recommitted at the same guest address.
        // Retain its old contents for transaction rollback before relinquishing
        // the unique arena page, exactly as the owned backing retained its Box.
        #[cfg(feature = "shared-guest-memory")]
        let bytes = match bytes {
            FrameBytes::Shared(page) if _retain => {
                FrameBytes::Released(page.address(), Box::new(*page))
            }
            bytes => bytes,
        };
        self.free.insert(frame.0);
        self.in_use = self.in_use.checked_sub(1).ok_or_else(|| {
            GuestMemoryError::runtime_defect("physical page usage underflowed during release")
        })?;
        Ok(bytes)
    }

    fn rollback(
        &mut self,
        undo: FrameUndo,
        images: &[[u8; GUEST_PAGE_SIZE_USIZE]],
    ) -> Result<(), GuestMemoryError> {
        match undo {
            FrameUndo::Bytes { frame, image } => {
                let bytes = images.get(image).ok_or_else(|| {
                    GuestMemoryError::runtime_defect(
                        "frame byte journal refers to a missing before-image",
                    )
                })?;
                self.bytes_mut(frame)?.copy_from_slice(bytes);
            }
            FrameUndo::Release { frame, bytes } => {
                #[cfg(feature = "shared-guest-memory")]
                let bytes = match bytes {
                    FrameBytes::Released(address, image) => {
                        let mut page = self
                            .arena
                            .as_mut()
                            .expect("released shared page has an arena")
                            .allocate(address)
                            .map_err(GuestMemoryError::runtime_defect)?;
                        page.copy_from_slice(image.as_slice());
                        FrameBytes::Shared(page)
                    }
                    bytes => bytes,
                };
                if !self.free.remove(&frame.0) {
                    return Err(GuestMemoryError::runtime_defect(
                        "released frame was missing from the allocator free set",
                    ));
                }
                let slot = self.frames.get_mut(frame.0).ok_or_else(|| {
                    GuestMemoryError::runtime_defect(
                        "released frame index disappeared during rollback",
                    )
                })?;
                if slot.is_some() {
                    return Err(GuestMemoryError::runtime_defect(
                        "released frame remained allocated during rollback",
                    ));
                }
                *slot = Some(bytes);
                self.in_use += 1;
            }
            FrameUndo::Allocation(allocation) => {
                let slot = self.frames.get_mut(allocation.frame.0).ok_or_else(|| {
                    GuestMemoryError::runtime_defect(
                        "allocated frame index disappeared during rollback",
                    )
                })?;
                if slot.take().is_none() {
                    return Err(GuestMemoryError::runtime_defect(
                        "allocated frame was not live during rollback",
                    ));
                }
                self.in_use = self.in_use.checked_sub(1).ok_or_else(|| {
                    GuestMemoryError::runtime_defect(
                        "physical page usage underflowed during rollback",
                    )
                })?;
                match allocation.source {
                    FrameAllocationSource::Reused => {
                        self.free.insert(allocation.frame.0);
                    }
                    FrameAllocationSource::Appended => {
                        if allocation.frame.0 + 1 != self.frames.len() {
                            return Err(GuestMemoryError::runtime_defect(
                                "appended frame was not the final allocator slot during rollback",
                            ));
                        }
                        self.frames.pop();
                    }
                }
            }
        }
        Ok(())
    }

    fn bytes(&self, frame: FrameId) -> Result<&[u8; GUEST_PAGE_SIZE_USIZE], GuestMemoryError> {
        self.frames
            .get(frame.0)
            .and_then(Option::as_ref)
            .map(|bytes| &**bytes)
            .ok_or_else(|| {
                GuestMemoryError::runtime_defect(
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
            .and_then(Option::as_mut)
            .map(|bytes| &mut **bytes)
            .ok_or_else(|| {
                GuestMemoryError::runtime_defect(
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

#[derive(Debug)]
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
    dirty_pages: BTreeSet<u32>,
    offered_generations: BTreeSet<u64>,
    offered_full_reflections: BTreeSet<u64>,
    initial_reflection_acknowledged: bool,
    transaction: Option<MemoryTransactionJournal>,
}

pub struct GuestMemoryTransaction<'a> {
    memory: &'a mut GuestMemory,
    active: bool,
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
        self.memory.commit_active_transaction();
        self.active = false;
    }
}

impl Drop for GuestMemoryTransaction<'_> {
    fn drop(&mut self) {
        if self.active {
            self.memory.rollback_active_transaction();
        }
    }
}

impl GuestMemory {
    /// Use one address-stable guest byte range for the Runtime and trusted CPU.
    /// Mapping/owner/protection authority remains in this object.
    #[cfg(feature = "shared-guest-memory")]
    pub fn new_shared(limits: MemoryLimits) -> Result<Self, GuestMemoryError> {
        let mut memory = Self::new(limits)?;
        memory.frames.arena =
            Some(vine_shared_memory::Arena::reserve().map_err(GuestMemoryError::resource_limit)?);
        Ok(memory)
    }

    #[cfg(feature = "shared-guest-memory")]
    pub fn shared_arena_descriptor(&self) -> Option<(u32, u32, u32)> {
        self.frames.arena.as_ref().map(|arena| {
            (
                arena.physical_base(),
                vine_shared_memory::PHYSICAL_BYTES,
                vine_shared_memory::GUEST_OFFSET,
            )
        })
    }

    pub fn new(limits: MemoryLimits) -> Result<Self, GuestMemoryError> {
        if limits.max_physical_pages == 0
            || limits.max_regions == 0
            || limits.max_owners == 0
            || limits.max_access_bytes == 0
        {
            return Err(GuestMemoryError::invalid_request(
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
            dirty_pages: BTreeSet::new(),
            offered_generations: BTreeSet::new(),
            offered_full_reflections: BTreeSet::new(),
            initial_reflection_acknowledged: false,
            transaction: None,
        })
    }

    pub fn begin_transaction(&mut self) -> GuestMemoryTransaction<'_> {
        assert!(
            self.transaction.is_none(),
            "nested GuestMemory transactions are not supported"
        );
        self.transaction = Some(MemoryTransactionJournal::new(self));
        GuestMemoryTransaction {
            memory: self,
            active: true,
        }
    }

    fn commit_active_transaction(&mut self) {
        self.transaction
            .take()
            .expect("active GuestMemoryTransaction lost its journal");
    }

    fn rollback_active_transaction(&mut self) {
        let journal = self
            .transaction
            .take()
            .expect("active GuestMemoryTransaction lost its journal");
        #[cfg(feature = "shared-guest-memory")]
        let shared_changed_pages: Vec<_> = if self.frames.arena.is_some() {
            journal.pages.keys().copied().collect()
        } else {
            Vec::new()
        };

        for undo in journal.frame_undo.into_iter().rev() {
            self.frames
                .rollback(undo, &journal.frame_images)
                .expect("GuestMemory transaction frame journal was inconsistent");
        }
        for (virtual_page, before) in journal.pages {
            match before {
                Some(page) => {
                    self.pages.insert(virtual_page, page);
                }
                None => {
                    self.pages.remove(&virtual_page);
                }
            }
        }
        self.dirty_pages = self
            .pages
            .iter()
            .filter_map(|(&virtual_page, page)| page.dirty_generation.map(|_| virtual_page))
            .collect();
        for (start, before) in journal.regions {
            match before {
                Some(region) => {
                    self.regions.insert(start, region);
                }
                None => {
                    self.regions.remove(&start);
                }
            }
        }
        for (owner, before) in journal.owners {
            match before {
                Some(state) => {
                    self.owners.insert(owner, state);
                }
                None => {
                    self.owners.remove(&owner);
                }
            }
        }
        self.pending_changes
            .truncate(journal.baseline_pending_changes);
        self.next_owner = journal.baseline_next_owner;
        self.generation = journal.baseline_generation;
        self.acknowledged_generation = journal.baseline_acknowledged_generation;
        for generation in journal.offered_generations {
            self.offered_generations.remove(&generation);
        }
        for generation in journal.offered_full_reflections {
            self.offered_full_reflections.remove(&generation);
        }
        #[cfg(feature = "shared-guest-memory")]
        if let Some(arena) = self.frames.arena.as_mut() {
            // A transaction may have offered metadata before it failed. Restore
            // its permission entries along with the authoritative page states.
            for page in shared_changed_pages {
                let flags = self.pages.get(&page).map_or(0, |p| {
                    u8::from(p.protection.read)
                        | u8::from(p.protection.write && !p.protection.execute) << 1
                });
                arena.set_permissions(page, flags);
            }
        }
    }

    fn journal_owner_before(&mut self, owner: MemoryOwnerId) {
        let should_record = self
            .transaction
            .as_ref()
            .is_some_and(|journal| !journal.owners.contains_key(&owner));
        if should_record {
            let before = self.owners.get(&owner).cloned();
            self.transaction
                .as_mut()
                .expect("checked transaction journal disappeared")
                .owners
                .insert(owner, before);
        }
    }

    fn journal_region_before(&mut self, start: u32) {
        let should_record = self
            .transaction
            .as_ref()
            .is_some_and(|journal| !journal.regions.contains_key(&start));
        if should_record {
            let before = self.regions.get(&start).cloned();
            self.transaction
                .as_mut()
                .expect("checked transaction journal disappeared")
                .regions
                .insert(start, before);
        }
    }

    fn journal_page_before(&mut self, virtual_page: u32) {
        let should_record = self
            .transaction
            .as_ref()
            .is_some_and(|journal| !journal.pages.contains_key(&virtual_page));
        if should_record {
            let before = self.pages.get(&virtual_page).cloned();
            self.transaction
                .as_mut()
                .expect("checked transaction journal disappeared")
                .pages
                .insert(virtual_page, before);
        }
    }

    fn journal_frame_bytes_before(&mut self, frame: FrameId) -> Result<(), GuestMemoryError> {
        let should_record = self.transaction.as_ref().is_some_and(|journal| {
            !journal.allocated_frames.contains(&frame.0)
                && !journal.frame_bytes_recorded.contains(&frame.0)
        });
        if should_record {
            let bytes = self.frames.bytes(frame)?;
            let journal = self
                .transaction
                .as_mut()
                .expect("checked transaction journal disappeared");
            // Batch before-images in one growable allocation, not one Box per page.
            let image = journal.frame_images.len();
            journal.frame_images.push(*bytes);
            journal.frame_bytes_recorded.insert(frame.0);
            journal.frame_undo.push(FrameUndo::Bytes { frame, image });
        }
        Ok(())
    }

    fn allocate_frame_zeroed(&mut self, virtual_page: u32) -> Result<FrameId, GuestMemoryError> {
        let allocation = self.frames.allocate_zeroed(virtual_page)?;
        if let Some(journal) = self.transaction.as_mut() {
            journal.allocated_frames.insert(allocation.frame.0);
            journal.frame_undo.push(FrameUndo::Allocation(allocation));
        }
        Ok(allocation.frame)
    }

    fn release_frame(&mut self, frame: FrameId) -> Result<(), GuestMemoryError> {
        let bytes = self.frames.release(frame, self.transaction.is_some())?;
        if let Some(journal) = self.transaction.as_mut() {
            journal.frame_undo.push(FrameUndo::Release { frame, bytes });
        }
        Ok(())
    }

    fn record_offered_generation(&mut self, generation: u64, full_reflection: bool) {
        if self.offered_generations.insert(generation)
            && let Some(journal) = self.transaction.as_mut()
        {
            journal.offered_generations.insert(generation);
        }
        if full_reflection
            && self.offered_full_reflections.insert(generation)
            && let Some(journal) = self.transaction.as_mut()
        {
            journal.offered_full_reflections.insert(generation);
        }
    }

    pub fn limits(&self) -> MemoryLimits {
        self.limits
    }

    pub(crate) fn owner_is_sealed(&self, owner: MemoryOwnerId) -> Result<bool, GuestMemoryError> {
        Ok(self.require_owner(owner)?.sealed)
    }

    pub(crate) fn validate_stack_owner(
        &self,
        owner: MemoryOwnerId,
        reserved: GuestRange,
        committed: GuestRange,
    ) -> Result<(), GuestMemoryError> {
        let state = self.require_owner(owner)?;
        if !state.sealed || state.kind != MemoryOwnerKind::Anonymous {
            return Err(GuestMemoryError::conflict(
                "registered stack requires a sealed anonymous memory owner",
            ));
        }
        if !reserved.is_page_aligned()
            || !committed.is_page_aligned()
            || !contains_range(reserved, committed)
            || !self.regions.values().any(|region| {
                region.owner == owner
                    && region.kind == MemoryRegionKind::Stack
                    && region.range == reserved
            })
        {
            return Err(GuestMemoryError::conflict(
                "registered stack ranges do not match the owner's live stack reservation",
            ));
        }
        for address in
            (u64::from(committed.start().0)..committed.end()).step_by(GUEST_PAGE_SIZE as usize)
        {
            let virtual_page = (address / GUEST_PAGE_SIZE) as u32;
            self.require_committed_owner(owner, &[virtual_page])?;
        }
        Ok(())
    }

    pub(crate) fn validate_sealed_image_owner(
        &self,
        owner: MemoryOwnerId,
        source: &ModuleIdentity,
        image_range: GuestRange,
    ) -> Result<(), GuestMemoryError> {
        self.validate_image_owner(owner, source, image_range, true)
    }

    pub(crate) fn validate_unsealed_image_owner(
        &self,
        owner: MemoryOwnerId,
        source: &ModuleIdentity,
        image_range: GuestRange,
    ) -> Result<(), GuestMemoryError> {
        self.validate_image_owner(owner, source, image_range, false)
    }

    fn validate_image_owner(
        &self,
        owner: MemoryOwnerId,
        source: &ModuleIdentity,
        image_range: GuestRange,
        expected_sealed: bool,
    ) -> Result<(), GuestMemoryError> {
        if !image_range.is_page_aligned() {
            return Err(GuestMemoryError::invalid_request(
                "registered module image range must use whole guest pages",
            ));
        }
        let owner_state = self.require_owner(owner)?;
        let MemoryOwnerKind::Image { source_sha256 } = &owner_state.kind else {
            return Err(GuestMemoryError::conflict(
                "registered module image owner is not an image owner",
            ));
        };
        if source_sha256 != &source.sha256 {
            return Err(GuestMemoryError::conflict(
                "registered module source does not match the guest-memory image owner",
            ));
        }
        if expected_sealed && !owner_state.sealed {
            return Err(GuestMemoryError::conflict(
                "registered module image owner must be sealed",
            ));
        }
        if !expected_sealed && owner_state.sealed {
            return Err(GuestMemoryError::conflict(
                "guest import binding requires an unsealed module image owner",
            ));
        }

        let mut owner_regions = self.regions.values().filter(|region| region.owner == owner);
        let exact_region = owner_regions.next().is_some_and(|region| {
            region.kind == MemoryRegionKind::Image && region.range == image_range
        });
        if !exact_region || owner_regions.next().is_some() {
            return Err(GuestMemoryError::conflict(
                "registered module image owner does not own exactly the recorded image range",
            ));
        }

        let owned_pages = self
            .pages
            .iter()
            .filter(|(_, page)| page.owner == owner)
            .collect::<Vec<_>>();
        let all_pages_in_range = owned_pages.iter().all(|(virtual_page, _)| {
            let address = u64::from(**virtual_page) * GUEST_PAGE_SIZE;
            address >= u64::from(image_range.start().0) && address < image_range.end()
        });
        if owned_pages.len() != image_range.page_count() || !all_pages_in_range {
            return Err(GuestMemoryError::conflict(
                "registered module image owner does not own every page in exactly its recorded range",
            ));
        }
        Ok(())
    }

    pub(crate) fn preflight_growth(
        &self,
        additional_owners: usize,
        additional_regions: usize,
        additional_physical_pages: usize,
    ) -> Result<(), GuestMemoryError> {
        let owner_ids = u32::try_from(additional_owners).map_err(|_| {
            GuestMemoryError::resource_limit("required memory owner IDs exceed x86 accounting")
        })?;
        self.next_owner.checked_add(owner_ids).ok_or_else(|| {
            GuestMemoryError::resource_limit("memory owner identifier space was exhausted")
        })?;
        let owners = self
            .owners
            .len()
            .checked_add(additional_owners)
            .ok_or_else(|| {
                GuestMemoryError::resource_limit(
                    "required memory owner count overflowed host accounting",
                )
            })?;
        if owners > self.limits.max_owners {
            return Err(GuestMemoryError::resource_limit(format!(
                "operation needs {additional_owners} additional owners; {} of {} are live",
                self.owners.len(),
                self.limits.max_owners
            )));
        }
        let regions = self
            .regions
            .len()
            .checked_add(additional_regions)
            .ok_or_else(|| {
                GuestMemoryError::resource_limit(
                    "required memory region count overflowed host accounting",
                )
            })?;
        if regions > self.limits.max_regions {
            return Err(GuestMemoryError::resource_limit(format!(
                "operation needs {additional_regions} additional regions; {} of {} are live",
                self.regions.len(),
                self.limits.max_regions
            )));
        }
        if additional_physical_pages > self.frames.available() {
            return Err(GuestMemoryError::resource_limit(format!(
                "operation needs {additional_physical_pages} physical pages; {} remain under quota",
                self.frames.available()
            )));
        }
        Ok(())
    }

    pub(crate) fn preflight_fixed_reservation(
        &self,
        start: GuestAddress,
        byte_length: u64,
    ) -> Result<GuestRange, GuestMemoryError> {
        let range = GuestRange::new(start, byte_length)?;
        require_page_aligned(range)?;
        #[cfg(feature = "shared-guest-memory")]
        if self.frames.arena.is_some() && range.end() > u64::from(vine_shared_memory::GUEST_BYTES) {
            return Err(GuestMemoryError::resource_limit(
                "Fast memory supports guest addresses below 2 GiB",
            ));
        }
        if self
            .regions
            .values()
            .any(|region| ranges_overlap(region.range, range))
        {
            return Err(GuestMemoryError::conflict(format!(
                "requested range [{:#010x}, {:#010x}) overlaps an existing region",
                range.start().0,
                range.end()
            )));
        }
        Ok(range)
    }

    pub fn create_owner(
        &mut self,
        kind: MemoryOwnerKind,
    ) -> Result<MemoryOwnerId, GuestMemoryError> {
        if self.owners.len() >= self.limits.max_owners {
            return Err(GuestMemoryError::resource_limit(format!(
                "memory owner limit {} was reached",
                self.limits.max_owners
            )));
        }
        let owner = MemoryOwnerId(self.next_owner);
        self.journal_owner_before(owner);
        self.next_owner = self.next_owner.checked_add(1).ok_or_else(|| {
            GuestMemoryError::resource_limit("memory owner identifier space was exhausted")
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
        self.reserve_fixed_internal(None, owner, kind, start, byte_length)
    }

    pub fn reserve_fixed_for_address_allocation(
        &mut self,
        address_allocation_id: u64,
        owner: MemoryOwnerId,
        kind: MemoryRegionKind,
        start: GuestAddress,
        byte_length: u64,
    ) -> Result<GuestRange, GuestMemoryError> {
        if address_allocation_id == 0 {
            return Err(GuestMemoryError::invalid_request(
                "an allocation-backed memory region requires a nonzero address allocation ID",
            ));
        }
        self.reserve_fixed_internal(Some(address_allocation_id), owner, kind, start, byte_length)
    }

    fn reserve_fixed_internal(
        &mut self,
        address_allocation_id: Option<u64>,
        owner: MemoryOwnerId,
        kind: MemoryRegionKind,
        start: GuestAddress,
        byte_length: u64,
    ) -> Result<GuestRange, GuestMemoryError> {
        self.require_owner(owner)?;
        if self.regions.len() >= self.limits.max_regions {
            return Err(GuestMemoryError::resource_limit(format!(
                "memory region limit {} was reached",
                self.limits.max_regions
            )));
        }
        let range = GuestRange::new(start, byte_length)?;
        require_page_aligned(range)?;
        #[cfg(feature = "shared-guest-memory")]
        if self.frames.arena.is_some() && range.end() > u64::from(vine_shared_memory::GUEST_BYTES) {
            return Err(GuestMemoryError::resource_limit(
                "Fast memory supports guest addresses below 2 GiB",
            ));
        }
        if self
            .regions
            .values()
            .any(|region| ranges_overlap(region.range, range))
        {
            return Err(GuestMemoryError::conflict(
                "reserved guest range overlaps an existing region",
            ));
        }
        self.ensure_generation_available()?;
        self.journal_region_before(start.0);
        self.regions.insert(
            start.0,
            MemoryRegion {
                address_allocation_id,
                range,
                owner,
                kind,
                initial_protection: None,
            },
        );
        self.record_change(
            vec![MappingOperation::Reserve {
                address_allocation_id,
                owner,
                kind,
                range,
            }],
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
                "memory owner cannot be destroyed while regions or pages remain live",
            ));
        }
        self.journal_owner_before(owner);
        self.owners
            .remove(&owner)
            .map(|state| state.kind)
            .ok_or_else(|| {
                GuestMemoryError::runtime_defect(
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
        require_page_aligned(range)?;
        let address_allocation_id = self.require_region_contains(owner, range)?;
        let pages = virtual_pages(range)?;
        if pages.clone().any(|page| self.pages.contains_key(&page)) {
            return Err(GuestMemoryError::conflict(
                "commit range contains an already committed page",
            ));
        }
        let page_count = range.page_count();
        if page_count > self.frames.available() {
            return Err(GuestMemoryError::resource_limit(format!(
                "commit requires {page_count} pages; {} remain under quota",
                self.frames.available()
            )));
        }
        let generation = self.next_generation()?;

        let mut allocated = Vec::with_capacity(page_count);
        for virtual_page in pages {
            match self.allocate_frame_zeroed(virtual_page) {
                Ok(frame) => allocated.push((virtual_page, frame)),
                Err(error) => {
                    for (_, frame) in allocated {
                        self.release_frame(frame)?;
                    }
                    return Err(error);
                }
            }
        }
        let region_start = self
            .regions
            .range(..=range.start().0)
            .next_back()
            .expect("validated committed region")
            .0
            .to_owned();
        self.journal_region_before(region_start);
        self.regions
            .get_mut(&region_start)
            .expect("validated region")
            .initial_protection
            .get_or_insert(protection);
        for (virtual_page, frame) in allocated {
            self.journal_page_before(virtual_page);
            self.pages.insert(
                virtual_page,
                PageState {
                    owner,
                    frame,
                    protection,
                    dirty_generation: Some(generation),
                },
            );
            self.dirty_pages.insert(virtual_page);
        }
        self.record_change(
            vec![MappingOperation::Commit {
                address_allocation_id,
                owner,
                range,
                protection,
            }],
            CacheEffects {
                flush_tlb: true,
                invalidate_instruction_fetch: protection.execute,
                invalidate_generated_code: protection.execute,
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
        require_page_aligned(range)?;
        let address_allocation_id = self.require_region_contains(owner, range)?;
        let pages: Vec<u32> = virtual_pages(range)?.collect();
        self.require_committed_owner(owner, &pages)?;
        self.ensure_generation_available()?;
        let touches_executable = protection.execute || self.pages_touch_executable(&pages);
        for virtual_page in pages {
            self.journal_page_before(virtual_page);
            let page = self.pages.get_mut(&virtual_page).ok_or_else(|| {
                GuestMemoryError::runtime_defect("protect preflight succeeded but page disappeared")
            })?;
            page.protection = protection;
        }
        self.record_change(
            vec![MappingOperation::Protect {
                address_allocation_id,
                owner,
                range,
                protection,
            }],
            CacheEffects {
                flush_tlb: true,
                invalidate_instruction_fetch: touches_executable,
                invalidate_generated_code: touches_executable,
            },
        )
    }

    pub fn decommit(
        &mut self,
        owner: MemoryOwnerId,
        range: GuestRange,
    ) -> Result<MemoryInvalidation, GuestMemoryError> {
        self.require_owner(owner)?;
        require_page_aligned(range)?;
        let address_allocation_id = self.require_region_contains(owner, range)?;
        let pages: Vec<u32> = virtual_pages(range)?.collect();
        self.require_committed_owner(owner, &pages)?;
        self.ensure_generation_available()?;
        let touches_executable = self.pages_touch_executable(&pages);
        for virtual_page in pages {
            self.journal_page_before(virtual_page);
            let page = self.pages.remove(&virtual_page).ok_or_else(|| {
                GuestMemoryError::runtime_defect(
                    "decommit preflight succeeded but page disappeared",
                )
            })?;
            self.dirty_pages.remove(&virtual_page);
            self.release_frame(page.frame)?;
        }
        self.record_change(
            vec![MappingOperation::Decommit {
                address_allocation_id,
                owner,
                range,
            }],
            CacheEffects {
                flush_tlb: true,
                invalidate_instruction_fetch: touches_executable,
                invalidate_generated_code: touches_executable,
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
                "release requires the exact base of an existing region",
            )
        })?;
        if region.owner != owner {
            return Err(GuestMemoryError::conflict(
                "memory region belongs to a different owner",
            ));
        }
        let range = region.range;
        let address_allocation_id = region.address_allocation_id;
        let pages: Vec<u32> = virtual_pages(range)?
            .filter(|page| self.pages.contains_key(page))
            .collect();
        self.require_committed_owner(owner, &pages)?;
        self.ensure_generation_available()?;
        let touches_executable = self.pages_touch_executable(&pages);
        for virtual_page in &pages {
            self.journal_page_before(*virtual_page);
            let page = self.pages.remove(virtual_page).ok_or_else(|| {
                GuestMemoryError::runtime_defect("release preflight succeeded but page disappeared")
            })?;
            self.dirty_pages.remove(virtual_page);
            self.release_frame(page.frame)?;
        }
        self.journal_region_before(start.0);
        self.regions.remove(&start.0);
        self.record_change(
            vec![MappingOperation::Release {
                address_allocation_id,
                owner,
                range,
            }],
            CacheEffects {
                flush_tlb: !pages.is_empty(),
                invalidate_instruction_fetch: touches_executable,
                invalidate_generated_code: touches_executable,
            },
        )
        .map(Some)
    }

    pub(crate) fn prepare_image_layout_at_transaction(
        &mut self,
        plan: &Pe32LoadPlan,
        load_base: GuestAddress,
    ) -> Result<ImageMemoryLayout, GuestMemoryError> {
        let source = &plan.source;
        if !load_base.0.is_multiple_of(0x1_0000) {
            return Err(GuestMemoryError::invalid_request(
                "image load base must be aligned to the 64 KiB Windows allocation granularity",
            ));
        }
        let preferred_end = u64::from(plan.image.preferred_image_base)
            .checked_add(u64::from(plan.image.size_of_image))
            .ok_or_else(|| {
                GuestMemoryError::invalid_request(
                    "preferred image base plus SizeOfImage overflowed 64-bit accounting",
                )
            })?;
        if preferred_end != plan.image.image_end {
            return Err(GuestMemoryError::invalid_request(
                "preferred image base plus SizeOfImage does not match the load plan image end",
            ));
        }
        let image_range = GuestRange::new(load_base, u64::from(plan.image.size_of_image))?;
        require_page_aligned(image_range)?;

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
            return Err(GuestMemoryError::resource_limit(format!(
                "image needs {} committed pages; {} remain under quota",
                page_specs.len(),
                self.frames.available()
            )));
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
            self.journal_owner_before(owner);
            self.owners.remove(&owner);
            return Err(error);
        }

        let mut allocated = Vec::with_capacity(page_specs.len());
        for (virtual_page, protection) in page_specs {
            match self.allocate_frame_zeroed(virtual_page) {
                Ok(frame) => allocated.push((virtual_page, protection, frame)),
                Err(error) => {
                    for (_, _, frame) in allocated {
                        self.release_frame(frame)?;
                    }
                    self.journal_region_before(image_range.start.0);
                    self.regions.remove(&image_range.start.0);
                    self.journal_owner_before(owner);
                    self.owners.remove(&owner);
                    return Err(error);
                }
            }
        }

        let committed_pages = allocated.len();
        let generation = self.next_generation()?;
        let mut mapping_operations = Vec::with_capacity(committed_pages);
        for (virtual_page, protection, frame) in allocated {
            self.journal_page_before(virtual_page);
            self.pages.insert(
                virtual_page,
                PageState {
                    owner,
                    frame,
                    protection,
                    dirty_generation: Some(generation),
                },
            );
            self.dirty_pages.insert(virtual_page);
            mapping_operations.push(MappingOperation::Commit {
                address_allocation_id: None,
                owner,
                range: GuestRange::new(
                    GuestAddress(
                        virtual_page
                            .checked_mul(GUEST_PAGE_SIZE as u32)
                            .ok_or_else(|| {
                                GuestMemoryError::runtime_defect(
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
            owner,
            image_range,
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
    ) -> Result<(), GuestMemoryError> {
        let owner_state = self.require_owner(owner)?;
        if owner_state.sealed {
            return Err(GuestMemoryError::conflict(
                "owner initialization writes are forbidden after sealing",
            ));
        }
        let segments = self.preflight_owner_access(owner, address, bytes.len())?;
        let touches_executable = self.segments_touch_executable(&segments);
        if self.initial_reflection_acknowledged {
            let generation = self.next_generation()?;
            self.write_segments(&segments, bytes, generation)?;
            self.record_change(
                Vec::new(),
                CacheEffects {
                    flush_tlb: false,
                    invalidate_instruction_fetch: touches_executable,
                    invalidate_generated_code: touches_executable,
                },
            )?;
        } else {
            self.write_segments(&segments, bytes, self.generation)?;
        };
        Ok(())
    }

    pub fn zero_initialization(
        &mut self,
        owner: MemoryOwnerId,
        address: GuestAddress,
        byte_length: usize,
    ) -> Result<(), GuestMemoryError> {
        let owner_state = self.require_owner(owner)?;
        if owner_state.sealed {
            return Err(GuestMemoryError::conflict(
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
            self.journal_page_before(segment.virtual_page);
            let frame = self
                .pages
                .get(&segment.virtual_page)
                .ok_or_else(|| {
                    GuestMemoryError::runtime_defect(
                        "zero-fill preflight succeeded but page disappeared",
                    )
                })?
                .frame;
            self.journal_frame_bytes_before(frame)?;
            let page = self.pages.get_mut(&segment.virtual_page).ok_or_else(|| {
                GuestMemoryError::runtime_defect(
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
            self.dirty_pages.insert(segment.virtual_page);
        }
        if self.initial_reflection_acknowledged {
            self.record_change(
                Vec::new(),
                CacheEffects {
                    flush_tlb: false,
                    invalidate_instruction_fetch: touches_executable,
                    invalidate_generated_code: touches_executable,
                },
            )?;
        }
        Ok(())
    }

    pub fn seal_owner(
        &mut self,
        owner: MemoryOwnerId,
    ) -> Result<MemoryInvalidation, GuestMemoryError> {
        if self.require_owner(owner)?.sealed {
            return Err(GuestMemoryError::conflict(
                "memory owner was sealed more than once",
            ));
        }
        self.ensure_generation_available()?;
        let has_executable_pages = self
            .pages
            .values()
            .any(|page| page.owner == owner && page.protection.execute);
        self.journal_owner_before(owner);
        let state = self.owners.get_mut(&owner).ok_or_else(|| {
            GuestMemoryError::runtime_defect(
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

    pub(crate) fn flush_instruction_cache(
        &mut self,
    ) -> Result<MemoryInvalidation, GuestMemoryError> {
        self.ensure_generation_available()?;
        self.record_change(
            Vec::new(),
            CacheEffects {
                flush_tlb: false,
                invalidate_instruction_fetch: true,
                invalidate_generated_code: true,
            },
        )
    }

    pub fn acknowledge_delta(&mut self, generation: u64) -> Result<(), GuestMemoryError> {
        if self.transaction.is_some() {
            return Err(GuestMemoryError::conflict(
                "memory deltas cannot be acknowledged before the active transaction commits",
            ));
        }
        if generation > self.generation {
            return Err(GuestMemoryError::invalid_request(format!(
                "acknowledged generation {generation}, current generation is {}",
                self.generation
            )));
        }
        if generation <= self.acknowledged_generation {
            return Err(GuestMemoryError::invalid_request(format!(
                "acknowledged generation {generation}, last acknowledged generation is {}",
                self.acknowledged_generation
            )));
        }
        if !self.offered_generations.contains(&generation) {
            return Err(GuestMemoryError::invalid_request(
                "memory generation must be obtained from a reflection or delta before acknowledgement",
            ));
        }
        if self.pending_changes.is_empty() {
            return Err(GuestMemoryError::invalid_request(
                "there is no pending memory delta to acknowledge",
            ));
        }

        let acknowledged_pages = self
            .dirty_pages
            .iter()
            .filter_map(|virtual_page| {
                self.pages
                    .get(virtual_page)
                    .and_then(|page| page.dirty_generation)
                    .is_some_and(|dirty_generation| dirty_generation <= generation)
                    .then_some(*virtual_page)
            })
            .collect::<Vec<_>>();
        for virtual_page in acknowledged_pages {
            self.pages
                .get_mut(&virtual_page)
                .expect("dirty-page index references a committed page")
                .dirty_generation = None;
            self.dirty_pages.remove(&virtual_page);
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

    #[cfg(test)]
    pub(crate) fn pending_delta(&mut self) -> Result<MemoryDelta, GuestMemoryError> {
        let invalidation = self.require_pending_delta()?;
        let mapping_operations = self.pending_mapping_operations();
        let dirty_pages = self.collect_dirty_pages()?;
        self.record_offered_generation(invalidation.generation, false);
        Ok(MemoryDelta {
            from_generation: self.acknowledged_generation,
            generation: invalidation.generation,
            page_size: GUEST_PAGE_SIZE,
            mapping_operations,
            dirty_pages,
            cache_effects: invalidation.into(),
        })
    }

    pub fn pending_delta_binary(&mut self) -> Result<MemoryDeltaBuffers, GuestMemoryError> {
        let invalidation = self.require_pending_delta()?;
        self.synchronize_shared_permissions();
        let shared_guest_base = self.shared_guest_base();
        let mut buffers = MemoryDeltaBuffers::new(
            self.acknowledged_generation,
            invalidation.generation,
            &self.pending_mapping_operations(),
            invalidation.into(),
            self.dirty_pages.len(),
            shared_guest_base,
        )?;
        // Encode directly from authoritative frames: no per-page allocation or intermediate copy.
        for &virtual_page in &self.dirty_pages {
            let page = self.require_dirty_page(virtual_page)?;
            let owner = self.require_owner(page.owner)?;
            buffers.push_page(
                virtual_page_address(virtual_page)?,
                page.owner,
                page_protection_flags(page.protection) | u32::from(owner.sealed) << 3,
                self.frames.bytes(page.frame)?,
            )?;
        }
        self.record_offered_generation(invalidation.generation, false);
        Ok(buffers)
    }

    fn require_pending_delta(&self) -> Result<MemoryInvalidation, GuestMemoryError> {
        if !self.initial_reflection_acknowledged {
            return Err(GuestMemoryError::invalid_request(
                "a full memory reflection must be acknowledged before incremental deltas",
            ));
        }
        self.pending_invalidation().ok_or_else(|| {
            GuestMemoryError::invalid_request("there is no pending memory delta to transfer")
        })
    }

    fn pending_mapping_operations(&self) -> Vec<MappingOperation> {
        self.pending_changes
            .iter()
            .flat_map(|change| change.mapping_operations.iter().cloned())
            .collect()
    }

    fn require_dirty_page(&self, virtual_page: u32) -> Result<&PageState, GuestMemoryError> {
        let page = self.pages.get(&virtual_page).ok_or_else(|| {
            GuestMemoryError::runtime_defect(
                "dirty-page index references an uncommitted guest page",
            )
        })?;
        if page.dirty_generation.is_none() {
            return Err(GuestMemoryError::runtime_defect(
                "dirty-page index references a clean guest page",
            ));
        }
        Ok(page)
    }

    #[cfg(test)]
    fn collect_dirty_pages(&self) -> Result<Vec<DirtyPage>, GuestMemoryError> {
        let mut dirty_pages = Vec::with_capacity(self.dirty_pages.len());
        for &virtual_page in &self.dirty_pages {
            let page = self.require_dirty_page(virtual_page)?;
            let address = virtual_page_address(virtual_page)?;
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
        self.ensure_synchronized()?;
        self.read_range_without_synchronization(address, byte_length, MemoryAccess::Read)
    }

    pub(crate) fn read_owned(
        &self,
        address: GuestAddress,
        byte_length: usize,
        owner: MemoryOwnerId,
    ) -> Result<Vec<u8>, GuestMemoryError> {
        let range = GuestRange::new(address, byte_length as u64)?;
        self.require_region_contains(owner, range)?;
        self.read(address, byte_length)
    }

    /// Internal owner preparation may inspect authoritative bytes after its
    /// synchronous transaction has published a delta. Guest execution still
    /// uses synchronized access and must acknowledge that delta first.
    pub(crate) fn read_owned_host(
        &self,
        address: GuestAddress,
        byte_length: usize,
        owner: MemoryOwnerId,
    ) -> Result<Vec<u8>, GuestMemoryError> {
        let range = GuestRange::new(address, byte_length as u64)?;
        self.require_region_contains(owner, range)?;
        self.read_range_without_synchronization(address, byte_length, MemoryAccess::Read)
    }

    pub fn validate_access(
        &self,
        address: GuestAddress,
        byte_length: usize,
        access: MemoryAccess,
    ) -> Result<(), GuestMemoryError> {
        self.ensure_synchronized()?;
        self.validate_range_without_synchronization(address, byte_length, access)
    }

    pub(crate) fn validate_owned_host_execute(
        &self,
        owner: MemoryOwnerId,
        address: GuestAddress,
        byte_length: usize,
    ) -> Result<(), GuestMemoryError> {
        self.preflight_owner_access(owner, address, byte_length)?;
        self.validate_host_execute(address, byte_length)
    }

    pub(crate) fn validate_host_execute(
        &self,
        address: GuestAddress,
        byte_length: usize,
    ) -> Result<(), GuestMemoryError> {
        // Host preparation checks live mappings without reading unsynchronized
        // guest bytes. CPU execution still requires reflection/delta acknowledgement.
        self.validate_range_without_synchronization(address, byte_length, MemoryAccess::Execute)
    }

    pub fn fetch(
        &self,
        address: GuestAddress,
        byte_length: usize,
    ) -> Result<Vec<u8>, GuestMemoryError> {
        self.ensure_synchronized()?;
        self.read_range_without_synchronization(address, byte_length, MemoryAccess::Execute)
    }

    pub(crate) fn validate_host_write(
        &self,
        address: GuestAddress,
        byte_length: usize,
    ) -> Result<(), GuestMemoryError> {
        // Host-side transactions may have just committed the allocation's pages.
        self.validate_range_without_synchronization(address, byte_length, MemoryAccess::Write)
    }

    pub fn write(&mut self, address: GuestAddress, bytes: &[u8]) -> Result<(), GuestMemoryError> {
        let segments = self.preflight_guest_access_without_synchronization(
            address,
            bytes.len(),
            MemoryAccess::Write,
        )?;
        let touches_executable = self.segments_touch_executable(&segments);
        let generation = self.next_generation()?;
        self.write_segments(&segments, bytes, generation)?;
        self.record_change(
            Vec::new(),
            CacheEffects {
                flush_tlb: false,
                invalidate_instruction_fetch: touches_executable,
                invalidate_generated_code: touches_executable,
            },
        )?;
        Ok(())
    }

    pub fn reconcile_cpu_writeback_binary(
        &mut self,
        control: &[u8],
        page_bytes: &[u8],
    ) -> Result<u64, GuestMemoryError> {
        if self.shared_guest_base() != 0 {
            return Err(GuestMemoryError::invalid_request(
                "shared guest memory has no CPU writeback transport",
            ));
        }
        let batch = decode_cpu_writeback_view(control, page_bytes, self.limits.max_physical_pages)?;
        self.reconcile_cpu_writeback_view(batch.base_generation, &batch.pages)
    }

    fn reconcile_cpu_writeback_view(
        &mut self,
        base_generation: u64,
        pages: &[CpuWritebackPageView<'_>],
    ) -> Result<u64, GuestMemoryError> {
        self.ensure_synchronized()?;
        if !self.initial_reflection_acknowledged {
            return Err(GuestMemoryError::invalid_request(
                "a full memory reflection must be acknowledged before CPU writeback",
            ));
        }
        if base_generation != self.generation || base_generation != self.acknowledged_generation {
            return Err(GuestMemoryError::conflict(format!(
                "CPU writeback generation {} does not match synchronized generation {}",
                base_generation, self.acknowledged_generation
            )));
        }
        if pages.len() > self.limits.max_physical_pages {
            return Err(GuestMemoryError::resource_limit(format!(
                "CPU writeback contains {} pages; limit is {}",
                pages.len(),
                self.limits.max_physical_pages
            )));
        }

        // Small batches avoid sorting; large batches use sorted order for duplicate detection.
        let mut inline_changed = [(0usize, FrameId(0)); 16];
        let mut heap_changed =
            (pages.len() > inline_changed.len()).then(|| Vec::with_capacity(pages.len()));
        let order = if pages.len() > inline_changed.len() {
            let mut order = (0..pages.len()).collect::<Vec<_>>();
            order.sort_unstable_by_key(|index| pages[*index].address);
            for pair in order.windows(2) {
                if pages[pair[0]].address == pages[pair[1]].address {
                    return Err(GuestMemoryError::conflict(
                        "CPU writeback contains the same guest page more than once",
                    ));
                }
            }
            Some(order)
        } else {
            for index in 0..pages.len() {
                let address = pages[index].address;
                if (0..index).any(|previous| pages[previous].address == address) {
                    return Err(GuestMemoryError::conflict(
                        "CPU writeback contains the same guest page more than once",
                    ));
                }
            }
            None
        };

        let mut changed_pages = 0usize;
        for offset in 0..pages.len() {
            let index = order.as_ref().map_or(offset, |order| order[offset]);
            let writeback = pages[index];
            if !writeback.address.0.is_multiple_of(GUEST_PAGE_SIZE as u32) {
                return Err(GuestMemoryError::invalid_request(
                    "CPU writeback page address must be guest-page aligned",
                ));
            }
            let bytes: &[u8; GUEST_PAGE_SIZE_USIZE] = writeback.bytes.try_into().map_err(|_| {
                GuestMemoryError::invalid_request(
                    "CPU writeback must provide exactly one page per dirty-page descriptor",
                )
            })?;
            let page = self.preflight_cpu_writeback_page(writeback.address)?;
            if page.owner != writeback.owner {
                return Err(GuestMemoryError::conflict(
                    "CPU writeback owner does not match the mapped guest page",
                ));
            }
            if page.dirty_generation.is_some() {
                return Err(GuestMemoryError::runtime_defect(
                    "a synchronized CPU writeback page retained host dirty state",
                ));
            }
            if !cpu_writeback_pages_equal(self.frames.bytes(page.frame)?, bytes) {
                if let Some(changed) = &mut heap_changed {
                    changed.push((index, page.frame));
                } else {
                    inline_changed[changed_pages] = (index, page.frame);
                }
                changed_pages += 1;
            }
        }

        let from_generation = self.generation;
        let generation = if changed_pages == 0 {
            from_generation
        } else {
            self.next_generation()?
        };
        if changed_pages != 0 {
            let changed_order = match &heap_changed {
                Some(changed) => changed.as_slice(),
                None => &inline_changed[..changed_pages],
            };
            for &(index, frame) in changed_order {
                let writeback = pages[index];
                self.frames
                    .bytes_mut(frame)
                    .expect("preflighted CPU writeback frame disappeared")
                    .copy_from_slice(writeback.bytes);
            }
            self.generation = generation;
            self.acknowledged_generation = generation;
        }

        Ok(generation)
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
            dirty_pages: self.dirty_pages.len(),
            generation: self.generation,
            acknowledged_generation: self.acknowledged_generation,
            pending_invalidation: self.pending_invalidation(),
        }
    }

    pub fn synchronization_state(&self) -> MemorySynchronizationState {
        MemorySynchronizationState {
            generation: self.generation,
            acknowledged_generation: self.acknowledged_generation,
        }
    }

    pub fn has_pending_delta(&self) -> bool {
        !self.pending_changes.is_empty()
    }

    pub fn regions(&self) -> impl ExactSizeIterator<Item = MemoryRegionInfo> + '_ {
        self.regions.values().map(|region| MemoryRegionInfo {
            address_allocation_id: region.address_allocation_id,
            owner: region.owner,
            kind: region.kind,
            range: region.range,
        })
    }

    pub(crate) fn query_windows_memory(&self, address: GuestAddress) -> [u32; 7] {
        let base = address.0 & !0xfff;
        let protection = |p: PageProtection| match (p.read, p.write, p.execute) {
            (_, true, true) => 0x40,
            (true, false, true) => 0x20,
            (false, false, true) => 0x10,
            (_, true, false) => 4,
            (true, false, false) => 2,
            (false, false, false) => 1,
        };
        let Some(region) = self
            .regions()
            .find(|r| r.range.start().0 <= base && u64::from(base) < r.range.end())
        else {
            let end = self
                .regions()
                .filter(|r| r.range.start().0 > base)
                .map(|r| r.range.start().0)
                .min()
                .unwrap_or(0x7fff_0000);
            return [base, 0, 0, end - base, 0x10000, 0, 0];
        };
        let page = self.pages.get(&(base / 4096));
        let current = page.map(|p| p.protection);
        let mut end = u64::from(base) + GUEST_PAGE_SIZE;
        if current.is_none() {
            end = self
                .pages
                .range((base / 4096)..)
                .next()
                .map_or(region.range.end(), |(number, _)| {
                    u64::from(*number) * GUEST_PAGE_SIZE
                })
                .min(region.range.end());
        } else {
            while end < region.range.end()
                && self
                    .pages
                    .get(&((end / GUEST_PAGE_SIZE) as u32))
                    .map(|p| p.protection)
                    == current
            {
                end += GUEST_PAGE_SIZE;
            }
        }
        let initial = self
            .regions
            .get(&region.range.start().0)
            .and_then(|r| r.initial_protection)
            .or_else(|| {
                self.pages
                    .get(&(region.range.start().0 / 4096))
                    .map(|p| p.protection)
            })
            .map_or(0, protection);
        [
            base,
            region.range.start().0,
            initial,
            (end - u64::from(base)) as u32,
            if current.is_some() { 0x1000 } else { 0x2000 },
            current.map_or(0, protection),
            if region.kind == MemoryRegionKind::Image {
                0x1000000
            } else {
                0x20000
            },
        ]
    }

    pub(crate) fn image_ranges(
        &self,
    ) -> impl Iterator<Item = Result<GuestRange, GuestMemoryError>> + '_ {
        self.regions
            .values()
            .filter(|region| region.kind == MemoryRegionKind::Image)
            .map(|region| {
                if !matches!(
                    &self.require_owner(region.owner)?.kind,
                    MemoryOwnerKind::Image { .. }
                ) {
                    return Err(GuestMemoryError::conflict(
                        "image memory region requires an image owner",
                    ));
                }
                Ok(region.range)
            })
    }

    pub(crate) fn committed_page(
        &self,
        address: GuestAddress,
    ) -> Result<Option<(MemoryOwnerId, PageProtection)>, GuestMemoryError> {
        if !address.0.is_multiple_of(GUEST_PAGE_SIZE as u32) {
            return Err(GuestMemoryError::invalid_request(
                "committed page queries require a page-aligned guest address",
            ));
        }
        Ok(self
            .pages
            .get(&(address.0 / GUEST_PAGE_SIZE as u32))
            .map(|page| (page.owner, page.protection)))
    }

    pub fn pending_reflection(&mut self) -> Result<MemoryReflection, GuestMemoryError> {
        if self.initial_reflection_acknowledged {
            return Err(GuestMemoryError::conflict(
                "incremental memory deltas must be used after the initial full reflection",
            ));
        }
        let invalidation = self.pending_invalidation().ok_or_else(|| {
            GuestMemoryError::invalid_request(
                "memory reflection requires a pending memory generation",
            )
        })?;
        let reflection = self.build_reflection(invalidation, false)?;
        self.record_offered_generation(invalidation.generation, true);
        Ok(reflection)
    }

    /// The common-memory CPU consumes mapping metadata, never a second copy of
    /// guest bytes. The zero-copy form is emitted only by an arena-backed owner.
    pub fn pending_reflection_binary(
        &mut self,
    ) -> Result<MemoryReflectionBuffers, GuestMemoryError> {
        let base = self.shared_guest_base();
        if base == 0 {
            return self.pending_reflection()?.encode_binary();
        }
        if self.initial_reflection_acknowledged {
            return Err(GuestMemoryError::conflict(
                "initial memory reflection already acknowledged",
            ));
        }
        self.synchronize_shared_permissions();
        let invalidation = self.pending_invalidation().ok_or_else(|| {
            GuestMemoryError::invalid_request("shared reflection has no pending generation")
        })?;
        let count = u32::try_from(self.pages.len()).map_err(|_| {
            GuestMemoryError::resource_limit("shared reflection page count overflow")
        })?;
        let mut control =
            Vec::with_capacity(MEMORY_REFLECTION_CONTROL_HEADER_BYTES + self.pages.len() * 16);
        control.extend_from_slice(MEMORY_REFLECTION_CONTROL_MAGIC);
        control.extend_from_slice(&(GUEST_PAGE_SIZE as u32).to_le_bytes());
        control.extend_from_slice(&invalidation.generation.to_le_bytes());
        control.extend_from_slice(&count.to_le_bytes());
        control.extend_from_slice(&16u32.to_le_bytes());
        let cache = u32::from(invalidation.data_tlb)
            | u32::from(invalidation.instruction_fetch) << 1
            | u32::from(invalidation.generated_code) << 2;
        control.extend_from_slice(&cache.to_le_bytes());
        control.extend_from_slice(&base.to_le_bytes());
        for (&virtual_page, page) in &self.pages {
            let owner = self.require_owner(page.owner)?;
            if page.protection.execute && !owner.sealed {
                return Err(GuestMemoryError::conflict(
                    "unsealed executable shared page",
                ));
            }
            let flags = page_protection_flags(page.protection)
                | u32::from(owner.sealed) << 3
                | u32::from(page.dirty_generation.is_some()) << 4;
            control.extend_from_slice(&virtual_page_address(virtual_page)?.0.to_le_bytes());
            control.extend_from_slice(&page.owner.0.to_le_bytes());
            control.extend_from_slice(&flags.to_le_bytes());
            control.extend_from_slice(&0u32.to_le_bytes());
        }
        self.record_offered_generation(invalidation.generation, true);
        Ok(MemoryReflectionBuffers {
            control,
            page_bytes: Vec::new(),
        })
    }

    fn shared_guest_base(&self) -> u32 {
        #[cfg(feature = "shared-guest-memory")]
        if let Some(arena) = &self.frames.arena {
            return arena.guest_base();
        }
        0
    }

    #[cfg(feature = "shared-guest-memory")]
    pub fn shared_permissions_pointer(&self) -> Option<u32> {
        self.frames
            .arena
            .as_ref()
            .map(|arena| arena.permissions_pointer())
    }

    #[cfg(feature = "shared-guest-memory")]
    pub fn shared_data_changes(&self) -> Option<(u64, Vec<u32>)> {
        if self.frames.arena.is_none()
            || self
                .pending_changes
                .iter()
                .any(|change| !change.mapping_operations.is_empty())
        {
            return None;
        }
        let pending = self.pending_invalidation()?;
        Some((
            pending.generation,
            self.dirty_pages.iter().map(|page| page * 4096).collect(),
        ))
    }

    #[cfg(feature = "shared-guest-memory")]
    pub fn offer_shared_data_changes(&mut self) -> Option<(u64, Vec<u32>)> {
        if !self.initial_reflection_acknowledged {
            return None;
        }
        let (generation, addresses) = self.shared_data_changes()?;
        self.record_offered_generation(generation, false);
        Some((generation, addresses))
    }

    fn synchronize_shared_permissions(&mut self) {
        #[cfg(feature = "shared-guest-memory")]
        {
            if self.frames.arena.is_none() {
                return;
            }
            let mut ranges = Vec::new();
            if self.initial_reflection_acknowledged {
                for operation in self.pending_mapping_operations() {
                    match operation {
                        MappingOperation::Reserve { .. } => {}
                        MappingOperation::Commit { range, .. }
                        | MappingOperation::Protect { range, .. }
                        | MappingOperation::Decommit { range, .. }
                        | MappingOperation::Release { range, .. } => ranges.push(range),
                    }
                }
            }
            let arena = self.frames.arena.as_mut().expect("shared arena is present");
            let flags = |page: &PageState| {
                u8::from(page.protection.read)
                    | u8::from(page.protection.write && !page.protection.execute) << 1
            };
            if !self.initial_reflection_acknowledged {
                for (&virtual_page, page) in &self.pages {
                    arena.set_permissions(virtual_page, flags(page));
                }
            } else {
                for range in ranges {
                    for address in
                        (u64::from(range.start.0)..range.end()).step_by(GUEST_PAGE_SIZE_USIZE)
                    {
                        if address >= u64::from(vine_shared_memory::GUEST_BYTES) {
                            continue;
                        }
                        let page = (address / GUEST_PAGE_SIZE) as u32;
                        arena.set_permissions(page, self.pages.get(&page).map_or(0, flags));
                    }
                }
            }
        }
    }

    /// Called with the Runtime exclusively borrowed for the entire CPU call.
    /// No Page reference survives this boundary. CPU data stores are already
    /// authoritative; this mode's generation tracks host mapping/byte changes.
    #[cfg(feature = "shared-guest-memory")]
    pub fn validate_shared_execution(&self, generation: u64) -> Result<(), GuestMemoryError> {
        self.ensure_synchronized()?;
        if self.shared_guest_base() == 0
            || !self.initial_reflection_acknowledged
            || generation != self.generation
        {
            return Err(GuestMemoryError::conflict(
                "shared CPU mapping generation is not current",
            ));
        }
        Ok(())
    }

    pub fn current_reflection(&self) -> Result<MemoryReflection, GuestMemoryError> {
        self.build_reflection(
            MemoryInvalidation {
                generation: self.generation,
                data_tlb: true,
                instruction_fetch: true,
                generated_code: true,
            },
            true,
        )
    }

    fn build_reflection(
        &self,
        invalidation: MemoryInvalidation,
        force_dirty: bool,
    ) -> Result<MemoryReflection, GuestMemoryError> {
        let mut pages = Vec::with_capacity(self.pages.len());
        for (&virtual_page, page) in &self.pages {
            let address = virtual_page_address(virtual_page)?;
            let owner = self.require_owner(page.owner)?;
            if page.protection.execute && !owner.sealed {
                return Err(GuestMemoryError::conflict(
                    "executable pages cannot be reflected before their owner is sealed",
                ));
            }
            let bytes = self.frames.bytes(page.frame)?.to_vec();
            let dirty = force_dirty || page.dirty_generation.is_some();
            pages.push(MemoryReflectionPage {
                address,
                owner: page.owner,
                owner_sealed: owner.sealed,
                protection: page.protection,
                dirty,
                bytes,
            });
        }
        Ok(MemoryReflection {
            invalidation,
            page_size: GUEST_PAGE_SIZE,
            pages,
        })
    }

    fn require_owner(&self, owner: MemoryOwnerId) -> Result<&OwnerState, GuestMemoryError> {
        self.owners
            .get(&owner)
            .ok_or_else(|| GuestMemoryError::invalid_request("memory owner identifier is not live"))
    }

    fn require_region_contains(
        &self,
        owner: MemoryOwnerId,
        range: GuestRange,
    ) -> Result<Option<u64>, GuestMemoryError> {
        let found = self
            .regions
            .values()
            .find(|region| region.owner == owner && contains_range(region.range, range));
        found
            .map(|region| region.address_allocation_id)
            .ok_or_else(|| {
                GuestMemoryError::invalid_request(
                    "guest range is not fully contained in a region reserved by this owner",
                )
            })
    }

    fn require_committed_owner(
        &self,
        owner: MemoryOwnerId,
        pages: &[u32],
    ) -> Result<(), GuestMemoryError> {
        for virtual_page in pages {
            let page = self.pages.get(virtual_page).ok_or_else(|| {
                GuestMemoryError::invalid_request(format!(
                    "virtual page {virtual_page:#x} is not committed"
                ))
            })?;
            if page.owner != owner {
                return Err(GuestMemoryError::conflict(
                    "committed page belongs to a different owner",
                ));
            }
        }
        Ok(())
    }

    fn ensure_synchronized(&self) -> Result<(), GuestMemoryError> {
        if let Some(pending) = self.pending_invalidation() {
            return Err(GuestMemoryError::synchronization(
                "guest access is blocked until the pending memory delta is applied and acknowledged",
                pending,
            ));
        }
        Ok(())
    }

    fn check_page_access(
        &self,
        page: &PageState,
        fault_address: GuestAddress,
        access: MemoryAccess,
    ) -> Result<(), GuestMemoryError> {
        let denied = match access {
            MemoryAccess::Read if !page.protection.read => Some(MemoryFaultReason::ReadDenied),
            MemoryAccess::Write if !page.protection.write => Some(MemoryFaultReason::WriteDenied),
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
                fault_address,
                access,
                reason,
            )));
        }
        Ok(())
    }

    fn check_access_bounds(
        &self,
        address: GuestAddress,
        byte_length: usize,
    ) -> Result<GuestRange, GuestMemoryError> {
        if byte_length == 0 {
            return Err(GuestMemoryError::invalid_request(
                "guest access must contain at least one byte",
            ));
        }
        if byte_length > self.limits.max_access_bytes {
            return Err(GuestMemoryError::resource_limit(format!(
                "guest access requests {byte_length} bytes; limit is {}",
                self.limits.max_access_bytes
            )));
        }
        let byte_length_u64 = u64::try_from(byte_length).map_err(|_| {
            GuestMemoryError::resource_limit(
                "guest access length does not fit 64-bit range accounting",
            )
        })?;
        GuestRange::new(address, byte_length_u64)
    }

    fn validate_range_without_synchronization(
        &self,
        address: GuestAddress,
        byte_length: usize,
        access: MemoryAccess,
    ) -> Result<(), GuestMemoryError> {
        let range = self.check_access_bounds(address, byte_length)?;
        let first_page = (u64::from(address.0) / GUEST_PAGE_SIZE) as u32;
        let last_page = ((range.end() - 1) / GUEST_PAGE_SIZE) as u32;
        if first_page == last_page {
            let Some(page) = self.pages.get(&first_page) else {
                return Err(GuestMemoryError::fault(memory_fault(
                    address,
                    access,
                    MemoryFaultReason::Unmapped,
                )));
            };
            return self.check_page_access(page, address, access);
        }
        let mut expected_page = first_page;
        for (&virtual_page, page) in self.pages.range(first_page..=last_page) {
            if virtual_page != expected_page {
                let fault_address = if expected_page == first_page {
                    address
                } else {
                    GuestAddress(expected_page * GUEST_PAGE_SIZE as u32)
                };
                return Err(GuestMemoryError::fault(memory_fault(
                    fault_address,
                    access,
                    MemoryFaultReason::Unmapped,
                )));
            }
            let fault_address = if expected_page == first_page {
                address
            } else {
                GuestAddress(expected_page * GUEST_PAGE_SIZE as u32)
            };
            self.check_page_access(page, fault_address, access)?;
            expected_page += 1;
        }
        if expected_page <= last_page {
            let fault_address = if expected_page == first_page {
                address
            } else {
                GuestAddress(expected_page * GUEST_PAGE_SIZE as u32)
            };
            return Err(GuestMemoryError::fault(memory_fault(
                fault_address,
                access,
                MemoryFaultReason::Unmapped,
            )));
        }
        Ok(())
    }

    fn read_range_without_synchronization(
        &self,
        address: GuestAddress,
        byte_length: usize,
        access: MemoryAccess,
    ) -> Result<Vec<u8>, GuestMemoryError> {
        let range = self.check_access_bounds(address, byte_length)?;
        let first_page = (u64::from(address.0) / GUEST_PAGE_SIZE) as u32;
        let last_page = ((range.end() - 1) / GUEST_PAGE_SIZE) as u32;
        let first_offset = (address.0 as usize) & (GUEST_PAGE_SIZE_USIZE - 1);
        if first_page == last_page {
            let Some(page) = self.pages.get(&first_page) else {
                return Err(GuestMemoryError::fault(memory_fault(
                    address,
                    access,
                    MemoryFaultReason::Unmapped,
                )));
            };
            self.check_page_access(page, address, access)?;
            let bytes = self.frames.bytes(page.frame)?;
            return Ok(bytes[first_offset..first_offset + byte_length].to_vec());
        }
        let mut output = Vec::with_capacity(byte_length);
        let mut expected_page = first_page;
        for (&virtual_page, page) in self.pages.range(first_page..=last_page) {
            if virtual_page != expected_page {
                let fault_address = if expected_page == first_page {
                    address
                } else {
                    GuestAddress(expected_page * GUEST_PAGE_SIZE as u32)
                };
                return Err(GuestMemoryError::fault(memory_fault(
                    fault_address,
                    access,
                    MemoryFaultReason::Unmapped,
                )));
            }
            let fault_address = if expected_page == first_page {
                address
            } else {
                GuestAddress(expected_page * GUEST_PAGE_SIZE as u32)
            };
            self.check_page_access(page, fault_address, access)?;
            let bytes = self.frames.bytes(page.frame)?;
            let start = if expected_page == first_page {
                first_offset
            } else {
                0
            };
            let remaining = byte_length - output.len();
            let take = (GUEST_PAGE_SIZE_USIZE - start).min(remaining);
            output.extend_from_slice(&bytes[start..start + take]);
            expected_page += 1;
        }
        if expected_page <= last_page {
            let fault_address = if expected_page == first_page {
                address
            } else {
                GuestAddress(expected_page * GUEST_PAGE_SIZE as u32)
            };
            return Err(GuestMemoryError::fault(memory_fault(
                fault_address,
                access,
                MemoryFaultReason::Unmapped,
            )));
        }
        Ok(output)
    }

    fn preflight_cpu_writeback_page(
        &self,
        address: GuestAddress,
    ) -> Result<&PageState, GuestMemoryError> {
        if GUEST_PAGE_SIZE_USIZE > self.limits.max_access_bytes {
            return Err(GuestMemoryError::resource_limit(format!(
                "guest access requests {GUEST_PAGE_SIZE_USIZE} bytes; limit is {}",
                self.limits.max_access_bytes
            )));
        }
        let page = self.pages.get(&(address.0 >> 12)).ok_or_else(|| {
            GuestMemoryError::fault(memory_fault(
                address,
                MemoryAccess::Write,
                MemoryFaultReason::Unmapped,
            ))
        })?;
        if !page.protection.write {
            return Err(GuestMemoryError::fault(memory_fault(
                address,
                MemoryAccess::Write,
                MemoryFaultReason::WriteDenied,
            )));
        }
        Ok(page)
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
                GuestMemoryError::invalid_request(format!(
                    "initialization address {:#010x} is not committed",
                    segment.fault_address.0
                ))
            })?;
            if page.owner != owner {
                return Err(GuestMemoryError::conflict(
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
                GuestMemoryError::runtime_defect("access preflight succeeded but page disappeared")
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
            self.journal_page_before(segment.virtual_page);
            let frame = self
                .pages
                .get(&segment.virtual_page)
                .ok_or_else(|| {
                    GuestMemoryError::runtime_defect(
                        "write preflight succeeded but the page disappeared",
                    )
                })?
                .frame;
            self.journal_frame_bytes_before(frame)?;
            let page = self.pages.get_mut(&segment.virtual_page).ok_or_else(|| {
                GuestMemoryError::runtime_defect("write preflight succeeded but page disappeared")
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
            self.dirty_pages.insert(segment.virtual_page);
            input_offset = input_end;
        }
        Ok(())
    }

    fn pages_touch_executable(&self, pages: &[u32]) -> bool {
        pages.iter().any(|page| {
            self.pages
                .get(page)
                .is_some_and(|state| state.protection.execute)
        })
    }

    fn segments_touch_executable(&self, segments: &[AccessSegment]) -> bool {
        segments.iter().any(|segment| {
            self.pages
                .get(&segment.virtual_page)
                .is_some_and(|page| page.protection.execute)
        })
    }

    fn next_generation(&self) -> Result<u64, GuestMemoryError> {
        self.generation
            .checked_add(1)
            .ok_or_else(|| GuestMemoryError::runtime_defect("memory generation counter overflowed"))
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

fn read_binary_u32(bytes: &[u8], offset: usize) -> Result<u32, GuestMemoryError> {
    let end = offset.checked_add(4).ok_or_else(|| {
        GuestMemoryError::invalid_request("CPU writeback control offset overflowed host accounting")
    })?;
    let value = bytes
        .get(offset..end)
        .ok_or_else(|| GuestMemoryError::invalid_request("CPU writeback control is truncated"))?;
    Ok(u32::from_le_bytes(
        value.try_into().expect("checked four-byte control field"),
    ))
}

fn read_binary_u64(bytes: &[u8], offset: usize) -> Result<u64, GuestMemoryError> {
    let end = offset.checked_add(8).ok_or_else(|| {
        GuestMemoryError::invalid_request("CPU writeback control offset overflowed host accounting")
    })?;
    let value = bytes
        .get(offset..end)
        .ok_or_else(|| GuestMemoryError::invalid_request("CPU writeback control is truncated"))?;
    Ok(u64::from_le_bytes(
        value.try_into().expect("checked eight-byte control field"),
    ))
}

fn page_protection_flags(protection: PageProtection) -> u32 {
    u32::from(protection.read)
        | u32::from(protection.write) << 1
        | u32::from(protection.execute) << 2
}

fn memory_region_kind_code(kind: MemoryRegionKind) -> u32 {
    match kind {
        MemoryRegionKind::Image => 1,
        MemoryRegionKind::Stack => 2,
        MemoryRegionKind::ThreadEnvironmentBlock => 3,
        MemoryRegionKind::ProcessEnvironmentBlock => 4,
        MemoryRegionKind::ProcessParameters => 5,
        MemoryRegionKind::CommandLine => 6,
        MemoryRegionKind::Heap => 7,
        MemoryRegionKind::Anonymous => 8,
        MemoryRegionKind::Runtime => 9,
        MemoryRegionKind::Surface => 10,
        MemoryRegionKind::GraphicsResource => 11,
        MemoryRegionKind::AudioResource => 12,
        MemoryRegionKind::GdiResource => 13,
    }
}

fn virtual_page_address(virtual_page: u32) -> Result<GuestAddress, GuestMemoryError> {
    virtual_page
        .checked_mul(GUEST_PAGE_SIZE as u32)
        .map(GuestAddress)
        .ok_or_else(|| {
            GuestMemoryError::runtime_defect(
                "committed virtual page does not fit the 32-bit guest address space",
            )
        })
}

#[cfg(test)]
#[test]
fn transaction_journals_only_touched_pages_not_the_baseline() {
    let mut memory = GuestMemory::new(MemoryLimits {
        max_physical_pages: 128,
        max_regions: 1,
        max_owners: 1,
        max_access_bytes: GUEST_PAGE_SIZE_USIZE,
    })
    .unwrap();
    let owner = memory.create_owner(MemoryOwnerKind::Runtime).unwrap();
    let address = GuestAddress(0x2600_0000);
    let range = memory
        .reserve_fixed(
            owner,
            MemoryRegionKind::Runtime,
            address,
            128 * GUEST_PAGE_SIZE,
        )
        .unwrap();
    let protection = PageProtection {
        read: true,
        write: true,
        execute: false,
    };
    memory.commit(owner, range, protection).unwrap();
    memory.seal_owner(owner).unwrap();
    let generation = memory.pending_reflection().unwrap().invalidation.generation;
    memory.acknowledge_delta(generation).unwrap();

    let mut transaction = memory.begin_transaction();
    assert!(
        transaction
            .transaction
            .as_ref()
            .unwrap()
            .frame_images
            .is_empty()
    );
    transaction.write(address, &[1]).unwrap();
    transaction
        .write(GuestAddress(address.0 + 1), &[2])
        .unwrap();
    let journal = transaction.transaction.as_ref().unwrap();
    assert_eq!(journal.pages.len(), 1);
    assert_eq!(journal.frame_images.len(), 1);
    transaction
        .write(GuestAddress(address.0 + GUEST_PAGE_SIZE as u32), &[3])
        .unwrap();
    let journal = transaction.transaction.as_ref().unwrap();
    assert_eq!(journal.pages.len(), 2);
    assert_eq!(journal.frame_images.len(), 2);
    drop(transaction);
    assert_eq!(memory.read(address, 2).unwrap(), [0, 0]);
}

#[cfg(test)]
#[test]
fn owned_host_reads_preserve_guest_sync_owner_range_protection_and_lifetime() {
    let mut memory = GuestMemory::new(MemoryLimits::default()).unwrap();
    let address = GuestAddress(0x2400_0000);
    let owner = memory.create_owner(MemoryOwnerKind::Runtime).unwrap();
    let other = memory.create_owner(MemoryOwnerKind::Runtime).unwrap();
    let range = memory
        .reserve_fixed(owner, MemoryRegionKind::Runtime, address, 4096)
        .unwrap();
    let adjacent = memory
        .reserve_fixed(
            other,
            MemoryRegionKind::Runtime,
            GuestAddress(address.0 + 4096),
            4096,
        )
        .unwrap();
    let readable = PageProtection {
        read: true,
        write: true,
        execute: false,
    };
    memory.commit(owner, range, readable).unwrap();
    memory.commit(other, adjacent, readable).unwrap();
    memory.seal_owner(owner).unwrap();
    memory.seal_owner(other).unwrap();
    let reflection = memory.pending_reflection().unwrap();
    memory
        .acknowledge_delta(reflection.invalidation.generation)
        .unwrap();
    memory.write(address, &[1, 2, 3, 4]).unwrap();
    let before = memory.stats();
    assert_eq!(
        memory.read_owned_host(address, 4, owner).unwrap(),
        [1, 2, 3, 4]
    );
    assert_eq!(
        memory.read(address, 4).unwrap_err().class,
        GuestMemoryErrorClass::SynchronizationRequired
    );
    assert!(memory.read_owned_host(address, 4, other).is_err());
    assert!(
        memory
            .read_owned_host(GuestAddress(address.0 + 4094), 4, owner)
            .is_err()
    );
    assert_eq!(memory.stats(), before);
    memory
        .protect(
            owner,
            range,
            PageProtection {
                read: false,
                write: true,
                execute: false,
            },
        )
        .unwrap();
    let denied = memory.read_owned_host(address, 4, owner).unwrap_err();
    assert_eq!(denied.fault.unwrap().reason, MemoryFaultReason::ReadDenied);
    memory.release_region(owner, address).unwrap();
    assert!(memory.read_owned_host(address, 4, owner).is_err());
    assert_eq!(
        memory.read(address, 4).unwrap_err().class,
        GuestMemoryErrorClass::SynchronizationRequired
    );
}

#[cfg(test)]
#[test]
fn snapshots_require_the_live_owner_read_protection_and_complete_range() {
    let mut memory = GuestMemory::new(MemoryLimits::default()).unwrap();
    let owner = memory.create_owner(MemoryOwnerKind::Runtime).unwrap();
    let other = memory.create_owner(MemoryOwnerKind::Runtime).unwrap();
    let address = GuestAddress(0x20000000);
    let range = memory
        .reserve_fixed(owner, MemoryRegionKind::Runtime, address, 4096)
        .unwrap();
    memory
        .commit(
            owner,
            range,
            PageProtection {
                read: true,
                write: true,
                execute: false,
            },
        )
        .unwrap();
    let acknowledge = |memory: &mut GuestMemory| {
        let generation = memory.pending_delta().unwrap().generation;
        memory.acknowledge_delta(generation).unwrap();
    };
    memory.write(address, &[1, 2, 3, 4]).unwrap();
    assert!(memory.read_owned(address, 4, owner).is_err());
    memory.seal_owner(owner).unwrap();
    memory.seal_owner(other).unwrap();
    let reflection = memory.pending_reflection().unwrap();
    memory
        .acknowledge_delta(reflection.invalidation.generation)
        .unwrap();
    let captured = memory.read_owned(address, 4, owner).unwrap();
    memory.write(address, &[9, 9, 9, 9]).unwrap();
    acknowledge(&mut memory);
    assert_eq!(captured, [1, 2, 3, 4]);
    assert!(memory.read_owned(address, 4, other).is_err());
    assert!(
        memory
            .read_owned(GuestAddress(address.0 + 4094), 4, owner)
            .is_err()
    );
    memory
        .protect(
            owner,
            range,
            PageProtection {
                read: false,
                write: false,
                execute: false,
            },
        )
        .unwrap();
    acknowledge(&mut memory);
    assert!(memory.read_owned(address, 4, owner).is_err());
    memory.decommit(owner, range).unwrap();
    acknowledge(&mut memory);
    assert!(memory.read_owned(address, 4, owner).is_err());
}

fn require_page_aligned(range: GuestRange) -> Result<(), GuestMemoryError> {
    if !range.is_page_aligned() {
        return Err(GuestMemoryError::invalid_request(
            "guest range start and length must be 4 KiB aligned",
        ));
    }
    Ok(())
}

fn virtual_pages(range: GuestRange) -> Result<Range<u32>, GuestMemoryError> {
    require_page_aligned(range)?;
    let start = u64::from(range.start.0) / GUEST_PAGE_SIZE;
    let end = range.end() / GUEST_PAGE_SIZE;
    let start = u32::try_from(start).map_err(|_| {
        GuestMemoryError::runtime_defect("virtual page start does not fit the page index type")
    })?;
    let end = u32::try_from(end).map_err(|_| {
        GuestMemoryError::runtime_defect("virtual page end does not fit the page index type")
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
        GuestMemoryError::invalid_request(format!("{label} guest address overflows 32 bits"))
    })?;
    let range = GuestRange::new(GuestAddress(address), u64::from(mapped_size))?;
    require_page_aligned(range)?;
    if !contains_range(image_range, range) {
        return Err(GuestMemoryError::invalid_request(format!(
            "{label} mapping extends outside SizeOfImage"
        )));
    }
    for virtual_page in virtual_pages(range)? {
        if pages.insert(virtual_page, protection).is_some() {
            return Err(GuestMemoryError::conflict(format!(
                "{label} overlaps another image page mapping"
            )));
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
            "guest access must contain at least one byte",
        ));
    }
    if byte_length > max_access_bytes {
        return Err(GuestMemoryError::resource_limit(format!(
            "guest access requests {byte_length} bytes; limit is {max_access_bytes}"
        )));
    }
    let byte_length_u64 = u64::try_from(byte_length).map_err(|_| {
        GuestMemoryError::resource_limit("guest access length does not fit 64-bit range accounting")
    })?;
    let range = GuestRange::new(address, byte_length_u64)?;
    let mut current = u64::from(address.0);
    let mut segments = Vec::new();
    while current < range.end() {
        let virtual_page = u32::try_from(current / GUEST_PAGE_SIZE).map_err(|_| {
            GuestMemoryError::runtime_defect("guest access page does not fit the page index type")
        })?;
        let page_offset = usize::try_from(current % GUEST_PAGE_SIZE).map_err(|_| {
            GuestMemoryError::runtime_defect("guest access offset does not fit the host index type")
        })?;
        let available = GUEST_PAGE_SIZE_USIZE - page_offset;
        let remaining = usize::try_from(range.end() - current).map_err(|_| {
            GuestMemoryError::resource_limit(
                "guest access remainder does not fit the host index type",
            )
        })?;
        let segment_length = available.min(remaining);
        let fault_address = GuestAddress(u32::try_from(current).map_err(|_| {
            GuestMemoryError::runtime_defect("guest access address does not fit 32 bits")
        })?);
        segments.push(AccessSegment {
            virtual_page,
            page_offset,
            byte_length: segment_length,
            fault_address,
        });
        current += u64::try_from(segment_length).map_err(|_| {
            GuestMemoryError::runtime_defect("guest access segment length does not fit 64 bits")
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

fn segment_bounds_error() -> GuestMemoryError {
    GuestMemoryError::runtime_defect("access segment escaped its preflighted page or input bounds")
}

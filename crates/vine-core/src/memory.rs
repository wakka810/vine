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
    Surface,
    GraphicsResource,
    AudioResource,
    GdiResource,
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
        #[serde(skip_serializing_if = "Option::is_none")]
        address_allocation_id: Option<u64>,
        owner: MemoryOwnerId,
        kind: MemoryRegionKind,
        range: GuestRange,
    },
    Commit {
        #[serde(skip_serializing_if = "Option::is_none")]
        address_allocation_id: Option<u64>,
        owner: MemoryOwnerId,
        range: GuestRange,
        protection: PageProtection,
    },
    Protect {
        #[serde(skip_serializing_if = "Option::is_none")]
        address_allocation_id: Option<u64>,
        owner: MemoryOwnerId,
        range: GuestRange,
        protection: PageProtection,
    },
    Decommit {
        #[serde(skip_serializing_if = "Option::is_none")]
        address_allocation_id: Option<u64>,
        owner: MemoryOwnerId,
        range: GuestRange,
    },
    Release {
        #[serde(skip_serializing_if = "Option::is_none")]
        address_allocation_id: Option<u64>,
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

pub const MEMORY_DELTA_BINARY_VERSION: u32 = 2;
const MEMORY_DELTA_CONTROL_MAGIC: &[u8; 8] = b"VINEDL01";
const MEMORY_DELTA_CONTROL_HEADER_BYTES: usize = 56;
const MEMORY_DELTA_MAPPING_DESCRIPTOR_BYTES: usize = 40;
const MEMORY_DELTA_PAGE_DESCRIPTOR_BYTES: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryDeltaBuffers {
    pub control: Vec<u8>,
    pub page_bytes: Vec<u8>,
}

impl MemoryDelta {
    pub fn encode_binary(&self) -> Result<MemoryDeltaBuffers, GuestMemoryError> {
        if self.page_size != GUEST_PAGE_SIZE {
            return Err(GuestMemoryError::invalid_request(
                "memory-delta-transport",
                "memory-delta-page-size-unsupported",
                "binary memory delta requires 4 KiB pages",
            ));
        }
        let mapping_count = u32::try_from(self.mapping_operations.len()).map_err(|_| {
            GuestMemoryError::resource_limit(
                "memory-delta-transport",
                "memory-delta-mapping-count-overflow",
                "memory delta mapping count does not fit the binary control format",
            )
        })?;
        let dirty_count = u32::try_from(self.dirty_pages.len()).map_err(|_| {
            GuestMemoryError::resource_limit(
                "memory-delta-transport",
                "memory-delta-page-count-overflow",
                "memory delta page count does not fit the binary control format",
            )
        })?;
        let mapping_bytes = self
            .mapping_operations
            .len()
            .checked_mul(MEMORY_DELTA_MAPPING_DESCRIPTOR_BYTES)
            .ok_or_else(|| {
                GuestMemoryError::resource_limit(
                    "memory-delta-transport",
                    "memory-delta-control-size-overflow",
                    "memory delta mapping size overflowed host accounting",
                )
            })?;
        let page_descriptor_bytes = self
            .dirty_pages
            .len()
            .checked_mul(MEMORY_DELTA_PAGE_DESCRIPTOR_BYTES)
            .ok_or_else(|| {
                GuestMemoryError::resource_limit(
                    "memory-delta-transport",
                    "memory-delta-control-size-overflow",
                    "memory delta page descriptor size overflowed host accounting",
                )
            })?;
        let payload_bytes = self
            .dirty_pages
            .len()
            .checked_mul(GUEST_PAGE_SIZE_USIZE)
            .ok_or_else(|| {
                GuestMemoryError::resource_limit(
                    "memory-delta-transport",
                    "memory-delta-payload-size-overflow",
                    "memory delta payload size overflowed host accounting",
                )
            })?;
        let control_bytes = MEMORY_DELTA_CONTROL_HEADER_BYTES
            .checked_add(mapping_bytes)
            .and_then(|bytes| bytes.checked_add(page_descriptor_bytes))
            .ok_or_else(|| {
                GuestMemoryError::resource_limit(
                    "memory-delta-transport",
                    "memory-delta-control-size-overflow",
                    "memory delta control size overflowed host accounting",
                )
            })?;
        let mut control = Vec::with_capacity(control_bytes);
        control.extend_from_slice(MEMORY_DELTA_CONTROL_MAGIC);
        control.extend_from_slice(&MEMORY_DELTA_BINARY_VERSION.to_le_bytes());
        control.extend_from_slice(&(GUEST_PAGE_SIZE as u32).to_le_bytes());
        control.extend_from_slice(&self.from_generation.to_le_bytes());
        control.extend_from_slice(&self.generation.to_le_bytes());
        control.extend_from_slice(&mapping_count.to_le_bytes());
        control.extend_from_slice(&(MEMORY_DELTA_MAPPING_DESCRIPTOR_BYTES as u32).to_le_bytes());
        control.extend_from_slice(&dirty_count.to_le_bytes());
        control.extend_from_slice(&(MEMORY_DELTA_PAGE_DESCRIPTOR_BYTES as u32).to_le_bytes());
        let cache_flags = u32::from(self.cache_effects.flush_tlb)
            | u32::from(self.cache_effects.invalidate_instruction_fetch) << 1
            | u32::from(self.cache_effects.invalidate_generated_code) << 2;
        control.extend_from_slice(&cache_flags.to_le_bytes());
        control.extend_from_slice(&0u32.to_le_bytes());

        for operation in &self.mapping_operations {
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

        let mut page_bytes = Vec::with_capacity(payload_bytes);
        for page in &self.dirty_pages {
            if page.bytes.len() != GUEST_PAGE_SIZE_USIZE {
                return Err(GuestMemoryError::runtime_defect(
                    "memory-delta-transport",
                    "memory-delta-page-size-mismatch",
                    "memory delta contains a page with a non-4-KiB payload",
                ));
            }
            let flags = page_protection_flags(page.protection) | u32::from(page.owner_sealed) << 3;
            control.extend_from_slice(&page.address.0.to_le_bytes());
            control.extend_from_slice(&page.owner.0.to_le_bytes());
            control.extend_from_slice(&flags.to_le_bytes());
            control.extend_from_slice(&0u32.to_le_bytes());
            page_bytes.extend_from_slice(&page.bytes);
        }
        Ok(MemoryDeltaBuffers {
            control,
            page_bytes,
        })
    }
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

pub const CPU_WRITEBACK_BINARY_VERSION: u32 = 1;
const CPU_WRITEBACK_CONTROL_MAGIC: &[u8; 8] = b"VINEWB01";
const CPU_WRITEBACK_CONTROL_HEADER_BYTES: usize = 32;
const CPU_WRITEBACK_DESCRIPTOR_BYTES: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CpuWritebackPage {
    pub address: GuestAddress,
    pub owner: MemoryOwnerId,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CpuWritebackBatch {
    pub base_generation: u64,
    pub pages: Vec<CpuWritebackPage>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CpuWritebackBuffers {
    pub control: Vec<u8>,
    pub page_bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CpuWritebackReceipt {
    pub from_generation: u64,
    pub generation: u64,
    pub touched_pages: usize,
    pub changed_pages: usize,
    pub unchanged_pages: usize,
    pub executable_pages: usize,
    pub changed_bytes: u64,
    pub transferred_bytes: u64,
    pub content_sha256: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CpuWritebackFastReceipt {
    pub from_generation: u64,
    pub generation: u64,
    pub touched_pages: usize,
    pub changed_pages: usize,
    pub unchanged_pages: usize,
    pub executable_pages: usize,
    pub changed_bytes: u64,
    pub transferred_bytes: u64,
}

#[derive(Debug, Clone, Copy)]
struct CpuWritebackPageView<'a> {
    address: GuestAddress,
    owner: MemoryOwnerId,
    bytes: &'a [u8],
}

#[derive(Debug)]
struct CpuWritebackBatchView<'a> {
    base_generation: u64,
    pages: Vec<CpuWritebackPageView<'a>>,
}

impl CpuWritebackBatch {
    pub fn encode_binary(&self) -> Result<CpuWritebackBuffers, GuestMemoryError> {
        let page_count = u32::try_from(self.pages.len()).map_err(|_| {
            GuestMemoryError::resource_limit(
                "cpu-writeback-transport",
                "cpu-writeback-page-count-overflow",
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
                            "cpu-writeback-transport",
                            "cpu-writeback-control-size-overflow",
                            "CPU writeback control size overflowed host accounting",
                        )
                    })?,
            )
            .ok_or_else(|| {
                GuestMemoryError::resource_limit(
                    "cpu-writeback-transport",
                    "cpu-writeback-control-size-overflow",
                    "CPU writeback control size overflowed host accounting",
                )
            })?;
        let payload_bytes = self
            .pages
            .len()
            .checked_mul(GUEST_PAGE_SIZE_USIZE)
            .ok_or_else(|| {
                GuestMemoryError::resource_limit(
                    "cpu-writeback-transport",
                    "cpu-writeback-payload-size-overflow",
                    "CPU writeback page payload size overflowed host accounting",
                )
            })?;
        let mut control = Vec::with_capacity(control_bytes);
        control.extend_from_slice(CPU_WRITEBACK_CONTROL_MAGIC);
        control.extend_from_slice(&CPU_WRITEBACK_BINARY_VERSION.to_le_bytes());
        control.extend_from_slice(&(GUEST_PAGE_SIZE as u32).to_le_bytes());
        control.extend_from_slice(&self.base_generation.to_le_bytes());
        control.extend_from_slice(&page_count.to_le_bytes());
        control.extend_from_slice(&(CPU_WRITEBACK_DESCRIPTOR_BYTES as u32).to_le_bytes());
        let mut page_bytes = Vec::with_capacity(payload_bytes);
        for page in &self.pages {
            if !page.address.0.is_multiple_of(GUEST_PAGE_SIZE as u32) {
                return Err(GuestMemoryError::invalid_request(
                    "cpu-writeback-transport",
                    "cpu-writeback-address-not-page-aligned",
                    "CPU writeback page address must be guest-page aligned",
                ));
            }
            if page.bytes.len() != GUEST_PAGE_SIZE_USIZE {
                return Err(GuestMemoryError::invalid_request(
                    "cpu-writeback-transport",
                    "cpu-writeback-page-size-mismatch",
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

    pub fn decode_binary(
        control: &[u8],
        page_bytes: &[u8],
        max_pages: usize,
    ) -> Result<Self, GuestMemoryError> {
        let view = decode_cpu_writeback_view(control, page_bytes, max_pages)?;
        Ok(Self {
            base_generation: view.base_generation,
            pages: view
                .pages
                .into_iter()
                .map(|page| CpuWritebackPage {
                    address: page.address,
                    owner: page.owner,
                    bytes: page.bytes.to_vec(),
                })
                .collect(),
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
            "cpu-writeback-transport",
            "cpu-writeback-control-truncated",
            "CPU writeback control header is truncated",
        ));
    }
    if control.get(..8) != Some(CPU_WRITEBACK_CONTROL_MAGIC.as_slice()) {
        return Err(GuestMemoryError::invalid_request(
            "cpu-writeback-transport",
            "cpu-writeback-control-magic-mismatch",
            "CPU writeback control magic is invalid",
        ));
    }
    let version = read_binary_u32(control, 8, "cpu-writeback-control-truncated")?;
    if version != CPU_WRITEBACK_BINARY_VERSION {
        return Err(GuestMemoryError::invalid_request(
            "cpu-writeback-transport",
            "cpu-writeback-version-unsupported",
            format!("CPU writeback binary version {version} is unsupported"),
        ));
    }
    let page_size = read_binary_u32(control, 12, "cpu-writeback-control-truncated")?;
    if page_size != GUEST_PAGE_SIZE as u32 {
        return Err(GuestMemoryError::invalid_request(
            "cpu-writeback-transport",
            "cpu-writeback-page-size-unsupported",
            "CPU writeback binary page size is not 4 KiB",
        ));
    }
    let base_generation = read_binary_u64(control, 16, "cpu-writeback-control-truncated")?;
    let page_count = read_binary_u32(control, 24, "cpu-writeback-control-truncated")? as usize;
    if page_count > max_pages {
        return Err(GuestMemoryError::resource_limit(
            "cpu-writeback-transport",
            "cpu-writeback-page-limit-exceeded",
            format!("CPU writeback contains {page_count} pages; limit is {max_pages}"),
        ));
    }
    let descriptor_bytes =
        read_binary_u32(control, 28, "cpu-writeback-control-truncated")? as usize;
    if descriptor_bytes != CPU_WRITEBACK_DESCRIPTOR_BYTES {
        return Err(GuestMemoryError::invalid_request(
            "cpu-writeback-transport",
            "cpu-writeback-descriptor-size-unsupported",
            "CPU writeback descriptor size is unsupported",
        ));
    }
    let expected_control = CPU_WRITEBACK_CONTROL_HEADER_BYTES
        .checked_add(page_count.checked_mul(descriptor_bytes).ok_or_else(|| {
            GuestMemoryError::resource_limit(
                "cpu-writeback-transport",
                "cpu-writeback-control-size-overflow",
                "CPU writeback descriptor size overflowed host accounting",
            )
        })?)
        .ok_or_else(|| {
            GuestMemoryError::resource_limit(
                "cpu-writeback-transport",
                "cpu-writeback-control-size-overflow",
                "CPU writeback control size overflowed host accounting",
            )
        })?;
    if control.len() != expected_control {
        return Err(GuestMemoryError::invalid_request(
            "cpu-writeback-transport",
            "cpu-writeback-control-length-mismatch",
            "CPU writeback control length does not match its page count",
        ));
    }
    let expected_payload = page_count
        .checked_mul(GUEST_PAGE_SIZE_USIZE)
        .ok_or_else(|| {
            GuestMemoryError::resource_limit(
                "cpu-writeback-transport",
                "cpu-writeback-payload-size-overflow",
                "CPU writeback page payload size overflowed host accounting",
            )
        })?;
    if page_bytes.len() != expected_payload {
        return Err(GuestMemoryError::invalid_request(
            "cpu-writeback-transport",
            "cpu-writeback-payload-length-mismatch",
            "CPU writeback page payload length does not match its page count",
        ));
    }

    let mut pages = Vec::with_capacity(page_count);
    for index in 0..page_count {
        let descriptor = CPU_WRITEBACK_CONTROL_HEADER_BYTES + index * descriptor_bytes;
        let address = GuestAddress(read_binary_u32(
            control,
            descriptor,
            "cpu-writeback-control-truncated",
        )?);
        let owner = MemoryOwnerId(read_binary_u32(
            control,
            descriptor + 4,
            "cpu-writeback-control-truncated",
        )?);
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryTransactionStats {
    pub baseline_committed_pages: usize,
    pub baseline_committed_bytes: u64,
    pub journaled_owner_states: usize,
    pub journaled_regions: usize,
    pub journaled_page_states: usize,
    pub journaled_frame_images: usize,
    pub journaled_frame_bytes: u64,
    pub frame_allocator_operations: usize,
    pub offered_generations: usize,
    pub peak_journaled_frame_bytes: u64,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryRegionSnapshot {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address_allocation_id: Option<u64>,
    pub owner: MemoryOwnerId,
    pub kind: MemoryRegionKind,
    pub range: GuestRange,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryReflection {
    pub invalidation: MemoryInvalidation,
    pub page_size: u64,
    pub pages: Vec<MemoryReflectionPage>,
    pub content_sha256: String,
}

pub const MEMORY_REFLECTION_BINARY_VERSION: u32 = 1;
const MEMORY_REFLECTION_CONTROL_MAGIC: &[u8; 8] = b"VINERF01";
const MEMORY_REFLECTION_CONTROL_HEADER_BYTES: usize = 40;
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
                "memory-reflection-transport",
                "memory-reflection-page-size-unsupported",
                "binary memory reflection requires 4 KiB pages",
            ));
        }
        let page_count = u32::try_from(self.pages.len()).map_err(|_| {
            GuestMemoryError::resource_limit(
                "memory-reflection-transport",
                "memory-reflection-page-count-overflow",
                "memory reflection page count does not fit the binary control format",
            )
        })?;
        let descriptor_bytes = self
            .pages
            .len()
            .checked_mul(MEMORY_REFLECTION_DESCRIPTOR_BYTES)
            .ok_or_else(|| {
                GuestMemoryError::resource_limit(
                    "memory-reflection-transport",
                    "memory-reflection-control-size-overflow",
                    "memory reflection descriptor size overflowed host accounting",
                )
            })?;
        let payload_bytes = self
            .pages
            .len()
            .checked_mul(GUEST_PAGE_SIZE_USIZE)
            .ok_or_else(|| {
                GuestMemoryError::resource_limit(
                    "memory-reflection-transport",
                    "memory-reflection-payload-size-overflow",
                    "memory reflection payload size overflowed host accounting",
                )
            })?;
        let control_bytes = MEMORY_REFLECTION_CONTROL_HEADER_BYTES
            .checked_add(descriptor_bytes)
            .ok_or_else(|| {
                GuestMemoryError::resource_limit(
                    "memory-reflection-transport",
                    "memory-reflection-control-size-overflow",
                    "memory reflection control size overflowed host accounting",
                )
            })?;
        let mut control = Vec::with_capacity(control_bytes);
        control.extend_from_slice(MEMORY_REFLECTION_CONTROL_MAGIC);
        control.extend_from_slice(&MEMORY_REFLECTION_BINARY_VERSION.to_le_bytes());
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
                    "memory-reflection-transport",
                    "memory-reflection-page-size-mismatch",
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
    address_allocation_id: Option<u64>,
    range: GuestRange,
    owner: MemoryOwnerId,
    kind: MemoryRegionKind,
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

#[derive(Debug, Clone)]
struct PhysicalFrames {
    limit: usize,
    frames: Vec<Option<Box<[u8; GUEST_PAGE_SIZE_USIZE]>>>,
    free: Vec<usize>,
    in_use: usize,
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
    Release {
        frame: FrameId,
        bytes: Box<[u8; GUEST_PAGE_SIZE_USIZE]>,
    },
    Bytes {
        frame: FrameId,
        bytes: Box<[u8; GUEST_PAGE_SIZE_USIZE]>,
    },
}

#[derive(Debug)]
struct MemoryTransactionJournal {
    baseline_committed_pages: usize,
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
    offered_generations: BTreeSet<u64>,
    offered_full_reflections: BTreeSet<u64>,
    journaled_frame_bytes: u64,
    peak_journaled_frame_bytes: u64,
}

impl MemoryTransactionJournal {
    fn new(memory: &GuestMemory) -> Self {
        Self {
            baseline_committed_pages: memory.pages.len(),
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
            offered_generations: BTreeSet::new(),
            offered_full_reflections: BTreeSet::new(),
            journaled_frame_bytes: 0,
            peak_journaled_frame_bytes: 0,
        }
    }

    fn push_frame_undo(&mut self, undo: FrameUndo) {
        if matches!(undo, FrameUndo::Release { .. } | FrameUndo::Bytes { .. }) {
            self.journaled_frame_bytes += GUEST_PAGE_SIZE;
            self.peak_journaled_frame_bytes = self
                .peak_journaled_frame_bytes
                .max(self.journaled_frame_bytes);
        }
        self.frame_undo.push(undo);
    }

    fn stats(&self) -> MemoryTransactionStats {
        MemoryTransactionStats {
            baseline_committed_pages: self.baseline_committed_pages,
            baseline_committed_bytes: self.baseline_committed_pages as u64 * GUEST_PAGE_SIZE,
            journaled_owner_states: self.owners.len(),
            journaled_regions: self.regions.len(),
            journaled_page_states: self.pages.len(),
            journaled_frame_images: self
                .frame_undo
                .iter()
                .filter(|undo| matches!(undo, FrameUndo::Release { .. } | FrameUndo::Bytes { .. }))
                .count(),
            journaled_frame_bytes: self.journaled_frame_bytes,
            frame_allocator_operations: self
                .frame_undo
                .iter()
                .filter(|undo| !matches!(undo, FrameUndo::Bytes { .. }))
                .count(),
            offered_generations: self.offered_generations.len()
                + self.offered_full_reflections.len(),
            peak_journaled_frame_bytes: self.peak_journaled_frame_bytes,
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
            free: Vec::new(),
            in_use: 0,
        }
    }

    fn available(&self) -> usize {
        self.limit.saturating_sub(self.in_use)
    }

    fn allocate_zeroed(&mut self) -> Result<FrameAllocation, GuestMemoryError> {
        if self.in_use >= self.limit {
            return Err(GuestMemoryError::resource_limit(
                "physical-allocator",
                "physical-page-quota-exceeded",
                format!("physical page quota {} was reached", self.limit),
            ));
        }
        let bytes = Box::new([0; GUEST_PAGE_SIZE_USIZE]);
        let (index, source) = if let Some(index) = self.free.pop() {
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

    fn release(
        &mut self,
        frame: FrameId,
    ) -> Result<Box<[u8; GUEST_PAGE_SIZE_USIZE]>, GuestMemoryError> {
        let slot = self.frames.get_mut(frame.0).ok_or_else(|| {
            GuestMemoryError::runtime_defect(
                "physical-allocator",
                "released-frame-index-invalid",
                "released frame index is outside the allocator table",
            )
        })?;
        let bytes = slot.take().ok_or_else(|| {
            GuestMemoryError::runtime_defect(
                "physical-allocator",
                "physical-frame-double-release",
                "physical frame was released more than once",
            )
        })?;
        self.free.push(frame.0);
        self.in_use = self.in_use.checked_sub(1).ok_or_else(|| {
            GuestMemoryError::runtime_defect(
                "physical-allocator",
                "physical-page-accounting-underflow",
                "physical page usage underflowed during release",
            )
        })?;
        Ok(bytes)
    }

    fn rollback(&mut self, undo: FrameUndo) -> Result<(), GuestMemoryError> {
        match undo {
            FrameUndo::Bytes { frame, bytes } => {
                self.bytes_mut(frame)?.copy_from_slice(bytes.as_ref());
            }
            FrameUndo::Release { frame, bytes } => {
                let free = self.free.pop().ok_or_else(|| {
                    GuestMemoryError::runtime_defect(
                        "memory-transaction",
                        "rollback-free-list-empty",
                        "released frame was missing from the allocator free list",
                    )
                })?;
                if free != frame.0 {
                    return Err(GuestMemoryError::runtime_defect(
                        "memory-transaction",
                        "rollback-free-list-order-mismatch",
                        "allocator free-list order changed inside a memory transaction",
                    ));
                }
                let slot = self.frames.get_mut(frame.0).ok_or_else(|| {
                    GuestMemoryError::runtime_defect(
                        "memory-transaction",
                        "rollback-released-frame-missing",
                        "released frame index disappeared during rollback",
                    )
                })?;
                if slot.is_some() {
                    return Err(GuestMemoryError::runtime_defect(
                        "memory-transaction",
                        "rollback-released-frame-reallocated",
                        "released frame remained allocated during rollback",
                    ));
                }
                *slot = Some(bytes);
                self.in_use += 1;
            }
            FrameUndo::Allocation(allocation) => {
                let slot = self.frames.get_mut(allocation.frame.0).ok_or_else(|| {
                    GuestMemoryError::runtime_defect(
                        "memory-transaction",
                        "rollback-allocated-frame-missing",
                        "allocated frame index disappeared during rollback",
                    )
                })?;
                if slot.take().is_none() {
                    return Err(GuestMemoryError::runtime_defect(
                        "memory-transaction",
                        "rollback-allocated-frame-released",
                        "allocated frame was not live during rollback",
                    ));
                }
                self.in_use = self.in_use.checked_sub(1).ok_or_else(|| {
                    GuestMemoryError::runtime_defect(
                        "memory-transaction",
                        "rollback-physical-page-accounting-underflow",
                        "physical page usage underflowed during rollback",
                    )
                })?;
                match allocation.source {
                    FrameAllocationSource::Reused => self.free.push(allocation.frame.0),
                    FrameAllocationSource::Appended => {
                        if allocation.frame.0 + 1 != self.frames.len() {
                            return Err(GuestMemoryError::runtime_defect(
                                "memory-transaction",
                                "rollback-appended-frame-order-mismatch",
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

    pub fn transaction_stats(&self) -> MemoryTransactionStats {
        self.memory
            .transaction
            .as_ref()
            .expect("active GuestMemoryTransaction lost its journal")
            .stats()
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

        for undo in journal.frame_undo.into_iter().rev() {
            self.frames
                .rollback(undo)
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
            let bytes = Box::new(*self.frames.bytes(frame)?);
            let journal = self
                .transaction
                .as_mut()
                .expect("checked transaction journal disappeared");
            journal.frame_bytes_recorded.insert(frame.0);
            journal.push_frame_undo(FrameUndo::Bytes { frame, bytes });
        }
        Ok(())
    }

    fn allocate_frame_zeroed(&mut self) -> Result<FrameId, GuestMemoryError> {
        let allocation = self.frames.allocate_zeroed()?;
        if let Some(journal) = self.transaction.as_mut() {
            journal.allocated_frames.insert(allocation.frame.0);
            journal.push_frame_undo(FrameUndo::Allocation(allocation));
        }
        Ok(allocation.frame)
    }

    fn release_frame(&mut self, frame: FrameId) -> Result<(), GuestMemoryError> {
        let bytes = self.frames.release(frame)?;
        if let Some(journal) = self.transaction.as_mut() {
            journal.push_frame_undo(FrameUndo::Release { frame, bytes });
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

    pub(crate) fn transaction_is_active(&self) -> bool {
        self.transaction.is_some()
    }

    pub(crate) fn owner_is_sealed(&self, owner: MemoryOwnerId) -> Result<bool, GuestMemoryError> {
        Ok(self.require_owner(owner)?.sealed)
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
                "module-image-registration",
                "module-image-range-not-page-aligned",
                "registered module image range must use whole guest pages",
            ));
        }
        let owner_state = self.require_owner(owner)?;
        let MemoryOwnerKind::Image { source_sha256 } = &owner_state.kind else {
            return Err(GuestMemoryError::conflict(
                "module-image-registration",
                "module-image-owner-kind-mismatch",
                "registered module image owner is not an image owner",
            ));
        };
        if source_sha256 != &source.sha256 {
            return Err(GuestMemoryError::conflict(
                "module-image-registration",
                "module-image-owner-source-mismatch",
                "registered module source does not match the guest-memory image owner",
            ));
        }
        if expected_sealed && !owner_state.sealed {
            return Err(GuestMemoryError::conflict(
                "module-image-registration",
                "module-image-owner-unsealed",
                "registered module image owner must be sealed",
            ));
        }
        if !expected_sealed && owner_state.sealed {
            return Err(GuestMemoryError::conflict(
                "module-image-registration",
                "module-image-owner-already-sealed",
                "guest import binding requires an unsealed module image owner",
            ));
        }

        let mut owner_regions = self.regions.values().filter(|region| region.owner == owner);
        let exact_region = owner_regions.next().is_some_and(|region| {
            region.kind == MemoryRegionKind::Image && region.range == image_range
        });
        if !exact_region || owner_regions.next().is_some() {
            return Err(GuestMemoryError::conflict(
                "module-image-registration",
                "module-image-owner-range-mismatch",
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
                "module-image-registration",
                "module-image-owner-pages-mismatch",
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
        self.journal_owner_before(owner);
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
                "region-table",
                "address-allocation-id-zero",
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
        self.journal_region_before(start.0);
        self.regions.insert(
            start.0,
            MemoryRegion {
                address_allocation_id,
                range,
                owner,
                kind,
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
                "memory-owner",
                "memory-owner-still-has-mappings",
                "memory owner cannot be destroyed while regions or pages remain live",
            ));
        }
        self.journal_owner_before(owner);
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
        let address_allocation_id = self.require_region_contains(owner, range)?;
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
            match self.allocate_frame_zeroed() {
                Ok(frame) => allocated.push((virtual_page, frame)),
                Err(error) => {
                    for (_, frame) in allocated {
                        self.release_frame(frame)?;
                    }
                    return Err(error);
                }
            }
        }
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
        let address_allocation_id = self.require_region_contains(owner, range)?;
        let pages: Vec<u32> = virtual_pages(range)?.collect();
        self.require_committed_owner(owner, &pages)?;
        self.ensure_generation_available()?;
        for virtual_page in pages {
            self.journal_page_before(virtual_page);
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
                address_allocation_id,
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
        let address_allocation_id = self.require_region_contains(owner, range)?;
        let pages: Vec<u32> = virtual_pages(range)?.collect();
        self.require_committed_owner(owner, &pages)?;
        self.ensure_generation_available()?;
        for virtual_page in pages {
            self.journal_page_before(virtual_page);
            let page = self.pages.remove(&virtual_page).ok_or_else(|| {
                GuestMemoryError::runtime_defect(
                    "page-table",
                    "preflighted-page-missing",
                    "decommit preflight succeeded but page disappeared",
                )
            })?;
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
        let address_allocation_id = region.address_allocation_id;
        let pages: Vec<u32> = virtual_pages(range)?
            .filter(|page| self.pages.contains_key(page))
            .collect();
        self.require_committed_owner(owner, &pages)?;
        self.ensure_generation_available()?;
        for virtual_page in &pages {
            self.journal_page_before(*virtual_page);
            let page = self.pages.remove(virtual_page).ok_or_else(|| {
                GuestMemoryError::runtime_defect(
                    "page-table",
                    "preflighted-page-missing",
                    "release preflight succeeded but page disappeared",
                )
            })?;
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

    pub(crate) fn prepare_image_layout_at_transaction(
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
            self.journal_owner_before(owner);
            self.owners.remove(&owner);
            return Err(error);
        }

        let mut allocated = Vec::with_capacity(page_specs.len());
        for (virtual_page, protection) in page_specs {
            match self.allocate_frame_zeroed() {
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
        let mut readable_pages = 0;
        let mut writable_pages = 0;
        let mut executable_pages = 0;
        for (virtual_page, protection, frame) in allocated {
            readable_pages += usize::from(protection.read);
            writable_pages += usize::from(protection.write);
            executable_pages += usize::from(protection.execute);
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
            mapping_operations.push(MappingOperation::Commit {
                address_allocation_id: None,
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
            self.journal_page_before(segment.virtual_page);
            let frame = self
                .pages
                .get(&segment.virtual_page)
                .ok_or_else(|| {
                    GuestMemoryError::runtime_defect(
                        "page-table",
                        "preflighted-page-missing",
                        "zero-fill preflight succeeded but page disappeared",
                    )
                })?
                .frame;
            self.journal_frame_bytes_before(frame)?;
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
        self.journal_owner_before(owner);
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
        if self.transaction.is_some() {
            return Err(GuestMemoryError::conflict(
                "memory-transaction",
                "memory-delta-acknowledgement-before-transaction-commit",
                "memory deltas cannot be acknowledged before the active transaction commits",
            ));
        }
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
        self.pending_delta_with_content_hash(true)
    }

    pub fn pending_delta_binary(&mut self) -> Result<MemoryDeltaBuffers, GuestMemoryError> {
        self.pending_delta_with_content_hash(false)?.encode_binary()
    }

    fn pending_delta_with_content_hash(
        &mut self,
        include_content_hash: bool,
    ) -> Result<MemoryDelta, GuestMemoryError> {
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
        let content_sha256 = if include_content_hash {
            hash_delta_content(&mapping_operations, &dirty_pages)
        } else {
            String::new()
        };
        self.record_offered_generation(invalidation.generation, false);
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

    pub fn validate_access(
        &self,
        address: GuestAddress,
        byte_length: usize,
        access: MemoryAccess,
    ) -> Result<(), GuestMemoryError> {
        self.preflight_guest_access(address, byte_length, access)
            .map(|_| ())
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
        let changed_bytes =
            self.reconcile_reflected_page_bytes(acknowledged_generation, owner, address, bytes)?;
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

    fn reconcile_reflected_page_bytes(
        &mut self,
        acknowledged_generation: u64,
        owner: MemoryOwnerId,
        address: GuestAddress,
        bytes: &[u8],
    ) -> Result<usize, GuestMemoryError> {
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
        self.journal_frame_bytes_before(frame)?;
        self.frames.bytes_mut(frame)?.copy_from_slice(bytes);
        Ok(changed_bytes)
    }

    pub fn reconcile_cpu_writeback_binary(
        &mut self,
        control: &[u8],
        page_bytes: &[u8],
    ) -> Result<CpuWritebackReceipt, GuestMemoryError> {
        let batch = decode_cpu_writeback_view(control, page_bytes, self.limits.max_physical_pages)?;
        let (receipt, content_sha256) = self.reconcile_cpu_writeback_view(&batch, true)?;
        Ok(CpuWritebackReceipt {
            from_generation: receipt.from_generation,
            generation: receipt.generation,
            touched_pages: receipt.touched_pages,
            changed_pages: receipt.changed_pages,
            unchanged_pages: receipt.unchanged_pages,
            executable_pages: receipt.executable_pages,
            changed_bytes: receipt.changed_bytes,
            transferred_bytes: receipt.transferred_bytes,
            content_sha256: content_sha256.expect("requested CPU writeback SHA-256 is present"),
        })
    }

    pub fn reconcile_cpu_writeback_binary_fast(
        &mut self,
        control: &[u8],
        page_bytes: &[u8],
    ) -> Result<CpuWritebackFastReceipt, GuestMemoryError> {
        let batch = decode_cpu_writeback_view(control, page_bytes, self.limits.max_physical_pages)?;
        let (receipt, content_sha256) = self.reconcile_cpu_writeback_view(&batch, false)?;
        debug_assert!(content_sha256.is_none());
        Ok(receipt)
    }

    pub fn reconcile_cpu_writeback(
        &mut self,
        batch: &CpuWritebackBatch,
    ) -> Result<CpuWritebackReceipt, GuestMemoryError> {
        let pages = batch
            .pages
            .iter()
            .map(|page| CpuWritebackPageView {
                address: page.address,
                owner: page.owner,
                bytes: &page.bytes,
            })
            .collect();
        let view = CpuWritebackBatchView {
            base_generation: batch.base_generation,
            pages,
        };
        let (receipt, content_sha256) = self.reconcile_cpu_writeback_view(&view, true)?;
        Ok(CpuWritebackReceipt {
            from_generation: receipt.from_generation,
            generation: receipt.generation,
            touched_pages: receipt.touched_pages,
            changed_pages: receipt.changed_pages,
            unchanged_pages: receipt.unchanged_pages,
            executable_pages: receipt.executable_pages,
            changed_bytes: receipt.changed_bytes,
            transferred_bytes: receipt.transferred_bytes,
            content_sha256: content_sha256.expect("requested CPU writeback SHA-256 is present"),
        })
    }

    fn reconcile_cpu_writeback_view(
        &mut self,
        batch: &CpuWritebackBatchView<'_>,
        include_content_hash: bool,
    ) -> Result<(CpuWritebackFastReceipt, Option<String>), GuestMemoryError> {
        self.ensure_synchronized()?;
        if !self.initial_reflection_acknowledged {
            return Err(GuestMemoryError::invalid_request(
                "cpu-writeback",
                "initial-memory-reflection-required",
                "a full memory reflection must be acknowledged before CPU writeback",
            ));
        }
        if batch.base_generation != self.generation
            || batch.base_generation != self.acknowledged_generation
        {
            return Err(GuestMemoryError::conflict(
                "cpu-writeback",
                "cpu-writeback-generation-conflict",
                format!(
                    "CPU writeback generation {} does not match synchronized generation {}",
                    batch.base_generation, self.acknowledged_generation
                ),
            ));
        }
        if batch.pages.len() > self.limits.max_physical_pages {
            return Err(GuestMemoryError::resource_limit(
                "cpu-writeback",
                "cpu-writeback-page-limit-exceeded",
                format!(
                    "CPU writeback contains {} pages; limit is {}",
                    batch.pages.len(),
                    self.limits.max_physical_pages
                ),
            ));
        }

        let mut order = (0..batch.pages.len()).collect::<Vec<_>>();
        order.sort_unstable_by_key(|index| batch.pages[*index].address);
        for pair in order.windows(2) {
            if batch.pages[pair[0]].address == batch.pages[pair[1]].address {
                return Err(GuestMemoryError::conflict(
                    "cpu-writeback",
                    "cpu-writeback-duplicate-page",
                    "CPU writeback contains the same guest page more than once",
                ));
            }
        }

        let mut changed_order = Vec::with_capacity(batch.pages.len());
        let mut executable_pages = 0usize;
        let mut changed_bytes = 0u64;
        let mut hasher = include_content_hash.then(Sha256::new);
        if let Some(hasher) = &mut hasher {
            hasher.update(batch.base_generation.to_le_bytes());
        }
        for &index in &order {
            let writeback = &batch.pages[index];
            if !writeback.address.0.is_multiple_of(GUEST_PAGE_SIZE as u32) {
                return Err(GuestMemoryError::invalid_request(
                    "cpu-writeback",
                    "cpu-writeback-address-not-page-aligned",
                    "CPU writeback page address must be guest-page aligned",
                ));
            }
            if writeback.bytes.len() != GUEST_PAGE_SIZE_USIZE {
                return Err(GuestMemoryError::invalid_request(
                    "cpu-writeback",
                    "cpu-writeback-page-size-mismatch",
                    "CPU writeback must provide exactly one page per dirty-page descriptor",
                ));
            }
            let segments = self.preflight_guest_access_without_synchronization(
                writeback.address,
                writeback.bytes.len(),
                MemoryAccess::Write,
            )?;
            let [segment] = segments.as_slice() else {
                return Err(GuestMemoryError::runtime_defect(
                    "cpu-writeback",
                    "cpu-writeback-segment-count-invalid",
                    "one aligned CPU writeback page did not produce one access segment",
                ));
            };
            let page = self.pages.get(&segment.virtual_page).ok_or_else(|| {
                GuestMemoryError::runtime_defect(
                    "cpu-writeback",
                    "cpu-writeback-preflighted-page-missing",
                    "CPU writeback preflight succeeded but the page disappeared",
                )
            })?;
            if page.owner != writeback.owner {
                return Err(GuestMemoryError::conflict(
                    "cpu-writeback",
                    "cpu-writeback-page-owner-mismatch",
                    "CPU writeback owner does not match the mapped guest page",
                ));
            }
            if page.dirty_generation.is_some() {
                return Err(GuestMemoryError::runtime_defect(
                    "cpu-writeback",
                    "cpu-writeback-page-already-dirty",
                    "a synchronized CPU writeback page retained host dirty state",
                ));
            }
            executable_pages += usize::from(page.protection.execute);
            let page_changed_bytes = self
                .frames
                .bytes(page.frame)?
                .iter()
                .zip(writeback.bytes)
                .filter(|(current, incoming)| current != incoming)
                .count();
            if page_changed_bytes != 0 {
                changed_order.push((index, segment.virtual_page));
                changed_bytes = changed_bytes
                    .checked_add(page_changed_bytes as u64)
                    .ok_or_else(|| {
                        GuestMemoryError::resource_limit(
                            "cpu-writeback",
                            "cpu-writeback-changed-byte-count-overflow",
                            "CPU writeback changed-byte count overflowed host accounting",
                        )
                    })?;
            }
            if let Some(hasher) = &mut hasher {
                hasher.update(writeback.address.0.to_le_bytes());
                hasher.update(writeback.owner.0.to_le_bytes());
                hasher.update(writeback.bytes);
            }
        }

        let from_generation = self.generation;
        let changed_pages = changed_order.len();
        let transferred_bytes = (batch.pages.len() as u64)
            .checked_mul(GUEST_PAGE_SIZE)
            .ok_or_else(|| {
                GuestMemoryError::resource_limit(
                    "cpu-writeback",
                    "cpu-writeback-transferred-byte-count-overflow",
                    "CPU writeback transfer size overflowed host accounting",
                )
            })?;
        let generation = if changed_pages == 0 {
            from_generation
        } else {
            self.next_generation()?
        };
        if changed_pages != 0 {
            let mut transaction = self.begin_transaction();
            for &(index, virtual_page) in &changed_order {
                let writeback = &batch.pages[index];
                transaction.commit_preflighted_cpu_writeback_page(
                    virtual_page,
                    writeback.owner,
                    writeback.bytes,
                )?;
            }
            transaction.generation = generation;
            transaction.acknowledged_generation = generation;
            transaction.commit_transaction();
        }

        Ok((
            CpuWritebackFastReceipt {
                from_generation,
                generation,
                touched_pages: batch.pages.len(),
                changed_pages,
                unchanged_pages: batch.pages.len() - changed_pages,
                executable_pages,
                changed_bytes,
                transferred_bytes,
            },
            hasher.map(|hasher| hex_digest(&hasher.finalize())),
        ))
    }

    fn commit_preflighted_cpu_writeback_page(
        &mut self,
        virtual_page: u32,
        owner: MemoryOwnerId,
        bytes: &[u8],
    ) -> Result<(), GuestMemoryError> {
        let page = self.pages.get(&virtual_page).ok_or_else(|| {
            GuestMemoryError::runtime_defect(
                "cpu-writeback",
                "cpu-writeback-planned-page-missing",
                "a preflighted CPU writeback page disappeared before commit",
            )
        })?;
        if page.owner != owner || page.dirty_generation.is_some() {
            return Err(GuestMemoryError::runtime_defect(
                "cpu-writeback",
                "cpu-writeback-plan-became-stale",
                "a preflighted CPU writeback page changed before commit",
            ));
        }
        let frame = page.frame;
        self.journal_frame_bytes_before(frame)?;
        self.frames.bytes_mut(frame)?.copy_from_slice(bytes);
        Ok(())
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

    pub fn region_snapshots(&self) -> Vec<MemoryRegionSnapshot> {
        self.regions
            .values()
            .map(|region| MemoryRegionSnapshot {
                address_allocation_id: region.address_allocation_id,
                owner: region.owner,
                kind: region.kind,
                range: region.range,
            })
            .collect()
    }

    pub(crate) fn committed_page(
        &self,
        address: GuestAddress,
    ) -> Result<Option<(MemoryOwnerId, PageProtection)>, GuestMemoryError> {
        if !address.0.is_multiple_of(GUEST_PAGE_SIZE as u32) {
            return Err(GuestMemoryError::invalid_request(
                "page-table",
                "page-query-address-not-aligned",
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
        let reflection = self.build_reflection(invalidation, false)?;
        self.record_offered_generation(invalidation.generation, true);
        Ok(reflection)
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

    pub fn current_content_sha256(&self) -> Result<String, GuestMemoryError> {
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
            let protection_bits = u8::from(page.protection.read)
                | u8::from(page.protection.write) << 1
                | u8::from(page.protection.execute) << 2;
            hasher.update(address.0.to_le_bytes());
            hasher.update(page.owner.0.to_le_bytes());
            hasher.update([protection_bits, u8::from(owner.sealed), 1]);
            hasher.update(self.frames.bytes(page.frame)?);
        }
        Ok(hex_digest(&hasher.finalize()))
    }

    fn build_reflection(
        &self,
        invalidation: MemoryInvalidation,
        force_dirty: bool,
    ) -> Result<MemoryReflection, GuestMemoryError> {
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
            let dirty = force_dirty || page.dirty_generation.is_some();
            hasher.update(address.0.to_le_bytes());
            hasher.update(page.owner.0.to_le_bytes());
            hasher.update([protection_bits, u8::from(owner.sealed), u8::from(dirty)]);
            hasher.update(&bytes);
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
    ) -> Result<Option<u64>, GuestMemoryError> {
        let found = self
            .regions
            .values()
            .find(|region| region.owner == owner && contains_range(region.range, range));
        found
            .map(|region| region.address_allocation_id)
            .ok_or_else(|| {
                GuestMemoryError::invalid_request(
                    "region-table",
                    "range-not-reserved-by-owner",
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
            self.journal_page_before(segment.virtual_page);
            let frame = self
                .pages
                .get(&segment.virtual_page)
                .ok_or_else(|| {
                    GuestMemoryError::runtime_defect(
                        "page-table",
                        "preflighted-page-missing",
                        "write preflight succeeded but the page disappeared",
                    )
                })?
                .frame;
            self.journal_frame_bytes_before(frame)?;
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

fn read_binary_u32(
    bytes: &[u8],
    offset: usize,
    code: &'static str,
) -> Result<u32, GuestMemoryError> {
    let end = offset.checked_add(4).ok_or_else(|| {
        GuestMemoryError::invalid_request(
            "cpu-writeback-transport",
            code,
            "CPU writeback control offset overflowed host accounting",
        )
    })?;
    let value = bytes.get(offset..end).ok_or_else(|| {
        GuestMemoryError::invalid_request(
            "cpu-writeback-transport",
            code,
            "CPU writeback control is truncated",
        )
    })?;
    Ok(u32::from_le_bytes(
        value.try_into().expect("checked four-byte control field"),
    ))
}

fn read_binary_u64(
    bytes: &[u8],
    offset: usize,
    code: &'static str,
) -> Result<u64, GuestMemoryError> {
    let end = offset.checked_add(8).ok_or_else(|| {
        GuestMemoryError::invalid_request(
            "cpu-writeback-transport",
            code,
            "CPU writeback control offset overflowed host accounting",
        )
    })?;
    let value = bytes.get(offset..end).ok_or_else(|| {
        GuestMemoryError::invalid_request(
            "cpu-writeback-transport",
            code,
            "CPU writeback control is truncated",
        )
    })?;
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
            MappingOperation::Reserve {
                address_allocation_id,
                owner,
                kind,
                range,
            } => {
                hasher.update([0]);
                hash_address_allocation_id(&mut hasher, *address_allocation_id);
                hash_mapping_fields(&mut hasher, *owner, *range);
                hasher.update([*kind as u8]);
            }
            MappingOperation::Commit {
                address_allocation_id,
                owner,
                range,
                protection,
            } => {
                hasher.update([1]);
                hash_address_allocation_id(&mut hasher, *address_allocation_id);
                hash_mapping_fields(&mut hasher, *owner, *range);
                hash_protection(&mut hasher, *protection);
            }
            MappingOperation::Protect {
                address_allocation_id,
                owner,
                range,
                protection,
            } => {
                hasher.update([2]);
                hash_address_allocation_id(&mut hasher, *address_allocation_id);
                hash_mapping_fields(&mut hasher, *owner, *range);
                hash_protection(&mut hasher, *protection);
            }
            MappingOperation::Decommit {
                address_allocation_id,
                owner,
                range,
            } => {
                hasher.update([3]);
                hash_address_allocation_id(&mut hasher, *address_allocation_id);
                hash_mapping_fields(&mut hasher, *owner, *range);
            }
            MappingOperation::Release {
                address_allocation_id,
                owner,
                range,
            } => {
                hasher.update([4]);
                hash_address_allocation_id(&mut hasher, *address_allocation_id);
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

fn hash_address_allocation_id(hasher: &mut Sha256, address_allocation_id: Option<u64>) {
    if let Some(address_allocation_id) = address_allocation_id {
        hasher.update(b"address-allocation-id");
        hasher.update(address_allocation_id.to_le_bytes());
    }
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

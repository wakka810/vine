//! Sparse execution mirror. Guest addresses index checked mappings; they are
//! never Wasm pointers. Runtime/GuestMemory remains the authoritative owner.

use std::collections::{BTreeMap, BTreeSet};
use std::ops::{Index, IndexMut};

pub const PAGE_SIZE: usize = 4096;
pub const PAGE_COUNT: usize = 1 << 20;
pub const READ: u32 = 1;
pub const WRITE: u32 = 2;
pub const EXECUTE: u32 = 4;
pub const MAPPED: u32 = 8;
pub const SEALED: u32 = 16;
/// Derived flag: writes through this mapping can change executable code
/// through another alias, requiring an immediate generated-code exit.
pub const CODE_ALIAS: u32 = 32;
pub const ENTRY_BYTES: u32 = 16;
pub const JOURNAL_MAGIC: u32 = 0x564a4e4c;

/// The only metadata used by generated loads/stores. The table and allocations
/// are outside guest address space. `data_ptr` is valid only in wasm32 builds.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PageEntry {
    pub data_ptr: u32,
    pub flags: u32,
    pub dirty: u32,
    pub owner: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {
    Read,
    Write,
    Execute,
}

impl Access {
    fn flag(self) -> u32 {
        match self {
            Self::Read => READ,
            Self::Write => WRITE,
            Self::Execute => EXECUTE,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MemoryError {
    Invalid(&'static str),
    ResourceLimit,
    Fault {
        address: u32,
        access: Access,
        error_code: u32,
    },
}

impl std::fmt::Display for MemoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invalid(message) => f.write_str(message),
            Self::ResourceLimit => f.write_str("execution mirror page limit exceeded"),
            Self::Fault {
                address,
                access,
                error_code,
            } => write!(
                f,
                "guest {access:?} fault at {address:#010x} ({error_code:#x})"
            ),
        }
    }
}
impl std::error::Error for MemoryError {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DirtyPage {
    pub address: u32,
    pub owner: u32,
    pub bytes: Vec<u8>,
}

// The stable 16-byte header immediately precedes entry zero. Generated stores
// append page indices here only after checking journal capacity and the entire
// guest access. This metadata is never addressable through guest mappings.
struct PageTable(Box<[PageEntry]>);
impl Index<usize> for PageTable {
    type Output = PageEntry;
    fn index(&self, page: usize) -> &Self::Output {
        &self.0[page + 1]
    }
}
impl IndexMut<usize> for PageTable {
    fn index_mut(&mut self, page: usize) -> &mut Self::Output {
        &mut self.0[page + 1]
    }
}

const SMALL_ACCESS_BYTES: usize = 10;

#[derive(Clone, Copy, Default)]
struct SmallSegment {
    page: u32,
    allocation: u32,
    offset: usize,
    length: usize,
}

// An immediate, fixed-size local span, never a deferred access capability.
// Ten bytes can touch at most two pages; guest addresses still resolve through
// the mirror's owned allocations, not through unchecked Wasm pointers.
struct SmallSpan {
    segments: [SmallSegment; 2],
    count: usize,
}

pub struct MirrorMemory {
    entries: PageTable,
    journal: Box<[u32]>,
    pending_dirty: BTreeSet<u32>,
    allocations: BTreeMap<u32, Box<[u8; PAGE_SIZE]>>,
    allocation_pages: BTreeMap<u32, BTreeSet<u32>>,
    mappings: BTreeMap<u32, u32>,
    stamps: BTreeMap<u32, u64>,
    next_allocation: u32,
    maximum_pages: usize,
    authority_generation: u64,
    epoch: u64,
    // Optional code-coherence journal, bounded by the guest page space. Normal
    // data stores never enter it; generated code writes are observed at finish.
    code_changes: Option<BTreeSet<u32>>,
}

impl MirrorMemory {
    pub(crate) fn track_code_changes(&mut self, enabled: bool) {
        self.code_changes = enabled.then(BTreeSet::new);
    }
    pub(crate) fn take_code_changes(&mut self) -> BTreeSet<u32> {
        self.code_changes
            .as_mut()
            .map(std::mem::take)
            .unwrap_or_default()
    }
    pub(crate) fn code_changes_pending(&self) -> bool {
        self.code_changes
            .as_ref()
            .is_some_and(|changes| !changes.is_empty())
    }
    pub(crate) fn execution_writes_pending(&self) -> bool {
        self.entries.0[0].flags != 0
    }
    fn mapping_changed(&mut self, page: u32) {
        if let Some(changes) = &mut self.code_changes {
            changes.insert(page);
        }
    }
    pub fn new(maximum_pages: usize) -> Result<Self, MemoryError> {
        if maximum_pages == 0 || maximum_pages > PAGE_COUNT {
            return Err(MemoryError::Invalid("invalid execution mirror page limit"));
        }
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(PAGE_COUNT + 1)
            .map_err(|_| MemoryError::ResourceLimit)?;
        entries.resize(PAGE_COUNT + 1, PageEntry::default());
        let mut journal = Vec::new();
        journal
            .try_reserve_exact(maximum_pages)
            .map_err(|_| MemoryError::ResourceLimit)?;
        journal.resize(maximum_pages, 0);
        let mut journal = journal.into_boxed_slice();
        entries[0] = PageEntry {
            data_ptr: journal.as_mut_ptr() as usize as u32,
            flags: 0,
            dirty: maximum_pages as u32,
            owner: JOURNAL_MAGIC,
        };
        Ok(Self {
            entries: PageTable(entries.into_boxed_slice()),
            journal,
            pending_dirty: BTreeSet::new(),
            allocations: BTreeMap::new(),
            allocation_pages: BTreeMap::new(),
            mappings: BTreeMap::new(),
            stamps: BTreeMap::new(),
            next_allocation: 0,
            maximum_pages,
            authority_generation: 0,
            epoch: 1,
            code_changes: None,
        })
    }

    pub fn page_table_ptr(&mut self) -> usize {
        self.entries.0[1..].as_mut_ptr() as usize
    }
    pub fn maximum_pages(&self) -> usize {
        self.maximum_pages
    }
    pub fn mapped_page_count(&self) -> usize {
        self.mappings.len()
    }
    /// Read-only diagnostic projection, after normal execution completion.
    /// Unlike dirty collection this cannot finish stores or create an ACK ticket.
    /// The alias identity is a guest address, never an allocation/pool pointer.
    pub(crate) fn capture_pages(
        &self,
        dirty_only: bool,
    ) -> Result<impl Iterator<Item = (u32, PageEntry, u32)> + '_, MemoryError> {
        if self.entries.0[0].flags != 0 {
            return Err(MemoryError::Invalid(
                "capture requires completed generated writes",
            ));
        }
        let pages: Box<dyn Iterator<Item = &u32> + '_> = if dirty_only {
            Box::new(self.pending_dirty.iter())
        } else {
            Box::new(self.mappings.keys())
        };
        Ok(pages.map(move |&page| {
            let allocation = self.mappings[&page];
            let root = *self.allocation_pages[&allocation]
                .first()
                .expect("live alias");
            (page << 12, self.entries[page as usize], root << 12)
        }))
    }
    /// Sparse host transport checks must not walk a large reserved but mostly
    /// uncommitted guest range. Returned values are virtual page indices.
    pub fn mapped_page_indices_in_range(&self, base: u32, end: u64) -> Vec<u32> {
        self.mappings
            .range((base >> 12)..)
            .take_while(|(page, _)| u64::from(**page) * (PAGE_SIZE as u64) < end)
            .map(|(&page, _)| page)
            .collect()
    }
    pub fn is_empty(&self) -> bool {
        self.mappings.is_empty()
    }
    pub fn has_pending_writes(&self) -> bool {
        self.entries.0[0].flags != 0 || !self.pending_dirty.is_empty()
    }
    pub fn preflight_mapping_changes(
        &self,
        final_count: usize,
        new_allocations: usize,
        mutations: usize,
    ) -> Result<(), MemoryError> {
        if final_count > self.maximum_pages {
            return Err(MemoryError::ResourceLimit);
        }
        let allocations = u32::try_from(new_allocations).map_err(|_| MemoryError::ResourceLimit)?;
        self.next_allocation
            .checked_add(allocations)
            .ok_or(MemoryError::ResourceLimit)?;
        self.epoch
            .checked_add(mutations as u64)
            .and_then(|epoch| epoch.checked_add(1))
            .ok_or(MemoryError::ResourceLimit)?;
        Ok(())
    }
    pub fn epoch(&self) -> u64 {
        self.epoch
    }
    pub fn generation(&self) -> u64 {
        self.authority_generation
    }
    /// Call only after the corresponding Runtime reflection/delta is applied
    /// and acknowledged, or after successful authoritative CPU writeback.
    pub fn set_generation(&mut self, generation: u64) {
        self.authority_generation = generation;
    }
    pub fn entry(&self, address: u32) -> PageEntry {
        self.entries[(address >> 12) as usize]
    }
    pub fn code_stamp(&self, address: u32) -> Option<u64> {
        self.stamps.get(&(address >> 12)).copied()
    }

    /// A missing-page dependency is legitimate for a generated fetch-fault
    /// stub. Normal decoded code additionally requires execute/seal permission.
    pub fn mapping_stamp_is_current(&self, address: u32, expected: Option<u64>) -> bool {
        let page = address >> 12;
        self.code_stamp(address) == expected
            && self.mappings.get(&page).is_none_or(|allocation| {
                self.allocation_pages[allocation]
                    .iter()
                    .all(|&alias| self.entries[alias as usize].dirty != 1)
            })
    }

    /// Admission immediately before invoking cached generated code. The module
    /// records each fetched code page's stamp after discovery. Mapping/owner,
    /// protection and code bytes all invalidate that stamp; a generated store
    /// awaiting finish rejects admission even before its stamp is refreshed.
    pub fn validate_code_stamps(&self, code_pages: &[(u32, u64)]) -> bool {
        !code_pages.is_empty()
            && code_pages.iter().all(|&(address, stamp)| {
                let page = address >> 12;
                let entry = self.entries[page as usize];
                entry.flags & (MAPPED | EXECUTE | SEALED) == (MAPPED | EXECUTE | SEALED)
                    && self.mapping_stamp_is_current(address, Some(stamp))
            })
    }

    fn refresh_code_aliases(&mut self, allocation: u32) {
        let Some(pages) = self.allocation_pages.get(&allocation) else {
            return;
        };
        let executable = pages
            .iter()
            .any(|&page| self.entries[page as usize].flags & EXECUTE != 0);
        for &page in pages {
            let entry = &mut self.entries[page as usize];
            entry.flags &= !CODE_ALIAS;
            if executable {
                entry.flags |= CODE_ALIAS;
            }
        }
    }

    fn next_epoch(&self) -> Result<u64, MemoryError> {
        self.epoch.checked_add(1).ok_or(MemoryError::ResourceLimit)
    }

    fn validate_mapping(&self, address: u32, flags: u32) -> Result<u32, MemoryError> {
        if !(address as usize).is_multiple_of(PAGE_SIZE) {
            return Err(MemoryError::Invalid("guest page is unaligned"));
        }
        if flags & !(READ | WRITE | EXECUTE | MAPPED | SEALED) != 0 {
            return Err(MemoryError::Invalid("unknown execution mirror protection"));
        }
        if flags & EXECUTE != 0 && flags & SEALED == 0 {
            return Err(MemoryError::Invalid("executable page owner is unsealed"));
        }
        Ok(address >> 12)
    }

    pub fn map_page(
        &mut self,
        address: u32,
        owner: u32,
        flags: u32,
        bytes: &[u8],
    ) -> Result<(), MemoryError> {
        let page = self.validate_mapping(address, flags)?;
        if bytes.len() != PAGE_SIZE {
            return Err(MemoryError::Invalid("mirror page needs exactly 4096 bytes"));
        }
        if self.mappings.contains_key(&page) {
            return Err(MemoryError::Invalid("guest page is already mapped"));
        }
        if self.mappings.len() >= self.maximum_pages {
            return Err(MemoryError::ResourceLimit);
        }
        let allocation = self
            .next_allocation
            .checked_add(1)
            .ok_or(MemoryError::ResourceLimit)?;
        self.finish_execution()?;
        let epoch = self.next_epoch()?;
        let mut data = Box::new([0u8; PAGE_SIZE]);
        data.copy_from_slice(bytes);
        let data_ptr = data.as_mut_ptr() as usize as u32;
        self.allocations.insert(allocation, data);
        self.allocation_pages
            .insert(allocation, BTreeSet::from([page]));
        self.mappings.insert(page, allocation);
        self.entries[page as usize] = PageEntry {
            data_ptr,
            flags: flags | MAPPED,
            dirty: 0,
            owner,
        };
        self.stamps.insert(page, epoch);
        self.mapping_changed(page);
        self.next_allocation = allocation;
        self.epoch = epoch;
        self.refresh_code_aliases(allocation);
        Ok(())
    }

    /// Explicit aliases share bytes. Reflection currently supplies independent
    /// virtual pages; an adapter must not infer aliases from equal contents.
    pub fn map_alias(
        &mut self,
        address: u32,
        existing_address: u32,
        owner: u32,
        flags: u32,
    ) -> Result<(), MemoryError> {
        let page = self.validate_mapping(address, flags)?;
        if !(existing_address as usize).is_multiple_of(PAGE_SIZE) {
            return Err(MemoryError::Invalid("alias source is unaligned"));
        }
        if self.mappings.contains_key(&page) {
            return Err(MemoryError::Invalid("guest page is already mapped"));
        }
        if self.mappings.len() >= self.maximum_pages {
            return Err(MemoryError::ResourceLimit);
        }
        let allocation = *self
            .mappings
            .get(&(existing_address >> 12))
            .ok_or(MemoryError::Invalid("alias source is unmapped"))?;
        self.finish_execution()?;
        let epoch = self.next_epoch()?;
        let data_ptr = self.entries[(existing_address >> 12) as usize].data_ptr;
        self.mappings.insert(page, allocation);
        self.allocation_pages
            .get_mut(&allocation)
            .expect("live alias allocation")
            .insert(page);
        self.entries[page as usize] = PageEntry {
            data_ptr,
            flags: flags | MAPPED,
            dirty: 0,
            owner,
        };
        self.stamps.insert(page, epoch);
        self.mapping_changed(page);
        self.epoch = epoch;
        self.refresh_code_aliases(allocation);
        Ok(())
    }

    pub fn unmap_page(&mut self, address: u32) -> Result<(), MemoryError> {
        let page = self.validate_mapping(address, 0)?;
        let allocation = *self
            .mappings
            .get(&page)
            .ok_or(MemoryError::Invalid("guest page is unmapped"))?;
        self.finish_execution()?;
        let epoch = self.next_epoch()?;
        self.mappings.remove(&page);
        self.pending_dirty.remove(&page);
        self.entries[page as usize] = PageEntry::default();
        self.stamps.remove(&page);
        self.mapping_changed(page);
        let pages = self
            .allocation_pages
            .get_mut(&allocation)
            .expect("live allocation");
        pages.remove(&page);
        if pages.is_empty() {
            self.allocations.remove(&allocation);
            self.allocation_pages.remove(&allocation);
        }
        self.epoch = epoch;
        self.refresh_code_aliases(allocation);
        Ok(())
    }

    pub fn protect_page(
        &mut self,
        address: u32,
        owner: u32,
        flags: u32,
    ) -> Result<(), MemoryError> {
        let page = self.validate_mapping(address, flags)?;
        let entry = self.entries[page as usize];
        if entry.flags & MAPPED == 0 || entry.owner != owner {
            return Err(MemoryError::Invalid("mapping owner mismatch"));
        }
        self.finish_execution()?;
        let epoch = self.next_epoch()?;
        self.entries[page as usize].flags = flags | MAPPED;
        self.stamps.insert(page, epoch);
        self.mapping_changed(page);
        self.epoch = epoch;
        self.refresh_code_aliases(self.mappings[&page]);
        Ok(())
    }

    /// Replace an existing page from an acknowledged authoritative snapshot.
    /// Its owner/protection must still match; never reset a changed mapping
    /// with a stale capture. This also supports repeated benchmark inputs.
    pub fn reset_page(
        &mut self,
        address: u32,
        owner: u32,
        flags: u32,
        bytes: &[u8],
    ) -> Result<(), MemoryError> {
        let page = self.validate_mapping(address, flags)?;
        if bytes.len() != PAGE_SIZE {
            return Err(MemoryError::Invalid("mirror page needs exactly 4096 bytes"));
        }
        let entry = self.entries[page as usize];
        if entry.owner != owner || entry.flags & !CODE_ALIAS != flags | MAPPED {
            return Err(MemoryError::Invalid(
                "snapshot no longer matches mapping owner/protection",
            ));
        }
        let allocation = *self
            .mappings
            .get(&page)
            .ok_or(MemoryError::Invalid("snapshot page is unmapped"))?;
        self.finish_execution()?;
        let changed = self.allocations[&allocation].as_slice() != bytes;
        let epoch = if changed {
            self.next_epoch()?
        } else {
            self.epoch
        };
        self.allocations
            .get_mut(&allocation)
            .expect("validated allocation")
            .copy_from_slice(bytes);
        for &alias in &self.allocation_pages[&allocation] {
            self.entries[alias as usize].dirty = 0;
            self.pending_dirty.remove(&alias);
            if changed {
                self.stamps.insert(alias, epoch);
                if self.entries[alias as usize].flags & EXECUTE != 0
                    && let Some(changes) = &mut self.code_changes
                {
                    changes.insert(alias);
                }
            }
        }
        self.epoch = epoch;
        Ok(())
    }

    fn segments(
        &self,
        address: u32,
        length: usize,
        access: Access,
    ) -> Result<Vec<(u32, usize, usize)>, MemoryError> {
        let end = u64::from(address)
            .checked_add(length as u64)
            .filter(|end| *end <= 1u64 << 32)
            .ok_or(MemoryError::Invalid(
                "guest access wraps the 32-bit address space",
            ))?;
        let mut current = u64::from(address);
        let mut result = Vec::new();
        while current < end {
            let page = (current >> 12) as u32;
            let entry = self.entries[page as usize];
            if entry.flags & MAPPED == 0
                || entry.flags & access.flag() == 0
                || access == Access::Execute && entry.flags & SEALED == 0
            {
                let error_code = 4
                    | u32::from(entry.flags & MAPPED != 0)
                    | (u32::from(access == Access::Write) << 1)
                    | (u32::from(access == Access::Execute) << 4);
                return Err(MemoryError::Fault {
                    address: current as u32,
                    access,
                    error_code,
                });
            }
            let offset = (current & 4095) as usize;
            let count = ((end - current) as usize).min(PAGE_SIZE - offset);
            result.push((page, offset, count));
            current += count as u64;
        }
        Ok(result)
    }

    fn small_span(
        &self,
        address: u32,
        length: usize,
        access: Access,
    ) -> Result<SmallSpan, MemoryError> {
        if !(1..=SMALL_ACCESS_BYTES).contains(&length) {
            return Err(MemoryError::Invalid("small access needs one to ten bytes"));
        }
        let end = u64::from(address) + length as u64;
        if end > 1u64 << 32 {
            return Err(MemoryError::Invalid(
                "guest access wraps the 32-bit address space",
            ));
        }
        let mut span = SmallSpan {
            segments: [SmallSegment::default(); 2],
            count: 0,
        };
        let mut current = u64::from(address);
        while current < end {
            let page = (current >> 12) as u32;
            let entry = self.entries[page as usize];
            if entry.flags & MAPPED == 0
                || entry.flags & access.flag() == 0
                || access == Access::Execute && entry.flags & SEALED == 0
            {
                return Err(MemoryError::Fault {
                    address: current as u32,
                    access,
                    error_code: 4
                        | u32::from(entry.flags & MAPPED != 0)
                        | (u32::from(access == Access::Write) << 1)
                        | (u32::from(access == Access::Execute) << 4),
                });
            }
            let allocation = *self.mappings.get(&page).ok_or(MemoryError::Invalid(
                "small access mapping has no allocation",
            ))?;
            let data = self
                .allocations
                .get(&allocation)
                .ok_or(MemoryError::Invalid(
                    "small access allocation is no longer live",
                ))?;
            if entry.data_ptr != data.as_ptr() as usize as u32 {
                return Err(MemoryError::Invalid("small access mapping pointer changed"));
            }
            let offset = (current & 4095) as usize;
            let count = ((end - current) as usize).min(PAGE_SIZE - offset);
            span.segments[span.count] = SmallSegment {
                page,
                allocation,
                offset,
                length: count,
            };
            span.count += 1;
            current += count as u64;
        }
        Ok(span)
    }

    /// Exact short x87 operands without temporary allocations. Full-span
    /// permission checks precede the copy, including the second page.
    pub(crate) fn read_small(
        &self,
        address: u32,
        length: usize,
    ) -> Result<[u8; SMALL_ACCESS_BYTES], MemoryError> {
        let span = self.small_span(address, length, Access::Read)?;
        let mut output = [0; SMALL_ACCESS_BYTES];
        let mut cursor = 0;
        for segment in &span.segments[..span.count] {
            output[cursor..cursor + segment.length].copy_from_slice(
                &self.allocations[&segment.allocation]
                    [segment.offset..segment.offset + segment.length],
            );
            cursor += segment.length;
        }
        Ok(output)
    }

    /// Preserve operand fault priority before FP preparation. A suppressed FP
    /// store does not consume journal capacity; an actual output is admitted
    /// again by write_small before any guest byte or FP state is committed.
    pub(crate) fn preflight_small_write(
        &self,
        address: u32,
        length: usize,
    ) -> Result<(), MemoryError> {
        self.small_span(address, length, Access::Write).map(|_| ())
    }

    /// Use the same deferred dirty journal as generated stores. Existing
    /// scalar writes remain pending; finish_execution expands aliases/stamps
    /// once at the outer boundary. The result requires an immediate SMC exit.
    pub(crate) fn write_small(&mut self, address: u32, bytes: &[u8]) -> Result<bool, MemoryError> {
        let span = self.small_span(address, bytes.len(), Access::Write)?;
        let header = self.entries.0[0];
        if header.data_ptr != self.journal.as_ptr() as usize as u32
            || header.dirty != self.maximum_pages as u32
            || header.owner != JOURNAL_MAGIC
            || header.flags > header.dirty
        {
            return Err(MemoryError::Invalid(
                "invalid generated dirty journal header",
            ));
        }
        let segments = &span.segments[..span.count];
        let needed = segments
            .iter()
            .filter(|segment| self.entries[segment.page as usize].dirty != 1)
            .count();
        if needed > (header.dirty - header.flags) as usize {
            return Err(MemoryError::ResourceLimit);
        }
        // Deferred observation must still be able to publish a fresh stamp.
        // No imported helper can change mappings between this check and copy.
        self.next_epoch()?;
        let code_modified = segments
            .iter()
            .any(|segment| self.entries[segment.page as usize].flags & (EXECUTE | CODE_ALIAS) != 0);
        let mut cursor = 0;
        for segment in segments {
            self.allocations
                .get_mut(&segment.allocation)
                .expect("validated small access allocation")
                [segment.offset..segment.offset + segment.length]
                .copy_from_slice(&bytes[cursor..cursor + segment.length]);
            cursor += segment.length;
        }
        // Every operation below is infallible after the full span/capacity
        // checks. Distinct guest pages are journaled even when they alias.
        let mut journal_count = header.flags as usize;
        for segment in segments {
            let entry = &mut self.entries[segment.page as usize];
            if entry.dirty != 1 {
                self.journal[journal_count] = segment.page;
                journal_count += 1;
                entry.dirty = 1;
            }
        }
        self.entries.0[0].flags = journal_count as u32;
        Ok(code_modified)
    }

    /// Validate an entire helper access without changing bytes, dirty state or
    /// epochs. Waiting x87 exceptions are checked by the caller before this
    /// memory admission. A subsequent store still validates before committing.
    pub fn preflight_access(
        &self,
        address: u32,
        length: usize,
        access: Access,
    ) -> Result<(), MemoryError> {
        self.segments(address, length, access).map(|_| ())
    }

    pub fn read(
        &self,
        address: u32,
        length: usize,
        access: Access,
    ) -> Result<Vec<u8>, MemoryError> {
        if access == Access::Write {
            return Err(MemoryError::Invalid("read requested write permission"));
        }
        let segments = self.segments(address, length, access)?;
        let mut result = Vec::new();
        result
            .try_reserve_exact(length)
            .map_err(|_| MemoryError::ResourceLimit)?;
        for (page, offset, count) in segments {
            let data = &self.allocations[&self.mappings[&page]];
            result.extend_from_slice(&data[offset..offset + count]);
        }
        Ok(result)
    }

    pub fn write(&mut self, address: u32, bytes: &[u8]) -> Result<(), MemoryError> {
        // Validate the complete access and epoch before changing any byte.
        let segments = self.segments(address, bytes.len(), Access::Write)?;
        self.finish_execution()?;
        let epoch = self.next_epoch()?;
        let changed: BTreeSet<u32> = segments
            .iter()
            .map(|(page, _, _)| self.mappings[page])
            .collect();
        let mut cursor = 0;
        for (page, offset, count) in segments {
            self.allocations
                .get_mut(&self.mappings[&page])
                .expect("validated allocation")[offset..offset + count]
                .copy_from_slice(&bytes[cursor..cursor + count]);
            cursor += count;
        }
        self.mark_aliases_dirty(&changed, epoch);
        self.epoch = epoch;
        Ok(())
    }

    fn mark_aliases_dirty(&mut self, allocations: &BTreeSet<u32>, epoch: u64) {
        for allocation in allocations {
            for &page in &self.allocation_pages[allocation] {
                // 1 means generated code wrote since the last observation;
                // 2 remains pending authoritative writeback, stamp updated.
                self.entries[page as usize].dirty = 2;
                self.pending_dirty.insert(page);
                self.stamps.insert(page, epoch);
                if self.entries[page as usize].flags & EXECUTE != 0
                    && let Some(changes) = &mut self.code_changes
                {
                    changes.insert(page);
                }
            }
        }
    }

    /// Run after generated execution and before cache lookup or writeback.
    /// A generated store may mark only its addressed alias, so expand it here.
    /// Borrowed JS views must be reacquired after any call that grows memory.
    pub fn finish_execution(&mut self) -> Result<(), MemoryError> {
        let header = self.entries.0[0];
        if header.data_ptr != self.journal.as_ptr() as usize as u32
            || header.dirty != self.maximum_pages as u32
            || header.owner != JOURNAL_MAGIC
            || header.flags > header.dirty
        {
            return Err(MemoryError::Invalid(
                "invalid generated dirty journal header",
            ));
        }
        let mut changed = BTreeSet::new();
        let mut observed = BTreeSet::new();
        for &page in &self.journal[..header.flags as usize] {
            let allocation = self
                .mappings
                .get(&page)
                .ok_or(MemoryError::Invalid("dirty journal page is unmapped"))?;
            if self.entries[page as usize].dirty != 1 || !observed.insert(page) {
                return Err(MemoryError::Invalid(
                    "invalid or duplicate dirty journal entry",
                ));
            }
            changed.insert(*allocation);
        }
        if !changed.is_empty() {
            let epoch = self.next_epoch()?;
            self.mark_aliases_dirty(&changed, epoch);
            self.epoch = epoch;
        }
        self.entries.0[0].flags = 0;
        Ok(())
    }

    pub fn collect_dirty_pages(&mut self) -> Result<Vec<DirtyPage>, MemoryError> {
        self.finish_execution()?;
        Ok(self
            .pending_dirty
            .iter()
            .map(|&page| {
                let entry = self.entries[page as usize];
                DirtyPage {
                    address: page << 12,
                    owner: entry.owner,
                    bytes: self.allocations[&self.mappings[&page]].to_vec(),
                }
            })
            .collect())
    }

    /// Only after successful GuestMemory whole-batch reconciliation.
    pub fn acknowledge_writeback(&mut self, generation: u64) -> Result<(), MemoryError> {
        if self.entries.0[0].flags != 0 {
            return Err(MemoryError::Invalid(
                "generated writes occurred after writeback snapshot",
            ));
        }
        for &page in &self.pending_dirty {
            self.entries[page as usize].dirty = 0;
        }
        self.pending_dirty.clear();
        self.authority_generation = generation;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exhausted_generation_rejects_execution_before_any_generated_store() {
        let mut backend = crate::Backend::new(2).unwrap();
        backend
            .memory
            .map_page(0x1000, 1, READ | EXECUTE | SEALED, &[0x90; PAGE_SIZE])
            .unwrap();
        backend.compile(0x1000, 1).unwrap();
        backend.state.eip = 0x1000;
        backend.memory.epoch = u64::MAX;
        let before = backend.state.clone();
        let bytes = backend
            .memory
            .read(0x1000, PAGE_SIZE, Access::Read)
            .unwrap();
        let header = backend.memory.entries.0[0];
        assert!(
            backend
                .validate(0x1000)
                .unwrap_err()
                .contains("generation exhausted")
        );
        assert!(
            backend
                .select(0x1000)
                .unwrap_err()
                .contains("generation exhausted")
        );
        assert_eq!(backend.state, before);
        assert_eq!(
            backend
                .memory
                .read(0x1000, PAGE_SIZE, Access::Read)
                .unwrap(),
            bytes
        );
        assert_eq!(backend.memory.entries.0[0], header);
        assert!(backend.memory.pending_dirty.is_empty());
    }
    fn generated_dirty(memory: &mut MirrorMemory, page: u32) {
        if memory.entries[page as usize].dirty != 1 {
            let cursor = memory.entries.0[0].flags as usize;
            memory.journal[cursor] = page;
            memory.entries.0[0].flags += 1;
            memory.entries[page as usize].dirty = 1;
        }
    }
    #[test]
    fn capture_rejects_unfinished_generated_writes_without_finishing_them() {
        let mut memory = MirrorMemory::new(1).unwrap();
        memory
            .map_page(0x1000, 1, READ | WRITE, &[0; PAGE_SIZE])
            .unwrap();
        generated_dirty(&mut memory, 1);
        let epoch = memory.epoch();
        let entry = memory.entry(0x1000);
        assert!(memory.capture_pages(false).is_err());
        assert_eq!(memory.epoch(), epoch);
        assert_eq!(memory.entry(0x1000), entry);
        assert_eq!(memory.entries.0[0].flags, 1);
        assert!(memory.pending_dirty.is_empty());
        memory.finish_execution().unwrap();
        assert_eq!(memory.capture_pages(true).unwrap().count(), 1);
    }
    #[test]
    fn small_reads_check_complete_spans_and_do_not_address_metadata() {
        let mut memory = MirrorMemory::new(3).unwrap();
        memory
            .map_page(0x1000, 1, READ | WRITE, &[0x11; PAGE_SIZE])
            .unwrap();
        memory
            .map_page(0x2000, 2, READ, &[0x22; PAGE_SIZE])
            .unwrap();
        assert_eq!(
            memory.read_small(0x1ffc, 10).unwrap(),
            [0x11, 0x11, 0x11, 0x11, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22]
        );
        memory.protect_page(0x2000, 2, 0).unwrap();
        assert_eq!(
            memory.read_small(0x1fff, 2),
            Err(MemoryError::Fault {
                address: 0x2000,
                access: Access::Read,
                error_code: 5,
            })
        );
        assert_eq!(
            memory.read_small(0x3001, 1),
            Err(MemoryError::Fault {
                address: 0x3001,
                access: Access::Read,
                error_code: 4,
            })
        );
        assert!(memory.read_small(0x1000, 0).is_err());
        assert!(memory.read_small(0x1000, 11).is_err());
        memory
            .map_page(0xffff_f000, 3, READ | WRITE, &[0x33; PAGE_SIZE])
            .unwrap();
        assert_eq!(memory.read_small(u32::MAX, 1).unwrap()[0], 0x33);
        assert!(memory.read_small(u32::MAX, 2).is_err());
        let before = memory.read_small(u32::MAX, 1).unwrap();
        assert!(memory.write_small(u32::MAX, &[1, 2]).is_err());
        assert_eq!(memory.read_small(u32::MAX, 1).unwrap(), before);
        // A corrupted internal pointer cannot redirect a short helper access.
        memory.entries[1].data_ptr = memory.page_table_ptr() as u32;
        assert!(matches!(
            memory.read_small(0x1000, 1),
            Err(MemoryError::Invalid(_))
        ));
    }

    #[test]
    fn small_store_fault_keeps_prior_generated_writes_and_retries_once() {
        let mut memory = MirrorMemory::new(3).unwrap();
        memory
            .map_page(0x1000, 1, READ | WRITE, &[9; PAGE_SIZE])
            .unwrap();
        memory.map_page(0x2000, 2, READ, &[3; PAGE_SIZE]).unwrap();
        memory
            .map_page(0x3000, 3, READ | WRITE, &[0; PAGE_SIZE])
            .unwrap();
        // Model an earlier completed scalar store, still pending observation.
        let allocation = memory.mappings[&3];
        memory.allocations.get_mut(&allocation).unwrap()[0] = 1;
        generated_dirty(&mut memory, 3);
        let epoch = memory.epoch();
        assert_eq!(
            memory.write_small(0x1ffe, &[1, 2, 3, 4]),
            Err(MemoryError::Fault {
                address: 0x2000,
                access: Access::Write,
                error_code: 7,
            })
        );
        assert_eq!(memory.read(0x1ffe, 4, Access::Read).unwrap(), [9, 9, 3, 3]);
        assert_eq!(memory.read_small(0x3000, 1).unwrap()[0], 1);
        assert_eq!(memory.entries.0[0].flags, 1);
        assert_eq!(memory.entries[3].dirty, 1);
        assert_eq!(memory.epoch(), epoch);
        memory.protect_page(0x2000, 2, READ | WRITE).unwrap();
        assert!(!memory.write_small(0x1ffe, &[1, 2, 3, 4]).unwrap());
        assert_eq!(memory.read(0x1ffe, 4, Access::Read).unwrap(), [1, 2, 3, 4]);
        assert_eq!(memory.read_small(0x3000, 1).unwrap()[0], 1);
        assert_eq!(memory.collect_dirty_pages().unwrap().len(), 3);
    }

    #[test]
    fn small_stores_share_scalar_journal_and_expand_code_aliases_at_finish() {
        let mut memory = MirrorMemory::new(4).unwrap();
        memory
            .map_page(0x1000, 1, READ | WRITE, &[0; PAGE_SIZE])
            .unwrap();
        memory.map_alias(0x2000, 0x1000, 2, READ | WRITE).unwrap();
        memory
            .map_alias(0x8000, 0x1000, 3, READ | EXECUTE | SEALED)
            .unwrap();
        memory
            .map_page(0x3000, 4, READ | WRITE, &[0; PAGE_SIZE])
            .unwrap();
        let stamp = memory.code_stamp(0x8000);
        let epoch = memory.epoch();
        generated_dirty(&mut memory, 3);
        assert!(memory.write_small(0x1ffe, &[1, 2, 3, 4]).unwrap());
        assert_eq!(memory.entries.0[0].flags, 3);
        assert_eq!(&memory.journal[..3], &[3, 1, 2]);
        assert!(memory.write_small(0x1001, &[8]).unwrap());
        assert_eq!(memory.entries.0[0].flags, 3);
        assert_eq!(memory.epoch(), epoch);
        assert_eq!(memory.code_stamp(0x8000), stamp);
        assert!(!memory.mapping_stamp_is_current(0x8000, stamp));
        assert_eq!(memory.read(0x8ffe, 2, Access::Read).unwrap(), [1, 2]);
        assert_eq!(memory.read(0x8000, 2, Access::Read).unwrap(), [3, 8]);
        assert_eq!(memory.collect_dirty_pages().unwrap().len(), 4);
        assert_eq!(memory.epoch(), epoch + 1);
        assert_ne!(memory.code_stamp(0x8000), stamp);
        assert_eq!(memory.entries.0[0].flags, 0);
        memory.acknowledge_writeback(9).unwrap();
        assert!(!memory.has_pending_writes());
    }

    #[test]
    fn small_store_resource_and_header_failures_do_not_partially_publish() {
        let mut memory = MirrorMemory::new(2).unwrap();
        memory
            .map_page(0x1000, 1, READ | WRITE, &[9; PAGE_SIZE])
            .unwrap();
        memory
            .map_page(0x2000, 2, READ | WRITE, &[8; PAGE_SIZE])
            .unwrap();
        let epoch = memory.epoch();
        // Defensive capacity test: two distinct pages need two slots. No byte
        // or dirty publication is allowed when only one slot remains.
        memory.entries.0[0].flags = 1;
        assert_eq!(
            memory.write_small(0x1fff, &[1, 2]),
            Err(MemoryError::ResourceLimit)
        );
        assert_eq!(memory.read(0x1fff, 2, Access::Read).unwrap(), [9, 8]);
        assert_eq!(memory.entries[1].dirty, 0);
        assert_eq!(memory.entries[2].dirty, 0);
        assert_eq!(memory.entries.0[0].flags, 1);
        assert_eq!(memory.epoch(), epoch);
        memory.entries.0[0].flags = 0;
        memory.entries.0[0].owner ^= 1;
        assert!(matches!(
            memory.write_small(0x1000, &[1]),
            Err(MemoryError::Invalid(_))
        ));
        memory.entries.0[0].owner ^= 1;
        memory.epoch = u64::MAX;
        assert_eq!(
            memory.write_small(0x1000, &[1]),
            Err(MemoryError::ResourceLimit)
        );
        assert_eq!(memory.read_small(0x1000, 1).unwrap()[0], 9);
        assert_eq!(memory.entries[1].dirty, 0);
        assert_eq!(memory.entries.0[0].flags, 0);
    }

    #[test]
    fn small_x87_store_resource_failure_keeps_fp_but_suppressed_store_ignores_capacity() {
        use crate::{
            state::{CpuState, exit_kind},
            x87::{self, Descriptor, OperandKind, Operation},
        };
        let mut memory = MirrorMemory::new(1).unwrap();
        memory
            .map_page(0x1000, 1, READ | WRITE, &[0x5a; PAGE_SIZE])
            .unwrap();
        let mut cpu = CpuState::thread(0x400000, 0x8000, 0);
        cpu.x87.registers[0] = vine_x87::Ext80::ONE.words();
        cpu.x87.tag = 0xfffc;
        let descriptor = Descriptor {
            operation: Operation::Store,
            operand: OperandKind::Float32,
            register: 0,
            destination_register: false,
            pops: 1,
            quiet: false,
        }
        .encode()
        .unwrap();
        memory.entries.0[0].flags = 1;
        let before = cpu.x87.clone();
        assert_eq!(
            x87::execute(
                &mut cpu,
                &mut memory,
                descriptor,
                3,
                0x1000,
                0x400000,
                0x118
            )
            .unwrap(),
            exit_kind::RESOURCE_LIMIT
        );
        assert_eq!(cpu.x87, before);
        assert_eq!(memory.read_small(0x1000, 4).unwrap()[..4], [0x5a; 4]);
        assert_eq!(memory.entries[1].dirty, 0);
        // Empty ST0 with unmasked invalid suppresses FSTP's store and pop.
        // There is no actual output requiring a journal slot.
        cpu.x87.tag = 0xffff;
        cpu.x87.control = 0x37e;
        assert_eq!(
            x87::execute(
                &mut cpu,
                &mut memory,
                descriptor,
                3,
                0x1000,
                0x400000,
                0x118
            )
            .unwrap(),
            0
        );
        assert_ne!(
            cpu.x87.status & u32::from(vine_x87::INVALID | vine_x87::STACK_FAULT),
            0
        );
        assert_eq!(cpu.x87.top(), 0);
        assert_eq!(cpu.x87.tag, 0xffff);
        assert_eq!(memory.read_small(0x1000, 4).unwrap()[..4], [0x5a; 4]);
        assert_eq!(memory.entries[1].dirty, 0);
        assert_eq!(memory.entries.0[0].flags, 1);
    }
    #[test]
    fn journal_retains_unacknowledged_pages_and_handles_mapping_retirement() {
        let mut memory = MirrorMemory::new(3).unwrap();
        memory
            .map_page(0x1000, 1, READ | WRITE, &[0; PAGE_SIZE])
            .unwrap();
        memory
            .map_page(0x2000, 2, READ | WRITE, &[0; PAGE_SIZE])
            .unwrap();
        generated_dirty(&mut memory, 1);
        generated_dirty(&mut memory, 1);
        assert_eq!(memory.entries.0[0].flags, 1);
        assert!(memory.acknowledge_writeback(1).is_err());
        assert!(memory.has_pending_writes());
        assert_eq!(memory.collect_dirty_pages().unwrap().len(), 1);
        generated_dirty(&mut memory, 2);
        // Retiring a mapping observes pending writes before changing identity.
        memory.unmap_page(0x2000).unwrap();
        memory
            .map_page(0x2000, 3, READ | WRITE, &[7; PAGE_SIZE])
            .unwrap();
        let dirty = memory.collect_dirty_pages().unwrap();
        assert_eq!(
            dirty.iter().map(|p| p.address).collect::<Vec<_>>(),
            [0x1000]
        );
        assert_eq!(memory.entry(0x2000).owner, 3);
        memory.acknowledge_writeback(2).unwrap();
        assert!(!memory.has_pending_writes());
    }
    #[test]
    fn malformed_journal_is_rejected_without_retiring_any_pending_write() {
        let mut memory = MirrorMemory::new(3).unwrap();
        memory
            .map_page(0x1000, 1, READ | WRITE, &[0; PAGE_SIZE])
            .unwrap();
        generated_dirty(&mut memory, 1);
        memory.journal[1] = 1;
        memory.entries.0[0].flags = 2;
        let epoch = memory.epoch();
        assert!(memory.finish_execution().is_err());
        assert_eq!(memory.entries[1].dirty, 1);
        assert_eq!(memory.entries.0[0].flags, 2);
        assert_eq!(memory.epoch(), epoch);
        memory.journal[1] = u32::MAX;
        assert!(memory.finish_execution().is_err());
        assert_eq!(memory.epoch(), epoch);
        memory.entries.0[0].flags = 1;
        memory.finish_execution().unwrap();
        assert_eq!(memory.entries[1].dirty, 2);
        assert_eq!(memory.collect_dirty_pages().unwrap().len(), 1);
    }
    #[test]
    fn failing_cross_page_store_changes_no_bytes_dirty_or_generation() {
        let mut memory = MirrorMemory::new(3).unwrap();
        memory
            .map_page(0x1000, 7, READ | WRITE, &[9; PAGE_SIZE])
            .unwrap();
        memory.map_page(0x2000, 8, READ, &[3; PAGE_SIZE]).unwrap();
        let epoch = memory.epoch();
        assert_eq!(
            memory.write(0x1ffe, &[1, 2, 3, 4]),
            Err(MemoryError::Fault {
                address: 0x2000,
                access: Access::Write,
                error_code: 7,
            })
        );
        assert_eq!(memory.read(0x1ffe, 4, Access::Read).unwrap(), [9, 9, 3, 3]);
        assert!(memory.collect_dirty_pages().unwrap().is_empty());
        assert_eq!(memory.epoch(), epoch);
        assert!(memory.write(u32::MAX, &[1, 2]).is_err());
        assert!(memory.read(0, 1, Access::Read).is_err());
    }

    #[test]
    fn aliases_share_bytes_and_dirty_and_code_stamps_follow_all_aliases() {
        let mut memory = MirrorMemory::new(3).unwrap();
        memory
            .map_page(0x1000, 7, READ | WRITE | EXECUTE | SEALED, &[0; PAGE_SIZE])
            .unwrap();
        memory.map_alias(0x5000, 0x1000, 7, READ | WRITE).unwrap();
        assert_ne!(memory.entry(0x5000).flags & CODE_ALIAS, 0);
        let stamp = memory.code_stamp(0x1000);
        memory.write(0x5002, &[0xcc]).unwrap();
        assert_eq!(memory.read(0x1002, 1, Access::Execute).unwrap(), [0xcc]);
        assert_ne!(memory.code_stamp(0x1000), stamp);
        let dirty = memory.collect_dirty_pages().unwrap();
        assert_eq!(
            dirty.iter().map(|page| page.address).collect::<Vec<_>>(),
            [0x1000, 0x5000]
        );
        assert_eq!(dirty[0].bytes, dirty[1].bytes);
        memory.acknowledge_writeback(9).unwrap();
        assert_eq!(memory.generation(), 9);
        assert!(memory.collect_dirty_pages().unwrap().is_empty());
        memory.unmap_page(0x1000).unwrap();
        assert_eq!(memory.read(0x5002, 1, Access::Read).unwrap(), [0xcc]);
        assert!(memory.read(0x1002, 1, Access::Read).is_err());
        assert_eq!(memory.entry(0x5000).flags & CODE_ALIAS, 0);
    }

    #[test]
    fn mapping_owner_execute_seal_and_allocation_lifetime_are_checked() {
        let mut memory = MirrorMemory::new(1).unwrap();
        assert!(
            memory
                .map_page(0x1000, 1, EXECUTE, &[0; PAGE_SIZE])
                .is_err()
        );
        memory.map_page(0x1000, 1, READ, &[0; PAGE_SIZE]).unwrap();
        let entry = memory.entry(0x1000);
        let epoch = memory.epoch();
        assert!(memory.protect_page(0x1000, 2, READ | WRITE).is_err());
        assert!(memory.map_page(0x2000, 1, READ, &[0; PAGE_SIZE]).is_err());
        assert_eq!(memory.entry(0x1000), entry);
        assert_eq!(memory.epoch(), epoch);
        assert_eq!(std::mem::size_of::<PageEntry>(), ENTRY_BYTES as usize);
        assert_eq!(std::mem::offset_of!(PageEntry, dirty), 8);
        assert!(matches!(
            memory.read(0x1000, 1, Access::Execute),
            Err(MemoryError::Fault { error_code: 21, .. })
        ));
    }

    #[test]
    fn warm_reset_preserves_allocation_and_retires_changed_code() {
        let mut memory = MirrorMemory::new(1).unwrap();
        let flags = READ | WRITE | EXECUTE | SEALED;
        memory
            .map_page(0x1000, 7, flags, &[0x90; PAGE_SIZE])
            .unwrap();
        let pointer = memory.entry(0x1000).data_ptr;
        let stamp = memory.code_stamp(0x1000);
        memory
            .reset_page(0x1000, 7, flags, &[0x90; PAGE_SIZE])
            .unwrap();
        assert_eq!(memory.code_stamp(0x1000), stamp);
        assert!(
            memory
                .reset_page(0x1000, 8, flags, &[0xcc; PAGE_SIZE])
                .is_err()
        );
        assert_eq!(memory.read(0x1000, 1, Access::Execute).unwrap(), [0x90]);
        memory
            .reset_page(0x1000, 7, flags, &[0xcc; PAGE_SIZE])
            .unwrap();
        assert_ne!(memory.code_stamp(0x1000), stamp);
        assert_eq!(memory.entry(0x1000).data_ptr, pointer);
        assert!(memory.collect_dirty_pages().unwrap().is_empty());
    }

    #[test]
    fn generated_alias_write_blocks_admission_then_invalidates_once_until_ack() {
        let mut memory = MirrorMemory::new(2).unwrap();
        memory
            .map_page(0x1000, 7, READ | EXECUTE | SEALED, &[0x90; PAGE_SIZE])
            .unwrap();
        memory.map_alias(0x5000, 0x1000, 7, READ | WRITE).unwrap();
        let stamps = [(0x1000, memory.code_stamp(0x1000).unwrap())];
        assert!(memory.validate_code_stamps(&stamps));
        // Generated Wasm stores mark the addressed entry, without a Rust call.
        memory.entries[5].dirty = 1;
        memory.journal[0] = 5;
        memory.entries.0[0].flags = 1;
        assert!(!memory.validate_code_stamps(&stamps));
        assert_eq!(memory.collect_dirty_pages().unwrap().len(), 2);
        let observed_epoch = memory.epoch();
        assert!(!memory.validate_code_stamps(&stamps));
        assert_eq!(memory.collect_dirty_pages().unwrap().len(), 2);
        assert_eq!(memory.epoch(), observed_epoch);
        memory.acknowledge_writeback(10).unwrap();
        assert!(memory.collect_dirty_pages().unwrap().is_empty());
        assert_eq!(memory.epoch(), observed_epoch);
    }

    #[test]
    fn runtime_guest_memory_remains_authority_for_whole_batch_writeback() {
        use vine_core::{
            GuestAddress, GuestMemory, MemoryLimits, MemoryOwnerKind, MemoryRegionKind,
            PageProtection,
        };
        let mut authority = GuestMemory::new(MemoryLimits::default()).unwrap();
        let owner = authority.create_owner(MemoryOwnerKind::Anonymous).unwrap();
        let range = authority
            .reserve_fixed(
                owner,
                MemoryRegionKind::Anonymous,
                GuestAddress(0x1000),
                0x2000,
            )
            .unwrap();
        authority
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
        authority
            .write_initialization(owner, GuestAddress(0x1000), &[7])
            .unwrap();
        authority.seal_owner(owner).unwrap();
        let reflection = authority.pending_reflection().unwrap();
        let mut mirror = MirrorMemory::new(2).unwrap();
        for page in &reflection.pages {
            let flags = u32::from(page.protection.read)
                | u32::from(page.protection.write) << 1
                | u32::from(page.protection.execute) << 2
                | (u32::from(page.owner_sealed) * SEALED);
            mirror
                .map_page(page.address.0, page.owner.0, flags, &page.bytes)
                .unwrap();
        }
        let generation = reflection.invalidation.generation;
        authority.acknowledge_delta(generation).unwrap();
        mirror.set_generation(generation);
        mirror.write(0x1000, &[11]).unwrap();
        mirror.write(0x2000, &[22]).unwrap();
        assert_eq!(authority.read(GuestAddress(0x1000), 1).unwrap(), [7]);
        let pages = mirror.collect_dirty_pages().unwrap();
        let mut control = b"VINEWBCK".to_vec();
        control.extend_from_slice(&(PAGE_SIZE as u32).to_le_bytes());
        control.extend_from_slice(&generation.to_le_bytes());
        control.extend_from_slice(&(pages.len() as u32).to_le_bytes());
        control.extend_from_slice(&8u32.to_le_bytes());
        let mut payload = Vec::new();
        for page in &pages {
            control.extend_from_slice(&page.address.to_le_bytes());
            control.extend_from_slice(&page.owner.to_le_bytes());
            payload.extend_from_slice(&page.bytes);
        }
        let mut wrong_owner = control.clone();
        wrong_owner[40..44].copy_from_slice(&(owner.0 + 1).to_le_bytes());
        assert!(
            authority
                .reconcile_cpu_writeback_binary(&wrong_owner, &payload)
                .is_err()
        );
        assert_eq!(authority.read(GuestAddress(0x1000), 1).unwrap(), [7]);
        assert_eq!(authority.read(GuestAddress(0x2000), 1).unwrap(), [0]);
        assert_ne!(mirror.entry(0x1000).dirty, 0);
        let mut stale = control.clone();
        stale[12..20].copy_from_slice(&(generation - 1).to_le_bytes());
        assert!(
            authority
                .reconcile_cpu_writeback_binary(&stale, &payload)
                .is_err()
        );
        assert_eq!(authority.read(GuestAddress(0x1000), 1).unwrap(), [7]);
        let accepted = authority
            .reconcile_cpu_writeback_binary(&control, &payload)
            .unwrap();
        mirror.acknowledge_writeback(accepted).unwrap();
        assert_eq!(authority.read(GuestAddress(0x1000), 1).unwrap(), [11]);
        assert_eq!(authority.read(GuestAddress(0x2000), 1).unwrap(), [22]);
        assert_eq!(mirror.generation(), accepted);
        assert!(mirror.collect_dirty_pages().unwrap().is_empty());
    }
}

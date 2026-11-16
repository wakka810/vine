//! Runtime memory transport for the independent execution mirror.
//!
//! Windows mapping ownership remains in GuestMemory. This adapter checks its
//! received reservation identities and complete VINEDELT batch before modifying
//! execution mappings. CPU bytes must be reconciled and acknowledged first.

use crate::memory::{CODE_ALIAS, EXECUTE, MAPPED, MirrorMemory, PAGE_SIZE, SEALED};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug)]
struct Reservation {
    base: u32,
    end: u64,
    owner: u32,
    allocation: u64,
    kind: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::{Access, READ, WRITE};

    fn update(
        from: u64,
        generation: u64,
        operations: &[[u32; 10]],
        pages: &[(u32, u32, u32, u8)],
    ) -> (Vec<u8>, Vec<u8>) {
        let mut control = b"VINEDELT".to_vec();
        control.extend_from_slice(&(PAGE_SIZE as u32).to_le_bytes());
        control.extend_from_slice(&from.to_le_bytes());
        control.extend_from_slice(&generation.to_le_bytes());
        for value in [operations.len() as u32, 40, pages.len() as u32, 16, 7, 0] {
            control.extend_from_slice(&value.to_le_bytes());
        }
        for operation in operations {
            for word in operation {
                control.extend_from_slice(&word.to_le_bytes());
            }
        }
        let mut payload = Vec::new();
        for &(address, owner, flags, fill) in pages {
            for value in [address, owner, flags, 0] {
                control.extend_from_slice(&value.to_le_bytes());
            }
            payload.extend_from_slice(&[fill; PAGE_SIZE]);
        }
        (control, payload)
    }
    fn initialized() -> (MirrorMemory, RuntimeMemoryAdapter) {
        let mut memory = MirrorMemory::new(3).unwrap();
        let mut adapter = RuntimeMemoryAdapter::new();
        let (control, payload) = update(
            0,
            3,
            &[[1, 7, 8, 0, 0x1000, 0x3000, 0, 11, 0, 0]],
            &[(0x1000, 7, 11, 1), (0x2000, 7, 11, 2)],
        );
        adapter
            .apply_update(&mut memory, true, &control, &payload)
            .unwrap();
        (memory, adapter)
    }
    #[test]
    fn capture_is_read_only_across_aliases_protection_and_pending_writeback() {
        let (mut memory, mut adapter) = initialized();
        memory.map_alias(0x3000, 0x1000, 7, WRITE | SEALED).unwrap();
        memory.protect_page(0x2000, 7, SEALED).unwrap();
        memory.write(0x3000, &[9]).unwrap();
        let (control, payload) = adapter.prepare_writeback(&mut memory).unwrap();
        let epoch = memory.epoch();
        let ticket = adapter.writeback;
        let full = adapter.capture_metadata(&memory, 3, false).unwrap();
        assert_eq!(wide(&full, 16).unwrap(), 3);
        assert_eq!(word(&full, 24).unwrap(), 1);
        assert_eq!(word(&full, 32).unwrap(), 3);
        assert_eq!(word(&full, 72).unwrap(), 8); // Runtime reservation kind
        assert_eq!(wide(&full, 64).unwrap(), 11); // Runtime allocation identity
        assert_eq!(word(&full, 80 + 24 + 8).unwrap(), MAPPED | SEALED);
        assert_eq!(word(&full, 80 + 48 + 8).unwrap(), MAPPED | WRITE | SEALED);
        assert_eq!(word(&full, 80 + 48 + 16).unwrap(), 0x1000);
        assert_eq!(word(&full, 80 + 48 + 20).unwrap(), 1);
        let dirty = adapter.capture_metadata(&memory, 3, true).unwrap();
        assert_eq!(word(&dirty, 24).unwrap(), 0);
        assert_eq!(word(&dirty, 32).unwrap(), 2);
        assert_eq!(word(&dirty, 48).unwrap(), 0x1000);
        assert_eq!(word(&dirty, 72).unwrap(), 0x3000);
        assert!(adapter.capture_metadata(&memory, 2, false).is_err());
        assert!(adapter.capture_metadata(&memory, 131073, true).is_err());
        assert_eq!(memory.epoch(), epoch);
        assert_eq!(adapter.writeback, ticket);
        assert_eq!(
            adapter.prepare_writeback(&mut memory).unwrap(),
            (control, payload)
        );
        adapter.acknowledge_writeback(&mut memory, 4).unwrap();
        assert_eq!(
            word(&adapter.capture_metadata(&memory, 3, true).unwrap(), 32).unwrap(),
            0
        );
        assert_eq!(memory.read(0x1000, 1, Access::Read).unwrap(), [9]);
    }
    #[test]
    fn late_invalid_page_and_mapping_preserve_complete_mirror_batch() {
        let (mut memory, mut adapter) = initialized();
        let epoch = memory.epoch();
        let (control, payload) = update(3, 4, &[], &[(0x1000, 7, 11, 9), (0x2000, 8, 11, 9)]);
        assert!(
            adapter
                .apply_update(&mut memory, false, &control, &payload)
                .is_err()
        );
        assert_eq!(memory.read(0x1000, 1, Access::Read).unwrap(), [1]);
        assert_eq!(memory.read(0x2000, 1, Access::Read).unwrap(), [2]);
        assert_eq!(memory.epoch(), epoch);
        assert_eq!(memory.generation(), 3);
        let (control, payload) = update(
            3,
            4,
            &[
                [3, 7, 0, 1, 0x1000, 0x1000, 0, 11, 0, 0],
                [3, 7, 0, 1, 0x2000, 0x1000, 0, 12, 0, 0],
            ],
            &[],
        );
        assert!(
            adapter
                .apply_update(&mut memory, false, &control, &payload)
                .is_err()
        );
        assert_ne!(memory.entry(0x1000).flags & WRITE, 0);
        assert_eq!(memory.epoch(), epoch);
    }

    #[test]
    fn sparse_address_space_reservation_and_release_visit_only_live_pages() {
        let mut memory = MirrorMemory::new(1).unwrap();
        let mut adapter = RuntimeMemoryAdapter::new();
        let (control, payload) = update(
            0,
            1,
            &[[1, 7, 8, 0, 0x1000, 0xffff_f000, 0, 11, 0, 0]],
            &[(0x8000_0000, 7, 11, 9)],
        );
        adapter
            .apply_update(&mut memory, true, &control, &payload)
            .unwrap();
        assert!(adapter.is_initialized());
        let (control, payload) =
            update(1, 2, &[[5, 7, 0, 0, 0x1000, 0xffff_f000, 0, 11, 0, 0]], &[]);
        adapter
            .apply_update(&mut memory, false, &control, &payload)
            .unwrap();
        assert_eq!(memory.mapped_page_count(), 0);
        assert_eq!(memory.generation(), 2);
        assert!(adapter.reservations.is_empty());
    }

    #[test]
    fn excessive_mapping_work_is_rejected_before_mutating_any_page() {
        let mut memory = MirrorMemory::new(1).unwrap();
        let mut adapter = RuntimeMemoryAdapter::new();
        let (control, payload) =
            update(0, 1, &[[1, 7, 8, 0, 0x1000, 0xffff_f000, 0, 11, 0, 0]], &[]);
        adapter
            .apply_update(&mut memory, true, &control, &payload)
            .unwrap();
        let epoch = memory.epoch();
        let (control, payload) = update(
            1,
            2,
            &[
                [2, 7, 0, 3, 0x1000, 0x1000, 0, 11, 0, 0],
                [2, 7, 0, 3, 0x2000, 0xffff_e000, 0, 11, 0, 0],
            ],
            &[(0x1000, 7, 11, 9)],
        );
        let error = adapter
            .apply_update(&mut memory, false, &control, &payload)
            .unwrap_err();
        assert!(error.contains("page work exceeds mirror quota"));
        assert_eq!(memory.mapped_page_count(), 0);
        assert_eq!(memory.epoch(), epoch);
        assert_eq!(memory.generation(), 1);
        assert_eq!(adapter.reservations.len(), 1);
    }
    #[test]
    fn commit_retirement_and_address_reuse_keep_reservation_identity() {
        let (mut memory, mut adapter) = initialized();
        let (control, payload) = update(
            3,
            4,
            &[[2, 7, 0, 3, 0x3000, 0x1000, 0, 11, 0, 0]],
            &[(0x3000, 7, 11, 3)],
        );
        adapter
            .apply_update(&mut memory, false, &control, &payload)
            .unwrap();
        assert_eq!(memory.read(0x3000, 1, Access::Read).unwrap(), [3]);
        let (control, payload) = update(
            4,
            5,
            &[
                [5, 7, 0, 0, 0x1000, 0x3000, 0, 11, 0, 0],
                [1, 8, 8, 0, 0x1000, 0x1000, 0, 12, 0, 0],
                [2, 8, 0, 1, 0x1000, 0x1000, 0, 12, 0, 0],
            ],
            &[(0x1000, 8, 9, 4)],
        );
        adapter
            .apply_update(&mut memory, false, &control, &payload)
            .unwrap();
        assert_eq!(memory.entry(0x1000).owner, 8);
        assert_eq!(memory.entry(0x1000).flags & 7, READ);
        assert!(memory.read(0x2000, 1, Access::Read).is_err());
    }
    #[test]
    fn writeback_ack_is_bound_to_snapshot_and_host_changes_wait_for_it() {
        let (mut memory, mut adapter) = initialized();
        memory.write(0x1000, &[5]).unwrap();
        let (control, payload) = adapter.prepare_writeback(&mut memory).unwrap();
        assert_eq!(&control[..8], b"VINEWBCK");
        assert_eq!(payload[0], 5);
        let (delta, bytes) = update(3, 4, &[], &[(0x2000, 7, 11, 9)]);
        assert!(
            adapter
                .apply_update(&mut memory, false, &delta, &bytes)
                .is_err()
        );
        memory.write(0x1000, &[6]).unwrap();
        assert!(adapter.acknowledge_writeback(&mut memory, 4).is_err());
        assert!(memory.has_pending_writes());
        assert_eq!(memory.generation(), 3);
        let (mut memory, mut adapter) = initialized();
        memory.write(0x1000, &[5]).unwrap();
        adapter.prepare_writeback(&mut memory).unwrap();
        adapter.acknowledge_writeback(&mut memory, 4).unwrap();
        assert!(!memory.has_pending_writes());
        assert_eq!(memory.generation(), 4);
        assert!(adapter.acknowledge_writeback(&mut memory, 4).is_err());
    }
}

#[derive(Clone)]
struct Page {
    owner: u32,
    allocation: u64,
    flags: u32,
    fresh: bool,
    bytes: Option<Vec<u8>>,
}

#[derive(Default)]
pub struct RuntimeMemoryAdapter {
    reservations: BTreeMap<u32, Reservation>,
    initialized: bool,
    writeback: Option<(u64, u64)>,
}

fn word(bytes: &[u8], at: usize) -> Result<u32, String> {
    let value = bytes.get(at..at + 4).ok_or("memory control is truncated")?;
    Ok(u32::from_le_bytes(value.try_into().expect("four bytes")))
}
fn wide(bytes: &[u8], at: usize) -> Result<u64, String> {
    let value = bytes.get(at..at + 8).ok_or("memory control is truncated")?;
    Ok(u64::from_le_bytes(value.try_into().expect("eight bytes")))
}
fn containing(
    reservations: &BTreeMap<u32, Reservation>,
    address: u32,
    end: u64,
) -> Option<&Reservation> {
    reservations
        .range(..=address)
        .next_back()
        .map(|(_, r)| r)
        .filter(|r| end <= r.end)
}
fn range(control: &[u8], at: usize) -> Result<(u32, u64), String> {
    let base = word(control, at)?;
    let length = wide(control, at + 4)?;
    let end = u64::from(base)
        .checked_add(length)
        .ok_or("mapping range overflow")?;
    if !base.is_multiple_of(PAGE_SIZE as u32)
        || length == 0
        || !length.is_multiple_of(PAGE_SIZE as u64)
        || end > 1u64 << 32
    {
        return Err("mapping range is unaligned or exceeds guest addressing".into());
    }
    Ok((base, end))
}

impl RuntimeMemoryAdapter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Bounded host diagnostics only. This preserves Runtime-provided identity,
    /// protection and aliases without altering guest state, epochs or writeback.
    /// Payload bytes are copied separately through existing host page access.
    pub fn capture_metadata(
        &self,
        memory: &MirrorMemory,
        maximum_pages: usize,
        dirty_only: bool,
    ) -> Result<Vec<u8>, String> {
        if maximum_pages == 0
            || maximum_pages > 131072
            || memory.mapped_page_count() > maximum_pages
        {
            return Err("capture page bound exceeded".into());
        }
        let pages = memory
            .capture_pages(dirty_only)
            .map_err(|e| e.to_string())?;
        let count = memory
            .capture_pages(dirty_only)
            .map_err(|e| e.to_string())?
            .count();
        let reservations = if dirty_only {
            0
        } else {
            self.reservations.len()
        };
        let length = reservations
            .checked_mul(32)
            .and_then(|n| count.checked_mul(24).and_then(|p| n.checked_add(p)))
            .and_then(|n| n.checked_add(48))
            .ok_or("capture metadata length overflow")?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(length)
            .map_err(|_| "capture metadata allocation failed")?;
        bytes.extend_from_slice(b"VINECAP1");
        for value in [PAGE_SIZE as u32, 1] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes.extend_from_slice(&memory.generation().to_le_bytes());
        for value in [
            reservations as u32,
            32,
            count as u32,
            24,
            memory.maximum_pages() as u32,
            u32::from(self.initialized) | u32::from(dirty_only) << 1,
        ] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        if !dirty_only {
            for r in self.reservations.values() {
                bytes.extend_from_slice(&r.base.to_le_bytes());
                bytes.extend_from_slice(&r.owner.to_le_bytes());
                bytes.extend_from_slice(&r.end.to_le_bytes());
                bytes.extend_from_slice(&r.allocation.to_le_bytes());
                bytes.extend_from_slice(&r.kind.to_le_bytes());
                bytes.extend_from_slice(&0u32.to_le_bytes());
            }
        }
        for (address, entry, root) in pages {
            for value in [
                address,
                entry.owner,
                entry.flags,
                entry.dirty,
                root,
                u32::from(root != address),
            ] {
                bytes.extend_from_slice(&value.to_le_bytes());
            }
        }
        Ok(bytes)
    }

    pub fn prepare_writeback(
        &mut self,
        memory: &mut MirrorMemory,
    ) -> Result<(Vec<u8>, Vec<u8>), String> {
        let pages = memory.collect_dirty_pages().map_err(|e| e.to_string())?;
        let generation = memory.generation();
        let epoch = memory.epoch();
        if let Some(pending) = self.writeback
            && pending != (generation, epoch)
        {
            return Err("previous CPU writeback no longer describes live execution".into());
        }
        let mut control = b"VINEWBCK".to_vec();
        control.extend_from_slice(&(PAGE_SIZE as u32).to_le_bytes());
        control.extend_from_slice(&generation.to_le_bytes());
        control.extend_from_slice(&(pages.len() as u32).to_le_bytes());
        control.extend_from_slice(&8u32.to_le_bytes());
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(
                pages
                    .len()
                    .checked_mul(PAGE_SIZE)
                    .ok_or("writeback payload overflow")?,
            )
            .map_err(|_| "writeback payload allocation failed")?;
        for page in &pages {
            control.extend_from_slice(&page.address.to_le_bytes());
            control.extend_from_slice(&page.owner.to_le_bytes());
            bytes.extend_from_slice(&page.bytes);
        }
        if !pages.is_empty() {
            self.writeback = Some((generation, epoch));
        }
        Ok((control, bytes))
    }

    /// The caller supplies only the generation returned by successful Runtime
    /// whole-batch reconciliation. A stale or intervened snapshot cannot ACK.
    pub fn acknowledge_writeback(
        &mut self,
        memory: &mut MirrorMemory,
        generation: u64,
    ) -> Result<(), String> {
        let (base, epoch) = self.writeback.ok_or("no CPU writeback is pending")?;
        if memory.generation() != base
            || memory.epoch() != epoch
            || !(generation == base || base.checked_add(1) == Some(generation))
        {
            return Err("CPU writeback acknowledgement is stale or invalid".into());
        }
        memory
            .acknowledge_writeback(generation)
            .map_err(|e| e.to_string())?;
        self.writeback = None;
        Ok(())
    }

    fn current_page(
        &self,
        memory: &MirrorMemory,
        changes: &BTreeMap<u32, Option<Page>>,
        address: u32,
    ) -> Option<Page> {
        if let Some(page) = changes.get(&address) {
            return page.clone();
        }
        let entry = memory.entry(address);
        if entry.flags & MAPPED == 0 {
            return None;
        }
        let reservation = containing(
            &self.reservations,
            address,
            u64::from(address) + PAGE_SIZE as u64,
        )?;
        Some(Page {
            owner: entry.owner,
            allocation: reservation.allocation,
            flags: entry.flags & !(MAPPED | CODE_ALIAS),
            fresh: false,
            bytes: None,
        })
    }

    fn candidate_addresses(
        memory: &MirrorMemory,
        changes: &BTreeMap<u32, Option<Page>>,
        base: u32,
        end: u64,
    ) -> BTreeSet<u32> {
        memory
            .mapped_page_indices_in_range(base, end)
            .into_iter()
            .map(|page| page << 12)
            .chain(
                changes
                    .range(base..)
                    .take_while(|(address, _)| u64::from(**address) < end)
                    .map(|(&address, _)| address),
            )
            .collect()
    }

    /// `initial` accepts reserve operations plus the complete initial reflection
    /// page set. Later calls consume the ordinary Runtime VINEDELT wire format.
    pub fn apply_update(
        &mut self,
        memory: &mut MirrorMemory,
        initial: bool,
        control: &[u8],
        payload: &[u8],
    ) -> Result<(), String> {
        if control.get(..8) != Some(b"VINEDELT")
            || control.len() < 52
            || word(control, 8)? != PAGE_SIZE as u32
        {
            return Err("invalid Runtime memory control header".into());
        }
        if initial == self.initialized {
            return Err("initial reflection lifecycle mismatch".into());
        }
        let from = wide(control, 12)?;
        let generation = wide(control, 20)?;
        if generation <= from || from != memory.generation() || initial && from != 0 {
            return Err("Runtime memory generation mismatch".into());
        }
        if memory.has_pending_writes() || self.writeback.is_some() {
            return Err("CPU writes must be acknowledged before host memory changes".into());
        }
        let operations = word(control, 28)? as usize;
        let pages = word(control, 36)? as usize;
        if operations > 65536
            || pages > memory.maximum_pages()
            || word(control, 32)? != 40
            || word(control, 40)? != 16
            || word(control, 44)? & !7 != 0
            || word(control, 48)? != 0
        {
            return Err("invalid or excessive Runtime memory descriptors".into());
        }
        let page_base = 52usize
            .checked_add(
                operations
                    .checked_mul(40)
                    .ok_or("mapping descriptor overflow")?,
            )
            .ok_or("mapping descriptor overflow")?;
        if page_base.checked_add(pages.checked_mul(16).ok_or("page descriptor overflow")?)
            != Some(control.len())
            || pages.checked_mul(PAGE_SIZE) != Some(payload.len())
        {
            return Err("Runtime memory control/payload size mismatch".into());
        }
        if initial && memory.mapped_page_count() != 0 {
            return Err("initial mirror already has mapped pages".into());
        }
        let mut reservations = self.reservations.clone();
        let mut changes: BTreeMap<u32, Option<Page>> = BTreeMap::new();
        let maximum_page_visits = memory.maximum_pages() * 4 + operations;
        let mut page_visits = 0usize;
        let mut charge_page_visits = |count: usize| -> Result<(), String> {
            page_visits = page_visits
                .checked_add(count)
                .filter(|&total| total <= maximum_page_visits)
                .ok_or("Runtime mapping page work exceeds mirror quota")?;
            Ok(())
        };
        for index in 0..operations {
            let at = 52 + index * 40;
            let operation = word(control, at)?;
            let owner = word(control, at + 4)?;
            let kind = word(control, at + 8)?;
            let flags = word(control, at + 12)?;
            let (base, end) = range(control, at + 16)?;
            let allocation = wide(control, at + 28)?;
            if owner == 0
                || word(control, at + 36)? != 0
                || flags & !7 != 0
                || !(1..=5).contains(&operation)
                || initial && operation != 1
                || operation != 1 && kind != 0
            {
                return Err("invalid Runtime mapping operation".into());
            }
            if operation == 1 {
                if flags != 0
                    || !(1..=13).contains(&kind)
                    || reservations.len() >= 65536
                    || reservations
                        .range(..=base)
                        .next_back()
                        .is_some_and(|(_, r)| u64::from(base) < r.end)
                    || reservations
                        .range(base..)
                        .next()
                        .is_some_and(|(_, r)| u64::from(r.base) < end)
                {
                    return Err("invalid or overlapping Runtime reservation".into());
                }
                let candidates = Self::candidate_addresses(memory, &changes, base, end);
                charge_page_visits(candidates.len())?;
                for address in candidates {
                    if self.current_page(memory, &changes, address).is_some() {
                        return Err("reservation overlaps mapped page".into());
                    }
                }
                reservations.insert(
                    base,
                    Reservation {
                        base,
                        end,
                        owner,
                        allocation,
                        kind,
                    },
                );
                continue;
            }
            let reservation = containing(&reservations, base, end)
                .ok_or("operation is outside a live reservation")?;
            if reservation.owner != owner || reservation.allocation != allocation {
                return Err("Runtime reservation owner/allocation mismatch".into());
            }
            if operation == 5 && (reservation.base != base || reservation.end != end) {
                return Err("release must match its entire reservation".into());
            }
            if operation >= 4 && flags != 0 {
                return Err("retirement operation contains protection flags".into());
            }
            let addresses: Box<dyn Iterator<Item = u32>> = if operation == 5 {
                let candidates = Self::candidate_addresses(memory, &changes, base, end);
                charge_page_visits(candidates.len())?;
                Box::new(candidates.into_iter())
            } else {
                charge_page_visits(((end - u64::from(base)) / PAGE_SIZE as u64) as usize)?;
                Box::new(
                    (u64::from(base)..end)
                        .step_by(PAGE_SIZE)
                        .map(|address| address as u32),
                )
            };
            for address in addresses {
                let previous = self.current_page(memory, &changes, address);
                match operation {
                    2 => {
                        if previous.is_some() {
                            return Err("commit targets an already committed page".into());
                        }
                        changes.insert(
                            address,
                            Some(Page {
                                owner,
                                allocation,
                                flags,
                                fresh: true,
                                bytes: None,
                            }),
                        );
                    }
                    3 | 4 => {
                        let mut page =
                            previous.ok_or("mapping operation targets an absent page")?;
                        if page.owner != owner || page.allocation != allocation {
                            return Err("mapping owner/allocation mismatch".into());
                        }
                        if operation == 3 {
                            page.flags = flags | (page.flags & SEALED);
                            changes.insert(address, Some(page));
                        } else {
                            changes.insert(address, None);
                        }
                    }
                    5 => {
                        if let Some(page) = previous {
                            if page.owner != owner || page.allocation != allocation {
                                return Err("released page owner/allocation mismatch".into());
                            }
                            changes.insert(address, None);
                        }
                    }
                    _ => unreachable!(),
                }
            }
            if operation == 5 {
                reservations.remove(&base);
            }
        }
        let mut observed = BTreeSet::new();
        for index in 0..pages {
            let at = page_base + index * 16;
            let address = word(control, at)?;
            let owner = word(control, at + 4)?;
            let flags = word(control, at + 8)?;
            if !address.is_multiple_of(PAGE_SIZE as u32)
                || owner == 0
                || flags & !15 != 0
                || word(control, at + 12)? != 0
                || !observed.insert(address)
            {
                return Err("invalid or repeated Runtime page descriptor".into());
            }
            let permissions = flags & 7;
            let sealed = if flags & 8 != 0 { SEALED } else { 0 };
            if permissions & EXECUTE != 0 && sealed == 0 {
                return Err("executable Runtime page owner is unsealed".into());
            }
            let mut page = if initial {
                let reservation = containing(
                    &reservations,
                    address,
                    u64::from(address) + PAGE_SIZE as u64,
                )
                .ok_or("initial page has no reservation")?;
                if reservation.owner != owner {
                    return Err("initial page owner disagrees with reservation".into());
                }
                Page {
                    owner,
                    allocation: reservation.allocation,
                    flags: permissions | sealed,
                    fresh: true,
                    bytes: None,
                }
            } else {
                self.current_page(memory, &changes, address)
                    .ok_or("Runtime page targets absent mapping")?
            };
            if page.owner != owner || page.flags & 7 != permissions {
                return Err("Runtime page owner/protection mismatch".into());
            }
            page.flags = permissions | sealed;
            page.bytes = Some(payload[index * PAGE_SIZE..(index + 1) * PAGE_SIZE].to_vec());
            changes.insert(address, Some(page));
        }
        let mut final_count = memory.mapped_page_count();
        let mut new_allocations = 0;
        for (&address, page) in &changes {
            let existed = memory.entry(address).flags & MAPPED != 0;
            if existed && page.as_ref().is_none_or(|p| p.fresh) {
                final_count -= 1;
            }
            if let Some(page) = page {
                if page.flags & EXECUTE != 0 && page.flags & SEALED == 0 {
                    return Err("executable mapping owner is unsealed".into());
                }
                if page.fresh {
                    if page.bytes.is_none() {
                        return Err("new committed page lacks authoritative bytes".into());
                    }
                    final_count += 1;
                    new_allocations += 1;
                }
            }
        }
        memory
            .preflight_mapping_changes(
                final_count,
                new_allocations,
                changes.len().saturating_mul(3),
            )
            .map_err(|e| e.to_string())?;
        // No fallible validation remains. Remove retired mappings first so
        // address reuse does not transiently consume another page quota.
        for (&address, page) in &changes {
            if memory.entry(address).flags & MAPPED != 0 && page.as_ref().is_none_or(|p| p.fresh) {
                memory
                    .unmap_page(address)
                    .expect("validated mapping retirement");
            }
        }
        for (&address, page) in &changes {
            let Some(page) = page else {
                continue;
            };
            if page.fresh {
                memory
                    .map_page(
                        address,
                        page.owner,
                        page.flags,
                        page.bytes.as_deref().expect("new page payload"),
                    )
                    .expect("validated mapping install");
            } else {
                if memory.entry(address).flags & !(MAPPED | CODE_ALIAS) != page.flags {
                    memory
                        .protect_page(address, page.owner, page.flags)
                        .expect("validated mapping protection");
                }
                if let Some(bytes) = &page.bytes {
                    memory
                        .reset_page(address, page.owner, page.flags, bytes)
                        .expect("validated Runtime page bytes");
                }
            }
        }
        memory.set_generation(generation);
        self.reservations = reservations;
        self.initialized = true;
        Ok(())
    }
}

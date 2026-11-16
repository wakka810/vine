//! Bounded compiled regions. Rust owns identity, admission and eviction; the
//! browser owns only Wasm instances indexed by `(backend handle, unit id)`.
use crate::memory::{EXECUTE, MAPPED, MirrorMemory, SEALED};
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
#[cfg(test)]
mod coherence_tests;
mod direct;
mod validation;
use validation::CachedUnit;

pub const RETIRE_ALL: u32 = u32::MAX;
pub const DEFAULT_MAXIMUM_UNITS: usize = 64;
pub const DEFAULT_MAXIMUM_CODE_BYTES: usize = 16 * 1024 * 1024;
pub const MAXIMUM_UNITS: usize = 512;
pub const MAXIMUM_CODE_BYTES: usize = DEFAULT_MAXIMUM_CODE_BYTES;
pub const MAXIMUM_PREPARED_UNITS: usize = 4096;
pub const MAXIMUM_PREPARED_BYTES: usize = 128 * 1024 * 1024;
pub const MAXIMUM_LINK_UNITS: usize = MAXIMUM_UNITS + MAXIMUM_PREPARED_UNITS;
#[derive(Clone, Copy)]
enum RetirementReason {
    Explicit = 0,
    Capacity = 1,
    Dependency = 2,
}
/// Exact admitted PCs own their invocation proof. Its lifetime follows the
/// existing resident-entry index, without an independently evicting memo.
#[derive(Default)]
struct Entry {
    units: BTreeSet<u32>,
    invocation: u64,
    binding: u64,
}

pub struct Unit {
    pub code: Box<[u8]>,
    pub entries: BTreeSet<u32>,
    pub dependencies: BTreeMap<u32, Option<u64>>,
    pub instruction_count: u32,
    /// Exact decoded suffix lengths, excluding block heads. A budget resume
    /// may use these to avoid rediscovering the enclosing region. They are
    /// compiler metadata, never guest pointers or external entry permissions.
    pub interior_limits: BTreeMap<u32, usize>,
    /// Known successors outside this unit's admitted entries. Hints only;
    /// preparation always rechecks the current code/mapping identity.
    pub preparation_frontier: Vec<u32>,
}
impl Unit {
    fn is_current(&self, memory: &MirrorMemory) -> bool {
        self.dependencies
            .iter()
            .all(|(&page, &stamp)| memory.mapping_stamp_is_current(page, stamp))
    }
    fn permits_invocation_proof(&self, memory: &MirrorMemory) -> bool {
        self.dependencies.iter().all(|(&address, stamp)| {
            // A present NX/unsealed fetch-fault dependency may be written by
            // ordinary stores without an SMC exit. Never retain that proof.
            stamp.is_none()
                || memory.entry(address).flags & (MAPPED | EXECUTE | SEALED)
                    == (MAPPED | EXECUTE | SEALED)
        })
    }
}

pub struct CodeCache {
    units: BTreeMap<u32, CachedUnit>,
    by_entry: HashMap<u32, Entry>,
    /// Shared dependency index for continuation lookup and optional eager
    /// invalidation. Pages narrow candidates; only exact decoded PCs and live
    /// stamps may provide a suffix bound or execution admission.
    by_page: BTreeMap<u32, BTreeSet<u32>>,
    recent: VecDeque<u32>,
    retired: VecDeque<u32>,
    retirement_counts: [u64; 3],
    selected: Option<u32>,
    next_id: u32,
    bytes: usize,
    prepared: BTreeSet<u32>,
    prepared_bytes: usize,
    prepared_capacity: usize,
    prepared_byte_limit: usize,
    maximum_units: usize,
    maximum_bytes: usize,
    slots: Box<[u32]>,
    by_slot: BTreeMap<u32, u32>,
    ready: BTreeSet<u32>,
    invocation_epoch: Option<u64>,
    invocation_id: u64,
    memo_hits: u32,
    memo_misses: u32,
    direct: Option<direct::DirectLinks>,
}
impl Default for CodeCache {
    fn default() -> Self {
        Self::new(DEFAULT_MAXIMUM_UNITS, DEFAULT_MAXIMUM_CODE_BYTES)
    }
}
impl CodeCache {
    fn new(maximum_units: usize, maximum_bytes: usize) -> Self {
        assert!(
            maximum_units > 0
                && maximum_units <= MAXIMUM_UNITS
                && maximum_bytes > 0
                && maximum_bytes <= MAXIMUM_CODE_BYTES
        );
        Self {
            units: BTreeMap::new(),
            by_entry: HashMap::new(),
            by_page: BTreeMap::new(),
            recent: VecDeque::new(),
            retired: VecDeque::new(),
            retirement_counts: [0; 3],
            selected: None,
            next_id: 0,
            bytes: 0,
            prepared: BTreeSet::new(),
            prepared_bytes: 0,
            prepared_capacity: 0,
            prepared_byte_limit: 0,
            maximum_units,
            maximum_bytes,
            slots: vec![0; maximum_units].into_boxed_slice(),
            by_slot: BTreeMap::new(),
            ready: BTreeSet::new(),
            invocation_epoch: None,
            invocation_id: 0,
            memo_hits: 0,
            memo_misses: 0,
            direct: None,
        }
    }
    pub fn selected_id(&self) -> u32 {
        self.selected.unwrap_or(0)
    }
    pub fn has_prepared_entry(&self, entry: u32, memory: &MirrorMemory) -> bool {
        self.by_entry.get(&entry).is_some_and(|indexed| {
            indexed
                .units
                .iter()
                .any(|id| self.prepared.contains(id) && self.units[id].is_current(memory))
        })
    }
    pub(crate) fn has_current_entry(&self, entry: u32, memory: &MirrorMemory) -> bool {
        self.by_entry.get(&entry).is_some_and(|indexed| {
            indexed
                .units
                .iter()
                .any(|id| self.units[id].is_current(memory))
        })
    }
    pub fn set_direct_links(
        &mut self,
        enabled: bool,
        memory: &mut MirrorMemory,
    ) -> Result<(), String> {
        if self.next_id != 0 {
            return Err("direct links are fixed before compilation".into());
        }
        let direct = if enabled {
            Some(direct::DirectLinks::new()?)
        } else {
            None
        };
        self.end_invocation();
        memory.track_code_changes(enabled);
        self.direct = direct;
        Ok(())
    }
    pub fn direct_pointer(&self) -> usize {
        self.direct.as_ref().map_or(0, direct::DirectLinks::pointer)
    }
    pub fn direct_bytes(&self) -> usize {
        self.direct.as_ref().map_or(0, direct::DirectLinks::bytes)
    }
    pub fn direct_counts(&self) -> (u32, u32, u32) {
        self.direct
            .as_ref()
            .map_or((0, 0, 0), direct::DirectLinks::counts)
    }
    /// Called only at an outer boundary. Retire dependencies affected by an
    /// observed code/mapping change, without walking unrelated cached units.
    pub fn reconcile(&mut self, memory: &mut MirrorMemory) {
        if self.direct.is_none() {
            return;
        }
        self.end_invocation();
        let changes = memory.take_code_changes();
        let affected: BTreeSet<_> = changes
            .iter()
            .flat_map(|page| {
                self.by_page
                    .get(&(page << 12))
                    .into_iter()
                    .flatten()
                    .copied()
            })
            .collect();
        for id in affected {
            if self
                .units
                .get(&id)
                .is_some_and(|unit| !unit.is_current(memory))
            {
                self.retire_for(id, RetirementReason::Dependency);
            }
        }
    }
    /// Experimental host resource budget, fixed before the first compilation
    /// identity is reserved. It never changes guest CPU or memory semantics.
    pub fn set_capacity_mode(&mut self, mode: u32) -> Result<(), String> {
        let (units, bytes) = match mode {
            0 => (DEFAULT_MAXIMUM_UNITS, DEFAULT_MAXIMUM_CODE_BYTES),
            1 => (MAXIMUM_UNITS, MAXIMUM_CODE_BYTES),
            _ => return Err("invalid compiled cache capacity mode".into()),
        };
        if self.next_id != 0 || self.prepared_capacity != 0 {
            return Err("compiled cache capacity is fixed after identity reservation".into());
        }
        self.slots = vec![0; units].into_boxed_slice();
        self.maximum_units = units;
        self.maximum_bytes = bytes;
        Ok(())
    }
    pub fn capacity(&self) -> usize {
        self.slots.len()
    }
    pub fn compilation_capacity(&self) -> usize {
        self.maximum_units
    }
    pub fn byte_limit(&self) -> usize {
        self.maximum_bytes + self.prepared_byte_limit
    }
    pub fn selected(&self) -> Option<&Unit> {
        self.selected
            .and_then(|id| self.units.get(&id))
            .map(|unit| &unit.unit)
    }
    pub fn len(&self) -> usize {
        self.units.len()
    }
    pub fn is_empty(&self) -> bool {
        self.units.is_empty()
    }
    pub fn bytes(&self) -> usize {
        self.bytes + self.prepared_bytes
    }
    pub(crate) fn fits_without_eviction(&self, bytes: usize) -> bool {
        // Preparation already reserved these table slots. Demand code may use
        // the unused slots without expanding either pool's byte budget, and
        // yields them back when a later prepared unit needs admission.
        self.units.len() < self.slots.len()
            && bytes <= self.maximum_bytes
            && self.bytes <= self.maximum_bytes - bytes
    }
    /// AOT residency is independent of the demand-code LRU. The browser grows
    /// its table to this Rust-owned bound before installing prepared code.
    pub(crate) fn enable_preparation(&mut self) {
        if self.prepared_capacity == 0 {
            self.prepared_capacity = MAXIMUM_PREPARED_UNITS;
            self.prepared_byte_limit = MAXIMUM_PREPARED_BYTES;
            let mut slots = self.slots.to_vec();
            slots.resize(self.maximum_units + self.prepared_capacity, 0);
            self.slots = slots.into_boxed_slice();
        }
    }
    pub(crate) fn fits_prepared(&self, bytes: usize) -> bool {
        self.prepared.len() < self.prepared_capacity
            && bytes <= self.prepared_byte_limit
            && self.prepared_bytes <= self.prepared_byte_limit - bytes
    }
    /// Promotion preserves code identity, native code and the installed slot.
    /// Only its eviction policy changes; dependency validation is identical.
    pub(crate) fn retain_selected(&mut self) -> bool {
        let id = self.selected.expect("selected preparation unit");
        if self.prepared.contains(&id) {
            return true;
        }
        let bytes = self.units[&id].code.len();
        if !self.fits_prepared(bytes) {
            return false;
        }
        self.prepared.insert(id);
        self.prepared_bytes += bytes;
        self.bytes -= bytes;
        self.recent.retain(|&other| other != id);
        true
    }
    pub fn budget_continuation_limit(&self, entry: u32, memory: &MirrorMemory) -> Option<usize> {
        self.by_page
            .get(&(entry & !4095))
            .into_iter()
            .flatten()
            .filter_map(|id| {
                let unit = &self.units[id];
                unit.interior_limits
                    .get(&entry)
                    .copied()
                    .filter(|_| unit.is_current(memory))
            })
            .min()
    }
    fn touch(&mut self, id: u32) {
        if !self.prepared.contains(&id) {
            self.recent.retain(|&other| other != id);
            self.recent.push_back(id);
        }
        self.selected = Some(id);
    }
    fn retire(&mut self, id: u32) {
        self.retire_for(id, RetirementReason::Explicit);
    }
    fn retire_for(&mut self, id: u32, reason: RetirementReason) {
        self.end_invocation();
        let Some(unit) = self.units.remove(&id) else {
            return;
        };
        self.retirement_counts[reason as usize] =
            self.retirement_counts[reason as usize].saturating_add(1);
        let unit = unit.unit;
        if self.prepared.remove(&id) {
            self.prepared_bytes -= unit.code.len();
        } else {
            self.bytes -= unit.code.len();
        }
        if let Some(direct) = &mut self.direct {
            for &entry in &unit.entries {
                direct.revoke(entry, id);
            }
        }
        for page in unit.dependencies.keys() {
            let ids = self.by_page.get_mut(page).expect("indexed code dependency");
            ids.remove(&id);
            if ids.is_empty() {
                self.by_page.remove(page);
            }
        }
        self.ready.remove(&id);
        let slot = self.by_slot.remove(&id).expect("indexed unit slot");
        self.slots[slot as usize - 1] = 0;
        for entry in unit.entries {
            let indexed = self.by_entry.get_mut(&entry).expect("indexed unit");
            indexed.units.remove(&id);
            if indexed.units.is_empty() {
                self.by_entry.remove(&entry);
            }
        }
        self.recent.retain(|&other| other != id);
        if self.selected == Some(id) {
            self.selected = None;
        }
        // Consumers that never poll must not grow an unbounded notification
        // history. A flush drops all browser instances, including still-valid
        // ones; those can be instantiated again from their unchanged Rust unit.
        if self.retired.front() == Some(&RETIRE_ALL) {
            return;
        }
        if self.retired.len() == self.capacity() {
            self.retired.clear();
            self.retired.push_back(RETIRE_ALL);
            // The browser will release every table binding, including valid
            // surviving units. None may remain callable until rebound.
            self.ready.clear();
            if let Some(direct) = &mut self.direct {
                direct.clear_bindings();
            }
        } else {
            self.retired.push_back(id);
        }
    }
    pub fn pop_retired(&mut self) -> u32 {
        self.retired.pop_front().unwrap_or(0)
    }
    /// Diagnostic totals, independent of notification draining or overflow.
    pub fn retirement_count(&self, reason: u32) -> u64 {
        self.retirement_counts
            .get(reason as usize)
            .copied()
            .unwrap_or(0)
    }
    pub fn clear(&mut self) {
        self.end_invocation();
        while let Some(&id) = self.units.keys().next() {
            self.retire(id);
        }
    }
    /// Runtime cache effects summarize a validated memory delta. Recheck the
    /// complete dependency stamps, including aliases and absent fetch pages,
    /// instead of discarding unrelated prepared code. This bounded scan runs
    /// only for code-affecting Runtime updates, not on ordinary data writes.
    pub fn invalidate_changed(&mut self, memory: &MirrorMemory) {
        self.end_invocation();
        let stale: Vec<_> = self
            .units
            .iter()
            .filter_map(|(&id, unit)| (!unit.is_current(memory)).then_some(id))
            .collect();
        for id in stale {
            self.retire_for(id, RetirementReason::Dependency);
        }
    }
    pub fn insert(&mut self, unit: Unit) -> Result<u32, String> {
        let id = self.reserve_id()?;
        self.insert_reserved(id, unit)
    }
    /// Reserve before code generation so its embedded identity cannot be
    /// reused after any compilation failure.
    pub fn reserve_id(&mut self) -> Result<u32, String> {
        let id = self
            .next_id
            .checked_add(1)
            .filter(|&id| id != RETIRE_ALL)
            .ok_or("compiled unit identity exhaustion")?;
        self.next_id = id;
        Ok(id)
    }
    pub fn insert_reserved(&mut self, id: u32, unit: Unit) -> Result<u32, String> {
        self.admit(id, unit, false)
    }
    pub(crate) fn insert_prepared(&mut self, id: u32, unit: Unit) -> Result<u32, String> {
        self.admit(id, unit, true)
    }
    fn admit(&mut self, id: u32, unit: Unit, prepared: bool) -> Result<u32, String> {
        self.end_invocation();
        if id == 0 || id != self.next_id || self.units.contains_key(&id) {
            return Err("invalid reserved compiled unit identity".into());
        }
        if unit.code.is_empty()
            || unit.entries.is_empty()
            || if prepared {
                !self.fits_prepared(unit.code.len())
            } else {
                unit.code.len() > self.maximum_bytes
            }
        {
            return Err("compiled unit exceeds cache budget or is empty".into());
        }
        // All fallible admission checks precede eviction. Never reuse an id,
        // even when mapping changes or options retire every existing unit.
        while if prepared {
            self.units.len() == self.slots.len()
        } else {
            !self.fits_without_eviction(unit.code.len())
        } {
            self.retire_for(
                *self.recent.front().expect("nonempty full cache"),
                RetirementReason::Capacity,
            );
        }
        let slot = self
            .slots
            .iter()
            .position(|slot| *slot == 0)
            .expect("bounded cache has free link slot");
        self.slots[slot] = id;
        self.by_slot.insert(id, slot as u32 + 1);
        if prepared {
            self.prepared.insert(id);
            self.prepared_bytes += unit.code.len();
        } else {
            self.bytes += unit.code.len();
        }
        for &entry in &unit.entries {
            self.by_entry.entry(entry).or_default().units.insert(id);
        }
        for &page in unit.dependencies.keys() {
            self.by_page.entry(page).or_default().insert(id);
        }
        self.units.insert(id, CachedUnit::new(unit));
        self.touch(id);
        Ok(id)
    }
    /// Slots are Rust-owned and never imply an installed Chrome resource.
    pub fn link_slot(&self, id: u32) -> u32 {
        self.by_slot.get(&id).copied().unwrap_or(0)
    }
    pub fn bind(&mut self, id: u32, slot: u32, memory: &MirrorMemory) -> Result<(), String> {
        self.end_invocation();
        if !self.retired.is_empty() {
            return Err("retired table bindings must be drained before binding".into());
        }
        if slot == 0 || self.link_slot(id) != slot {
            return Err("stale compiled unit link slot".into());
        }
        if !self.units[&id].is_current(memory) {
            self.retire_for(id, RetirementReason::Dependency);
            return Err("compiled code changed before link binding".into());
        }
        self.ready.insert(id);
        let unit = &self.units[&id];
        if let Some(direct) = &mut self.direct
            && !unit.dependencies.is_empty()
            && unit.dependencies.iter().all(|(&page, stamp)| {
                stamp.is_some()
                    && memory.entry(page).flags & (MAPPED | EXECUTE | SEALED)
                        == MAPPED | EXECUTE | SEALED
            })
        {
            let binding = (u64::from(id) << 32) | u64::from(slot);
            for &entry in &unit.entries {
                direct.publish(entry, binding);
            }
        }
        Ok(())
    }
    /// Direct Wasm-to-Wasm resolver. A cached but uninstalled unit deliberately
    /// misses so the outer host can instantiate it; this never compiles code.
    pub fn resolve(&mut self, entry: u32, memory: &MirrorMemory) -> u64 {
        if let Some(epoch) = self.invocation_epoch {
            if epoch != memory.epoch() {
                self.end_invocation();
            } else {
                if let Some(indexed) = self.by_entry.get(&entry)
                    && indexed.invocation == self.invocation_id
                {
                    self.selected = Some((indexed.binding >> 32) as u32);
                    if let Some(direct) = &mut self.direct {
                        direct.selected((indexed.binding >> 32) as u32);
                    }
                    self.memo_hits = self.memo_hits.saturating_add(1);
                    return indexed.binding;
                }
                self.memo_misses = self.memo_misses.saturating_add(1);
            }
        }
        let Some(id) = self.select_for(entry, memory, true) else {
            if let Some(direct) = &mut self.direct {
                direct.selected(0);
            }
            return 0;
        };
        if let Some(direct) = &mut self.direct {
            direct.selected(id);
        }
        if !self.ready.contains(&id) {
            return 0;
        }
        let binding = (u64::from(id) << 32) | u64::from(self.link_slot(id));
        self.remember_link(entry, id, binding, memory);
        binding
    }
    pub fn invocation_is_current(&self, memory: &MirrorMemory) -> bool {
        self.invocation_epoch == Some(memory.epoch())
    }
    fn remember_link(&mut self, entry: u32, id: u32, binding: u64, memory: &MirrorMemory) {
        if self.invocation_epoch == Some(memory.epoch())
            && self.units[&id].certify_invocation(memory.epoch(), self.invocation_id)
        {
            let indexed = self.by_entry.get_mut(&entry).expect("validated entry");
            indexed.invocation = self.invocation_id;
            indexed.binding = binding;
        }
    }
    /// Only an outer host admission starts this proof. The host must call end
    /// in finally around run_linked, including native Wasm traps. No mutation
    /// or browser/Runtime callback may interleave with the synchronous chain.
    pub fn begin_invocation(
        &mut self,
        id: u32,
        entry: u32,
        memory: &MirrorMemory,
    ) -> Result<(), String> {
        self.end_invocation();
        self.validate(id, entry, memory)?;
        if !self.ready.contains(&id) {
            return Err("initial linked unit is not bound".into());
        }
        // Reusing the memory epoch alone would accept a prior invocation's
        // proof after an unfinished code-alias store. Never recycle this ID.
        self.invocation_id = self
            .invocation_id
            .checked_add(1)
            .ok_or("compiled invocation identity exhaustion")?;
        self.memo_hits = 0;
        self.memo_misses = 0;
        self.invocation_epoch = Some(memory.epoch());
        self.touch(id);
        let binding = (u64::from(id) << 32) | u64::from(self.link_slot(id));
        self.remember_link(entry, id, binding, memory);
        // Unfinished journals (including a fault/resource fixture) retain the
        // checked resolver path. Do not consume or repair them at admission:
        // that would change the guest's fault priority or completed effects.
        if let Some(direct) = &mut self.direct
            && !memory.code_changes_pending()
            && !memory.execution_writes_pending()
        {
            direct.begin(id);
        }
        Ok(())
    }
    pub fn end_invocation(&mut self) {
        if let Some(direct) = &mut self.direct
            && let Some(id) = direct.end()
        {
            self.selected = Some(id);
        }
        if self.invocation_epoch.take().is_some()
            && let Some(id) = self.selected
        {
            // Memo hits avoid all LRU searches; publish only the final selected
            // unit's recency when returning to the outer host.
            self.touch(id);
        }
    }
    pub fn memo_counts(&self) -> (u32, u32) {
        (self.memo_hits, self.memo_misses)
    }
    pub fn validate(&mut self, id: u32, entry: u32, memory: &MirrorMemory) -> Result<(), String> {
        self.validate_for(id, entry, memory, false)
    }
    fn validate_for(
        &mut self,
        id: u32,
        entry: u32,
        memory: &MirrorMemory,
        chain: bool,
    ) -> Result<(), String> {
        let invocation = if chain && self.invocation_epoch == Some(memory.epoch()) {
            self.invocation_id
        } else {
            0
        };
        let unit = self.units.get(&id).ok_or("stale compiled unit identity")?;
        if !unit.is_current_for(memory, invocation) {
            self.retire_for(id, RetirementReason::Dependency);
            return Err("compiled code mapping/content changed or has unobserved writes; recompile required".into());
        }
        if !unit.entries.contains(&entry) {
            return Err("entry is outside compiled region".into());
        }
        Ok(())
    }
    /// Read-only admission for an outstanding CPU/Runtime completion. Failure
    /// must retain the pending operation and its diagnostic state unchanged.
    pub fn check_code_span(
        &self,
        id: u32,
        eip: u32,
        length: u32,
        memory: &MirrorMemory,
    ) -> Result<(), String> {
        let unit = self.units.get(&id).ok_or("stale compiled unit identity")?;
        if !(1..=15).contains(&length)
            || (0..length).any(|offset| {
                eip.checked_add(offset)
                    .is_none_or(|address| !unit.dependencies.contains_key(&(address & !4095)))
            })
        {
            return Err(
                "CPU completion instruction is outside its compiled code dependencies".into(),
            );
        }
        if !unit.is_current(memory) {
            return Err("compiled code changed during CPU completion".into());
        }
        Ok(())
    }
    pub fn select(&mut self, entry: u32, memory: &MirrorMemory) -> Option<u32> {
        self.select_for(entry, memory, false)
    }
    fn select_for(&mut self, entry: u32, memory: &MirrorMemory, chain: bool) -> Option<u32> {
        if let Some(id) = self.selected
            && self.units[&id].entries.contains(&entry)
            && self.validate_for(id, entry, memory, chain).is_ok()
        {
            self.touch(id);
            return Some(id);
        }
        self.selected = None;
        // Prepared units do not participate in LRU churn. Prefer their exact
        // entry index, validating only candidates rather than walking the
        // potentially much larger prepared catalog on every dispatch.
        let mut before = u32::MAX;
        loop {
            let candidate = self.by_entry.get(&entry).and_then(|indexed| {
                indexed
                    .units
                    .range(..before)
                    .rev()
                    .find(|id| self.prepared.contains(id))
                    .copied()
            });
            let Some(id) = candidate else { break };
            before = id;
            if self.validate_for(id, entry, memory, chain).is_ok() {
                self.touch(id);
                return Some(id);
            }
        }
        // Entries index the candidates. Check their stamps only, never every
        // cached region. Resolve does not allocate a temporary candidate list.
        let mut index = self.recent.len();
        while index > 0 {
            index -= 1;
            let id = self.recent[index];
            if self
                .by_entry
                .get(&entry)
                .is_some_and(|indexed| indexed.units.contains(&id))
                && self.validate_for(id, entry, memory, chain).is_ok()
            {
                self.touch(id);
                return Some(id);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::{EXECUTE, PAGE_SIZE, READ, SEALED, WRITE};

    fn memory() -> MirrorMemory {
        let mut memory = MirrorMemory::new(4).unwrap();
        for (address, owner) in [(0x1000, 1), (0x2000, 2)] {
            memory
                .map_page(address, owner, READ | EXECUTE | SEALED, &[0x90; PAGE_SIZE])
                .unwrap();
        }
        memory
    }
    fn unit(memory: &MirrorMemory, entry: u32, bytes: usize) -> Unit {
        Unit {
            code: vec![0; bytes].into_boxed_slice(),
            entries: [entry].into(),
            dependencies: [(entry & !4095, memory.code_stamp(entry))].into(),
            instruction_count: 1,
            interior_limits: BTreeMap::new(),
            preparation_frontier: Vec::new(),
        }
    }
    #[test]
    fn continuation_lengths_require_live_code_and_never_grant_entry_permission() {
        let mut memory = memory();
        let mut cache = CodeCache::default();
        let mut code = unit(&memory, 0x1000, 4);
        code.interior_limits.insert(0x1001, 2);
        let id = cache.insert(code).unwrap();
        assert_eq!(cache.budget_continuation_limit(0x1001, &memory), Some(2));
        assert_eq!(cache.budget_continuation_limit(0x1000, &memory), None);
        assert_eq!(cache.select(0x1001, &memory), None);
        assert!(cache.validate(id, 0x1001, &memory).is_err());
        memory.protect_page(0x1000, 1, READ | SEALED).unwrap();
        assert_eq!(cache.budget_continuation_limit(0x1001, &memory), None);
    }
    #[test]
    fn indexed_continuations_cover_prepared_and_demand_code_with_full_dependencies() {
        let mut memory = memory();
        let mut cache = CodeCache::new(2, 8);
        cache.enable_preparation();
        let mut wide = unit(&memory, 0x1000, 4);
        wide.dependencies.insert(0x2000, memory.code_stamp(0x2000));
        wide.interior_limits
            .extend([(0x1001, 8), (0x1fff, 4), (0x2000, 3)]);
        let prepared = cache.reserve_id().unwrap();
        cache.insert_prepared(prepared, wide).unwrap();
        let mut short = unit(&memory, 0x1000, 4);
        short.interior_limits.insert(0x1001, 2);
        let demand = cache.insert(short).unwrap();
        assert_eq!(cache.budget_continuation_limit(0x1001, &memory), Some(2));
        assert_eq!(cache.budget_continuation_limit(0x1fff, &memory), Some(4));
        assert_eq!(cache.budget_continuation_limit(0x2000, &memory), Some(3));
        assert_eq!(cache.budget_continuation_limit(0x2001, &memory), None);
        assert_eq!(cache.select(0x1001, &memory), None);
        assert!(cache.validate(prepared, 0x1001, &memory).is_err());
        cache.retire(demand);
        assert_eq!(cache.budget_continuation_limit(0x1001, &memory), Some(8));
        // A different dependency page must revoke the suffix even when its
        // own page's mapping and bytes have not changed.
        memory.protect_page(0x2000, 2, READ | SEALED).unwrap();
        assert_eq!(cache.budget_continuation_limit(0x1001, &memory), None);
        assert!(cache.validate(prepared, 0x1000, &memory).is_err());
        assert_eq!(cache.budget_continuation_limit(0x1fff, &memory), None);
        for entry in [0x1000, 0x1004, 0x1008] {
            let mut code = unit(&memory, entry, 4);
            code.interior_limits.insert(entry + 1, 1);
            cache.insert(code).unwrap();
        }
        assert_eq!(cache.budget_continuation_limit(0x1001, &memory), None);
        assert_eq!(cache.budget_continuation_limit(0x1005, &memory), Some(1));
        assert_eq!(cache.budget_continuation_limit(0x1009, &memory), Some(1));
        cache.clear();
        assert_eq!(cache.budget_continuation_limit(0x1005, &memory), None);
    }
    #[test]
    fn prepared_residency_has_independent_limits_identity_and_invalidation() {
        let mut memory = memory();
        let mut cache = CodeCache::new(1, 4);
        cache.enable_preparation();
        cache.prepared_capacity = 2;
        cache.prepared_byte_limit = 8;
        let first = cache.insert(unit(&memory, 0x1000, 4)).unwrap();
        let slot = cache.link_slot(first);
        cache.bind(first, slot, &memory).unwrap();
        assert!(cache.retain_selected());
        assert_eq!(cache.link_slot(first), slot);
        assert_eq!(
            cache.resolve(0x1000, &memory),
            u64::from(first) << 32 | u64::from(slot)
        );
        let second = cache.reserve_id().unwrap();
        cache
            .insert_prepared(second, unit(&memory, 0x1001, 4))
            .unwrap();
        let third = cache.insert(unit(&memory, 0x2000, 4)).unwrap();
        assert!(
            !cache.retain_selected(),
            "full prepared pool cannot consume JIT residency"
        );
        let failed = cache.reserve_id().unwrap();
        assert!(
            cache
                .insert_prepared(failed, unit(&memory, 0x2001, 1))
                .is_err()
        );
        assert_eq!(cache.selected_id(), third);
        assert_eq!(cache.pop_retired(), 0);
        assert_eq!((cache.len(), cache.bytes()), (3, 12));
        let fourth = cache.insert(unit(&memory, 0x2001, 4)).unwrap();
        assert_eq!(cache.pop_retired(), third);
        assert_eq!(cache.select(0x1000, &memory), Some(first));
        memory.map_alias(0x3000, 0x1000, 3, READ | WRITE).unwrap();
        memory.write(0x3000, &[0xcc]).unwrap();
        assert!(cache.validate(first, 0x1000, &memory).is_err());
        assert_eq!(cache.select(0x1001, &memory), None);
        assert_eq!((cache.prepared.len(), cache.prepared_bytes), (0, 0));
        assert_eq!(cache.select(0x2001, &memory), Some(fourth));
        cache.clear();
        assert_eq!((cache.len(), cache.bytes()), (0, 0));
        assert!(cache.slots.iter().all(|id| *id == 0));
    }
    #[test]
    fn demand_borrows_unused_prepared_slots_and_yields_them_without_evicting_aot() {
        let memory = memory();
        let mut cache = CodeCache::new(1, 12);
        cache.enable_preparation();
        cache.prepared_capacity = 2;
        cache.prepared_byte_limit = 8;
        cache.slots = vec![0; 3].into_boxed_slice();
        let first = cache.reserve_id().unwrap();
        cache
            .insert_prepared(first, unit(&memory, 0x1000, 4))
            .unwrap();
        let oldest = cache.insert(unit(&memory, 0x2000, 4)).unwrap();
        let newer = cache.insert(unit(&memory, 0x2001, 4)).unwrap();
        assert_eq!(cache.len(), 3);
        assert_eq!(cache.retirement_count(1), 0);
        assert!(cache.select(0x2000, &memory).is_some());
        let second = cache.reserve_id().unwrap();
        cache
            .insert_prepared(second, unit(&memory, 0x1001, 4))
            .unwrap();
        assert_eq!(
            cache.pop_retired(),
            newer,
            "preparation reclaims the least recent demand slot"
        );
        assert_eq!(cache.prepared, [first, second].into());
        assert_eq!(cache.select(0x2000, &memory), Some(oldest));
        let next = cache.insert(unit(&memory, 0x2002, 4)).unwrap();
        assert_eq!(cache.pop_retired(), oldest);
        assert_eq!(cache.select(0x2002, &memory), Some(next));
        assert!(cache.has_prepared_entry(0x1000, &memory));
        assert!(cache.has_prepared_entry(0x1001, &memory));
        assert_eq!(cache.retirement_count(1), 2);
    }
    #[test]
    fn borrowed_slots_preserve_demand_byte_limit_and_rejected_admission_is_atomic() {
        let memory = memory();
        let mut cache = CodeCache::new(1, 8);
        cache.enable_preparation();
        let prepared = cache.reserve_id().unwrap();
        cache
            .insert_prepared(prepared, unit(&memory, 0x1000, 4))
            .unwrap();
        let first = cache.insert(unit(&memory, 0x2000, 4)).unwrap();
        let second = cache.insert(unit(&memory, 0x2001, 4)).unwrap();
        assert!(cache.insert(unit(&memory, 0x2002, 9)).is_err());
        assert_eq!(cache.pop_retired(), 0);
        let third = cache.insert(unit(&memory, 0x2002, 4)).unwrap();
        assert_eq!(cache.pop_retired(), first);
        assert_eq!(cache.bytes, 8);
        assert_eq!(cache.prepared_bytes, 4);
        assert_eq!(cache.select(0x2001, &memory), Some(second));
        assert_eq!(cache.select(0x2002, &memory), Some(third));
        assert!(cache.has_prepared_entry(0x1000, &memory));
    }
    #[test]
    fn retirement_overflow_revokes_prepared_bindings_until_reinstalled() {
        let memory = memory();
        let mut cache = CodeCache::new(1, 4);
        cache.enable_preparation();
        cache.prepared_capacity = 1;
        cache.prepared_byte_limit = 4;
        cache.slots = vec![0; 2].into_boxed_slice();
        let id = cache.reserve_id().unwrap();
        cache.insert_prepared(id, unit(&memory, 0x1000, 4)).unwrap();
        cache.bind(id, 1, &memory).unwrap();
        for _ in 0..5 {
            cache.insert(unit(&memory, 0x2000, 4)).unwrap();
        }
        assert_eq!(cache.retired.len(), 1);
        assert_eq!(cache.select(0x1000, &memory), Some(id));
        assert_eq!(cache.resolve(0x1000, &memory), 0);
        assert!(cache.bind(id, 1, &memory).is_err());
        assert_eq!(cache.pop_retired(), RETIRE_ALL);
        cache.bind(id, 1, &memory).unwrap();
        assert_eq!(cache.resolve(0x1000, &memory), u64::from(id) << 32 | 1);
    }
    #[test]
    fn configured_capacity_preserves_identity_slots_and_retirement_bounds() {
        let memory = memory();
        for mode in [0, 1] {
            let mut cache = CodeCache::default();
            cache.set_capacity_mode(mode).unwrap();
            let capacity = if mode == 0 { 64 } else { 512 };
            assert_eq!(cache.capacity(), capacity);
            assert_eq!(cache.slots.len(), capacity);
            let mut ids = Vec::new();
            for index in 0..capacity {
                let id = cache
                    .insert(unit(&memory, 0x1000 + index as u32, 4))
                    .unwrap();
                assert_eq!(cache.link_slot(id), index as u32 + 1);
                cache.bind(id, cache.link_slot(id), &memory).unwrap();
                ids.push(id);
            }
            assert_eq!(cache.pop_retired(), 0);
            let last = *ids.last().unwrap();
            cache
                .begin_invocation(last, 0x1000 + capacity as u32 - 1, &memory)
                .unwrap();
            assert_eq!(
                cache.resolve(0x1000 + capacity as u32 - 1, &memory),
                u64::from(last) << 32 | capacity as u64
            );
            let next = cache.insert(unit(&memory, 0x2000, 4)).unwrap();
            assert_eq!(cache.len(), capacity);
            assert!(next > last);
            assert_eq!(cache.invocation_epoch, None);
            assert_eq!(cache.link_slot(ids[0]), 0);
            assert_eq!(cache.link_slot(next), 1);
            assert!(cache.bind(next, 1, &memory).is_err());
            assert_eq!(cache.pop_retired(), ids[0]);
            cache.bind(next, 1, &memory).unwrap();
            assert!(cache.bind(ids[0], 1, &memory).is_err());
            cache.clear();
            assert!(cache.ready.is_empty());
            assert!(cache.slots.iter().all(|&slot| slot == 0));
            for _ in 0..capacity {
                let retired = cache.pop_retired();
                assert!(retired > 0 && retired != RETIRE_ALL);
            }
            assert_eq!(cache.pop_retired(), 0);
            assert!(cache.set_capacity_mode(1 - mode).is_err());
            assert_eq!(cache.capacity(), capacity);
            assert!(cache.reserve_id().unwrap() > next);
        }
    }
    #[test]
    fn byte_budget_evicts_before_unit_budget_and_failed_reservation_freezes_capacity() {
        let memory = memory();
        let mut cache = CodeCache::new(MAXIMUM_UNITS, 8);
        let first = cache.insert(unit(&memory, 0x1000, 4)).unwrap();
        assert!(cache.fits_without_eviction(4));
        assert!(!cache.fits_without_eviction(5));
        let second = cache.insert(unit(&memory, 0x1001, 4)).unwrap();
        assert!(!cache.fits_without_eviction(1));
        assert!(!cache.fits_without_eviction(usize::MAX));
        assert_eq!(cache.pop_retired(), 0);
        cache.select(0x1000, &memory);
        let third = cache.insert(unit(&memory, 0x1002, 4)).unwrap();
        assert_eq!((cache.len(), cache.bytes(), cache.capacity()), (2, 8, 512));
        assert_eq!(cache.pop_retired(), second);
        assert_eq!(cache.select(0x1000, &memory), Some(first));
        assert_eq!(cache.select(0x1002, &memory), Some(third));
        let mut failed = CodeCache::default();
        assert!(failed.set_capacity_mode(2).is_err());
        let id = failed.reserve_id().unwrap();
        assert!(
            failed
                .insert_reserved(id, unit(&memory, 0x1000, 0))
                .is_err()
        );
        assert!(failed.set_capacity_mode(1).is_err());
        assert_eq!(failed.capacity(), DEFAULT_MAXIMUM_UNITS);
        assert_eq!(failed.byte_limit(), DEFAULT_MAXIMUM_CODE_BYTES);
    }
    #[test]
    fn expanded_notification_overflow_revokes_survivors_and_remains_bounded() {
        let memory = memory();
        let mut cache = CodeCache::default();
        cache.set_capacity_mode(1).unwrap();
        let survivor = cache.insert(unit(&memory, 0x1000, 4)).unwrap();
        cache.bind(survivor, 1, &memory).unwrap();
        for index in 0..(2 * cache.capacity() + 2) {
            cache
                .insert(unit(&memory, 0x2000 + (index as u32 % 4096), 4))
                .unwrap();
            cache.select(0x1000, &memory);
        }
        assert_eq!(cache.len(), MAXIMUM_UNITS);
        assert_eq!(cache.retired.len(), 1);
        assert_eq!(cache.pop_retired(), RETIRE_ALL);
        assert_eq!(cache.pop_retired(), 0);
        assert_eq!(cache.resolve(0x1000, &memory), 0);
        cache.bind(survivor, 1, &memory).unwrap();
        assert_ne!(cache.resolve(0x1000, &memory), 0);
    }
    #[test]
    fn independent_entries_reuse_units_and_lru_has_bounded_bytes_and_retired_ids() {
        let memory = memory();
        let mut cache = CodeCache::new(2, 10);
        let first = cache.insert(unit(&memory, 0x1000, 4)).unwrap();
        let second = cache.insert(unit(&memory, 0x2000, 4)).unwrap();
        assert_eq!(cache.select(0x1000, &memory), Some(first));
        let third = cache.insert(unit(&memory, 0x1001, 6)).unwrap();
        assert!(third > second && second > first);
        assert_eq!((cache.len(), cache.bytes()), (2, 10));
        assert_eq!(cache.pop_retired(), second);
        assert_eq!(cache.select(0x2000, &memory), None);
        assert!(cache.validate(second, 0x2000, &memory).is_err());
        assert_eq!(cache.select(0x1000, &memory), Some(first));
        assert!(cache.insert(unit(&memory, 0x2000, 11)).is_err());
        assert_eq!(
            (cache.len(), cache.bytes(), cache.selected_id()),
            (2, 10, first)
        );
    }
    #[test]
    fn runtime_code_effects_preserve_unrelated_aot_and_validate_all_dependency_kinds() {
        for mutation in 0..6 {
            let mut memory = memory();
            let mut cache = CodeCache::default();
            cache.enable_preparation();
            memory.map_alias(0x3000, 0x1000, 1, READ | WRITE).unwrap();
            let mut dependent = unit(&memory, 0x1000, 4);
            dependent.dependencies.insert(0x4000, None);
            let first = cache.reserve_id().unwrap();
            cache.insert_prepared(first, dependent).unwrap();
            let survivor = cache.reserve_id().unwrap();
            cache
                .insert_prepared(survivor, unit(&memory, 0x2000, 4))
                .unwrap();
            for id in [first, survivor] {
                cache.bind(id, cache.link_slot(id), &memory).unwrap();
            }
            match mutation {
                0 => memory
                    .reset_page(0x3000, 1, READ | WRITE, &[0xcc; PAGE_SIZE])
                    .unwrap(),
                1 => memory.protect_page(0x1000, 1, READ | SEALED).unwrap(),
                2 => memory.unmap_page(0x1000).unwrap(),
                3 => {
                    memory.unmap_page(0x1000).unwrap();
                    memory
                        .map_page(0x1000, 9, READ | EXECUTE | SEALED, &[0x90; PAGE_SIZE])
                        .unwrap();
                }
                4 => memory
                    .map_page(0x4000, 2, READ | EXECUTE | SEALED, &[0x90; PAGE_SIZE])
                    .unwrap(),
                _ => memory
                    .reset_page(0x1000, 1, READ | EXECUTE | SEALED, &[0x90; PAGE_SIZE])
                    .unwrap(),
            }
            cache.invalidate_changed(&memory);
            assert_eq!(cache.pop_retired(), if mutation == 5 { 0 } else { first });
            assert_eq!(cache.retirement_count(2), u64::from(mutation != 5));
            assert_eq!(cache.retirement_count(0), 0);
            assert!(cache.has_prepared_entry(0x2000, &memory));
            assert_eq!(cache.resolve(0x2000, &memory) >> 32, u64::from(survivor));
            if mutation != 5 {
                assert!(cache.validate(first, 0x1000, &memory).is_err());
            }
        }
    }
    #[test]
    fn code_alias_mapping_owner_and_missing_page_dependencies_invalidate_before_execution() {
        let mut memory = memory();
        let mut cache = CodeCache::default();
        let first = cache.insert(unit(&memory, 0x1000, 4)).unwrap();
        memory.map_alias(0x3000, 0x1000, 1, READ | WRITE).unwrap();
        memory.write(0x3000, &[0xcc]).unwrap();
        assert!(cache.validate(first, 0x1000, &memory).is_err());
        assert_eq!(cache.pop_retired(), first);
        let second = cache.insert(unit(&memory, 0x2000, 4)).unwrap();
        memory.unmap_page(0x2000).unwrap();
        memory
            .map_page(0x2000, 3, READ | EXECUTE | SEALED, &[0x90; PAGE_SIZE])
            .unwrap();
        assert_eq!(cache.select(0x2000, &memory), None);
        assert_eq!(cache.pop_retired(), second);
        let missing = cache.insert(unit(&memory, 0x4000, 4)).unwrap();
        assert_eq!(cache.select(0x4000, &memory), Some(missing));
        memory
            .map_page(0x4000, 4, READ | EXECUTE | SEALED, &[0x90; PAGE_SIZE])
            .unwrap();
        assert!(cache.validate(missing, 0x4000, &memory).is_err());
    }
    #[test]
    fn notification_overflow_is_bounded_and_identity_exhaustion_does_not_evict() {
        let memory = memory();
        let mut cache = CodeCache::new(1, 4);
        for _ in 0..5 {
            cache.insert(unit(&memory, 0x1000, 4)).unwrap();
        }
        assert_eq!(cache.retired.len(), 1);
        assert_eq!(cache.pop_retired(), RETIRE_ALL);
        let selected = cache.selected_id();
        cache.next_id = RETIRE_ALL - 1;
        assert!(cache.insert(unit(&memory, 0x2000, 4)).is_err());
        assert_eq!(cache.selected_id(), selected);
        cache.validate(selected, 0x1000, &memory).unwrap();
    }
    #[test]
    fn internal_links_require_a_live_binding_and_reject_stale_slots_and_code() {
        let mut memory = memory();
        let mut cache = CodeCache::new(2, 8);
        let first = cache.insert(unit(&memory, 0x1000, 4)).unwrap();
        let first_slot = cache.link_slot(first);
        assert_ne!(first_slot, 0);
        assert_eq!(cache.resolve(0x1000, &memory), 0);
        assert!(cache.bind(first, first_slot + 1, &memory).is_err());
        cache.bind(first, first_slot, &memory).unwrap();
        assert_eq!(
            cache.resolve(0x1000, &memory),
            (u64::from(first) << 32) | u64::from(first_slot)
        );
        let second = cache.insert(unit(&memory, 0x2000, 4)).unwrap();
        cache
            .bind(second, cache.link_slot(second), &memory)
            .unwrap();
        let third = cache.insert(unit(&memory, 0x2001, 4)).unwrap();
        assert_eq!(cache.link_slot(first), 0);
        assert_eq!(cache.link_slot(third), first_slot);
        assert!(cache.bind(third, first_slot, &memory).is_err());
        assert_eq!(cache.pop_retired(), first);
        cache.bind(third, first_slot, &memory).unwrap();
        assert!(cache.bind(first, first_slot, &memory).is_err());
        memory.unmap_page(0x2000).unwrap();
        memory
            .map_page(0x2000, 9, READ | EXECUTE | SEALED, &[0x90; PAGE_SIZE])
            .unwrap();
        assert_eq!(cache.resolve(0x2001, &memory), 0);
        assert_eq!(cache.link_slot(third), 0);
        assert_eq!(cache.pop_retired(), third);
    }
    #[test]
    fn retirement_flush_revokes_surviving_bindings_and_failed_compilation_ids() {
        let memory = memory();
        let mut cache = CodeCache::new(2, 8);
        let survivor = cache.insert(unit(&memory, 0x1000, 4)).unwrap();
        let slot = cache.link_slot(survivor);
        cache.bind(survivor, slot, &memory).unwrap();
        for _ in 0..4 {
            let other = cache.insert(unit(&memory, 0x2000, 4)).unwrap();
            assert!(other > survivor);
            cache.select(0x1000, &memory); // retain survivor while old others evict
        }
        assert_eq!(cache.pop_retired(), RETIRE_ALL);
        assert_eq!(cache.resolve(0x1000, &memory), 0);
        cache.bind(survivor, slot, &memory).unwrap();
        assert_ne!(cache.resolve(0x1000, &memory), 0);
        let failed = cache.reserve_id().unwrap();
        assert!(
            cache
                .insert_reserved(failed, unit(&memory, 0x2000, 9))
                .is_err()
        );
        assert!(cache.reserve_id().unwrap() > failed);
    }

    #[test]
    fn invocation_entry_proofs_distinguish_full_eip_and_end_at_outer_return() {
        let memory = memory();
        let mut cache = CodeCache::default();
        let first_entry = 0x1000;
        let other_entry = 0x2000;
        let first = cache.insert(unit(&memory, first_entry, 4)).unwrap();
        let second = cache.insert(unit(&memory, other_entry, 4)).unwrap();
        cache.bind(first, cache.link_slot(first), &memory).unwrap();
        cache
            .bind(second, cache.link_slot(second), &memory)
            .unwrap();
        cache.begin_invocation(first, first_entry, &memory).unwrap();
        assert_eq!(cache.resolve(first_entry, &memory) >> 32, u64::from(first));
        assert_eq!(cache.resolve(other_entry, &memory) >> 32, u64::from(second));
        assert_eq!(cache.resolve(other_entry, &memory) >> 32, u64::from(second));
        assert_eq!(cache.resolve(first_entry, &memory) >> 32, u64::from(first));
        assert_eq!(cache.memo_counts(), (3, 1));
        cache.end_invocation();
        assert_eq!(cache.resolve(first_entry, &memory) >> 32, u64::from(first));
        assert_eq!(cache.memo_counts(), (3, 1));
        assert_eq!(cache.recent.back(), Some(&first));
    }

    #[test]
    fn entry_proof_cannot_follow_retired_owner_or_reused_slot() {
        let memory = memory();
        let mut cache = CodeCache::default();
        let mut shared = unit(&memory, 0x1000, 4);
        shared.entries.insert(0x1010);
        let survivor = cache.insert(shared).unwrap();
        let retired = cache.insert(unit(&memory, 0x1010, 4)).unwrap();
        for id in [survivor, retired] {
            cache.bind(id, cache.link_slot(id), &memory).unwrap();
        }
        cache.begin_invocation(retired, 0x1010, &memory).unwrap();
        assert_eq!(cache.resolve(0x1010, &memory) >> 32, u64::from(retired));
        let slot = cache.link_slot(retired);
        cache.retire(retired);
        assert_eq!(cache.pop_retired(), retired);
        let replacement = cache.insert(unit(&memory, 0x2000, 4)).unwrap();
        assert_eq!(cache.link_slot(replacement), slot);
        cache.bind(replacement, slot, &memory).unwrap();
        cache
            .begin_invocation(replacement, 0x2000, &memory)
            .unwrap();
        assert_eq!(cache.resolve(0x1010, &memory) >> 32, u64::from(survivor));
        assert_eq!(cache.resolve(0x1011, &memory), 0);
        assert_eq!(cache.resolve(0x2000, &memory) >> 32, u64::from(replacement));
    }

    #[test]
    fn nx_fault_dependencies_never_memoize_and_ordinary_alias_writes_revoke_them() {
        for address in [0x3000, 0x4000] {
            let mut memory = memory();
            memory
                .map_page(0x3000, 3, READ | WRITE, &[0; PAGE_SIZE])
                .unwrap();
            memory.map_alias(0x4000, 0x3000, 3, READ | WRITE).unwrap();
            let mut cache = CodeCache::default();
            let mut candidate = unit(&memory, 0x1000, 4);
            candidate
                .dependencies
                .insert(0x3000, memory.code_stamp(0x3000));
            let id = cache.insert(candidate).unwrap();
            cache.bind(id, cache.link_slot(id), &memory).unwrap();
            cache.begin_invocation(id, 0x1000, &memory).unwrap();
            assert_ne!(cache.resolve(0x1000, &memory), 0);
            assert_ne!(cache.resolve(0x1000, &memory), 0);
            assert_eq!(cache.memo_counts(), (0, 2));
            let epoch = memory.epoch();
            assert!(!memory.write_small(address, &[9]).unwrap());
            assert_eq!(memory.epoch(), epoch);
            assert_eq!(cache.resolve(0x1000, &memory), 0);
            assert_eq!(cache.pop_retired(), id);
        }
    }

    #[test]
    fn absent_dependencies_are_stable_only_until_mapping_changes() {
        let mut memory = memory();
        let mut cache = CodeCache::default();
        let mut candidate = unit(&memory, 0x1000, 4);
        candidate.dependencies.insert(0x3000, None);
        let id = cache.insert(candidate).unwrap();
        cache.bind(id, cache.link_slot(id), &memory).unwrap();
        cache.begin_invocation(id, 0x1000, &memory).unwrap();
        assert_ne!(cache.resolve(0x1000, &memory), 0);
        assert_eq!(cache.memo_counts(), (1, 0));
        memory.map_page(0x3000, 3, READ, &[0; PAGE_SIZE]).unwrap();
        assert_eq!(cache.resolve(0x1000, &memory), 0);
        assert_eq!(cache.pop_retired(), id);
    }

    #[test]
    fn binding_and_outer_return_clear_proofs_before_code_alias_store() {
        let mut memory = memory();
        memory.map_alias(0x3000, 0x1000, 1, READ | WRITE).unwrap();
        let mut cache = CodeCache::default();
        let id = cache.insert(unit(&memory, 0x1000, 4)).unwrap();
        let slot = cache.link_slot(id);
        cache.bind(id, slot, &memory).unwrap();
        cache.begin_invocation(id, 0x1000, &memory).unwrap();
        assert_ne!(cache.resolve(0x1000, &memory), 0);
        cache.bind(id, slot, &memory).unwrap();
        assert_eq!(cache.invocation_epoch, None);
        cache.begin_invocation(id, 0x1000, &memory).unwrap();
        cache.end_invocation();
        let epoch = memory.epoch();
        assert!(memory.write_small(0x3000, &[0xcc]).unwrap());
        assert_eq!(memory.epoch(), epoch);
        assert_eq!(cache.resolve(0x1000, &memory), 0);
        assert_eq!(cache.pop_retired(), id);
    }

    #[test]
    fn different_entries_share_live_code_but_never_grant_an_undecoded_entry() {
        let mut memory = memory();
        memory
            .map_page(0x3000, 3, READ | WRITE, &[0; PAGE_SIZE])
            .unwrap();
        let mut cache = CodeCache::default();
        let mut candidate = unit(&memory, 0x1000, 4);
        candidate.entries.extend([0x1010, 0x1020]);
        let id = cache.insert(candidate).unwrap();
        cache.bind(id, cache.link_slot(id), &memory).unwrap();
        cache.begin_invocation(id, 0x1000, &memory).unwrap();
        assert!(!memory.write_small(0x3000, &[7]).unwrap());
        for entry in [0x1010, 0x1020, 0x1010] {
            assert_eq!(cache.resolve(entry, &memory) >> 32, u64::from(id));
        }
        assert_eq!(cache.resolve(0x1011, &memory), 0);
        assert!(cache.validate(id, 0x1011, &memory).is_err());
        assert!(memory.execution_writes_pending());
        assert_eq!(
            memory.read(0x3000, 1, crate::memory::Access::Read).unwrap(),
            [7]
        );
        cache.end_invocation();
        memory.finish_execution().unwrap();
        assert_eq!(cache.select(0x1020, &memory), Some(id));
    }

    #[test]
    fn same_epoch_new_invocation_cannot_reuse_another_units_stale_code_proof() {
        for prepared in [false, true] {
            let mut memory = memory();
            memory.map_alias(0x3000, 0x1000, 1, READ | WRITE).unwrap();
            let mut cache = CodeCache::default();
            let mut candidate = unit(&memory, 0x1000, 4);
            candidate.entries.extend([0x1010, 0x1020]);
            let target = cache.insert(candidate).unwrap();
            if prepared {
                cache.enable_preparation();
                assert!(cache.retain_selected());
            }
            let clean = cache.insert(unit(&memory, 0x2000, 4)).unwrap();
            for id in [target, clean] {
                cache.bind(id, cache.link_slot(id), &memory).unwrap();
            }
            cache.begin_invocation(target, 0x1000, &memory).unwrap();
            assert_eq!(cache.resolve(0x1010, &memory) >> 32, u64::from(target));
            cache.end_invocation();
            let epoch = memory.epoch();
            assert!(memory.write_small(0x3000, &[0xcc]).unwrap());
            assert_eq!(memory.epoch(), epoch);
            // The next initial unit is clean: only the eventual target has an
            // unobserved alias write. Its prior invocation cannot authorize it.
            cache.begin_invocation(clean, 0x2000, &memory).unwrap();
            assert_eq!(cache.resolve(0x1020, &memory), 0);
            assert_eq!(cache.pop_retired(), target);
            assert_ne!(cache.resolve(0x2000, &memory), 0);
            assert!(memory.execution_writes_pending());
            assert_eq!(
                memory.read(0x3000, 1, crate::memory::Access::Read).unwrap(),
                [0xcc]
            );
        }
    }

    #[test]
    fn outer_admission_and_completion_recheck_pending_alias_writes_after_shared_entries() {
        for completion in [false, true] {
            let mut memory = memory();
            memory.map_alias(0x3000, 0x1000, 1, READ | WRITE).unwrap();
            let mut cache = CodeCache::default();
            let mut candidate = unit(&memory, 0x1000, 4);
            candidate.entries.insert(0x1010);
            let id = cache.insert(candidate).unwrap();
            cache.bind(id, cache.link_slot(id), &memory).unwrap();
            cache.begin_invocation(id, 0x1000, &memory).unwrap();
            assert_ne!(cache.resolve(0x1010, &memory), 0);
            assert!(memory.write_small(0x3000, &[0xcc]).unwrap());
            if completion {
                // Failed read-only completion keeps the pending identity.
                assert!(cache.check_code_span(id, 0x1010, 1, &memory).is_err());
                assert!(cache.units.contains_key(&id));
                assert_eq!(cache.pop_retired(), 0);
            } else {
                assert!(cache.validate(id, 0x1010, &memory).is_err());
                assert_eq!(cache.pop_retired(), id);
            }
            assert!(memory.execution_writes_pending());
        }
    }

    #[test]
    fn invocation_identity_exhaustion_fails_without_recycling_a_proof() {
        let memory = memory();
        let mut cache = CodeCache::default();
        let id = cache.insert(unit(&memory, 0x1000, 4)).unwrap();
        cache.bind(id, cache.link_slot(id), &memory).unwrap();
        cache.begin_invocation(id, 0x1000, &memory).unwrap();
        cache.invocation_id = u64::MAX;
        assert!(
            cache
                .begin_invocation(id, 0x1000, &memory)
                .unwrap_err()
                .contains("identity exhaustion")
        );
        assert_eq!(cache.invocation_epoch, None);
        assert_eq!(cache.invocation_id, u64::MAX);
        assert_eq!(cache.pop_retired(), 0);
        assert_ne!(cache.resolve(0x1000, &memory), 0);
    }
}

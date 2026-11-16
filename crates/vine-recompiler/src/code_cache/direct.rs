//! Protected PC directory. Only Rust publishes bindings; generated code may
//! read them during a synchronous admitted invocation and update its counters.
use crate::memory::PAGE_COUNT;
use std::collections::BTreeMap;

// Header: active, last unit, hits, misses; then one pointer per guest page.
pub(super) const HEADER_WORDS: usize = 4;
const PAGE_ENTRIES: usize = 4096;
const PAGE_BYTES: usize = PAGE_ENTRIES * 8;
const MAXIMUM_BYTES: usize = 32 * 1024 * 1024;
const ROOT_BYTES: usize = (PAGE_COUNT + HEADER_WORDS) * 4;
const MAXIMUM_PAGES: usize = (MAXIMUM_BYTES - ROOT_BYTES) / PAGE_BYTES;

struct Page {
    cells: Box<[u64]>,
    occupied: usize,
}

pub(super) struct DirectLinks {
    root: Box<[u32]>,
    pages: BTreeMap<u32, Page>,
    capacity_misses: u32,
}

impl DirectLinks {
    pub fn new() -> Result<Self, String> {
        let mut root = Vec::new();
        root.try_reserve_exact(PAGE_COUNT + HEADER_WORDS)
            .map_err(|_| "direct link directory allocation failed")?;
        root.resize(PAGE_COUNT + HEADER_WORDS, 0);
        Ok(Self {
            root: root.into_boxed_slice(),
            pages: BTreeMap::new(),
            capacity_misses: 0,
        })
    }
    pub fn pointer(&self) -> usize {
        self.root.as_ptr() as usize
    }
    pub fn bytes(&self) -> usize {
        ROOT_BYTES + self.pages.len() * PAGE_BYTES
    }
    pub fn counts(&self) -> (u32, u32, u32) {
        (self.root[2], self.root[3], self.capacity_misses)
    }
    pub fn begin(&mut self, id: u32) {
        self.root[1] = id;
        self.root[0] = 1;
    }
    pub fn end(&mut self) -> Option<u32> {
        let was_active = self.root[0] != 0;
        self.root[0] = 0;
        (was_active && self.root[1] != 0).then_some(self.root[1])
    }
    pub fn selected(&mut self, id: u32) {
        self.root[1] = id;
    }
    pub fn publish(&mut self, entry: u32, binding: u64) {
        let index = entry >> 12;
        if !self.pages.contains_key(&index) {
            let mut cells = Vec::new();
            if self.pages.len() == MAXIMUM_PAGES || cells.try_reserve_exact(PAGE_ENTRIES).is_err() {
                // Optional optimization capacity. The checked resolver still
                // executes this PC, and the missed publication is observable.
                self.capacity_misses = self.capacity_misses.saturating_add(1);
                return;
            }
            cells.resize(PAGE_ENTRIES, 0);
            let cells = cells.into_boxed_slice();
            self.root[HEADER_WORDS + index as usize] = cells.as_ptr() as usize as u32;
            self.pages.insert(index, Page { cells, occupied: 0 });
        }
        let page = self
            .pages
            .get_mut(&index)
            .expect("allocated directory page");
        let cell = &mut page.cells[(entry & 4095) as usize];
        page.occupied += usize::from(*cell == 0);
        *cell = binding;
    }
    pub fn revoke(&mut self, entry: u32, id: u32) {
        let index = entry >> 12;
        let Some(page) = self.pages.get_mut(&index) else {
            return;
        };
        let cell = &mut page.cells[(entry & 4095) as usize];
        if (*cell >> 32) as u32 != id {
            return;
        }
        *cell = 0;
        page.occupied -= 1;
        if page.occupied == 0 {
            // Revoke the pointer before releasing its allocation. Root offsets
            // and allocated page offsets survive Wasm memory growth.
            self.root[HEADER_WORDS + index as usize] = 0;
            self.pages.remove(&index);
        }
    }
    pub fn clear_bindings(&mut self) {
        self.end();
        for &index in self.pages.keys() {
            self.root[HEADER_WORDS + index as usize] = 0;
        }
        self.pages.clear();
    }
    #[cfg(test)]
    pub fn active(&self) -> bool {
        self.root[0] != 0
    }
    #[cfg(test)]
    pub fn binding(&self, entry: u32) -> u64 {
        self.pages
            .get(&(entry >> 12))
            .map_or(0, |p| p.cells[(entry & 4095) as usize])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn directory_budget_is_bounded_and_retirement_recovers_sparse_pages() {
        let mut directory = DirectLinks::new().unwrap();
        let binding = (17u64 << 32) | 3;
        for page in 0..MAXIMUM_PAGES {
            directory.publish((page as u32) << 12, binding);
        }
        assert!(directory.bytes() <= MAXIMUM_BYTES);
        let extra = u32::MAX;
        directory.publish(extra, binding);
        assert_eq!(directory.binding(extra), 0);
        assert_eq!(directory.counts().2, 1);
        directory.revoke(0, 17);
        directory.publish(extra, binding);
        assert_eq!(directory.binding(extra), binding);
        assert!(directory.bytes() <= MAXIMUM_BYTES);
        directory.begin(17);
        directory.clear_bindings();
        assert!(!directory.active());
        assert_eq!(directory.bytes(), ROOT_BYTES);
        assert_eq!(directory.binding(extra), 0);
    }
}

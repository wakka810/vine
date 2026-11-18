use std::collections::{BTreeMap, BTreeSet};

use super::RuntimeStateError;

const MENU_LIMIT: usize = 4096;
const ITEM_LIMIT: usize = 65536;
const TEXT_LIMIT: usize = 8 * 1024 * 1024;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeMenuItem {
    pub kind: u32,
    pub state: u32,
    pub command: u32,
    pub submenu: u32,
    pub data: u32,
    pub text: Vec<u16>,
}

impl RuntimeMenuItem {
    pub fn apply(&mut self, mask: u32, incoming: Self) {
        if mask & 0x110 != 0 {
            self.kind = incoming.kind;
        }
        if mask & 1 != 0 {
            self.state = incoming.state;
        }
        if mask & 2 != 0 {
            self.command = incoming.command;
        }
        if mask & 4 != 0 {
            self.submenu = incoming.submenu;
        }
        if mask & 0x20 != 0 {
            self.data = incoming.data;
        }
        if mask & 0x50 != 0 {
            self.text = incoming.text;
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RuntimeMenu {
    popup: bool,
    items: Vec<RuntimeMenuItem>,
}

#[derive(Debug, Default)]
#[cfg_attr(test, derive(Clone, PartialEq, Eq))]
pub(super) struct RuntimeMenus {
    menus: BTreeMap<u32, RuntimeMenu>,
    item_count: usize,
    text_units: usize,
}

impl RuntimeMenus {
    pub(super) fn destroy_tree(&mut self, handle: u32) {
        if let Some(menu) = self.menus.remove(&handle) {
            for item in menu.items {
                self.item_count -= 1;
                self.text_units -= item.text.len();
                if item.submenu != 0 {
                    self.destroy_tree(item.submenu);
                }
            }
        }
    }
    pub fn create(&mut self, handle: u32, popup: bool) -> Result<(), RuntimeStateError> {
        if self.menus.len() >= MENU_LIMIT || self.menus.contains_key(&handle) || handle == 0 {
            return Err(RuntimeStateError::new(
                "menu identity or capacity is exhausted",
            ));
        }
        self.menus.insert(
            handle,
            RuntimeMenu {
                popup,
                items: Vec::new(),
            },
        );
        Ok(())
    }

    pub fn count(&self, handle: u32) -> Option<usize> {
        self.menus.get(&handle).map(|menu| menu.items.len())
    }

    pub fn is_popup(&self, handle: u32) -> Option<bool> {
        self.menus.get(&handle).map(|menu| menu.popup)
    }

    pub fn locate(&self, handle: u32, item: u32, by_position: bool) -> Option<(u32, usize)> {
        let menu = self.menus.get(&handle)?;
        if by_position {
            return menu
                .items
                .get(item as usize)
                .map(|_| (handle, item as usize));
        }
        let mut stack = vec![(handle, 0)];
        while let Some((handle, index)) = stack.pop() {
            let menu = self.menus.get(&handle)?;
            let Some(entry) = menu.items.get(index) else {
                continue;
            };
            if entry.command == item {
                return Some((handle, index));
            }
            stack.push((handle, index + 1));
            if entry.submenu != 0 {
                stack.push((entry.submenu, 0));
            }
        }
        None
    }

    pub fn get(&self, handle: u32, item: u32, by_position: bool) -> Option<&RuntimeMenuItem> {
        let (handle, index) = self.locate(handle, item, by_position)?;
        self.menus.get(&handle)?.items.get(index)
    }

    fn validate_link(
        &self,
        handle: u32,
        index: Option<usize>,
        submenu: u32,
    ) -> Result<(), RuntimeStateError> {
        if submenu == 0 {
            return Ok(());
        }
        if !self.menus.contains_key(&submenu) {
            return Err(RuntimeStateError::new(
                "submenu handle does not name a live menu",
            ));
        }
        for (&parent, menu) in &self.menus {
            if menu
                .items
                .iter()
                .enumerate()
                .any(|(i, item)| item.submenu == submenu && (parent != handle || Some(i) != index))
            {
                return Err(RuntimeStateError::new(
                    "submenu already belongs to another menu item",
                ));
            }
        }
        let mut stack = vec![submenu];
        let mut visited = BTreeSet::new();
        while let Some(child) = stack.pop() {
            if child == handle {
                return Err(RuntimeStateError::new(
                    "submenu attachment would create a cycle",
                ));
            }
            if !visited.insert(child) {
                continue;
            }
            let menu = self.menus.get(&child).expect("live submenu tree");
            stack.extend(
                menu.items
                    .iter()
                    .filter_map(|item| (item.submenu != 0).then_some(item.submenu)),
            );
        }
        Ok(())
    }

    pub fn put(
        &mut self,
        handle: u32,
        item: u32,
        by_position: bool,
        mask: u32,
        incoming: RuntimeMenuItem,
        insert: bool,
    ) -> Result<bool, RuntimeStateError> {
        let Some(menu) = self.menus.get(&handle) else {
            return Ok(false);
        };
        let (handle, index) = if insert {
            if by_position {
                (handle, (item as usize).min(menu.items.len()))
            } else {
                self.locate(handle, item, false)
                    .unwrap_or((handle, menu.items.len()))
            }
        } else {
            let Some(found) = self.locate(handle, item, by_position) else {
                return Ok(false);
            };
            found
        };
        let old = if insert {
            None
        } else {
            Some(&self.menus[&handle].items[index])
        };
        let mut next = old.cloned().unwrap_or_default();
        next.apply(mask, incoming);
        self.validate_link(handle, (!insert).then_some(index), next.submenu)?;
        let text_units =
            self.text_units - old.map_or(0, |entry| entry.text.len()) + next.text.len();
        let item_count = self.item_count + usize::from(insert);
        if next.text.len() > 4096 || text_units > TEXT_LIMIT || item_count > ITEM_LIMIT {
            return Err(RuntimeStateError::new(
                "menu text or item capacity is exhausted",
            ));
        }
        let menu = self.menus.get_mut(&handle).expect("validated menu");
        if insert {
            menu.items.insert(index, next);
        } else {
            menu.items[index] = next;
        }
        self.text_units = text_units;
        self.item_count = item_count;
        Ok(true)
    }

    pub fn remove(&mut self, handle: u32, item: u32, by_position: bool) -> bool {
        let Some((handle, index)) = self.locate(handle, item, by_position) else {
            return false;
        };
        let removed = self
            .menus
            .get_mut(&handle)
            .expect("validated menu")
            .items
            .remove(index);
        self.item_count -= 1;
        self.text_units -= removed.text.len();
        // RemoveMenu detaches the submenu; it does not destroy it.
        true
    }
}

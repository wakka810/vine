use std::collections::BTreeMap;

#[derive(Debug, Clone)]
struct OpenKey {
    root: u32,
    path: String,
    access: u32,
}

/// Process-local handles into the fixed Windows environment. No host registry
/// or user cursor customizations are imported into the guest.
#[derive(Debug)]
pub(crate) struct RuntimeRegistry {
    handles: BTreeMap<u32, OpenKey>,
    next_handle: u32,
}

impl Default for RuntimeRegistry {
    fn default() -> Self {
        Self {
            handles: BTreeMap::new(),
            next_handle: 0x5201_0001,
        }
    }
}

impl RuntimeRegistry {
    fn predefined(handle: u32) -> bool {
        matches!(handle, 0x8000_0000..=0x8000_0003 | 0x8000_0005)
    }

    fn key(&self, handle: u32) -> Result<OpenKey, u32> {
        if Self::predefined(handle) {
            return Ok(OpenKey {
                root: handle,
                path: String::new(),
                access: 0x20019,
            });
        }
        self.handles.get(&handle).cloned().ok_or(6)
    }

    pub(crate) fn open(&mut self, parent: u32, subkey: &str, access: u32) -> Result<u32, u32> {
        let mut key = self.key(parent)?;
        // This environment exposes read-only configuration. Writes must not
        // acquire a handle that claims authority to persist registry changes.
        if access & !0x20019 != 0 {
            return Err(5);
        }
        if subkey.is_empty() && Self::predefined(parent) {
            return Ok(parent);
        }
        if subkey.starts_with('\\') || subkey.ends_with('\\') || subkey.contains("\\\\") {
            return Err(161);
        }
        if !subkey.is_empty() {
            if !key.path.is_empty() {
                key.path.push('\\');
            }
            key.path.push_str(&subkey.to_lowercase());
        }
        let exists = key.path.is_empty()
            || (key.root == 0x8000_0001
                && matches!(
                    key.path.as_str(),
                    "control panel" | "control panel\\cursors"
                ));
        if !exists {
            return Err(2);
        }
        if self.handles.len() >= 4096 || self.next_handle >= 0x5300_0000 {
            return Err(8);
        }
        key.access = access;
        let handle = self.next_handle;
        self.next_handle += 1;
        self.handles.insert(handle, key);
        Ok(handle)
    }

    pub(crate) fn query(&self, handle: u32) -> Result<(), u32> {
        if self.key(handle)?.access & 1 == 0 {
            return Err(5);
        }
        // The fixed environment uses built-in cursors, with no per-user
        // overrides or unnamed values in these keys.
        Err(2)
    }

    pub(crate) fn close(&mut self, handle: u32) -> u32 {
        if Self::predefined(handle) || self.handles.remove(&handle).is_some() {
            0
        } else {
            6
        }
    }
}

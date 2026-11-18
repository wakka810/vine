use std::collections::BTreeMap;
use std::sync::Arc;

use super::RuntimeStateError;
use crate::memory::{GuestAddress, GuestMemory};

pub(crate) const FONT_RESOURCE_MAX_BYTES: usize = 16 * 1024 * 1024;
const FONT_RESOURCE_TOTAL_BYTES: usize = 64 * 1024 * 1024;
const FONT_RESOURCE_LIMIT: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeFontResources {
    // These private registrations are separate from selectable GDI objects.
    // Retain a copy because callers may free their source immediately.
    resources: BTreeMap<u32, Arc<[u8]>>,
    byte_length: usize,
    next_handle: u32,
}

impl Default for RuntimeFontResources {
    fn default() -> Self {
        Self {
            resources: BTreeMap::new(),
            byte_length: 0,
            next_handle: 0x4703_0001,
        }
    }
}

impl RuntimeFontResources {
    pub(super) fn add(
        &mut self,
        memory: &mut GuestMemory,
        bytes: Vec<u8>,
        count_output: GuestAddress,
    ) -> Result<Option<u32>, RuntimeStateError> {
        if bytes.len() > FONT_RESOURCE_MAX_BYTES
            || self.resources.len() >= FONT_RESOURCE_LIMIT
            || bytes.len() > FONT_RESOURCE_TOTAL_BYTES - self.byte_length
            || self.next_handle > 0x4703_ffff
        {
            return Ok(None);
        }
        let Some(count) = font_count(&bytes) else {
            return Ok(None);
        };
        let Some(next_handle) = self.next_handle.checked_add(1) else {
            return Ok(None);
        };
        let handle = self.next_handle;
        let bytes: Arc<[u8]> = bytes.into();
        // The only fallible mutation precedes publication of the registration.
        memory
            .write(count_output, &count.to_le_bytes())
            .map_err(|error| RuntimeStateError::new(format!("font count output: {error}")))?;
        self.byte_length += bytes.len();
        self.resources.insert(handle, bytes);
        self.next_handle = next_handle;
        Ok(Some(handle))
    }

    pub(super) fn remove(&mut self, handle: u32) -> bool {
        let Some(bytes) = self.resources.remove(&handle) else {
            return false;
        };
        self.byte_length -= bytes.len();
        true
    }
}

fn font_count(bytes: &[u8]) -> Option<u32> {
    let face_count = ttf_parser::fonts_in_collection(bytes).unwrap_or(1);
    if !(1..=64).contains(&face_count) {
        return None;
    }
    let mut count = 0;
    for index in 0..face_count {
        let face = ttf_parser::Face::parse(bytes, index).ok()?;
        let raw = face.raw_face();
        if raw.table_records.len() > 256 {
            return None;
        }
        let mut previous_tag = None;
        for record in raw.table_records {
            let end = record.offset.checked_add(record.length)? as usize;
            if end > bytes.len() || previous_tag.is_some_and(|tag| tag >= record.tag) {
                return None;
            }
            previous_tag = Some(record.tag);
        }
        let tables = face.tables();
        if tables.cmap.is_none()
            || tables.name.is_none()
            || tables.hmtx.is_none()
            || (tables.glyf.is_none() && tables.cff.is_none())
        {
            return None;
        }
        // Windows publishes an additional @ face for the East Asian code pages,
        // including fonts whose vertical metrics are synthesized from hmtx.
        let vertical = raw
            .table(ttf_parser::Tag::from_bytes(b"OS/2"))
            .filter(|table| {
                tables.os2.is_some() && table.get(..2).is_some_and(|version| version != [0, 0])
            })
            .and_then(|table| table.get(78..82))
            .map(|data| u32::from_be_bytes(data.try_into().expect("four-byte code page field")))
            .is_some_and(|code_pages| code_pages & 0x003e_0000 != 0);
        count += 1 + u32::from(vertical);
    }
    Some(count)
}

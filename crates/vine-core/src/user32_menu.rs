use super::{
    ApiCallFrame, ApiEndpointIdentity, ApiFault, ApiImmediate, GuestAddress, MemoryAccess,
    required_argument,
};
use crate::runtime::{Runtime, RuntimeMenuItem, RuntimeWindowLifecycle};

const INVALID_MENU: u32 = 1401;
const ITEM_NOT_FOUND: u32 = 1456;

fn failure(error: u32) -> ApiImmediate {
    ApiImmediate::new(Some(0)).with_last_error(error)
}

fn menu_info(
    endpoint: &ApiEndpointIdentity,
    runtime: &Runtime,
    address: GuestAddress,
) -> Result<Option<Vec<u8>>, ApiFault> {
    let header = runtime
        .memory
        .read(address, 4)
        .map_err(|e| ApiFault::from_memory(e, endpoint))?;
    if u32::from_le_bytes(header.try_into().unwrap()) != 48 {
        return Ok(None);
    }
    let bytes = runtime
        .memory
        .read(address, 48)
        .map_err(|e| ApiFault::from_memory(e, endpoint))?;
    let mask = word(&bytes, 4);
    if mask & !0x1ff != 0 || (mask & 0x10 != 0 && mask & 0x1c0 != 0) {
        return Ok(None);
    }
    Ok(Some(bytes))
}

fn word(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

fn put_word(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

pub(super) fn handle(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut Runtime,
    name: &str,
) -> Result<ApiImmediate, ApiFault> {
    if matches!(name, "CreateMenu" | "CreatePopupMenu") {
        let handle = runtime
            .create_menu(name == "CreatePopupMenu")
            .map_err(|e| ApiFault::from_runtime(e, endpoint))?;
        return Ok(ApiImmediate::new(Some(handle)));
    }
    let handle = required_argument(endpoint, frame, 0, "window or menu")?;
    if matches!(name, "SetMenu" | "GetMenu" | "DrawMenuBar") {
        let Some(window) = runtime
            .window_option(handle)
            .map_err(|e| ApiFault::from_runtime(e, endpoint))?
        else {
            return Ok(failure(1400));
        };
        if name == "GetMenu" {
            return Ok(ApiImmediate::new(Some(window.menu_handle)));
        }
        if name == "DrawMenuBar" {
            if window.lifecycle != RuntimeWindowLifecycle::Created
                || window.creation.owner_thread_id != runtime.current_thread_id()
            {
                return Err(ApiFault::new(
                    "menu redraw requires a created owner-thread window",
                ));
            }
            if window.visible && window.menu_handle != 0 {
                return Err(ApiFault::new(
                    "visible menu bar rendering is not implemented",
                ));
            }
            // A hidden window has no menu pixels to repaint. The Rust menu tree
            // remains available for queries and future attachment.
            return Ok(ApiImmediate::new(Some(1)));
        }
        let menu = required_argument(endpoint, frame, 1, "menu")?;
        if menu != 0 && runtime.menu_count(menu).is_none() {
            return Ok(failure(INVALID_MENU));
        }
        runtime
            .set_menu(handle, menu)
            .map_err(|e| ApiFault::from_runtime(e, endpoint))?;
        return Ok(ApiImmediate::new(Some(1)));
    }
    let Some(count) = runtime.menu_count(handle) else {
        return Ok(ApiImmediate::new(Some(if name == "GetMenuItemCount" {
            u32::MAX
        } else {
            0
        }))
        .with_last_error(INVALID_MENU));
    };
    if name == "GetMenuItemCount" {
        return Ok(ApiImmediate::new(Some(count as u32)));
    }
    let item = required_argument(endpoint, frame, 1, "menu item")?;
    let flags = required_argument(endpoint, frame, 2, "item selection")?;
    if name == "RemoveMenu" {
        if flags & !0x400 != 0 {
            return Err(ApiFault::new("RemoveMenu flags are not implemented"));
        }
        let removed = runtime
            .remove_menu_item(handle, item, flags != 0)
            .map_err(|e| ApiFault::from_runtime(e, endpoint))?;
        return Ok(if removed {
            ApiImmediate::new(Some(1))
        } else {
            failure(ITEM_NOT_FOUND)
        });
    }
    let address = GuestAddress(required_argument(
        endpoint,
        frame,
        3,
        "menu item information",
    )?);
    let Some(mut bytes) = menu_info(endpoint, runtime, address)? else {
        return Ok(failure(87));
    };
    let mask = word(&bytes, 4);
    let by_position = flags != 0;
    if name == "GetMenuItemInfoW" {
        let Some(entry) = runtime.menu_item(handle, item, by_position) else {
            return Ok(failure(ITEM_NOT_FOUND));
        };
        let text_address = GuestAddress(word(&bytes, 36));
        let capacity = word(&bytes, 40) as usize;
        let mut result = ApiImmediate::new(Some(1));
        for (bit, offset, value) in [
            (0x110, 8, entry.kind),
            (1, 12, entry.state),
            (2, 16, entry.command),
            (4, 20, entry.submenu),
            (0x20, 32, entry.data),
            (8, 24, 0),
            (8, 28, 0),
            (0x80, 44, 0),
        ] {
            if mask & bit != 0 {
                put_word(&mut bytes, offset, value);
            }
        }
        if mask & 0x50 != 0 {
            let length = if entry.kind & 0x800 != 0 {
                0
            } else {
                entry.text.len()
            };
            let copied = if text_address.0 == 0 {
                length
            } else {
                length.min(capacity.saturating_sub(1))
            };
            if text_address.0 != 0 && capacity != 0 {
                let text: Vec<u8> = entry.text[..copied]
                    .iter()
                    .copied()
                    .chain([0])
                    .flat_map(u16::to_le_bytes)
                    .collect();
                let text_end = u64::from(text_address.0) + text.len() as u64;
                if u64::from(text_address.0) < u64::from(address.0) + 48
                    && u64::from(address.0) < text_end
                {
                    return Err(ApiFault::new(
                        "menu item text overlaps its output structure",
                    ));
                }
                runtime
                    .memory
                    .validate_access(text_address, text.len(), MemoryAccess::Write)
                    .map_err(|e| ApiFault::from_memory(e, endpoint))?;
                result = result.with_guest_write(text_address, text);
            }
            put_word(&mut bytes, 40, copied as u32);
        }
        runtime
            .memory
            .validate_access(address, 48, MemoryAccess::Write)
            .map_err(|e| ApiFault::from_memory(e, endpoint))?;
        return Ok(result.with_guest_write(address, bytes));
    }
    let kind = if mask & 0x110 != 0 {
        word(&bytes, 8)
    } else {
        0
    };
    let state = if mask & 1 != 0 { word(&bytes, 12) } else { 0 };
    if kind & !0x6a60 != 0
        || state & !0x108b != 0
        || (mask & 8 != 0 && (word(&bytes, 24) != 0 || word(&bytes, 28) != 0))
        || (mask & 0x80 != 0 && word(&bytes, 44) != 0)
    {
        return Err(ApiFault::new(
            "menu item drawing, state, or bitmap variant is not implemented",
        ));
    }
    let submenu = if mask & 4 != 0 { word(&bytes, 20) } else { 0 };
    if submenu != 0 && runtime.menu_count(submenu).is_none() {
        return Ok(failure(INVALID_MENU));
    }
    let text_address = GuestAddress(word(&bytes, 36));
    let text = if mask & 0x50 != 0 && kind & 0x800 == 0 && text_address.0 != 0 {
        crate::win32::read_utf16_nul(endpoint, &runtime.memory, text_address, 4097)?
            .encode_utf16()
            .collect()
    } else {
        Vec::new()
    };
    let incoming = RuntimeMenuItem {
        kind,
        state,
        command: word(&bytes, 16),
        submenu,
        data: word(&bytes, 32),
        text,
    };
    let changed = runtime
        .put_menu_item(
            handle,
            item,
            by_position,
            mask,
            incoming,
            name == "InsertMenuItemW",
        )
        .map_err(|e| ApiFault::from_runtime(e, endpoint))?;
    Ok(if changed {
        ApiImmediate::new(Some(1))
    } else {
        failure(ITEM_NOT_FOUND)
    })
}

use std::collections::BTreeMap;

use encoding_rs::SHIFT_JIS;

use crate::memory::GuestAddress;
use crate::module_graph::RuntimeModuleId;
use crate::pe32::Rva;

use super::gdi::RuntimeDeviceContextId;
use super::{RuntimeRect, RuntimeStateError, RuntimeThreadId};

pub const RUNTIME_SHARED_ICON_LIMIT: usize = 4_096;
pub const RUNTIME_SHARED_CURSOR_LIMIT: usize = 256;
pub const RUNTIME_WINDOW_CLASS_LIMIT: usize = 4_096;
pub const RUNTIME_WINDOW_CLASS_NAME_A_LIMIT: usize = 256;
pub const RUNTIME_WINDOW_LIMIT: usize = 4_096;
pub const RUNTIME_SYSTEM_MENU_LIMIT: usize = RUNTIME_WINDOW_LIMIT;
pub const RUNTIME_DEFAULT_IME_WINDOW_LIMIT: usize = 4_096;
pub const RUNTIME_WINDOW_NAME_A_LIMIT: usize = 512;
pub const RUNTIME_ICON_HANDLE_BASE: u32 = 0x4901_0001;
pub const RUNTIME_CURSOR_HANDLE_BASE: u32 = 0x4301_0001;
pub const RUNTIME_MENU_HANDLE_BASE: u32 = 0x4e01_0001;
pub const RUNTIME_WINDOW_HANDLE_BASE: u32 = 0x4801_0001;
pub const RUNTIME_SYSTEM_CURSOR_ARROW_RESOURCE_ID: u32 = 0x7f00;
pub const RUNTIME_WINDOW_CLASS_ATOM_BASE: u16 = 0xc000;
pub const RUNTIME_X86_PAINT_STRUCT_BYTE_LENGTH: usize = 64;
pub const RUNTIME_WINDOW_EXTENDED_STYLE_ACCEPT_FILES: u32 = 0x0000_0010;
pub const RUNTIME_WINDOW_CLASS_STYLE_DOUBLE_CLICKS: u32 = 0x0000_0008;
pub const RUNTIME_WINDOW_STYLE_VISIBLE: u32 = 0x1000_0000;
pub const RUNTIME_WINDOW_STYLE_MINIMIZED: u32 = 0x2000_0000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct RuntimeIconId(pub u64);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct RuntimeIconResource {
    pub module_id: RuntimeModuleId,
    pub module_load_base: GuestAddress,
    pub group_resource_id: u32,
    pub language_id: u32,
    pub icon_resource_id: u32,
    pub group_data_rva: Rva,
    pub group_data_size: u32,
    pub icon_data_rva: Rva,
    pub icon_data_size: u32,
    pub width: u16,
    pub height: u16,
    pub planes: u16,
    pub bits_per_pixel: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeIcon {
    pub id: RuntimeIconId,
    pub handle: u32,
    pub shared: bool,
    pub resource: RuntimeIconResource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct RuntimeCursorId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RuntimeSystemCursorKind {
    Arrow,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct RuntimeCursorResource {
    pub module_id: RuntimeModuleId,
    pub module_load_base: GuestAddress,
    pub group_resource_id: u32,
    pub language_id: u32,
    pub cursor_resource_id: u32,
    pub group_data_rva: Rva,
    pub group_data_size: u32,
    pub cursor_data_rva: Rva,
    pub cursor_data_size: u32,
    pub width: u16,
    pub height: u16,
    pub planes: u16,
    pub bits_per_pixel: u16,
    pub hotspot_x: u16,
    pub hotspot_y: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum RuntimeCursorSource {
    SystemPredefined {
        resource_id: u32,
        cursor: RuntimeSystemCursorKind,
    },
    ModuleResource {
        resource: RuntimeCursorResource,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeCursor {
    pub id: RuntimeCursorId,
    pub handle: u32,
    pub shared: bool,
    pub source: RuntimeCursorSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct RuntimeWindowClassId(pub u64);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct RuntimeWindowClassModule {
    pub module_id: RuntimeModuleId,
    pub load_base: GuestAddress,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct RuntimeWindowProcedure {
    pub module_id: RuntimeModuleId,
    pub address: GuestAddress,
    pub rva: Rva,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct RuntimeWindowClassIconReference {
    pub icon_id: RuntimeIconId,
    pub handle: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct RuntimeWindowClassCursorReference {
    pub cursor_id: RuntimeCursorId,
    pub handle: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeWindowClassRegistration {
    pub module: RuntimeWindowClassModule,
    pub style: u32,
    pub window_procedure: RuntimeWindowProcedure,
    pub class_extra_bytes: u32,
    pub window_extra_bytes: u32,
    pub icon: Option<RuntimeWindowClassIconReference>,
    pub cursor: Option<RuntimeWindowClassCursorReference>,
    pub background_brush: u32,
    pub menu_name: Option<String>,
    pub class_name: String,
    pub small_icon: Option<RuntimeWindowClassIconReference>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeWindowClass {
    pub id: RuntimeWindowClassId,
    pub atom: u16,
    pub local: bool,
    pub registration: RuntimeWindowClassRegistration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct RuntimeWindowId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RuntimeWindowCreationMessage {
    NonClientCreate,
    NonClientCalculate,
    Create,
    Size,
    Move,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeWindowLifecycle {
    Creating {
        message: RuntimeWindowCreationMessage,
    },
    Created,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeWindowClassReference {
    pub class_id: RuntimeWindowClassId,
    pub atom: u16,
    pub module_id: RuntimeModuleId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeWindowNameA {
    pub value: String,
}

impl RuntimeWindowNameA {
    pub fn windows_932_bytes(&self) -> Result<Vec<u8>, RuntimeStateError> {
        if self.value.contains('\0') {
            return Err(RuntimeStateError::new(
                "logical window title requires copied Windows-932 text without embedded NULs",
            ));
        }
        let (bytes, _, had_errors) = SHIFT_JIS.encode(&self.value);
        if had_errors || bytes.len() > RUNTIME_WINDOW_NAME_A_LIMIT {
            return Err(RuntimeStateError::new(
                "logical window title must be representable within the bounded Windows-932 profile",
            ));
        }
        Ok(bytes.into_owned())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeWindowGeometry {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub client_x: i32,
    pub client_y: i32,
    pub client_width: u32,
    pub client_height: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeWindowIconReference {
    pub icon_id: RuntimeIconId,
    pub handle: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeSetWindowPosVariant {
    InitialShow,
    InitialHide,
    ActivateSameGeometry,
    RepeatActivation,
    Resize,
    FrameChangeMove,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeSetWindowPosCallbackMessage {
    WindowPositionChanged,
    GetSmallIcon,
    WindowPositionChanging,
    NonClientCalculate,
    Size,
    ActivationWindowPositionChanging,
    ActivateApplication,
    NonClientActivate,
    Activate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeSetWindowPosDefaultCallbackMessage {
    ImeSetContext,
    SetFocus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeSetWindowPosRequest {
    pub window_handle: u32,
    pub variant: RuntimeSetWindowPosVariant,
    pub next_geometry: RuntimeWindowGeometry,
    pub next_visible: bool,
    pub next_topmost: bool,
    pub entry_last_error: u32,
    pub callback_flags: u32,
    pub callback_insert_after: u32,
    pub activation_insert_after: u32,
    pub activation_internal_flags: u32,
    pub resize_nccalc_window_pos_flags: u32,
    pub resize_changed_flags: u32,
    pub frame_change_nccalc_window_pos_flags: u32,
    pub frame_change_changed_flags: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeSetWindowPosOperation {
    pub window_id: RuntimeWindowId,
    pub window_handle: u32,
    pub expected_generation: u64,
    pub variant: RuntimeSetWindowPosVariant,
    pub previous_geometry: RuntimeWindowGeometry,
    pub next_geometry: RuntimeWindowGeometry,
    pub previous_visible: bool,
    pub next_visible: bool,
    pub previous_topmost: bool,
    pub next_topmost: bool,
    pub previous_effective_style: u32,
    pub next_effective_style: u32,
    pub previous_update_rect: Option<RuntimeRect>,
    pub next_update_rect: Option<RuntimeRect>,
    pub entry_last_error: u32,
    pub callback_flags: u32,
    pub callback_insert_after: u32,
    pub activation_insert_after: u32,
    pub activation_internal_flags: u32,
    pub resize_nccalc_window_pos_flags: u32,
    pub resize_changed_flags: u32,
    pub frame_change_nccalc_window_pos_flags: u32,
    pub frame_change_changed_flags: u32,
    pub small_icon_handle: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeUpdateWindowOperation {
    pub window_id: RuntimeWindowId,
    pub window_handle: u32,
    pub owner_thread_id: RuntimeThreadId,
    pub expected_paint_generation: u64,
    pub update_rect: RuntimeRect,
    pub entry_last_error: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimePaintSession {
    pub window_id: RuntimeWindowId,
    pub window_handle: u32,
    pub owner_thread_id: RuntimeThreadId,
    pub paint_generation: u64,
    pub update_rect: RuntimeRect,
    pub device_context_id: RuntimeDeviceContextId,
    pub device_context_handle: u32,
    pub paint_struct_address: GuestAddress,
    pub paint_struct_bytes: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeSetWindowPosDefaultOperation {
    pub window_id: RuntimeWindowId,
    pub window_handle: u32,
    pub owner_thread_id: RuntimeThreadId,
    pub expected_generation: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeWindowCreationRequest {
    pub owner_thread_id: RuntimeThreadId,
    pub class: RuntimeWindowClassReference,
    pub instance: RuntimeWindowClassModule,
    pub name: RuntimeWindowNameA,
    pub extended_style: u32,
    pub effective_extended_style: u32,
    pub style: u32,
    pub effective_style: u32,
    pub geometry: RuntimeWindowGeometry,
    pub parent: u32,
    pub menu: u32,
    pub create_parameter: u32,
    pub entry_last_error: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeWindow {
    pub id: RuntimeWindowId,
    pub handle: u32,
    pub creation: RuntimeWindowCreationRequest,
    pub lifecycle: RuntimeWindowLifecycle,
    pub visible: bool,
    pub topmost: bool,
    pub pending_update_rect: Option<RuntimeRect>,
    pub paint_generation: u64,
    pub small_icon: Option<RuntimeWindowIconReference>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct RuntimeMenuId(pub u64);

#[derive(Debug)]
#[cfg_attr(test, derive(Clone, PartialEq, Eq))]
pub struct RuntimeSystemMenu {
    pub id: RuntimeMenuId,
    pub handle: u32,
    pub owner_window_id: RuntimeWindowId,
    pub owner_window_handle: u32,
    pub close_item_command: u32,
    pub close_item_enabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct RuntimeImeWindowId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeImeStatusWindowRequest {
    Close,
    Open,
}

pub const RUNTIME_MESSAGE_BOX_SERVICE: &str = "browser.message-box";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeMessageBoxIcon {
    None,
    Error,
    Question,
    Warning,
    Information,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeMessageBoxButton {
    pub id: u32,
    pub label: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeMessageBox {
    pub title: String,
    pub text: String,
    pub buttons: Vec<RuntimeMessageBoxButton>,
    pub default_button: u32,
    pub cancel_button: Option<u32>,
    pub icon: RuntimeMessageBoxIcon,
}

impl RuntimeMessageBox {
    pub(super) fn new(title: String, text: String, flags: u32) -> Result<Self, RuntimeStateError> {
        // App-modal and system-modal have the same process-local blocking
        // behavior here; Chrome's modal surface is already above the game.
        if flags & !0x137f != 0 || flags & 0x3000 == 0x2000 {
            return Err(RuntimeStateError::new(
                "MessageBox flags are not implemented",
            ));
        }
        let ids: &[u32] = match flags & 0xf {
            0 => &[1],
            1 => &[1, 2],
            2 => &[3, 4, 5],
            3 => &[6, 7, 2],
            4 => &[6, 7],
            5 => &[4, 2],
            6 => &[2, 10, 11],
            _ => {
                return Err(RuntimeStateError::new(
                    "MessageBox button type is not implemented",
                ));
            }
        };
        let icon = match flags & 0xf0 {
            0 => RuntimeMessageBoxIcon::None,
            0x10 => RuntimeMessageBoxIcon::Error,
            0x20 => RuntimeMessageBoxIcon::Question,
            0x30 => RuntimeMessageBoxIcon::Warning,
            0x40 => RuntimeMessageBoxIcon::Information,
            _ => return Err(RuntimeStateError::new("MessageBox icon is not implemented")),
        };
        let default_button = *ids
            .get(((flags & 0x300) >> 8) as usize)
            .ok_or_else(|| RuntimeStateError::new("MessageBox default button does not exist"))?;
        let buttons = ids
            .iter()
            .map(|id| RuntimeMessageBoxButton {
                id: *id,
                label: match id {
                    1 => "OK",
                    2 => "キャンセル",
                    3 => "中止",
                    4 => "再試行",
                    5 => "無視",
                    6 => "はい",
                    7 => "いいえ",
                    10 => "再実行",
                    11 => "続行",
                    _ => unreachable!(),
                },
            })
            .collect();
        let cancel_button = if ids.contains(&2) {
            Some(2)
        } else if flags & 0xf == 0 {
            Some(1)
        } else {
            None
        };
        Ok(Self {
            title,
            text,
            buttons,
            default_button,
            cancel_button,
            icon,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeImeWindow {
    pub id: RuntimeImeWindowId,
    pub handle: u32,
    pub owner_thread_id: RuntimeThreadId,
    pub class_name: String,
    pub status_window_request: Option<RuntimeImeStatusWindowRequest>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeWindowCreationCallbackAction {
    Continue {
        window: RuntimeWindow,
        next_message: RuntimeWindowCreationMessage,
    },
    Created {
        window: RuntimeWindow,
    },
    Aborted {
        window: RuntimeWindow,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeSetWindowPosCallbackAction {
    Continue {
        window: RuntimeWindow,
        operation: RuntimeSetWindowPosOperation,
        next_message: RuntimeSetWindowPosCallbackMessage,
    },
    Completed {
        window: RuntimeWindow,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeSetWindowPosDefaultCallbackAction {
    Continue {
        operation: RuntimeSetWindowPosDefaultOperation,
        next_message: RuntimeSetWindowPosDefaultCallbackMessage,
    },
    Completed,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct RuntimeWindowClassKey {
    module_id: RuntimeModuleId,
    canonical_name: String,
}

#[derive(Debug)]
enum RuntimeWindowCreationCallbackDisposition {
    Update {
        next: Box<RuntimeWindow>,
        action: RuntimeWindowCreationCallbackAction,
        next_generation: u64,
    },
    Remove {
        window_id: RuntimeWindowId,
        window_handle: u32,
        action: RuntimeWindowCreationCallbackAction,
        next_generation: u64,
    },
}

#[derive(Debug)]
pub(super) enum RuntimeSetWindowPosCallbackDisposition {
    Validate {
        action: RuntimeSetWindowPosCallbackAction,
    },
    Update {
        next: Box<RuntimeWindow>,
        action: RuntimeSetWindowPosCallbackAction,
        next_generation: u64,
    },
    Activate {
        window_id: RuntimeWindowId,
        owner_thread_id: RuntimeThreadId,
        action: RuntimeSetWindowPosCallbackAction,
        next_generation: u64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RuntimeImeAppWindowTarget {
    pub window_id: RuntimeWindowId,
    pub window_handle: u32,
    pub owner_thread_id: RuntimeThreadId,
}

#[derive(Debug)]
#[cfg_attr(test, derive(Clone, PartialEq, Eq))]
pub(super) struct RuntimeUser32 {
    icons: BTreeMap<RuntimeIconId, RuntimeIcon>,
    resource_index: BTreeMap<RuntimeIconResource, RuntimeIconId>,
    handle_index: BTreeMap<u32, RuntimeIconId>,
    cursors: BTreeMap<RuntimeCursorId, RuntimeCursor>,
    cursor_source_index: BTreeMap<RuntimeCursorSource, RuntimeCursorId>,
    cursor_handle_index: BTreeMap<u32, RuntimeCursorId>,
    current_cursor: Option<RuntimeCursorId>,
    window_classes: BTreeMap<RuntimeWindowClassId, RuntimeWindowClass>,
    window_class_name_index: BTreeMap<RuntimeWindowClassKey, RuntimeWindowClassId>,
    window_class_atom_index: BTreeMap<u16, RuntimeWindowClassId>,
    windows: BTreeMap<RuntimeWindowId, RuntimeWindow>,
    window_handle_index: BTreeMap<u32, RuntimeWindowId>,
    paint_sessions: BTreeMap<RuntimeWindowId, RuntimePaintSession>,
    system_menus: BTreeMap<RuntimeMenuId, RuntimeSystemMenu>,
    system_menu_window_index: BTreeMap<RuntimeWindowId, RuntimeMenuId>,
    menu_handle_index: BTreeMap<u32, RuntimeMenuId>,
    ime_windows: BTreeMap<RuntimeImeWindowId, RuntimeImeWindow>,
    ime_window_handle_index: BTreeMap<u32, RuntimeImeWindowId>,
    default_ime_windows: BTreeMap<RuntimeThreadId, RuntimeImeWindowId>,
    active_windows: BTreeMap<RuntimeThreadId, RuntimeWindowId>,
    focused_windows: BTreeMap<RuntimeThreadId, RuntimeWindowId>,
    foreground_window: Option<RuntimeWindowId>,
    next_icon_id: u64,
    next_icon_handle: u32,
    next_cursor_id: u64,
    next_cursor_handle: u32,
    next_window_class_id: u64,
    next_window_class_atom: u32,
    next_window_id: u64,
    next_menu_id: u64,
    next_menu_handle: u32,
    next_ime_window_id: u64,
    next_window_handle: u32,
    generation: u64,
}

impl Default for RuntimeUser32 {
    fn default() -> Self {
        Self {
            icons: BTreeMap::new(),
            resource_index: BTreeMap::new(),
            handle_index: BTreeMap::new(),
            cursors: BTreeMap::new(),
            cursor_source_index: BTreeMap::new(),
            cursor_handle_index: BTreeMap::new(),
            current_cursor: None,
            window_classes: BTreeMap::new(),
            window_class_name_index: BTreeMap::new(),
            window_class_atom_index: BTreeMap::new(),
            windows: BTreeMap::new(),
            window_handle_index: BTreeMap::new(),
            paint_sessions: BTreeMap::new(),
            system_menus: BTreeMap::new(),
            system_menu_window_index: BTreeMap::new(),
            menu_handle_index: BTreeMap::new(),
            ime_windows: BTreeMap::new(),
            ime_window_handle_index: BTreeMap::new(),
            default_ime_windows: BTreeMap::new(),
            active_windows: BTreeMap::new(),
            focused_windows: BTreeMap::new(),
            foreground_window: None,
            next_icon_id: 1,
            next_icon_handle: RUNTIME_ICON_HANDLE_BASE,
            next_cursor_id: 1,
            next_cursor_handle: RUNTIME_CURSOR_HANDLE_BASE,
            next_window_class_id: 1,
            next_window_class_atom: u32::from(RUNTIME_WINDOW_CLASS_ATOM_BASE),
            next_window_id: 1,
            next_menu_id: 1,
            next_menu_handle: RUNTIME_MENU_HANDLE_BASE,
            next_ime_window_id: 1,
            next_window_handle: RUNTIME_WINDOW_HANDLE_BASE,
            generation: 0,
        }
    }
}

impl RuntimeUser32 {
    pub(super) fn load_shared_icon(
        &mut self,
        resource: RuntimeIconResource,
    ) -> Result<u32, RuntimeStateError> {
        validate_icon_resource(&resource)?;
        if let Some(id) = self.resource_index.get(&resource) {
            let icon = self.icons.get(id).ok_or_else(|| {
                RuntimeStateError::new("shared icon resource index does not resolve to a live icon")
            })?;
            if icon.id != *id
                || icon.resource != resource
                || self.handle_index.get(&icon.handle) != Some(id)
            {
                return Err(RuntimeStateError::new(
                    "shared icon handle and resource indexes do not match their live identity",
                ));
            }
            return Ok(icon.handle);
        }
        if self.icons.len() >= RUNTIME_SHARED_ICON_LIMIT {
            return Err(RuntimeStateError::new(
                "process shared icon count exceeds the runtime bound",
            ));
        }
        let id = RuntimeIconId(self.next_icon_id);
        if id.0 == 0 || self.icons.contains_key(&id) {
            return Err(RuntimeStateError::new(
                "shared icon allocator selected an invalid or live identity",
            ));
        }
        if self.next_icon_handle < RUNTIME_ICON_HANDLE_BASE
            || self.handle_index.contains_key(&self.next_icon_handle)
        {
            return Err(RuntimeStateError::new(
                "shared icon allocator selected an invalid or live handle",
            ));
        }
        let next_icon_id = self
            .next_icon_id
            .checked_add(1)
            .ok_or_else(|| RuntimeStateError::new("shared icon ID space is exhausted"))?;
        let next_icon_handle = self
            .next_icon_handle
            .checked_add(1)
            .ok_or_else(|| RuntimeStateError::new("shared icon handle space is exhausted"))?;
        let next_generation = self.next_generation()?;
        let icon = RuntimeIcon {
            id,
            handle: self.next_icon_handle,
            shared: true,
            resource,
        };
        let handle = icon.handle;
        // One synchronous owner; publication follows every fallible identity check.
        self.next_icon_id = next_icon_id;
        self.next_icon_handle = next_icon_handle;
        self.generation = next_generation;
        let previous_resource = self.resource_index.insert(icon.resource.clone(), icon.id);
        let previous_handle = self.handle_index.insert(icon.handle, icon.id);
        let previous_icon = self.icons.insert(icon.id, icon);
        debug_assert!(previous_resource.is_none());
        debug_assert!(previous_handle.is_none());
        debug_assert!(previous_icon.is_none());
        Ok(handle)
    }

    pub(super) fn load_shared_cursor(
        &mut self,
        source: RuntimeCursorSource,
    ) -> Result<u32, RuntimeStateError> {
        validate_cursor_source(&source)?;
        if let Some(id) = self.cursor_source_index.get(&source) {
            let cursor = self.cursors.get(id).ok_or_else(|| {
                RuntimeStateError::new(
                    "shared cursor source index does not resolve to a live cursor",
                )
            })?;
            if cursor.id != *id
                || cursor.source != source
                || self.cursor_handle_index.get(&cursor.handle) != Some(id)
            {
                return Err(RuntimeStateError::new(
                    "shared cursor handle and source indexes do not match their live identity",
                ));
            }
            return Ok(cursor.handle);
        }
        if self.cursors.len() >= RUNTIME_SHARED_CURSOR_LIMIT {
            return Err(RuntimeStateError::new(
                "process shared cursor count exceeds the runtime bound",
            ));
        }
        let id = RuntimeCursorId(self.next_cursor_id);
        if id.0 == 0 || self.cursors.contains_key(&id) {
            return Err(RuntimeStateError::new(
                "shared cursor allocator selected an invalid or live identity",
            ));
        }
        if self.next_cursor_handle < RUNTIME_CURSOR_HANDLE_BASE
            || self
                .cursor_handle_index
                .contains_key(&self.next_cursor_handle)
        {
            return Err(RuntimeStateError::new(
                "shared cursor allocator selected an invalid or live handle",
            ));
        }
        let next_cursor_id = self
            .next_cursor_id
            .checked_add(1)
            .ok_or_else(|| RuntimeStateError::new("shared cursor ID space is exhausted"))?;
        let next_cursor_handle = self
            .next_cursor_handle
            .checked_add(1)
            .ok_or_else(|| RuntimeStateError::new("shared cursor handle space is exhausted"))?;
        let next_generation = self.next_generation()?;
        let initialize_current_cursor = self.current_cursor.is_none()
            && matches!(
                &source,
                RuntimeCursorSource::SystemPredefined {
                    cursor: RuntimeSystemCursorKind::Arrow,
                    ..
                }
            );
        let cursor = RuntimeCursor {
            id,
            handle: self.next_cursor_handle,
            shared: true,
            source,
        };
        let handle = cursor.handle;
        // All fallible identity, capacity and lifetime checks precede publication.
        self.next_cursor_id = next_cursor_id;
        self.next_cursor_handle = next_cursor_handle;
        self.generation = next_generation;
        let previous_source = self
            .cursor_source_index
            .insert(cursor.source.clone(), cursor.id);
        let previous_handle = self.cursor_handle_index.insert(cursor.handle, cursor.id);
        let previous_cursor = self.cursors.insert(cursor.id, cursor);
        debug_assert!(previous_source.is_none());
        debug_assert!(previous_handle.is_none());
        debug_assert!(previous_cursor.is_none());
        if initialize_current_cursor {
            self.current_cursor = Some(id);
        }
        Ok(handle)
    }

    pub(super) fn set_current_cursor(
        &mut self,
        handle: Option<u32>,
    ) -> Result<u32, RuntimeStateError> {
        let previous_handle = self.current_cursor()?.map_or(0, |cursor| cursor.handle);
        let next = handle
            .map(|handle| self.cursor(handle).map(|cursor| cursor.id))
            .transpose()?;
        let next_generation = (next != self.current_cursor)
            .then(|| self.next_generation())
            .transpose()?;
        // Validation above precedes this synchronous selection; no callback or
        // browser handoff occurs between resolving the handle and publication.
        self.current_cursor = next;
        if let Some(next_generation) = next_generation {
            self.generation = next_generation;
        }
        Ok(previous_handle)
    }

    pub(super) fn register_window_class(
        &mut self,
        registration: RuntimeWindowClassRegistration,
    ) -> Result<u16, RuntimeStateError> {
        validate_window_class_registration(&registration)?;
        self.validate_window_class_references(&registration)?;
        let key = window_class_key(&registration)?;
        if let Some(id) = self.window_class_name_index.get(&key) {
            let class = self.window_classes.get(id).ok_or_else(|| {
                RuntimeStateError::new("window class name index does not resolve to a live class")
            })?;
            if class.id != *id
                || class.registration.module.module_id != key.module_id
                || class.registration.class_name.to_ascii_lowercase() != key.canonical_name
                || self.window_class_atom_index.get(&class.atom) != Some(id)
            {
                return Err(RuntimeStateError::new(
                    "window class atom and name indexes do not match their live identity",
                ));
            }
            return Ok(0);
        }
        if self.window_classes.len() >= RUNTIME_WINDOW_CLASS_LIMIT {
            return Err(RuntimeStateError::new(
                "process-local window class count exceeds the runtime bound",
            ));
        }
        let id = RuntimeWindowClassId(self.next_window_class_id);
        if id.0 == 0 || self.window_classes.contains_key(&id) {
            return Err(RuntimeStateError::new(
                "window class allocator selected an invalid or live identity",
            ));
        }
        let atom = u16::try_from(self.next_window_class_atom).map_err(|_| {
            RuntimeStateError::new("process-local window class atom space is exhausted")
        })?;
        if atom < RUNTIME_WINDOW_CLASS_ATOM_BASE || self.window_class_atom_index.contains_key(&atom)
        {
            return Err(RuntimeStateError::new(
                "window class atom allocator selected an invalid or live atom",
            ));
        }
        let next_class_id = self
            .next_window_class_id
            .checked_add(1)
            .ok_or_else(|| RuntimeStateError::new("window class ID space is exhausted"))?;
        let next_class_atom = self
            .next_window_class_atom
            .checked_add(1)
            .ok_or_else(|| RuntimeStateError::new("window class atom space is exhausted"))?;
        // Module, callback, class references, limits, and both allocators are
        // checked before this synchronous owner publishes the class.
        let next_generation = self.next_generation()?;
        let class = RuntimeWindowClass {
            id,
            atom,
            local: true,
            registration,
        };
        self.next_window_class_id = next_class_id;
        self.next_window_class_atom = next_class_atom;
        self.generation = next_generation;
        let previous_name = self.window_class_name_index.insert(key, id);
        let previous_atom = self.window_class_atom_index.insert(atom, id);
        let previous_class = self.window_classes.insert(id, class);
        debug_assert!(previous_name.is_none());
        debug_assert!(previous_atom.is_none());
        debug_assert!(previous_class.is_none());
        Ok(atom)
    }

    pub(super) fn checked_window_creation(
        &self,
        request: RuntimeWindowCreationRequest,
    ) -> Result<RuntimeWindow, RuntimeStateError> {
        validate_window_creation_request(&request)?;
        self.validate_window_creation_references(&request)?;
        if self.windows.len() >= RUNTIME_WINDOW_LIMIT {
            return Err(RuntimeStateError::new(
                "logical window count exceeds the runtime bound",
            ));
        }
        let id = RuntimeWindowId(self.next_window_id);
        if id.0 == 0 || self.windows.contains_key(&id) {
            return Err(RuntimeStateError::new(
                "logical window allocator selected an invalid or live identity",
            ));
        }
        if self.next_window_handle < RUNTIME_WINDOW_HANDLE_BASE
            || self
                .window_handle_index
                .contains_key(&self.next_window_handle)
            || self
                .ime_window_handle_index
                .contains_key(&self.next_window_handle)
        {
            return Err(RuntimeStateError::new(
                "logical HWND allocator selected an invalid or live handle",
            ));
        }
        self.next_window_id
            .checked_add(1)
            .ok_or_else(|| RuntimeStateError::new("logical window ID space is exhausted"))?;
        self.next_window_handle
            .checked_add(1)
            .ok_or_else(|| RuntimeStateError::new("logical HWND space is exhausted"))?;
        self.next_generation()?;
        Ok(RuntimeWindow {
            id,
            handle: self.next_window_handle,
            creation: request,
            lifecycle: RuntimeWindowLifecycle::Creating {
                message: RuntimeWindowCreationMessage::NonClientCreate,
            },
            visible: false,
            topmost: false,
            pending_update_rect: None,
            paint_generation: 0,
            small_icon: None,
        })
    }

    pub(super) fn publish_window_creation(
        &mut self,
        window: RuntimeWindow,
    ) -> (RuntimeWindowId, u32) {
        let result = (window.id, window.handle);
        self.next_window_id += 1;
        self.next_window_handle += 1;
        self.generation += 1;
        let previous_handle = self.window_handle_index.insert(window.handle, window.id);
        let previous_window = self.windows.insert(window.id, window);
        debug_assert!(previous_handle.is_none());
        debug_assert!(previous_window.is_none());
        result
    }

    pub(super) fn get_system_menu(
        &mut self,
        window_handle: u32,
        close_item_command: u32,
    ) -> Result<Option<u32>, RuntimeStateError> {
        if close_item_command == 0 {
            return Err(RuntimeStateError::new(
                "logical system menu requires a nonzero close-item command",
            ));
        }
        let Some(owner_window) = self.window_option(window_handle)? else {
            return Ok(None);
        };
        if owner_window.lifecycle != RuntimeWindowLifecycle::Created {
            return Err(RuntimeStateError::new(
                "logical system menu requires a created owner window",
            ));
        }
        if let Some(id) = self.system_menu_window_index.get(&owner_window.id) {
            let menu = self.system_menus.get(id).ok_or_else(|| {
                RuntimeStateError::new(
                    "logical window system-menu index does not resolve to a live menu",
                )
            })?;
            self.validate_system_menu_identity(menu)?;
            if menu.close_item_command != close_item_command {
                return Err(RuntimeStateError::new(
                    "logical system menu close-item command differs from its creation contract",
                ));
            }
            return Ok(Some(menu.handle));
        }
        if self.system_menus.len() >= RUNTIME_SYSTEM_MENU_LIMIT {
            return Err(RuntimeStateError::new(
                "logical system-menu count exceeds the runtime bound",
            ));
        }
        let id = RuntimeMenuId(self.next_menu_id);
        if id.0 == 0 || self.system_menus.contains_key(&id) {
            return Err(RuntimeStateError::new(
                "logical system-menu allocator selected an invalid or live identity",
            ));
        }
        if self.next_menu_handle < RUNTIME_MENU_HANDLE_BASE
            || self.menu_handle_index.contains_key(&self.next_menu_handle)
        {
            return Err(RuntimeStateError::new(
                "logical HMENU allocator selected an invalid or live handle",
            ));
        }
        let next_menu_id = self.next_menu_id.checked_add(1).ok_or_else(|| {
            RuntimeStateError::new("logical system-menu identity space is exhausted")
        })?;
        let next_menu_handle = self
            .next_menu_handle
            .checked_add(1)
            .ok_or_else(|| RuntimeStateError::new("logical HMENU space is exhausted"))?;
        let menu = RuntimeSystemMenu {
            id,
            handle: self.next_menu_handle,
            owner_window_id: owner_window.id,
            owner_window_handle: owner_window.handle,
            close_item_command,
            close_item_enabled: true,
        };
        let handle = menu.handle;
        // All identity, lifetime, capacity and overflow checks precede publication.
        self.next_menu_id = next_menu_id;
        self.next_menu_handle = next_menu_handle;
        self.system_menu_window_index
            .insert(menu.owner_window_id, menu.id);
        self.menu_handle_index.insert(menu.handle, menu.id);
        self.system_menus.insert(menu.id, menu);
        Ok(Some(handle))
    }

    pub(super) fn enable_system_menu_close_item(
        &self,
        menu_handle: u32,
        close_item_command: u32,
    ) -> Result<Option<u32>, RuntimeStateError> {
        let Some(menu) = self.system_menu_option(menu_handle)? else {
            return Ok(None);
        };
        if close_item_command == 0 || menu.close_item_command != close_item_command {
            return Err(RuntimeStateError::new(
                "EnableMenuItem command does not identify the logical system close item",
            ));
        }
        if !menu.close_item_enabled {
            return Err(RuntimeStateError::new(
                "the bounded logical system menu must begin with an enabled close item",
            ));
        }

        Ok(Some(0))
    }

    pub(super) fn resolve_ime_app_window_target(
        &self,
        window_handle: u32,
        owner_thread_id: RuntimeThreadId,
    ) -> Result<RuntimeImeAppWindowTarget, RuntimeStateError> {
        let window = self.window(window_handle)?;
        self.validate_created_window_owner(window, owner_thread_id)?;
        Ok(RuntimeImeAppWindowTarget {
            window_id: window.id,
            window_handle: window.handle,
            owner_thread_id: window.creation.owner_thread_id,
        })
    }

    pub(super) fn validate_default_ime_window(
        &self,
        window_handle: u32,
        owner_thread_id: RuntimeThreadId,
    ) -> Result<RuntimeImeAppWindowTarget, RuntimeStateError> {
        let target = self.resolve_ime_app_window_target(window_handle, owner_thread_id)?;
        if let Some(id) = self.default_ime_windows.get(&owner_thread_id) {
            let window = self.ime_windows.get(id).ok_or_else(|| {
                RuntimeStateError::new(
                    "default IME window index does not resolve to a live logical window",
                )
            })?;
            self.validate_ime_window_identity(window)?;
            if window.owner_thread_id != owner_thread_id {
                return Err(RuntimeStateError::new(
                    "default IME window index resolves to a different owner thread",
                ));
            }
            return Ok(target);
        }
        if self.ime_windows.len() >= RUNTIME_DEFAULT_IME_WINDOW_LIMIT {
            return Err(RuntimeStateError::new(
                "logical default IME window count exceeds the runtime bound",
            ));
        }
        let id = RuntimeImeWindowId(self.next_ime_window_id);
        if id.0 == 0 || self.ime_windows.contains_key(&id) {
            return Err(RuntimeStateError::new(
                "logical default IME window allocator selected an invalid or live identity",
            ));
        }
        if self.next_window_handle < RUNTIME_WINDOW_HANDLE_BASE
            || self
                .window_handle_index
                .contains_key(&self.next_window_handle)
            || self
                .ime_window_handle_index
                .contains_key(&self.next_window_handle)
        {
            return Err(RuntimeStateError::new(
                "logical HWND allocator selected an invalid or live IME window handle",
            ));
        }
        self.next_ime_window_id.checked_add(1).ok_or_else(|| {
            RuntimeStateError::new("logical default IME window identity space is exhausted")
        })?;
        self.next_window_handle
            .checked_add(1)
            .ok_or_else(|| RuntimeStateError::new("logical HWND space is exhausted"))?;
        self.next_generation()?;
        Ok(target)
    }

    // Called synchronously after validate_default_ime_window, with no intervening
    // User32 mutation. IMM32 publication may occur between validation and here.
    pub(super) fn ensure_default_ime_window(&mut self, owner_thread_id: RuntimeThreadId) -> u32 {
        if let Some(id) = self.default_ime_windows.get(&owner_thread_id) {
            return self.ime_windows[id].handle;
        }
        let window = RuntimeImeWindow {
            id: RuntimeImeWindowId(self.next_ime_window_id),
            handle: self.next_window_handle,
            owner_thread_id,
            class_name: "IME".to_owned(),
            status_window_request: None,
        };
        let handle = window.handle;
        self.next_ime_window_id += 1;
        self.next_window_handle += 1;
        self.generation += 1;
        self.ime_window_handle_index
            .insert(window.handle, window.id);
        self.default_ime_windows.insert(owner_thread_id, window.id);
        self.ime_windows.insert(window.id, window);
        handle
    }

    fn window_creation_callback_disposition(
        &self,
        window_id: RuntimeWindowId,
        expected_message: RuntimeWindowCreationMessage,
        result: u32,
    ) -> Result<RuntimeWindowCreationCallbackDisposition, RuntimeStateError> {
        let previous = self.windows.get(&window_id).cloned().ok_or_else(|| {
            RuntimeStateError::new("logical window creation callback identifies no live window")
        })?;
        if self.window_handle_index.get(&previous.handle) != Some(&window_id) {
            return Err(RuntimeStateError::new(
                "logical HWND index does not match its window identity",
            ));
        }
        if previous.lifecycle
            != (RuntimeWindowLifecycle::Creating {
                message: expected_message,
            })
        {
            return Err(RuntimeStateError::new(
                "logical window creation callback is not the expected synchronous message",
            ));
        }
        self.validate_window_creation_references(&previous.creation)?;

        let abort = (expected_message == RuntimeWindowCreationMessage::NonClientCreate
            && result == 0)
            || (expected_message == RuntimeWindowCreationMessage::Create && result == u32::MAX);
        let next_generation = self.next_generation()?;
        let disposition = if abort {
            RuntimeWindowCreationCallbackDisposition::Remove {
                action: RuntimeWindowCreationCallbackAction::Aborted {
                    window: previous.clone(),
                },
                window_id: previous.id,
                window_handle: previous.handle,
                next_generation,
            }
        } else {
            let next_message = match expected_message {
                RuntimeWindowCreationMessage::NonClientCreate => {
                    Some(RuntimeWindowCreationMessage::NonClientCalculate)
                }
                RuntimeWindowCreationMessage::NonClientCalculate => {
                    Some(RuntimeWindowCreationMessage::Create)
                }
                RuntimeWindowCreationMessage::Create => Some(RuntimeWindowCreationMessage::Size),
                RuntimeWindowCreationMessage::Size => Some(RuntimeWindowCreationMessage::Move),
                RuntimeWindowCreationMessage::Move => None,
            };
            let mut next = previous.clone();
            if let Some(next_message) = next_message {
                next.lifecycle = RuntimeWindowLifecycle::Creating {
                    message: next_message,
                };
                RuntimeWindowCreationCallbackDisposition::Update {
                    action: RuntimeWindowCreationCallbackAction::Continue {
                        window: next.clone(),
                        next_message,
                    },
                    next: Box::new(next),
                    next_generation,
                }
            } else {
                next.lifecycle = RuntimeWindowLifecycle::Created;
                RuntimeWindowCreationCallbackDisposition::Update {
                    action: RuntimeWindowCreationCallbackAction::Created {
                        window: next.clone(),
                    },
                    next: Box::new(next),
                    next_generation,
                }
            }
        };
        Ok(disposition)
    }

    pub(super) fn preview_window_creation_callback(
        &self,
        window_id: RuntimeWindowId,
        message: RuntimeWindowCreationMessage,
        result: u32,
    ) -> Result<RuntimeWindowCreationCallbackAction, RuntimeStateError> {
        let disposition = self.window_creation_callback_disposition(window_id, message, result)?;
        Ok(match disposition {
            RuntimeWindowCreationCallbackDisposition::Update { action, .. }
            | RuntimeWindowCreationCallbackDisposition::Remove { action, .. } => action,
        })
    }

    pub(super) fn complete_window_creation_callback(
        &mut self,
        window_id: RuntimeWindowId,
        message: RuntimeWindowCreationMessage,
        result: u32,
    ) -> Result<RuntimeWindowCreationCallbackAction, RuntimeStateError> {
        let disposition = self.window_creation_callback_disposition(window_id, message, result)?;
        Ok(match disposition {
            RuntimeWindowCreationCallbackDisposition::Update {
                next,
                action,
                next_generation,
            } => {
                let next = *next;
                self.generation = next_generation;
                self.windows.insert(next.id, next);
                action
            }
            RuntimeWindowCreationCallbackDisposition::Remove {
                window_id,
                window_handle,
                action,
                next_generation,
            } => {
                self.generation = next_generation;
                self.window_handle_index.remove(&window_handle);
                self.windows.remove(&window_id);
                action
            }
        })
    }

    fn validate_window_creation_references(
        &self,
        request: &RuntimeWindowCreationRequest,
    ) -> Result<(), RuntimeStateError> {
        let class = self
            .window_classes
            .get(&request.class.class_id)
            .ok_or_else(|| {
                RuntimeStateError::new("logical window refers to a non-live process-local class")
            })?;
        if class.atom != request.class.atom
            || class.registration.module.module_id != request.class.module_id
            || class.registration.module != request.instance
            || self.window_class_atom_index.get(&class.atom) != Some(&class.id)
        {
            return Err(RuntimeStateError::new(
                "logical window class identity, ATOM, and module do not match",
            ));
        }
        Ok(())
    }

    fn validate_window_class_references(
        &self,
        registration: &RuntimeWindowClassRegistration,
    ) -> Result<(), RuntimeStateError> {
        for reference in [registration.icon, registration.small_icon]
            .into_iter()
            .flatten()
        {
            let icon = self.icons.get(&reference.icon_id).ok_or_else(|| {
                RuntimeStateError::new("window class refers to a non-live logical icon identity")
            })?;
            if icon.handle != reference.handle
                || self.handle_index.get(&reference.handle) != Some(&reference.icon_id)
            {
                return Err(RuntimeStateError::new(
                    "window class icon ID and logical handle do not identify the same icon",
                ));
            }
        }
        if let Some(reference) = registration.cursor {
            let cursor = self.cursors.get(&reference.cursor_id).ok_or_else(|| {
                RuntimeStateError::new("window class refers to a non-live logical cursor identity")
            })?;
            if cursor.handle != reference.handle
                || self.cursor_handle_index.get(&reference.handle) != Some(&reference.cursor_id)
            {
                return Err(RuntimeStateError::new(
                    "window class cursor ID and logical handle do not identify the same cursor",
                ));
            }
        }
        Ok(())
    }

    pub(super) fn icon_option(
        &self,
        handle: u32,
    ) -> Result<Option<&RuntimeIcon>, RuntimeStateError> {
        let Some(id) = self.handle_index.get(&handle).copied() else {
            return Ok(None);
        };
        match self.icons.get(&id) {
            Some(icon) if icon.id == id && icon.handle == handle => Ok(Some(icon)),
            _ => Err(RuntimeStateError::new(
                "logical HICON index does not resolve to the indexed icon identity",
            )),
        }
    }

    pub fn icon(&self, handle: u32) -> Result<&RuntimeIcon, RuntimeStateError> {
        self.icon_option(handle)?.ok_or_else(|| {
            RuntimeStateError::new(format!("logical HICON {handle:#010x} is not live"))
        })
    }

    #[cfg(test)]
    pub(crate) fn icons(&self) -> &BTreeMap<RuntimeIconId, RuntimeIcon> {
        &self.icons
    }

    pub(super) fn cursor_option(
        &self,
        handle: u32,
    ) -> Result<Option<&RuntimeCursor>, RuntimeStateError> {
        let Some(id) = self.cursor_handle_index.get(&handle).copied() else {
            return Ok(None);
        };
        match self.cursors.get(&id) {
            Some(cursor) if cursor.id == id && cursor.handle == handle => Ok(Some(cursor)),
            _ => Err(RuntimeStateError::new(
                "logical HCURSOR index does not resolve to the indexed cursor identity",
            )),
        }
    }

    pub fn cursor(&self, handle: u32) -> Result<&RuntimeCursor, RuntimeStateError> {
        self.cursor_option(handle)?.ok_or_else(|| {
            RuntimeStateError::new(format!("logical HCURSOR {handle:#010x} is not live"))
        })
    }

    pub(super) fn current_cursor(&self) -> Result<Option<&RuntimeCursor>, RuntimeStateError> {
        let Some(id) = self.current_cursor else {
            return Ok(None);
        };
        let cursor = self.cursors.get(&id).ok_or_else(|| {
            RuntimeStateError::new("current cursor identity does not resolve to a live cursor")
        })?;
        if self.cursor_handle_index.get(&cursor.handle) != Some(&id) {
            return Err(RuntimeStateError::new(
                "current cursor identity does not match its handle index",
            ));
        }
        Ok(Some(cursor))
    }

    #[cfg(test)]
    pub(crate) fn cursor_for_source(
        &self,
        source: &RuntimeCursorSource,
    ) -> Result<&RuntimeCursor, RuntimeStateError> {
        validate_cursor_source(source)?;
        let id = self
            .cursor_source_index
            .get(source)
            .copied()
            .ok_or_else(|| RuntimeStateError::new("logical shared cursor source is not loaded"))?;
        let cursor = self.cursors.get(&id).ok_or_else(|| {
            RuntimeStateError::new(
                "logical shared cursor source index does not resolve to a live cursor",
            )
        })?;
        if cursor.id != id
            || &cursor.source != source
            || self.cursor_handle_index.get(&cursor.handle) != Some(&id)
        {
            return Err(RuntimeStateError::new(
                "logical shared cursor source does not match its handle index",
            ));
        }
        Ok(cursor)
    }

    #[cfg(test)]
    pub(crate) fn cursors(&self) -> &BTreeMap<RuntimeCursorId, RuntimeCursor> {
        &self.cursors
    }

    pub fn window_class(&self, atom: u16) -> Result<&RuntimeWindowClass, RuntimeStateError> {
        let id = self
            .window_class_atom_index
            .get(&atom)
            .copied()
            .ok_or_else(|| {
                RuntimeStateError::new(format!("logical window class ATOM {atom:#06x} is not live"))
            })?;
        let class = self.window_classes.get(&id).ok_or_else(|| {
            RuntimeStateError::new(
                "logical window class ATOM index does not resolve to a live class",
            )
        })?;
        let key = RuntimeWindowClassKey {
            module_id: class.registration.module.module_id,
            canonical_name: class.registration.class_name.to_ascii_lowercase(),
        };
        if class.id != id
            || class.atom != atom
            || self.window_class_name_index.get(&key) != Some(&id)
        {
            return Err(RuntimeStateError::new(
                "logical window class ATOM and name indexes disagree about the live class",
            ));
        }
        Ok(class)
    }

    pub(super) fn window_class_by_name_option(
        &self,
        module_id: RuntimeModuleId,
        name: &str,
    ) -> Result<Option<&RuntimeWindowClass>, RuntimeStateError> {
        let key = window_class_key_from_name(module_id, name)?;
        let Some(id) = self.window_class_name_index.get(&key).copied() else {
            return Ok(None);
        };
        let class = self.window_classes.get(&id).ok_or_else(|| {
            RuntimeStateError::new(
                "logical window class name index does not resolve to a live class",
            )
        })?;
        if class.id != id
            || class.registration.module.module_id != key.module_id
            || class.registration.class_name.to_ascii_lowercase() != key.canonical_name
            || self.window_class_atom_index.get(&class.atom) != Some(&id)
        {
            return Err(RuntimeStateError::new(
                "logical window class name and ATOM indexes disagree about the live class",
            ));
        }
        Ok(Some(class))
    }

    #[cfg(test)]
    pub(crate) fn window_class_by_name(
        &self,
        module_id: RuntimeModuleId,
        name: &str,
    ) -> Result<&RuntimeWindowClass, RuntimeStateError> {
        self.window_class_by_name_option(module_id, name)?
            .ok_or_else(|| {
                RuntimeStateError::new(
                    "process-local window class name is not registered for the module",
                )
            })
    }

    #[cfg(test)]
    pub(crate) fn window_classes(&self) -> &BTreeMap<RuntimeWindowClassId, RuntimeWindowClass> {
        &self.window_classes
    }

    pub(super) fn window_option(
        &self,
        handle: u32,
    ) -> Result<Option<&RuntimeWindow>, RuntimeStateError> {
        let Some(id) = self.window_handle_index.get(&handle).copied() else {
            return Ok(None);
        };
        match self.windows.get(&id) {
            Some(window) if window.handle == handle && window.id == id => Ok(Some(window)),
            _ => Err(RuntimeStateError::new(
                "logical HWND index does not resolve to the indexed window identity",
            )),
        }
    }

    pub fn window(&self, handle: u32) -> Result<&RuntimeWindow, RuntimeStateError> {
        self.window_option(handle)?.ok_or_else(|| {
            RuntimeStateError::new(format!("logical HWND {handle:#010x} is not live"))
        })
    }

    pub fn window_is_iconic(&self, handle: u32) -> Result<Option<bool>, RuntimeStateError> {
        let Some(window) = self.window_option(handle)? else {
            return Ok(None);
        };
        if window.lifecycle != RuntimeWindowLifecycle::Created {
            return Err(RuntimeStateError::new(
                "logical iconic-state queries require a created window",
            ));
        }
        Ok(Some(
            window.creation.effective_style & RUNTIME_WINDOW_STYLE_MINIMIZED != 0,
        ))
    }

    pub fn ime_window(&self, handle: u32) -> Result<&RuntimeImeWindow, RuntimeStateError> {
        let id = self
            .ime_window_handle_index
            .get(&handle)
            .copied()
            .ok_or_else(|| {
                RuntimeStateError::new(format!("logical IME HWND {handle:#010x} is not live"))
            })?;
        let window = self.ime_windows.get(&id).ok_or_else(|| {
            RuntimeStateError::new("logical IME HWND index does not resolve to a live window")
        })?;
        self.validate_ime_window_identity(window)?;
        Ok(window)
    }

    #[cfg(test)]
    pub(crate) fn ime_windows(&self) -> &BTreeMap<RuntimeImeWindowId, RuntimeImeWindow> {
        &self.ime_windows
    }

    pub(super) fn control_ime_status_window(
        &mut self,
        window_handle: u32,
        request: RuntimeImeStatusWindowRequest,
    ) -> Result<(), RuntimeStateError> {
        let previous = self.ime_window(window_handle)?;
        if previous.status_window_request == Some(request) {
            return Ok(());
        }
        let id = previous.id;
        let generation = self.next_generation()?;
        // Built-in IME status control does not invoke guest code or leave Rust.
        // The HWND owns the IME thread identity; the calling thread may differ.
        let window = self.ime_windows.get_mut(&id).expect("validated IME HWND");
        window.status_window_request = Some(request);
        self.generation = generation;
        Ok(())
    }

    pub(super) fn resolve_small_icon_window_procedure(
        &self,
        window_handle: u32,
        owner_thread_id: RuntimeThreadId,
        icon_handle: u32,
    ) -> Result<RuntimeWindowProcedure, RuntimeStateError> {
        let window = self.window(window_handle)?;
        if window.creation.owner_thread_id != owner_thread_id {
            return Err(RuntimeStateError::new(
                "synchronous logical window messages require the selected owner thread",
            ));
        }
        if window.lifecycle != RuntimeWindowLifecycle::Created {
            return Err(RuntimeStateError::new(
                "synchronous logical window messages require a created window",
            ));
        }
        self.validate_window_creation_references(&window.creation)?;
        let class = self
            .window_classes
            .get(&window.creation.class.class_id)
            .ok_or_else(|| {
                RuntimeStateError::new(
                    "live logical window class identity disappeared during message resolution",
                )
            })?;
        validate_window_class_registration(&class.registration)?;
        self.validate_window_class_references(&class.registration)?;

        let icon = self.icon(icon_handle)?;
        if !icon.shared {
            return Err(RuntimeStateError::new(
                "synchronous small-icon message requires a live shared icon",
            ));
        }
        Ok(class.registration.window_procedure.clone())
    }

    pub(super) fn resolve_window_procedure(
        &self,
        window_handle: u32,
        owner_thread_id: RuntimeThreadId,
    ) -> Result<RuntimeWindowProcedure, RuntimeStateError> {
        let window = self.window(window_handle)?;
        self.validate_created_window_owner(window, owner_thread_id)?;
        let class = self
            .window_classes
            .get(&window.creation.class.class_id)
            .ok_or_else(|| {
                RuntimeStateError::new(
                    "live logical window class identity disappeared during message resolution",
                )
            })?;
        validate_window_class_registration(&class.registration)?;
        self.validate_window_class_references(&class.registration)?;
        Ok(class.registration.window_procedure.clone())
    }

    pub(super) fn set_window_small_icon(
        &mut self,
        window_handle: u32,
        owner_thread_id: RuntimeThreadId,
        icon_handle: u32,
    ) -> Result<u32, RuntimeStateError> {
        self.resolve_small_icon_window_procedure(window_handle, owner_thread_id, icon_handle)?;
        let previous = self.window(window_handle)?;
        self.validate_window_icon_reference(previous.small_icon)?;
        let previous_handle = previous.small_icon.map_or(0, |icon| icon.handle);
        let icon = self.icon(icon_handle)?;
        let reference = RuntimeWindowIconReference {
            icon_id: icon.id,
            handle: icon.handle,
        };
        if previous.small_icon == Some(reference) {
            return Ok(previous_handle);
        }
        let next_generation = self.next_generation()?;
        let id = previous.id;
        self.windows
            .get_mut(&id)
            .expect("validated window identity")
            .small_icon = Some(reference);
        self.generation = next_generation;
        Ok(previous_handle)
    }

    pub(super) fn set_window_title(
        &mut self,
        window_handle: u32,
        owner_thread_id: RuntimeThreadId,
        title: RuntimeWindowNameA,
    ) -> Result<(), RuntimeStateError> {
        let previous = self.window(window_handle)?;
        self.validate_created_window_owner(previous, owner_thread_id)?;
        title.windows_932_bytes()?;
        self.validate_window_creation_references(&previous.creation)?;
        if previous.creation.name == title {
            return Ok(());
        }
        let next_generation = self.next_generation()?;
        let id = previous.id;
        let window = self
            .windows
            .get_mut(&id)
            .expect("validated window identity");
        window.creation.name = title;
        self.generation = next_generation;
        Ok(())
    }

    pub(super) fn set_window_style(
        &mut self,
        window_handle: u32,
        owner_thread_id: RuntimeThreadId,
        effective_style: u32,
    ) -> Result<u32, RuntimeStateError> {
        let previous = self.window(window_handle)?;
        self.validate_created_window_owner(previous, owner_thread_id)?;
        self.validate_window_icon_reference(previous.small_icon)?;
        let visible = effective_style & RUNTIME_WINDOW_STYLE_VISIBLE != 0;
        let previous_style = previous.creation.effective_style;
        if effective_style != previous_style || visible != previous.visible {
            let id = previous.id;
            let next_generation = self.next_generation()?;
            let window = self
                .windows
                .get_mut(&id)
                .expect("validated window disappeared");
            window.creation.effective_style = effective_style;
            window.visible = visible;
            self.generation = next_generation;
        }
        Ok(previous_style)
    }

    pub(super) fn set_window_file_drop_acceptance(
        &mut self,
        window_handle: u32,
        accepts_files: bool,
    ) -> Result<(), RuntimeStateError> {
        let previous = self.window(window_handle)?;
        if previous.lifecycle != RuntimeWindowLifecycle::Created {
            return Err(RuntimeStateError::new(
                "logical file-drop acceptance requires a created window",
            ));
        }
        self.validate_window_creation_references(&previous.creation)?;
        self.validate_window_icon_reference(previous.small_icon)?;
        let previous_accepts_files = previous.creation.effective_extended_style
            & RUNTIME_WINDOW_EXTENDED_STYLE_ACCEPT_FILES
            != 0;
        if previous_accepts_files == accepts_files {
            return Ok(());
        }
        let id = previous.id;
        let generation = self.next_generation()?;
        let window = self.windows.get_mut(&id).expect("validated HWND");
        if accepts_files {
            window.creation.effective_extended_style |= RUNTIME_WINDOW_EXTENDED_STYLE_ACCEPT_FILES;
        } else {
            window.creation.effective_extended_style &= !RUNTIME_WINDOW_EXTENDED_STYLE_ACCEPT_FILES;
        }
        self.generation = generation;
        Ok(())
    }

    pub(super) fn preview_set_window_pos(
        &self,
        owner_thread_id: RuntimeThreadId,
        request: RuntimeSetWindowPosRequest,
    ) -> Result<RuntimeSetWindowPosOperation, RuntimeStateError> {
        self.resolve_window_procedure(request.window_handle, owner_thread_id)?;
        let window = self.window(request.window_handle)?;
        self.validate_window_icon_reference(window.small_icon)?;
        if request.callback_flags == 0
            || (request.variant == RuntimeSetWindowPosVariant::Resize
                && (request.resize_nccalc_window_pos_flags == 0
                    || request.resize_changed_flags == 0))
            || (request.variant == RuntimeSetWindowPosVariant::FrameChangeMove
                && (request.frame_change_nccalc_window_pos_flags == 0
                    || request.frame_change_changed_flags == 0))
        {
            return Err(RuntimeStateError::new(
                "logical window positioning requires nonzero callback flags",
            ));
        }
        let mut next_creation = window.creation.clone();
        next_creation.geometry = request.next_geometry;
        validate_window_creation_request(&next_creation)?;
        let target = Some(window.id);
        let repeat_activation = self.active_windows.get(&owner_thread_id).copied() == target
            && self.focused_windows.get(&owner_thread_id).copied() == target
            && self.foreground_window == target;
        let variant = if request.variant == RuntimeSetWindowPosVariant::ActivateSameGeometry
            && repeat_activation
        {
            RuntimeSetWindowPosVariant::RepeatActivation
        } else {
            request.variant
        };
        validate_position_geometry_transition(
            window.creation.geometry,
            request.next_geometry,
            variant,
        )?;
        let small_icon_handle = window.small_icon.map_or(0, |icon| icon.handle);
        let next_effective_style = if request.next_visible {
            window.creation.effective_style | RUNTIME_WINDOW_STYLE_VISIBLE
        } else {
            window.creation.effective_style & !RUNTIME_WINDOW_STYLE_VISIBLE
        };
        let next_update_rect = match variant {
            RuntimeSetWindowPosVariant::FrameChangeMove => {
                Some(full_client_update_rect(request.next_geometry)?)
            }
            _ => window.pending_update_rect,
        };
        match variant {
            RuntimeSetWindowPosVariant::InitialShow
                if window.visible
                    || window.topmost
                    || !request.next_visible
                    || request.next_topmost
                    || request.next_geometry == window.creation.geometry
                    || small_icon_handle == 0 =>
            {
                return Err(RuntimeStateError::new(
                    "initial logical window show requires a hidden non-topmost window, moved geometry, and a live small icon",
                ));
            }
            RuntimeSetWindowPosVariant::InitialHide
                if !window.visible
                    || window.topmost
                    || request.next_visible
                    || request.next_topmost
                    || request.next_geometry != window.creation.geometry =>
            {
                return Err(RuntimeStateError::new(
                    "initial logical window hide requires a visible non-topmost window with unchanged geometry",
                ));
            }
            RuntimeSetWindowPosVariant::ActivateSameGeometry
                if !window.visible
                    || window.topmost
                    || !request.next_visible
                    || request.next_topmost
                    || request.next_geometry != window.creation.geometry
                    || window.creation.effective_style & RUNTIME_WINDOW_STYLE_VISIBLE == 0
                    || self.active_windows.contains_key(&owner_thread_id)
                    || self.focused_windows.contains_key(&owner_thread_id)
                    || self.foreground_window.is_some() =>
            {
                return Err(RuntimeStateError::new(
                    "same-geometry activation requires one visible inactive owner-thread window and no imported foreground identity",
                ));
            }
            RuntimeSetWindowPosVariant::RepeatActivation
                if !window.visible
                    || window.topmost
                    || !request.next_visible
                    || request.next_topmost
                    || request.next_geometry != window.creation.geometry
                    || window.creation.effective_style & RUNTIME_WINDOW_STYLE_VISIBLE == 0
                    || !repeat_activation =>
            {
                return Err(RuntimeStateError::new(
                    "repeat activation requires the visible active, focused, foreground owner-thread window with unchanged geometry",
                ));
            }
            RuntimeSetWindowPosVariant::Resize
                if !window.visible
                    || window.topmost
                    || !request.next_visible
                    || request.next_topmost
                    || request.next_geometry == window.creation.geometry
                    || window.creation.effective_style & RUNTIME_WINDOW_STYLE_VISIBLE == 0
                    || self.active_windows.get(&owner_thread_id).copied() != Some(window.id)
                    || self.focused_windows.get(&owner_thread_id).copied() != Some(window.id)
                    || self.foreground_window != Some(window.id) =>
            {
                return Err(RuntimeStateError::new(
                    "logical resize requires one visible active, focused, foreground owner-thread window and changed bounded geometry",
                ));
            }
            RuntimeSetWindowPosVariant::FrameChangeMove
                if !window.visible
                    || window.topmost
                    || !request.next_visible
                    || request.next_topmost
                    || request.next_geometry == window.creation.geometry
                    || window.creation.effective_style & RUNTIME_WINDOW_STYLE_VISIBLE == 0
                    || self.active_windows.get(&owner_thread_id).copied() != Some(window.id)
                    || self.focused_windows.get(&owner_thread_id).copied() != Some(window.id)
                    || self.foreground_window != Some(window.id) =>
            {
                return Err(RuntimeStateError::new(
                    "frame-change move requires one visible active, focused, foreground owner-thread window and changed bounded position",
                ));
            }
            _ => {}
        }
        Ok(RuntimeSetWindowPosOperation {
            window_id: window.id,
            window_handle: window.handle,
            expected_generation: self.generation,
            variant,
            previous_geometry: window.creation.geometry,
            next_geometry: request.next_geometry,
            previous_visible: window.visible,
            next_visible: request.next_visible,
            previous_topmost: window.topmost,
            next_topmost: request.next_topmost,
            previous_effective_style: window.creation.effective_style,
            next_effective_style,
            previous_update_rect: window.pending_update_rect,
            next_update_rect,
            entry_last_error: request.entry_last_error,
            callback_flags: request.callback_flags,
            callback_insert_after: request.callback_insert_after,
            activation_insert_after: request.activation_insert_after,
            activation_internal_flags: request.activation_internal_flags,
            resize_nccalc_window_pos_flags: request.resize_nccalc_window_pos_flags,
            resize_changed_flags: request.resize_changed_flags,
            frame_change_nccalc_window_pos_flags: request.frame_change_nccalc_window_pos_flags,
            frame_change_changed_flags: request.frame_change_changed_flags,
            small_icon_handle,
        })
    }

    pub(super) fn checked_set_window_pos_callback(
        &self,
        operation: RuntimeSetWindowPosOperation,
        message: RuntimeSetWindowPosCallbackMessage,
        result: u32,
    ) -> Result<RuntimeSetWindowPosCallbackDisposition, RuntimeStateError> {
        let previous = self
            .validate_set_window_pos_operation(operation, message)?
            .clone();
        let disposition = match (operation.variant, message) {
            (
                RuntimeSetWindowPosVariant::InitialShow,
                RuntimeSetWindowPosCallbackMessage::WindowPositionChanged,
            ) => {
                if result != 0 {
                    return Err(RuntimeStateError::new(
                        "initial window-position callback must return the observed zero result",
                    ));
                }
                RuntimeSetWindowPosCallbackDisposition::Validate {
                    action: RuntimeSetWindowPosCallbackAction::Continue {
                        window: previous,
                        operation,
                        next_message: RuntimeSetWindowPosCallbackMessage::GetSmallIcon,
                    },
                }
            }
            (
                RuntimeSetWindowPosVariant::InitialShow,
                RuntimeSetWindowPosCallbackMessage::GetSmallIcon,
            ) => {
                if result != operation.small_icon_handle {
                    return Err(RuntimeStateError::new(
                        "WM_GETICON callback did not return the window's live small-icon identity",
                    ));
                }
                self.window_position_update_disposition(previous, operation)?
            }
            (
                RuntimeSetWindowPosVariant::InitialHide,
                RuntimeSetWindowPosCallbackMessage::WindowPositionChanged,
            ) => {
                if result != 0 {
                    return Err(RuntimeStateError::new(
                        "initial hide callback must return the observed zero result",
                    ));
                }
                self.window_position_update_disposition(previous, operation)?
            }
            (RuntimeSetWindowPosVariant::InitialHide, _) => {
                return Err(RuntimeStateError::new(
                    "initial hide received an out-of-order logical window callback",
                ));
            }
            (RuntimeSetWindowPosVariant::InitialShow, _) => {
                return Err(RuntimeStateError::new(
                    "initial show received an out-of-order logical window callback",
                ));
            }
            (
                RuntimeSetWindowPosVariant::ActivateSameGeometry,
                RuntimeSetWindowPosCallbackMessage::WindowPositionChanging,
            ) => {
                require_set_window_pos_callback_result(result, 0, "initial activation WINDOWPOS")?;
                let next_generation = self.next_generation()?;
                let mut next_operation = operation;
                next_operation.expected_generation = next_generation;
                RuntimeSetWindowPosCallbackDisposition::Activate {
                    window_id: previous.id,
                    owner_thread_id: previous.creation.owner_thread_id,
                    action: RuntimeSetWindowPosCallbackAction::Continue {
                        window: previous,
                        operation: next_operation,
                        next_message:
                            RuntimeSetWindowPosCallbackMessage::ActivationWindowPositionChanging,
                    },
                    next_generation,
                }
            }
            (
                RuntimeSetWindowPosVariant::ActivateSameGeometry,
                RuntimeSetWindowPosCallbackMessage::ActivationWindowPositionChanging,
            ) => {
                require_set_window_pos_callback_result(result, 0, "activation WINDOWPOS")?;
                RuntimeSetWindowPosCallbackDisposition::Validate {
                    action: RuntimeSetWindowPosCallbackAction::Continue {
                        window: previous,
                        operation,
                        next_message: RuntimeSetWindowPosCallbackMessage::ActivateApplication,
                    },
                }
            }
            (
                RuntimeSetWindowPosVariant::ActivateSameGeometry,
                RuntimeSetWindowPosCallbackMessage::ActivateApplication,
            ) => {
                require_set_window_pos_callback_result(result, 0, "WM_ACTIVATEAPP")?;
                RuntimeSetWindowPosCallbackDisposition::Validate {
                    action: RuntimeSetWindowPosCallbackAction::Continue {
                        window: previous,
                        operation,
                        next_message: RuntimeSetWindowPosCallbackMessage::NonClientActivate,
                    },
                }
            }
            (
                RuntimeSetWindowPosVariant::ActivateSameGeometry,
                RuntimeSetWindowPosCallbackMessage::NonClientActivate,
            ) => {
                // The notification activates the nonclient area (wParam = TRUE);
                // Windows ignores its return value.
                RuntimeSetWindowPosCallbackDisposition::Validate {
                    action: RuntimeSetWindowPosCallbackAction::Continue {
                        window: previous,
                        operation,
                        next_message: RuntimeSetWindowPosCallbackMessage::Activate,
                    },
                }
            }
            (
                RuntimeSetWindowPosVariant::ActivateSameGeometry,
                RuntimeSetWindowPosCallbackMessage::Activate,
            ) => {
                require_set_window_pos_callback_result(result, 0, "WM_ACTIVATE")?;
                RuntimeSetWindowPosCallbackDisposition::Validate {
                    action: RuntimeSetWindowPosCallbackAction::Completed { window: previous },
                }
            }
            (RuntimeSetWindowPosVariant::ActivateSameGeometry, _) => {
                return Err(RuntimeStateError::new(
                    "same-geometry activation received an out-of-order logical window callback",
                ));
            }
            (
                RuntimeSetWindowPosVariant::RepeatActivation,
                RuntimeSetWindowPosCallbackMessage::WindowPositionChanging,
            ) => {
                require_set_window_pos_callback_result(result, 0, "repeat activation WINDOWPOS")?;
                RuntimeSetWindowPosCallbackDisposition::Validate {
                    action: RuntimeSetWindowPosCallbackAction::Completed { window: previous },
                }
            }
            (RuntimeSetWindowPosVariant::RepeatActivation, _) => {
                return Err(RuntimeStateError::new(
                    "repeat activation received an out-of-order logical window callback",
                ));
            }
            (
                RuntimeSetWindowPosVariant::Resize,
                RuntimeSetWindowPosCallbackMessage::WindowPositionChanging,
            ) => {
                require_set_window_pos_callback_result(result, 0, "resize WINDOWPOS")?;
                RuntimeSetWindowPosCallbackDisposition::Validate {
                    action: RuntimeSetWindowPosCallbackAction::Continue {
                        window: previous,
                        operation,
                        next_message: RuntimeSetWindowPosCallbackMessage::NonClientCalculate,
                    },
                }
            }
            (
                RuntimeSetWindowPosVariant::Resize,
                RuntimeSetWindowPosCallbackMessage::NonClientCalculate,
            ) => {
                require_set_window_pos_callback_result(result, 0, "WM_NCCALCSIZE")?;
                self.window_resize_update_disposition(previous, operation)?
            }
            (RuntimeSetWindowPosVariant::Resize, RuntimeSetWindowPosCallbackMessage::Size) => {
                require_set_window_pos_callback_result(result, 0, "WM_SIZE")?;
                RuntimeSetWindowPosCallbackDisposition::Validate {
                    action: RuntimeSetWindowPosCallbackAction::Continue {
                        window: previous,
                        operation,
                        next_message: RuntimeSetWindowPosCallbackMessage::WindowPositionChanged,
                    },
                }
            }
            (
                RuntimeSetWindowPosVariant::Resize,
                RuntimeSetWindowPosCallbackMessage::WindowPositionChanged,
            ) => {
                require_set_window_pos_callback_result(result, 0, "resize WM_WINDOWPOSCHANGED")?;
                RuntimeSetWindowPosCallbackDisposition::Validate {
                    action: RuntimeSetWindowPosCallbackAction::Completed { window: previous },
                }
            }
            (RuntimeSetWindowPosVariant::Resize, _) => {
                return Err(RuntimeStateError::new(
                    "logical resize received an out-of-order window callback",
                ));
            }
            (
                RuntimeSetWindowPosVariant::FrameChangeMove,
                RuntimeSetWindowPosCallbackMessage::WindowPositionChanging,
            ) => {
                require_set_window_pos_callback_result(result, 0, "frame-change move WINDOWPOS")?;
                RuntimeSetWindowPosCallbackDisposition::Validate {
                    action: RuntimeSetWindowPosCallbackAction::Continue {
                        window: previous,
                        operation,
                        next_message: RuntimeSetWindowPosCallbackMessage::NonClientCalculate,
                    },
                }
            }
            (
                RuntimeSetWindowPosVariant::FrameChangeMove,
                RuntimeSetWindowPosCallbackMessage::NonClientCalculate,
            ) => {
                require_set_window_pos_callback_result(result, 0, "frame-change WM_NCCALCSIZE")?;
                self.window_geometry_update_disposition(
                    previous,
                    operation,
                    RuntimeSetWindowPosCallbackMessage::WindowPositionChanged,
                )?
            }
            (
                RuntimeSetWindowPosVariant::FrameChangeMove,
                RuntimeSetWindowPosCallbackMessage::WindowPositionChanged,
            ) => {
                require_set_window_pos_callback_result(
                    result,
                    0,
                    "frame-change WM_WINDOWPOSCHANGED",
                )?;
                RuntimeSetWindowPosCallbackDisposition::Validate {
                    action: RuntimeSetWindowPosCallbackAction::Completed { window: previous },
                }
            }
            (RuntimeSetWindowPosVariant::FrameChangeMove, _) => {
                return Err(RuntimeStateError::new(
                    "frame-change move received an out-of-order window callback",
                ));
            }
        };
        Ok(disposition)
    }

    pub(super) fn preview_set_window_pos_callback(
        &self,
        operation: RuntimeSetWindowPosOperation,
        message: RuntimeSetWindowPosCallbackMessage,
        result: u32,
    ) -> Result<RuntimeSetWindowPosCallbackAction, RuntimeStateError> {
        let disposition = self.checked_set_window_pos_callback(operation, message, result)?;
        Ok(match disposition {
            RuntimeSetWindowPosCallbackDisposition::Validate { action }
            | RuntimeSetWindowPosCallbackDisposition::Update { action, .. }
            | RuntimeSetWindowPosCallbackDisposition::Activate { action, .. } => action,
        })
    }

    #[cfg(test)]
    fn complete_set_window_pos_callback(
        &mut self,
        operation: RuntimeSetWindowPosOperation,
        message: RuntimeSetWindowPosCallbackMessage,
        result: u32,
    ) -> Result<RuntimeSetWindowPosCallbackAction, RuntimeStateError> {
        let disposition = self.checked_set_window_pos_callback(operation, message, result)?;
        Ok(self.publish_set_window_pos_callback(disposition))
    }

    pub(super) fn publish_set_window_pos_callback(
        &mut self,
        disposition: RuntimeSetWindowPosCallbackDisposition,
    ) -> RuntimeSetWindowPosCallbackAction {
        match disposition {
            RuntimeSetWindowPosCallbackDisposition::Validate { action } => action,
            RuntimeSetWindowPosCallbackDisposition::Update {
                next,
                action,
                next_generation,
            } => {
                let next = *next;
                self.generation = next_generation;
                self.windows.insert(next.id, next);
                action
            }
            RuntimeSetWindowPosCallbackDisposition::Activate {
                window_id,
                owner_thread_id,
                action,
                next_generation,
            } => {
                self.active_windows.insert(owner_thread_id, window_id);
                self.foreground_window = Some(window_id);
                self.generation = next_generation;
                action
            }
        }
    }

    pub(super) fn preview_update_window(
        &self,
        owner_thread_id: RuntimeThreadId,
        window_handle: u32,
        entry_last_error: u32,
    ) -> Result<RuntimeUpdateWindowOperation, RuntimeStateError> {
        self.resolve_window_procedure(window_handle, owner_thread_id)?;
        let window = self.window(window_handle)?;
        self.validate_window_icon_reference(window.small_icon)?;
        let expected_update_rect = full_client_update_rect(window.creation.geometry)?;
        let target = Some(window.id);
        if !window.visible
            || window.topmost
            || window.creation.effective_style & RUNTIME_WINDOW_STYLE_VISIBLE == 0
            || window.pending_update_rect != Some(expected_update_rect)
            || self.paint_sessions.contains_key(&window.id)
            || self.active_windows.get(&owner_thread_id).copied() != target
            || self.focused_windows.get(&owner_thread_id).copied() != target
            || self.foreground_window != target
        {
            return Err(RuntimeStateError::new(
                "synchronous paint requires the visible active, focused, foreground owner-thread window with one full-client update region",
            ));
        }
        Ok(RuntimeUpdateWindowOperation {
            window_id: window.id,
            window_handle: window.handle,
            owner_thread_id,
            expected_paint_generation: window.paint_generation,
            update_rect: expected_update_rect,
            entry_last_error,
        })
    }

    pub(super) fn validate_update_window_callback(
        &self,
        operation: RuntimeUpdateWindowOperation,
    ) -> Result<(), RuntimeStateError> {
        let window = self.window_by_id(operation.window_id)?;
        let target = Some(window.id);
        let expected_paint_generation = operation
            .expected_paint_generation
            .checked_add(1)
            .ok_or_else(|| {
                RuntimeStateError::new("logical window paint generation is exhausted")
            })?;
        if window.handle != operation.window_handle
            || window.creation.owner_thread_id != operation.owner_thread_id
            || window.pending_update_rect.is_some()
            || window.paint_generation != expected_paint_generation
            || self.paint_sessions.contains_key(&window.id)
            || full_client_update_rect(window.creation.geometry)? != operation.update_rect
            || !window.visible
            || window.topmost
            || window.creation.effective_style & RUNTIME_WINDOW_STYLE_VISIBLE == 0
            || self.active_windows.get(&operation.owner_thread_id).copied() != target
            || self
                .focused_windows
                .get(&operation.owner_thread_id)
                .copied()
                != target
            || self.foreground_window != target
        {
            return Err(RuntimeStateError::new(
                "logical window did not complete the expected full-client paint cycle",
            ));
        }
        self.validate_created_window_owner(window, operation.owner_thread_id)?;
        self.validate_window_icon_reference(window.small_icon)?;
        self.validate_window_creation_references(&window.creation)?;
        Ok(())
    }

    pub(super) fn validate_begin_paint(
        &self,
        owner_thread_id: RuntimeThreadId,
        window_handle: u32,
        paint_struct_address: GuestAddress,
        device_context_id: RuntimeDeviceContextId,
        device_context_handle: u32,
    ) -> Result<RuntimePaintSession, RuntimeStateError> {
        if paint_struct_address.0 == 0 || device_context_id.0 == 0 || device_context_handle == 0 {
            return Err(RuntimeStateError::new(
                "BeginPaint requires the reached non-NULL PAINTSTRUCT, logical HDC, and erase-pending variant",
            ));
        }
        let window = self.window(window_handle)?;
        self.validate_created_window_owner(window, owner_thread_id)?;
        self.validate_window_creation_references(&window.creation)?;
        self.validate_window_icon_reference(window.small_icon)?;
        let update_rect = full_client_update_rect(window.creation.geometry)?;
        let target = Some(window.id);
        if window.pending_update_rect != Some(update_rect)
            || self.paint_sessions.contains_key(&window.id)
            || !window.visible
            || window.topmost
            || window.creation.effective_style & RUNTIME_WINDOW_STYLE_VISIBLE == 0
            || self.active_windows.get(&owner_thread_id).copied() != target
            || self.focused_windows.get(&owner_thread_id).copied() != target
            || self.foreground_window != target
        {
            return Err(RuntimeStateError::new(
                "BeginPaint requires one visible active owner-thread window with a full-client update region and no live paint session",
            ));
        }
        let paint_generation = window.paint_generation.checked_add(1).ok_or_else(|| {
            RuntimeStateError::new("logical window paint generation is exhausted")
        })?;
        let paint_struct_bytes = x86_paint_struct_bytes(device_context_handle, true, update_rect);
        let session = RuntimePaintSession {
            window_id: window.id,
            window_handle: window.handle,
            owner_thread_id,
            paint_generation,
            update_rect,
            device_context_id,
            device_context_handle,
            paint_struct_address,
            paint_struct_bytes,
        };
        self.next_generation()?;
        Ok(session)
    }

    // GDI identity and guest PAINTSTRUCT output are checked before publication.
    pub(super) fn finish_begin_paint(&mut self, session: RuntimePaintSession) {
        let window = self
            .windows
            .get_mut(&session.window_id)
            .expect("validated paint window");
        window.pending_update_rect = None;
        self.paint_sessions.insert(session.window_id, session);
        self.generation += 1;
    }

    pub(super) fn validate_end_paint(
        &self,
        owner_thread_id: RuntimeThreadId,
        window_handle: u32,
        paint_struct_address: GuestAddress,
        paint_struct_bytes: &[u8],
    ) -> Result<&RuntimePaintSession, RuntimeStateError> {
        let window = self.window(window_handle)?;
        self.validate_created_window_owner(window, owner_thread_id)?;
        let session = self.paint_sessions.get(&window.id).ok_or_else(|| {
            RuntimeStateError::new(
                "EndPaint requires a live BeginPaint session for the exact logical window",
            )
        })?;
        if session.window_handle != window_handle
            || session.owner_thread_id != owner_thread_id
            || session.paint_struct_address != paint_struct_address
            || session.paint_struct_bytes != paint_struct_bytes
            || window.pending_update_rect.is_some()
            || window.paint_generation.checked_add(1) != Some(session.paint_generation)
        {
            return Err(RuntimeStateError::new(
                "EndPaint requires the exact HWND, PAINTSTRUCT bytes, owner thread, and active paint generation",
            ));
        }
        self.validate_window_creation_references(&window.creation)?;
        self.validate_window_icon_reference(window.small_icon)?;
        self.next_generation()?;
        Ok(session)
    }

    // Called only after both User32 and the owning GDI HDC pass validation;
    // no callback or host operation can interleave with the GDI release.
    pub(super) fn finish_end_paint(&mut self, window_id: RuntimeWindowId, paint_generation: u64) {
        let window = self
            .windows
            .get_mut(&window_id)
            .expect("validated paint window");
        window.paint_generation = paint_generation;
        self.paint_sessions.remove(&window_id);
        self.generation += 1;
    }

    pub(super) fn require_non_paint_device_context(
        &self,
        device_context_id: RuntimeDeviceContextId,
        device_context_handle: u32,
    ) -> Result<(), RuntimeStateError> {
        if self.paint_sessions.values().any(|session| {
            session.device_context_id == device_context_id
                || session.device_context_handle == device_context_handle
        }) {
            return Err(RuntimeStateError::new(
                "a BeginPaint HDC must be retired through the matching EndPaint session",
            ));
        }
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn paint_sessions(&self) -> &BTreeMap<RuntimeWindowId, RuntimePaintSession> {
        &self.paint_sessions
    }

    fn window_position_update_disposition(
        &self,
        previous: RuntimeWindow,
        operation: RuntimeSetWindowPosOperation,
    ) -> Result<RuntimeSetWindowPosCallbackDisposition, RuntimeStateError> {
        let mut next = previous.clone();
        next.creation.geometry = operation.next_geometry;
        next.creation.effective_style = operation.next_effective_style;
        next.visible = operation.next_visible;
        next.topmost = operation.next_topmost;
        Ok(RuntimeSetWindowPosCallbackDisposition::Update {
            action: RuntimeSetWindowPosCallbackAction::Completed {
                window: next.clone(),
            },
            next: Box::new(next),
            next_generation: self.next_generation()?,
        })
    }

    fn window_resize_update_disposition(
        &self,
        previous: RuntimeWindow,
        operation: RuntimeSetWindowPosOperation,
    ) -> Result<RuntimeSetWindowPosCallbackDisposition, RuntimeStateError> {
        self.window_geometry_update_disposition(
            previous,
            operation,
            RuntimeSetWindowPosCallbackMessage::Size,
        )
    }

    fn window_geometry_update_disposition(
        &self,
        previous: RuntimeWindow,
        operation: RuntimeSetWindowPosOperation,
        next_message: RuntimeSetWindowPosCallbackMessage,
    ) -> Result<RuntimeSetWindowPosCallbackDisposition, RuntimeStateError> {
        let next_generation = self.next_generation()?;
        let mut next = previous.clone();
        next.creation.geometry = operation.next_geometry;
        next.pending_update_rect = operation.next_update_rect;
        let mut next_operation = operation;
        next_operation.expected_generation = next_generation;
        Ok(RuntimeSetWindowPosCallbackDisposition::Update {
            action: RuntimeSetWindowPosCallbackAction::Continue {
                window: next.clone(),
                operation: next_operation,
                next_message,
            },
            next: Box::new(next),
            next_generation,
        })
    }

    fn validate_set_window_pos_operation(
        &self,
        operation: RuntimeSetWindowPosOperation,
        message: RuntimeSetWindowPosCallbackMessage,
    ) -> Result<&RuntimeWindow, RuntimeStateError> {
        let focus_committed = operation.variant == RuntimeSetWindowPosVariant::ActivateSameGeometry
            && message == RuntimeSetWindowPosCallbackMessage::Activate;
        let expected_generation = if focus_committed {
            operation.expected_generation.checked_add(1)
        } else {
            Some(operation.expected_generation)
        };
        if Some(self.generation) != expected_generation {
            return Err(RuntimeStateError::new(
                "User32 state changed after window-position callback preparation",
            ));
        }
        let window = self.window_by_id(operation.window_id)?;
        let expected_next_effective_style = if operation.next_visible {
            operation.previous_effective_style | RUNTIME_WINDOW_STYLE_VISIBLE
        } else {
            operation.previous_effective_style & !RUNTIME_WINDOW_STYLE_VISIBLE
        };
        let geometry_committed = matches!(
            operation.variant,
            RuntimeSetWindowPosVariant::Resize | RuntimeSetWindowPosVariant::FrameChangeMove
        ) && matches!(
            message,
            RuntimeSetWindowPosCallbackMessage::Size
                | RuntimeSetWindowPosCallbackMessage::WindowPositionChanged
        );
        let expected_geometry = if geometry_committed {
            operation.next_geometry
        } else {
            operation.previous_geometry
        };
        let update_rect_committed = operation.variant
            == RuntimeSetWindowPosVariant::FrameChangeMove
            && message == RuntimeSetWindowPosCallbackMessage::WindowPositionChanged;
        let expected_update_rect = if update_rect_committed {
            operation.next_update_rect
        } else {
            operation.previous_update_rect
        };
        let derived_next_update_rect = match operation.variant {
            RuntimeSetWindowPosVariant::FrameChangeMove => {
                Some(full_client_update_rect(operation.next_geometry)?)
            }
            _ => operation.previous_update_rect,
        };
        if window.handle != operation.window_handle
            || window.creation.geometry != expected_geometry
            || window.visible != operation.previous_visible
            || window.topmost != operation.previous_topmost
            || window.creation.effective_style != operation.previous_effective_style
            || operation.next_effective_style != expected_next_effective_style
            || window.pending_update_rect != expected_update_rect
            || operation.next_update_rect != derived_next_update_rect
            || window.small_icon.map_or(0, |icon| icon.handle) != operation.small_icon_handle
        {
            return Err(RuntimeStateError::new(
                "logical window state changed after window-position callback preparation",
            ));
        }
        self.validate_created_window_owner(window, window.creation.owner_thread_id)?;
        self.validate_window_icon_reference(window.small_icon)?;
        if operation.variant == RuntimeSetWindowPosVariant::ActivateSameGeometry {
            let owner_thread_id = window.creation.owner_thread_id;
            let target = Some(window.id);
            let (expected_active, expected_focused, expected_foreground) =
                if message == RuntimeSetWindowPosCallbackMessage::WindowPositionChanging {
                    (None, None, None)
                } else if focus_committed {
                    (target, target, target)
                } else {
                    (target, None, target)
                };
            if self.active_windows.get(&owner_thread_id).copied() != expected_active
                || self.focused_windows.get(&owner_thread_id).copied() != expected_focused
                || self.foreground_window != expected_foreground
            {
                return Err(RuntimeStateError::new(
                    "logical active, focused, or foreground relation changed during window positioning",
                ));
            }
        }
        if operation.variant == RuntimeSetWindowPosVariant::RepeatActivation {
            let owner_thread_id = window.creation.owner_thread_id;
            let target = Some(window.id);
            if self.active_windows.get(&owner_thread_id).copied() != target
                || self.focused_windows.get(&owner_thread_id).copied() != target
                || self.foreground_window != target
            {
                return Err(RuntimeStateError::new(
                    "logical active, focused, or foreground relation changed during repeat activation",
                ));
            }
        }
        if matches!(
            operation.variant,
            RuntimeSetWindowPosVariant::Resize | RuntimeSetWindowPosVariant::FrameChangeMove
        ) {
            let owner_thread_id = window.creation.owner_thread_id;
            let target = Some(window.id);
            if self.active_windows.get(&owner_thread_id).copied() != target
                || self.focused_windows.get(&owner_thread_id).copied() != target
                || self.foreground_window != target
            {
                return Err(RuntimeStateError::new(
                    "logical active, focused, or foreground relation changed during resize or frame-change move",
                ));
            }
        }
        Ok(window)
    }

    pub(super) fn preview_set_window_pos_focus(
        &self,
        window_handle: u32,
        owner_thread_id: RuntimeThreadId,
    ) -> Result<RuntimeSetWindowPosDefaultOperation, RuntimeStateError> {
        let window = self.window(window_handle)?.clone();
        self.validate_created_window_owner(&window, owner_thread_id)?;
        if self.active_windows.get(&owner_thread_id).copied() != Some(window.id)
            || self.focused_windows.contains_key(&owner_thread_id)
            || self.foreground_window != Some(window.id)
        {
            return Err(RuntimeStateError::new(
                "activation default processing requires the active foreground window without logical focus",
            ));
        }
        Ok(RuntimeSetWindowPosDefaultOperation {
            window_id: window.id,
            window_handle: window.handle,
            owner_thread_id,
            expected_generation: self.next_generation()?,
        })
    }

    pub(super) fn focus_set_window_pos_window(
        &mut self,
        window_handle: u32,
        owner_thread_id: RuntimeThreadId,
    ) -> Result<RuntimeSetWindowPosDefaultOperation, RuntimeStateError> {
        // Preflight of the callback frame is synchronous; no guest code or host
        // operation can run between preview and this fresh owner validation.
        let operation = self.preview_set_window_pos_focus(window_handle, owner_thread_id)?;
        self.focused_windows
            .insert(owner_thread_id, operation.window_id);
        self.generation = operation.expected_generation;
        Ok(operation)
    }

    pub(super) fn set_window_pos_default_callback_action(
        &self,
        operation: RuntimeSetWindowPosDefaultOperation,
        message: RuntimeSetWindowPosDefaultCallbackMessage,
        result: u32,
    ) -> Result<RuntimeSetWindowPosDefaultCallbackAction, RuntimeStateError> {
        self.validate_set_window_pos_default_operation(operation)?;
        require_set_window_pos_callback_result(result, 0, "activation default callback")?;
        Ok(match message {
            RuntimeSetWindowPosDefaultCallbackMessage::ImeSetContext => {
                RuntimeSetWindowPosDefaultCallbackAction::Continue {
                    operation,
                    next_message: RuntimeSetWindowPosDefaultCallbackMessage::SetFocus,
                }
            }
            RuntimeSetWindowPosDefaultCallbackMessage::SetFocus => {
                RuntimeSetWindowPosDefaultCallbackAction::Completed
            }
        })
    }

    fn validate_set_window_pos_default_operation(
        &self,
        operation: RuntimeSetWindowPosDefaultOperation,
    ) -> Result<(), RuntimeStateError> {
        if self.generation != operation.expected_generation {
            return Err(RuntimeStateError::new(
                "User32 state changed during activation default processing",
            ));
        }
        let window = self.window_by_id(operation.window_id)?;
        if window.handle != operation.window_handle
            || window.creation.owner_thread_id != operation.owner_thread_id
            || self.active_windows.get(&operation.owner_thread_id).copied() != Some(window.id)
            || self
                .focused_windows
                .get(&operation.owner_thread_id)
                .copied()
                != Some(window.id)
            || self.foreground_window != Some(window.id)
        {
            return Err(RuntimeStateError::new(
                "logical activation identity changed during default callback processing",
            ));
        }
        self.validate_created_window_owner(window, operation.owner_thread_id)
    }

    pub fn active_window_handle(
        &self,
        owner_thread_id: RuntimeThreadId,
    ) -> Result<Option<u32>, RuntimeStateError> {
        self.window_relation_handle(self.active_windows.get(&owner_thread_id).copied())
    }

    pub fn focused_window_handle(
        &self,
        owner_thread_id: RuntimeThreadId,
    ) -> Result<Option<u32>, RuntimeStateError> {
        self.window_relation_handle(self.focused_windows.get(&owner_thread_id).copied())
    }

    pub fn foreground_window_handle(&self) -> Result<Option<u32>, RuntimeStateError> {
        self.window_relation_handle(self.foreground_window)
    }

    pub(super) fn resolve_same_window_focus(
        &self,
        window_handle: u32,
        owner_thread_id: RuntimeThreadId,
    ) -> Result<u32, RuntimeStateError> {
        let window = self.window(window_handle)?;
        self.validate_created_window_owner(window, owner_thread_id)?;
        if self.active_windows.get(&owner_thread_id).copied() != Some(window.id)
            || self.focused_windows.get(&owner_thread_id).copied() != Some(window.id)
            || self.foreground_window != Some(window.id)
        {
            return Err(RuntimeStateError::new(
                "bounded SetFocus requires the current-thread active, focused, foreground logical window",
            ));
        }
        Ok(window.handle)
    }

    fn window_relation_handle(
        &self,
        window_id: Option<RuntimeWindowId>,
    ) -> Result<Option<u32>, RuntimeStateError> {
        window_id
            .map(|id| self.window_by_id(id).map(|window| window.handle))
            .transpose()
    }

    fn validate_created_window_owner(
        &self,
        window: &RuntimeWindow,
        owner_thread_id: RuntimeThreadId,
    ) -> Result<(), RuntimeStateError> {
        if window.creation.owner_thread_id != owner_thread_id {
            return Err(RuntimeStateError::new(
                "synchronous logical window messages require the selected owner thread",
            ));
        }
        if window.lifecycle != RuntimeWindowLifecycle::Created {
            return Err(RuntimeStateError::new(
                "synchronous logical window messages require a created window",
            ));
        }
        self.validate_window_creation_references(&window.creation)
    }

    fn validate_ime_window_identity(
        &self,
        window: &RuntimeImeWindow,
    ) -> Result<(), RuntimeStateError> {
        if window.id.0 == 0
            || window.handle < RUNTIME_WINDOW_HANDLE_BASE
            || window.class_name != "IME"
            || self.ime_window_handle_index.get(&window.handle) != Some(&window.id)
            || self.window_handle_index.contains_key(&window.handle)
        {
            return Err(RuntimeStateError::new(
                "logical default IME window does not match its indexes",
            ));
        }
        Ok(())
    }

    fn validate_window_icon_reference(
        &self,
        reference: Option<RuntimeWindowIconReference>,
    ) -> Result<(), RuntimeStateError> {
        let Some(reference) = reference else {
            return Ok(());
        };
        let icon = self.icon(reference.handle)?;
        if icon.id != reference.icon_id {
            return Err(RuntimeStateError::new(
                "logical window icon handle and icon identity do not match",
            ));
        }
        Ok(())
    }

    pub fn window_by_id(&self, id: RuntimeWindowId) -> Result<&RuntimeWindow, RuntimeStateError> {
        let window = self
            .windows
            .get(&id)
            .ok_or_else(|| RuntimeStateError::new("logical window ID is not live"))?;
        if self.window_handle_index.get(&window.handle) != Some(&id) {
            return Err(RuntimeStateError::new(
                "logical window ID does not match its HWND index",
            ));
        }
        Ok(window)
    }

    #[cfg(test)]
    pub(crate) fn windows(&self) -> &BTreeMap<RuntimeWindowId, RuntimeWindow> {
        &self.windows
    }

    pub(super) fn system_menu_option(
        &self,
        handle: u32,
    ) -> Result<Option<&RuntimeSystemMenu>, RuntimeStateError> {
        let Some(id) = self.menu_handle_index.get(&handle).copied() else {
            return Ok(None);
        };
        let menu = self.system_menus.get(&id).ok_or_else(|| {
            RuntimeStateError::new("logical HMENU index does not resolve to a live menu")
        })?;
        if menu.id != id || menu.handle != handle {
            return Err(RuntimeStateError::new(
                "logical HMENU index does not match its menu identity",
            ));
        }
        self.validate_system_menu_identity(menu)?;
        Ok(Some(menu))
    }

    #[cfg(test)]
    pub(crate) fn system_menu(&self, handle: u32) -> Result<&RuntimeSystemMenu, RuntimeStateError> {
        self.system_menu_option(handle)?.ok_or_else(|| {
            RuntimeStateError::new(format!("logical HMENU {handle:#010x} is not live"))
        })
    }

    #[cfg(test)]
    pub(crate) fn system_menus(&self) -> &BTreeMap<RuntimeMenuId, RuntimeSystemMenu> {
        &self.system_menus
    }

    fn validate_system_menu_identity(
        &self,
        menu: &RuntimeSystemMenu,
    ) -> Result<(), RuntimeStateError> {
        let owner_window = self.windows.get(&menu.owner_window_id).ok_or_else(|| {
            RuntimeStateError::new("logical system menu refers to a non-live owner window")
        })?;
        if menu.id.0 == 0
            || menu.handle < RUNTIME_MENU_HANDLE_BASE
            || menu.close_item_command == 0
            || !menu.close_item_enabled
            || owner_window.handle != menu.owner_window_handle
            || owner_window.lifecycle != RuntimeWindowLifecycle::Created
            || self.window_handle_index.get(&menu.owner_window_handle)
                != Some(&menu.owner_window_id)
            || self.system_menu_window_index.get(&menu.owner_window_id) != Some(&menu.id)
            || self.menu_handle_index.get(&menu.handle) != Some(&menu.id)
        {
            return Err(RuntimeStateError::new(
                "logical system menu identity, owner relation, or close-item state is inconsistent",
            ));
        }
        Ok(())
    }

    fn next_generation(&self) -> Result<u64, RuntimeStateError> {
        self.generation
            .checked_add(1)
            .ok_or_else(|| RuntimeStateError::new("User32 resource generation is exhausted"))
    }
}

fn require_set_window_pos_callback_result(
    actual: u32,
    expected: u32,
    callback: &'static str,
) -> Result<(), RuntimeStateError> {
    if actual != expected {
        return Err(RuntimeStateError::new(format!(
            "{callback} must return the observed value {expected:#010x}"
        )));
    }
    Ok(())
}

fn validate_icon_resource(resource: &RuntimeIconResource) -> Result<(), RuntimeStateError> {
    if resource.module_id.0 == 0
        || resource.module_load_base.0 == 0
        || resource.group_resource_id > u32::from(u16::MAX)
        || resource.language_id > u32::from(u16::MAX)
        || resource.icon_resource_id > u32::from(u16::MAX)
        || resource.group_data_rva.0 == 0
        || resource.group_data_size == 0
        || resource.icon_data_rva.0 == 0
        || resource.icon_data_size == 0
        || resource.width == 0
        || resource.width > 256
        || resource.height == 0
        || resource.height > 256
        || resource.planes == 0
        || resource.bits_per_pixel == 0
    {
        return Err(RuntimeStateError::new(
            "shared icon resource identity and dimensions must be bounded PE32 metadata",
        ));
    }
    Ok(())
}

fn validate_cursor_source(source: &RuntimeCursorSource) -> Result<(), RuntimeStateError> {
    match source {
        RuntimeCursorSource::SystemPredefined {
            resource_id,
            cursor: RuntimeSystemCursorKind::Arrow,
        } if *resource_id == RUNTIME_SYSTEM_CURSOR_ARROW_RESOURCE_ID => Ok(()),
        RuntimeCursorSource::SystemPredefined { .. } => Err(RuntimeStateError::new(
            "logical system cursor source does not match its predefined cursor identity",
        )),
        RuntimeCursorSource::ModuleResource { resource } => validate_cursor_resource(resource),
    }
}

fn validate_cursor_resource(resource: &RuntimeCursorResource) -> Result<(), RuntimeStateError> {
    if resource.module_id.0 == 0
        || resource.module_load_base.0 == 0
        || resource.group_resource_id > u32::from(u16::MAX)
        || resource.language_id > u32::from(u16::MAX)
        || resource.cursor_resource_id > u32::from(u16::MAX)
        || resource.group_data_rva.0 == 0
        || resource.group_data_size == 0
        || resource.cursor_data_rva.0 == 0
        || resource.cursor_data_size < 44
        || resource.width == 0
        || resource.width > 256
        || resource.height == 0
        || resource.height > 256
        || resource.planes == 0
        || resource.bits_per_pixel == 0
        || resource.hotspot_x >= resource.width
        || resource.hotspot_y >= resource.height
    {
        return Err(RuntimeStateError::new(
            "shared cursor resource identity, dimensions, and hotspot must be bounded PE32 metadata",
        ));
    }
    Ok(())
}

fn validate_window_class_registration(
    registration: &RuntimeWindowClassRegistration,
) -> Result<(), RuntimeStateError> {
    let module = &registration.module;
    let window_procedure = &registration.window_procedure;
    let expected_window_procedure_address = module
        .load_base
        .0
        .checked_add(window_procedure.rva.0)
        .map(GuestAddress);
    if module.module_id.0 == 0
        || module.load_base.0 == 0
        || window_procedure.module_id != module.module_id
        || window_procedure.address.0 == 0
        || window_procedure.rva.0 == 0
        || expected_window_procedure_address != Some(window_procedure.address)
    {
        return Err(RuntimeStateError::new(
            "window class requires a mapped module identity and same-module x86 stdcall WndProc descriptor",
        ));
    }
    validate_window_class_name(&registration.class_name, false)?;
    if let Some(menu_name) = registration.menu_name.as_ref() {
        validate_window_class_name(menu_name, true)?;
    }
    Ok(())
}

fn validate_window_class_name(name: &str, allow_empty: bool) -> Result<(), RuntimeStateError> {
    let (bytes, _, had_errors) = SHIFT_JIS.encode(name);
    if (!allow_empty && name.is_empty())
        || had_errors
        || bytes.len() > RUNTIME_WINDOW_CLASS_NAME_A_LIMIT
        || name.contains('\0')
    {
        return Err(RuntimeStateError::new(
            "window class and menu names require bounded copied Windows-932 text",
        ));
    }
    Ok(())
}

fn full_client_update_rect(
    geometry: RuntimeWindowGeometry,
) -> Result<RuntimeRect, RuntimeStateError> {
    let right = i32::try_from(geometry.client_width).map_err(|_| {
        RuntimeStateError::new(
            "logical update-region width must fit the x86 RECT coordinate domain",
        )
    })?;
    let bottom = i32::try_from(geometry.client_height).map_err(|_| {
        RuntimeStateError::new(
            "logical update-region height must fit the x86 RECT coordinate domain",
        )
    })?;
    Ok(RuntimeRect {
        left: 0,
        top: 0,
        right,
        bottom,
    })
}

fn x86_paint_struct_bytes(
    device_context_handle: u32,
    erase: bool,
    update_rect: RuntimeRect,
) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(RUNTIME_X86_PAINT_STRUCT_BYTE_LENGTH);
    bytes.extend_from_slice(&device_context_handle.to_le_bytes());
    bytes.extend_from_slice(&u32::from(erase).to_le_bytes());
    bytes.extend_from_slice(&update_rect.left.to_le_bytes());
    bytes.extend_from_slice(&update_rect.top.to_le_bytes());
    bytes.extend_from_slice(&update_rect.right.to_le_bytes());
    bytes.extend_from_slice(&update_rect.bottom.to_le_bytes());
    bytes.extend_from_slice(&0_u32.to_le_bytes());
    bytes.extend_from_slice(&0_u32.to_le_bytes());
    bytes.extend_from_slice(&[0; 32]);
    debug_assert_eq!(bytes.len(), RUNTIME_X86_PAINT_STRUCT_BYTE_LENGTH);
    bytes
}

fn validate_position_geometry_transition(
    previous: RuntimeWindowGeometry,
    next: RuntimeWindowGeometry,
    variant: RuntimeSetWindowPosVariant,
) -> Result<(), RuntimeStateError> {
    let previous_client_offset_x = i64::from(previous.client_x) - i64::from(previous.x);
    let previous_client_offset_y = i64::from(previous.client_y) - i64::from(previous.y);
    let next_client_offset_x = i64::from(next.client_x) - i64::from(next.x);
    let next_client_offset_y = i64::from(next.client_y) - i64::from(next.y);
    let previous_client_right_extent =
        i64::from(previous.width) - previous_client_offset_x - i64::from(previous.client_width);
    let previous_client_bottom_extent =
        i64::from(previous.height) - previous_client_offset_y - i64::from(previous.client_height);
    let next_client_right_extent =
        i64::from(next.width) - next_client_offset_x - i64::from(next.client_width);
    let next_client_bottom_extent =
        i64::from(next.height) - next_client_offset_y - i64::from(next.client_height);
    let valid = if variant == RuntimeSetWindowPosVariant::Resize {
        next.x == previous.x
            && next.y == previous.y
            && next.client_x == previous.client_x
            && next.client_y == previous.client_y
            && next.width != previous.width
            && next.height != previous.height
            && next.client_width != previous.client_width
            && next.client_height != previous.client_height
            && next_client_offset_x == previous_client_offset_x
            && next_client_offset_y == previous_client_offset_y
            && next_client_right_extent == previous_client_right_extent
            && next_client_bottom_extent == previous_client_bottom_extent
    } else {
        next.width == previous.width
            && next.height == previous.height
            && next.client_width == previous.client_width
            && next.client_height == previous.client_height
            && next_client_offset_x == previous_client_offset_x
            && next_client_offset_y == previous_client_offset_y
            && (variant != RuntimeSetWindowPosVariant::FrameChangeMove
                || (next.x != previous.x && next.y != previous.y))
    };
    if !valid {
        return Err(RuntimeStateError::new(
            "bounded positioning must preserve non-client extents and either preserve all sizes or perform the reached same-position resize",
        ));
    }
    Ok(())
}

fn validate_window_creation_request(
    request: &RuntimeWindowCreationRequest,
) -> Result<(), RuntimeStateError> {
    let geometry = request.geometry;
    let outer_right = i64::from(geometry.x) + i64::from(geometry.width);
    let outer_bottom = i64::from(geometry.y) + i64::from(geometry.height);
    let client_right = i64::from(geometry.client_x) + i64::from(geometry.client_width);
    let client_bottom = i64::from(geometry.client_y) + i64::from(geometry.client_height);
    if request.owner_thread_id.0 == 0
        || request.class.class_id.0 == 0
        || request.class.atom < RUNTIME_WINDOW_CLASS_ATOM_BASE
        || request.class.module_id.0 == 0
        || request.instance.module_id != request.class.module_id
        || request.instance.load_base.0 == 0
        || request.name.value.is_empty()
        || request.name.windows_932_bytes().is_err()
        || request.style == 0
        || request.effective_style & request.style != request.style
        || request.effective_extended_style & request.extended_style != request.extended_style
        || request.parent != 0
        || request.menu != 0
        || request.create_parameter != 0
        || geometry.width == 0
        || geometry.height == 0
        || geometry.client_width == 0
        || geometry.client_height == 0
        || geometry.width > 32_768
        || geometry.height > 32_768
        || geometry.client_width > geometry.width
        || geometry.client_height > geometry.height
        || geometry.client_x < geometry.x
        || geometry.client_y < geometry.y
        || client_right > outer_right
        || client_bottom > outer_bottom
        || outer_right > i64::from(i32::MAX)
        || outer_bottom > i64::from(i32::MAX)
    {
        return Err(RuntimeStateError::new(
            "logical window creation requires bounded copied text, a live class/module relation, NULL unowned relations, and contained client geometry",
        ));
    }
    Ok(())
}

fn window_class_key(
    registration: &RuntimeWindowClassRegistration,
) -> Result<RuntimeWindowClassKey, RuntimeStateError> {
    window_class_key_from_name(registration.module.module_id, &registration.class_name)
}

fn window_class_key_from_name(
    module_id: RuntimeModuleId,
    name: &str,
) -> Result<RuntimeWindowClassKey, RuntimeStateError> {
    let (bytes, _, had_errors) = SHIFT_JIS.encode(name);
    if module_id.0 == 0
        || name.is_empty()
        || had_errors
        || bytes.len() > RUNTIME_WINDOW_CLASS_NAME_A_LIMIT
        || name.contains('\0')
    {
        return Err(RuntimeStateError::new(
            "window class lookup requires a live module and bounded nonempty Windows-932 name",
        ));
    }
    Ok(RuntimeWindowClassKey {
        module_id,
        canonical_name: name.to_ascii_lowercase(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_lookup_distinguishes_absence_from_a_dangling_handle_index() {
        let mut user32 = RuntimeUser32::default();
        let handle = RUNTIME_WINDOW_HANDLE_BASE;
        assert!(user32.window_option(handle).unwrap().is_none());
        user32
            .window_handle_index
            .insert(handle, RuntimeWindowId(1));
        user32.window_option(handle).unwrap_err();
        user32.window(handle).unwrap_err();
    }

    #[test]
    fn window_lookup_rejects_forged_handle_alias_without_exposing_the_window() {
        let (mut user32, handle, _) = created_window_with_small_icon();
        let window_id = user32.window(handle).unwrap().id;
        let forged_handle = handle + 1;
        assert!(user32.window_option(forged_handle).unwrap().is_none());
        user32.window_handle_index.insert(forged_handle, window_id);
        user32.window_option(forged_handle).unwrap_err();
        assert_eq!(user32.window(handle).unwrap().id, window_id);
    }

    #[test]
    fn icon_and_cursor_lookups_reject_forged_handle_aliases() {
        let mut user32 = RuntimeUser32::default();
        let icon = user32.load_shared_icon(resource(101, 3, 0x3000)).unwrap();
        let cursor = user32.load_shared_cursor(arrow_cursor()).unwrap();
        let forged_icon = icon + 1;
        let forged_cursor = cursor + 1;
        assert!(user32.icon_option(forged_icon).unwrap().is_none());
        assert!(user32.cursor_option(forged_cursor).unwrap().is_none());
        user32.icon(forged_icon).unwrap_err();
        user32.cursor(forged_cursor).unwrap_err();
        user32
            .handle_index
            .insert(forged_icon, user32.icon(icon).unwrap().id);
        user32
            .cursor_handle_index
            .insert(forged_cursor, user32.cursor(cursor).unwrap().id);
        let before = user32.clone();
        user32.icon_option(forged_icon).unwrap_err();
        user32.cursor_option(forged_cursor).unwrap_err();
        user32.icon(forged_icon).unwrap_err();
        user32.cursor(forged_cursor).unwrap_err();
        user32.set_current_cursor(Some(forged_cursor)).unwrap_err();
        assert_eq!(user32, before);
        assert_eq!(user32.icon(icon).unwrap().id, user32.icon(icon).unwrap().id);
        assert_eq!(
            user32.cursor(cursor).unwrap().id,
            user32.cursor(cursor).unwrap().id
        );
    }

    #[test]
    fn icon_resource_and_cursor_source_indexes_reject_live_identity_aliases() {
        let mut user32 = RuntimeUser32::default();
        let first_icon = resource(101, 3, 0x3000);
        let second_icon = resource(102, 4, 0x4000);
        let first_cursor = arrow_cursor();
        let second_cursor = module_cursor(106, 5, 0x6000);
        let icon = user32.load_shared_icon(second_icon).unwrap();
        let cursor = user32.load_shared_cursor(second_cursor).unwrap();
        let icon_id = user32.icon(icon).unwrap().id;
        let cursor_id = user32.cursor(cursor).unwrap().id;
        user32.resource_index.insert(first_icon.clone(), icon_id);
        user32
            .cursor_source_index
            .insert(first_cursor.clone(), cursor_id);
        let before = user32.clone();
        user32.load_shared_icon(first_icon).unwrap_err();
        user32.load_shared_cursor(first_cursor.clone()).unwrap_err();
        user32.cursor_for_source(&first_cursor).unwrap_err();
        assert_eq!(user32, before);
    }

    #[test]
    fn shared_icons_reuse_identity() {
        let mut user32 = RuntimeUser32::default();
        let first_resource = resource(101, 3, 0x3000);
        assert!(user32.icons().is_empty());
        let first = user32.load_shared_icon(first_resource.clone()).unwrap();
        assert_eq!(user32.icon(first).unwrap().id, RuntimeIconId(1));
        assert_eq!(first, RUNTIME_ICON_HANDLE_BASE);

        let before_reuse = user32.clone();
        let repeated = user32.load_shared_icon(first_resource).unwrap();
        assert_eq!(repeated, first);
        assert_eq!(user32, before_reuse);

        let next = user32.load_shared_icon(resource(102, 4, 0x4000)).unwrap();
        assert_eq!(user32.icon(next).unwrap().id, RuntimeIconId(2));

        assert_eq!(user32.icons().len(), 2);
    }

    #[test]
    fn shared_icon_capacity_failure_preserves_identity_and_allocators() {
        let mut user32 = RuntimeUser32::default();
        for ordinal in 1..=RUNTIME_SHARED_ICON_LIMIT {
            let ordinal = u32::try_from(ordinal).unwrap();
            let result = user32
                .load_shared_icon(resource(ordinal, ordinal, 0x3000 + ordinal))
                .unwrap();
            assert_eq!(user32.icon(result).unwrap().id.0, u64::from(ordinal));
        }
        let before = user32.clone();
        let _error = user32
            .load_shared_icon(resource(4_097, 4_097, 0x5000))
            .expect_err("shared icon capacity must be bounded");

        assert_eq!(user32, before);
    }

    #[test]
    fn shared_cursor_identity_selection_and_failures_are_atomic() {
        let mut user32 = RuntimeUser32::default();
        user32.load_shared_icon(resource(101, 3, 0x3000)).unwrap();

        let first = user32.load_shared_cursor(arrow_cursor()).unwrap();
        assert_eq!(user32.cursor(first).unwrap().id, RuntimeCursorId(1));
        assert_eq!(first, RUNTIME_CURSOR_HANDLE_BASE);

        assert_eq!(
            user32.current_cursor().unwrap().map(|cursor| cursor.id),
            Some(user32.cursor(first).unwrap().id)
        );
        let before_reuse = user32.clone();
        let repeated = user32.load_shared_cursor(arrow_cursor()).unwrap();
        assert_eq!(repeated, first);
        assert_eq!(user32, before_reuse);

        assert_eq!(user32.cursors().len(), 1);

        let module = user32
            .load_shared_cursor(module_cursor(106, 5, 0x6000))
            .unwrap();
        assert_eq!(user32.cursor(module).unwrap().id, RuntimeCursorId(2));
        assert_eq!(module, RUNTIME_CURSOR_HANDLE_BASE + 1);

        let before_reuse = user32.clone();
        let repeated_module = user32
            .load_shared_cursor(module_cursor(106, 5, 0x6000))
            .unwrap();
        assert_eq!(repeated_module, module);
        assert_eq!(user32, before_reuse);

        assert_eq!(user32.cursors().len(), 2);
        assert_eq!(
            user32.current_cursor().unwrap().map(|cursor| cursor.id),
            Some(user32.cursor(first).unwrap().id)
        );

        assert_eq!(user32.set_current_cursor(Some(module)).unwrap(), first);
        assert_eq!(
            user32.current_cursor().unwrap().map(|cursor| cursor.id),
            Some(user32.cursor(module).unwrap().id)
        );
        assert_eq!(user32.set_current_cursor(None).unwrap(), module);
        assert_eq!(user32.current_cursor().unwrap(), None);

        let before_invalid_handle = user32.clone();
        user32
            .set_current_cursor(Some(RUNTIME_CURSOR_HANDLE_BASE + 99))
            .unwrap_err();
        assert_eq!(user32, before_invalid_handle);

        // Re-loading an existing arrow is a resource lookup, not an implicit SetCursor call.
        let before_reload = user32.clone();
        let repeated_after_clear = user32.load_shared_cursor(arrow_cursor()).unwrap();
        assert_eq!(repeated_after_clear, first);
        assert_eq!(user32, before_reload);
        assert_eq!(user32.current_cursor().unwrap(), None);

        let invalid = RuntimeCursorSource::SystemPredefined {
            resource_id: 0x7f01,
            cursor: RuntimeSystemCursorKind::Arrow,
        };
        let before_invalid = user32.clone();
        user32.load_shared_cursor(invalid).unwrap_err();
        assert_eq!(user32, before_invalid);
    }

    #[test]
    fn shared_cursor_capacity_failure_preserves_identity_and_allocators() {
        let mut user32 = RuntimeUser32::default();
        for ordinal in 1..=RUNTIME_SHARED_CURSOR_LIMIT {
            let ordinal = u32::try_from(ordinal).unwrap();
            let source = module_cursor(ordinal, ordinal, 0x4000 + ordinal * 0x1000);
            let result = user32.load_shared_cursor(source).unwrap();
            assert_eq!(user32.cursor(result).unwrap().id.0, u64::from(ordinal));
        }
        let before = user32.clone();
        let _error = user32
            .load_shared_cursor(module_cursor(257, 257, 0x0010_5000))
            .expect_err("shared cursor capacity must be bounded");

        assert_eq!(user32, before);
    }

    #[test]
    fn window_class_indexes_reject_live_class_aliases() {
        let mut user32 = RuntimeUser32::default();
        let icon = user32.load_shared_icon(resource(101, 3, 0x3000)).unwrap();
        let cursor = user32.load_shared_cursor(arrow_cursor()).unwrap();
        let first = user32
            .register_window_class(window_class_registration(
                &user32,
                icon,
                cursor,
                "First fixture",
                0x28,
                0x92550,
            ))
            .unwrap();
        let second = user32
            .register_window_class(window_class_registration(
                &user32,
                icon,
                cursor,
                "Second fixture",
                0x3020,
                0x2e6e0,
            ))
            .unwrap();
        let first_id = user32.window_class(first).unwrap().id;
        let second_id = user32.window_class(second).unwrap().id;
        let key = window_class_key_from_name(RuntimeModuleId(1), "First fixture").unwrap();
        assert!(
            user32
                .window_class_by_name_option(RuntimeModuleId(1), "Missing fixture")
                .unwrap()
                .is_none()
        );
        user32
            .window_class_name_index
            .insert(key.clone(), second_id);
        let before = user32.clone();
        user32
            .window_class_by_name_option(RuntimeModuleId(1), "First fixture")
            .unwrap_err();
        user32
            .register_window_class(window_class_registration(
                &user32,
                icon,
                cursor,
                "First fixture",
                0x28,
                0x92550,
            ))
            .unwrap_err();
        assert_eq!(user32, before);
        user32.window_class_name_index.insert(key, first_id);
        user32.window_class_atom_index.insert(first, second_id);
        user32.window_class(first).unwrap_err();
        user32
            .window_class_by_name_option(RuntimeModuleId(1), "First fixture")
            .unwrap_err();
    }

    #[test]
    fn window_class_registration_is_module_scoped_case_folded_and_failure_atomic() {
        let mut user32 = RuntimeUser32::default();
        let icon = user32.load_shared_icon(resource(101, 3, 0x3000)).unwrap();
        let cursor = user32.load_shared_cursor(arrow_cursor()).unwrap();

        let main_registration =
            window_class_registration(&user32, icon, cursor, "Main fixture window", 0x28, 0x92550);
        assert!(user32.window_classes().is_empty());
        let main = user32
            .register_window_class(main_registration.clone())
            .unwrap();
        assert_eq!(
            user32.window_class(main).unwrap().id,
            RuntimeWindowClassId(1)
        );
        assert_eq!(main, RUNTIME_WINDOW_CLASS_ATOM_BASE);
        assert_eq!(
            user32.window_class(main).unwrap().registration,
            main_registration
        );
        assert_eq!(
            user32
                .window_class_by_name(RuntimeModuleId(1), "MAIN FIXTURE WINDOW")
                .unwrap()
                .id,
            user32.window_class(main).unwrap().id
        );

        let mut duplicate_registration = main_registration.clone();
        duplicate_registration.class_name = "main fixture WINDOW".to_owned();
        let generation_before_duplicate = user32.generation;
        let duplicate = user32
            .register_window_class(duplicate_registration)
            .unwrap();
        assert_eq!(duplicate, 0);
        assert_eq!(user32.generation, generation_before_duplicate);
        assert_eq!(user32.window_classes().len(), 1);

        let child = user32
            .register_window_class(window_class_registration(
                &user32,
                icon,
                cursor,
                "Child fixture window",
                0x3020,
                0x2e6e0,
            ))
            .unwrap();
        assert_eq!(
            user32.window_class(child).unwrap().id,
            RuntimeWindowClassId(2)
        );
        assert_eq!(child, RUNTIME_WINDOW_CLASS_ATOM_BASE + 1);

        let mut bad_reference =
            window_class_registration(&user32, icon, cursor, "bad reference", 0x28, 0x92550);
        bad_reference.icon.as_mut().unwrap().handle ^= 1;
        let before_bad_reference = user32.clone();
        user32.register_window_class(bad_reference).unwrap_err();
        assert_eq!(user32, before_bad_reference);
    }

    #[test]
    fn invalid_window_class_metadata_and_atom_exhaustion_do_not_consume_identity() {
        let mut user32 = RuntimeUser32::default();
        let icon = user32.load_shared_icon(resource(101, 3, 0x3000)).unwrap();
        let cursor = user32.load_shared_cursor(arrow_cursor()).unwrap();
        let valid = window_class_registration(&user32, icon, cursor, "valid class", 0x28, 0x92550);

        for invalid in [
            {
                let mut invalid = valid.clone();
                invalid.module.module_id = RuntimeModuleId(0);
                invalid
            },
            {
                let mut invalid = valid.clone();
                invalid.window_procedure.address.0 += 1;
                invalid
            },
            {
                let mut invalid = valid.clone();
                invalid.class_name = "class-\u{1f600}".to_owned();
                invalid
            },
        ] {
            let before = user32.clone();
            user32.register_window_class(invalid).unwrap_err();
            assert_eq!(user32, before);
        }

        user32.next_window_class_atom = u32::from(u16::MAX) + 1;
        let before_exhaustion = user32.clone();
        user32
            .register_window_class(valid)
            .expect_err("logical ATOM space must be bounded");
        assert_eq!(user32, before_exhaustion);
    }

    #[test]
    fn window_class_capacity_and_allocator_failure_preserve_state() {
        let mut user32 = RuntimeUser32::default();
        let icon = user32.load_shared_icon(resource(101, 3, 0x3000)).unwrap();
        let cursor = user32.load_shared_cursor(arrow_cursor()).unwrap();
        for ordinal in 0..RUNTIME_WINDOW_CLASS_LIMIT {
            let registration = window_class_registration(
                &user32,
                icon,
                cursor,
                &format!("class {ordinal}"),
                0x28,
                0x92550,
            );
            let class = user32.register_window_class(registration).unwrap();
            assert_eq!(user32.window_class(class).unwrap().id.0, ordinal as u64 + 1);
        }
        let registration =
            window_class_registration(&user32, icon, cursor, "over capacity", 0x28, 0x92550);
        let before = user32.clone();
        assert!(user32.register_window_class(registration.clone()).is_err());
        assert_eq!(user32, before);
        // Reuse does not allocate even at capacity.
        assert_eq!(
            user32
                .register_window_class(window_class_registration(
                    &user32, icon, cursor, "CLASS 0", 0x28, 0x92550,
                ))
                .unwrap(),
            0
        );
        assert_eq!(user32, before);

        let mut user32 = RuntimeUser32::default();
        let icon = user32.load_shared_icon(resource(101, 3, 0x3000)).unwrap();
        let cursor = user32.load_shared_cursor(arrow_cursor()).unwrap();
        let registration =
            window_class_registration(&user32, icon, cursor, "new class", 0x28, 0x92550);
        user32.next_window_class_id = u64::MAX;
        let before = user32.clone();
        assert!(user32.register_window_class(registration.clone()).is_err());
        assert_eq!(user32, before);
        user32.next_window_class_id = 1;
        user32.generation = u64::MAX;
        let before = user32.clone();
        assert!(user32.register_window_class(registration).is_err());
        assert_eq!(user32, before);
    }

    #[test]
    fn logical_window_creation_callbacks_are_ordered_and_abort_without_live_identity_leaks() {
        let mut user32 = RuntimeUser32::default();
        let icon = user32.load_shared_icon(resource(101, 3, 0x3000)).unwrap();
        let cursor = user32.load_shared_cursor(arrow_cursor()).unwrap();
        let class = user32
            .register_window_class(window_class_registration(
                &user32,
                icon,
                cursor,
                "Main fixture window",
                0x28,
                0x92550,
            ))
            .unwrap();

        let creation = window_creation_request(&user32, class);
        let creation_plan = user32.checked_window_creation(creation.clone()).unwrap();
        let preview = (creation_plan.id, creation_plan.handle);
        assert_eq!(preview.0, RuntimeWindowId(1));
        assert_eq!(preview.1, RUNTIME_WINDOW_HANDLE_BASE);
        assert!(
            user32.windows().is_empty(),
            "planning must not publish HWND state"
        );
        let created = user32.publish_window_creation(creation_plan);
        assert_eq!(created, preview);
        assert_eq!(
            user32.window(created.1).unwrap().lifecycle,
            RuntimeWindowLifecycle::Creating {
                message: RuntimeWindowCreationMessage::NonClientCreate,
            }
        );

        for (message, result, next) in [
            (
                RuntimeWindowCreationMessage::NonClientCreate,
                1,
                Some(RuntimeWindowCreationMessage::NonClientCalculate),
            ),
            (
                RuntimeWindowCreationMessage::NonClientCalculate,
                0,
                Some(RuntimeWindowCreationMessage::Create),
            ),
            (
                RuntimeWindowCreationMessage::Create,
                0,
                Some(RuntimeWindowCreationMessage::Size),
            ),
            (
                RuntimeWindowCreationMessage::Size,
                0,
                Some(RuntimeWindowCreationMessage::Move),
            ),
            (RuntimeWindowCreationMessage::Move, 0, None),
        ] {
            let plan = (created.0, message, result);
            let preview = user32
                .preview_window_creation_callback(plan.0, plan.1, plan.2)
                .unwrap()
                .clone();
            let action = user32
                .complete_window_creation_callback(plan.0, plan.1, plan.2)
                .unwrap();
            assert_eq!(action, preview);
            match (action, next) {
                (
                    RuntimeWindowCreationCallbackAction::Continue { next_message, .. },
                    Some(expected),
                ) => assert_eq!(next_message, expected),
                (RuntimeWindowCreationCallbackAction::Created { window }, None) => {
                    assert_eq!(window.lifecycle, RuntimeWindowLifecycle::Created)
                }
                other => panic!("unexpected creation transition {other:?}"),
            }
        }
        assert_eq!(
            user32.window(created.1).unwrap().lifecycle,
            RuntimeWindowLifecycle::Created
        );

        let rejected_plan = user32.checked_window_creation(creation.clone()).unwrap();
        let rejected = user32.publish_window_creation(rejected_plan);
        assert_eq!(rejected.0, RuntimeWindowId(2));
        assert_eq!(rejected.1, RUNTIME_WINDOW_HANDLE_BASE + 1);
        let abort_plan = (rejected.0, RuntimeWindowCreationMessage::NonClientCreate, 0);
        assert!(matches!(
            user32
                .complete_window_creation_callback(abort_plan.0, abort_plan.1, abort_plan.2)
                .unwrap(),
            RuntimeWindowCreationCallbackAction::Aborted { .. }
        ));
        user32.window(rejected.1).unwrap_err();
        assert_eq!(user32.windows().len(), 1);

        let create_rejected_plan = user32.checked_window_creation(creation).unwrap();
        let create_rejected = user32.publish_window_creation(create_rejected_plan);
        let first = (
            create_rejected.0,
            RuntimeWindowCreationMessage::NonClientCreate,
            1,
        );
        user32
            .complete_window_creation_callback(first.0, first.1, first.2)
            .unwrap();
        let second = (
            create_rejected.0,
            RuntimeWindowCreationMessage::NonClientCalculate,
            0,
        );
        user32
            .complete_window_creation_callback(second.0, second.1, second.2)
            .unwrap();
        let create_abort = (
            create_rejected.0,
            RuntimeWindowCreationMessage::Create,
            u32::MAX,
        );
        user32
            .complete_window_creation_callback(create_abort.0, create_abort.1, create_abort.2)
            .unwrap();
        user32.window(create_rejected.1).unwrap_err();
        assert_eq!(user32.windows().len(), 1);
    }

    #[test]
    fn nonclient_activation_notification_ignores_the_guest_return_value() {
        for result in [0, 1, u32::MAX] {
            let (mut user32, window_handle, _) = created_window_with_small_icon();
            let owner_thread_id = RuntimeThreadId(0x2000);
            user32
                .set_window_style(window_handle, owner_thread_id, 0x90ca_0000)
                .unwrap();
            let geometry = user32.window(window_handle).unwrap().creation.geometry;
            let mut operation = user32
                .preview_set_window_pos(
                    owner_thread_id,
                    RuntimeSetWindowPosRequest {
                        window_handle,
                        variant: RuntimeSetWindowPosVariant::ActivateSameGeometry,
                        next_geometry: geometry,
                        next_visible: true,
                        next_topmost: false,
                        entry_last_error: 0x1234_5678,
                        callback_flags: 0x010a,
                        callback_insert_after: 0xffff_fffe,
                        activation_insert_after: 0,
                        activation_internal_flags: 3,
                        resize_nccalc_window_pos_flags: 0x190e,
                        resize_changed_flags: 0x110e,
                        frame_change_nccalc_window_pos_flags: 0x1825,
                        frame_change_changed_flags: 0x0825,
                    },
                )
                .unwrap();
            for message in [
                RuntimeSetWindowPosCallbackMessage::WindowPositionChanging,
                RuntimeSetWindowPosCallbackMessage::ActivationWindowPositionChanging,
                RuntimeSetWindowPosCallbackMessage::ActivateApplication,
            ] {
                let RuntimeSetWindowPosCallbackAction::Continue {
                    operation: next, ..
                } = user32
                    .complete_set_window_pos_callback(operation, message, 0)
                    .unwrap()
                else {
                    panic!("activation must continue to the nonclient notification");
                };
                operation = next;
            }
            let before = user32.clone();
            assert!(matches!(
                user32
                    .complete_set_window_pos_callback(
                        operation,
                        RuntimeSetWindowPosCallbackMessage::NonClientActivate,
                        result,
                    )
                    .unwrap(),
                RuntimeSetWindowPosCallbackAction::Continue {
                    next_message: RuntimeSetWindowPosCallbackMessage::Activate,
                    ..
                }
            ));
            assert_eq!(user32, before);
        }
    }

    #[test]
    fn small_icon_updates_directly_and_window_position_callbacks_validate_live_operations() {
        let (mut user32, window_handle, small_icon_handle) = created_window_with_small_icon();
        let initial = user32.window(window_handle).unwrap().clone();
        let before_repeated = user32.clone();
        assert_eq!(
            user32
                .set_window_small_icon(window_handle, RuntimeThreadId(0x2000), small_icon_handle)
                .unwrap(),
            small_icon_handle
        );
        assert_eq!(user32, before_repeated);

        let show_geometry = RuntimeWindowGeometry {
            x: 15_360,
            y: 8_640,
            width: 806,
            height: 629,
            client_x: 15_363,
            client_y: 8_666,
            client_width: 800,
            client_height: 600,
        };
        let show = user32
            .preview_set_window_pos(
                RuntimeThreadId(0x2000),
                RuntimeSetWindowPosRequest {
                    window_handle,
                    variant: RuntimeSetWindowPosVariant::InitialShow,
                    next_geometry: show_geometry,
                    next_visible: true,
                    next_topmost: false,
                    entry_last_error: 0x1234_5678,
                    callback_flags: 0x1000_0d59,
                    callback_insert_after: 0xffff_fffe,
                    activation_insert_after: 0,
                    activation_internal_flags: 3,
                    resize_nccalc_window_pos_flags: 0x190e,
                    resize_changed_flags: 0x110e,
                    frame_change_nccalc_window_pos_flags: 0x1825,
                    frame_change_changed_flags: 0x0825,
                },
            )
            .unwrap();
        let first_callback = (
            show,
            RuntimeSetWindowPosCallbackMessage::WindowPositionChanged,
            0,
        );
        assert!(matches!(
            user32
                .preview_set_window_pos_callback(
                    first_callback.0,
                    first_callback.1,
                    first_callback.2
                )
                .unwrap(),
            RuntimeSetWindowPosCallbackAction::Continue {
                next_message: RuntimeSetWindowPosCallbackMessage::GetSmallIcon,
                ..
            }
        ));
        let before_first_callback = user32.clone();
        user32
            .complete_set_window_pos_callback(first_callback.0, first_callback.1, first_callback.2)
            .unwrap();
        assert_eq!(
            user32, before_first_callback,
            "the first synchronous callback must not publish geometry or visibility"
        );
        let final_callback = (
            show,
            RuntimeSetWindowPosCallbackMessage::GetSmallIcon,
            small_icon_handle,
        );
        assert_eq!(user32.window(window_handle).unwrap(), &initial);
        let shown = user32
            .complete_set_window_pos_callback(final_callback.0, final_callback.1, final_callback.2)
            .unwrap();
        let RuntimeSetWindowPosCallbackAction::Completed { window: shown } = shown else {
            panic!("show must complete after the icon relation is validated");
        };
        assert_eq!(shown.creation.geometry, show_geometry);
        assert!(shown.visible);
        assert_eq!(shown.creation.effective_style, 0x94ca_0000);
        assert_eq!(user32.window(window_handle).unwrap(), &shown);

        let hide = user32
            .preview_set_window_pos(
                RuntimeThreadId(0x2000),
                RuntimeSetWindowPosRequest {
                    window_handle,
                    variant: RuntimeSetWindowPosVariant::InitialHide,
                    next_geometry: show_geometry,
                    next_visible: false,
                    next_topmost: false,
                    entry_last_error: 0x1234_5678,
                    callback_flags: 0x0000_1d9f,
                    callback_insert_after: 0xffff_fffe,
                    activation_insert_after: 0,
                    activation_internal_flags: 3,
                    resize_nccalc_window_pos_flags: 0x190e,
                    resize_changed_flags: 0x110e,
                    frame_change_nccalc_window_pos_flags: 0x1825,
                    frame_change_changed_flags: 0x0825,
                },
            )
            .unwrap();
        let stale_hide = (
            hide,
            RuntimeSetWindowPosCallbackMessage::WindowPositionChanged,
            0,
        );
        let mut intervening_resource = resource(102, 5, 0x5000);
        intervening_resource.width = 16;
        intervening_resource.height = 16;
        intervening_resource.icon_data_size = 1_128;
        user32.load_shared_icon(intervening_resource).unwrap();
        let before_stale = user32.clone();
        user32
            .complete_set_window_pos_callback(stale_hide.0, stale_hide.1, stale_hide.2)
            .unwrap_err();
        assert_eq!(user32, before_stale);
        assert!(user32.window(window_handle).unwrap().visible);

        let fresh_hide = user32
            .preview_set_window_pos(
                RuntimeThreadId(0x2000),
                RuntimeSetWindowPosRequest {
                    window_handle,
                    variant: RuntimeSetWindowPosVariant::InitialHide,
                    next_geometry: show_geometry,
                    next_visible: false,
                    next_topmost: false,
                    entry_last_error: 0x1234_5678,
                    callback_flags: 0x0000_1d9f,
                    callback_insert_after: 0xffff_fffe,
                    activation_insert_after: 0,
                    activation_internal_flags: 3,
                    resize_nccalc_window_pos_flags: 0x190e,
                    resize_changed_flags: 0x110e,
                    frame_change_nccalc_window_pos_flags: 0x1825,
                    frame_change_changed_flags: 0x0825,
                },
            )
            .unwrap();
        let hide_plan = (
            fresh_hide,
            RuntimeSetWindowPosCallbackMessage::WindowPositionChanged,
            0,
        );
        let duplicate = hide_plan;
        let hidden = user32
            .complete_set_window_pos_callback(hide_plan.0, hide_plan.1, hide_plan.2)
            .unwrap();
        assert!(matches!(
            hidden,
            RuntimeSetWindowPosCallbackAction::Completed { ref window }
                if !window.visible
                    && window.creation.geometry == show_geometry
                    && window.creation.effective_style == 0x84ca_0000
        ));
        let before_duplicate = user32.clone();
        user32
            .complete_set_window_pos_callback(duplicate.0, duplicate.1, duplicate.2)
            .unwrap_err();
        assert_eq!(user32, before_duplicate);

        let style = user32
            .set_window_style(window_handle, RuntimeThreadId(0x2000), 0x90ca_0000)
            .unwrap();
        assert_eq!(style, 0x84ca_0000);
        let updated = user32.window(window_handle).unwrap();
        assert_eq!(updated.creation.effective_style, 0x90ca_0000);
        assert!(updated.visible);
        let before_repeated = user32.clone();
        assert_eq!(
            user32
                .set_window_style(window_handle, RuntimeThreadId(0x2000), 0x90ca_0000)
                .unwrap(),
            0x90ca_0000
        );
        assert_eq!(user32, before_repeated);

        let owner_thread_id = RuntimeThreadId(0x2000);
        let window_id = user32.window(window_handle).unwrap().id;
        user32.active_windows.insert(owner_thread_id, window_id);
        user32.focused_windows.insert(owner_thread_id, window_id);
        user32.foreground_window = Some(window_id);
        user32.generation += 1;
        let repeat_request = RuntimeSetWindowPosRequest {
            window_handle,
            variant: RuntimeSetWindowPosVariant::ActivateSameGeometry,
            next_geometry: show_geometry,
            next_visible: true,
            next_topmost: false,
            entry_last_error: 0x1234_5678,
            callback_flags: 0x0000_010a,
            callback_insert_after: 0xffff_fffe,
            activation_insert_after: 0,
            activation_internal_flags: 3,
            resize_nccalc_window_pos_flags: 0x190e,
            resize_changed_flags: 0x110e,
            frame_change_nccalc_window_pos_flags: 0x1825,
            frame_change_changed_flags: 0x0825,
        };
        let repeat = user32
            .preview_set_window_pos(owner_thread_id, repeat_request)
            .unwrap();
        assert_eq!(repeat.variant, RuntimeSetWindowPosVariant::RepeatActivation);
        let before_wrong_result = user32.clone();
        user32
            .preview_set_window_pos_callback(
                repeat,
                RuntimeSetWindowPosCallbackMessage::WindowPositionChanging,
                1,
            )
            .unwrap_err();
        assert_eq!(user32, before_wrong_result);
        let repeat_plan = (
            repeat,
            RuntimeSetWindowPosCallbackMessage::WindowPositionChanging,
            0,
        );
        assert!(matches!(
            user32.preview_set_window_pos_callback(repeat_plan.0, repeat_plan.1, repeat_plan.2).unwrap(),
            RuntimeSetWindowPosCallbackAction::Completed { window }
                if &window == user32.window(window_handle).unwrap()
        ));
        let before_repeat_commit = user32.clone();
        assert!(matches!(
            user32
                .complete_set_window_pos_callback(repeat_plan.0, repeat_plan.1, repeat_plan.2)
                .unwrap(),
            RuntimeSetWindowPosCallbackAction::Completed { .. }
        ));
        assert_eq!(user32, before_repeat_commit);

        let move_geometry = RuntimeWindowGeometry {
            x: 317,
            y: 154,
            client_x: 320,
            client_y: 180,
            ..show_geometry
        };
        let move_operation = user32
            .preview_set_window_pos(
                owner_thread_id,
                RuntimeSetWindowPosRequest {
                    window_handle,
                    variant: RuntimeSetWindowPosVariant::FrameChangeMove,
                    next_geometry: move_geometry,
                    next_visible: true,
                    next_topmost: false,
                    entry_last_error: 0x1234_5678,
                    callback_flags: 0x0025,
                    callback_insert_after: 0,
                    activation_insert_after: 0,
                    activation_internal_flags: 3,
                    resize_nccalc_window_pos_flags: 0x190e,
                    resize_changed_flags: 0x110e,
                    frame_change_nccalc_window_pos_flags: 0x1825,
                    frame_change_changed_flags: 0x0825,
                },
            )
            .unwrap();
        assert_eq!(
            move_operation.variant,
            RuntimeSetWindowPosVariant::FrameChangeMove
        );
        let before_wrong_move_result = user32.clone();
        user32
            .preview_set_window_pos_callback(
                move_operation,
                RuntimeSetWindowPosCallbackMessage::WindowPositionChanging,
                1,
            )
            .unwrap_err();
        assert_eq!(user32, before_wrong_move_result);

        let changing = (
            move_operation,
            RuntimeSetWindowPosCallbackMessage::WindowPositionChanging,
            0,
        );
        let before_changing = user32.clone();
        assert!(matches!(
            user32
                .complete_set_window_pos_callback(changing.0, changing.1, changing.2)
                .unwrap(),
            RuntimeSetWindowPosCallbackAction::Continue {
                next_message: RuntimeSetWindowPosCallbackMessage::NonClientCalculate,
                ..
            }
        ));
        assert_eq!(user32, before_changing);

        let nccalc = (
            move_operation,
            RuntimeSetWindowPosCallbackMessage::NonClientCalculate,
            0,
        );
        let stale_nccalc = nccalc;
        let moved = user32
            .complete_set_window_pos_callback(nccalc.0, nccalc.1, nccalc.2)
            .unwrap();
        let RuntimeSetWindowPosCallbackAction::Continue {
            window: moved_window,
            operation: moved_operation,
            next_message: RuntimeSetWindowPosCallbackMessage::WindowPositionChanged,
        } = moved
        else {
            panic!("frame-change NCCALCSIZE must commit geometry before changed notification");
        };
        assert_eq!(moved_window.creation.geometry, move_geometry);
        assert_eq!(user32.window(window_handle).unwrap(), &moved_window);
        let before_stale_nccalc = user32.clone();
        user32
            .complete_set_window_pos_callback(stale_nccalc.0, stale_nccalc.1, stale_nccalc.2)
            .unwrap_err();
        assert_eq!(user32, before_stale_nccalc);

        let changed = (
            moved_operation,
            RuntimeSetWindowPosCallbackMessage::WindowPositionChanged,
            0,
        );
        let before_changed = user32.clone();
        assert!(matches!(
            user32.complete_set_window_pos_callback(changed.0, changed.1, changed.2).unwrap(),
            RuntimeSetWindowPosCallbackAction::Completed { window }
                if window == moved_window
        ));
        assert_eq!(user32, before_changed);
        let full_client_update_rect = RuntimeRect {
            left: 0,
            top: 0,
            right: 800,
            bottom: 600,
        };
        assert_eq!(
            moved_window.pending_update_rect,
            Some(full_client_update_rect)
        );
        assert_eq!(
            user32.active_windows.get(&owner_thread_id),
            Some(&moved_window.id)
        );
        assert_eq!(
            user32.focused_windows.get(&owner_thread_id),
            Some(&moved_window.id)
        );
        assert_eq!(user32.foreground_window, Some(moved_window.id));

        let update = user32
            .preview_update_window(owner_thread_id, window_handle, 0x1234_5678)
            .unwrap();
        assert_eq!(update.update_rect, full_client_update_rect);
        user32.validate_update_window_callback(update).unwrap_err();
        let paint_struct_address = GuestAddress(0x2000);
        let mut exhausted = user32.clone();
        exhausted.generation = u64::MAX;
        let before_exhausted = exhausted.clone();
        assert!(
            exhausted
                .validate_begin_paint(
                    owner_thread_id,
                    window_handle,
                    paint_struct_address,
                    RuntimeDeviceContextId(1),
                    0x4701_0001
                )
                .is_err()
        );
        assert_eq!(exhausted, before_exhausted);
        let mut exhausted = user32.clone();
        exhausted
            .windows
            .get_mut(&window_id)
            .unwrap()
            .paint_generation = u64::MAX;
        let before_exhausted = exhausted.clone();
        assert!(
            exhausted
                .validate_begin_paint(
                    owner_thread_id,
                    window_handle,
                    paint_struct_address,
                    RuntimeDeviceContextId(1),
                    0x4701_0001
                )
                .is_err()
        );
        assert_eq!(exhausted, before_exhausted);
        let begin = user32
            .validate_begin_paint(
                owner_thread_id,
                window_handle,
                paint_struct_address,
                RuntimeDeviceContextId(1),
                0x4701_0001,
            )
            .unwrap();
        let window_id = begin.window_id;
        user32.finish_begin_paint(begin);
        let session = user32.paint_sessions[&window_id].clone();
        assert_eq!(session.update_rect, full_client_update_rect);
        assert_eq!(session.paint_generation, 1);
        assert_eq!(session.paint_struct_bytes.len(), 64);
        assert_eq!(
            user32.window(window_handle).unwrap().pending_update_rect,
            None,
            "BeginPaint must consume the update region"
        );
        let before_duplicate = user32.clone();
        assert!(
            user32
                .validate_begin_paint(
                    owner_thread_id,
                    window_handle,
                    paint_struct_address,
                    RuntimeDeviceContextId(2),
                    0x4701_0002
                )
                .is_err()
        );
        assert_eq!(user32, before_duplicate);
        let mut corrupted_bytes = session.paint_struct_bytes.clone();
        corrupted_bytes[4] = 0;
        let before_wrong_end = user32.clone();
        user32
            .validate_end_paint(
                owner_thread_id,
                window_handle,
                session.paint_struct_address,
                &corrupted_bytes,
            )
            .unwrap_err();
        assert_eq!(user32, before_wrong_end);
        let mut exhausted = user32.clone();
        exhausted.generation = u64::MAX;
        let before_exhausted = exhausted.clone();
        assert!(
            exhausted
                .validate_end_paint(
                    owner_thread_id,
                    window_handle,
                    session.paint_struct_address,
                    &session.paint_struct_bytes
                )
                .is_err()
        );
        assert_eq!(exhausted, before_exhausted);
        let end = user32
            .validate_end_paint(
                owner_thread_id,
                window_handle,
                session.paint_struct_address,
                &session.paint_struct_bytes,
            )
            .unwrap();
        assert_eq!(end, &session);
        user32.finish_end_paint(end.window_id, end.paint_generation);
        assert!(user32.paint_sessions.is_empty());
        assert_eq!(user32.window(window_handle).unwrap().paint_generation, 1);
        let before_duplicate = user32.clone();
        assert!(
            user32
                .validate_end_paint(
                    owner_thread_id,
                    window_handle,
                    session.paint_struct_address,
                    &session.paint_struct_bytes
                )
                .is_err()
        );
        assert_eq!(user32, before_duplicate);

        let title = RuntimeWindowNameA {
            value: "Paint callback title".to_owned(),
        };
        user32
            .set_window_title(window_handle, owner_thread_id, title.clone())
            .unwrap();
        user32
            .validate_update_window_callback(update)
            .expect("unrelated title update must not invalidate a completed paint cycle");
        let painted = user32.window(window_handle).unwrap();
        assert_eq!(painted.creation.name, title);
        assert!(painted.pending_update_rect.is_none());
        assert_eq!(painted.paint_generation, 1);
        user32
            .preview_update_window(owner_thread_id, window_handle, 0)
            .unwrap_err();

        let before_unknown = user32.clone();
        user32
            .set_window_style(window_handle + 1, RuntimeThreadId(0x2000), 0x90ca_0000)
            .unwrap_err();
        assert_eq!(user32, before_unknown);
    }

    #[test]
    fn window_title_copies_windows_932_and_rejects_invalid_changes_atomically() {
        let (mut user32, window_handle, _) = created_window_with_small_icon();
        let title = RuntimeWindowNameA {
            value: "日本語".to_owned(),
        };
        assert_eq!(
            title.windows_932_bytes().unwrap(),
            [0x93, 0xfa, 0x96, 0x7b, 0x8c, 0xea]
        );
        user32
            .set_window_title(window_handle, RuntimeThreadId(0x2000), title.clone())
            .unwrap();
        let updated = user32.window(window_handle).unwrap().clone();
        assert_eq!(updated.creation.name, title);
        user32
            .set_window_title(window_handle, RuntimeThreadId(0x2000), title.clone())
            .unwrap();
        assert_eq!(user32.window(window_handle).unwrap(), &updated);

        for (owner, value) in [
            (RuntimeThreadId(0x2001), "other"),
            (RuntimeThreadId(0x2000), "title-😀"),
        ] {
            user32
                .set_window_title(
                    window_handle,
                    owner,
                    RuntimeWindowNameA {
                        value: value.to_owned(),
                    },
                )
                .expect_err("foreign owner or unencodable title must be rejected");
            assert_eq!(user32.window(window_handle).unwrap(), &updated);
            assert_eq!(user32.windows().len(), 1);
        }
        user32
            .set_window_title(
                window_handle,
                RuntimeThreadId(0x2000),
                RuntimeWindowNameA {
                    value: String::new(),
                },
            )
            .unwrap();
        assert!(
            user32
                .window(window_handle)
                .unwrap()
                .creation
                .name
                .value
                .is_empty()
        );
    }

    #[test]
    fn file_drop_acceptance_uses_effective_extended_style_and_is_failure_atomic() {
        let (mut user32, window_handle, _) = created_window_with_small_icon();
        assert_eq!(
            user32
                .window(window_handle)
                .unwrap()
                .creation
                .effective_extended_style
                & RUNTIME_WINDOW_EXTENDED_STYLE_ACCEPT_FILES,
            0
        );

        user32
            .set_window_file_drop_acceptance(window_handle, true)
            .unwrap();
        assert_ne!(
            user32
                .window(window_handle)
                .unwrap()
                .creation
                .effective_extended_style
                & RUNTIME_WINDOW_EXTENDED_STYLE_ACCEPT_FILES,
            0
        );
        let before_repeated = user32.clone();
        user32
            .set_window_file_drop_acceptance(window_handle, true)
            .unwrap();
        assert_eq!(user32, before_repeated);

        user32
            .set_window_file_drop_acceptance(window_handle, false)
            .unwrap();
        assert_eq!(
            user32
                .window(window_handle)
                .unwrap()
                .creation
                .effective_extended_style
                & RUNTIME_WINDOW_EXTENDED_STYLE_ACCEPT_FILES,
            0
        );

        let before_unknown = user32.clone();
        user32
            .set_window_file_drop_acceptance(window_handle + 2, true)
            .unwrap_err();
        assert_eq!(user32, before_unknown);
    }

    #[test]
    fn system_menu_creation_checks_all_allocators_before_publication() {
        for exhausted in 0..2 {
            let (mut user32, window_handle, _) = created_window_with_small_icon();
            let next_id = user32.next_menu_id;
            let next_handle = user32.next_menu_handle;
            match exhausted {
                0 => user32.next_menu_id = u64::MAX,
                _ => user32.next_menu_handle = u32::MAX,
            }
            let before = user32.clone();
            assert!(user32.get_system_menu(window_handle, 0xf060).is_err());
            assert_eq!(user32, before);
            assert!(user32.system_menus.is_empty());
            assert!(user32.system_menu_window_index.is_empty());
            assert!(user32.menu_handle_index.is_empty());
            user32.next_menu_id = next_id;
            user32.next_menu_handle = next_handle;
            let handle = user32
                .get_system_menu(window_handle, 0xf060)
                .unwrap()
                .unwrap();
            assert_eq!(handle, next_handle);
            let menu = user32.system_menu(handle).unwrap();
            assert_eq!(menu.id, RuntimeMenuId(next_id));
            assert_eq!(menu.owner_window_handle, window_handle);
            assert_eq!(user32.system_menus.len(), 1);
            assert_eq!(user32.system_menu_window_index.len(), 1);
            assert_eq!(user32.menu_handle_index.len(), 1);
            assert_eq!(
                user32.get_system_menu(window_handle, 0xf060).unwrap(),
                Some(handle)
            );
        }
    }

    #[test]
    fn window_style_changes_validate_owner_and_capacity_before_mutation() {
        let (mut user32, window_handle, _) = created_window_with_small_icon();
        let before = user32.clone();
        assert!(
            user32
                .set_window_style(window_handle, RuntimeThreadId(0x9999), 0x90ca_0000)
                .is_err()
        );
        assert_eq!(user32, before);
        user32.generation = u64::MAX;
        let exhausted = user32.clone();
        assert!(
            user32
                .set_window_style(window_handle, RuntimeThreadId(0x2000), 0x90ca_0000)
                .is_err()
        );
        assert_eq!(user32, exhausted);
        let unchanged_style = user32
            .window(window_handle)
            .unwrap()
            .creation
            .effective_style;
        assert_eq!(
            user32
                .set_window_style(window_handle, RuntimeThreadId(0x2000), unchanged_style)
                .unwrap(),
            unchanged_style
        );
        assert_eq!(user32, exhausted);
    }

    #[test]
    fn system_menu_lookup_rejects_forged_handle_alias() {
        let (mut user32, window_handle, _) = created_window_with_small_icon();
        let handle = user32
            .get_system_menu(window_handle, 0xf060)
            .unwrap()
            .unwrap();
        let alias = handle + 1;
        assert!(user32.system_menu_option(alias).unwrap().is_none());
        let before_unknown = user32.clone();
        assert_eq!(
            user32.enable_system_menu_close_item(alias, 0xf060).unwrap(),
            None
        );
        assert_eq!(
            user32.get_system_menu(window_handle + 1, 0xf060).unwrap(),
            None
        );
        assert_eq!(user32, before_unknown);
        user32.menu_handle_index.insert(alias, RuntimeMenuId(1));
        let before = user32.clone();
        user32.system_menu_option(alias).unwrap_err();
        user32.system_menu(alias).unwrap_err();
        user32
            .enable_system_menu_close_item(alias, 0xf060)
            .unwrap_err();
        assert_eq!(user32, before);
        assert!(user32.system_menu_option(handle).unwrap().is_some());
    }

    #[test]
    fn system_menu_identity_is_window_owned_reentrant_and_failure_atomic() {
        let (mut user32, window_handle, _) = created_window_with_small_icon();
        let user32_generation = user32.generation;
        let first = user32
            .get_system_menu(window_handle, 0xf060)
            .unwrap()
            .unwrap();
        assert_eq!(first, RUNTIME_MENU_HANDLE_BASE);
        assert_eq!(user32.generation, user32_generation);

        let menu = user32.system_menu(first).unwrap();
        assert_eq!(menu.id, RuntimeMenuId(1));
        assert_eq!(menu.owner_window_handle, window_handle);
        assert_eq!(menu.close_item_command, 0xf060);
        assert!(menu.close_item_enabled);

        let style = user32
            .set_window_style(window_handle, RuntimeThreadId(0x2000), 0x90ca_0000)
            .expect("window style update preserves the live system menu");
        assert_ne!(style, 0x90ca_0000);
        assert_eq!(
            user32
                .window(window_handle)
                .unwrap()
                .creation
                .effective_style,
            0x90ca_0000
        );
        let repeated = user32
            .get_system_menu(window_handle, 0xf060)
            .unwrap()
            .unwrap();
        assert_eq!(repeated, first);
        assert_eq!(user32.system_menus().len(), 1);

        let enabled = user32
            .enable_system_menu_close_item(first, 0xf060)
            .unwrap()
            .unwrap();
        assert_eq!(enabled, 0);

        let before_invalid_item = user32.clone();
        assert!(user32.enable_system_menu_close_item(first, 0xf061).is_err());
        assert_eq!(
            user32
                .enable_system_menu_close_item(first + 1, 0xf060)
                .unwrap(),
            None
        );
        assert_eq!(user32, before_invalid_item);
        let before_exhaustion = user32.clone();
        user32.next_menu_handle = u32::MAX;
        let exhausted = user32.clone();
        let other_window = user32.publish_window_creation(
            user32
                .checked_window_creation(user32.window(window_handle).unwrap().creation.clone())
                .unwrap(),
        );
        for (message, result) in [
            (RuntimeWindowCreationMessage::NonClientCreate, 1),
            (RuntimeWindowCreationMessage::NonClientCalculate, 0),
            (RuntimeWindowCreationMessage::Create, 0),
            (RuntimeWindowCreationMessage::Size, 0),
            (RuntimeWindowCreationMessage::Move, 0),
        ] {
            let plan = (other_window.0, message, result);
            user32
                .complete_window_creation_callback(plan.0, plan.1, plan.2)
                .unwrap();
        }
        user32.get_system_menu(other_window.1, 0xf060).unwrap_err();
        assert_eq!(user32.next_menu_handle, u32::MAX);
        assert_eq!(user32.system_menus(), exhausted.system_menus());
        user32.next_menu_handle = before_exhaustion.next_menu_handle;
    }

    fn resource(
        group_resource_id: u32,
        icon_resource_id: u32,
        icon_data_rva: u32,
    ) -> RuntimeIconResource {
        RuntimeIconResource {
            module_id: RuntimeModuleId(1),
            module_load_base: GuestAddress(0x1000_0000),
            group_resource_id,
            language_id: 1_041,
            icon_resource_id,
            group_data_rva: Rva(0x2000),
            group_data_size: 48,
            icon_data_rva: Rva(icon_data_rva),
            icon_data_size: 4_264,
            width: 32,
            height: 32,
            planes: 1,
            bits_per_pixel: 32,
        }
    }

    fn created_window_with_small_icon() -> (RuntimeUser32, u32, u32) {
        let mut user32 = RuntimeUser32::default();
        let large_icon = user32.load_shared_icon(resource(101, 3, 0x3000)).unwrap();
        let mut small_resource = resource(101, 4, 0x4000);
        small_resource.width = 16;
        small_resource.height = 16;
        small_resource.icon_data_size = 1_128;
        let small_icon = user32.load_shared_icon(small_resource).unwrap();
        let cursor = user32.load_shared_cursor(arrow_cursor()).unwrap();
        let class = user32
            .register_window_class(window_class_registration(
                &user32,
                large_icon,
                cursor,
                "Main fixture window",
                0x28,
                0x92550,
            ))
            .unwrap();
        let created = user32.publish_window_creation(
            user32
                .checked_window_creation(window_creation_request(&user32, class))
                .unwrap(),
        );
        for (message, result) in [
            (RuntimeWindowCreationMessage::NonClientCreate, 1),
            (RuntimeWindowCreationMessage::NonClientCalculate, 0),
            (RuntimeWindowCreationMessage::Create, 0),
            (RuntimeWindowCreationMessage::Size, 0),
            (RuntimeWindowCreationMessage::Move, 0),
        ] {
            let plan = (created.0, message, result);
            user32
                .complete_window_creation_callback(plan.0, plan.1, plan.2)
                .unwrap();
        }
        assert_eq!(
            user32
                .set_window_small_icon(created.1, RuntimeThreadId(0x2000), small_icon)
                .unwrap(),
            0
        );
        (user32, created.1, small_icon)
    }

    #[test]
    fn iconic_state_queries_use_effective_style_without_mutating_window_authority() {
        let (mut user32, window_handle, _) = created_window_with_small_icon();
        let before_normal = user32.clone();
        assert_eq!(user32.window_is_iconic(window_handle).unwrap(), Some(false));
        assert_eq!(user32, before_normal);

        let window_id = user32.window_handle_index[&window_handle];
        user32
            .windows
            .get_mut(&window_id)
            .unwrap()
            .creation
            .effective_style |= RUNTIME_WINDOW_STYLE_MINIMIZED;
        let before_minimized = user32.clone();
        assert_eq!(user32.window_is_iconic(window_handle).unwrap(), Some(true));
        assert_eq!(user32, before_minimized);

        user32.windows.get_mut(&window_id).unwrap().lifecycle = RuntimeWindowLifecycle::Creating {
            message: RuntimeWindowCreationMessage::Move,
        };
        let before_creating = user32.clone();
        user32.window_is_iconic(window_handle).unwrap_err();
        assert_eq!(user32, before_creating);

        assert_eq!(user32.window_is_iconic(0x1234).unwrap(), None);
        assert_eq!(user32, before_creating);
    }

    #[test]
    fn default_ime_window_rejects_exhaustion_without_publishing_a_hwnd() {
        for exhausted in 0..3 {
            let (mut user32, handle, _) = created_window_with_small_icon();
            let owner = user32.window(handle).unwrap().creation.owner_thread_id;
            match exhausted {
                0 => user32.next_window_handle = u32::MAX,
                1 => user32.next_ime_window_id = u64::MAX,
                _ => user32.generation = u64::MAX,
            }
            let before = user32.clone();
            assert!(user32.validate_default_ime_window(handle, owner).is_err());
            assert_eq!(user32, before);
            assert!(user32.ime_windows.is_empty());
            assert!(user32.default_ime_windows.is_empty());
        }
    }

    #[test]
    fn default_ime_window_uses_shared_hwnd_authority_and_tracks_status_requests() {
        let (mut user32, app_window_handle, _) = created_window_with_small_icon();
        let owner_thread_id = user32
            .window(app_window_handle)
            .unwrap()
            .creation
            .owner_thread_id;
        user32
            .validate_default_ime_window(app_window_handle, owner_thread_id)
            .unwrap();
        let handle = user32.ensure_default_ime_window(owner_thread_id);
        assert_eq!(handle, app_window_handle + 1);
        let ime = user32.ime_window(handle).unwrap();
        assert_eq!(ime.class_name, "IME");
        assert_eq!(ime.owner_thread_id, owner_thread_id);
        assert_eq!(ime.status_window_request, None);

        user32
            .validate_default_ime_window(app_window_handle, owner_thread_id)
            .unwrap();
        let before_repeated = user32.clone();
        let repeated = user32.ensure_default_ime_window(owner_thread_id);
        assert_eq!(repeated, handle);
        assert_eq!(user32, before_repeated);

        user32
            .control_ime_status_window(handle, RuntimeImeStatusWindowRequest::Close)
            .unwrap();
        assert_eq!(
            user32.ime_window(handle).unwrap().status_window_request,
            Some(RuntimeImeStatusWindowRequest::Close)
        );
        let before_repeated_close = user32.clone();
        user32
            .control_ime_status_window(handle, RuntimeImeStatusWindowRequest::Close)
            .unwrap();
        assert_eq!(user32, before_repeated_close);
        user32
            .control_ime_status_window(handle, RuntimeImeStatusWindowRequest::Open)
            .unwrap();
        assert_eq!(
            user32.ime_window(handle).unwrap().status_window_request,
            Some(RuntimeImeStatusWindowRequest::Open)
        );
    }

    fn arrow_cursor() -> RuntimeCursorSource {
        RuntimeCursorSource::SystemPredefined {
            resource_id: RUNTIME_SYSTEM_CURSOR_ARROW_RESOURCE_ID,
            cursor: RuntimeSystemCursorKind::Arrow,
        }
    }

    fn module_cursor(
        group_resource_id: u32,
        cursor_resource_id: u32,
        cursor_data_rva: u32,
    ) -> RuntimeCursorSource {
        RuntimeCursorSource::ModuleResource {
            resource: RuntimeCursorResource {
                module_id: RuntimeModuleId(1),
                module_load_base: GuestAddress(0x1000_0000),
                group_resource_id,
                language_id: 1_041,
                cursor_resource_id,
                group_data_rva: Rva(0x2000 + group_resource_id * 0x10),
                group_data_size: 20,
                cursor_data_rva: Rva(cursor_data_rva),
                cursor_data_size: 2_220,
                width: 32,
                height: 32,
                planes: 1,
                bits_per_pixel: 8,
                hotspot_x: 0,
                hotspot_y: 0,
            },
        }
    }

    fn window_class_registration(
        user32: &RuntimeUser32,
        icon: u32,
        cursor: u32,
        name: &str,
        style: u32,
        window_procedure_rva: u32,
    ) -> RuntimeWindowClassRegistration {
        RuntimeWindowClassRegistration {
            module: RuntimeWindowClassModule {
                module_id: RuntimeModuleId(1),
                load_base: GuestAddress(0x1000_0000),
            },
            style,
            window_procedure: RuntimeWindowProcedure {
                module_id: RuntimeModuleId(1),
                address: GuestAddress(0x1000_0000 + window_procedure_rva),
                rva: Rva(window_procedure_rva),
            },
            class_extra_bytes: 0,
            window_extra_bytes: 0,
            icon: Some(RuntimeWindowClassIconReference {
                icon_id: user32.icon(icon).unwrap().id,
                handle: icon,
            }),
            cursor: Some(RuntimeWindowClassCursorReference {
                cursor_id: user32.cursor(cursor).unwrap().id,
                handle: cursor,
            }),
            background_brush: 0,
            menu_name: None,
            class_name: name.to_owned(),
            small_icon: None,
        }
    }

    fn window_creation_request(user32: &RuntimeUser32, atom: u16) -> RuntimeWindowCreationRequest {
        let class = user32.window_class(atom).unwrap();
        RuntimeWindowCreationRequest {
            owner_thread_id: RuntimeThreadId(0x2000),
            class: RuntimeWindowClassReference {
                class_id: class.id,
                atom: class.atom,
                module_id: RuntimeModuleId(1),
            },
            instance: RuntimeWindowClassModule {
                module_id: RuntimeModuleId(1),
                load_base: GuestAddress(0x1000_0000),
            },
            name: RuntimeWindowNameA {
                value: "Ethornell - BURIKO General Interpreter".to_owned(),
            },
            extended_style: 0x0004_0000,
            effective_extended_style: 0x0004_0100,
            style: 0x80ca_0000,
            effective_style: 0x84ca_0000,
            geometry: RuntimeWindowGeometry {
                x: 557,
                y: 214,
                width: 806,
                height: 629,
                client_x: 560,
                client_y: 240,
                client_width: 800,
                client_height: 600,
            },
            parent: 0,
            menu: 0,
            create_parameter: 0,
            entry_last_error: 7,
        }
    }
}

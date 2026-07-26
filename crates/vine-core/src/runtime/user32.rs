use std::collections::BTreeMap;

use serde::Serialize;

use crate::memory::GuestAddress;
use crate::module_graph::RuntimeModuleId;
use crate::pe32::Rva;

use super::{RuntimeStateError, RuntimeThreadId};

pub const LOGICAL_ICON_HANDLE_NAMESPACE: &str = "vine.logical-icon.v1";
pub const LOGICAL_CURSOR_HANDLE_NAMESPACE: &str = "vine.logical-cursor.v1";
pub const LOGICAL_WINDOW_CLASS_ATOM_NAMESPACE: &str = "vine.logical-window-class-atom.v1";
pub const LOGICAL_WINDOW_HANDLE_NAMESPACE: &str = "vine.logical-window.v1";
pub const RUNTIME_SHARED_ICON_LIMIT: usize = 4_096;
pub const RUNTIME_SHARED_CURSOR_LIMIT: usize = 256;
pub const RUNTIME_WINDOW_CLASS_LIMIT: usize = 4_096;
pub const RUNTIME_WINDOW_CLASS_NAME_A_LIMIT: usize = 256;
pub const RUNTIME_WINDOW_CLASS_NAME_A_ENCODING: &str = "windows-932-v1";
pub const RUNTIME_WINDOW_LIMIT: usize = 4_096;
pub const RUNTIME_DEFAULT_IME_WINDOW_LIMIT: usize = 4_096;
pub const RUNTIME_WINDOW_NAME_A_LIMIT: usize = 512;
pub const RUNTIME_WINDOW_NAME_A_ENCODING: &str = "windows-932-v1";
pub const RUNTIME_ICON_HANDLE_BASE: u32 = 0x4901_0001;
pub const RUNTIME_CURSOR_HANDLE_BASE: u32 = 0x4301_0001;
pub const RUNTIME_WINDOW_HANDLE_BASE: u32 = 0x4801_0001;
pub const RUNTIME_SYSTEM_CURSOR_ARROW_RESOURCE_ID: u32 = 0x7f00;
pub const RUNTIME_WINDOW_CLASS_ATOM_BASE: u16 = 0xc000;
pub const RUNTIME_WINDOW_PROCEDURE_ARGUMENT_BYTES: u32 = 16;
pub const RUNTIME_WINDOW_EXTENDED_STYLE_ACCEPT_FILES: u32 = 0x0000_0010;
pub const RUNTIME_WINDOW_STYLE_VISIBLE: u32 = 0x1000_0000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct RuntimeIconId(pub u64);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeIconResource {
    pub module_id: RuntimeModuleId,
    pub module_load_base: GuestAddress,
    pub module_source_sha256: String,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeIcon {
    pub id: RuntimeIconId,
    pub handle: u32,
    pub namespace: String,
    pub shared: bool,
    pub resource: RuntimeIconResource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeLoadIconAResult {
    pub icon_id: RuntimeIconId,
    pub handle: u32,
    pub already_loaded: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct RuntimeCursorId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeSystemCursorKind {
    Arrow,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeCursorResource {
    pub module_id: RuntimeModuleId,
    pub module_load_base: GuestAddress,
    pub module_source_sha256: String,
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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum RuntimeCursorSource {
    SystemPredefined {
        #[serde(rename = "resourceId")]
        resource_id: u32,
        cursor: RuntimeSystemCursorKind,
    },
    ModuleResource {
        resource: RuntimeCursorResource,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeCursor {
    pub id: RuntimeCursorId,
    pub handle: u32,
    pub namespace: String,
    pub shared: bool,
    pub source: RuntimeCursorSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeLoadCursorAResult {
    pub cursor_id: RuntimeCursorId,
    pub handle: u32,
    pub already_loaded: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct RuntimeWindowClassId(pub u64);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeWindowClassModule {
    pub module_id: RuntimeModuleId,
    pub load_base: GuestAddress,
    pub source_sha256: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeWindowProcedureCallingConvention {
    Stdcall,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeWindowProcedure {
    pub module_id: RuntimeModuleId,
    pub address: GuestAddress,
    pub rva: Rva,
    pub calling_convention: RuntimeWindowProcedureCallingConvention,
    pub argument_bytes: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeWindowClassIconReference {
    pub icon_id: RuntimeIconId,
    pub handle: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeWindowClassCursorReference {
    pub cursor_id: RuntimeCursorId,
    pub handle: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeWindowClassNameA {
    pub value: String,
    pub encoding: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeWindowClassRegistration {
    pub module: RuntimeWindowClassModule,
    pub style: u32,
    pub window_procedure: RuntimeWindowProcedure,
    pub class_extra_bytes: u32,
    pub window_extra_bytes: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<RuntimeWindowClassIconReference>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<RuntimeWindowClassCursorReference>,
    pub background_brush: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub menu_name: Option<RuntimeWindowClassNameA>,
    pub class_name: RuntimeWindowClassNameA,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub small_icon: Option<RuntimeWindowClassIconReference>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeWindowClass {
    pub id: RuntimeWindowClassId,
    pub atom: u16,
    pub atom_namespace: String,
    pub local: bool,
    #[serde(flatten)]
    pub registration: RuntimeWindowClassRegistration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeRegisterClassExAResult {
    pub class_id: RuntimeWindowClassId,
    pub atom: u16,
    pub already_registered: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct RuntimeWindowId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeWindowCreationMessage {
    NonClientCreate,
    NonClientCalculate,
    Create,
    Size,
    Move,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum RuntimeWindowLifecycle {
    Creating {
        message: RuntimeWindowCreationMessage,
    },
    Created,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeWindowClassReference {
    pub class_id: RuntimeWindowClassId,
    pub atom: u16,
    pub module_id: RuntimeModuleId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeWindowNameA {
    pub value: String,
    pub encoding: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeWindowIconReference {
    pub icon_id: RuntimeIconId,
    pub handle: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeSetWindowPosVariant {
    InitialShow,
    InitialHide,
    ActivateSameGeometry,
    Resize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
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
    pub callback_flags: u32,
    pub callback_insert_after: u32,
    pub activation_insert_after: u32,
    pub activation_internal_flags: u32,
    pub resize_nccalc_window_pos_flags: u32,
    pub resize_changed_flags: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
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
    pub callback_flags: u32,
    pub callback_insert_after: u32,
    pub activation_insert_after: u32,
    pub activation_internal_flags: u32,
    pub resize_nccalc_window_pos_flags: u32,
    pub resize_changed_flags: u32,
    pub small_icon_handle: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeSetWindowPosDefaultOperation {
    pub window_id: RuntimeWindowId,
    pub window_handle: u32,
    pub owner_thread_id: RuntimeThreadId,
    pub expected_generation: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeWindow {
    pub id: RuntimeWindowId,
    pub handle: u32,
    pub namespace: String,
    #[serde(flatten)]
    pub creation: RuntimeWindowCreationRequest,
    pub lifecycle: RuntimeWindowLifecycle,
    pub visible: bool,
    pub topmost: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub small_icon: Option<RuntimeWindowIconReference>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct RuntimeImeWindowId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeImeStatusWindowRequest {
    Close,
    Open,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeImeWindow {
    pub id: RuntimeImeWindowId,
    pub handle: u32,
    pub namespace: String,
    pub owner_thread_id: RuntimeThreadId,
    pub class_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_window_request: Option<RuntimeImeStatusWindowRequest>,
    pub generation: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeDefaultImeWindowResult {
    pub ime_window_id: RuntimeImeWindowId,
    pub handle: u32,
    pub already_created: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeCreateWindowExAResult {
    pub window_id: RuntimeWindowId,
    pub handle: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeSetWindowStyleResult {
    pub previous_effective_style: u32,
    pub effective_style: u32,
    pub visible: bool,
    pub changed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeWindowFileDropAcceptanceResult {
    pub previous_accepts_files: bool,
    pub accepts_files: bool,
    pub changed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "action", rename_all = "kebab-case")]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "action", rename_all = "kebab-case")]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "action", rename_all = "kebab-case")]
pub enum RuntimeSetWindowPosDefaultCallbackAction {
    Continue {
        operation: RuntimeSetWindowPosDefaultOperation,
        next_message: RuntimeSetWindowPosDefaultCallbackMessage,
    },
    Completed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RuntimeIconLoadDisposition {
    Existing(RuntimeIcon),
    Create {
        icon: RuntimeIcon,
        next_icon_id: u64,
        next_icon_handle: u32,
        next_generation: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RuntimeCursorLoadDisposition {
    Existing(RuntimeCursor),
    Create {
        cursor: RuntimeCursor,
        next_cursor_id: u64,
        next_cursor_handle: u32,
        next_generation: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct RuntimeWindowClassKey {
    module_id: RuntimeModuleId,
    canonical_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RuntimeWindowClassRegistrationDisposition {
    Existing(RuntimeWindowClass),
    Create {
        key: RuntimeWindowClassKey,
        class: RuntimeWindowClass,
        next_class_id: u64,
        next_class_atom: u32,
        next_generation: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RuntimeWindowCreationDisposition {
    Create {
        window: RuntimeWindow,
        next_window_id: u64,
        next_window_handle: u32,
        next_generation: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RuntimeDefaultImeWindowDisposition {
    Existing(RuntimeImeWindow),
    Create {
        window: RuntimeImeWindow,
        next_ime_window_id: u64,
        next_window_handle: u32,
        next_generation: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RuntimeImeControlDisposition {
    Unchanged(RuntimeImeWindow),
    Update {
        previous: RuntimeImeWindow,
        next: RuntimeImeWindow,
        next_generation: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RuntimeWindowCreationCallbackDisposition {
    Update {
        previous: RuntimeWindow,
        next: Box<RuntimeWindow>,
        action: RuntimeWindowCreationCallbackAction,
        next_generation: u64,
    },
    Remove {
        previous: RuntimeWindow,
        action: RuntimeWindowCreationCallbackAction,
        next_generation: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RuntimeWindowSmallIconDisposition {
    Unchanged {
        window: RuntimeWindow,
        previous_handle: u32,
    },
    Update {
        previous: RuntimeWindow,
        next: Box<RuntimeWindow>,
        previous_handle: u32,
        next_generation: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RuntimeWindowStyleUpdateDisposition {
    Unchanged {
        window: RuntimeWindow,
        previous_effective_style: u32,
    },
    Update {
        previous: RuntimeWindow,
        next: Box<RuntimeWindow>,
        previous_effective_style: u32,
        next_generation: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RuntimeWindowFileDropAcceptanceDisposition {
    Unchanged {
        window: RuntimeWindow,
        accepts_files: bool,
    },
    Update {
        previous: RuntimeWindow,
        next: Box<RuntimeWindow>,
        previous_accepts_files: bool,
        next_generation: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RuntimeSetWindowPosCallbackDisposition {
    Validate {
        window: RuntimeWindow,
        action: RuntimeSetWindowPosCallbackAction,
    },
    Update {
        previous: RuntimeWindow,
        next: Box<RuntimeWindow>,
        action: RuntimeSetWindowPosCallbackAction,
        next_generation: u64,
    },
    Activate {
        previous: RuntimeWindow,
        action: RuntimeSetWindowPosCallbackAction,
        next_generation: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeSetWindowPosFocusPlan {
    expected_generation: u64,
    window: RuntimeWindow,
    previous_focused_window: Option<RuntimeWindowId>,
    next_focused_window: RuntimeWindowId,
    next_generation: u64,
    operation: RuntimeSetWindowPosDefaultOperation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeIconLoadPlan {
    expected_generation: u64,
    disposition: RuntimeIconLoadDisposition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeCursorLoadPlan {
    expected_generation: u64,
    disposition: RuntimeCursorLoadDisposition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeWindowClassRegistrationPlan {
    expected_generation: u64,
    disposition: RuntimeWindowClassRegistrationDisposition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeWindowCreationPlan {
    expected_generation: u64,
    disposition: RuntimeWindowCreationDisposition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeDefaultImeWindowPlan {
    expected_generation: u64,
    app_window: RuntimeWindow,
    disposition: RuntimeDefaultImeWindowDisposition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeImeControlPlan {
    expected_generation: u64,
    disposition: RuntimeImeControlDisposition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RuntimeImeAppWindowTarget {
    pub window_id: RuntimeWindowId,
    pub window_handle: u32,
    pub owner_thread_id: RuntimeThreadId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeWindowCreationCallbackPlan {
    expected_generation: u64,
    disposition: RuntimeWindowCreationCallbackDisposition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeWindowSmallIconPlan {
    expected_generation: u64,
    disposition: RuntimeWindowSmallIconDisposition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeWindowStyleUpdatePlan {
    expected_generation: u64,
    disposition: RuntimeWindowStyleUpdateDisposition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeWindowFileDropAcceptancePlan {
    expected_generation: u64,
    disposition: RuntimeWindowFileDropAcceptanceDisposition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeSetWindowPosCallbackPlan {
    expected_generation: u64,
    disposition: RuntimeSetWindowPosCallbackDisposition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RuntimeUser32TeardownPlan {
    expected_generation: u64,
    next_generation: u64,
    retired_icons: usize,
    retired_cursors: usize,
    retired_window_classes: usize,
    retired_windows: usize,
    retired_ime_windows: usize,
    retired_default_ime_windows: usize,
    retired_active_windows: usize,
    retired_focused_windows: usize,
    retired_foreground_window: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RuntimeUser32 {
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    icons: BTreeMap<RuntimeIconId, RuntimeIcon>,
    #[serde(skip)]
    resource_index: BTreeMap<RuntimeIconResource, RuntimeIconId>,
    #[serde(skip)]
    handle_index: BTreeMap<u32, RuntimeIconId>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    cursors: BTreeMap<RuntimeCursorId, RuntimeCursor>,
    #[serde(skip)]
    cursor_source_index: BTreeMap<RuntimeCursorSource, RuntimeCursorId>,
    #[serde(skip)]
    cursor_handle_index: BTreeMap<u32, RuntimeCursorId>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    window_classes: BTreeMap<RuntimeWindowClassId, RuntimeWindowClass>,
    #[serde(skip)]
    window_class_name_index: BTreeMap<RuntimeWindowClassKey, RuntimeWindowClassId>,
    #[serde(skip)]
    window_class_atom_index: BTreeMap<u16, RuntimeWindowClassId>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    windows: BTreeMap<RuntimeWindowId, RuntimeWindow>,
    #[serde(skip)]
    window_handle_index: BTreeMap<u32, RuntimeWindowId>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    ime_windows: BTreeMap<RuntimeImeWindowId, RuntimeImeWindow>,
    #[serde(skip)]
    ime_window_handle_index: BTreeMap<u32, RuntimeImeWindowId>,
    #[serde(skip)]
    default_ime_windows: BTreeMap<RuntimeThreadId, RuntimeImeWindowId>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    active_windows: BTreeMap<RuntimeThreadId, RuntimeWindowId>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    focused_windows: BTreeMap<RuntimeThreadId, RuntimeWindowId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    foreground_window: Option<RuntimeWindowId>,
    next_icon_id: u64,
    next_icon_handle: u32,
    next_cursor_id: u64,
    next_cursor_handle: u32,
    next_window_class_id: u64,
    next_window_class_atom: u32,
    next_window_id: u64,
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
            window_classes: BTreeMap::new(),
            window_class_name_index: BTreeMap::new(),
            window_class_atom_index: BTreeMap::new(),
            windows: BTreeMap::new(),
            window_handle_index: BTreeMap::new(),
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
            next_ime_window_id: 1,
            next_window_handle: RUNTIME_WINDOW_HANDLE_BASE,
            generation: 0,
        }
    }
}

impl RuntimeUser32 {
    pub(super) fn plan_shared_icon_load(
        &self,
        resource: RuntimeIconResource,
    ) -> Result<RuntimeIconLoadPlan, RuntimeStateError> {
        validate_icon_resource(&resource)?;
        if let Some(id) = self.resource_index.get(&resource) {
            let icon = self.icons.get(id).ok_or_else(|| {
                RuntimeStateError::runtime_defect(
                    "runtime-user32",
                    "icon-resource-index-dangling",
                    "shared icon resource index does not resolve to a live icon",
                )
            })?;
            if self.handle_index.get(&icon.handle) != Some(id) {
                return Err(RuntimeStateError::runtime_defect(
                    "runtime-user32",
                    "icon-handle-index-mismatch",
                    "shared icon handle index does not match its resource identity",
                ));
            }
            return Ok(RuntimeIconLoadPlan {
                expected_generation: self.generation,
                disposition: RuntimeIconLoadDisposition::Existing(icon.clone()),
            });
        }
        if self.icons.len() >= RUNTIME_SHARED_ICON_LIMIT {
            return Err(RuntimeStateError::resource_limit(
                "runtime-user32",
                "shared-icon-limit-exceeded",
                "process shared icon count exceeds the runtime bound",
            ));
        }
        let id = RuntimeIconId(self.next_icon_id);
        if id.0 == 0 || self.icons.contains_key(&id) {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-user32",
                "icon-id-collision",
                "shared icon allocator selected an invalid or live identity",
            ));
        }
        if self.next_icon_handle < RUNTIME_ICON_HANDLE_BASE
            || self.handle_index.contains_key(&self.next_icon_handle)
        {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-user32",
                "icon-handle-collision",
                "shared icon allocator selected an invalid or live handle",
            ));
        }
        let next_icon_id = self.next_icon_id.checked_add(1).ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-user32",
                "icon-id-space-exhausted",
                "shared icon ID space is exhausted",
            )
        })?;
        let next_icon_handle = self.next_icon_handle.checked_add(1).ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-user32",
                "icon-handle-space-exhausted",
                "shared icon handle space is exhausted",
            )
        })?;
        Ok(RuntimeIconLoadPlan {
            expected_generation: self.generation,
            disposition: RuntimeIconLoadDisposition::Create {
                icon: RuntimeIcon {
                    id,
                    handle: self.next_icon_handle,
                    namespace: LOGICAL_ICON_HANDLE_NAMESPACE.to_owned(),
                    shared: true,
                    resource,
                },
                next_icon_id,
                next_icon_handle,
                next_generation: self.next_generation()?,
            },
        })
    }

    pub(super) fn preflight_shared_icon_load(
        &self,
        plan: &RuntimeIconLoadPlan,
    ) -> Result<(), RuntimeStateError> {
        if self.generation != plan.expected_generation {
            return Err(RuntimeStateError::conflict(
                "runtime-user32",
                "icon-load-plan-stale",
                "shared icon state changed after load planning",
            ));
        }
        match &plan.disposition {
            RuntimeIconLoadDisposition::Existing(expected) => {
                if self.icons.get(&expected.id) != Some(expected)
                    || self.resource_index.get(&expected.resource) != Some(&expected.id)
                    || self.handle_index.get(&expected.handle) != Some(&expected.id)
                {
                    return Err(RuntimeStateError::conflict(
                        "runtime-user32",
                        "icon-load-plan-stale",
                        "shared icon identity changed after load planning",
                    ));
                }
            }
            RuntimeIconLoadDisposition::Create { icon, .. } => {
                if self.next_icon_id != icon.id.0
                    || self.next_icon_handle != icon.handle
                    || self.icons.contains_key(&icon.id)
                    || self.resource_index.contains_key(&icon.resource)
                    || self.handle_index.contains_key(&icon.handle)
                {
                    return Err(RuntimeStateError::conflict(
                        "runtime-user32",
                        "icon-load-plan-stale",
                        "shared icon allocator changed after load planning",
                    ));
                }
            }
        }
        Ok(())
    }

    pub(super) fn commit_shared_icon_load(
        &mut self,
        plan: RuntimeIconLoadPlan,
    ) -> Result<RuntimeLoadIconAResult, RuntimeStateError> {
        self.preflight_shared_icon_load(&plan)?;
        match plan.disposition {
            RuntimeIconLoadDisposition::Existing(icon) => Ok(RuntimeLoadIconAResult {
                icon_id: icon.id,
                handle: icon.handle,
                already_loaded: true,
            }),
            RuntimeIconLoadDisposition::Create {
                icon,
                next_icon_id,
                next_icon_handle,
                next_generation,
            } => {
                let result = RuntimeLoadIconAResult {
                    icon_id: icon.id,
                    handle: icon.handle,
                    already_loaded: false,
                };
                self.next_icon_id = next_icon_id;
                self.next_icon_handle = next_icon_handle;
                self.generation = next_generation;
                let previous_resource = self.resource_index.insert(icon.resource.clone(), icon.id);
                let previous_handle = self.handle_index.insert(icon.handle, icon.id);
                let previous_icon = self.icons.insert(icon.id, icon);
                debug_assert!(previous_resource.is_none());
                debug_assert!(previous_handle.is_none());
                debug_assert!(previous_icon.is_none());
                Ok(result)
            }
        }
    }

    pub(super) fn plan_shared_cursor_load(
        &self,
        source: RuntimeCursorSource,
    ) -> Result<RuntimeCursorLoadPlan, RuntimeStateError> {
        validate_cursor_source(&source)?;
        if let Some(id) = self.cursor_source_index.get(&source) {
            let cursor = self.cursors.get(id).ok_or_else(|| {
                RuntimeStateError::runtime_defect(
                    "runtime-user32",
                    "cursor-source-index-dangling",
                    "shared cursor source index does not resolve to a live cursor",
                )
            })?;
            if self.cursor_handle_index.get(&cursor.handle) != Some(id) {
                return Err(RuntimeStateError::runtime_defect(
                    "runtime-user32",
                    "cursor-handle-index-mismatch",
                    "shared cursor handle index does not match its source identity",
                ));
            }
            return Ok(RuntimeCursorLoadPlan {
                expected_generation: self.generation,
                disposition: RuntimeCursorLoadDisposition::Existing(cursor.clone()),
            });
        }
        if self.cursors.len() >= RUNTIME_SHARED_CURSOR_LIMIT {
            return Err(RuntimeStateError::resource_limit(
                "runtime-user32",
                "shared-cursor-limit-exceeded",
                "process shared cursor count exceeds the runtime bound",
            ));
        }
        let id = RuntimeCursorId(self.next_cursor_id);
        if id.0 == 0 || self.cursors.contains_key(&id) {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-user32",
                "cursor-id-collision",
                "shared cursor allocator selected an invalid or live identity",
            ));
        }
        if self.next_cursor_handle < RUNTIME_CURSOR_HANDLE_BASE
            || self
                .cursor_handle_index
                .contains_key(&self.next_cursor_handle)
        {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-user32",
                "cursor-handle-collision",
                "shared cursor allocator selected an invalid or live handle",
            ));
        }
        let next_cursor_id = self.next_cursor_id.checked_add(1).ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-user32",
                "cursor-id-space-exhausted",
                "shared cursor ID space is exhausted",
            )
        })?;
        let next_cursor_handle = self.next_cursor_handle.checked_add(1).ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-user32",
                "cursor-handle-space-exhausted",
                "shared cursor handle space is exhausted",
            )
        })?;
        Ok(RuntimeCursorLoadPlan {
            expected_generation: self.generation,
            disposition: RuntimeCursorLoadDisposition::Create {
                cursor: RuntimeCursor {
                    id,
                    handle: self.next_cursor_handle,
                    namespace: LOGICAL_CURSOR_HANDLE_NAMESPACE.to_owned(),
                    shared: true,
                    source,
                },
                next_cursor_id,
                next_cursor_handle,
                next_generation: self.next_generation()?,
            },
        })
    }

    pub(super) fn preflight_shared_cursor_load(
        &self,
        plan: &RuntimeCursorLoadPlan,
    ) -> Result<(), RuntimeStateError> {
        if self.generation != plan.expected_generation {
            return Err(RuntimeStateError::conflict(
                "runtime-user32",
                "cursor-load-plan-stale",
                "shared cursor state changed after load planning",
            ));
        }
        match &plan.disposition {
            RuntimeCursorLoadDisposition::Existing(expected) => {
                if self.cursors.get(&expected.id) != Some(expected)
                    || self.cursor_source_index.get(&expected.source) != Some(&expected.id)
                    || self.cursor_handle_index.get(&expected.handle) != Some(&expected.id)
                {
                    return Err(RuntimeStateError::conflict(
                        "runtime-user32",
                        "cursor-load-plan-stale",
                        "shared cursor identity changed after load planning",
                    ));
                }
            }
            RuntimeCursorLoadDisposition::Create { cursor, .. } => {
                if self.next_cursor_id != cursor.id.0
                    || self.next_cursor_handle != cursor.handle
                    || self.cursors.contains_key(&cursor.id)
                    || self.cursor_source_index.contains_key(&cursor.source)
                    || self.cursor_handle_index.contains_key(&cursor.handle)
                {
                    return Err(RuntimeStateError::conflict(
                        "runtime-user32",
                        "cursor-load-plan-stale",
                        "shared cursor allocator changed after load planning",
                    ));
                }
            }
        }
        Ok(())
    }

    pub(super) fn commit_shared_cursor_load(
        &mut self,
        plan: RuntimeCursorLoadPlan,
    ) -> Result<RuntimeLoadCursorAResult, RuntimeStateError> {
        self.preflight_shared_cursor_load(&plan)?;
        match plan.disposition {
            RuntimeCursorLoadDisposition::Existing(cursor) => Ok(RuntimeLoadCursorAResult {
                cursor_id: cursor.id,
                handle: cursor.handle,
                already_loaded: true,
            }),
            RuntimeCursorLoadDisposition::Create {
                cursor,
                next_cursor_id,
                next_cursor_handle,
                next_generation,
            } => {
                let result = RuntimeLoadCursorAResult {
                    cursor_id: cursor.id,
                    handle: cursor.handle,
                    already_loaded: false,
                };
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
                Ok(result)
            }
        }
    }

    pub(super) fn plan_window_class_registration(
        &self,
        registration: RuntimeWindowClassRegistration,
    ) -> Result<RuntimeWindowClassRegistrationPlan, RuntimeStateError> {
        validate_window_class_registration(&registration)?;
        self.validate_window_class_references(&registration)?;
        let key = window_class_key(&registration)?;
        if let Some(id) = self.window_class_name_index.get(&key) {
            let class = self.window_classes.get(id).ok_or_else(|| {
                RuntimeStateError::runtime_defect(
                    "runtime-user32",
                    "window-class-name-index-dangling",
                    "window class name index does not resolve to a live class",
                )
            })?;
            if self.window_class_atom_index.get(&class.atom) != Some(id) {
                return Err(RuntimeStateError::runtime_defect(
                    "runtime-user32",
                    "window-class-atom-index-mismatch",
                    "window class atom index does not match its name identity",
                ));
            }
            return Ok(RuntimeWindowClassRegistrationPlan {
                expected_generation: self.generation,
                disposition: RuntimeWindowClassRegistrationDisposition::Existing(class.clone()),
            });
        }
        if self.window_classes.len() >= RUNTIME_WINDOW_CLASS_LIMIT {
            return Err(RuntimeStateError::resource_limit(
                "runtime-user32",
                "window-class-limit-exceeded",
                "process-local window class count exceeds the runtime bound",
            ));
        }
        let id = RuntimeWindowClassId(self.next_window_class_id);
        if id.0 == 0 || self.window_classes.contains_key(&id) {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-user32",
                "window-class-id-collision",
                "window class allocator selected an invalid or live identity",
            ));
        }
        let atom = u16::try_from(self.next_window_class_atom).map_err(|_| {
            RuntimeStateError::resource_limit(
                "runtime-user32",
                "window-class-atom-space-exhausted",
                "process-local window class atom space is exhausted",
            )
        })?;
        if atom < RUNTIME_WINDOW_CLASS_ATOM_BASE || self.window_class_atom_index.contains_key(&atom)
        {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-user32",
                "window-class-atom-collision",
                "window class atom allocator selected an invalid or live atom",
            ));
        }
        let next_class_id = self.next_window_class_id.checked_add(1).ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-user32",
                "window-class-id-space-exhausted",
                "window class ID space is exhausted",
            )
        })?;
        let next_class_atom = self.next_window_class_atom.checked_add(1).ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-user32",
                "window-class-atom-space-exhausted",
                "window class atom space is exhausted",
            )
        })?;
        Ok(RuntimeWindowClassRegistrationPlan {
            expected_generation: self.generation,
            disposition: RuntimeWindowClassRegistrationDisposition::Create {
                key,
                class: RuntimeWindowClass {
                    id,
                    atom,
                    atom_namespace: LOGICAL_WINDOW_CLASS_ATOM_NAMESPACE.to_owned(),
                    local: true,
                    registration,
                },
                next_class_id,
                next_class_atom,
                next_generation: self.next_generation()?,
            },
        })
    }

    pub(super) fn preflight_window_class_registration(
        &self,
        plan: &RuntimeWindowClassRegistrationPlan,
    ) -> Result<(), RuntimeStateError> {
        if self.generation != plan.expected_generation {
            return Err(RuntimeStateError::conflict(
                "runtime-user32",
                "window-class-registration-plan-stale",
                "User32 state changed after window class registration planning",
            ));
        }
        match &plan.disposition {
            RuntimeWindowClassRegistrationDisposition::Existing(expected) => {
                let key = window_class_key(&expected.registration)?;
                if self.window_classes.get(&expected.id) != Some(expected)
                    || self.window_class_name_index.get(&key) != Some(&expected.id)
                    || self.window_class_atom_index.get(&expected.atom) != Some(&expected.id)
                {
                    return Err(RuntimeStateError::conflict(
                        "runtime-user32",
                        "window-class-registration-plan-stale",
                        "existing window class identity changed after planning",
                    ));
                }
                self.validate_window_class_references(&expected.registration)?;
            }
            RuntimeWindowClassRegistrationDisposition::Create { key, class, .. } => {
                if self.next_window_class_id != class.id.0
                    || self.next_window_class_atom != u32::from(class.atom)
                    || self.window_classes.contains_key(&class.id)
                    || self.window_class_name_index.contains_key(key)
                    || self.window_class_atom_index.contains_key(&class.atom)
                {
                    return Err(RuntimeStateError::conflict(
                        "runtime-user32",
                        "window-class-registration-plan-stale",
                        "window class allocators or indexes changed after planning",
                    ));
                }
                self.validate_window_class_references(&class.registration)?;
            }
        }
        Ok(())
    }

    pub(super) fn commit_window_class_registration(
        &mut self,
        plan: RuntimeWindowClassRegistrationPlan,
    ) -> Result<RuntimeRegisterClassExAResult, RuntimeStateError> {
        self.preflight_window_class_registration(&plan)?;
        match plan.disposition {
            RuntimeWindowClassRegistrationDisposition::Existing(class) => {
                Ok(RuntimeRegisterClassExAResult {
                    class_id: class.id,
                    atom: class.atom,
                    already_registered: true,
                })
            }
            RuntimeWindowClassRegistrationDisposition::Create {
                key,
                class,
                next_class_id,
                next_class_atom,
                next_generation,
            } => {
                let result = RuntimeRegisterClassExAResult {
                    class_id: class.id,
                    atom: class.atom,
                    already_registered: false,
                };
                self.next_window_class_id = next_class_id;
                self.next_window_class_atom = next_class_atom;
                self.generation = next_generation;
                let previous_name = self.window_class_name_index.insert(key, class.id);
                let previous_atom = self.window_class_atom_index.insert(class.atom, class.id);
                let previous_class = self.window_classes.insert(class.id, class);
                debug_assert!(previous_name.is_none());
                debug_assert!(previous_atom.is_none());
                debug_assert!(previous_class.is_none());
                Ok(result)
            }
        }
    }

    pub(super) fn plan_window_creation(
        &self,
        request: RuntimeWindowCreationRequest,
    ) -> Result<RuntimeWindowCreationPlan, RuntimeStateError> {
        validate_window_creation_request(&request)?;
        self.validate_window_creation_references(&request)?;
        if self.windows.len() >= RUNTIME_WINDOW_LIMIT {
            return Err(RuntimeStateError::resource_limit(
                "runtime-user32",
                "window-limit-exceeded",
                "logical window count exceeds the runtime bound",
            ));
        }
        let id = RuntimeWindowId(self.next_window_id);
        if id.0 == 0 || self.windows.contains_key(&id) {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-user32",
                "window-id-collision",
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
            return Err(RuntimeStateError::runtime_defect(
                "runtime-user32",
                "window-handle-collision",
                "logical HWND allocator selected an invalid or live handle",
            ));
        }
        let next_window_id = self.next_window_id.checked_add(1).ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-user32",
                "window-id-space-exhausted",
                "logical window ID space is exhausted",
            )
        })?;
        let next_window_handle = self.next_window_handle.checked_add(1).ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-user32",
                "window-handle-space-exhausted",
                "logical HWND space is exhausted",
            )
        })?;
        Ok(RuntimeWindowCreationPlan {
            expected_generation: self.generation,
            disposition: RuntimeWindowCreationDisposition::Create {
                window: RuntimeWindow {
                    id,
                    handle: self.next_window_handle,
                    namespace: LOGICAL_WINDOW_HANDLE_NAMESPACE.to_owned(),
                    creation: request,
                    lifecycle: RuntimeWindowLifecycle::Creating {
                        message: RuntimeWindowCreationMessage::NonClientCreate,
                    },
                    visible: false,
                    topmost: false,
                    small_icon: None,
                },
                next_window_id,
                next_window_handle,
                next_generation: self.next_generation()?,
            },
        })
    }

    #[cfg(test)]
    pub(super) fn window_creation_result(
        &self,
        plan: &RuntimeWindowCreationPlan,
    ) -> RuntimeCreateWindowExAResult {
        match &plan.disposition {
            RuntimeWindowCreationDisposition::Create { window, .. } => {
                RuntimeCreateWindowExAResult {
                    window_id: window.id,
                    handle: window.handle,
                }
            }
        }
    }

    pub(super) fn window_creation_preview<'a>(
        &self,
        plan: &'a RuntimeWindowCreationPlan,
    ) -> &'a RuntimeWindow {
        match &plan.disposition {
            RuntimeWindowCreationDisposition::Create { window, .. } => window,
        }
    }

    pub(super) fn preflight_window_creation(
        &self,
        plan: &RuntimeWindowCreationPlan,
    ) -> Result<(), RuntimeStateError> {
        if self.generation != plan.expected_generation {
            return Err(RuntimeStateError::conflict(
                "runtime-user32",
                "window-creation-plan-stale",
                "User32 state changed after logical window creation planning",
            ));
        }
        match &plan.disposition {
            RuntimeWindowCreationDisposition::Create { window, .. } => {
                if self.next_window_id != window.id.0
                    || self.next_window_handle != window.handle
                    || self.windows.contains_key(&window.id)
                    || self.window_handle_index.contains_key(&window.handle)
                    || self.ime_window_handle_index.contains_key(&window.handle)
                {
                    return Err(RuntimeStateError::conflict(
                        "runtime-user32",
                        "window-creation-plan-stale",
                        "logical window allocators or indexes changed after planning",
                    ));
                }
                self.validate_window_creation_references(&window.creation)?;
            }
        }
        Ok(())
    }

    pub(super) fn commit_window_creation(
        &mut self,
        plan: RuntimeWindowCreationPlan,
    ) -> Result<RuntimeCreateWindowExAResult, RuntimeStateError> {
        self.preflight_window_creation(&plan)?;
        match plan.disposition {
            RuntimeWindowCreationDisposition::Create {
                window,
                next_window_id,
                next_window_handle,
                next_generation,
            } => {
                let result = RuntimeCreateWindowExAResult {
                    window_id: window.id,
                    handle: window.handle,
                };
                self.next_window_id = next_window_id;
                self.next_window_handle = next_window_handle;
                self.generation = next_generation;
                let previous_handle = self.window_handle_index.insert(window.handle, window.id);
                let previous_window = self.windows.insert(window.id, window);
                debug_assert!(previous_handle.is_none());
                debug_assert!(previous_window.is_none());
                Ok(result)
            }
        }
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

    pub(super) fn plan_default_ime_window(
        &self,
        window_handle: u32,
        owner_thread_id: RuntimeThreadId,
    ) -> Result<RuntimeDefaultImeWindowPlan, RuntimeStateError> {
        let app_window = self.window(window_handle)?.clone();
        self.validate_created_window_owner(&app_window, owner_thread_id)?;
        if let Some(id) = self.default_ime_windows.get(&owner_thread_id) {
            let window = self.ime_windows.get(id).cloned().ok_or_else(|| {
                RuntimeStateError::runtime_defect(
                    "runtime-user32",
                    "default-ime-window-index-dangling",
                    "default IME window index does not resolve to a live logical window",
                )
            })?;
            self.validate_ime_window_identity(&window)?;
            if window.owner_thread_id != owner_thread_id {
                return Err(RuntimeStateError::runtime_defect(
                    "runtime-user32",
                    "default-ime-window-owner-mismatch",
                    "default IME window index resolves to a different owner thread",
                ));
            }
            return Ok(RuntimeDefaultImeWindowPlan {
                expected_generation: self.generation,
                app_window,
                disposition: RuntimeDefaultImeWindowDisposition::Existing(window),
            });
        }
        if self.ime_windows.len() >= RUNTIME_DEFAULT_IME_WINDOW_LIMIT {
            return Err(RuntimeStateError::resource_limit(
                "runtime-user32",
                "default-ime-window-limit-exceeded",
                "logical default IME window count exceeds the runtime bound",
            ));
        }
        let id = RuntimeImeWindowId(self.next_ime_window_id);
        if id.0 == 0 || self.ime_windows.contains_key(&id) {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-user32",
                "default-ime-window-id-collision",
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
            return Err(RuntimeStateError::runtime_defect(
                "runtime-user32",
                "default-ime-window-handle-collision",
                "logical HWND allocator selected an invalid or live IME window handle",
            ));
        }
        let next_ime_window_id = self.next_ime_window_id.checked_add(1).ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-user32",
                "default-ime-window-id-space-exhausted",
                "logical default IME window identity space is exhausted",
            )
        })?;
        let next_window_handle = self.next_window_handle.checked_add(1).ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-user32",
                "window-handle-space-exhausted",
                "logical HWND space is exhausted",
            )
        })?;
        let next_generation = self.next_generation()?;
        Ok(RuntimeDefaultImeWindowPlan {
            expected_generation: self.generation,
            app_window,
            disposition: RuntimeDefaultImeWindowDisposition::Create {
                window: RuntimeImeWindow {
                    id,
                    handle: self.next_window_handle,
                    namespace: LOGICAL_WINDOW_HANDLE_NAMESPACE.to_owned(),
                    owner_thread_id,
                    class_name: "IME".to_owned(),
                    status_window_request: None,
                    generation: next_generation,
                },
                next_ime_window_id,
                next_window_handle,
                next_generation,
            },
        })
    }

    pub(super) fn default_ime_window_target(
        &self,
        plan: &RuntimeDefaultImeWindowPlan,
    ) -> RuntimeImeAppWindowTarget {
        RuntimeImeAppWindowTarget {
            window_id: plan.app_window.id,
            window_handle: plan.app_window.handle,
            owner_thread_id: plan.app_window.creation.owner_thread_id,
        }
    }

    pub(super) fn preflight_default_ime_window(
        &self,
        plan: &RuntimeDefaultImeWindowPlan,
    ) -> Result<(), RuntimeStateError> {
        if self.generation != plan.expected_generation
            || self.windows.get(&plan.app_window.id) != Some(&plan.app_window)
            || self.window_handle_index.get(&plan.app_window.handle) != Some(&plan.app_window.id)
        {
            return Err(RuntimeStateError::conflict(
                "runtime-user32",
                "default-ime-window-plan-stale",
                "User32 app-window state changed after default IME window planning",
            ));
        }
        self.validate_created_window_owner(
            &plan.app_window,
            plan.app_window.creation.owner_thread_id,
        )?;
        match &plan.disposition {
            RuntimeDefaultImeWindowDisposition::Existing(window) => {
                self.validate_ime_window_identity(window)?;
                if self.default_ime_windows.get(&window.owner_thread_id) != Some(&window.id)
                    || self.ime_windows.get(&window.id) != Some(window)
                {
                    return Err(RuntimeStateError::conflict(
                        "runtime-user32",
                        "default-ime-window-plan-stale",
                        "default IME window identity changed after planning",
                    ));
                }
            }
            RuntimeDefaultImeWindowDisposition::Create { window, .. } => {
                if self.next_ime_window_id != window.id.0
                    || self.next_window_handle != window.handle
                    || self.ime_windows.len() >= RUNTIME_DEFAULT_IME_WINDOW_LIMIT
                    || self.ime_windows.contains_key(&window.id)
                    || self.ime_window_handle_index.contains_key(&window.handle)
                    || self.window_handle_index.contains_key(&window.handle)
                    || self
                        .default_ime_windows
                        .contains_key(&window.owner_thread_id)
                {
                    return Err(RuntimeStateError::conflict(
                        "runtime-user32",
                        "default-ime-window-plan-stale",
                        "default IME window allocators or indexes changed after planning",
                    ));
                }
            }
        }
        Ok(())
    }

    pub(super) fn commit_default_ime_window_preflighted(
        &mut self,
        plan: RuntimeDefaultImeWindowPlan,
    ) -> RuntimeDefaultImeWindowResult {
        debug_assert!(self.preflight_default_ime_window(&plan).is_ok());
        match plan.disposition {
            RuntimeDefaultImeWindowDisposition::Existing(window) => RuntimeDefaultImeWindowResult {
                ime_window_id: window.id,
                handle: window.handle,
                already_created: true,
            },
            RuntimeDefaultImeWindowDisposition::Create {
                window,
                next_ime_window_id,
                next_window_handle,
                next_generation,
            } => {
                let result = RuntimeDefaultImeWindowResult {
                    ime_window_id: window.id,
                    handle: window.handle,
                    already_created: false,
                };
                self.next_ime_window_id = next_ime_window_id;
                self.next_window_handle = next_window_handle;
                self.generation = next_generation;
                let previous_handle = self
                    .ime_window_handle_index
                    .insert(window.handle, window.id);
                let previous_default = self
                    .default_ime_windows
                    .insert(window.owner_thread_id, window.id);
                let previous_window = self.ime_windows.insert(window.id, window);
                debug_assert!(previous_handle.is_none());
                debug_assert!(previous_default.is_none());
                debug_assert!(previous_window.is_none());
                result
            }
        }
    }

    pub(super) fn plan_window_creation_callback(
        &self,
        window_id: RuntimeWindowId,
        expected_message: RuntimeWindowCreationMessage,
        result: u32,
    ) -> Result<RuntimeWindowCreationCallbackPlan, RuntimeStateError> {
        let previous = self.windows.get(&window_id).cloned().ok_or_else(|| {
            RuntimeStateError::not_found(
                "runtime-user32",
                "window-id-unknown",
                "logical window creation callback identifies no live window",
            )
        })?;
        if self.window_handle_index.get(&previous.handle) != Some(&window_id) {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-user32",
                "window-handle-index-mismatch",
                "logical HWND index does not match its window identity",
            ));
        }
        if previous.lifecycle
            != (RuntimeWindowLifecycle::Creating {
                message: expected_message,
            })
        {
            return Err(RuntimeStateError::conflict(
                "runtime-user32",
                "window-creation-callback-order-mismatch",
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
                previous,
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
                    previous,
                    next: Box::new(next),
                    next_generation,
                }
            } else {
                next.lifecycle = RuntimeWindowLifecycle::Created;
                RuntimeWindowCreationCallbackDisposition::Update {
                    action: RuntimeWindowCreationCallbackAction::Created {
                        window: next.clone(),
                    },
                    previous,
                    next: Box::new(next),
                    next_generation,
                }
            }
        };
        Ok(RuntimeWindowCreationCallbackPlan {
            expected_generation: self.generation,
            disposition,
        })
    }

    pub(super) fn window_creation_callback_action<'a>(
        &self,
        plan: &'a RuntimeWindowCreationCallbackPlan,
    ) -> &'a RuntimeWindowCreationCallbackAction {
        match &plan.disposition {
            RuntimeWindowCreationCallbackDisposition::Update { action, .. }
            | RuntimeWindowCreationCallbackDisposition::Remove { action, .. } => action,
        }
    }

    pub(super) fn preflight_window_creation_callback(
        &self,
        plan: &RuntimeWindowCreationCallbackPlan,
    ) -> Result<(), RuntimeStateError> {
        if self.generation != plan.expected_generation {
            return Err(RuntimeStateError::conflict(
                "runtime-user32",
                "window-creation-callback-plan-stale",
                "User32 state changed after logical window callback planning",
            ));
        }
        let previous = match &plan.disposition {
            RuntimeWindowCreationCallbackDisposition::Update { previous, .. }
            | RuntimeWindowCreationCallbackDisposition::Remove { previous, .. } => previous,
        };
        if self.windows.get(&previous.id) != Some(previous)
            || self.window_handle_index.get(&previous.handle) != Some(&previous.id)
        {
            return Err(RuntimeStateError::conflict(
                "runtime-user32",
                "window-creation-callback-plan-stale",
                "logical window identity changed after callback planning",
            ));
        }
        self.validate_window_creation_references(&previous.creation)
    }

    pub(super) fn commit_window_creation_callback(
        &mut self,
        plan: RuntimeWindowCreationCallbackPlan,
    ) -> Result<RuntimeWindowCreationCallbackAction, RuntimeStateError> {
        self.preflight_window_creation_callback(&plan)?;
        match plan.disposition {
            RuntimeWindowCreationCallbackDisposition::Update {
                next,
                action,
                next_generation,
                ..
            } => {
                let next = *next;
                self.generation = next_generation;
                let previous = self.windows.insert(next.id, next);
                debug_assert!(previous.is_some());
                Ok(action)
            }
            RuntimeWindowCreationCallbackDisposition::Remove {
                previous,
                action,
                next_generation,
            } => {
                self.generation = next_generation;
                let removed_handle = self.window_handle_index.remove(&previous.handle);
                let removed_window = self.windows.remove(&previous.id);
                debug_assert_eq!(removed_handle, Some(previous.id));
                debug_assert_eq!(removed_window.as_ref(), Some(&previous));
                Ok(action)
            }
        }
    }

    fn validate_window_creation_references(
        &self,
        request: &RuntimeWindowCreationRequest,
    ) -> Result<(), RuntimeStateError> {
        let class = self
            .window_classes
            .get(&request.class.class_id)
            .ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-user32",
                    "window-class-id-unknown",
                    "logical window refers to a non-live process-local class",
                )
            })?;
        if class.atom != request.class.atom
            || class.registration.module.module_id != request.class.module_id
            || class.registration.module != request.instance
            || self.window_class_atom_index.get(&class.atom) != Some(&class.id)
        {
            return Err(RuntimeStateError::invalid_input(
                "runtime-user32",
                "window-class-reference-mismatch",
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
                RuntimeStateError::not_found(
                    "runtime-user32",
                    "window-class-icon-unknown",
                    "window class refers to a non-live logical icon identity",
                )
            })?;
            if icon.handle != reference.handle
                || self.handle_index.get(&reference.handle) != Some(&reference.icon_id)
            {
                return Err(RuntimeStateError::invalid_input(
                    "runtime-user32",
                    "window-class-icon-mismatch",
                    "window class icon ID and logical handle do not identify the same icon",
                ));
            }
        }
        if let Some(reference) = registration.cursor {
            let cursor = self.cursors.get(&reference.cursor_id).ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-user32",
                    "window-class-cursor-unknown",
                    "window class refers to a non-live logical cursor identity",
                )
            })?;
            if cursor.handle != reference.handle
                || self.cursor_handle_index.get(&reference.handle) != Some(&reference.cursor_id)
            {
                return Err(RuntimeStateError::invalid_input(
                    "runtime-user32",
                    "window-class-cursor-mismatch",
                    "window class cursor ID and logical handle do not identify the same cursor",
                ));
            }
        }
        Ok(())
    }

    pub(super) fn plan_process_teardown(
        &self,
    ) -> Result<Option<RuntimeUser32TeardownPlan>, RuntimeStateError> {
        if self.icons.is_empty()
            && self.cursors.is_empty()
            && self.window_classes.is_empty()
            && self.windows.is_empty()
            && self.ime_windows.is_empty()
            && self.default_ime_windows.is_empty()
            && self.active_windows.is_empty()
            && self.focused_windows.is_empty()
            && self.foreground_window.is_none()
        {
            return Ok(None);
        }
        Ok(Some(RuntimeUser32TeardownPlan {
            expected_generation: self.generation,
            next_generation: self.next_generation()?,
            retired_icons: self.icons.len(),
            retired_cursors: self.cursors.len(),
            retired_window_classes: self.window_classes.len(),
            retired_windows: self.windows.len(),
            retired_ime_windows: self.ime_windows.len(),
            retired_default_ime_windows: self.default_ime_windows.len(),
            retired_active_windows: self.active_windows.len(),
            retired_focused_windows: self.focused_windows.len(),
            retired_foreground_window: self.foreground_window.is_some(),
        }))
    }

    pub(super) fn preflight_process_teardown(
        &self,
        plan: &RuntimeUser32TeardownPlan,
    ) -> Result<(), RuntimeStateError> {
        if self.generation != plan.expected_generation
            || self.icons.len() != plan.retired_icons
            || self.resource_index.len() != plan.retired_icons
            || self.handle_index.len() != plan.retired_icons
            || self.cursors.len() != plan.retired_cursors
            || self.cursor_source_index.len() != plan.retired_cursors
            || self.cursor_handle_index.len() != plan.retired_cursors
            || self.window_classes.len() != plan.retired_window_classes
            || self.window_class_name_index.len() != plan.retired_window_classes
            || self.window_class_atom_index.len() != plan.retired_window_classes
            || self.windows.len() != plan.retired_windows
            || self.window_handle_index.len() != plan.retired_windows
            || self.ime_windows.len() != plan.retired_ime_windows
            || self.ime_window_handle_index.len() != plan.retired_ime_windows
            || self.default_ime_windows.len() != plan.retired_default_ime_windows
            || self.active_windows.len() != plan.retired_active_windows
            || self.focused_windows.len() != plan.retired_focused_windows
            || self.foreground_window.is_some() != plan.retired_foreground_window
        {
            return Err(RuntimeStateError::conflict(
                "runtime-user32",
                "user32-teardown-plan-stale",
                "User32 resource state changed after process teardown planning",
            ));
        }
        Ok(())
    }

    pub(super) fn commit_process_teardown(&mut self, plan: RuntimeUser32TeardownPlan) {
        debug_assert!(self.preflight_process_teardown(&plan).is_ok());
        self.windows.clear();
        self.window_handle_index.clear();
        self.ime_windows.clear();
        self.ime_window_handle_index.clear();
        self.default_ime_windows.clear();
        self.active_windows.clear();
        self.focused_windows.clear();
        self.foreground_window = None;
        self.window_classes.clear();
        self.window_class_name_index.clear();
        self.window_class_atom_index.clear();
        self.icons.clear();
        self.resource_index.clear();
        self.handle_index.clear();
        self.cursors.clear();
        self.cursor_source_index.clear();
        self.cursor_handle_index.clear();
        self.generation = plan.next_generation;
    }

    pub fn icon(&self, handle: u32) -> Result<&RuntimeIcon, RuntimeStateError> {
        let id = self.handle_index.get(&handle).copied().ok_or_else(|| {
            RuntimeStateError::not_found(
                "runtime-user32",
                "icon-handle-unknown",
                format!("logical HICON {handle:#010x} is not live"),
            )
        })?;
        self.icons.get(&id).ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-user32",
                "icon-handle-index-dangling",
                "logical HICON index does not resolve to a live icon",
            )
        })
    }

    pub fn icons(&self) -> &BTreeMap<RuntimeIconId, RuntimeIcon> {
        &self.icons
    }

    pub fn cursor(&self, handle: u32) -> Result<&RuntimeCursor, RuntimeStateError> {
        let id = self
            .cursor_handle_index
            .get(&handle)
            .copied()
            .ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-user32",
                    "cursor-handle-unknown",
                    format!("logical HCURSOR {handle:#010x} is not live"),
                )
            })?;
        self.cursors.get(&id).ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-user32",
                "cursor-handle-index-dangling",
                "logical HCURSOR index does not resolve to a live cursor",
            )
        })
    }

    pub fn cursors(&self) -> &BTreeMap<RuntimeCursorId, RuntimeCursor> {
        &self.cursors
    }

    pub fn window_class(&self, atom: u16) -> Result<&RuntimeWindowClass, RuntimeStateError> {
        let id = self
            .window_class_atom_index
            .get(&atom)
            .copied()
            .ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-user32",
                    "window-class-atom-unknown",
                    format!("logical window class ATOM {atom:#06x} is not live"),
                )
            })?;
        self.window_classes.get(&id).ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-user32",
                "window-class-atom-index-dangling",
                "logical window class ATOM index does not resolve to a live class",
            )
        })
    }

    pub fn window_class_by_name(
        &self,
        module_id: RuntimeModuleId,
        name: &str,
    ) -> Result<&RuntimeWindowClass, RuntimeStateError> {
        let key = window_class_key_from_name(module_id, name)?;
        let id = self
            .window_class_name_index
            .get(&key)
            .copied()
            .ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-user32",
                    "window-class-name-unknown",
                    "process-local window class name is not registered for the module",
                )
            })?;
        self.window_classes.get(&id).ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-user32",
                "window-class-name-index-dangling",
                "logical window class name index does not resolve to a live class",
            )
        })
    }

    pub fn window_classes(&self) -> &BTreeMap<RuntimeWindowClassId, RuntimeWindowClass> {
        &self.window_classes
    }

    pub fn window(&self, handle: u32) -> Result<&RuntimeWindow, RuntimeStateError> {
        let id = self
            .window_handle_index
            .get(&handle)
            .copied()
            .ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-user32",
                    "window-handle-unknown",
                    format!("logical HWND {handle:#010x} is not live"),
                )
            })?;
        self.windows.get(&id).ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-user32",
                "window-handle-index-dangling",
                "logical HWND index does not resolve to a live window",
            )
        })
    }

    pub fn ime_window(&self, handle: u32) -> Result<&RuntimeImeWindow, RuntimeStateError> {
        let id = self
            .ime_window_handle_index
            .get(&handle)
            .copied()
            .ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-user32",
                    "ime-window-handle-unknown",
                    format!("logical IME HWND {handle:#010x} is not live"),
                )
            })?;
        let window = self.ime_windows.get(&id).ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-user32",
                "ime-window-handle-index-dangling",
                "logical IME HWND index does not resolve to a live window",
            )
        })?;
        self.validate_ime_window_identity(window)?;
        Ok(window)
    }

    pub fn ime_windows(&self) -> &BTreeMap<RuntimeImeWindowId, RuntimeImeWindow> {
        &self.ime_windows
    }

    pub(super) fn plan_ime_control(
        &self,
        window_handle: u32,
        owner_thread_id: RuntimeThreadId,
        request: RuntimeImeStatusWindowRequest,
    ) -> Result<RuntimeImeControlPlan, RuntimeStateError> {
        let previous = self.ime_window(window_handle)?.clone();
        if previous.owner_thread_id != owner_thread_id {
            return Err(RuntimeStateError::conflict(
                "runtime-user32",
                "ime-window-message-owner-thread-mismatch",
                "synchronous IME window control requires the selected owner thread",
            ));
        }
        let disposition = if previous.status_window_request == Some(request) {
            RuntimeImeControlDisposition::Unchanged(previous)
        } else {
            let mut next = previous.clone();
            let next_generation = self.next_generation()?;
            next.status_window_request = Some(request);
            next.generation = next_generation;
            RuntimeImeControlDisposition::Update {
                previous,
                next,
                next_generation,
            }
        };
        Ok(RuntimeImeControlPlan {
            expected_generation: self.generation,
            disposition,
        })
    }

    pub(super) fn preflight_ime_control(
        &self,
        plan: &RuntimeImeControlPlan,
    ) -> Result<(), RuntimeStateError> {
        if self.generation != plan.expected_generation {
            return Err(RuntimeStateError::conflict(
                "runtime-user32",
                "ime-window-control-plan-stale",
                "User32 state changed after IME window control planning",
            ));
        }
        let previous = match &plan.disposition {
            RuntimeImeControlDisposition::Unchanged(previous)
            | RuntimeImeControlDisposition::Update { previous, .. } => previous,
        };
        if self.ime_windows.get(&previous.id) != Some(previous)
            || self.ime_window_handle_index.get(&previous.handle) != Some(&previous.id)
            || self.default_ime_windows.get(&previous.owner_thread_id) != Some(&previous.id)
        {
            return Err(RuntimeStateError::conflict(
                "runtime-user32",
                "ime-window-control-plan-stale",
                "logical IME window identity changed after control planning",
            ));
        }
        self.validate_ime_window_identity(previous)
    }

    pub(super) fn commit_ime_control(
        &mut self,
        plan: RuntimeImeControlPlan,
    ) -> Result<RuntimeImeWindow, RuntimeStateError> {
        self.preflight_ime_control(&plan)?;
        match plan.disposition {
            RuntimeImeControlDisposition::Unchanged(window) => Ok(window),
            RuntimeImeControlDisposition::Update {
                next,
                next_generation,
                ..
            } => {
                self.generation = next_generation;
                self.ime_windows.insert(next.id, next.clone());
                Ok(next)
            }
        }
    }

    pub(super) fn resolve_small_icon_window_procedure(
        &self,
        window_handle: u32,
        owner_thread_id: RuntimeThreadId,
        icon_handle: u32,
        icon_width: u16,
        icon_height: u16,
    ) -> Result<RuntimeWindowProcedure, RuntimeStateError> {
        let window = self.window(window_handle)?;
        if window.creation.owner_thread_id != owner_thread_id {
            return Err(RuntimeStateError::conflict(
                "runtime-user32",
                "window-message-owner-thread-mismatch",
                "synchronous logical window messages require the selected owner thread",
            ));
        }
        if window.lifecycle != RuntimeWindowLifecycle::Created {
            return Err(RuntimeStateError::conflict(
                "runtime-user32",
                "window-message-lifecycle-state-invalid",
                "synchronous logical window messages require a created window",
            ));
        }
        self.validate_window_creation_references(&window.creation)?;
        let class = self
            .window_classes
            .get(&window.creation.class.class_id)
            .ok_or_else(|| {
                RuntimeStateError::runtime_defect(
                    "runtime-user32",
                    "window-message-class-identity-missing",
                    "live logical window class identity disappeared during message resolution",
                )
            })?;
        validate_window_class_registration(&class.registration)?;
        self.validate_window_class_references(&class.registration)?;

        let icon = self.icon(icon_handle)?;
        if !icon.shared || icon.resource.width != icon_width || icon.resource.height != icon_height
        {
            return Err(RuntimeStateError::invalid_input(
                "runtime-user32",
                "window-message-icon-variant-invalid",
                "synchronous small-icon message requires a live shared icon of the controlled dimensions",
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
                RuntimeStateError::runtime_defect(
                    "runtime-user32",
                    "window-message-class-identity-missing",
                    "live logical window class identity disappeared during message resolution",
                )
            })?;
        validate_window_class_registration(&class.registration)?;
        self.validate_window_class_references(&class.registration)?;
        Ok(class.registration.window_procedure.clone())
    }

    pub(super) fn plan_window_small_icon(
        &self,
        window_handle: u32,
        owner_thread_id: RuntimeThreadId,
        icon_handle: u32,
        icon_width: u16,
        icon_height: u16,
    ) -> Result<RuntimeWindowSmallIconPlan, RuntimeStateError> {
        self.resolve_small_icon_window_procedure(
            window_handle,
            owner_thread_id,
            icon_handle,
            icon_width,
            icon_height,
        )?;
        let previous = self.window(window_handle)?.clone();
        self.validate_window_icon_reference(previous.small_icon)?;
        let icon = self.icon(icon_handle)?;
        let reference = RuntimeWindowIconReference {
            icon_id: icon.id,
            handle: icon.handle,
        };
        let previous_handle = previous.small_icon.map_or(0, |icon| icon.handle);
        let disposition = if previous.small_icon == Some(reference) {
            RuntimeWindowSmallIconDisposition::Unchanged {
                window: previous,
                previous_handle,
            }
        } else {
            let mut next = previous.clone();
            next.small_icon = Some(reference);
            RuntimeWindowSmallIconDisposition::Update {
                previous,
                next: Box::new(next),
                previous_handle,
                next_generation: self.next_generation()?,
            }
        };
        Ok(RuntimeWindowSmallIconPlan {
            expected_generation: self.generation,
            disposition,
        })
    }

    pub(super) fn preflight_window_small_icon(
        &self,
        plan: &RuntimeWindowSmallIconPlan,
    ) -> Result<(), RuntimeStateError> {
        if self.generation != plan.expected_generation {
            return Err(RuntimeStateError::conflict(
                "runtime-user32",
                "window-small-icon-plan-stale",
                "User32 state changed after logical window small-icon planning",
            ));
        }
        let (previous, next) = match &plan.disposition {
            RuntimeWindowSmallIconDisposition::Unchanged { window, .. } => (window, window),
            RuntimeWindowSmallIconDisposition::Update { previous, next, .. } => {
                (previous, next.as_ref())
            }
        };
        if self.windows.get(&previous.id) != Some(previous)
            || self.window_handle_index.get(&previous.handle) != Some(&previous.id)
        {
            return Err(RuntimeStateError::conflict(
                "runtime-user32",
                "window-small-icon-plan-stale",
                "logical window identity changed after small-icon planning",
            ));
        }
        self.validate_window_creation_references(&previous.creation)?;
        self.validate_window_icon_reference(previous.small_icon)?;
        self.validate_window_icon_reference(next.small_icon)
    }

    pub(super) fn commit_window_small_icon(
        &mut self,
        plan: RuntimeWindowSmallIconPlan,
    ) -> Result<u32, RuntimeStateError> {
        self.preflight_window_small_icon(&plan)?;
        match plan.disposition {
            RuntimeWindowSmallIconDisposition::Unchanged {
                previous_handle, ..
            } => Ok(previous_handle),
            RuntimeWindowSmallIconDisposition::Update {
                next,
                previous_handle,
                next_generation,
                ..
            } => {
                let next = *next;
                self.generation = next_generation;
                let replaced = self.windows.insert(next.id, next);
                debug_assert!(replaced.is_some());
                Ok(previous_handle)
            }
        }
    }

    pub(super) fn plan_window_style_update(
        &self,
        window_handle: u32,
        owner_thread_id: RuntimeThreadId,
        requested_style: u32,
        retained_style_mask: u32,
    ) -> Result<RuntimeWindowStyleUpdatePlan, RuntimeStateError> {
        let previous = self.window(window_handle)?.clone();
        self.validate_created_window_owner(&previous, owner_thread_id)?;
        self.validate_window_icon_reference(previous.small_icon)?;
        let effective_style =
            requested_style | (previous.creation.effective_style & retained_style_mask);
        let visible = effective_style & RUNTIME_WINDOW_STYLE_VISIBLE != 0;
        let disposition = if effective_style == previous.creation.effective_style
            && visible == previous.visible
        {
            RuntimeWindowStyleUpdateDisposition::Unchanged {
                previous_effective_style: previous.creation.effective_style,
                window: previous,
            }
        } else {
            let mut next = previous.clone();
            next.creation.effective_style = effective_style;
            next.visible = visible;
            RuntimeWindowStyleUpdateDisposition::Update {
                previous_effective_style: previous.creation.effective_style,
                previous,
                next: Box::new(next),
                next_generation: self.next_generation()?,
            }
        };
        Ok(RuntimeWindowStyleUpdatePlan {
            expected_generation: self.generation,
            disposition,
        })
    }

    pub(super) fn preflight_window_style_update(
        &self,
        plan: &RuntimeWindowStyleUpdatePlan,
    ) -> Result<(), RuntimeStateError> {
        if self.generation != plan.expected_generation {
            return Err(RuntimeStateError::conflict(
                "runtime-user32",
                "window-style-plan-stale",
                "User32 state changed after logical window style planning",
            ));
        }
        let (previous, next) = match &plan.disposition {
            RuntimeWindowStyleUpdateDisposition::Unchanged { window, .. } => (window, window),
            RuntimeWindowStyleUpdateDisposition::Update { previous, next, .. } => {
                (previous, next.as_ref())
            }
        };
        if self.windows.get(&previous.id) != Some(previous)
            || self.window_handle_index.get(&previous.handle) != Some(&previous.id)
            || (next.creation.effective_style & RUNTIME_WINDOW_STYLE_VISIBLE != 0) != next.visible
        {
            return Err(RuntimeStateError::conflict(
                "runtime-user32",
                "window-style-plan-stale",
                "logical window identity or style changed after style planning",
            ));
        }
        self.validate_window_creation_references(&previous.creation)?;
        self.validate_window_creation_references(&next.creation)?;
        self.validate_window_icon_reference(previous.small_icon)?;
        self.validate_window_icon_reference(next.small_icon)
    }

    pub(super) fn commit_window_style_update(
        &mut self,
        plan: RuntimeWindowStyleUpdatePlan,
    ) -> Result<RuntimeSetWindowStyleResult, RuntimeStateError> {
        self.preflight_window_style_update(&plan)?;
        match plan.disposition {
            RuntimeWindowStyleUpdateDisposition::Unchanged {
                window,
                previous_effective_style,
            } => Ok(RuntimeSetWindowStyleResult {
                previous_effective_style,
                effective_style: window.creation.effective_style,
                visible: window.visible,
                changed: false,
            }),
            RuntimeWindowStyleUpdateDisposition::Update {
                next,
                previous_effective_style,
                next_generation,
                ..
            } => {
                let next = *next;
                let result = RuntimeSetWindowStyleResult {
                    previous_effective_style,
                    effective_style: next.creation.effective_style,
                    visible: next.visible,
                    changed: true,
                };
                self.generation = next_generation;
                let replaced = self.windows.insert(next.id, next);
                debug_assert!(replaced.is_some());
                Ok(result)
            }
        }
    }

    pub(super) fn plan_window_file_drop_acceptance(
        &self,
        window_handle: u32,
        accepts_files: bool,
    ) -> Result<RuntimeWindowFileDropAcceptancePlan, RuntimeStateError> {
        let previous = self.window(window_handle)?.clone();
        if previous.lifecycle != RuntimeWindowLifecycle::Created {
            return Err(RuntimeStateError::conflict(
                "runtime-user32",
                "window-file-drop-lifecycle-state-invalid",
                "logical file-drop acceptance requires a created window",
            ));
        }
        self.validate_window_creation_references(&previous.creation)?;
        self.validate_window_icon_reference(previous.small_icon)?;
        let previous_accepts_files = previous.creation.effective_extended_style
            & RUNTIME_WINDOW_EXTENDED_STYLE_ACCEPT_FILES
            != 0;
        let disposition = if previous_accepts_files == accepts_files {
            RuntimeWindowFileDropAcceptanceDisposition::Unchanged {
                window: previous,
                accepts_files,
            }
        } else {
            let mut next = previous.clone();
            if accepts_files {
                next.creation.effective_extended_style |=
                    RUNTIME_WINDOW_EXTENDED_STYLE_ACCEPT_FILES;
            } else {
                next.creation.effective_extended_style &=
                    !RUNTIME_WINDOW_EXTENDED_STYLE_ACCEPT_FILES;
            }
            RuntimeWindowFileDropAcceptanceDisposition::Update {
                previous,
                next: Box::new(next),
                previous_accepts_files,
                next_generation: self.next_generation()?,
            }
        };
        Ok(RuntimeWindowFileDropAcceptancePlan {
            expected_generation: self.generation,
            disposition,
        })
    }

    pub(super) fn preflight_window_file_drop_acceptance(
        &self,
        plan: &RuntimeWindowFileDropAcceptancePlan,
    ) -> Result<(), RuntimeStateError> {
        if self.generation != plan.expected_generation {
            return Err(RuntimeStateError::conflict(
                "runtime-user32",
                "window-file-drop-plan-stale",
                "User32 state changed after logical file-drop acceptance planning",
            ));
        }
        let (previous, next, accepts_files) = match &plan.disposition {
            RuntimeWindowFileDropAcceptanceDisposition::Unchanged {
                window,
                accepts_files,
            } => (window, window, *accepts_files),
            RuntimeWindowFileDropAcceptanceDisposition::Update { previous, next, .. } => (
                previous,
                next.as_ref(),
                next.creation.effective_extended_style & RUNTIME_WINDOW_EXTENDED_STYLE_ACCEPT_FILES
                    != 0,
            ),
        };
        if self.windows.get(&previous.id) != Some(previous)
            || self.window_handle_index.get(&previous.handle) != Some(&previous.id)
            || next.lifecycle != RuntimeWindowLifecycle::Created
            || (next.creation.effective_extended_style & RUNTIME_WINDOW_EXTENDED_STYLE_ACCEPT_FILES
                != 0)
                != accepts_files
        {
            return Err(RuntimeStateError::conflict(
                "runtime-user32",
                "window-file-drop-plan-stale",
                "logical window identity or file-drop acceptance changed after planning",
            ));
        }
        validate_window_creation_request(&next.creation)?;
        self.validate_window_creation_references(&previous.creation)?;
        self.validate_window_creation_references(&next.creation)?;
        self.validate_window_icon_reference(previous.small_icon)?;
        self.validate_window_icon_reference(next.small_icon)
    }

    pub(super) fn commit_window_file_drop_acceptance(
        &mut self,
        plan: RuntimeWindowFileDropAcceptancePlan,
    ) -> Result<RuntimeWindowFileDropAcceptanceResult, RuntimeStateError> {
        self.preflight_window_file_drop_acceptance(&plan)?;
        match plan.disposition {
            RuntimeWindowFileDropAcceptanceDisposition::Unchanged { accepts_files, .. } => {
                Ok(RuntimeWindowFileDropAcceptanceResult {
                    previous_accepts_files: accepts_files,
                    accepts_files,
                    changed: false,
                })
            }
            RuntimeWindowFileDropAcceptanceDisposition::Update {
                next,
                previous_accepts_files,
                next_generation,
                ..
            } => {
                let next = *next;
                let accepts_files = next.creation.effective_extended_style
                    & RUNTIME_WINDOW_EXTENDED_STYLE_ACCEPT_FILES
                    != 0;
                let result = RuntimeWindowFileDropAcceptanceResult {
                    previous_accepts_files,
                    accepts_files,
                    changed: true,
                };
                self.generation = next_generation;
                let replaced = self.windows.insert(next.id, next);
                debug_assert!(replaced.is_some());
                Ok(result)
            }
        }
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
        {
            return Err(RuntimeStateError::invalid_input(
                "runtime-user32",
                "set-window-pos-callback-flags-invalid",
                "logical window positioning requires nonzero callback flags",
            ));
        }
        let mut next_creation = window.creation.clone();
        next_creation.geometry = request.next_geometry;
        validate_window_creation_request(&next_creation)?;
        validate_position_geometry_transition(
            window.creation.geometry,
            request.next_geometry,
            request.variant,
        )?;
        let small_icon_handle = window.small_icon.map_or(0, |icon| icon.handle);
        let next_effective_style = if request.next_visible {
            window.creation.effective_style | RUNTIME_WINDOW_STYLE_VISIBLE
        } else {
            window.creation.effective_style & !RUNTIME_WINDOW_STYLE_VISIBLE
        };
        match request.variant {
            RuntimeSetWindowPosVariant::InitialShow
                if window.visible
                    || window.topmost
                    || !request.next_visible
                    || request.next_topmost
                    || request.next_geometry == window.creation.geometry
                    || small_icon_handle == 0 =>
            {
                return Err(RuntimeStateError::conflict(
                    "runtime-user32",
                    "set-window-pos-show-state-invalid",
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
                return Err(RuntimeStateError::conflict(
                    "runtime-user32",
                    "set-window-pos-hide-state-invalid",
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
                return Err(RuntimeStateError::conflict(
                    "runtime-user32",
                    "set-window-pos-activation-state-invalid",
                    "same-geometry activation requires one visible inactive owner-thread window and no imported foreground identity",
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
                return Err(RuntimeStateError::conflict(
                    "runtime-user32",
                    "set-window-pos-resize-state-invalid",
                    "logical resize requires one visible active, focused, foreground owner-thread window and changed bounded geometry",
                ));
            }
            _ => {}
        }
        Ok(RuntimeSetWindowPosOperation {
            window_id: window.id,
            window_handle: window.handle,
            expected_generation: self.generation,
            variant: request.variant,
            previous_geometry: window.creation.geometry,
            next_geometry: request.next_geometry,
            previous_visible: window.visible,
            next_visible: request.next_visible,
            previous_topmost: window.topmost,
            next_topmost: request.next_topmost,
            previous_effective_style: window.creation.effective_style,
            next_effective_style,
            callback_flags: request.callback_flags,
            callback_insert_after: request.callback_insert_after,
            activation_insert_after: request.activation_insert_after,
            activation_internal_flags: request.activation_internal_flags,
            resize_nccalc_window_pos_flags: request.resize_nccalc_window_pos_flags,
            resize_changed_flags: request.resize_changed_flags,
            small_icon_handle,
        })
    }

    pub(super) fn plan_set_window_pos_callback(
        &self,
        operation: RuntimeSetWindowPosOperation,
        message: RuntimeSetWindowPosCallbackMessage,
        result: u32,
    ) -> Result<RuntimeSetWindowPosCallbackPlan, RuntimeStateError> {
        let previous = self
            .validate_set_window_pos_operation(operation, message)?
            .clone();
        let disposition = match (operation.variant, message) {
            (
                RuntimeSetWindowPosVariant::InitialShow,
                RuntimeSetWindowPosCallbackMessage::WindowPositionChanged,
            ) => {
                if result != 0 {
                    return Err(RuntimeStateError::invalid_input(
                        "runtime-user32",
                        "set-window-pos-window-procedure-result-invalid",
                        "initial window-position callback must return the observed zero result",
                    ));
                }
                RuntimeSetWindowPosCallbackDisposition::Validate {
                    window: previous.clone(),
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
                    return Err(RuntimeStateError::invalid_input(
                        "runtime-user32",
                        "set-window-pos-small-icon-result-mismatch",
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
                    return Err(RuntimeStateError::invalid_input(
                        "runtime-user32",
                        "set-window-pos-window-procedure-result-invalid",
                        "initial hide callback must return the observed zero result",
                    ));
                }
                self.window_position_update_disposition(previous, operation)?
            }
            (RuntimeSetWindowPosVariant::InitialHide, _) => {
                return Err(RuntimeStateError::invalid_input(
                    "runtime-user32",
                    "set-window-pos-callback-order-mismatch",
                    "initial hide received an out-of-order logical window callback",
                ));
            }
            (RuntimeSetWindowPosVariant::InitialShow, _) => {
                return Err(RuntimeStateError::invalid_input(
                    "runtime-user32",
                    "set-window-pos-callback-order-mismatch",
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
                    previous: previous.clone(),
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
                    window: previous.clone(),
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
                    window: previous.clone(),
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
                require_set_window_pos_callback_result(result, 1, "WM_NCACTIVATE")?;
                RuntimeSetWindowPosCallbackDisposition::Validate {
                    window: previous.clone(),
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
                    window: previous.clone(),
                    action: RuntimeSetWindowPosCallbackAction::Completed { window: previous },
                }
            }
            (RuntimeSetWindowPosVariant::ActivateSameGeometry, _) => {
                return Err(RuntimeStateError::invalid_input(
                    "runtime-user32",
                    "set-window-pos-callback-order-mismatch",
                    "same-geometry activation received an out-of-order logical window callback",
                ));
            }
            (
                RuntimeSetWindowPosVariant::Resize,
                RuntimeSetWindowPosCallbackMessage::WindowPositionChanging,
            ) => {
                require_set_window_pos_callback_result(result, 0, "resize WINDOWPOS")?;
                RuntimeSetWindowPosCallbackDisposition::Validate {
                    window: previous.clone(),
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
                    window: previous.clone(),
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
                    window: previous.clone(),
                    action: RuntimeSetWindowPosCallbackAction::Completed { window: previous },
                }
            }
            (RuntimeSetWindowPosVariant::Resize, _) => {
                return Err(RuntimeStateError::invalid_input(
                    "runtime-user32",
                    "set-window-pos-callback-order-mismatch",
                    "logical resize received an out-of-order window callback",
                ));
            }
        };
        Ok(RuntimeSetWindowPosCallbackPlan {
            expected_generation: self.generation,
            disposition,
        })
    }

    pub(super) fn set_window_pos_callback_action<'a>(
        &self,
        plan: &'a RuntimeSetWindowPosCallbackPlan,
    ) -> &'a RuntimeSetWindowPosCallbackAction {
        match &plan.disposition {
            RuntimeSetWindowPosCallbackDisposition::Validate { action, .. }
            | RuntimeSetWindowPosCallbackDisposition::Update { action, .. }
            | RuntimeSetWindowPosCallbackDisposition::Activate { action, .. } => action,
        }
    }

    pub(super) fn preflight_set_window_pos_callback(
        &self,
        plan: &RuntimeSetWindowPosCallbackPlan,
    ) -> Result<(), RuntimeStateError> {
        if self.generation != plan.expected_generation {
            return Err(RuntimeStateError::conflict(
                "runtime-user32",
                "set-window-pos-callback-plan-stale",
                "User32 state changed after window-position callback planning",
            ));
        }
        let previous = match &plan.disposition {
            RuntimeSetWindowPosCallbackDisposition::Validate { window, .. } => window,
            RuntimeSetWindowPosCallbackDisposition::Update { previous, .. } => previous,
            RuntimeSetWindowPosCallbackDisposition::Activate { previous, .. } => previous,
        };
        if self.windows.get(&previous.id) != Some(previous)
            || self.window_handle_index.get(&previous.handle) != Some(&previous.id)
        {
            return Err(RuntimeStateError::conflict(
                "runtime-user32",
                "set-window-pos-callback-plan-stale",
                "logical window state changed after position callback planning",
            ));
        }
        self.validate_window_creation_references(&previous.creation)?;
        self.validate_window_icon_reference(previous.small_icon)?;
        if matches!(
            &plan.disposition,
            RuntimeSetWindowPosCallbackDisposition::Activate { .. }
        ) && (self
            .active_windows
            .contains_key(&previous.creation.owner_thread_id)
            || self
                .focused_windows
                .contains_key(&previous.creation.owner_thread_id)
            || self.foreground_window.is_some())
        {
            return Err(RuntimeStateError::conflict(
                "runtime-user32",
                "set-window-pos-callback-plan-stale",
                "logical activation relations changed after callback planning",
            ));
        }
        Ok(())
    }

    pub(super) fn commit_set_window_pos_callback(
        &mut self,
        plan: RuntimeSetWindowPosCallbackPlan,
    ) -> Result<RuntimeSetWindowPosCallbackAction, RuntimeStateError> {
        self.preflight_set_window_pos_callback(&plan)?;
        Ok(self.commit_preflighted_set_window_pos_callback(plan))
    }

    pub(super) fn commit_preflighted_set_window_pos_callback(
        &mut self,
        plan: RuntimeSetWindowPosCallbackPlan,
    ) -> RuntimeSetWindowPosCallbackAction {
        match plan.disposition {
            RuntimeSetWindowPosCallbackDisposition::Validate { action, .. } => action,
            RuntimeSetWindowPosCallbackDisposition::Update {
                next,
                action,
                next_generation,
                ..
            } => {
                let next = *next;
                self.generation = next_generation;
                let replaced = self.windows.insert(next.id, next);
                debug_assert!(replaced.is_some());
                action
            }
            RuntimeSetWindowPosCallbackDisposition::Activate {
                previous,
                action,
                next_generation,
                ..
            } => {
                let replaced = self
                    .active_windows
                    .insert(previous.creation.owner_thread_id, previous.id);
                debug_assert!(replaced.is_none());
                debug_assert!(
                    !self
                        .focused_windows
                        .contains_key(&previous.creation.owner_thread_id)
                );
                self.foreground_window = Some(previous.id);
                self.generation = next_generation;
                action
            }
        }
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
            previous,
            next: Box::new(next),
            next_generation: self.next_generation()?,
        })
    }

    fn window_resize_update_disposition(
        &self,
        previous: RuntimeWindow,
        operation: RuntimeSetWindowPosOperation,
    ) -> Result<RuntimeSetWindowPosCallbackDisposition, RuntimeStateError> {
        let next_generation = self.next_generation()?;
        let mut next = previous.clone();
        next.creation.geometry = operation.next_geometry;
        let mut next_operation = operation;
        next_operation.expected_generation = next_generation;
        Ok(RuntimeSetWindowPosCallbackDisposition::Update {
            action: RuntimeSetWindowPosCallbackAction::Continue {
                window: next.clone(),
                operation: next_operation,
                next_message: RuntimeSetWindowPosCallbackMessage::Size,
            },
            previous,
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
            return Err(RuntimeStateError::conflict(
                "runtime-user32",
                "set-window-pos-operation-stale",
                "User32 state changed after window-position callback preparation",
            ));
        }
        let window = self.window_by_id(operation.window_id)?;
        let expected_next_effective_style = if operation.next_visible {
            operation.previous_effective_style | RUNTIME_WINDOW_STYLE_VISIBLE
        } else {
            operation.previous_effective_style & !RUNTIME_WINDOW_STYLE_VISIBLE
        };
        let expected_geometry = if operation.variant == RuntimeSetWindowPosVariant::Resize
            && matches!(
                message,
                RuntimeSetWindowPosCallbackMessage::Size
                    | RuntimeSetWindowPosCallbackMessage::WindowPositionChanged
            ) {
            operation.next_geometry
        } else {
            operation.previous_geometry
        };
        if window.handle != operation.window_handle
            || window.creation.geometry != expected_geometry
            || window.visible != operation.previous_visible
            || window.topmost != operation.previous_topmost
            || window.creation.effective_style != operation.previous_effective_style
            || operation.next_effective_style != expected_next_effective_style
            || window.small_icon.map_or(0, |icon| icon.handle) != operation.small_icon_handle
        {
            return Err(RuntimeStateError::conflict(
                "runtime-user32",
                "set-window-pos-operation-stale",
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
                return Err(RuntimeStateError::conflict(
                    "runtime-user32",
                    "set-window-pos-activation-state-stale",
                    "logical active, focused, or foreground relation changed during window positioning",
                ));
            }
        }
        if operation.variant == RuntimeSetWindowPosVariant::Resize {
            let owner_thread_id = window.creation.owner_thread_id;
            let target = Some(window.id);
            if self.active_windows.get(&owner_thread_id).copied() != target
                || self.focused_windows.get(&owner_thread_id).copied() != target
                || self.foreground_window != target
            {
                return Err(RuntimeStateError::conflict(
                    "runtime-user32",
                    "set-window-pos-resize-state-stale",
                    "logical active, focused, or foreground relation changed during resize",
                ));
            }
        }
        Ok(window)
    }

    pub(super) fn plan_set_window_pos_focus(
        &self,
        window_handle: u32,
        owner_thread_id: RuntimeThreadId,
    ) -> Result<RuntimeSetWindowPosFocusPlan, RuntimeStateError> {
        let window = self.window(window_handle)?.clone();
        self.validate_created_window_owner(&window, owner_thread_id)?;
        if self.active_windows.get(&owner_thread_id).copied() != Some(window.id)
            || self.focused_windows.contains_key(&owner_thread_id)
            || self.foreground_window != Some(window.id)
        {
            return Err(RuntimeStateError::conflict(
                "runtime-user32",
                "set-window-pos-focus-state-invalid",
                "activation default processing requires the active foreground window without logical focus",
            ));
        }
        let next_generation = self.next_generation()?;
        Ok(RuntimeSetWindowPosFocusPlan {
            expected_generation: self.generation,
            previous_focused_window: None,
            next_focused_window: window.id,
            operation: RuntimeSetWindowPosDefaultOperation {
                window_id: window.id,
                window_handle: window.handle,
                owner_thread_id,
                expected_generation: next_generation,
            },
            window,
            next_generation,
        })
    }

    pub(super) fn set_window_pos_focus_operation(
        &self,
        plan: &RuntimeSetWindowPosFocusPlan,
    ) -> RuntimeSetWindowPosDefaultOperation {
        plan.operation
    }

    pub(super) fn preflight_set_window_pos_focus(
        &self,
        plan: &RuntimeSetWindowPosFocusPlan,
    ) -> Result<(), RuntimeStateError> {
        if self.generation != plan.expected_generation
            || self.windows.get(&plan.window.id) != Some(&plan.window)
            || self
                .active_windows
                .get(&plan.operation.owner_thread_id)
                .copied()
                != Some(plan.window.id)
            || self
                .focused_windows
                .get(&plan.operation.owner_thread_id)
                .copied()
                != plan.previous_focused_window
            || self.foreground_window != Some(plan.window.id)
        {
            return Err(RuntimeStateError::conflict(
                "runtime-user32",
                "set-window-pos-focus-plan-stale",
                "logical activation state changed after focus planning",
            ));
        }
        Ok(())
    }

    pub(super) fn commit_set_window_pos_focus(
        &mut self,
        plan: RuntimeSetWindowPosFocusPlan,
    ) -> Result<RuntimeSetWindowPosDefaultOperation, RuntimeStateError> {
        self.preflight_set_window_pos_focus(&plan)?;
        self.focused_windows
            .insert(plan.operation.owner_thread_id, plan.next_focused_window);
        self.generation = plan.next_generation;
        Ok(plan.operation)
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
            return Err(RuntimeStateError::conflict(
                "runtime-user32",
                "set-window-pos-default-operation-stale",
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
            return Err(RuntimeStateError::conflict(
                "runtime-user32",
                "set-window-pos-default-operation-stale",
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
            return Err(RuntimeStateError::conflict(
                "runtime-user32",
                "window-message-owner-thread-mismatch",
                "synchronous logical window messages require the selected owner thread",
            ));
        }
        if window.lifecycle != RuntimeWindowLifecycle::Created {
            return Err(RuntimeStateError::conflict(
                "runtime-user32",
                "window-message-lifecycle-state-invalid",
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
            || window.namespace != LOGICAL_WINDOW_HANDLE_NAMESPACE
            || window.class_name != "IME"
            || self.ime_window_handle_index.get(&window.handle) != Some(&window.id)
            || self.window_handle_index.contains_key(&window.handle)
        {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-user32",
                "ime-window-record-inconsistent",
                "logical default IME window does not match its namespace or indexes",
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
            return Err(RuntimeStateError::runtime_defect(
                "runtime-user32",
                "window-icon-reference-mismatch",
                "logical window icon handle and icon identity do not match",
            ));
        }
        Ok(())
    }

    pub fn window_by_id(&self, id: RuntimeWindowId) -> Result<&RuntimeWindow, RuntimeStateError> {
        let window = self.windows.get(&id).ok_or_else(|| {
            RuntimeStateError::not_found(
                "runtime-user32",
                "window-id-unknown",
                "logical window ID is not live",
            )
        })?;
        if self.window_handle_index.get(&window.handle) != Some(&id) {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-user32",
                "window-handle-index-mismatch",
                "logical window ID does not match its HWND index",
            ));
        }
        Ok(window)
    }

    pub fn windows(&self) -> &BTreeMap<RuntimeWindowId, RuntimeWindow> {
        &self.windows
    }

    pub(super) fn is_empty(&self) -> bool {
        self.icons.is_empty()
            && self.cursors.is_empty()
            && self.window_classes.is_empty()
            && self.windows.is_empty()
            && self.ime_windows.is_empty()
            && self.default_ime_windows.is_empty()
            && self.active_windows.is_empty()
            && self.focused_windows.is_empty()
            && self.foreground_window.is_none()
    }

    fn next_generation(&self) -> Result<u64, RuntimeStateError> {
        self.generation.checked_add(1).ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-user32",
                "user32-generation-exhausted",
                "User32 resource generation is exhausted",
            )
        })
    }
}

fn require_set_window_pos_callback_result(
    actual: u32,
    expected: u32,
    callback: &'static str,
) -> Result<(), RuntimeStateError> {
    if actual != expected {
        return Err(RuntimeStateError::invalid_input(
            "runtime-user32",
            "set-window-pos-window-procedure-result-invalid",
            format!("{callback} must return the observed value {expected:#010x}"),
        ));
    }
    Ok(())
}

fn validate_icon_resource(resource: &RuntimeIconResource) -> Result<(), RuntimeStateError> {
    if resource.module_id.0 == 0
        || resource.module_load_base.0 == 0
        || resource.module_source_sha256.len() != 64
        || !resource
            .module_source_sha256
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
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
        return Err(RuntimeStateError::invalid_input(
            "runtime-user32",
            "icon-resource-invalid",
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
        RuntimeCursorSource::SystemPredefined { .. } => Err(RuntimeStateError::invalid_input(
            "runtime-user32",
            "system-cursor-source-invalid",
            "logical system cursor source does not match its predefined cursor identity",
        )),
        RuntimeCursorSource::ModuleResource { resource } => validate_cursor_resource(resource),
    }
}

fn validate_cursor_resource(resource: &RuntimeCursorResource) -> Result<(), RuntimeStateError> {
    if resource.module_id.0 == 0
        || resource.module_load_base.0 == 0
        || resource.module_source_sha256.len() != 64
        || !resource
            .module_source_sha256
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
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
        return Err(RuntimeStateError::invalid_input(
            "runtime-user32",
            "cursor-resource-invalid",
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
        || module.source_sha256.len() != 64
        || !module
            .source_sha256
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        || window_procedure.module_id != module.module_id
        || window_procedure.address.0 == 0
        || window_procedure.rva.0 == 0
        || expected_window_procedure_address != Some(window_procedure.address)
        || window_procedure.calling_convention != RuntimeWindowProcedureCallingConvention::Stdcall
        || window_procedure.argument_bytes != RUNTIME_WINDOW_PROCEDURE_ARGUMENT_BYTES
    {
        return Err(RuntimeStateError::invalid_input(
            "runtime-user32",
            "window-class-module-or-procedure-invalid",
            "window class requires a mapped module identity and same-module x86 stdcall WndProc descriptor",
        ));
    }
    validate_window_class_name(&registration.class_name, false)?;
    if let Some(menu_name) = registration.menu_name.as_ref() {
        validate_window_class_name(menu_name, true)?;
    }
    Ok(())
}

fn validate_window_class_name(
    name: &RuntimeWindowClassNameA,
    allow_empty: bool,
) -> Result<(), RuntimeStateError> {
    if (!allow_empty && name.value.is_empty())
        || name.value.len() > RUNTIME_WINDOW_CLASS_NAME_A_LIMIT
        || !name.value.is_ascii()
        || name.value.as_bytes().contains(&0)
        || name.encoding != RUNTIME_WINDOW_CLASS_NAME_A_ENCODING
    {
        return Err(RuntimeStateError::invalid_input(
            "runtime-user32",
            "window-class-name-invalid",
            "window class and menu names require bounded copied Windows-932 ASCII text",
        ));
    }
    Ok(())
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
    };
    if !valid {
        return Err(RuntimeStateError::invalid_input(
            "runtime-user32",
            "set-window-pos-geometry-transition-invalid",
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
        || request.name.value.len() > RUNTIME_WINDOW_NAME_A_LIMIT
        || !request.name.value.is_ascii()
        || request.name.value.as_bytes().contains(&0)
        || request.name.encoding != RUNTIME_WINDOW_NAME_A_ENCODING
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
        return Err(RuntimeStateError::invalid_input(
            "runtime-user32",
            "window-creation-request-invalid",
            "logical window creation requires bounded copied text, a live class/module relation, NULL unowned relations, and contained client geometry",
        ));
    }
    Ok(())
}

fn window_class_key(
    registration: &RuntimeWindowClassRegistration,
) -> Result<RuntimeWindowClassKey, RuntimeStateError> {
    window_class_key_from_name(
        registration.module.module_id,
        &registration.class_name.value,
    )
}

fn window_class_key_from_name(
    module_id: RuntimeModuleId,
    name: &str,
) -> Result<RuntimeWindowClassKey, RuntimeStateError> {
    if module_id.0 == 0
        || name.is_empty()
        || name.len() > RUNTIME_WINDOW_CLASS_NAME_A_LIMIT
        || !name.is_ascii()
        || name.as_bytes().contains(&0)
    {
        return Err(RuntimeStateError::invalid_input(
            "runtime-user32",
            "window-class-key-invalid",
            "window class lookup requires a live module and bounded nonempty ASCII name",
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
    fn shared_icon_plans_reuse_identity_and_teardown_atomically() {
        let mut user32 = RuntimeUser32::default();
        let first_resource = resource(101, 3, 0x3000);
        let first_plan = user32
            .plan_shared_icon_load(first_resource.clone())
            .unwrap();
        assert!(user32.icons().is_empty());
        let first = user32.commit_shared_icon_load(first_plan).unwrap();
        assert_eq!(first.icon_id, RuntimeIconId(1));
        assert_eq!(first.handle, RUNTIME_ICON_HANDLE_BASE);
        assert!(!first.already_loaded);

        let repeated_plan = user32
            .plan_shared_icon_load(first_resource)
            .expect("repeated icon plan");
        let repeated = user32.commit_shared_icon_load(repeated_plan).unwrap();
        assert_eq!(repeated.icon_id, first.icon_id);
        assert_eq!(repeated.handle, first.handle);
        assert!(repeated.already_loaded);

        let stale = user32
            .plan_shared_icon_load(resource(102, 4, 0x4000))
            .unwrap();
        let intervening_plan = user32
            .plan_shared_icon_load(resource(103, 5, 0x5000))
            .unwrap();
        user32.commit_shared_icon_load(intervening_plan).unwrap();
        let before_stale = user32.clone();
        assert_eq!(
            user32.commit_shared_icon_load(stale).unwrap_err().code,
            "icon-load-plan-stale"
        );
        assert_eq!(user32, before_stale);

        let stale_teardown = user32.plan_process_teardown().unwrap().unwrap();
        let later_plan = user32
            .plan_shared_icon_load(resource(104, 6, 0x6000))
            .unwrap();
        user32.commit_shared_icon_load(later_plan).unwrap();
        assert_eq!(
            user32
                .preflight_process_teardown(&stale_teardown)
                .unwrap_err()
                .code,
            "user32-teardown-plan-stale"
        );
        let teardown = user32.plan_process_teardown().unwrap().unwrap();
        user32.preflight_process_teardown(&teardown).unwrap();
        user32.commit_process_teardown(teardown);
        assert!(user32.icons().is_empty());
        assert!(user32.resource_index.is_empty());
        assert!(user32.handle_index.is_empty());
    }

    #[test]
    fn shared_icon_capacity_failure_preserves_identity_and_allocators() {
        let mut user32 = RuntimeUser32::default();
        for ordinal in 1..=RUNTIME_SHARED_ICON_LIMIT {
            let ordinal = u32::try_from(ordinal).unwrap();
            let plan = user32
                .plan_shared_icon_load(resource(ordinal, ordinal, 0x3000 + ordinal))
                .unwrap();
            let result = user32.commit_shared_icon_load(plan).unwrap();
            assert_eq!(result.icon_id.0, u64::from(ordinal));
        }
        let before = user32.clone();
        let error = user32
            .plan_shared_icon_load(resource(4_097, 4_097, 0x5000))
            .expect_err("shared icon capacity must be bounded");
        assert_eq!(
            error.class,
            super::super::RuntimeStateErrorClass::ResourceLimit
        );
        assert_eq!(error.code, "shared-icon-limit-exceeded");
        assert_eq!(user32, before);
    }

    #[test]
    fn shared_cursor_identity_and_cross_resource_plans_are_atomic() {
        let mut user32 = RuntimeUser32::default();
        let stale_cursor = user32.plan_shared_cursor_load(arrow_cursor()).unwrap();
        let icon_plan = user32
            .plan_shared_icon_load(resource(101, 3, 0x3000))
            .unwrap();
        user32.commit_shared_icon_load(icon_plan).unwrap();
        let before_stale = user32.clone();
        assert_eq!(
            user32
                .commit_shared_cursor_load(stale_cursor)
                .unwrap_err()
                .code,
            "cursor-load-plan-stale"
        );
        assert_eq!(user32, before_stale);

        let first_plan = user32.plan_shared_cursor_load(arrow_cursor()).unwrap();
        let first = user32.commit_shared_cursor_load(first_plan).unwrap();
        assert_eq!(first.cursor_id, RuntimeCursorId(1));
        assert_eq!(first.handle, RUNTIME_CURSOR_HANDLE_BASE);
        assert!(!first.already_loaded);
        let repeated_plan = user32.plan_shared_cursor_load(arrow_cursor()).unwrap();
        let repeated = user32.commit_shared_cursor_load(repeated_plan).unwrap();
        assert_eq!(repeated.cursor_id, first.cursor_id);
        assert_eq!(repeated.handle, first.handle);
        assert!(repeated.already_loaded);
        assert_eq!(user32.cursors().len(), 1);

        let module_plan = user32
            .plan_shared_cursor_load(module_cursor(106, 5, 0x6000))
            .unwrap();
        let module = user32.commit_shared_cursor_load(module_plan).unwrap();
        assert_eq!(module.cursor_id, RuntimeCursorId(2));
        assert_eq!(module.handle, RUNTIME_CURSOR_HANDLE_BASE + 1);
        assert!(!module.already_loaded);
        let repeated_module = user32
            .commit_shared_cursor_load(
                user32
                    .plan_shared_cursor_load(module_cursor(106, 5, 0x6000))
                    .unwrap(),
            )
            .unwrap();
        assert_eq!(repeated_module.cursor_id, module.cursor_id);
        assert_eq!(repeated_module.handle, module.handle);
        assert!(repeated_module.already_loaded);
        assert_eq!(user32.cursors().len(), 2);

        let invalid = RuntimeCursorSource::SystemPredefined {
            resource_id: 0x7f01,
            cursor: RuntimeSystemCursorKind::Arrow,
        };
        let before_invalid = user32.clone();
        assert_eq!(
            user32.plan_shared_cursor_load(invalid).unwrap_err().code,
            "system-cursor-source-invalid"
        );
        assert_eq!(user32, before_invalid);

        let teardown = user32.plan_process_teardown().unwrap().unwrap();
        user32.preflight_process_teardown(&teardown).unwrap();
        user32.commit_process_teardown(teardown);
        assert!(user32.icons().is_empty());
        assert!(user32.cursors().is_empty());
        assert!(user32.cursor_source_index.is_empty());
        assert!(user32.cursor_handle_index.is_empty());
    }

    #[test]
    fn shared_cursor_capacity_failure_preserves_identity_and_allocators() {
        let mut user32 = RuntimeUser32::default();
        for ordinal in 1..=RUNTIME_SHARED_CURSOR_LIMIT {
            let ordinal = u32::try_from(ordinal).unwrap();
            let source = module_cursor(ordinal, ordinal, 0x4000 + ordinal * 0x1000);
            let plan = user32.plan_shared_cursor_load(source).unwrap();
            let result = user32.commit_shared_cursor_load(plan).unwrap();
            assert_eq!(result.cursor_id.0, u64::from(ordinal));
        }
        let before = user32.clone();
        let error = user32
            .plan_shared_cursor_load(module_cursor(257, 257, 0x0010_5000))
            .expect_err("shared cursor capacity must be bounded");
        assert_eq!(
            error.class,
            super::super::RuntimeStateErrorClass::ResourceLimit
        );
        assert_eq!(error.code, "shared-cursor-limit-exceeded");
        assert_eq!(user32, before);
    }

    #[test]
    fn window_class_registration_is_module_scoped_case_folded_and_failure_atomic() {
        let mut user32 = RuntimeUser32::default();
        let icon_plan = user32
            .plan_shared_icon_load(resource(101, 3, 0x3000))
            .unwrap();
        let icon = user32.commit_shared_icon_load(icon_plan).unwrap();
        let cursor_plan = user32.plan_shared_cursor_load(arrow_cursor()).unwrap();
        let cursor = user32.commit_shared_cursor_load(cursor_plan).unwrap();

        let main_registration =
            window_class_registration(icon, cursor, "BGI - Main window", 0x28, 0x92550);
        let main_plan = user32
            .plan_window_class_registration(main_registration.clone())
            .unwrap();
        assert!(user32.window_classes().is_empty());
        let main = user32.commit_window_class_registration(main_plan).unwrap();
        assert_eq!(main.class_id, RuntimeWindowClassId(1));
        assert_eq!(main.atom, RUNTIME_WINDOW_CLASS_ATOM_BASE);
        assert!(!main.already_registered);
        assert_eq!(
            user32.window_class(main.atom).unwrap().registration,
            main_registration
        );
        assert_eq!(
            user32
                .window_class_by_name(RuntimeModuleId(1), "bgi - MAIN WINDOW")
                .unwrap()
                .id,
            main.class_id
        );

        let mut duplicate_registration = main_registration.clone();
        duplicate_registration.class_name.value = "bgi - main WINDOW".to_owned();
        let generation_before_duplicate = user32.generation;
        let duplicate_plan = user32
            .plan_window_class_registration(duplicate_registration)
            .unwrap();
        let duplicate = user32
            .commit_window_class_registration(duplicate_plan)
            .unwrap();
        assert_eq!(duplicate.class_id, main.class_id);
        assert_eq!(duplicate.atom, main.atom);
        assert!(duplicate.already_registered);
        assert_eq!(user32.generation, generation_before_duplicate);
        assert_eq!(user32.window_classes().len(), 1);

        let child_plan = user32
            .plan_window_class_registration(window_class_registration(
                icon,
                cursor,
                "BGI - Child window",
                0x3020,
                0x2e6e0,
            ))
            .unwrap();
        let child = user32.commit_window_class_registration(child_plan).unwrap();
        assert_eq!(child.class_id, RuntimeWindowClassId(2));
        assert_eq!(child.atom, RUNTIME_WINDOW_CLASS_ATOM_BASE + 1);

        let stale = user32
            .plan_window_class_registration(window_class_registration(
                icon,
                cursor,
                "stale class",
                0x28,
                0x92550,
            ))
            .unwrap();
        let intervening_icon = user32
            .plan_shared_icon_load(resource(102, 4, 0x4000))
            .unwrap();
        user32.commit_shared_icon_load(intervening_icon).unwrap();
        let before_stale = user32.clone();
        assert_eq!(
            user32
                .commit_window_class_registration(stale)
                .unwrap_err()
                .code,
            "window-class-registration-plan-stale"
        );
        assert_eq!(user32, before_stale);

        let mut bad_reference =
            window_class_registration(icon, cursor, "bad reference", 0x28, 0x92550);
        bad_reference.icon.as_mut().unwrap().handle ^= 1;
        let before_bad_reference = user32.clone();
        assert_eq!(
            user32
                .plan_window_class_registration(bad_reference)
                .unwrap_err()
                .code,
            "window-class-icon-mismatch"
        );
        assert_eq!(user32, before_bad_reference);

        let teardown = user32.plan_process_teardown().unwrap().unwrap();
        user32.preflight_process_teardown(&teardown).unwrap();
        user32.commit_process_teardown(teardown);
        assert!(user32.is_empty());
        assert!(user32.window_class_name_index.is_empty());
        assert!(user32.window_class_atom_index.is_empty());
        assert_eq!(
            user32
                .window_class(main.atom)
                .expect_err("process teardown retires class ATOM")
                .code,
            "window-class-atom-unknown"
        );
    }

    #[test]
    fn invalid_window_class_metadata_and_atom_exhaustion_do_not_consume_identity() {
        let mut user32 = RuntimeUser32::default();
        let icon_plan = user32
            .plan_shared_icon_load(resource(101, 3, 0x3000))
            .unwrap();
        let icon = user32.commit_shared_icon_load(icon_plan).unwrap();
        let cursor_plan = user32.plan_shared_cursor_load(arrow_cursor()).unwrap();
        let cursor = user32.commit_shared_cursor_load(cursor_plan).unwrap();
        let valid = window_class_registration(icon, cursor, "valid class", 0x28, 0x92550);

        for (invalid, expected_code) in [
            {
                let mut invalid = valid.clone();
                invalid.module.source_sha256 = "A".repeat(64);
                (invalid, "window-class-module-or-procedure-invalid")
            },
            {
                let mut invalid = valid.clone();
                invalid.window_procedure.address.0 += 1;
                (invalid, "window-class-module-or-procedure-invalid")
            },
            {
                let mut invalid = valid.clone();
                invalid.class_name.value = "class-\u{3042}".to_owned();
                (invalid, "window-class-name-invalid")
            },
        ] {
            let before = user32.clone();
            let error = user32.plan_window_class_registration(invalid).unwrap_err();
            assert_eq!(error.code, expected_code);
            assert_eq!(user32, before);
        }

        user32.next_window_class_atom = u32::from(u16::MAX) + 1;
        let before_exhaustion = user32.clone();
        let error = user32
            .plan_window_class_registration(valid)
            .expect_err("logical ATOM space must be bounded");
        assert_eq!(error.code, "window-class-atom-space-exhausted");
        assert_eq!(user32, before_exhaustion);
    }

    #[test]
    fn logical_window_creation_callbacks_are_ordered_and_abort_without_live_identity_leaks() {
        let mut user32 = RuntimeUser32::default();
        let icon_plan = user32
            .plan_shared_icon_load(resource(101, 3, 0x3000))
            .unwrap();
        let icon = user32.commit_shared_icon_load(icon_plan).unwrap();
        let cursor_plan = user32.plan_shared_cursor_load(arrow_cursor()).unwrap();
        let cursor = user32.commit_shared_cursor_load(cursor_plan).unwrap();
        let class_plan = user32
            .plan_window_class_registration(window_class_registration(
                icon,
                cursor,
                "BGI - Main window",
                0x28,
                0x92550,
            ))
            .unwrap();
        let class = user32.commit_window_class_registration(class_plan).unwrap();

        let creation = window_creation_request(class);
        let creation_plan = user32.plan_window_creation(creation.clone()).unwrap();
        let preview = user32.window_creation_result(&creation_plan);
        assert_eq!(preview.window_id, RuntimeWindowId(1));
        assert_eq!(preview.handle, RUNTIME_WINDOW_HANDLE_BASE);
        assert!(
            user32.windows().is_empty(),
            "planning must not publish HWND state"
        );
        let created = user32.commit_window_creation(creation_plan).unwrap();
        assert_eq!(created, preview);
        assert_eq!(
            user32.window(created.handle).unwrap().lifecycle,
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
            let plan = user32
                .plan_window_creation_callback(created.window_id, message, result)
                .unwrap();
            let preview = user32.window_creation_callback_action(&plan).clone();
            let action = user32.commit_window_creation_callback(plan).unwrap();
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
            user32.window(created.handle).unwrap().lifecycle,
            RuntimeWindowLifecycle::Created
        );

        let rejected_plan = user32.plan_window_creation(creation.clone()).unwrap();
        let rejected = user32.commit_window_creation(rejected_plan).unwrap();
        assert_eq!(rejected.window_id, RuntimeWindowId(2));
        assert_eq!(rejected.handle, RUNTIME_WINDOW_HANDLE_BASE + 1);
        let abort_plan = user32
            .plan_window_creation_callback(
                rejected.window_id,
                RuntimeWindowCreationMessage::NonClientCreate,
                0,
            )
            .unwrap();
        assert!(matches!(
            user32.commit_window_creation_callback(abort_plan).unwrap(),
            RuntimeWindowCreationCallbackAction::Aborted { .. }
        ));
        assert_eq!(
            user32.window(rejected.handle).unwrap_err().code,
            "window-handle-unknown"
        );
        assert_eq!(user32.windows().len(), 1);

        let create_rejected_plan = user32.plan_window_creation(creation).unwrap();
        let create_rejected = user32.commit_window_creation(create_rejected_plan).unwrap();
        let first = user32
            .plan_window_creation_callback(
                create_rejected.window_id,
                RuntimeWindowCreationMessage::NonClientCreate,
                1,
            )
            .unwrap();
        user32.commit_window_creation_callback(first).unwrap();
        let second = user32
            .plan_window_creation_callback(
                create_rejected.window_id,
                RuntimeWindowCreationMessage::NonClientCalculate,
                0,
            )
            .unwrap();
        user32.commit_window_creation_callback(second).unwrap();
        let create_abort = user32
            .plan_window_creation_callback(
                create_rejected.window_id,
                RuntimeWindowCreationMessage::Create,
                u32::MAX,
            )
            .unwrap();
        user32
            .commit_window_creation_callback(create_abort)
            .unwrap();
        assert_eq!(
            user32.window(create_rejected.handle).unwrap_err().code,
            "window-handle-unknown"
        );
        assert_eq!(user32.windows().len(), 1);
    }

    #[test]
    fn small_icon_and_window_position_plans_are_atomic_and_reject_stale_commits() {
        let (mut user32, window_handle, small_icon_handle) = created_window_with_small_icon();
        let initial = user32.window(window_handle).unwrap().clone();
        let repeated_icon = user32
            .plan_window_small_icon(
                window_handle,
                RuntimeThreadId(0x2000),
                small_icon_handle,
                16,
                16,
            )
            .unwrap();
        let before_repeated = user32.clone();
        assert_eq!(
            user32.commit_window_small_icon(repeated_icon).unwrap(),
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
                    callback_flags: 0x1000_0d59,
                    callback_insert_after: 0xffff_fffe,
                    activation_insert_after: 0,
                    activation_internal_flags: 3,
                    resize_nccalc_window_pos_flags: 0x190e,
                    resize_changed_flags: 0x110e,
                },
            )
            .unwrap();
        let first_callback = user32
            .plan_set_window_pos_callback(
                show,
                RuntimeSetWindowPosCallbackMessage::WindowPositionChanged,
                0,
            )
            .unwrap();
        assert!(matches!(
            user32.set_window_pos_callback_action(&first_callback),
            RuntimeSetWindowPosCallbackAction::Continue {
                next_message: RuntimeSetWindowPosCallbackMessage::GetSmallIcon,
                ..
            }
        ));
        let before_first_callback = user32.clone();
        user32
            .commit_set_window_pos_callback(first_callback)
            .unwrap();
        assert_eq!(
            user32, before_first_callback,
            "the first synchronous callback must not publish geometry or visibility"
        );
        let final_callback = user32
            .plan_set_window_pos_callback(
                show,
                RuntimeSetWindowPosCallbackMessage::GetSmallIcon,
                small_icon_handle,
            )
            .unwrap();
        assert_eq!(user32.window(window_handle).unwrap(), &initial);
        let shown = user32
            .commit_set_window_pos_callback(final_callback)
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
                    callback_flags: 0x0000_1d9f,
                    callback_insert_after: 0xffff_fffe,
                    activation_insert_after: 0,
                    activation_internal_flags: 3,
                    resize_nccalc_window_pos_flags: 0x190e,
                    resize_changed_flags: 0x110e,
                },
            )
            .unwrap();
        let stale_hide = user32
            .plan_set_window_pos_callback(
                hide,
                RuntimeSetWindowPosCallbackMessage::WindowPositionChanged,
                0,
            )
            .unwrap();
        let mut intervening_resource = resource(102, 5, 0x5000);
        intervening_resource.width = 16;
        intervening_resource.height = 16;
        intervening_resource.icon_data_size = 1_128;
        let intervening = user32.plan_shared_icon_load(intervening_resource).unwrap();
        user32.commit_shared_icon_load(intervening).unwrap();
        let before_stale = user32.clone();
        assert_eq!(
            user32
                .commit_set_window_pos_callback(stale_hide)
                .unwrap_err()
                .code,
            "set-window-pos-callback-plan-stale"
        );
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
                    callback_flags: 0x0000_1d9f,
                    callback_insert_after: 0xffff_fffe,
                    activation_insert_after: 0,
                    activation_internal_flags: 3,
                    resize_nccalc_window_pos_flags: 0x190e,
                    resize_changed_flags: 0x110e,
                },
            )
            .unwrap();
        let hide_plan = user32
            .plan_set_window_pos_callback(
                fresh_hide,
                RuntimeSetWindowPosCallbackMessage::WindowPositionChanged,
                0,
            )
            .unwrap();
        let duplicate = hide_plan.clone();
        let hidden = user32.commit_set_window_pos_callback(hide_plan).unwrap();
        assert!(matches!(
            hidden,
            RuntimeSetWindowPosCallbackAction::Completed { ref window }
                if !window.visible
                    && window.creation.geometry == show_geometry
                    && window.creation.effective_style == 0x84ca_0000
        ));
        let before_duplicate = user32.clone();
        assert_eq!(
            user32
                .commit_set_window_pos_callback(duplicate)
                .unwrap_err()
                .code,
            "set-window-pos-callback-plan-stale"
        );
        assert_eq!(user32, before_duplicate);

        let style_plan = user32
            .plan_window_style_update(
                window_handle,
                RuntimeThreadId(0x2000),
                0x90ca_0000,
                0x0400_0000,
            )
            .unwrap();
        let stale_style = style_plan.clone();
        let style = user32.commit_window_style_update(style_plan).unwrap();
        assert_eq!(style.previous_effective_style, 0x84ca_0000);
        assert_eq!(style.effective_style, 0x94ca_0000);
        assert!(style.visible);
        assert!(style.changed);
        assert_eq!(
            user32
                .commit_window_style_update(stale_style)
                .unwrap_err()
                .code,
            "window-style-plan-stale"
        );

        let repeated = user32
            .plan_window_style_update(
                window_handle,
                RuntimeThreadId(0x2000),
                0x90ca_0000,
                0x0400_0000,
            )
            .unwrap();
        let before_repeated = user32.clone();
        assert_eq!(
            user32.commit_window_style_update(repeated).unwrap(),
            RuntimeSetWindowStyleResult {
                previous_effective_style: 0x94ca_0000,
                effective_style: 0x94ca_0000,
                visible: true,
                changed: false,
            }
        );
        assert_eq!(user32, before_repeated);

        let before_unknown = user32.clone();
        assert_eq!(
            user32
                .plan_window_style_update(
                    window_handle + 1,
                    RuntimeThreadId(0x2000),
                    0x90ca_0000,
                    0x0400_0000,
                )
                .unwrap_err()
                .code,
            "window-handle-unknown"
        );
        assert_eq!(user32, before_unknown);
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

        let enable = user32
            .plan_window_file_drop_acceptance(window_handle, true)
            .unwrap();
        let duplicate = enable.clone();
        assert_eq!(
            user32.commit_window_file_drop_acceptance(enable).unwrap(),
            RuntimeWindowFileDropAcceptanceResult {
                previous_accepts_files: false,
                accepts_files: true,
                changed: true,
            }
        );
        assert_ne!(
            user32
                .window(window_handle)
                .unwrap()
                .creation
                .effective_extended_style
                & RUNTIME_WINDOW_EXTENDED_STYLE_ACCEPT_FILES,
            0
        );
        let before_duplicate = user32.clone();
        assert_eq!(
            user32
                .commit_window_file_drop_acceptance(duplicate)
                .unwrap_err()
                .code,
            "window-file-drop-plan-stale"
        );
        assert_eq!(user32, before_duplicate);

        let repeated = user32
            .plan_window_file_drop_acceptance(window_handle, true)
            .unwrap();
        let before_repeated = user32.clone();
        assert_eq!(
            user32.commit_window_file_drop_acceptance(repeated).unwrap(),
            RuntimeWindowFileDropAcceptanceResult {
                previous_accepts_files: true,
                accepts_files: true,
                changed: false,
            }
        );
        assert_eq!(user32, before_repeated);

        let disable = user32
            .plan_window_file_drop_acceptance(window_handle, false)
            .unwrap();
        assert_eq!(
            user32.commit_window_file_drop_acceptance(disable).unwrap(),
            RuntimeWindowFileDropAcceptanceResult {
                previous_accepts_files: true,
                accepts_files: false,
                changed: true,
            }
        );
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
        assert_eq!(
            user32
                .plan_window_file_drop_acceptance(window_handle + 2, true)
                .unwrap_err()
                .code,
            "window-handle-unknown"
        );
        assert_eq!(user32, before_unknown);
    }

    fn resource(
        group_resource_id: u32,
        icon_resource_id: u32,
        icon_data_rva: u32,
    ) -> RuntimeIconResource {
        RuntimeIconResource {
            module_id: RuntimeModuleId(1),
            module_load_base: GuestAddress(0x1000_0000),
            module_source_sha256: "0".repeat(64),
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
        let large_icon = user32
            .commit_shared_icon_load(
                user32
                    .plan_shared_icon_load(resource(101, 3, 0x3000))
                    .unwrap(),
            )
            .unwrap();
        let mut small_resource = resource(101, 4, 0x4000);
        small_resource.width = 16;
        small_resource.height = 16;
        small_resource.icon_data_size = 1_128;
        let small_icon = user32
            .commit_shared_icon_load(user32.plan_shared_icon_load(small_resource).unwrap())
            .unwrap();
        let cursor = user32
            .commit_shared_cursor_load(user32.plan_shared_cursor_load(arrow_cursor()).unwrap())
            .unwrap();
        let class = user32
            .commit_window_class_registration(
                user32
                    .plan_window_class_registration(window_class_registration(
                        large_icon,
                        cursor,
                        "BGI - Main window",
                        0x28,
                        0x92550,
                    ))
                    .unwrap(),
            )
            .unwrap();
        let created = user32
            .commit_window_creation(
                user32
                    .plan_window_creation(window_creation_request(class))
                    .unwrap(),
            )
            .unwrap();
        for (message, result) in [
            (RuntimeWindowCreationMessage::NonClientCreate, 1),
            (RuntimeWindowCreationMessage::NonClientCalculate, 0),
            (RuntimeWindowCreationMessage::Create, 0),
            (RuntimeWindowCreationMessage::Size, 0),
            (RuntimeWindowCreationMessage::Move, 0),
        ] {
            let plan = user32
                .plan_window_creation_callback(created.window_id, message, result)
                .unwrap();
            user32.commit_window_creation_callback(plan).unwrap();
        }
        let icon_plan = user32
            .plan_window_small_icon(
                created.handle,
                RuntimeThreadId(0x2000),
                small_icon.handle,
                16,
                16,
            )
            .unwrap();
        assert_eq!(user32.commit_window_small_icon(icon_plan).unwrap(), 0);
        (user32, created.handle, small_icon.handle)
    }

    #[test]
    fn default_ime_window_uses_shared_hwnd_authority_and_tracks_status_requests() {
        let (mut user32, app_window_handle, _) = created_window_with_small_icon();
        let owner_thread_id = user32
            .window(app_window_handle)
            .unwrap()
            .creation
            .owner_thread_id;
        let plan = user32
            .plan_default_ime_window(app_window_handle, owner_thread_id)
            .unwrap();
        user32.preflight_default_ime_window(&plan).unwrap();
        let result = user32.commit_default_ime_window_preflighted(plan);
        assert_eq!(result.handle, app_window_handle + 1);
        assert!(!result.already_created);
        let ime = user32.ime_window(result.handle).unwrap();
        assert_eq!(ime.class_name, "IME");
        assert_eq!(ime.owner_thread_id, owner_thread_id);
        assert_eq!(ime.status_window_request, None);

        let repeated = user32
            .plan_default_ime_window(app_window_handle, owner_thread_id)
            .unwrap();
        let repeated = user32.commit_default_ime_window_preflighted(repeated);
        assert_eq!(repeated.handle, result.handle);
        assert!(repeated.already_created);

        let close = user32
            .plan_ime_control(
                result.handle,
                owner_thread_id,
                RuntimeImeStatusWindowRequest::Close,
            )
            .unwrap();
        user32.commit_ime_control(close).unwrap();
        assert_eq!(
            user32
                .ime_window(result.handle)
                .unwrap()
                .status_window_request,
            Some(RuntimeImeStatusWindowRequest::Close)
        );
        let open = user32
            .plan_ime_control(
                result.handle,
                owner_thread_id,
                RuntimeImeStatusWindowRequest::Open,
            )
            .unwrap();
        user32.commit_ime_control(open).unwrap();
        assert_eq!(
            user32
                .ime_window(result.handle)
                .unwrap()
                .status_window_request,
            Some(RuntimeImeStatusWindowRequest::Open)
        );

        let teardown = user32.plan_process_teardown().unwrap().unwrap();
        user32.preflight_process_teardown(&teardown).unwrap();
        user32.commit_process_teardown(teardown);
        assert!(user32.is_empty());
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
                module_source_sha256: "0".repeat(64),
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
        icon: RuntimeLoadIconAResult,
        cursor: RuntimeLoadCursorAResult,
        name: &str,
        style: u32,
        window_procedure_rva: u32,
    ) -> RuntimeWindowClassRegistration {
        RuntimeWindowClassRegistration {
            module: RuntimeWindowClassModule {
                module_id: RuntimeModuleId(1),
                load_base: GuestAddress(0x1000_0000),
                source_sha256: "0".repeat(64),
            },
            style,
            window_procedure: RuntimeWindowProcedure {
                module_id: RuntimeModuleId(1),
                address: GuestAddress(0x1000_0000 + window_procedure_rva),
                rva: Rva(window_procedure_rva),
                calling_convention: RuntimeWindowProcedureCallingConvention::Stdcall,
                argument_bytes: RUNTIME_WINDOW_PROCEDURE_ARGUMENT_BYTES,
            },
            class_extra_bytes: 0,
            window_extra_bytes: 0,
            icon: Some(RuntimeWindowClassIconReference {
                icon_id: icon.icon_id,
                handle: icon.handle,
            }),
            cursor: Some(RuntimeWindowClassCursorReference {
                cursor_id: cursor.cursor_id,
                handle: cursor.handle,
            }),
            background_brush: 0,
            menu_name: None,
            class_name: RuntimeWindowClassNameA {
                value: name.to_owned(),
                encoding: RUNTIME_WINDOW_CLASS_NAME_A_ENCODING.to_owned(),
            },
            small_icon: None,
        }
    }

    fn window_creation_request(
        class: RuntimeRegisterClassExAResult,
    ) -> RuntimeWindowCreationRequest {
        RuntimeWindowCreationRequest {
            owner_thread_id: RuntimeThreadId(0x2000),
            class: RuntimeWindowClassReference {
                class_id: class.class_id,
                atom: class.atom,
                module_id: RuntimeModuleId(1),
            },
            instance: RuntimeWindowClassModule {
                module_id: RuntimeModuleId(1),
                load_base: GuestAddress(0x1000_0000),
                source_sha256: "0".repeat(64),
            },
            name: RuntimeWindowNameA {
                value: "Ethornell - BURIKO General Interpreter".to_owned(),
                encoding: RUNTIME_WINDOW_NAME_A_ENCODING.to_owned(),
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

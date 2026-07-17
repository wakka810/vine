use std::collections::BTreeMap;

use serde::Serialize;

use crate::memory::GuestAddress;
use crate::module_graph::RuntimeModuleId;
use crate::pe32::Rva;

use super::RuntimeStateError;

pub const LOGICAL_ICON_HANDLE_NAMESPACE: &str = "vine.logical-icon.v1";
pub const LOGICAL_CURSOR_HANDLE_NAMESPACE: &str = "vine.logical-cursor.v1";
pub const LOGICAL_WINDOW_CLASS_ATOM_NAMESPACE: &str = "vine.logical-window-class-atom.v1";
pub const RUNTIME_SHARED_ICON_LIMIT: usize = 4_096;
pub const RUNTIME_SHARED_CURSOR_LIMIT: usize = 256;
pub const RUNTIME_WINDOW_CLASS_LIMIT: usize = 4_096;
pub const RUNTIME_WINDOW_CLASS_NAME_A_LIMIT: usize = 256;
pub const RUNTIME_WINDOW_CLASS_NAME_A_ENCODING: &str = "windows-932-v1";
pub const RUNTIME_ICON_HANDLE_BASE: u32 = 0x4901_0001;
pub const RUNTIME_CURSOR_HANDLE_BASE: u32 = 0x4301_0001;
pub const RUNTIME_SYSTEM_CURSOR_ARROW_RESOURCE_ID: u32 = 0x7f00;
pub const RUNTIME_WINDOW_CLASS_ATOM_BASE: u16 = 0xc000;
pub const RUNTIME_WINDOW_PROCEDURE_ARGUMENT_BYTES: u32 = 16;

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
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum RuntimeCursorSource {
    SystemPredefined {
        #[serde(rename = "resourceId")]
        resource_id: u32,
        cursor: RuntimeSystemCursorKind,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RuntimeUser32TeardownPlan {
    expected_generation: u64,
    next_generation: u64,
    retired_icons: usize,
    retired_cursors: usize,
    retired_window_classes: usize,
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
    next_icon_id: u64,
    next_icon_handle: u32,
    next_cursor_id: u64,
    next_cursor_handle: u32,
    next_window_class_id: u64,
    next_window_class_atom: u32,
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
            next_icon_id: 1,
            next_icon_handle: RUNTIME_ICON_HANDLE_BASE,
            next_cursor_id: 1,
            next_cursor_handle: RUNTIME_CURSOR_HANDLE_BASE,
            next_window_class_id: 1,
            next_window_class_atom: u32::from(RUNTIME_WINDOW_CLASS_ATOM_BASE),
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

    pub(super) fn plan_shared_system_cursor_load(
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

    pub(super) fn preflight_shared_system_cursor_load(
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

    pub(super) fn commit_shared_system_cursor_load(
        &mut self,
        plan: RuntimeCursorLoadPlan,
    ) -> Result<RuntimeLoadCursorAResult, RuntimeStateError> {
        self.preflight_shared_system_cursor_load(&plan)?;
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
        if self.icons.is_empty() && self.cursors.is_empty() && self.window_classes.is_empty() {
            return Ok(None);
        }
        Ok(Some(RuntimeUser32TeardownPlan {
            expected_generation: self.generation,
            next_generation: self.next_generation()?,
            retired_icons: self.icons.len(),
            retired_cursors: self.cursors.len(),
            retired_window_classes: self.window_classes.len(),
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

    pub(super) fn is_empty(&self) -> bool {
        self.icons.is_empty() && self.cursors.is_empty() && self.window_classes.is_empty()
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
    }
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
    fn shared_system_cursor_identity_and_cross_resource_plans_are_atomic() {
        let mut user32 = RuntimeUser32::default();
        let stale_cursor = user32
            .plan_shared_system_cursor_load(arrow_cursor())
            .unwrap();
        let icon_plan = user32
            .plan_shared_icon_load(resource(101, 3, 0x3000))
            .unwrap();
        user32.commit_shared_icon_load(icon_plan).unwrap();
        let before_stale = user32.clone();
        assert_eq!(
            user32
                .commit_shared_system_cursor_load(stale_cursor)
                .unwrap_err()
                .code,
            "cursor-load-plan-stale"
        );
        assert_eq!(user32, before_stale);

        let first_plan = user32
            .plan_shared_system_cursor_load(arrow_cursor())
            .unwrap();
        let first = user32.commit_shared_system_cursor_load(first_plan).unwrap();
        assert_eq!(first.cursor_id, RuntimeCursorId(1));
        assert_eq!(first.handle, RUNTIME_CURSOR_HANDLE_BASE);
        assert!(!first.already_loaded);
        let repeated_plan = user32
            .plan_shared_system_cursor_load(arrow_cursor())
            .unwrap();
        let repeated = user32
            .commit_shared_system_cursor_load(repeated_plan)
            .unwrap();
        assert_eq!(repeated.cursor_id, first.cursor_id);
        assert_eq!(repeated.handle, first.handle);
        assert!(repeated.already_loaded);
        assert_eq!(user32.cursors().len(), 1);

        let invalid = RuntimeCursorSource::SystemPredefined {
            resource_id: 0x7f01,
            cursor: RuntimeSystemCursorKind::Arrow,
        };
        let before_invalid = user32.clone();
        assert_eq!(
            user32
                .plan_shared_system_cursor_load(invalid)
                .unwrap_err()
                .code,
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
    fn window_class_registration_is_module_scoped_case_folded_and_failure_atomic() {
        let mut user32 = RuntimeUser32::default();
        let icon_plan = user32
            .plan_shared_icon_load(resource(101, 3, 0x3000))
            .unwrap();
        let icon = user32.commit_shared_icon_load(icon_plan).unwrap();
        let cursor_plan = user32
            .plan_shared_system_cursor_load(arrow_cursor())
            .unwrap();
        let cursor = user32
            .commit_shared_system_cursor_load(cursor_plan)
            .unwrap();

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
        let cursor_plan = user32
            .plan_shared_system_cursor_load(arrow_cursor())
            .unwrap();
        let cursor = user32
            .commit_shared_system_cursor_load(cursor_plan)
            .unwrap();
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

    fn arrow_cursor() -> RuntimeCursorSource {
        RuntimeCursorSource::SystemPredefined {
            resource_id: RUNTIME_SYSTEM_CURSOR_ARROW_RESOURCE_ID,
            cursor: RuntimeSystemCursorKind::Arrow,
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
}

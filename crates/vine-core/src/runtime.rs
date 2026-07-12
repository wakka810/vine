use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;

use serde::Serialize;

use crate::callbacks::{CALLBACK_RETURN_ARENA_BASE, CALLBACK_RETURN_ARENA_SIZE};
use crate::dynamic_exports::{
    DYNAMIC_EXPORT_ARENA_BASE, DYNAMIC_EXPORT_ARENA_SIZE, DYNAMIC_EXPORT_ENDPOINT_NAMESPACE,
    DYNAMIC_EXPORT_SLOT_SIZE, DynamicExportThunk,
};
use crate::imports::{
    IMPORT_THUNK_ARENA_BASE, IMPORT_THUNK_ARENA_SIZE, ImportThunkArena, ImportThunkId,
    RuntimeExport,
};
use crate::materialize::ImageMaterialization;
use crate::memory::{
    GUEST_PAGE_SIZE, GuestAddress, GuestMemory, GuestMemoryError, GuestMemoryErrorClass,
    GuestRange, MemoryInvalidation, MemoryOwnerId,
};
use crate::module_graph::{
    RuntimeModuleClass, RuntimeModuleGraph, RuntimeModuleGraphError, RuntimeModuleGraphErrorClass,
    RuntimeModuleId, RuntimeModuleLoadState, RuntimeModuleRegistration,
};
use crate::module_images::{
    RuntimeModuleImage, RuntimeModuleImageError, RuntimeModuleImageErrorClass,
    RuntimeModuleImageRegistry, RuntimeResolvedExport,
};
use crate::pe32::{ExportLookup, ImportTarget, LoaderIngressReport};
use crate::snapshot::ModuleIdentity;

pub const LOGICAL_PRIVATE_HEAP_NAMESPACE: &str = "vine.logical-private-heap.v1";
pub const LOGICAL_LOADED_MODULE_NAMESPACE: &str = "vine.logical-loaded-module.v1";
pub const LOGICAL_TLS_INDEX_NAMESPACE: &str = "vine.logical-tls-index.v1";
pub const LOGICAL_FLS_INDEX_NAMESPACE: &str = "vine.logical-fls-index.v1";
pub const LOGICAL_ENCODED_POINTER_NAMESPACE: &str = "vine.logical-encoded-pointer.v1";
pub const RUNTIME_PROCESS_OBJECT_NAMESPACE: &str = "vine.runtime-process-object.v1";
pub const RUNTIME_THREAD_OBJECT_NAMESPACE: &str = "vine.runtime-thread-object.v1";
pub const RUNTIME_PRIVATE_HEAP_ARENA_BASE: GuestAddress = GuestAddress(0x2000_0000);
pub const RUNTIME_PRIVATE_HEAP_ARENA_STRIDE: u32 = 0x0100_0000;
pub const RUNTIME_PRIVATE_HEAP_BACKING_LIMIT: u16 = 32;
pub const RUNTIME_PRIVATE_HEAP_ALIGNMENT: u32 = 8;
pub const RUNTIME_PRIVATE_HEAP_UNINITIALIZED_BYTE: u8 = 0xa5;
pub const RUNTIME_PRIVATE_HEAP_FREED_BYTE: u8 = 0xdd;
pub const RUNTIME_PRIVATE_HEAP_MAX_LIVE_ALLOCATIONS: usize = 65_536;
pub const TLS_OUT_OF_INDEXES: u32 = u32::MAX;
pub const FLS_OUT_OF_INDEXES: u32 = u32::MAX;
pub const RUNTIME_FLS_INDEX_LIMIT: u32 = 128;
pub const X86_STARTUP_INFO_W_BYTE_LENGTH: usize = 68;
pub const X86_CRITICAL_SECTION_BYTE_LENGTH: usize = 24;
pub const RUNTIME_CRITICAL_SECTION_LIMIT: usize = 4096;

const LOGICAL_PRIVATE_HEAP_HANDLE_PREFIX: u32 = 0x6801_0000;
const RUNTIME_PRIVATE_HEAP_OWNER: &str = "runtime-process-private-heaps";
const RUNTIME_PRIVATE_HEAP_VARIANT: &str = "heap-create-metadata-v1";
const MAX_RUNTIME_THREAD_MESSAGES: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct RuntimeProcessId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct RuntimeThreadId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeObjectKind {
    Process,
    Thread,
    PrivateHeap,
    Module,
}

impl RuntimeObjectKind {
    const fn handle_tag(self) -> u8 {
        match self {
            Self::Process => 0x50,
            Self::Thread => 0x54,
            Self::PrivateHeap => 0x68,
            Self::Module => 0x4d,
        }
    }

    fn from_handle_tag(tag: u8) -> Option<Self> {
        match tag {
            0x50 => Some(Self::Process),
            0x54 => Some(Self::Thread),
            0x68 => Some(Self::PrivateHeap),
            0x4d => Some(Self::Module),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct RuntimeHandle(pub u32);

impl RuntimeHandle {
    pub fn from_parts(
        kind: RuntimeObjectKind,
        generation: u8,
        slot: u16,
    ) -> Result<Self, RuntimeStateError> {
        if generation == 0 || slot == 0 {
            return Err(RuntimeStateError::invalid_input(
                "runtime-object",
                "runtime-handle-parts-invalid",
                "runtime handles require nonzero generation and slot fields",
            ));
        }
        Ok(Self(
            (u32::from(kind.handle_tag()) << 24) | (u32::from(generation) << 16) | u32::from(slot),
        ))
    }

    pub fn kind(self) -> Result<RuntimeObjectKind, RuntimeStateError> {
        let tag = (self.0 >> 24) as u8;
        RuntimeObjectKind::from_handle_tag(tag).ok_or_else(|| {
            RuntimeStateError::invalid_input(
                "runtime-object",
                "runtime-handle-type-unknown",
                format!(
                    "runtime handle {:#010x} has unknown type tag {tag:#04x}",
                    self.0
                ),
            )
        })
    }

    pub const fn generation(self) -> u8 {
        ((self.0 >> 16) & 0xff) as u8
    }

    pub const fn slot(self) -> u16 {
        (self.0 & 0xffff) as u16
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum RuntimeObjectKey {
    Process { process_id: u32 },
    Thread { thread_id: u32 },
    PrivateHeap { handle: u32 },
    Module { handle: u32 },
}

impl RuntimeObjectKey {
    pub const fn kind(&self) -> RuntimeObjectKind {
        match self {
            Self::Process { .. } => RuntimeObjectKind::Process,
            Self::Thread { .. } => RuntimeObjectKind::Thread,
            Self::PrivateHeap { .. } => RuntimeObjectKind::PrivateHeap,
            Self::Module { .. } => RuntimeObjectKind::Module,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct RuntimeObjectId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeHandleClosePolicy {
    CloseHandle,
    SubsystemOwned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum RuntimeWaitState {
    Nonsignaled,
    Signaled { exit_code: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct RuntimeWaitRegistrationId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeWaitRegistration {
    pub id: RuntimeWaitRegistrationId,
    pub thread_id: RuntimeThreadId,
    pub object_id: RuntimeObjectId,
    pub handle: RuntimeHandle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeWaitCompletion {
    pub registration: RuntimeWaitRegistration,
    pub wait_state: RuntimeWaitState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "outcome", rename_all = "kebab-case")]
pub enum RuntimeWaitDisposition {
    AlreadySignaled {
        object_id: RuntimeObjectId,
        wait_state: RuntimeWaitState,
    },
    Pending {
        registration: RuntimeWaitRegistration,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct RuntimeMessageId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeThreadMessage {
    pub id: RuntimeMessageId,
    pub source_thread_id: RuntimeThreadId,
    pub target_thread_id: RuntimeThreadId,
    pub hwnd: u32,
    pub message: u32,
    pub w_param: u32,
    pub l_param: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeMessageWake {
    pub thread_id: RuntimeThreadId,
    pub message_id: RuntimeMessageId,
    pub queue_depth: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "outcome", rename_all = "kebab-case")]
pub enum RuntimeMessageReceiveDisposition {
    Message { message: RuntimeThreadMessage },
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeObjectRecord {
    pub id: RuntimeObjectId,
    pub key: RuntimeObjectKey,
    pub kind: RuntimeObjectKind,
    pub namespace: String,
    pub close_policy: RuntimeHandleClosePolicy,
    pub wait_state: Option<RuntimeWaitState>,
    pub open_handles: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeHandleEntry {
    pub handle: RuntimeHandle,
    pub object_id: RuntimeObjectId,
    pub kind: RuntimeObjectKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeHandleSlot {
    generation: u8,
    live: bool,
    retired: bool,
}

struct RuntimeObjectRetirementPlan {
    id: RuntimeObjectId,
    key: RuntimeObjectKey,
    handles: Vec<RuntimeHandleEntry>,
    next_slots: Vec<(u32, RuntimeHandleSlot)>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeObjectTable {
    objects: BTreeMap<RuntimeObjectId, RuntimeObjectRecord>,
    handles: BTreeMap<u32, RuntimeHandleEntry>,
    slots: BTreeMap<u32, RuntimeHandleSlot>,
    #[serde(skip)]
    identity_index: BTreeMap<RuntimeObjectKey, RuntimeObjectId>,
    next_object_id: u64,
}

impl Default for RuntimeObjectTable {
    fn default() -> Self {
        Self {
            objects: BTreeMap::new(),
            handles: BTreeMap::new(),
            slots: BTreeMap::new(),
            identity_index: BTreeMap::new(),
            next_object_id: 1,
        }
    }
}

impl RuntimeObjectTable {
    pub fn register_object(
        &mut self,
        key: RuntimeObjectKey,
    ) -> Result<RuntimeObjectId, RuntimeStateError> {
        if self.identity_index.contains_key(&key) {
            return Err(RuntimeStateError::conflict(
                "runtime-object",
                "runtime-object-identity-duplicate",
                "runtime object identity is already registered",
            ));
        }
        let id = RuntimeObjectId(self.next_object_id);
        self.next_object_id = self.next_object_id.checked_add(1).ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-object",
                "runtime-object-id-space-exhausted",
                "runtime object ID space is exhausted",
            )
        })?;
        let kind = key.kind();
        let (namespace, close_policy, wait_state) = match kind {
            RuntimeObjectKind::Process | RuntimeObjectKind::Thread => (
                if kind == RuntimeObjectKind::Process {
                    RUNTIME_PROCESS_OBJECT_NAMESPACE
                } else {
                    RUNTIME_THREAD_OBJECT_NAMESPACE
                },
                RuntimeHandleClosePolicy::CloseHandle,
                Some(RuntimeWaitState::Nonsignaled),
            ),
            RuntimeObjectKind::PrivateHeap | RuntimeObjectKind::Module => (
                if kind == RuntimeObjectKind::PrivateHeap {
                    LOGICAL_PRIVATE_HEAP_NAMESPACE
                } else {
                    LOGICAL_LOADED_MODULE_NAMESPACE
                },
                RuntimeHandleClosePolicy::SubsystemOwned,
                None,
            ),
        };
        let record = RuntimeObjectRecord {
            id,
            key: key.clone(),
            kind,
            namespace: namespace.to_owned(),
            close_policy,
            wait_state,
            open_handles: 0,
        };
        self.objects.insert(id, record);
        self.identity_index.insert(key, id);
        Ok(id)
    }

    pub fn register_object_with_handle(
        &mut self,
        key: RuntimeObjectKey,
        handle: RuntimeHandle,
    ) -> Result<RuntimeObjectId, RuntimeStateError> {
        self.preflight_exact_handle(key.kind(), handle)?;
        let id = self.register_object(key.clone())?;
        if let Err(error) = self.insert_exact_handle(id, handle) {
            self.objects.remove(&id);
            self.identity_index.remove(&key);
            return Err(error);
        }
        Ok(id)
    }

    fn register_object_and_open_handle(
        &mut self,
        key: RuntimeObjectKey,
    ) -> Result<(RuntimeObjectId, RuntimeHandle), RuntimeStateError> {
        let handle = self.next_available_handle(key.kind())?;
        let previous_next_object_id = self.next_object_id;
        let id = self.register_object(key.clone())?;
        if let Err(error) = self.insert_exact_handle(id, handle) {
            self.objects.remove(&id);
            self.identity_index.remove(&key);
            self.next_object_id = previous_next_object_id;
            return Err(error);
        }
        Ok((id, handle))
    }

    pub fn open_handle(
        &mut self,
        key: &RuntimeObjectKey,
    ) -> Result<RuntimeHandle, RuntimeStateError> {
        let object_id = self.identity_index.get(key).copied().ok_or_else(|| {
            RuntimeStateError::not_found(
                "runtime-object",
                "runtime-object-identity-unknown",
                "runtime object identity is not registered",
            )
        })?;
        let kind = key.kind();
        let handle = self.next_available_handle(kind)?;
        self.insert_exact_handle(object_id, handle)?;
        Ok(handle)
    }

    pub fn handle_for_object(
        &self,
        key: &RuntimeObjectKey,
    ) -> Result<RuntimeHandle, RuntimeStateError> {
        let object_id = self.identity_index.get(key).copied().ok_or_else(|| {
            RuntimeStateError::not_found(
                "runtime-object",
                "runtime-object-identity-unknown",
                "runtime object identity is not registered",
            )
        })?;
        self.handles
            .values()
            .find(|entry| entry.object_id == object_id)
            .map(|entry| entry.handle)
            .ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-object",
                    "runtime-object-handle-closed",
                    "runtime object has no open handle",
                )
            })
    }

    pub fn object_for_handle(
        &self,
        handle: RuntimeHandle,
        expected_kind: RuntimeObjectKind,
    ) -> Result<&RuntimeObjectRecord, RuntimeStateError> {
        let entry = self.lookup_entry(handle, expected_kind)?;
        self.objects.get(&entry.object_id).ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-object",
                "runtime-handle-object-dangling",
                "live runtime handle references a missing object",
            )
        })
    }

    pub fn close_handle(
        &mut self,
        handle: RuntimeHandle,
    ) -> Result<RuntimeHandleEntry, RuntimeStateError> {
        let kind = handle.kind()?;
        let entry = *self.lookup_entry(handle, kind)?;
        let record = self.objects.get(&entry.object_id).ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-object",
                "runtime-handle-object-dangling",
                "live runtime handle references a missing object",
            )
        })?;
        if record.close_policy != RuntimeHandleClosePolicy::CloseHandle {
            return Err(RuntimeStateError::conflict(
                "runtime-object",
                "runtime-handle-close-policy",
                "runtime handle must be retired by its owning subsystem",
            ));
        }
        if record.open_handles == 0 {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-object",
                "runtime-object-handle-count-underflow",
                "runtime object has a live handle but zero handle count",
            ));
        }
        let slot_key = runtime_handle_slot_key(kind, handle.slot());
        let slot = self.slots.get_mut(&slot_key).ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-object",
                "runtime-handle-slot-missing",
                "live runtime handle has no slot state",
            )
        })?;
        let record = self.objects.get_mut(&entry.object_id).ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-object",
                "runtime-handle-object-dangling",
                "closed runtime handle references a missing object",
            )
        })?;
        let removed = self.handles.remove(&handle.0).ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-object",
                "runtime-handle-entry-missing",
                "live runtime handle disappeared during close",
            )
        })?;
        record.open_handles -= 1;
        slot.live = false;
        if slot.generation == u8::MAX {
            slot.retired = true;
        } else {
            slot.generation += 1;
        }
        Ok(removed)
    }

    pub(crate) fn retire_subsystem_objects(
        &mut self,
        keys: &[RuntimeObjectKey],
    ) -> Result<Vec<RuntimeHandleEntry>, RuntimeStateError> {
        let mut unique_keys = BTreeSet::new();
        let mut plans = Vec::with_capacity(keys.len());
        for key in keys {
            if !unique_keys.insert(key.clone()) {
                return Err(RuntimeStateError::invalid_input(
                    "runtime-object",
                    "runtime-object-retirement-duplicate",
                    "one subsystem object was requested for retirement more than once",
                ));
            }
            let id = self.identity_index.get(key).copied().ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-object",
                    "runtime-object-identity-unknown",
                    "subsystem retirement targets an unregistered object",
                )
            })?;
            let record = self.objects.get(&id).ok_or_else(|| {
                RuntimeStateError::runtime_defect(
                    "runtime-object",
                    "runtime-object-index-dangling",
                    "runtime object identity index references a missing object",
                )
            })?;
            if record.close_policy != RuntimeHandleClosePolicy::SubsystemOwned {
                return Err(RuntimeStateError::conflict(
                    "runtime-object",
                    "runtime-object-retirement-policy",
                    "only the owning subsystem can retire this runtime object",
                ));
            }
            let handles: Vec<_> = self
                .handles
                .values()
                .filter(|entry| entry.object_id == id)
                .copied()
                .collect();
            if handles.len() != record.open_handles {
                return Err(RuntimeStateError::runtime_defect(
                    "runtime-object",
                    "runtime-object-handle-count-mismatch",
                    "subsystem object handle count does not match its live handle entries",
                ));
            }
            let mut next_slots = Vec::with_capacity(handles.len());
            for entry in &handles {
                let live = self.lookup_entry(entry.handle, record.kind)?;
                if live != entry {
                    return Err(RuntimeStateError::runtime_defect(
                        "runtime-object",
                        "runtime-object-retirement-entry-mismatch",
                        "subsystem retirement found inconsistent handle metadata",
                    ));
                }
                let slot_key = runtime_handle_slot_key(record.kind, entry.handle.slot());
                let slot = self.slots.get(&slot_key).copied().ok_or_else(|| {
                    RuntimeStateError::runtime_defect(
                        "runtime-object",
                        "runtime-handle-slot-missing",
                        "subsystem-owned live handle has no slot state",
                    )
                })?;
                next_slots.push((
                    slot_key,
                    RuntimeHandleSlot {
                        generation: if slot.generation == u8::MAX {
                            slot.generation
                        } else {
                            slot.generation + 1
                        },
                        live: false,
                        retired: slot.generation == u8::MAX,
                    },
                ));
            }
            plans.push(RuntimeObjectRetirementPlan {
                id,
                key: key.clone(),
                handles,
                next_slots,
            });
        }

        let mut retired_handles = Vec::new();
        for plan in plans {
            for entry in plan.handles {
                self.handles.remove(&entry.handle.0);
                retired_handles.push(entry);
            }
            for (slot_key, slot) in plan.next_slots {
                self.slots.insert(slot_key, slot);
            }
            self.objects.remove(&plan.id);
            self.identity_index.remove(&plan.key);
        }
        Ok(retired_handles)
    }

    pub fn wait_state(
        &self,
        handle: RuntimeHandle,
        expected_kind: RuntimeObjectKind,
    ) -> Result<RuntimeWaitState, RuntimeStateError> {
        self.object_for_handle(handle, expected_kind)?
            .wait_state
            .ok_or_else(|| {
                RuntimeStateError::invalid_input(
                    "runtime-object",
                    "runtime-object-not-waitable",
                    "runtime object type does not expose a wait state",
                )
            })
    }

    pub fn set_wait_state(
        &mut self,
        key: &RuntimeObjectKey,
        state: RuntimeWaitState,
    ) -> Result<(), RuntimeStateError> {
        let updates = self.plan_wait_state_updates(&[(key.clone(), state)])?;
        self.apply_wait_state_updates(updates);
        Ok(())
    }

    pub fn objects(&self) -> &BTreeMap<RuntimeObjectId, RuntimeObjectRecord> {
        &self.objects
    }

    pub fn handles(&self) -> &BTreeMap<u32, RuntimeHandleEntry> {
        &self.handles
    }

    fn plan_wait_state_updates(
        &self,
        updates: &[(RuntimeObjectKey, RuntimeWaitState)],
    ) -> Result<Vec<(RuntimeObjectId, RuntimeObjectRecord)>, RuntimeStateError> {
        let mut planned: Vec<(RuntimeObjectId, RuntimeObjectRecord)> =
            Vec::with_capacity(updates.len());
        for (key, state) in updates {
            let id = self.identity_index.get(key).copied().ok_or_else(|| {
                RuntimeStateError::runtime_defect(
                    "runtime-object",
                    "runtime-wait-object-missing",
                    "runtime wait state targets an unregistered object",
                )
            })?;
            if planned.iter().any(|(planned_id, _)| *planned_id == id) {
                return Err(RuntimeStateError::runtime_defect(
                    "runtime-object",
                    "runtime-wait-object-duplicate-update",
                    "one runtime wait object has duplicate state updates",
                ));
            }
            let mut record = self.objects.get(&id).cloned().ok_or_else(|| {
                RuntimeStateError::runtime_defect(
                    "runtime-object",
                    "runtime-object-index-dangling",
                    "runtime object identity index references a missing object",
                )
            })?;
            if record.wait_state.is_none() {
                return Err(RuntimeStateError::invalid_input(
                    "runtime-object",
                    "runtime-object-not-waitable",
                    "runtime object type does not expose a wait state",
                ));
            }
            record.wait_state = Some(*state);
            planned.push((id, record));
        }
        Ok(planned)
    }

    fn apply_wait_state_updates(&mut self, updates: Vec<(RuntimeObjectId, RuntimeObjectRecord)>) {
        for (id, record) in updates {
            self.objects.insert(id, record);
        }
    }

    fn next_available_handle(
        &self,
        kind: RuntimeObjectKind,
    ) -> Result<RuntimeHandle, RuntimeStateError> {
        for slot in 1..=u16::MAX {
            let slot_key = runtime_handle_slot_key(kind, slot);
            let generation = match self.slots.get(&slot_key) {
                Some(state) if !state.live && !state.retired => state.generation,
                Some(_) => continue,
                None => 1,
            };
            return RuntimeHandle::from_parts(kind, generation, slot);
        }
        Err(RuntimeStateError::resource_limit(
            "runtime-object",
            "runtime-handle-slot-space-exhausted",
            "runtime handle slot space is exhausted for this object type",
        ))
    }

    fn preflight_exact_handle(
        &self,
        expected_kind: RuntimeObjectKind,
        handle: RuntimeHandle,
    ) -> Result<(), RuntimeStateError> {
        let kind = handle.kind()?;
        if kind != expected_kind {
            return Err(runtime_handle_type_mismatch(handle, expected_kind, kind));
        }
        if handle.generation() == 0 || handle.slot() == 0 {
            return Err(RuntimeStateError::invalid_input(
                "runtime-object",
                "runtime-handle-parts-invalid",
                "runtime handles require nonzero generation and slot fields",
            ));
        }
        let slot_key = runtime_handle_slot_key(kind, handle.slot());
        match self.slots.get(&slot_key) {
            Some(state) if state.live => Err(RuntimeStateError::conflict(
                "runtime-object",
                "runtime-handle-slot-live",
                "runtime handle slot is already live",
            )),
            Some(state) if state.retired || state.generation != handle.generation() => {
                Err(RuntimeStateError::not_found(
                    "runtime-object",
                    "runtime-handle-stale",
                    "runtime handle generation does not match the current slot generation",
                ))
            }
            None if handle.generation() != 1 => Err(RuntimeStateError::not_found(
                "runtime-object",
                "runtime-handle-stale",
                "a new runtime handle slot must begin at generation one",
            )),
            _ => Ok(()),
        }
    }

    fn insert_exact_handle(
        &mut self,
        object_id: RuntimeObjectId,
        handle: RuntimeHandle,
    ) -> Result<(), RuntimeStateError> {
        let kind = self
            .objects
            .get(&object_id)
            .map(|record| record.kind)
            .ok_or_else(|| {
                RuntimeStateError::runtime_defect(
                    "runtime-object",
                    "runtime-object-id-unknown",
                    "handle insertion targets an unknown runtime object",
                )
            })?;
        self.preflight_exact_handle(kind, handle)?;
        if self.handles.contains_key(&handle.0) {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-object",
                "runtime-handle-entry-collision",
                "free runtime handle slot still has a table entry",
            ));
        }
        let record = self.objects.get_mut(&object_id).ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-object",
                "runtime-object-id-unknown",
                "inserted handle targets an unknown runtime object",
            )
        })?;
        if record.open_handles == usize::MAX {
            return Err(RuntimeStateError::resource_limit(
                "runtime-object",
                "runtime-object-handle-count-overflow",
                "runtime object handle count overflowed",
            ));
        }
        let slot_key = runtime_handle_slot_key(kind, handle.slot());
        self.slots.insert(
            slot_key,
            RuntimeHandleSlot {
                generation: handle.generation(),
                live: true,
                retired: false,
            },
        );
        self.handles.insert(
            handle.0,
            RuntimeHandleEntry {
                handle,
                object_id,
                kind,
            },
        );
        record.open_handles += 1;
        Ok(())
    }

    fn lookup_entry(
        &self,
        handle: RuntimeHandle,
        expected_kind: RuntimeObjectKind,
    ) -> Result<&RuntimeHandleEntry, RuntimeStateError> {
        let kind = handle.kind()?;
        if kind != expected_kind {
            return Err(runtime_handle_type_mismatch(handle, expected_kind, kind));
        }
        if handle.generation() == 0 || handle.slot() == 0 {
            return Err(RuntimeStateError::invalid_input(
                "runtime-object",
                "runtime-handle-parts-invalid",
                "runtime handles require nonzero generation and slot fields",
            ));
        }
        let slot_key = runtime_handle_slot_key(kind, handle.slot());
        let slot = self.slots.get(&slot_key).ok_or_else(|| {
            RuntimeStateError::not_found(
                "runtime-object",
                "runtime-handle-unknown",
                format!("runtime handle {:#010x} was never allocated", handle.0),
            )
        })?;
        if slot.retired || slot.generation != handle.generation() || !slot.live {
            return Err(RuntimeStateError::not_found(
                "runtime-object",
                "runtime-handle-stale",
                format!("runtime handle {:#010x} is closed or stale", handle.0),
            ));
        }
        self.handles.get(&handle.0).ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-object",
                "runtime-handle-entry-missing",
                "live runtime handle slot has no table entry",
            )
        })
    }
}

const fn runtime_handle_slot_key(kind: RuntimeObjectKind, slot: u16) -> u32 {
    (kind.handle_tag() as u32) << 16 | slot as u32
}

fn runtime_handle_type_mismatch(
    handle: RuntimeHandle,
    expected: RuntimeObjectKind,
    actual: RuntimeObjectKind,
) -> RuntimeStateError {
    RuntimeStateError::invalid_input(
        "runtime-object",
        "runtime-handle-type-mismatch",
        format!(
            "runtime handle {:#010x} has type {actual:?}, expected {expected:?}",
            handle.0
        ),
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct RuntimeCallbackFrameId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeAddressRange {
    pub base: GuestAddress,
    pub byte_length: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessAddressSpaceLayout {
    pub import_thunks: RuntimeAddressRange,
    pub callback_returns: RuntimeAddressRange,
    pub dynamic_exports: RuntimeAddressRange,
}

impl ProcessAddressSpaceLayout {
    pub fn vine_v1() -> Self {
        Self {
            import_thunks: RuntimeAddressRange {
                base: IMPORT_THUNK_ARENA_BASE,
                byte_length: IMPORT_THUNK_ARENA_SIZE,
            },
            callback_returns: RuntimeAddressRange {
                base: CALLBACK_RETURN_ARENA_BASE,
                byte_length: CALLBACK_RETURN_ARENA_SIZE,
            },
            dynamic_exports: RuntimeAddressRange {
                base: DYNAMIC_EXPORT_ARENA_BASE,
                byte_length: DYNAMIC_EXPORT_ARENA_SIZE,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeClockState {
    pub wall_clock_file_time: u64,
    pub tick_count: u32,
    pub performance_counter: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeStartupInfo {
    pub profile_id: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeBootstrap {
    pub process_id: RuntimeProcessId,
    pub initial_thread_id: RuntimeThreadId,
    pub initial_last_error: u32,
    pub address_space: ProcessAddressSpaceLayout,
    pub clock: RuntimeClockState,
    pub startup_info: RuntimeStartupInfo,
    pub encoded_pointer_namespace: String,
    pub encoded_pointer_token_prefix: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeHeap {
    pub owner: String,
    pub variant_id: String,
    pub namespace: String,
    pub handle: u32,
    pub creation_flags: u32,
    pub initial_size: u32,
    pub maximum_size: u32,
    pub growable: bool,
    pub live_allocations: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct RuntimeHeapAllocationId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeHeapInitialization {
    Zeroed,
    DeterministicUninitialized,
}

impl RuntimeHeapInitialization {
    pub const fn fill_byte(self) -> u8 {
        match self {
            Self::Zeroed => 0,
            Self::DeterministicUninitialized => RUNTIME_PRIVATE_HEAP_UNINITIALIZED_BYTE,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeHeapAllocation {
    pub id: RuntimeHeapAllocationId,
    pub address: GuestAddress,
    pub byte_length: u32,
    pub reserved_byte_length: u32,
    pub initialization: RuntimeHeapInitialization,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeHeapBacking {
    pub memory_owner: MemoryOwnerId,
    pub range: GuestRange,
    pub alignment: u32,
    pub committed_bytes: u32,
    pub high_water_mark: u32,
    pub next_allocation_id: RuntimeHeapAllocationId,
    pub allocations: BTreeMap<u32, RuntimeHeapAllocation>,
    pub free_blocks: BTreeMap<u32, u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeHeapTeardown {
    pub handles: Vec<u32>,
    pub allocations_discarded: usize,
    pub reserved_bytes_released: u64,
    pub committed_bytes_released: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeModule {
    pub owner: String,
    pub variant_id: String,
    pub namespace: String,
    pub handle: u32,
    pub requested_name: String,
    pub canonical_name: String,
    pub runtime_system_module: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeGuestImport {
    pub requested_module_id: RuntimeModuleId,
    pub requested_module: String,
    #[serde(flatten)]
    pub target: ImportTarget,
    pub iat_address: GuestAddress,
    pub original_iat_value: u32,
    pub resolution: RuntimeGuestImportResolution,
}

impl RuntimeGuestImport {
    pub fn resolved_address(&self) -> GuestAddress {
        match &self.resolution {
            RuntimeGuestImportResolution::GuestExport { export } => export.address,
            RuntimeGuestImportResolution::RuntimeExport { endpoint } => endpoint.address,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum RuntimeGuestImportResolution {
    GuestExport {
        export: RuntimeResolvedExport,
    },
    RuntimeExport {
        endpoint: RuntimeSystemExportEndpoint,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeSystemExportEndpoint {
    pub id: ImportThunkId,
    pub address: GuestAddress,
    pub export: RuntimeExport,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeSystemExportTable {
    pub arena: ImportThunkArena,
    pub endpoints: Vec<RuntimeSystemExportEndpoint>,
}

impl RuntimeSystemExportTable {
    pub fn endpoint(
        &self,
        module: &str,
        target: &ImportTarget,
    ) -> Option<&RuntimeSystemExportEndpoint> {
        self.endpoints
            .iter()
            .find(|endpoint| runtime_export_matches_import(&endpoint.export, module, target))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeModuleDependencyEdge {
    pub requester: RuntimeModuleId,
    pub dependency: RuntimeModuleId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeGuestImportBinding {
    pub requester_module_id: RuntimeModuleId,
    pub requester_handle: u32,
    pub source: ModuleIdentity,
    pub image_owner: MemoryOwnerId,
    pub image_load_base: GuestAddress,
    pub imports: Vec<RuntimeGuestImport>,
    pub dependency_edges: Vec<RuntimeModuleDependencyEdge>,
    pub image_owner_sealed: bool,
    pub invalidation: MemoryInvalidation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeGuestImportBindingPlan {
    pub(crate) requester_module_id: RuntimeModuleId,
    pub(crate) requester_handle: u32,
    pub(crate) source: ModuleIdentity,
    pub(crate) image_owner: MemoryOwnerId,
    pub(crate) image_load_base: GuestAddress,
    pub(crate) imports: Vec<RuntimeGuestImport>,
    pub(crate) dependency_edges: Vec<RuntimeModuleDependencyEdge>,
    pub(crate) previous_graph: RuntimeModuleGraph,
    pub(crate) next_graph: RuntimeModuleGraph,
    pub(crate) previous_runtime_system_exports: Option<RuntimeSystemExportTable>,
}

pub const DLL_PROCESS_DETACH: u32 = 0;
pub const DLL_PROCESS_ATTACH: u32 = 1;
pub const DLL_PROCESS_STATIC_RESERVED: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeDllLifecyclePhase {
    Idle,
    ProcessAttaching,
    AttachUnwinding,
    ProcessAttached,
    ProcessDetaching,
    ProcessDetached,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeDllCallbackReason {
    ProcessAttach,
    ProcessDetach,
}

impl RuntimeDllCallbackReason {
    pub const fn value(self) -> u32 {
        match self {
            Self::ProcessAttach => DLL_PROCESS_ATTACH,
            Self::ProcessDetach => DLL_PROCESS_DETACH,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeDllCallbackRequest {
    pub frame: RuntimeCallbackFrame,
    pub module_id: RuntimeModuleId,
    pub module_handle: u32,
    pub module_name: String,
    pub module_base: GuestAddress,
    pub entry_point: GuestAddress,
    pub reason: RuntimeDllCallbackReason,
    pub reserved: u32,
}

impl RuntimeDllCallbackRequest {
    pub fn arguments(&self) -> [u32; 3] {
        [self.module_base.0, self.reason.value(), self.reserved]
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeDllLifecycleCompletion {
    pub phase: RuntimeDllLifecyclePhase,
    pub graph_order: Vec<RuntimeModuleId>,
    pub callback_order: Vec<RuntimeModuleId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeDllAttachFailure {
    pub failed_module_id: RuntimeModuleId,
    pub failed_module: String,
    pub unwound_modules: Vec<RuntimeModuleId>,
    pub retryable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "action", rename_all = "kebab-case")]
pub enum RuntimeDllLifecycleAction {
    Callback {
        request: RuntimeDllCallbackRequest,
    },
    Complete {
        completion: RuntimeDllLifecycleCompletion,
    },
    AttachFailed {
        failure: RuntimeDllAttachFailure,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RuntimeDllOperationKind {
    Attach,
    AttachUnwind,
    Detach,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RuntimeDllOperation {
    kind: RuntimeDllOperationKind,
    graph_order: Vec<RuntimeModuleId>,
    remaining: VecDeque<RuntimeModuleId>,
    completed: Vec<RuntimeModuleId>,
    pending: Option<RuntimeDllCallbackRequest>,
    attach_failure: Option<(RuntimeModuleId, String)>,
    return_sentinel: GuestAddress,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeDynamicEndpoint {
    pub namespace: String,
    pub id: u32,
    pub address: GuestAddress,
    pub module: String,
    pub name: String,
    pub lifetime: String,
}

impl From<&DynamicExportThunk> for RuntimeDynamicEndpoint {
    fn from(thunk: &DynamicExportThunk) -> Self {
        Self {
            namespace: thunk.namespace.to_owned(),
            id: thunk.id.0,
            address: thunk.address,
            module: thunk.module.clone(),
            name: thunk.name.clone(),
            lifetime: thunk.lifetime.to_owned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum RuntimeTlsValue {
    Raw {
        value: u32,
    },
    DynamicEndpoint {
        endpoint_id: u32,
        address: GuestAddress,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeFlsSlot {
    pub namespace: String,
    pub index: u32,
    pub callback: Option<GuestAddress>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeCriticalSection {
    pub address: GuestAddress,
    pub spin_count: u32,
    pub owning_thread_id: Option<RuntimeThreadId>,
    pub recursion_count: u32,
}

impl RuntimeTlsValue {
    pub fn raw_value(&self) -> u32 {
        match self {
            Self::Raw { value } => *value,
            Self::DynamicEndpoint { address, .. } => address.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeEncodedPointer {
    pub namespace: String,
    pub token_id: u16,
    pub token: u32,
    pub source: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeCallbackFrame {
    pub id: RuntimeCallbackFrameId,
    pub thread_id: RuntimeThreadId,
    pub target: GuestAddress,
    pub return_sentinel: GuestAddress,
    pub entry_last_error: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum SchedulerThreadState {
    Initialized,
    Runnable,
    WaitingHost {
        request_id: u64,
    },
    WaitingObject {
        registration_id: RuntimeWaitRegistrationId,
        object_id: RuntimeObjectId,
    },
    WaitingMessage,
    InCallback {
        depth: usize,
    },
    Terminating {
        exit_code: u32,
    },
    Terminated {
        exit_code: u32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum RuntimeProcessLifecycleState {
    Bootstrapping,
    Running,
    ExitRequested {
        exit_code: u32,
        initiating_thread_id: RuntimeThreadId,
    },
    Exited {
        exit_code: u32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum RuntimeThreadLifecycleState {
    Initialized,
    Running,
    ExitRequested { exit_code: u32 },
    Exited { exit_code: u32 },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeHostRequest {
    pub request_id: u64,
    pub thread_id: RuntimeThreadId,
    pub service: String,
    pub resume_state: SchedulerThreadState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeHostCompletion {
    pub request_id: u64,
    pub thread_id: RuntimeThreadId,
    pub service: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeCancellation {
    pub thread_id: RuntimeThreadId,
    pub exit_code: u32,
    pub host_requests_cancelled: usize,
    pub callback_frames_cancelled: usize,
    pub wait_registrations_cancelled: usize,
    pub messages_discarded: usize,
    pub scheduler_events_discarded: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeProcessCancellation {
    pub initiating_thread_id: RuntimeThreadId,
    pub exit_code: u32,
    pub thread_cancellations: Vec<RuntimeCancellation>,
    pub host_requests_cancelled: usize,
    pub callback_frames_cancelled: usize,
    pub wait_registrations_cancelled: usize,
    pub messages_discarded: usize,
    pub scheduler_events_discarded: usize,
}

struct RuntimeThreadExitRequestPlan {
    thread_id: RuntimeThreadId,
    next_thread: RuntimeThreadState,
    cancelled_wait: Option<RuntimeWaitRegistration>,
    cancellation: RuntimeCancellation,
}

struct RuntimeThreadExitCompletionPlan {
    thread_id: RuntimeThreadId,
    exit_code: u32,
    next_thread: RuntimeThreadState,
}

struct RuntimeProcessExitRequestPlan {
    initiating_thread_id: RuntimeThreadId,
    exit_code: u32,
    thread_plans: Vec<RuntimeThreadExitRequestPlan>,
    cancellation: RuntimeProcessCancellation,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum RuntimeHeapBackingPlan {
    New {
        range: GuestRange,
        initial_committed_bytes: u32,
    },
    Existing {
        memory_owner: MemoryOwnerId,
        range: GuestRange,
        committed_bytes: u32,
    },
}

impl RuntimeHeapBackingPlan {
    pub(crate) const fn range(self) -> GuestRange {
        match self {
            Self::New { range, .. } | Self::Existing { range, .. } => range,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct RuntimeHeapAllocationPlan {
    pub handle: u32,
    pub allocation: RuntimeHeapAllocation,
    pub allocation_offset: u32,
    pub backing: RuntimeHeapBackingPlan,
    pub source_free_block: Option<(u32, u32)>,
    pub previous_high_water_mark: u32,
    pub next_committed_bytes: u32,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct RuntimeHeapFreePlan {
    pub handle: u32,
    pub allocation: RuntimeHeapAllocation,
    pub memory_owner: MemoryOwnerId,
    pub merged_free_block: (u32, u32),
    pub predecessor: Option<(u32, u32)>,
    pub successor: Option<(u32, u32)>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct RuntimeHeapDestructionPlan {
    pub handle: u32,
    pub backing: Option<(MemoryOwnerId, GuestRange, u32)>,
    pub live_allocations: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "event", content = "completion", rename_all = "kebab-case")]
pub enum RuntimeSchedulerEvent {
    HostCompleted(RuntimeHostCompletion),
    WaitSatisfied(RuntimeWaitCompletion),
    MessageReady(RuntimeMessageWake),
}

impl RuntimeSchedulerEvent {
    pub const fn thread_id(&self) -> RuntimeThreadId {
        match self {
            Self::HostCompleted(completion) => completion.thread_id,
            Self::WaitSatisfied(completion) => completion.registration.thread_id,
            Self::MessageReady(wake) => wake.thread_id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeThreadState {
    pub id: RuntimeThreadId,
    pub lifecycle: RuntimeThreadLifecycleState,
    pub last_error: u32,
    pub tls_values: BTreeMap<u32, RuntimeTlsValue>,
    pub fls_values: BTreeMap<u32, u32>,
    pub callback_frames: Vec<RuntimeCallbackFrame>,
    pub message_queue: VecDeque<RuntimeThreadMessage>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeProcessState {
    pub id: RuntimeProcessId,
    pub lifecycle: RuntimeProcessLifecycleState,
    pub address_space: ProcessAddressSpaceLayout,
    pub clock: RuntimeClockState,
    pub startup_info: RuntimeStartupInfo,
    pub heap_termination_on_corruption: bool,
    pub heaps: BTreeMap<u32, RuntimeHeap>,
    pub modules: BTreeMap<u32, RuntimeModule>,
    pub allocated_tls_indices: BTreeSet<u32>,
    pub fls_slots: BTreeMap<u32, RuntimeFlsSlot>,
    pub critical_sections: BTreeMap<GuestAddress, RuntimeCriticalSection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeScheduler {
    pub current_thread_id: RuntimeThreadId,
    pub threads: BTreeMap<RuntimeThreadId, SchedulerThreadState>,
    pub wait_registrations: BTreeMap<RuntimeWaitRegistrationId, RuntimeWaitRegistration>,
    pub object_waiters: BTreeMap<RuntimeObjectId, VecDeque<RuntimeWaitRegistrationId>>,
    pub events: VecDeque<RuntimeSchedulerEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeRegistries {
    pub dynamic_endpoints: BTreeMap<u32, RuntimeDynamicEndpoint>,
    pub encoded_pointers: BTreeMap<u32, RuntimeEncodedPointer>,
    pub host_requests: BTreeMap<u64, RuntimeHostRequest>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeState {
    process: RuntimeProcessState,
    threads: BTreeMap<RuntimeThreadId, RuntimeThreadState>,
    scheduler: RuntimeScheduler,
    objects: RuntimeObjectTable,
    module_graph: RuntimeModuleGraph,
    #[serde(skip)]
    module_images: RuntimeModuleImageRegistry,
    #[serde(skip)]
    module_import_bindings: BTreeMap<RuntimeModuleId, RuntimeGuestImportBinding>,
    #[serde(skip)]
    runtime_system_exports: Option<RuntimeSystemExportTable>,
    #[serde(skip)]
    dll_lifecycle_phase: RuntimeDllLifecyclePhase,
    #[serde(skip)]
    dll_operation: Option<RuntimeDllOperation>,
    registries: RuntimeRegistries,
    #[serde(skip)]
    heap_backings: BTreeMap<u32, RuntimeHeapBacking>,
    encoded_pointer_namespace: String,
    encoded_pointer_token_prefix: u32,
    encoded_pointer_sources: BTreeMap<u32, u32>,
    next_encoded_pointer_id: u16,
    next_private_heap_slot: u16,
    next_tls_index: u32,
    next_fls_index: u32,
    next_callback_frame_id: u64,
    next_host_request_id: u64,
    next_wait_registration_id: u64,
    next_message_id: u64,
}

impl RuntimeState {
    pub fn new(bootstrap: RuntimeBootstrap) -> Result<Self, RuntimeStateError> {
        if bootstrap.process_id.0 == 0 || bootstrap.initial_thread_id.0 == 0 {
            return Err(RuntimeStateError::invalid_input(
                "runtime-state",
                "zero-process-or-thread-id",
                "runtime process and initial thread IDs must be nonzero",
            ));
        }
        if bootstrap.startup_info.profile_id.is_empty()
            || bootstrap.startup_info.bytes.len() != X86_STARTUP_INFO_W_BYTE_LENGTH
        {
            return Err(RuntimeStateError::invalid_input(
                "runtime-state",
                "startup-info-profile-invalid",
                "runtime startup profile requires a nonempty identity and one x86 STARTUPINFOW image",
            ));
        }
        validate_address_space(bootstrap.address_space)?;
        if bootstrap.encoded_pointer_namespace != LOGICAL_ENCODED_POINTER_NAMESPACE
            || bootstrap.encoded_pointer_token_prefix == 0
            || bootstrap.encoded_pointer_token_prefix & 0xffff != 0
        {
            return Err(RuntimeStateError::invalid_input(
                "runtime-state",
                "encoded-pointer-policy-invalid",
                "encoded pointer policy requires the runtime namespace and a nonzero 16-bit-aligned token prefix",
            ));
        }

        let process_key = RuntimeObjectKey::Process {
            process_id: bootstrap.process_id.0,
        };
        let thread_key = RuntimeObjectKey::Thread {
            thread_id: bootstrap.initial_thread_id.0,
        };
        let mut objects = RuntimeObjectTable::default();
        objects.register_object_and_open_handle(process_key)?;
        objects.register_object_and_open_handle(thread_key)?;

        let initial_thread = RuntimeThreadState {
            id: bootstrap.initial_thread_id,
            lifecycle: RuntimeThreadLifecycleState::Initialized,
            last_error: bootstrap.initial_last_error,
            tls_values: BTreeMap::new(),
            fls_values: BTreeMap::new(),
            callback_frames: Vec::new(),
            message_queue: VecDeque::new(),
        };
        Ok(Self {
            process: RuntimeProcessState {
                id: bootstrap.process_id,
                lifecycle: RuntimeProcessLifecycleState::Bootstrapping,
                address_space: bootstrap.address_space,
                clock: bootstrap.clock,
                startup_info: bootstrap.startup_info,
                heap_termination_on_corruption: false,
                heaps: BTreeMap::new(),
                modules: BTreeMap::new(),
                allocated_tls_indices: BTreeSet::new(),
                fls_slots: BTreeMap::new(),
                critical_sections: BTreeMap::new(),
            },
            threads: BTreeMap::from([(bootstrap.initial_thread_id, initial_thread)]),
            scheduler: RuntimeScheduler {
                current_thread_id: bootstrap.initial_thread_id,
                threads: BTreeMap::from([(
                    bootstrap.initial_thread_id,
                    SchedulerThreadState::Initialized,
                )]),
                wait_registrations: BTreeMap::new(),
                object_waiters: BTreeMap::new(),
                events: VecDeque::new(),
            },
            objects,
            module_graph: RuntimeModuleGraph::default(),
            module_images: RuntimeModuleImageRegistry::default(),
            module_import_bindings: BTreeMap::new(),
            runtime_system_exports: None,
            dll_lifecycle_phase: RuntimeDllLifecyclePhase::Idle,
            dll_operation: None,
            registries: RuntimeRegistries {
                dynamic_endpoints: BTreeMap::new(),
                encoded_pointers: BTreeMap::new(),
                host_requests: BTreeMap::new(),
            },
            heap_backings: BTreeMap::new(),
            encoded_pointer_namespace: bootstrap.encoded_pointer_namespace,
            encoded_pointer_token_prefix: bootstrap.encoded_pointer_token_prefix,
            encoded_pointer_sources: BTreeMap::new(),
            next_encoded_pointer_id: 1,
            next_private_heap_slot: 1,
            next_tls_index: 0,
            next_fls_index: 0,
            next_callback_frame_id: 1,
            next_host_request_id: 1,
            next_wait_registration_id: 1,
            next_message_id: 1,
        })
    }

    pub fn process_id(&self) -> RuntimeProcessId {
        self.process.id
    }

    pub fn process(&self) -> &RuntimeProcessState {
        &self.process
    }

    pub fn current_thread_id(&self) -> RuntimeThreadId {
        self.scheduler.current_thread_id
    }

    pub fn current_thread_state(&self) -> Result<&RuntimeThreadState, RuntimeStateError> {
        self.current_thread()
    }

    pub fn thread_state(
        &self,
        thread_id: RuntimeThreadId,
    ) -> Result<&RuntimeThreadState, RuntimeStateError> {
        self.threads.get(&thread_id).ok_or_else(|| {
            RuntimeStateError::not_found(
                "runtime-thread-registry",
                "runtime-thread-id-unknown",
                format!("runtime thread ID {} is not registered", thread_id.0),
            )
        })
    }

    pub fn scheduler(&self) -> &RuntimeScheduler {
        &self.scheduler
    }

    pub fn objects(&self) -> &RuntimeObjectTable {
        &self.objects
    }

    pub fn module_graph(&self) -> &RuntimeModuleGraph {
        &self.module_graph
    }

    pub fn module_images(&self) -> &RuntimeModuleImageRegistry {
        &self.module_images
    }

    pub fn resolve_guest_export(
        &self,
        module: &str,
        symbol: ExportLookup,
    ) -> Result<RuntimeResolvedExport, RuntimeModuleImageError> {
        self.module_images.resolve(module, symbol)
    }

    pub(crate) fn register_module_image(
        &mut self,
        memory: &GuestMemory,
        handle: u32,
        loader: &LoaderIngressReport,
        materialization: &ImageMaterialization,
    ) -> Result<&RuntimeModuleImage, RuntimeModuleImageError> {
        self.module(handle)
            .map_err(runtime_module_image_state_error)?;
        let node = self
            .module_graph
            .module_by_handle(handle)
            .map_err(RuntimeModuleImageError::from_module_graph)?
            .clone();
        self.module_images
            .register(memory, &node, loader, materialization)
    }

    pub fn module_import_bindings(&self) -> &BTreeMap<RuntimeModuleId, RuntimeGuestImportBinding> {
        &self.module_import_bindings
    }

    pub fn runtime_system_exports(&self) -> Option<&RuntimeSystemExportTable> {
        self.runtime_system_exports.as_ref()
    }

    pub fn guest_import_binding(
        &self,
        handle: u32,
    ) -> Result<&RuntimeGuestImportBinding, RuntimeStateError> {
        self.module(handle)?;
        let module_id = self
            .module_graph
            .module_by_handle(handle)
            .map_err(runtime_module_graph_error)?
            .id;
        self.module_import_bindings.get(&module_id).ok_or_else(|| {
            RuntimeStateError::not_found(
                "runtime-guest-imports",
                "guest-import-binding-missing",
                format!("module handle {handle:#010x} has no committed guest import binding"),
            )
        })
    }

    pub(crate) fn plan_guest_import_binding(
        &self,
        requester_handle: u32,
        source: ModuleIdentity,
        image_owner: MemoryOwnerId,
        image_load_base: GuestAddress,
        imports: Vec<RuntimeGuestImport>,
        dependency_edges: Vec<RuntimeModuleDependencyEdge>,
    ) -> Result<RuntimeGuestImportBindingPlan, RuntimeStateError> {
        self.module(requester_handle)?;
        let requester = self
            .module_graph
            .module_by_handle(requester_handle)
            .map_err(runtime_module_graph_error)?;
        if requester.state != RuntimeModuleLoadState::Registered {
            return Err(RuntimeStateError::conflict(
                "runtime-guest-imports",
                "guest-import-requester-state-invalid",
                "guest imports can only bind while the requester module is registered",
            ));
        }
        if self.module_import_bindings.contains_key(&requester.id) {
            return Err(RuntimeStateError::conflict(
                "runtime-guest-imports",
                "guest-import-binding-duplicate",
                "requester module already has a committed guest import binding",
            ));
        }
        if self.module_images.images().contains_key(&requester.id) {
            return Err(RuntimeStateError::conflict(
                "runtime-guest-imports",
                "guest-import-requester-image-already-registered",
                "requester image must bind imports before mapped image registration",
            ));
        }
        if imports
            .iter()
            .any(|import| import.resolved_address().0 == 0)
        {
            return Err(RuntimeStateError::invalid_input(
                "runtime-guest-imports",
                "guest-import-target-address-null",
                "resolved guest import address cannot be null",
            ));
        }

        let dependency_edges = dependency_edges
            .into_iter()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let previous_graph = self.module_graph.clone();
        let mut next_graph = previous_graph.clone();
        for edge in &dependency_edges {
            let requester_node = next_graph
                .module(edge.requester)
                .map_err(runtime_module_graph_error)?;
            next_graph
                .module(edge.dependency)
                .map_err(runtime_module_graph_error)?;
            if !requester_node.dependencies.contains(&edge.dependency) {
                next_graph
                    .add_dependency(edge.requester, edge.dependency)
                    .map_err(runtime_module_graph_error)?;
            }
        }
        Ok(RuntimeGuestImportBindingPlan {
            requester_module_id: requester.id,
            requester_handle,
            source,
            image_owner,
            image_load_base,
            imports,
            dependency_edges,
            previous_graph,
            next_graph,
            previous_runtime_system_exports: self.runtime_system_exports.clone(),
        })
    }

    pub(crate) fn commit_guest_import_binding(
        &mut self,
        plan: RuntimeGuestImportBindingPlan,
        invalidation: MemoryInvalidation,
        runtime_system_exports: Option<RuntimeSystemExportTable>,
    ) -> Result<&RuntimeGuestImportBinding, RuntimeStateError> {
        if self.module_graph != plan.previous_graph {
            return Err(RuntimeStateError::conflict(
                "runtime-guest-imports",
                "guest-import-plan-graph-stale",
                "module graph changed after guest import planning",
            ));
        }
        if self
            .module_import_bindings
            .contains_key(&plan.requester_module_id)
        {
            return Err(RuntimeStateError::conflict(
                "runtime-guest-imports",
                "guest-import-binding-duplicate",
                "requester module already has a committed guest import binding",
            ));
        }
        if self.runtime_system_exports != plan.previous_runtime_system_exports {
            return Err(RuntimeStateError::conflict(
                "runtime-guest-imports",
                "guest-import-runtime-export-table-stale",
                "runtime system export table changed after guest import planning",
            ));
        }
        let imports_runtime_exports = plan.imports.iter().any(|import| {
            matches!(
                &import.resolution,
                RuntimeGuestImportResolution::RuntimeExport { .. }
            )
        });
        match (
            plan.previous_runtime_system_exports.as_ref(),
            runtime_system_exports.as_ref(),
            imports_runtime_exports,
        ) {
            (None, Some(table), true)
                if plan.imports.iter().all(|import| match &import.resolution {
                    RuntimeGuestImportResolution::RuntimeExport { endpoint } => {
                        table.endpoints.contains(endpoint)
                    }
                    RuntimeGuestImportResolution::GuestExport { .. } => true,
                }) => {}
            (Some(_), None, _) | (None, None, false) => {}
            (None, Some(_), false) => {
                return Err(RuntimeStateError::invalid_input(
                    "runtime-guest-imports",
                    "guest-import-runtime-export-table-unneeded",
                    "a runtime export table cannot be installed without a runtime-system import",
                ));
            }
            (None, None, true) => {
                return Err(RuntimeStateError::conflict(
                    "runtime-guest-imports",
                    "guest-import-runtime-export-table-missing",
                    "runtime-system imports require an installed runtime export table",
                ));
            }
            (None, Some(_), true) => {
                return Err(RuntimeStateError::invalid_input(
                    "runtime-guest-imports",
                    "guest-import-runtime-export-endpoint-missing",
                    "new runtime export table does not own every planned runtime endpoint",
                ));
            }
            (Some(_), Some(_), _) => {
                return Err(RuntimeStateError::conflict(
                    "runtime-guest-imports",
                    "guest-import-runtime-export-table-duplicate",
                    "runtime export table is already installed",
                ));
            }
        }
        let binding = RuntimeGuestImportBinding {
            requester_module_id: plan.requester_module_id,
            requester_handle: plan.requester_handle,
            source: plan.source,
            image_owner: plan.image_owner,
            image_load_base: plan.image_load_base,
            imports: plan.imports,
            dependency_edges: plan.dependency_edges,
            image_owner_sealed: true,
            invalidation,
        };
        self.module_graph = plan.next_graph;
        if let Some(runtime_system_exports) = runtime_system_exports {
            self.runtime_system_exports = Some(runtime_system_exports);
        }
        self.module_import_bindings
            .insert(plan.requester_module_id, binding);
        self.module_import_bindings
            .get(&plan.requester_module_id)
            .ok_or_else(|| {
                RuntimeStateError::runtime_defect(
                    "runtime-guest-imports",
                    "committed-guest-import-binding-missing",
                    "guest import binding disappeared after commit",
                )
            })
    }

    pub fn process_handle(&self) -> Result<RuntimeHandle, RuntimeStateError> {
        self.objects.handle_for_object(&RuntimeObjectKey::Process {
            process_id: self.process.id.0,
        })
    }

    pub fn current_thread_handle(&self) -> Result<RuntimeHandle, RuntimeStateError> {
        self.thread_handle(self.current_thread_id())
    }

    pub fn thread_handle(
        &self,
        thread_id: RuntimeThreadId,
    ) -> Result<RuntimeHandle, RuntimeStateError> {
        self.thread_state(thread_id)?;
        self.objects.handle_for_object(&RuntimeObjectKey::Thread {
            thread_id: thread_id.0,
        })
    }

    pub fn open_process_handle(&mut self) -> Result<RuntimeHandle, RuntimeStateError> {
        self.objects.open_handle(&RuntimeObjectKey::Process {
            process_id: self.process.id.0,
        })
    }

    pub fn open_current_thread_handle(&mut self) -> Result<RuntimeHandle, RuntimeStateError> {
        self.open_thread_handle(self.current_thread_id())
    }

    pub fn open_thread_handle(
        &mut self,
        thread_id: RuntimeThreadId,
    ) -> Result<RuntimeHandle, RuntimeStateError> {
        self.thread_state(thread_id)?;
        self.objects.open_handle(&RuntimeObjectKey::Thread {
            thread_id: thread_id.0,
        })
    }

    pub fn object_for_handle(
        &self,
        handle: RuntimeHandle,
        expected_kind: RuntimeObjectKind,
    ) -> Result<&RuntimeObjectRecord, RuntimeStateError> {
        self.objects.object_for_handle(handle, expected_kind)
    }

    pub fn close_handle(
        &mut self,
        handle: RuntimeHandle,
    ) -> Result<RuntimeHandleEntry, RuntimeStateError> {
        self.objects.close_handle(handle)
    }

    pub fn object_wait_state(
        &self,
        handle: RuntimeHandle,
        expected_kind: RuntimeObjectKind,
    ) -> Result<RuntimeWaitState, RuntimeStateError> {
        self.objects.wait_state(handle, expected_kind)
    }

    pub fn registries(&self) -> &RuntimeRegistries {
        &self.registries
    }

    pub fn clock(&self) -> &RuntimeClockState {
        &self.process.clock
    }

    pub fn startup_info(&self) -> &RuntimeStartupInfo {
        &self.process.startup_info
    }

    pub fn current_last_error(&self) -> Result<u32, RuntimeStateError> {
        Ok(self.current_thread()?.last_error)
    }

    pub fn set_current_last_error(&mut self, value: u32) -> Result<(), RuntimeStateError> {
        self.current_thread_mut()?.last_error = value;
        Ok(())
    }

    pub fn scheduler_state(&self) -> Result<SchedulerThreadState, RuntimeStateError> {
        self.scheduler_state_for_thread(self.scheduler.current_thread_id)
    }

    pub fn scheduler_state_for_thread(
        &self,
        thread_id: RuntimeThreadId,
    ) -> Result<SchedulerThreadState, RuntimeStateError> {
        self.scheduler
            .threads
            .get(&thread_id)
            .copied()
            .ok_or_else(|| {
                RuntimeStateError::runtime_defect(
                    "runtime-scheduler",
                    "thread-scheduler-state-missing",
                    format!("runtime thread ID {} has no scheduler state", thread_id.0),
                )
            })
    }

    pub fn start_initial_thread(&mut self) -> Result<(), RuntimeStateError> {
        if self.process.lifecycle != RuntimeProcessLifecycleState::Bootstrapping
            || self.current_thread()?.lifecycle != RuntimeThreadLifecycleState::Initialized
            || self.scheduler_state()? != SchedulerThreadState::Initialized
        {
            return Err(RuntimeStateError::conflict(
                "runtime-lifecycle",
                "initial-thread-start-transition-invalid",
                "initial thread start requires matching process, thread, and scheduler bootstrap states",
            ));
        }
        let thread_id = self.current_thread_id();
        let mut next_thread = self.current_thread()?.clone();
        next_thread.lifecycle = RuntimeThreadLifecycleState::Running;
        self.process.lifecycle = RuntimeProcessLifecycleState::Running;
        self.threads.insert(thread_id, next_thread);
        self.scheduler
            .threads
            .insert(thread_id, SchedulerThreadState::Runnable);
        Ok(())
    }

    pub fn register_thread(
        &mut self,
        thread_id: RuntimeThreadId,
        initial_last_error: u32,
    ) -> Result<RuntimeHandle, RuntimeStateError> {
        if self.process.lifecycle != RuntimeProcessLifecycleState::Running {
            return Err(RuntimeStateError::conflict(
                "runtime-thread-registry",
                "thread-registration-process-state-invalid",
                "thread registration requires a running process",
            ));
        }
        if thread_id.0 == 0 {
            return Err(RuntimeStateError::invalid_input(
                "runtime-thread-registry",
                "thread-id-zero",
                "runtime thread IDs must be nonzero",
            ));
        }
        let thread_registered = self.threads.contains_key(&thread_id);
        let scheduler_registered = self.scheduler.threads.contains_key(&thread_id);
        if thread_registered != scheduler_registered {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-thread-registry",
                "thread-registry-scheduler-mismatch",
                "thread and scheduler registries disagree about a thread identity",
            ));
        }
        if thread_registered {
            return Err(RuntimeStateError::conflict(
                "runtime-thread-registry",
                "thread-id-duplicate",
                format!("runtime thread ID {} is already registered", thread_id.0),
            ));
        }
        let thread_key = RuntimeObjectKey::Thread {
            thread_id: thread_id.0,
        };
        if self.objects.identity_index.contains_key(&thread_key) {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-thread-registry",
                "thread-registry-object-orphan",
                "thread object identity exists without a thread registry record",
            ));
        }
        let (_, handle) = self.objects.register_object_and_open_handle(thread_key)?;
        self.threads.insert(
            thread_id,
            RuntimeThreadState {
                id: thread_id,
                lifecycle: RuntimeThreadLifecycleState::Running,
                last_error: initial_last_error,
                tls_values: BTreeMap::new(),
                fls_values: BTreeMap::new(),
                callback_frames: Vec::new(),
                message_queue: VecDeque::new(),
            },
        );
        self.scheduler
            .threads
            .insert(thread_id, SchedulerThreadState::Runnable);
        Ok(handle)
    }

    pub fn select_thread(&mut self, thread_id: RuntimeThreadId) -> Result<(), RuntimeStateError> {
        if self.process.lifecycle != RuntimeProcessLifecycleState::Running {
            return Err(RuntimeStateError::conflict(
                "runtime-thread-registry",
                "thread-selection-process-state-invalid",
                "thread selection requires a running process",
            ));
        }
        let thread = self.thread_state(thread_id)?;
        let scheduler_state = self.scheduler_state_for_thread(thread_id)?;
        if thread.lifecycle != RuntimeThreadLifecycleState::Running
            || !matches!(
                scheduler_state,
                SchedulerThreadState::Runnable
                    | SchedulerThreadState::WaitingHost { .. }
                    | SchedulerThreadState::WaitingObject { .. }
                    | SchedulerThreadState::WaitingMessage
                    | SchedulerThreadState::InCallback { .. }
            )
        {
            return Err(RuntimeStateError::conflict(
                "runtime-thread-registry",
                "thread-selection-state-invalid",
                "thread selection requires matching live thread and scheduler states",
            ));
        }
        let thread_key = RuntimeObjectKey::Thread {
            thread_id: thread_id.0,
        };
        if !self.objects.identity_index.contains_key(&thread_key) {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-thread-registry",
                "thread-registry-object-missing",
                "registered thread has no runtime object identity",
            ));
        }
        self.scheduler.current_thread_id = thread_id;
        Ok(())
    }

    pub fn wait_current_thread_on(
        &mut self,
        handle: RuntimeHandle,
    ) -> Result<RuntimeWaitDisposition, RuntimeStateError> {
        if self.process.lifecycle != RuntimeProcessLifecycleState::Running
            || self.current_thread()?.lifecycle != RuntimeThreadLifecycleState::Running
            || self.scheduler_state()? != SchedulerThreadState::Runnable
        {
            return Err(RuntimeStateError::conflict(
                "runtime-wait",
                "wait-registration-thread-state-invalid",
                "wait registration requires a running process and runnable current thread",
            ));
        }
        let thread_id = self.current_thread_id();
        if self
            .scheduler
            .wait_registrations
            .values()
            .any(|registration| registration.thread_id == thread_id)
        {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-wait",
                "wait-registration-thread-duplicate",
                "runnable thread already owns a wait registration",
            ));
        }
        let kind = handle.kind()?;
        let object = self.objects.object_for_handle(handle, kind)?.clone();
        let wait_state = object.wait_state.ok_or_else(|| {
            RuntimeStateError::invalid_input(
                "runtime-wait",
                "runtime-object-not-waitable",
                "runtime object type does not expose a wait state",
            )
        })?;
        if matches!(wait_state, RuntimeWaitState::Signaled { .. }) {
            return Ok(RuntimeWaitDisposition::AlreadySignaled {
                object_id: object.id,
                wait_state,
            });
        }
        let id = RuntimeWaitRegistrationId(self.next_wait_registration_id);
        let next_wait_registration_id =
            self.next_wait_registration_id
                .checked_add(1)
                .ok_or_else(|| {
                    RuntimeStateError::resource_limit(
                        "runtime-wait",
                        "wait-registration-id-overflow",
                        "wait registration ID space is exhausted",
                    )
                })?;
        if self.scheduler.wait_registrations.contains_key(&id) {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-wait",
                "wait-registration-id-collision",
                "wait registration allocator selected a live ID",
            ));
        }
        let registration = RuntimeWaitRegistration {
            id,
            thread_id,
            object_id: object.id,
            handle,
        };
        self.next_wait_registration_id = next_wait_registration_id;
        self.scheduler.wait_registrations.insert(id, registration);
        self.scheduler
            .object_waiters
            .entry(object.id)
            .or_default()
            .push_back(id);
        self.scheduler.threads.insert(
            thread_id,
            SchedulerThreadState::WaitingObject {
                registration_id: id,
                object_id: object.id,
            },
        );
        Ok(RuntimeWaitDisposition::Pending { registration })
    }

    pub fn post_thread_message(
        &mut self,
        target_thread_id: RuntimeThreadId,
        hwnd: u32,
        message: u32,
        w_param: u32,
        l_param: u32,
    ) -> Result<RuntimeThreadMessage, RuntimeStateError> {
        if self.process.lifecycle != RuntimeProcessLifecycleState::Running
            || self.current_thread()?.lifecycle != RuntimeThreadLifecycleState::Running
            || !matches!(
                self.scheduler_state()?,
                SchedulerThreadState::Runnable | SchedulerThreadState::InCallback { .. }
            )
        {
            return Err(RuntimeStateError::conflict(
                "runtime-message",
                "message-post-source-state-invalid",
                "message post requires a running process and executing source thread",
            ));
        }
        let source_thread_id = self.current_thread_id();
        let target_thread = self.thread_state(target_thread_id)?;
        let target_scheduler_state = self.scheduler_state_for_thread(target_thread_id)?;
        if target_thread.lifecycle != RuntimeThreadLifecycleState::Running
            || !matches!(
                target_scheduler_state,
                SchedulerThreadState::Runnable
                    | SchedulerThreadState::WaitingHost { .. }
                    | SchedulerThreadState::WaitingObject { .. }
                    | SchedulerThreadState::WaitingMessage
                    | SchedulerThreadState::InCallback { .. }
            )
        {
            return Err(RuntimeStateError::conflict(
                "runtime-message",
                "message-post-target-state-invalid",
                "message target requires matching live thread and scheduler states",
            ));
        }
        if target_thread.message_queue.len() >= MAX_RUNTIME_THREAD_MESSAGES {
            return Err(RuntimeStateError::resource_limit(
                "runtime-message",
                "message-queue-capacity-exceeded",
                "runtime thread message queue reached its controlled capacity",
            ));
        }
        if target_scheduler_state == SchedulerThreadState::WaitingMessage
            && !target_thread.message_queue.is_empty()
        {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-message",
                "waiting-message-thread-queue-nonempty",
                "message-waiting thread must have an empty queue",
            ));
        }
        let id = RuntimeMessageId(self.next_message_id);
        let next_message_id = self.next_message_id.checked_add(1).ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-message",
                "message-id-overflow",
                "runtime message ID space is exhausted",
            )
        })?;
        let posted = RuntimeThreadMessage {
            id,
            source_thread_id,
            target_thread_id,
            hwnd,
            message,
            w_param,
            l_param,
        };
        let queue_depth = {
            let target = self.threads.get_mut(&target_thread_id).ok_or_else(|| {
                RuntimeStateError::runtime_defect(
                    "runtime-message",
                    "message-target-disappeared",
                    "message target disappeared after complete registry preflight",
                )
            })?;
            target.message_queue.push_back(posted);
            target.message_queue.len()
        };
        self.next_message_id = next_message_id;
        if target_scheduler_state == SchedulerThreadState::WaitingMessage {
            self.scheduler
                .threads
                .insert(target_thread_id, SchedulerThreadState::Runnable);
            self.scheduler
                .events
                .push_back(RuntimeSchedulerEvent::MessageReady(RuntimeMessageWake {
                    thread_id: target_thread_id,
                    message_id: id,
                    queue_depth,
                }));
        }
        Ok(posted)
    }

    pub fn receive_current_thread_message(
        &mut self,
    ) -> Result<RuntimeMessageReceiveDisposition, RuntimeStateError> {
        if self.process.lifecycle != RuntimeProcessLifecycleState::Running
            || self.current_thread()?.lifecycle != RuntimeThreadLifecycleState::Running
            || self.scheduler_state()? != SchedulerThreadState::Runnable
        {
            return Err(RuntimeStateError::conflict(
                "runtime-message",
                "message-receive-thread-state-invalid",
                "message receive requires a running process and runnable owner thread",
            ));
        }
        let thread_id = self.current_thread_id();
        if let Some(message) = self.current_thread_mut()?.message_queue.pop_front() {
            return Ok(RuntimeMessageReceiveDisposition::Message { message });
        }
        self.scheduler
            .threads
            .insert(thread_id, SchedulerThreadState::WaitingMessage);
        Ok(RuntimeMessageReceiveDisposition::Blocked)
    }

    pub fn set_heap_termination_on_corruption(&mut self, enabled: bool) {
        self.process.heap_termination_on_corruption = enabled;
    }

    pub fn create_heap(&mut self, heap: RuntimeHeap) -> Result<&RuntimeHeap, RuntimeStateError> {
        if heap.owner.is_empty()
            || heap.variant_id.is_empty()
            || heap.namespace != LOGICAL_PRIVATE_HEAP_NAMESPACE
            || heap.handle == 0
        {
            return Err(RuntimeStateError::invalid_input(
                "runtime-heap",
                "heap-identity-invalid",
                "runtime heap requires controlled owner, variant, namespace, and nonzero handle",
            ));
        }
        if self.process.heaps.contains_key(&heap.handle) {
            return Err(RuntimeStateError::conflict(
                "runtime-heap",
                "heap-handle-duplicate",
                "runtime heap handle is already live",
            ));
        }
        let handle = heap.handle;
        self.objects.register_object_with_handle(
            RuntimeObjectKey::PrivateHeap { handle },
            RuntimeHandle(handle),
        )?;
        self.process.heaps.insert(handle, heap);
        self.process.heaps.get(&handle).ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-heap",
                "inserted-heap-missing",
                "runtime heap disappeared after insertion",
            )
        })
    }

    pub fn allocate_private_heap(
        &mut self,
        creation_flags: u32,
        initial_size: u32,
        maximum_size: u32,
    ) -> Result<&RuntimeHeap, RuntimeStateError> {
        loop {
            let slot = self.next_private_heap_slot;
            if slot == 0 {
                return Err(RuntimeStateError::resource_limit(
                    "runtime-heap",
                    "private-heap-handle-space-exhausted",
                    "logical private heap handle space is exhausted",
                ));
            }
            self.next_private_heap_slot = self.next_private_heap_slot.checked_add(1).unwrap_or(0);
            let handle = LOGICAL_PRIVATE_HEAP_HANDLE_PREFIX | u32::from(slot);
            if self.process.heaps.contains_key(&handle) {
                continue;
            }
            return self.create_heap(RuntimeHeap {
                owner: RUNTIME_PRIVATE_HEAP_OWNER.to_owned(),
                variant_id: RUNTIME_PRIVATE_HEAP_VARIANT.to_owned(),
                namespace: LOGICAL_PRIVATE_HEAP_NAMESPACE.to_owned(),
                handle,
                creation_flags,
                initial_size,
                maximum_size,
                growable: maximum_size == 0,
                live_allocations: 0,
            });
        }
    }

    pub fn heap(&self, handle: u32) -> Result<&RuntimeHeap, RuntimeStateError> {
        self.objects
            .object_for_handle(RuntimeHandle(handle), RuntimeObjectKind::PrivateHeap)?;
        self.process.heaps.get(&handle).ok_or_else(|| {
            RuntimeStateError::not_found(
                "runtime-heap",
                "heap-handle-unknown",
                format!("logical heap handle {handle:#010x} is not live"),
            )
        })
    }

    pub fn heap_backing(
        &self,
        handle: u32,
    ) -> Result<Option<&RuntimeHeapBacking>, RuntimeStateError> {
        self.heap(handle)?;
        Ok(self.heap_backings.get(&handle))
    }

    pub(crate) fn plan_private_heap_allocation(
        &self,
        handle: u32,
        byte_length: u32,
        initialization: RuntimeHeapInitialization,
    ) -> Result<RuntimeHeapAllocationPlan, RuntimeStateError> {
        self.require_private_heap_operation_state()?;
        if byte_length == 0 {
            return Err(RuntimeStateError::invalid_input(
                "runtime-heap-backing",
                "heap-allocation-size-zero",
                "private heap backing does not accept zero-byte allocations",
            ));
        }
        let heap = self.heap(handle)?;
        let reserved_byte_length = align_runtime_heap_value(
            byte_length,
            RUNTIME_PRIVATE_HEAP_ALIGNMENT,
            "heap-allocation-size-overflow",
        )?;
        if usize::try_from(heap.live_allocations).map_err(|_| {
            RuntimeStateError::runtime_defect(
                "runtime-heap-backing",
                "heap-live-allocation-count-not-representable",
                "heap live allocation count does not fit host accounting",
            )
        })? >= RUNTIME_PRIVATE_HEAP_MAX_LIVE_ALLOCATIONS
        {
            return Err(RuntimeStateError::resource_limit(
                "runtime-heap-backing",
                "heap-live-allocation-limit-exceeded",
                format!(
                    "private heap allocation limit {} was reached",
                    RUNTIME_PRIVATE_HEAP_MAX_LIVE_ALLOCATIONS
                ),
            ));
        }

        let (
            backing_plan,
            allocation_id,
            allocation_offset,
            source_free_block,
            previous_high_water_mark,
            next_committed_bytes,
        ) = if let Some(backing) = self.heap_backings.get(&handle) {
            validate_runtime_heap_backing(heap, backing)?;
            let allocation_id = backing.next_allocation_id;
            allocation_id.0.checked_add(1).ok_or_else(|| {
                RuntimeStateError::resource_limit(
                    "runtime-heap-backing",
                    "heap-allocation-id-space-exhausted",
                    "private heap allocation identifier space is exhausted",
                )
            })?;
            let source_free_block = backing
                .free_blocks
                .iter()
                .find(|(_, available)| **available >= reserved_byte_length)
                .map(|(offset, available)| (*offset, *available));
            let allocation_offset = source_free_block
                .map(|(offset, _)| offset)
                .unwrap_or(backing.high_water_mark);
            let allocation_end = allocation_offset
                .checked_add(reserved_byte_length)
                .ok_or_else(|| {
                    RuntimeStateError::resource_limit(
                        "runtime-heap-backing",
                        "heap-allocation-range-overflow",
                        "private heap allocation range overflowed x86 accounting",
                    )
                })?;
            let capacity = u32::try_from(backing.range.byte_length()).map_err(|_| {
                RuntimeStateError::runtime_defect(
                    "runtime-heap-backing",
                    "heap-backing-capacity-not-representable",
                    "private heap backing capacity does not fit x86 accounting",
                )
            })?;
            if allocation_end > capacity {
                return Err(RuntimeStateError::resource_limit(
                    "runtime-heap-backing",
                    "heap-backing-capacity-exceeded",
                    format!(
                        "allocation needs {allocation_end:#x} bytes in a {capacity:#x}-byte heap backing"
                    ),
                ));
            }
            let required_commit = align_runtime_heap_value(
                allocation_end,
                GUEST_PAGE_SIZE as u32,
                "heap-commit-size-overflow",
            )?;
            (
                RuntimeHeapBackingPlan::Existing {
                    memory_owner: backing.memory_owner,
                    range: backing.range,
                    committed_bytes: backing.committed_bytes,
                },
                allocation_id,
                allocation_offset,
                source_free_block,
                backing.high_water_mark,
                backing.committed_bytes.max(required_commit),
            )
        } else {
            if heap.live_allocations != 0 {
                return Err(RuntimeStateError::runtime_defect(
                    "runtime-heap-backing",
                    "heap-allocation-count-without-backing",
                    "logical heap has live allocations but no guest-memory backing",
                ));
            }
            let slot = RuntimeHandle(handle).slot();
            if slot == 0 || slot > RUNTIME_PRIVATE_HEAP_BACKING_LIMIT {
                return Err(RuntimeStateError::resource_limit(
                    "runtime-heap-backing",
                    "heap-backing-slot-limit-exceeded",
                    format!(
                        "heap handle slot {slot} exceeds the {}-backing controlled arena",
                        RUNTIME_PRIVATE_HEAP_BACKING_LIMIT
                    ),
                ));
            }
            let slot_offset = u32::from(slot - 1)
                .checked_mul(RUNTIME_PRIVATE_HEAP_ARENA_STRIDE)
                .ok_or_else(|| {
                    RuntimeStateError::resource_limit(
                        "runtime-heap-backing",
                        "heap-backing-address-overflow",
                        "private heap arena slot offset overflowed x86 accounting",
                    )
                })?;
            let base = RUNTIME_PRIVATE_HEAP_ARENA_BASE
                .0
                .checked_add(slot_offset)
                .map(GuestAddress)
                .ok_or_else(|| {
                    RuntimeStateError::resource_limit(
                        "runtime-heap-backing",
                        "heap-backing-address-overflow",
                        "private heap arena base overflowed x86 accounting",
                    )
                })?;
            let capacity = if heap.maximum_size == 0 {
                RUNTIME_PRIVATE_HEAP_ARENA_STRIDE
            } else {
                align_runtime_heap_value(
                    heap.maximum_size,
                    GUEST_PAGE_SIZE as u32,
                    "heap-maximum-size-overflow",
                )?
            };
            if capacity > RUNTIME_PRIVATE_HEAP_ARENA_STRIDE {
                return Err(RuntimeStateError::resource_limit(
                    "runtime-heap-backing",
                    "heap-backing-capacity-policy-exceeded",
                    format!(
                        "heap requests {capacity:#x} bytes; the controlled per-heap limit is {:#x}",
                        RUNTIME_PRIVATE_HEAP_ARENA_STRIDE
                    ),
                ));
            }
            let initial_committed_bytes = if heap.initial_size == 0 {
                0
            } else {
                align_runtime_heap_value(
                    heap.initial_size,
                    GUEST_PAGE_SIZE as u32,
                    "heap-initial-size-overflow",
                )?
            };
            if initial_committed_bytes > capacity {
                return Err(RuntimeStateError::invalid_input(
                    "runtime-heap-backing",
                    "heap-initial-size-exceeds-maximum",
                    "private heap initial size exceeds its bounded maximum size",
                ));
            }
            if reserved_byte_length > capacity {
                return Err(RuntimeStateError::resource_limit(
                    "runtime-heap-backing",
                    "heap-backing-capacity-exceeded",
                    "private heap allocation exceeds the controlled backing capacity",
                ));
            }
            let range = GuestRange::new(base, u64::from(capacity))
                .map_err(RuntimeStateError::from_memory)?;
            let required_commit = align_runtime_heap_value(
                reserved_byte_length,
                GUEST_PAGE_SIZE as u32,
                "heap-commit-size-overflow",
            )?;
            (
                RuntimeHeapBackingPlan::New {
                    range,
                    initial_committed_bytes,
                },
                RuntimeHeapAllocationId(1),
                0,
                None,
                0,
                initial_committed_bytes.max(required_commit),
            )
        };

        let address = backing_plan
            .range()
            .start()
            .0
            .checked_add(allocation_offset)
            .map(GuestAddress)
            .ok_or_else(|| {
                RuntimeStateError::resource_limit(
                    "runtime-heap-backing",
                    "heap-allocation-address-overflow",
                    "private heap allocation address overflowed x86 accounting",
                )
            })?;
        Ok(RuntimeHeapAllocationPlan {
            handle,
            allocation: RuntimeHeapAllocation {
                id: allocation_id,
                address,
                byte_length,
                reserved_byte_length,
                initialization,
            },
            allocation_offset,
            backing: backing_plan,
            source_free_block,
            previous_high_water_mark,
            next_committed_bytes,
        })
    }

    pub(crate) fn commit_private_heap_allocation(
        &mut self,
        plan: RuntimeHeapAllocationPlan,
        new_memory_owner: Option<MemoryOwnerId>,
    ) -> Result<RuntimeHeapAllocation, RuntimeStateError> {
        let mut next_heap = self.heap(plan.handle)?.clone();
        next_heap.live_allocations =
            next_heap.live_allocations.checked_add(1).ok_or_else(|| {
                RuntimeStateError::resource_limit(
                    "runtime-heap-backing",
                    "heap-live-allocation-count-overflow",
                    "private heap live allocation count overflowed",
                )
            })?;
        let allocation_end = plan
            .allocation_offset
            .checked_add(plan.allocation.reserved_byte_length)
            .ok_or_else(|| {
                RuntimeStateError::runtime_defect(
                    "runtime-heap-backing",
                    "planned-heap-allocation-range-overflow",
                    "planned private heap allocation range overflowed",
                )
            })?;
        let next_allocation_id =
            RuntimeHeapAllocationId(plan.allocation.id.0.checked_add(1).ok_or_else(|| {
                RuntimeStateError::resource_limit(
                    "runtime-heap-backing",
                    "heap-allocation-id-space-exhausted",
                    "private heap allocation identifier space is exhausted",
                )
            })?);

        match plan.backing {
            RuntimeHeapBackingPlan::New {
                range,
                initial_committed_bytes: _,
            } => {
                if self.heap_backings.contains_key(&plan.handle) {
                    return Err(RuntimeStateError::conflict(
                        "runtime-heap-backing",
                        "heap-backing-plan-stale",
                        "private heap backing appeared after allocation planning",
                    ));
                }
                let memory_owner = new_memory_owner.ok_or_else(|| {
                    RuntimeStateError::runtime_defect(
                        "runtime-heap-backing",
                        "heap-backing-memory-owner-missing",
                        "new private heap backing has no guest-memory owner",
                    )
                })?;
                let mut allocations = BTreeMap::new();
                allocations.insert(plan.allocation.address.0, plan.allocation);
                self.heap_backings.insert(
                    plan.handle,
                    RuntimeHeapBacking {
                        memory_owner,
                        range,
                        alignment: RUNTIME_PRIVATE_HEAP_ALIGNMENT,
                        committed_bytes: plan.next_committed_bytes,
                        high_water_mark: allocation_end,
                        next_allocation_id,
                        allocations,
                        free_blocks: BTreeMap::new(),
                    },
                );
            }
            RuntimeHeapBackingPlan::Existing {
                memory_owner,
                range,
                committed_bytes,
            } => {
                if new_memory_owner.is_some() {
                    return Err(RuntimeStateError::runtime_defect(
                        "runtime-heap-backing",
                        "heap-backing-memory-owner-unexpected",
                        "existing private heap allocation supplied a new memory owner",
                    ));
                }
                let backing = self.heap_backings.get(&plan.handle).ok_or_else(|| {
                    RuntimeStateError::conflict(
                        "runtime-heap-backing",
                        "heap-backing-plan-stale",
                        "private heap backing disappeared after allocation planning",
                    )
                })?;
                if backing.memory_owner != memory_owner
                    || backing.range != range
                    || backing.committed_bytes != committed_bytes
                    || backing.high_water_mark != plan.previous_high_water_mark
                    || backing.next_allocation_id != plan.allocation.id
                    || backing.allocations.contains_key(&plan.allocation.address.0)
                    || plan.source_free_block.is_some_and(|block| {
                        backing.free_blocks.get(&block.0).copied() != Some(block.1)
                    })
                {
                    return Err(RuntimeStateError::conflict(
                        "runtime-heap-backing",
                        "heap-allocation-plan-stale",
                        "private heap allocation metadata changed after planning",
                    ));
                }
                let backing = self.heap_backings.get_mut(&plan.handle).ok_or_else(|| {
                    RuntimeStateError::runtime_defect(
                        "runtime-heap-backing",
                        "preflighted-heap-backing-missing",
                        "preflighted private heap backing disappeared",
                    )
                })?;
                if let Some((free_offset, free_length)) = plan.source_free_block {
                    backing.free_blocks.remove(&free_offset);
                    let remaining = free_length - plan.allocation.reserved_byte_length;
                    if remaining != 0 {
                        backing.free_blocks.insert(allocation_end, remaining);
                    }
                } else {
                    backing.high_water_mark = allocation_end;
                }
                backing.committed_bytes = plan.next_committed_bytes;
                backing.next_allocation_id = next_allocation_id;
                backing
                    .allocations
                    .insert(plan.allocation.address.0, plan.allocation);
            }
        }
        self.process.heaps.insert(plan.handle, next_heap);
        Ok(plan.allocation)
    }

    pub(crate) fn plan_private_heap_free(
        &self,
        handle: u32,
        address: GuestAddress,
    ) -> Result<RuntimeHeapFreePlan, RuntimeStateError> {
        self.require_private_heap_operation_state()?;
        let heap = self.heap(handle)?;
        let backing = self.heap_backings.get(&handle).ok_or_else(|| {
            RuntimeStateError::not_found(
                "runtime-heap-backing",
                "heap-allocation-pointer-invalid",
                "private heap has no backing containing the requested pointer",
            )
        })?;
        validate_runtime_heap_backing(heap, backing)?;
        let allocation = match backing.allocations.get(&address.0).copied() {
            Some(allocation) => allocation,
            None => {
                let offset = address.0.checked_sub(backing.range.start().0);
                return Err(
                    if offset.is_some_and(|offset| backing.free_blocks.contains_key(&offset)) {
                        RuntimeStateError::conflict(
                            "runtime-heap-backing",
                            "heap-allocation-double-free",
                            "private heap pointer belongs to an already freed block",
                        )
                    } else {
                        RuntimeStateError::not_found(
                            "runtime-heap-backing",
                            "heap-allocation-pointer-invalid",
                            "private heap pointer does not identify a live allocation",
                        )
                    },
                );
            }
        };
        let offset = allocation
            .address
            .0
            .checked_sub(backing.range.start().0)
            .ok_or_else(|| {
                RuntimeStateError::runtime_defect(
                    "runtime-heap-backing",
                    "heap-allocation-outside-backing",
                    "live private heap allocation precedes its backing range",
                )
            })?;
        let allocation_end = offset
            .checked_add(allocation.reserved_byte_length)
            .ok_or_else(|| {
                RuntimeStateError::runtime_defect(
                    "runtime-heap-backing",
                    "heap-allocation-range-overflow",
                    "live private heap allocation range overflowed",
                )
            })?;
        let predecessor = backing
            .free_blocks
            .range(..offset)
            .next_back()
            .map(|(start, length)| (*start, *length));
        let successor = backing
            .free_blocks
            .range(offset..)
            .next()
            .map(|(start, length)| (*start, *length));
        if predecessor
            .is_some_and(|(start, length)| start.checked_add(length).is_none_or(|end| end > offset))
            || successor.is_some_and(|(start, _)| start < allocation_end)
        {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-heap-backing",
                "heap-free-block-overlap",
                "private heap free blocks overlap a live allocation",
            ));
        }
        let merge_predecessor =
            predecessor.filter(|(start, length)| start.checked_add(*length) == Some(offset));
        let merge_successor = successor.filter(|(start, _)| *start == allocation_end);
        let merged_start = merge_predecessor.map(|(start, _)| start).unwrap_or(offset);
        let merged_length = allocation
            .reserved_byte_length
            .checked_add(merge_predecessor.map(|(_, length)| length).unwrap_or(0))
            .and_then(|length| {
                length.checked_add(merge_successor.map(|(_, length)| length).unwrap_or(0))
            })
            .ok_or_else(|| {
                RuntimeStateError::runtime_defect(
                    "runtime-heap-backing",
                    "heap-free-block-size-overflow",
                    "coalesced private heap free block overflowed x86 accounting",
                )
            })?;
        Ok(RuntimeHeapFreePlan {
            handle,
            allocation,
            memory_owner: backing.memory_owner,
            merged_free_block: (merged_start, merged_length),
            predecessor: merge_predecessor,
            successor: merge_successor,
        })
    }

    pub(crate) fn commit_private_heap_free(
        &mut self,
        plan: RuntimeHeapFreePlan,
    ) -> Result<RuntimeHeapAllocation, RuntimeStateError> {
        let mut next_heap = self.heap(plan.handle)?.clone();
        if next_heap.live_allocations == 0 {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-heap-backing",
                "heap-live-allocation-count-underflow",
                "private heap free would underflow its live allocation count",
            ));
        }
        let backing = self.heap_backings.get(&plan.handle).ok_or_else(|| {
            RuntimeStateError::conflict(
                "runtime-heap-backing",
                "heap-free-plan-stale",
                "private heap backing disappeared after free planning",
            )
        })?;
        if backing.memory_owner != plan.memory_owner
            || backing.allocations.get(&plan.allocation.address.0) != Some(&plan.allocation)
            || plan
                .predecessor
                .is_some_and(|block| backing.free_blocks.get(&block.0).copied() != Some(block.1))
            || plan
                .successor
                .is_some_and(|block| backing.free_blocks.get(&block.0).copied() != Some(block.1))
        {
            return Err(RuntimeStateError::conflict(
                "runtime-heap-backing",
                "heap-free-plan-stale",
                "private heap allocation metadata changed after free planning",
            ));
        }
        let backing = self.heap_backings.get_mut(&plan.handle).ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-heap-backing",
                "preflighted-heap-backing-missing",
                "preflighted private heap backing disappeared before free commit",
            )
        })?;
        backing.allocations.remove(&plan.allocation.address.0);
        if let Some((offset, _)) = plan.predecessor {
            backing.free_blocks.remove(&offset);
        }
        if let Some((offset, _)) = plan.successor {
            backing.free_blocks.remove(&offset);
        }
        backing
            .free_blocks
            .insert(plan.merged_free_block.0, plan.merged_free_block.1);
        next_heap.live_allocations -= 1;
        self.process.heaps.insert(plan.handle, next_heap);
        Ok(plan.allocation)
    }

    pub(crate) fn plan_private_heap_destruction(
        &self,
        handle: u32,
    ) -> Result<RuntimeHeapDestructionPlan, RuntimeStateError> {
        self.require_private_heap_operation_state()?;
        self.plan_private_heap_destruction_unchecked(handle)
    }

    pub(crate) fn plan_process_heap_teardown(
        &self,
    ) -> Result<Vec<RuntimeHeapDestructionPlan>, RuntimeStateError> {
        if !matches!(
            self.process.lifecycle,
            RuntimeProcessLifecycleState::ExitRequested { .. }
        ) {
            return Err(RuntimeStateError::conflict(
                "runtime-heap-backing",
                "process-heap-teardown-state-invalid",
                "process heap teardown requires an active process exit request",
            ));
        }
        self.process
            .heaps
            .keys()
            .copied()
            .map(|handle| self.plan_private_heap_destruction_unchecked(handle))
            .collect()
    }

    fn plan_private_heap_destruction_unchecked(
        &self,
        handle: u32,
    ) -> Result<RuntimeHeapDestructionPlan, RuntimeStateError> {
        let heap = self.heap(handle)?;
        let backing = self.heap_backings.get(&handle);
        if let Some(backing) = backing {
            validate_runtime_heap_backing(heap, backing)?;
        } else if heap.live_allocations != 0 {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-heap-backing",
                "heap-allocation-count-without-backing",
                "logical heap has live allocations but no guest-memory backing",
            ));
        }
        Ok(RuntimeHeapDestructionPlan {
            handle,
            backing: backing
                .map(|backing| (backing.memory_owner, backing.range, backing.committed_bytes)),
            live_allocations: backing.map_or(0, |backing| backing.allocations.len()),
        })
    }

    pub(crate) fn commit_private_heap_destruction(
        &mut self,
        plans: &[RuntimeHeapDestructionPlan],
    ) -> Result<RuntimeHeapTeardown, RuntimeStateError> {
        let mut handles = Vec::with_capacity(plans.len());
        let mut allocations_discarded = 0usize;
        let mut reserved_bytes_released = 0u64;
        let mut committed_bytes_released = 0u64;
        for plan in plans {
            let heap = self.heap(plan.handle)?;
            let backing = self.heap_backings.get(&plan.handle);
            let actual_backing = backing
                .map(|backing| (backing.memory_owner, backing.range, backing.committed_bytes));
            if actual_backing != plan.backing
                || backing.map_or(0, |backing| backing.allocations.len()) != plan.live_allocations
                || usize::try_from(heap.live_allocations).ok() != Some(plan.live_allocations)
            {
                return Err(RuntimeStateError::conflict(
                    "runtime-heap-backing",
                    "heap-destruction-plan-stale",
                    "private heap metadata changed after destruction planning",
                ));
            }
            handles.push(plan.handle);
            allocations_discarded = allocations_discarded
                .checked_add(plan.live_allocations)
                .ok_or_else(|| {
                    RuntimeStateError::resource_limit(
                        "runtime-heap-backing",
                        "heap-teardown-allocation-count-overflow",
                        "private heap teardown allocation count overflowed host accounting",
                    )
                })?;
            if let Some((_, range, committed_bytes)) = plan.backing {
                reserved_bytes_released = reserved_bytes_released
                    .checked_add(range.byte_length())
                    .ok_or_else(|| {
                        RuntimeStateError::resource_limit(
                            "runtime-heap-backing",
                            "heap-teardown-reserved-byte-count-overflow",
                            "private heap teardown reserved bytes overflowed host accounting",
                        )
                    })?;
                committed_bytes_released = committed_bytes_released
                    .checked_add(u64::from(committed_bytes))
                    .ok_or_else(|| {
                        RuntimeStateError::resource_limit(
                            "runtime-heap-backing",
                            "heap-teardown-committed-byte-count-overflow",
                            "private heap teardown committed bytes overflowed host accounting",
                        )
                    })?;
            }
        }
        let keys: Vec<_> = handles
            .iter()
            .map(|handle| RuntimeObjectKey::PrivateHeap { handle: *handle })
            .collect();
        self.objects.retire_subsystem_objects(&keys)?;
        for handle in &handles {
            self.heap_backings.remove(handle);
            self.process.heaps.remove(handle);
        }
        Ok(RuntimeHeapTeardown {
            handles,
            allocations_discarded,
            reserved_bytes_released,
            committed_bytes_released,
        })
    }

    fn require_private_heap_operation_state(&self) -> Result<(), RuntimeStateError> {
        if self.process.lifecycle != RuntimeProcessLifecycleState::Running
            || self.current_thread()?.lifecycle != RuntimeThreadLifecycleState::Running
        {
            return Err(RuntimeStateError::conflict(
                "runtime-heap-backing",
                "heap-operation-lifecycle-state-invalid",
                "private heap allocation operations require a running process and thread",
            ));
        }
        Ok(())
    }

    pub fn load_module(
        &mut self,
        module: RuntimeModule,
    ) -> Result<&RuntimeModule, RuntimeStateError> {
        let class = if module.runtime_system_module {
            RuntimeModuleClass::RuntimeSystemModule
        } else {
            RuntimeModuleClass::GuestDynamicLibrary
        };
        self.register_module_record(module, class)
    }

    pub fn register_main_module(
        &mut self,
        module: RuntimeModule,
    ) -> Result<&RuntimeModule, RuntimeStateError> {
        if module.runtime_system_module {
            return Err(RuntimeStateError::invalid_input(
                "runtime-module",
                "main-module-class-invalid",
                "main executable cannot be registered as a runtime system module",
            ));
        }
        self.register_module_record(module, RuntimeModuleClass::MainExecutable)
    }

    fn register_module_record(
        &mut self,
        module: RuntimeModule,
        class: RuntimeModuleClass,
    ) -> Result<&RuntimeModule, RuntimeStateError> {
        self.require_dll_operation_inactive_for_module_graph_mutation()?;
        if module.owner.is_empty()
            || module.variant_id.is_empty()
            || module.namespace != LOGICAL_LOADED_MODULE_NAMESPACE
            || module.handle == 0
            || module.canonical_name.is_empty()
        {
            return Err(RuntimeStateError::invalid_input(
                "runtime-module",
                "module-identity-invalid",
                "runtime module requires controlled owner, variant, namespace, handle, and name",
            ));
        }
        if self.process.modules.contains_key(&module.handle)
            || self.process.modules.values().any(|loaded| {
                loaded
                    .canonical_name
                    .eq_ignore_ascii_case(&module.canonical_name)
            })
        {
            return Err(RuntimeStateError::conflict(
                "runtime-module",
                "module-identity-duplicate",
                "runtime module handle or canonical name is already loaded",
            ));
        }
        let handle = module.handle;
        let graph_id = self
            .module_graph
            .register_module(RuntimeModuleRegistration {
                handle,
                requested_name: module.requested_name.clone(),
                canonical_name: module.canonical_name.clone(),
                class,
            })
            .map_err(runtime_module_graph_error)?;
        if let Err(error) = self
            .objects
            .register_object_with_handle(RuntimeObjectKey::Module { handle }, RuntimeHandle(handle))
        {
            self.module_graph
                .rollback_registration(graph_id)
                .map_err(runtime_module_graph_error)?;
            return Err(error);
        }
        self.process.modules.insert(handle, module);
        self.process.modules.get(&handle).ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-module",
                "inserted-module-missing",
                "runtime module disappeared after insertion",
            )
        })
    }

    pub fn module(&self, handle: u32) -> Result<&RuntimeModule, RuntimeStateError> {
        self.objects
            .object_for_handle(RuntimeHandle(handle), RuntimeObjectKind::Module)?;
        self.process.modules.get(&handle).ok_or_else(|| {
            RuntimeStateError::not_found(
                "runtime-module",
                "module-handle-unknown",
                format!("logical module handle {handle:#010x} is not loaded"),
            )
        })
    }

    pub fn module_by_name(&self, name: &str) -> Result<&RuntimeModule, RuntimeStateError> {
        let handle = self
            .module_graph
            .module_by_name(name)
            .map_err(runtime_module_graph_error)?
            .handle;
        self.module(handle)
    }

    pub fn add_module_dependency(
        &mut self,
        requester_handle: u32,
        dependency_handle: u32,
    ) -> Result<(), RuntimeStateError> {
        self.require_dll_operation_inactive_for_module_graph_mutation()?;
        self.module(requester_handle)?;
        self.module(dependency_handle)?;
        let requester = self
            .module_graph
            .module_by_handle(requester_handle)
            .map_err(runtime_module_graph_error)?
            .id;
        let dependency = self
            .module_graph
            .module_by_handle(dependency_handle)
            .map_err(runtime_module_graph_error)?
            .id;
        self.module_graph
            .add_dependency(requester, dependency)
            .map_err(runtime_module_graph_error)
    }

    pub fn add_current_thread_module_reference(
        &mut self,
        handle: u32,
    ) -> Result<u32, RuntimeStateError> {
        self.require_dll_operation_inactive_for_module_graph_mutation()?;
        self.module(handle)?;
        let module = self
            .module_graph
            .module_by_handle(handle)
            .map_err(runtime_module_graph_error)?
            .id;
        self.module_graph
            .add_dynamic_reference(module, self.current_thread_id().0)
            .map_err(runtime_module_graph_error)
    }

    pub fn release_current_thread_module_reference(
        &mut self,
        handle: u32,
    ) -> Result<u32, RuntimeStateError> {
        self.require_dll_operation_inactive_for_module_graph_mutation()?;
        self.module(handle)?;
        let module = self
            .module_graph
            .module_by_handle(handle)
            .map_err(runtime_module_graph_error)?
            .id;
        self.module_graph
            .release_dynamic_reference(module, self.current_thread_id().0)
            .map_err(runtime_module_graph_error)
    }

    pub fn mark_module_ready(&mut self, handle: u32) -> Result<(), RuntimeStateError> {
        self.require_dll_operation_inactive_for_module_graph_mutation()?;
        self.module(handle)?;
        let module = self
            .module_graph
            .module_by_handle(handle)
            .map_err(runtime_module_graph_error)?
            .id;
        self.module_graph
            .mark_ready(module)
            .map_err(runtime_module_graph_error)
    }

    pub fn attach_modules(&mut self) -> Result<Vec<RuntimeModuleId>, RuntimeStateError> {
        if self.dll_lifecycle_phase != RuntimeDllLifecyclePhase::Idle {
            return Err(RuntimeStateError::conflict(
                "runtime-dll-lifecycle",
                "direct-module-attach-after-dll-lifecycle-forbidden",
                "direct module attach cannot bypass the active DLL lifecycle coordinator",
            ));
        }
        self.require_dll_operation_inactive_for_module_graph_mutation()?;
        self.module_graph
            .attach_all()
            .map_err(runtime_module_graph_error)
    }

    pub fn detach_modules(&mut self) -> Result<Vec<RuntimeModuleId>, RuntimeStateError> {
        if self.dll_lifecycle_phase != RuntimeDllLifecyclePhase::Idle {
            return Err(RuntimeStateError::conflict(
                "runtime-dll-lifecycle",
                "direct-module-detach-after-dll-lifecycle-forbidden",
                "direct module detach cannot bypass the active DLL lifecycle coordinator",
            ));
        }
        self.require_dll_operation_inactive_for_module_graph_mutation()?;
        self.module_graph
            .detach_all()
            .map_err(runtime_module_graph_error)
    }

    fn require_dll_operation_inactive_for_module_graph_mutation(
        &self,
    ) -> Result<(), RuntimeStateError> {
        if self.dll_operation.is_some() {
            return Err(RuntimeStateError::conflict(
                "runtime-dll-lifecycle",
                "module-graph-mutation-during-dll-callback-forbidden",
                "module graph mutation is not supported while DLL callbacks are in progress",
            ));
        }
        Ok(())
    }

    pub fn dll_lifecycle_phase(&self) -> RuntimeDllLifecyclePhase {
        self.dll_lifecycle_phase
    }

    pub fn begin_dll_process_attach(
        &mut self,
        return_sentinel: GuestAddress,
    ) -> Result<RuntimeDllLifecycleAction, RuntimeStateError> {
        if self.dll_lifecycle_phase != RuntimeDllLifecyclePhase::Idle
            || self.dll_operation.is_some()
        {
            return Err(RuntimeStateError::conflict(
                "runtime-dll-lifecycle",
                "dll-process-attach-state-invalid",
                "DLL process attach requires an idle lifecycle coordinator",
            ));
        }
        self.require_dll_callback_thread_state()?;
        validate_dll_return_sentinel(return_sentinel)?;
        let graph_order = self
            .module_graph
            .planned_attach_order()
            .map_err(runtime_module_graph_error)?;
        let callback_order = self.dll_callback_order(&graph_order)?;
        self.dll_lifecycle_phase = RuntimeDllLifecyclePhase::ProcessAttaching;
        self.dll_operation = Some(RuntimeDllOperation {
            kind: RuntimeDllOperationKind::Attach,
            graph_order,
            remaining: callback_order.into(),
            completed: Vec::new(),
            pending: None,
            attach_failure: None,
            return_sentinel,
        });
        match self.schedule_next_dll_callback() {
            Ok(action) => Ok(action),
            Err(error) => {
                self.dll_operation = None;
                self.dll_lifecycle_phase = RuntimeDllLifecyclePhase::Idle;
                Err(error)
            }
        }
    }

    pub fn begin_dll_process_detach(
        &mut self,
        return_sentinel: GuestAddress,
    ) -> Result<RuntimeDllLifecycleAction, RuntimeStateError> {
        if self.dll_lifecycle_phase != RuntimeDllLifecyclePhase::ProcessAttached
            || self.dll_operation.is_some()
        {
            return Err(RuntimeStateError::conflict(
                "runtime-dll-lifecycle",
                "dll-process-detach-state-invalid",
                "DLL process detach requires a completed process attach",
            ));
        }
        self.require_dll_callback_thread_state()?;
        validate_dll_return_sentinel(return_sentinel)?;
        let graph_order = self
            .module_graph
            .planned_detach_order()
            .map_err(runtime_module_graph_error)?;
        let callback_order = self.dll_callback_order(&graph_order)?;
        self.dll_lifecycle_phase = RuntimeDllLifecyclePhase::ProcessDetaching;
        self.dll_operation = Some(RuntimeDllOperation {
            kind: RuntimeDllOperationKind::Detach,
            graph_order,
            remaining: callback_order.into(),
            completed: Vec::new(),
            pending: None,
            attach_failure: None,
            return_sentinel,
        });
        match self.schedule_next_dll_callback() {
            Ok(action) => Ok(action),
            Err(error) => {
                self.dll_operation = None;
                self.dll_lifecycle_phase = RuntimeDllLifecyclePhase::ProcessAttached;
                Err(error)
            }
        }
    }

    pub fn complete_dll_callback(
        &mut self,
        frame_id: RuntimeCallbackFrameId,
        result: u32,
    ) -> Result<RuntimeDllLifecycleAction, RuntimeStateError> {
        let pending = self
            .dll_operation
            .as_ref()
            .and_then(|operation| operation.pending.as_ref())
            .ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-dll-lifecycle",
                    "dll-callback-not-pending",
                    "DLL lifecycle coordinator has no pending callback",
                )
            })?;
        if pending.frame.id != frame_id {
            return Err(RuntimeStateError::conflict(
                "runtime-dll-lifecycle",
                "dll-callback-frame-mismatch",
                "DLL callback completion does not identify the pending callback frame",
            ));
        }
        self.preflight_dll_callback_completion(result)?;
        self.pop_callback_frame(frame_id)?;
        let mut operation = self.dll_operation.take().ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-dll-lifecycle",
                "dll-operation-disappeared",
                "validated DLL lifecycle operation disappeared before completion",
            )
        })?;
        let pending = operation.pending.take().ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-dll-lifecycle",
                "dll-pending-callback-disappeared",
                "validated DLL callback disappeared before completion",
            )
        })?;
        match operation.kind {
            RuntimeDllOperationKind::Attach if result == 0 => {
                operation.kind = RuntimeDllOperationKind::AttachUnwind;
                operation.attach_failure = Some((pending.module_id, pending.module_name));
                operation.remaining = operation.completed.iter().rev().copied().collect();
                operation.completed.clear();
                self.dll_lifecycle_phase = RuntimeDllLifecyclePhase::AttachUnwinding;
            }
            RuntimeDllOperationKind::Attach
            | RuntimeDllOperationKind::AttachUnwind
            | RuntimeDllOperationKind::Detach => {
                operation.completed.push(pending.module_id);
            }
        }
        self.dll_operation = Some(operation);
        self.schedule_next_dll_callback()
    }

    fn preflight_dll_callback_completion(&self, result: u32) -> Result<(), RuntimeStateError> {
        let operation = self.dll_operation.as_ref().ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-dll-lifecycle",
                "dll-operation-disappeared",
                "validated DLL lifecycle operation disappeared before completion preflight",
            )
        })?;
        if !operation.remaining.is_empty()
            || (operation.kind == RuntimeDllOperationKind::Attach && result == 0)
            || operation.kind == RuntimeDllOperationKind::AttachUnwind
        {
            return Ok(());
        }
        let current_order = match operation.kind {
            RuntimeDllOperationKind::Attach => self
                .module_graph
                .planned_attach_order()
                .map_err(runtime_module_graph_error)?,
            RuntimeDllOperationKind::Detach => self
                .module_graph
                .planned_detach_order()
                .map_err(runtime_module_graph_error)?,
            RuntimeDllOperationKind::AttachUnwind => {
                return Err(RuntimeStateError::runtime_defect(
                    "runtime-dll-lifecycle",
                    "dll-attach-unwind-finalization-preflight-invalid",
                    "attach unwind cannot commit module graph lifecycle state",
                ));
            }
        };
        if current_order != operation.graph_order {
            return Err(RuntimeStateError::conflict(
                "runtime-dll-lifecycle",
                "dll-lifecycle-graph-order-changed",
                "module graph order changed while DLL callbacks were in progress",
            ));
        }
        Ok(())
    }

    fn require_dll_callback_thread_state(&self) -> Result<(), RuntimeStateError> {
        if self.process.lifecycle != RuntimeProcessLifecycleState::Running
            || self.current_thread()?.lifecycle != RuntimeThreadLifecycleState::Running
            || self.scheduler_state()? != SchedulerThreadState::Runnable
        {
            return Err(RuntimeStateError::conflict(
                "runtime-dll-lifecycle",
                "dll-callback-thread-state-invalid",
                "DLL process callbacks require a running process and runnable selected thread",
            ));
        }
        Ok(())
    }

    fn dll_callback_order(
        &self,
        graph_order: &[RuntimeModuleId],
    ) -> Result<Vec<RuntimeModuleId>, RuntimeStateError> {
        let mut callbacks = Vec::new();
        for module_id in graph_order {
            let node = self
                .module_graph
                .module(*module_id)
                .map_err(runtime_module_graph_error)?;
            if matches!(
                node.class,
                RuntimeModuleClass::MainExecutable | RuntimeModuleClass::GuestDynamicLibrary
            ) {
                if !self.module_import_bindings.contains_key(module_id) {
                    return Err(RuntimeStateError::conflict(
                        "runtime-dll-lifecycle",
                        "dll-module-import-binding-missing",
                        format!(
                            "module {} has no committed import binding",
                            node.canonical_name
                        ),
                    ));
                }
                let image = self
                    .module_images
                    .image(*module_id)
                    .map_err(runtime_module_image_error)?;
                if node.class == RuntimeModuleClass::GuestDynamicLibrary
                    && image.entry_point.is_some()
                {
                    callbacks.push(*module_id);
                }
            }
        }
        Ok(callbacks)
    }

    fn schedule_next_dll_callback(
        &mut self,
    ) -> Result<RuntimeDllLifecycleAction, RuntimeStateError> {
        let mut operation = self.dll_operation.take().ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-dll-lifecycle",
                "dll-operation-missing",
                "DLL lifecycle scheduling requires an active operation",
            )
        })?;
        if operation.pending.is_some() {
            self.dll_operation = Some(operation);
            return Err(RuntimeStateError::conflict(
                "runtime-dll-lifecycle",
                "dll-callback-already-pending",
                "DLL lifecycle operation already has a pending callback",
            ));
        }
        let Some(module_id) = operation.remaining.pop_front() else {
            return self.finish_dll_operation(operation);
        };
        let reason = match operation.kind {
            RuntimeDllOperationKind::Attach => RuntimeDllCallbackReason::ProcessAttach,
            RuntimeDllOperationKind::AttachUnwind | RuntimeDllOperationKind::Detach => {
                RuntimeDllCallbackReason::ProcessDetach
            }
        };
        let request =
            match self.create_dll_callback_request(module_id, reason, operation.return_sentinel) {
                Ok(request) => request,
                Err(error) => {
                    operation.remaining.push_front(module_id);
                    self.dll_operation = Some(operation);
                    return Err(error);
                }
            };
        operation.pending = Some(request.clone());
        self.dll_operation = Some(operation);
        Ok(RuntimeDllLifecycleAction::Callback { request })
    }

    fn create_dll_callback_request(
        &mut self,
        module_id: RuntimeModuleId,
        reason: RuntimeDllCallbackReason,
        return_sentinel: GuestAddress,
    ) -> Result<RuntimeDllCallbackRequest, RuntimeStateError> {
        let node = self
            .module_graph
            .module(module_id)
            .map_err(runtime_module_graph_error)?;
        let image = self
            .module_images
            .image(module_id)
            .map_err(runtime_module_image_error)?;
        let entry_point = image.entry_point.ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-dll-lifecycle",
                "dll-callback-entry-point-missing",
                "DLL callback order includes a module without an entry point",
            )
        })?;
        let module_handle = node.handle;
        let module_name = node.canonical_name.clone();
        let module_base = image.load_base;
        let frame = self.push_callback_frame(entry_point, return_sentinel)?;
        Ok(RuntimeDllCallbackRequest {
            frame,
            module_id,
            module_handle,
            module_name,
            module_base,
            entry_point,
            reason,
            reserved: DLL_PROCESS_STATIC_RESERVED,
        })
    }

    fn finish_dll_operation(
        &mut self,
        operation: RuntimeDllOperation,
    ) -> Result<RuntimeDllLifecycleAction, RuntimeStateError> {
        match operation.kind {
            RuntimeDllOperationKind::Attach => {
                let graph_order = match self.module_graph.attach_all() {
                    Ok(graph_order) => graph_order,
                    Err(error) => {
                        self.dll_operation = Some(operation);
                        return Err(runtime_module_graph_error(error));
                    }
                };
                self.dll_lifecycle_phase = RuntimeDllLifecyclePhase::ProcessAttached;
                Ok(RuntimeDllLifecycleAction::Complete {
                    completion: RuntimeDllLifecycleCompletion {
                        phase: self.dll_lifecycle_phase,
                        graph_order,
                        callback_order: operation.completed,
                    },
                })
            }
            RuntimeDllOperationKind::AttachUnwind => {
                let (failed_module_id, failed_module) =
                    operation.attach_failure.ok_or_else(|| {
                        RuntimeStateError::runtime_defect(
                            "runtime-dll-lifecycle",
                            "dll-attach-failure-identity-missing",
                            "DLL attach unwind lost the failing module identity",
                        )
                    })?;
                self.dll_lifecycle_phase = RuntimeDllLifecyclePhase::Idle;
                Ok(RuntimeDllLifecycleAction::AttachFailed {
                    failure: RuntimeDllAttachFailure {
                        failed_module_id,
                        failed_module,
                        unwound_modules: operation.completed,
                        retryable: true,
                    },
                })
            }
            RuntimeDllOperationKind::Detach => {
                let graph_order = match self.module_graph.detach_all() {
                    Ok(graph_order) => graph_order,
                    Err(error) => {
                        self.dll_operation = Some(operation);
                        return Err(runtime_module_graph_error(error));
                    }
                };
                self.dll_lifecycle_phase = RuntimeDllLifecyclePhase::ProcessDetached;
                Ok(RuntimeDllLifecycleAction::Complete {
                    completion: RuntimeDllLifecycleCompletion {
                        phase: self.dll_lifecycle_phase,
                        graph_order,
                        callback_order: operation.completed,
                    },
                })
            }
        }
    }

    pub fn register_dynamic_endpoint(
        &mut self,
        endpoint: RuntimeDynamicEndpoint,
    ) -> Result<&RuntimeDynamicEndpoint, RuntimeStateError> {
        if endpoint.namespace != DYNAMIC_EXPORT_ENDPOINT_NAMESPACE
            || endpoint.id == 0
            || endpoint.module.is_empty()
            || endpoint.name.is_empty()
            || endpoint.lifetime.is_empty()
        {
            return Err(RuntimeStateError::invalid_input(
                "runtime-endpoint",
                "dynamic-endpoint-identity-invalid",
                "dynamic endpoint requires controlled namespace, ID, module, and name",
            ));
        }
        let slot_offset = u64::from(endpoint.id - 1)
            .checked_mul(u64::from(DYNAMIC_EXPORT_SLOT_SIZE))
            .ok_or_else(|| {
                RuntimeStateError::resource_limit(
                    "runtime-endpoint",
                    "dynamic-endpoint-slot-overflow",
                    "dynamic endpoint slot offset overflowed runtime accounting",
                )
            })?;
        let dynamic_exports = self.process.address_space.dynamic_exports;
        let expected_address = u64::from(dynamic_exports.base.0)
            .checked_add(slot_offset)
            .filter(|_| slot_offset < dynamic_exports.byte_length)
            .and_then(|address| u32::try_from(address).ok())
            .ok_or_else(|| {
                RuntimeStateError::invalid_input(
                    "runtime-endpoint",
                    "dynamic-endpoint-outside-arena",
                    "dynamic endpoint ID does not fit the process dynamic-export arena",
                )
            })?;
        if endpoint.address != GuestAddress(expected_address) {
            return Err(RuntimeStateError::invalid_input(
                "runtime-endpoint",
                "dynamic-endpoint-slot-mismatch",
                "dynamic endpoint address does not match its stable process-arena slot",
            ));
        }
        if self.registries.dynamic_endpoints.contains_key(&endpoint.id)
            || self
                .registries
                .dynamic_endpoints
                .values()
                .any(|registered| registered.address == endpoint.address)
        {
            return Err(RuntimeStateError::conflict(
                "runtime-endpoint",
                "dynamic-endpoint-duplicate",
                "dynamic endpoint ID or address is already registered",
            ));
        }
        let id = endpoint.id;
        self.registries.dynamic_endpoints.insert(id, endpoint);
        self.registries.dynamic_endpoints.get(&id).ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-endpoint",
                "inserted-dynamic-endpoint-missing",
                "dynamic endpoint disappeared after insertion",
            )
        })
    }

    pub fn register_dynamic_export_thunk(
        &mut self,
        thunk: &DynamicExportThunk,
    ) -> Result<&RuntimeDynamicEndpoint, RuntimeStateError> {
        self.register_dynamic_endpoint(RuntimeDynamicEndpoint::from(thunk))
    }

    pub fn dynamic_endpoint(&self, id: u32) -> Result<&RuntimeDynamicEndpoint, RuntimeStateError> {
        self.registries.dynamic_endpoints.get(&id).ok_or_else(|| {
            RuntimeStateError::not_found(
                "runtime-endpoint",
                "dynamic-endpoint-id-unknown",
                format!("dynamic endpoint ID {id} is not registered"),
            )
        })
    }

    pub fn dynamic_endpoint_at(
        &self,
        address: GuestAddress,
    ) -> Result<&RuntimeDynamicEndpoint, RuntimeStateError> {
        self.registries
            .dynamic_endpoints
            .values()
            .find(|endpoint| endpoint.address == address)
            .ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-endpoint",
                    "dynamic-endpoint-address-unknown",
                    format!(
                        "dynamic endpoint address {:#010x} is not registered",
                        address.0
                    ),
                )
            })
    }

    pub fn dynamic_endpoint_by_name(
        &self,
        module: &str,
        name: &str,
    ) -> Result<&RuntimeDynamicEndpoint, RuntimeStateError> {
        self.registries
            .dynamic_endpoints
            .values()
            .find(|endpoint| endpoint.module.eq_ignore_ascii_case(module) && endpoint.name == name)
            .ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-endpoint",
                    "dynamic-endpoint-name-unknown",
                    format!("dynamic endpoint {module}!{name} is not registered"),
                )
            })
    }

    pub fn allocate_tls_index(&mut self) -> Result<u32, RuntimeStateError> {
        if self.next_tls_index == TLS_OUT_OF_INDEXES {
            return Err(RuntimeStateError::resource_limit(
                "runtime-tls",
                "tls-index-space-exhausted",
                "logical TLS index space is exhausted",
            ));
        }
        let index = self.next_tls_index;
        if !self.process.allocated_tls_indices.insert(index) {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-tls",
                "tls-index-allocator-collision",
                "logical TLS allocator selected an already allocated index",
            ));
        }
        self.next_tls_index = self.next_tls_index.checked_add(1).ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-tls",
                "tls-index-counter-overflow",
                "logical TLS index counter overflowed",
            )
        })?;
        Ok(index)
    }

    pub fn allocate_fls_slot(
        &mut self,
        callback: Option<GuestAddress>,
    ) -> Result<&RuntimeFlsSlot, RuntimeStateError> {
        if self.process.lifecycle != RuntimeProcessLifecycleState::Running
            || self.current_thread()?.lifecycle != RuntimeThreadLifecycleState::Running
            || !matches!(
                self.scheduler_state()?,
                SchedulerThreadState::Runnable | SchedulerThreadState::InCallback { .. }
            )
        {
            return Err(RuntimeStateError::conflict(
                "runtime-fls",
                "fls-allocation-lifecycle-state-invalid",
                "FLS allocation requires a running process and active current thread",
            ));
        }
        if self.next_fls_index >= RUNTIME_FLS_INDEX_LIMIT {
            return Err(RuntimeStateError::resource_limit(
                "runtime-fls",
                "fls-index-space-exhausted",
                format!("logical FLS allocation is limited to {RUNTIME_FLS_INDEX_LIMIT} slots"),
            ));
        }
        let index = self.next_fls_index;
        if self.process.fls_slots.contains_key(&index) {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-fls",
                "fls-index-allocator-collision",
                "logical FLS allocator selected an already allocated index",
            ));
        }
        self.next_fls_index = index.checked_add(1).ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-fls",
                "fls-index-counter-overflow",
                "logical FLS index counter overflowed",
            )
        })?;
        self.process.fls_slots.insert(
            index,
            RuntimeFlsSlot {
                namespace: LOGICAL_FLS_INDEX_NAMESPACE.to_owned(),
                index,
                callback,
            },
        );
        self.process.fls_slots.get(&index).ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-fls",
                "inserted-fls-slot-missing",
                "logical FLS slot disappeared after insertion",
            )
        })
    }

    pub fn fls_slot(&self, index: u32) -> Result<&RuntimeFlsSlot, RuntimeStateError> {
        self.process.fls_slots.get(&index).ok_or_else(|| {
            RuntimeStateError::not_found(
                "runtime-fls",
                "fls-index-unallocated",
                format!("logical FLS index {index} is not allocated"),
            )
        })
    }

    pub fn set_fls_value(
        &mut self,
        index: u32,
        value: u32,
    ) -> Result<Option<u32>, RuntimeStateError> {
        if self.process.lifecycle != RuntimeProcessLifecycleState::Running
            || self.current_thread()?.lifecycle != RuntimeThreadLifecycleState::Running
            || !matches!(
                self.scheduler_state()?,
                SchedulerThreadState::Runnable | SchedulerThreadState::InCallback { .. }
            )
        {
            return Err(RuntimeStateError::conflict(
                "runtime-fls",
                "fls-value-lifecycle-state-invalid",
                "FLS value storage requires a running process and active current thread",
            ));
        }
        if !self.process.fls_slots.contains_key(&index) {
            return Err(RuntimeStateError::not_found(
                "runtime-fls",
                "fls-index-unallocated",
                format!("logical FLS index {index} is not allocated"),
            ));
        }
        Ok(self.current_thread_mut()?.fls_values.insert(index, value))
    }

    pub fn fls_value(&self, index: u32) -> Result<u32, RuntimeStateError> {
        self.fls_slot(index)?;
        self.current_thread()?
            .fls_values
            .get(&index)
            .copied()
            .ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-fls",
                    "fls-value-uninitialized",
                    format!("logical FLS index {index} has no value for the current thread"),
                )
            })
    }

    pub fn set_tls_value(&mut self, index: u32, value: u32) -> Result<(), RuntimeStateError> {
        if !self.process.allocated_tls_indices.contains(&index) {
            return Err(RuntimeStateError::not_found(
                "runtime-tls",
                "tls-index-unallocated",
                format!("logical TLS index {index} is not allocated"),
            ));
        }
        let tls_value = self
            .dynamic_endpoint_at(GuestAddress(value))
            .map(|endpoint| RuntimeTlsValue::DynamicEndpoint {
                endpoint_id: endpoint.id,
                address: endpoint.address,
            })
            .unwrap_or(RuntimeTlsValue::Raw { value });
        self.current_thread_mut()?
            .tls_values
            .insert(index, tls_value);
        Ok(())
    }

    pub fn tls_value(&self, index: u32) -> Result<&RuntimeTlsValue, RuntimeStateError> {
        self.current_thread()?
            .tls_values
            .get(&index)
            .ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-tls",
                    "tls-value-uninitialized",
                    format!("current thread has no value for logical TLS index {index}"),
                )
            })
    }

    pub fn initialized_tls_thread_count(&self, index: u32) -> usize {
        self.threads
            .values()
            .filter(|thread| thread.tls_values.contains_key(&index))
            .count()
    }

    pub fn initialize_critical_section(
        &mut self,
        address: GuestAddress,
        spin_count: u32,
    ) -> Result<&RuntimeCriticalSection, RuntimeStateError> {
        self.require_critical_section_operation_state()?;
        if address.0 == 0 || !address.0.is_multiple_of(4) {
            return Err(RuntimeStateError::invalid_input(
                "runtime-critical-section",
                "critical-section-address-invalid",
                "critical section storage must be a nonzero 4-byte-aligned guest address",
            ));
        }
        if spin_count > 0x00ff_ffff {
            return Err(RuntimeStateError::invalid_input(
                "runtime-critical-section",
                "critical-section-spin-count-invalid",
                "critical section spin count exceeds the Win32 24-bit range",
            ));
        }
        if self.process.critical_sections.contains_key(&address) {
            return Err(RuntimeStateError::conflict(
                "runtime-critical-section",
                "critical-section-already-initialized",
                format!(
                    "critical section storage {:#010x} is already initialized",
                    address.0
                ),
            ));
        }
        if self.process.critical_sections.len() >= RUNTIME_CRITICAL_SECTION_LIMIT {
            return Err(RuntimeStateError::resource_limit(
                "runtime-critical-section",
                "critical-section-limit-exceeded",
                "process critical section count exceeds the runtime bound",
            ));
        }
        self.process.critical_sections.insert(
            address,
            RuntimeCriticalSection {
                address,
                spin_count,
                owning_thread_id: None,
                recursion_count: 0,
            },
        );
        self.critical_section(address)
    }

    pub fn enter_critical_section(
        &mut self,
        address: GuestAddress,
    ) -> Result<&RuntimeCriticalSection, RuntimeStateError> {
        self.require_critical_section_operation_state()?;
        let current_thread_id = self.current_thread_id();
        let section = self
            .process
            .critical_sections
            .get_mut(&address)
            .ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-critical-section",
                    "critical-section-uninitialized",
                    format!(
                        "critical section storage {:#010x} is not initialized",
                        address.0
                    ),
                )
            })?;
        match section.owning_thread_id {
            None if section.recursion_count == 0 => {
                section.owning_thread_id = Some(current_thread_id);
                section.recursion_count = 1;
            }
            Some(owner) if owner == current_thread_id && section.recursion_count != 0 => {
                section.recursion_count =
                    section.recursion_count.checked_add(1).ok_or_else(|| {
                        RuntimeStateError::resource_limit(
                            "runtime-critical-section",
                            "critical-section-recursion-overflow",
                            "critical section recursion count overflowed",
                        )
                    })?;
            }
            Some(owner) if owner != current_thread_id && section.recursion_count != 0 => {
                return Err(RuntimeStateError::conflict(
                    "runtime-critical-section",
                    "critical-section-contended",
                    format!(
                        "critical section {:#010x} is owned by thread {}",
                        address.0, owner.0
                    ),
                ));
            }
            _ => {
                return Err(RuntimeStateError::runtime_defect(
                    "runtime-critical-section",
                    "critical-section-owner-recursion-inconsistent",
                    "critical section owner and recursion count disagree",
                ));
            }
        }
        self.critical_section(address)
    }

    pub fn leave_critical_section(
        &mut self,
        address: GuestAddress,
    ) -> Result<&RuntimeCriticalSection, RuntimeStateError> {
        self.require_critical_section_operation_state()?;
        let current_thread_id = self.current_thread_id();
        let section = self
            .process
            .critical_sections
            .get_mut(&address)
            .ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-critical-section",
                    "critical-section-uninitialized",
                    format!(
                        "critical section storage {:#010x} is not initialized",
                        address.0
                    ),
                )
            })?;
        if section.owning_thread_id != Some(current_thread_id) || section.recursion_count == 0 {
            return Err(RuntimeStateError::conflict(
                "runtime-critical-section",
                "critical-section-not-owned-by-current-thread",
                format!(
                    "critical section {:#010x} is not owned by current thread {}",
                    address.0, current_thread_id.0
                ),
            ));
        }
        section.recursion_count -= 1;
        if section.recursion_count == 0 {
            section.owning_thread_id = None;
        }
        self.critical_section(address)
    }

    pub fn critical_section(
        &self,
        address: GuestAddress,
    ) -> Result<&RuntimeCriticalSection, RuntimeStateError> {
        self.process.critical_sections.get(&address).ok_or_else(|| {
            RuntimeStateError::not_found(
                "runtime-critical-section",
                "critical-section-uninitialized",
                format!(
                    "critical section storage {:#010x} is not initialized",
                    address.0
                ),
            )
        })
    }

    fn require_critical_section_operation_state(&self) -> Result<(), RuntimeStateError> {
        if self.process.lifecycle != RuntimeProcessLifecycleState::Running
            || self.current_thread()?.lifecycle != RuntimeThreadLifecycleState::Running
            || !matches!(
                self.scheduler_state()?,
                SchedulerThreadState::Runnable | SchedulerThreadState::InCallback { .. }
            )
        {
            return Err(RuntimeStateError::conflict(
                "runtime-critical-section",
                "critical-section-lifecycle-state-invalid",
                "critical section operations require a running process and active current thread",
            ));
        }
        Ok(())
    }

    pub fn encoded_pointer_namespace(&self) -> &str {
        &self.encoded_pointer_namespace
    }

    pub fn encode_pointer(
        &mut self,
        source: u32,
    ) -> Result<&RuntimeEncodedPointer, RuntimeStateError> {
        if let Some(token) = self.encoded_pointer_sources.get(&source) {
            return self.registries.encoded_pointers.get(token).ok_or_else(|| {
                RuntimeStateError::runtime_defect(
                    "runtime-pointer-encoding",
                    "encoded-pointer-source-index-dangling",
                    "encoded pointer source index references a missing token",
                )
            });
        }
        if self.next_encoded_pointer_id == 0 {
            return Err(RuntimeStateError::resource_limit(
                "runtime-pointer-encoding",
                "encoded-pointer-token-space-exhausted",
                "logical encoded pointer token space is exhausted",
            ));
        }
        let token_id = self.next_encoded_pointer_id;
        let token = self.encoded_pointer_token_prefix | u32::from(token_id);
        if self.registries.encoded_pointers.contains_key(&token) {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-pointer-encoding",
                "encoded-pointer-token-collision",
                "logical encoded pointer allocator selected a live token",
            ));
        }
        self.next_encoded_pointer_id = self.next_encoded_pointer_id.checked_add(1).unwrap_or(0);
        self.encoded_pointer_sources.insert(source, token);
        self.registries.encoded_pointers.insert(
            token,
            RuntimeEncodedPointer {
                namespace: self.encoded_pointer_namespace.clone(),
                token_id,
                token,
                source,
            },
        );
        self.registries.encoded_pointers.get(&token).ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-pointer-encoding",
                "inserted-encoded-pointer-missing",
                "logical encoded pointer disappeared after insertion",
            )
        })
    }

    pub fn encoded_pointer_for_source(
        &self,
        source: u32,
    ) -> Result<&RuntimeEncodedPointer, RuntimeStateError> {
        let token = self.encoded_pointer_sources.get(&source).ok_or_else(|| {
            RuntimeStateError::not_found(
                "runtime-pointer-encoding",
                "encoded-pointer-source-unknown",
                format!("guest pointer source {source:#010x} has not been encoded"),
            )
        })?;
        self.registries.encoded_pointers.get(token).ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-pointer-encoding",
                "encoded-pointer-source-index-dangling",
                "encoded pointer source index references a missing token",
            )
        })
    }

    pub fn decode_pointer(&self, token: u32) -> Result<&RuntimeEncodedPointer, RuntimeStateError> {
        let token_id = token & 0xffff;
        if token_id == 0 || token & 0xffff_0000 != self.encoded_pointer_token_prefix {
            return Err(RuntimeStateError::invalid_input(
                "runtime-pointer-encoding",
                "encoded-pointer-token-invalid",
                format!("pointer token {token:#010x} is outside the runtime-owned token namespace"),
            ));
        }
        let encoded = self
            .registries
            .encoded_pointers
            .get(&token)
            .ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-pointer-encoding",
                    "encoded-pointer-token-unknown",
                    format!("pointer token {token:#010x} is not registered in this process"),
                )
            })?;
        if encoded.namespace != self.encoded_pointer_namespace
            || encoded.token != token
            || u32::from(encoded.token_id) != token_id
        {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-pointer-encoding",
                "encoded-pointer-record-inconsistent",
                "registered encoded pointer does not match its process token identity",
            ));
        }
        match self.encoded_pointer_sources.get(&encoded.source) {
            Some(indexed_token) if *indexed_token == token => Ok(encoded),
            _ => Err(RuntimeStateError::runtime_defect(
                "runtime-pointer-encoding",
                "encoded-pointer-source-index-inconsistent",
                "registered encoded pointer does not match its process source index",
            )),
        }
    }

    pub fn push_callback_frame(
        &mut self,
        target: GuestAddress,
        return_sentinel: GuestAddress,
    ) -> Result<RuntimeCallbackFrame, RuntimeStateError> {
        let current_depth = self.current_thread()?.callback_frames.len();
        match self.scheduler_state()? {
            SchedulerThreadState::Runnable if current_depth == 0 => {}
            SchedulerThreadState::InCallback { depth } if depth == current_depth => {}
            _ => {
                return Err(RuntimeStateError::conflict(
                    "runtime-callback",
                    "callback-entry-thread-state-invalid",
                    "callback entry requires a runnable thread or the matching nested callback state",
                ));
            }
        }
        let id = RuntimeCallbackFrameId(self.next_callback_frame_id);
        self.next_callback_frame_id =
            self.next_callback_frame_id.checked_add(1).ok_or_else(|| {
                RuntimeStateError::resource_limit(
                    "runtime-callback",
                    "callback-frame-id-overflow",
                    "callback frame ID space is exhausted",
                )
            })?;
        let frame = RuntimeCallbackFrame {
            id,
            thread_id: self.current_thread_id(),
            target,
            return_sentinel,
            entry_last_error: self.current_last_error()?,
        };
        let depth = {
            let thread = self.current_thread_mut()?;
            thread.callback_frames.push(frame);
            thread.callback_frames.len()
        };
        self.set_scheduler_state(SchedulerThreadState::InCallback { depth })?;
        Ok(frame)
    }

    pub fn pop_callback_frame(
        &mut self,
        expected: RuntimeCallbackFrameId,
    ) -> Result<RuntimeCallbackFrame, RuntimeStateError> {
        let current_depth = self.current_thread()?.callback_frames.len();
        if self.scheduler_state()?
            != (SchedulerThreadState::InCallback {
                depth: current_depth,
            })
        {
            return Err(RuntimeStateError::conflict(
                "runtime-callback",
                "callback-return-thread-state-invalid",
                "callback return requires the scheduler to identify the current callback depth",
            ));
        }
        let (frame, remaining_depth) = {
            let thread = self.current_thread_mut()?;
            let frame = thread.callback_frames.last().copied().ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-callback",
                    "callback-frame-stack-empty",
                    "current thread has no callback frame to return",
                )
            })?;
            if frame.id != expected {
                return Err(RuntimeStateError::conflict(
                    "runtime-callback",
                    "callback-frame-return-order-mismatch",
                    "callback frame return is not strict LIFO order",
                ));
            }
            thread.callback_frames.pop();
            (frame, thread.callback_frames.len())
        };
        let scheduler_state = if remaining_depth == 0 {
            SchedulerThreadState::Runnable
        } else {
            SchedulerThreadState::InCallback {
                depth: remaining_depth,
            }
        };
        self.set_scheduler_state(scheduler_state)?;
        Ok(frame)
    }

    pub fn begin_host_request(
        &mut self,
        service: impl Into<String>,
    ) -> Result<RuntimeHostRequest, RuntimeStateError> {
        let service = service.into();
        if service.is_empty() {
            return Err(RuntimeStateError::invalid_input(
                "runtime-host-request",
                "host-service-empty",
                "host request requires a nonempty service identity",
            ));
        }
        let resume_state = match self.scheduler_state()? {
            state @ (SchedulerThreadState::Runnable | SchedulerThreadState::InCallback { .. }) => {
                state
            }
            _ => {
                return Err(RuntimeStateError::conflict(
                    "runtime-host-request",
                    "thread-not-runnable",
                    "only a runnable guest thread or active callback can begin a host request",
                ));
            }
        };
        let request_id = self.next_host_request_id;
        self.next_host_request_id = self.next_host_request_id.checked_add(1).ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-host-request",
                "host-request-id-overflow",
                "host request ID space is exhausted",
            )
        })?;
        let request = RuntimeHostRequest {
            request_id,
            thread_id: self.current_thread_id(),
            service,
            resume_state,
        };
        self.registries
            .host_requests
            .insert(request_id, request.clone());
        self.set_scheduler_state(SchedulerThreadState::WaitingHost { request_id })?;
        Ok(request)
    }

    pub fn complete_host_request(
        &mut self,
        request_id: u64,
    ) -> Result<RuntimeHostCompletion, RuntimeStateError> {
        let request = self
            .registries
            .host_requests
            .remove(&request_id)
            .ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-host-request",
                    "host-request-id-unknown",
                    format!("host request ID {request_id} is not pending"),
                )
            })?;
        if self.scheduler_state()? != (SchedulerThreadState::WaitingHost { request_id }) {
            self.registries.host_requests.insert(request_id, request);
            return Err(RuntimeStateError::conflict(
                "runtime-host-request",
                "host-completion-thread-state-mismatch",
                "host completion does not match the current thread wait state",
            ));
        }
        if request.thread_id != self.current_thread_id() {
            self.registries.host_requests.insert(request_id, request);
            return Err(RuntimeStateError::conflict(
                "runtime-host-request",
                "host-completion-thread-mismatch",
                "host completion belongs to a different guest thread",
            ));
        }
        self.set_scheduler_state(request.resume_state)?;
        let completion = RuntimeHostCompletion {
            request_id,
            thread_id: request.thread_id,
            service: request.service,
        };
        self.scheduler
            .events
            .push_back(RuntimeSchedulerEvent::HostCompleted(completion.clone()));
        Ok(completion)
    }

    pub fn pop_scheduler_event(&mut self) -> Option<RuntimeSchedulerEvent> {
        self.scheduler.events.pop_front()
    }

    pub fn request_current_thread_exit(
        &mut self,
        exit_code: u32,
    ) -> Result<RuntimeCancellation, RuntimeStateError> {
        if self.process.lifecycle != RuntimeProcessLifecycleState::Running {
            return Err(RuntimeStateError::conflict(
                "runtime-lifecycle",
                "thread-exit-request-transition-invalid",
                "thread exit request requires a running process",
            ));
        }
        let plan = self.plan_thread_exit_request(self.current_thread_id(), exit_code)?;
        let cancellation = plan.cancellation;
        self.commit_thread_exit_request(plan);
        Ok(cancellation)
    }

    pub fn complete_current_thread_exit(&mut self) -> Result<(), RuntimeStateError> {
        if self.process.lifecycle != RuntimeProcessLifecycleState::Running {
            return Err(RuntimeStateError::conflict(
                "runtime-lifecycle",
                "thread-exit-completion-transition-invalid",
                "thread exit completion requires a running process",
            ));
        }
        let completion = self.plan_thread_exit_completion(self.current_thread_id())?;
        let wait_updates = self.objects.plan_wait_state_updates(&[(
            RuntimeObjectKey::Thread {
                thread_id: completion.thread_id.0,
            },
            RuntimeWaitState::Signaled {
                exit_code: completion.exit_code,
            },
        )])?;
        let wake_completions = self.plan_waiter_wakes(&wait_updates, &[completion.thread_id])?;
        self.objects.apply_wait_state_updates(wait_updates);
        self.commit_thread_exit_completion(completion);
        self.commit_waiter_wakes(wake_completions);
        Ok(())
    }

    pub fn cancel_current_thread(
        &mut self,
        exit_code: u32,
    ) -> Result<RuntimeCancellation, RuntimeStateError> {
        if self.process.lifecycle != RuntimeProcessLifecycleState::Running {
            return Err(RuntimeStateError::conflict(
                "runtime-lifecycle",
                "thread-exit-request-transition-invalid",
                "thread cancellation requires a running process",
            ));
        }
        let plan = self.plan_thread_exit_request(self.current_thread_id(), exit_code)?;
        let wait_updates = self.objects.plan_wait_state_updates(&[(
            RuntimeObjectKey::Thread {
                thread_id: plan.thread_id.0,
            },
            RuntimeWaitState::Signaled { exit_code },
        )])?;
        let wake_completions = self.plan_waiter_wakes(&wait_updates, &[plan.thread_id])?;
        let cancellation = plan.cancellation;
        let mut exited_thread = plan.next_thread.clone();
        exited_thread.lifecycle = RuntimeThreadLifecycleState::Exited { exit_code };
        self.objects.apply_wait_state_updates(wait_updates);
        self.commit_thread_exit_request(plan);
        self.threads.insert(cancellation.thread_id, exited_thread);
        self.scheduler.threads.insert(
            cancellation.thread_id,
            SchedulerThreadState::Terminated { exit_code },
        );
        self.commit_waiter_wakes(wake_completions);
        Ok(cancellation)
    }

    pub fn request_current_process_exit(
        &mut self,
        exit_code: u32,
    ) -> Result<RuntimeProcessCancellation, RuntimeStateError> {
        if self.process.lifecycle != RuntimeProcessLifecycleState::Running {
            return Err(RuntimeStateError::conflict(
                "runtime-lifecycle",
                "process-exit-request-transition-invalid",
                "process exit request requires a running process",
            ));
        }
        let plan = self.plan_process_exit_request(exit_code)?;
        let cancellation = plan.cancellation.clone();
        self.commit_process_exit_request(plan);
        Ok(cancellation)
    }

    pub fn complete_current_process_exit(&mut self) -> Result<(), RuntimeStateError> {
        let exit_code = match self.process.lifecycle {
            RuntimeProcessLifecycleState::ExitRequested {
                exit_code,
                initiating_thread_id,
            } if initiating_thread_id == self.current_thread_id() => exit_code,
            _ => {
                return Err(RuntimeStateError::conflict(
                    "runtime-lifecycle",
                    "process-exit-completion-transition-invalid",
                    "process exit completion requires a matching prior exit request",
                ));
            }
        };
        if !self.process.heaps.is_empty() {
            return Err(RuntimeStateError::conflict(
                "runtime-heap-backing",
                "process-heaps-live-at-exit-completion",
                "process exit completion requires private heap teardown",
            ));
        }
        let completions = self.plan_process_exit_completion(exit_code)?;
        let mut wait_states = Vec::with_capacity(completions.len() + 1);
        for completion in &completions {
            wait_states.push((
                RuntimeObjectKey::Thread {
                    thread_id: completion.thread_id.0,
                },
                RuntimeWaitState::Signaled { exit_code },
            ));
        }
        wait_states.push((
            RuntimeObjectKey::Process {
                process_id: self.process.id.0,
            },
            RuntimeWaitState::Signaled { exit_code },
        ));
        let wait_updates = self.objects.plan_wait_state_updates(&wait_states)?;
        let excluded_threads: Vec<_> = completions
            .iter()
            .map(|completion| completion.thread_id)
            .collect();
        let wake_completions = self.plan_waiter_wakes(&wait_updates, &excluded_threads)?;
        if !wake_completions.is_empty() {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-wait",
                "process-exit-surviving-waiter",
                "process exit cannot leave a registered waiter to resume",
            ));
        }
        self.objects.apply_wait_state_updates(wait_updates);
        for completion in completions {
            self.commit_thread_exit_completion(completion);
        }
        self.process.lifecycle = RuntimeProcessLifecycleState::Exited { exit_code };
        Ok(())
    }

    pub fn terminate_current_process(
        &mut self,
        exit_code: u32,
    ) -> Result<RuntimeProcessCancellation, RuntimeStateError> {
        if self.process.lifecycle != RuntimeProcessLifecycleState::Running {
            return Err(RuntimeStateError::conflict(
                "runtime-lifecycle",
                "process-exit-request-transition-invalid",
                "process termination requires a running process",
            ));
        }
        if !self.process.heaps.is_empty() {
            return Err(RuntimeStateError::conflict(
                "runtime-heap-backing",
                "process-heaps-live-at-direct-termination",
                "direct process termination cannot bypass private heap teardown",
            ));
        }
        let plan = self.plan_process_exit_request(exit_code)?;
        let mut wait_states = Vec::with_capacity(plan.thread_plans.len() + 1);
        for thread_plan in &plan.thread_plans {
            wait_states.push((
                RuntimeObjectKey::Thread {
                    thread_id: thread_plan.thread_id.0,
                },
                RuntimeWaitState::Signaled { exit_code },
            ));
        }
        wait_states.push((
            RuntimeObjectKey::Process {
                process_id: self.process.id.0,
            },
            RuntimeWaitState::Signaled { exit_code },
        ));
        let wait_updates = self.objects.plan_wait_state_updates(&wait_states)?;
        let excluded_threads: Vec<_> = plan
            .thread_plans
            .iter()
            .map(|thread_plan| thread_plan.thread_id)
            .collect();
        let wake_completions = self.plan_waiter_wakes(&wait_updates, &excluded_threads)?;
        if !wake_completions.is_empty() {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-wait",
                "process-exit-surviving-waiter",
                "process termination cannot leave a registered waiter to resume",
            ));
        }
        let cancellation = plan.cancellation.clone();
        self.objects.apply_wait_state_updates(wait_updates);
        for thread_plan in plan.thread_plans {
            let thread_id = thread_plan.thread_id;
            let mut exited_thread = thread_plan.next_thread.clone();
            exited_thread.lifecycle = RuntimeThreadLifecycleState::Exited { exit_code };
            self.commit_thread_exit_request(thread_plan);
            self.threads.insert(thread_id, exited_thread);
            self.scheduler
                .threads
                .insert(thread_id, SchedulerThreadState::Terminated { exit_code });
        }
        self.process.lifecycle = RuntimeProcessLifecycleState::Exited { exit_code };
        Ok(cancellation)
    }

    fn plan_thread_wait_cancellation(
        &self,
        thread_id: RuntimeThreadId,
        scheduler_state: SchedulerThreadState,
    ) -> Result<Option<RuntimeWaitRegistration>, RuntimeStateError> {
        let registrations: Vec<_> = self
            .scheduler
            .wait_registrations
            .values()
            .filter(|registration| registration.thread_id == thread_id)
            .copied()
            .collect();
        match scheduler_state {
            SchedulerThreadState::WaitingObject {
                registration_id,
                object_id,
            } => {
                if registrations.len() != 1 {
                    return Err(RuntimeStateError::runtime_defect(
                        "runtime-wait",
                        "waiting-thread-registration-count-invalid",
                        "waiting thread must own exactly one wait registration",
                    ));
                }
                let registration = registrations[0];
                if registration.id != registration_id || registration.object_id != object_id {
                    return Err(RuntimeStateError::runtime_defect(
                        "runtime-wait",
                        "waiting-thread-registration-mismatch",
                        "scheduler wait state does not match the thread registration",
                    ));
                }
                let queue = self
                    .scheduler
                    .object_waiters
                    .get(&object_id)
                    .ok_or_else(|| {
                        RuntimeStateError::runtime_defect(
                            "runtime-wait",
                            "wait-object-queue-missing",
                            "wait registration has no object queue",
                        )
                    })?;
                if queue
                    .iter()
                    .filter(|&&queued_id| queued_id == registration_id)
                    .count()
                    != 1
                {
                    return Err(RuntimeStateError::runtime_defect(
                        "runtime-wait",
                        "wait-object-queue-registration-mismatch",
                        "wait registration must occur exactly once in its object queue",
                    ));
                }
                Ok(Some(registration))
            }
            _ if registrations.is_empty() => Ok(None),
            _ => Err(RuntimeStateError::runtime_defect(
                "runtime-wait",
                "nonwaiting-thread-registration-present",
                "nonwaiting thread owns a wait registration",
            )),
        }
    }

    fn plan_waiter_wakes(
        &self,
        wait_updates: &[(RuntimeObjectId, RuntimeObjectRecord)],
        excluded_threads: &[RuntimeThreadId],
    ) -> Result<Vec<RuntimeWaitCompletion>, RuntimeStateError> {
        let mut completions = Vec::new();
        let mut planned_threads = BTreeSet::new();
        for (object_id, record) in wait_updates {
            let wait_state = match record.wait_state {
                Some(state @ RuntimeWaitState::Signaled { .. }) => state,
                _ => continue,
            };
            let queued_ids = self
                .scheduler
                .object_waiters
                .get(object_id)
                .cloned()
                .unwrap_or_default();
            for registration in self
                .scheduler
                .wait_registrations
                .values()
                .filter(|registration| registration.object_id == *object_id)
            {
                if !queued_ids.contains(&registration.id) {
                    return Err(RuntimeStateError::runtime_defect(
                        "runtime-wait",
                        "wait-registration-object-index-missing",
                        "wait registration is absent from its object queue",
                    ));
                }
            }
            let mut seen_registration_ids = BTreeSet::new();
            for registration_id in queued_ids {
                if !seen_registration_ids.insert(registration_id) {
                    return Err(RuntimeStateError::runtime_defect(
                        "runtime-wait",
                        "wait-object-queue-registration-duplicate",
                        "object wait queue contains a duplicate registration",
                    ));
                }
                let registration = self
                    .scheduler
                    .wait_registrations
                    .get(&registration_id)
                    .copied()
                    .ok_or_else(|| {
                        RuntimeStateError::runtime_defect(
                            "runtime-wait",
                            "wait-object-queue-registration-missing",
                            "object wait queue references an unknown registration",
                        )
                    })?;
                if registration.object_id != *object_id {
                    return Err(RuntimeStateError::runtime_defect(
                        "runtime-wait",
                        "wait-object-queue-object-mismatch",
                        "object wait queue references a registration for another object",
                    ));
                }
                if self.thread_state(registration.thread_id)?.lifecycle
                    != RuntimeThreadLifecycleState::Running
                    || self.scheduler_state_for_thread(registration.thread_id)?
                        != (SchedulerThreadState::WaitingObject {
                            registration_id,
                            object_id: *object_id,
                        })
                {
                    return Err(RuntimeStateError::runtime_defect(
                        "runtime-wait",
                        "wait-registration-thread-state-mismatch",
                        "wait registration does not match a running waiting thread",
                    ));
                }
                if excluded_threads.contains(&registration.thread_id) {
                    continue;
                }
                if !planned_threads.insert(registration.thread_id) {
                    return Err(RuntimeStateError::runtime_defect(
                        "runtime-wait",
                        "wait-wake-thread-duplicate",
                        "one thread cannot be woken by multiple object signals",
                    ));
                }
                completions.push(RuntimeWaitCompletion {
                    registration,
                    wait_state,
                });
            }
        }
        Ok(completions)
    }

    fn commit_waiter_wakes(&mut self, completions: Vec<RuntimeWaitCompletion>) {
        for completion in completions {
            self.commit_remove_wait_registration(completion.registration);
            self.scheduler.threads.insert(
                completion.registration.thread_id,
                SchedulerThreadState::Runnable,
            );
            self.scheduler
                .events
                .push_back(RuntimeSchedulerEvent::WaitSatisfied(completion));
        }
    }

    fn commit_remove_wait_registration(&mut self, registration: RuntimeWaitRegistration) {
        self.scheduler.wait_registrations.remove(&registration.id);
        let remove_queue = if let Some(queue) = self
            .scheduler
            .object_waiters
            .get_mut(&registration.object_id)
        {
            queue.retain(|&registration_id| registration_id != registration.id);
            queue.is_empty()
        } else {
            false
        };
        if remove_queue {
            self.scheduler
                .object_waiters
                .remove(&registration.object_id);
        }
    }

    fn plan_thread_exit_request(
        &self,
        thread_id: RuntimeThreadId,
        exit_code: u32,
    ) -> Result<RuntimeThreadExitRequestPlan, RuntimeStateError> {
        let mut next_thread = self.thread_state(thread_id)?.clone();
        let scheduler_state = self.scheduler_state_for_thread(thread_id)?;
        if next_thread.lifecycle != RuntimeThreadLifecycleState::Running
            || !matches!(
                scheduler_state,
                SchedulerThreadState::Runnable
                    | SchedulerThreadState::WaitingHost { .. }
                    | SchedulerThreadState::WaitingObject { .. }
                    | SchedulerThreadState::WaitingMessage
                    | SchedulerThreadState::InCallback { .. }
            )
        {
            return Err(RuntimeStateError::conflict(
                "runtime-lifecycle",
                "thread-exit-request-transition-invalid",
                "thread exit request requires a running thread and a live scheduler state",
            ));
        }
        let cancelled_wait = self.plan_thread_wait_cancellation(thread_id, scheduler_state)?;
        let callback_frames_cancelled = next_thread.callback_frames.len();
        next_thread.callback_frames.clear();
        let messages_discarded = next_thread.message_queue.len();
        next_thread.message_queue.clear();
        next_thread.lifecycle = RuntimeThreadLifecycleState::ExitRequested { exit_code };
        let host_requests_cancelled = self
            .registries
            .host_requests
            .values()
            .filter(|request| request.thread_id == thread_id)
            .count();
        let scheduler_events_discarded = self
            .scheduler
            .events
            .iter()
            .filter(|event| event.thread_id() == thread_id)
            .count();
        Ok(RuntimeThreadExitRequestPlan {
            thread_id,
            next_thread,
            cancelled_wait,
            cancellation: RuntimeCancellation {
                thread_id,
                exit_code,
                host_requests_cancelled,
                callback_frames_cancelled,
                wait_registrations_cancelled: usize::from(cancelled_wait.is_some()),
                messages_discarded,
                scheduler_events_discarded,
            },
        })
    }

    fn commit_thread_exit_request(&mut self, plan: RuntimeThreadExitRequestPlan) {
        self.registries
            .host_requests
            .retain(|_, request| request.thread_id != plan.thread_id);
        if let Some(registration) = plan.cancelled_wait {
            self.commit_remove_wait_registration(registration);
        }
        self.scheduler
            .events
            .retain(|event| event.thread_id() != plan.thread_id);
        self.threads.insert(plan.thread_id, plan.next_thread);
        self.scheduler.threads.insert(
            plan.thread_id,
            SchedulerThreadState::Terminating {
                exit_code: plan.cancellation.exit_code,
            },
        );
    }

    fn plan_thread_exit_completion(
        &self,
        thread_id: RuntimeThreadId,
    ) -> Result<RuntimeThreadExitCompletionPlan, RuntimeStateError> {
        let mut next_thread = self.thread_state(thread_id)?.clone();
        let exit_code = match next_thread.lifecycle {
            RuntimeThreadLifecycleState::ExitRequested { exit_code } => exit_code,
            _ => {
                return Err(RuntimeStateError::conflict(
                    "runtime-lifecycle",
                    "thread-exit-completion-transition-invalid",
                    "thread exit completion requires a prior exit request",
                ));
            }
        };
        if self.scheduler_state_for_thread(thread_id)?
            != (SchedulerThreadState::Terminating { exit_code })
            || self
                .registries
                .host_requests
                .values()
                .any(|request| request.thread_id == thread_id)
            || !next_thread.callback_frames.is_empty()
            || !next_thread.message_queue.is_empty()
            || self
                .scheduler
                .wait_registrations
                .values()
                .any(|registration| registration.thread_id == thread_id)
            || self
                .scheduler
                .events
                .iter()
                .any(|event| event.thread_id() == thread_id)
        {
            return Err(RuntimeStateError::conflict(
                "runtime-lifecycle",
                "thread-exit-completion-state-invalid",
                "thread exit completion requires terminating scheduler state and no retained work",
            ));
        }
        next_thread.lifecycle = RuntimeThreadLifecycleState::Exited { exit_code };
        Ok(RuntimeThreadExitCompletionPlan {
            thread_id,
            exit_code,
            next_thread,
        })
    }

    fn commit_thread_exit_completion(&mut self, plan: RuntimeThreadExitCompletionPlan) {
        self.threads.insert(plan.thread_id, plan.next_thread);
        self.scheduler.threads.insert(
            plan.thread_id,
            SchedulerThreadState::Terminated {
                exit_code: plan.exit_code,
            },
        );
    }

    fn plan_process_exit_request(
        &self,
        exit_code: u32,
    ) -> Result<RuntimeProcessExitRequestPlan, RuntimeStateError> {
        let initiating_thread_id = self.current_thread_id();
        if self.thread_state(initiating_thread_id)?.lifecycle
            != RuntimeThreadLifecycleState::Running
        {
            return Err(RuntimeStateError::conflict(
                "runtime-lifecycle",
                "process-exit-initiating-thread-state-invalid",
                "process exit requires a running initiating thread",
            ));
        }
        let mut thread_plans = Vec::new();
        for (&thread_id, thread) in &self.threads {
            match thread.lifecycle {
                RuntimeThreadLifecycleState::Running => {
                    thread_plans.push(self.plan_thread_exit_request(thread_id, exit_code)?);
                }
                RuntimeThreadLifecycleState::Exited { exit_code } => {
                    if self.scheduler_state_for_thread(thread_id)?
                        != (SchedulerThreadState::Terminated { exit_code })
                    {
                        return Err(RuntimeStateError::runtime_defect(
                            "runtime-lifecycle",
                            "exited-thread-scheduler-state-mismatch",
                            "exited thread does not have the matching terminated scheduler state",
                        ));
                    }
                }
                RuntimeThreadLifecycleState::Initialized
                | RuntimeThreadLifecycleState::ExitRequested { .. } => {
                    return Err(RuntimeStateError::conflict(
                        "runtime-lifecycle",
                        "process-exit-thread-state-invalid",
                        "process exit requires every non-exited thread to be running",
                    ));
                }
            }
        }
        if !thread_plans
            .iter()
            .any(|plan| plan.thread_id == initiating_thread_id)
        {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-lifecycle",
                "process-exit-initiating-thread-plan-missing",
                "process exit did not plan the initiating thread",
            ));
        }
        let thread_cancellations: Vec<_> =
            thread_plans.iter().map(|plan| plan.cancellation).collect();
        let cancellation = RuntimeProcessCancellation {
            initiating_thread_id,
            exit_code,
            host_requests_cancelled: thread_cancellations
                .iter()
                .map(|cancellation| cancellation.host_requests_cancelled)
                .sum(),
            callback_frames_cancelled: thread_cancellations
                .iter()
                .map(|cancellation| cancellation.callback_frames_cancelled)
                .sum(),
            wait_registrations_cancelled: thread_cancellations
                .iter()
                .map(|cancellation| cancellation.wait_registrations_cancelled)
                .sum(),
            messages_discarded: thread_cancellations
                .iter()
                .map(|cancellation| cancellation.messages_discarded)
                .sum(),
            scheduler_events_discarded: thread_cancellations
                .iter()
                .map(|cancellation| cancellation.scheduler_events_discarded)
                .sum(),
            thread_cancellations,
        };
        Ok(RuntimeProcessExitRequestPlan {
            initiating_thread_id,
            exit_code,
            thread_plans,
            cancellation,
        })
    }

    fn commit_process_exit_request(&mut self, plan: RuntimeProcessExitRequestPlan) {
        for thread_plan in plan.thread_plans {
            self.commit_thread_exit_request(thread_plan);
        }
        self.process.lifecycle = RuntimeProcessLifecycleState::ExitRequested {
            exit_code: plan.exit_code,
            initiating_thread_id: plan.initiating_thread_id,
        };
    }

    fn plan_process_exit_completion(
        &self,
        exit_code: u32,
    ) -> Result<Vec<RuntimeThreadExitCompletionPlan>, RuntimeStateError> {
        let mut completions = Vec::new();
        for (&thread_id, thread) in &self.threads {
            match thread.lifecycle {
                RuntimeThreadLifecycleState::ExitRequested {
                    exit_code: thread_exit_code,
                } if thread_exit_code == exit_code => {
                    completions.push(self.plan_thread_exit_completion(thread_id)?);
                }
                RuntimeThreadLifecycleState::Exited {
                    exit_code: thread_exit_code,
                } => {
                    if self.scheduler_state_for_thread(thread_id)?
                        != (SchedulerThreadState::Terminated {
                            exit_code: thread_exit_code,
                        })
                    {
                        return Err(RuntimeStateError::runtime_defect(
                            "runtime-lifecycle",
                            "exited-thread-scheduler-state-mismatch",
                            "exited thread does not have the matching terminated scheduler state",
                        ));
                    }
                }
                _ => {
                    return Err(RuntimeStateError::runtime_defect(
                        "runtime-lifecycle",
                        "process-thread-exit-state-mismatch",
                        "process exit request does not match every live thread exit state",
                    ));
                }
            }
        }
        Ok(completions)
    }

    fn current_thread(&self) -> Result<&RuntimeThreadState, RuntimeStateError> {
        self.threads
            .get(&self.scheduler.current_thread_id)
            .ok_or_else(|| {
                RuntimeStateError::runtime_defect(
                    "runtime-thread",
                    "current-thread-state-missing",
                    "scheduler current thread has no runtime thread state",
                )
            })
    }

    fn current_thread_mut(&mut self) -> Result<&mut RuntimeThreadState, RuntimeStateError> {
        self.threads
            .get_mut(&self.scheduler.current_thread_id)
            .ok_or_else(|| {
                RuntimeStateError::runtime_defect(
                    "runtime-thread",
                    "current-thread-state-missing",
                    "scheduler current thread has no runtime thread state",
                )
            })
    }

    fn set_scheduler_state(
        &mut self,
        state: SchedulerThreadState,
    ) -> Result<(), RuntimeStateError> {
        let entry = self
            .scheduler
            .threads
            .get_mut(&self.scheduler.current_thread_id)
            .ok_or_else(|| {
                RuntimeStateError::runtime_defect(
                    "runtime-scheduler",
                    "current-thread-scheduler-state-missing",
                    "current runtime thread has no scheduler state",
                )
            })?;
        *entry = state;
        Ok(())
    }
}

fn validate_address_space(layout: ProcessAddressSpaceLayout) -> Result<(), RuntimeStateError> {
    let ranges = [
        ("import-thunks", layout.import_thunks),
        ("callback-returns", layout.callback_returns),
        ("dynamic-exports", layout.dynamic_exports),
    ];
    let mut checked = Vec::with_capacity(ranges.len());
    for (name, range) in ranges {
        let start = u64::from(range.base.0);
        let end = start
            .checked_add(range.byte_length)
            .filter(|end| *end <= u64::from(u32::MAX) + 1)
            .ok_or_else(|| {
                RuntimeStateError::invalid_input(
                    "runtime-state",
                    "address-space-range-invalid",
                    format!("process {name} range is empty or crosses the x86 address limit"),
                )
            })?;
        if range.byte_length == 0 {
            return Err(RuntimeStateError::invalid_input(
                "runtime-state",
                "address-space-range-invalid",
                format!("process {name} range is empty or crosses the x86 address limit"),
            ));
        }
        checked.push((name, start, end));
    }
    for left in 0..checked.len() {
        for right in (left + 1)..checked.len() {
            let (left_name, left_start, left_end) = checked[left];
            let (right_name, right_start, right_end) = checked[right];
            if left_start < right_end && right_start < left_end {
                return Err(RuntimeStateError::invalid_input(
                    "runtime-state",
                    "address-space-ranges-overlap",
                    format!("process {left_name} and {right_name} ranges overlap"),
                ));
            }
        }
    }
    Ok(())
}

fn align_runtime_heap_value(
    value: u32,
    alignment: u32,
    overflow_code: &'static str,
) -> Result<u32, RuntimeStateError> {
    if alignment == 0 || !alignment.is_power_of_two() {
        return Err(RuntimeStateError::runtime_defect(
            "runtime-heap-backing",
            "heap-alignment-policy-invalid",
            "private heap alignment must be a nonzero power of two",
        ));
    }
    value
        .checked_add(alignment - 1)
        .map(|rounded| rounded & !(alignment - 1))
        .ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-heap-backing",
                overflow_code,
                "private heap value overflowed while applying alignment",
            )
        })
}

fn validate_runtime_heap_backing(
    heap: &RuntimeHeap,
    backing: &RuntimeHeapBacking,
) -> Result<(), RuntimeStateError> {
    let capacity = u32::try_from(backing.range.byte_length()).map_err(|_| {
        RuntimeStateError::runtime_defect(
            "runtime-heap-backing",
            "heap-backing-capacity-not-representable",
            "private heap backing capacity does not fit x86 accounting",
        )
    })?;
    if backing.memory_owner.0 == 0
        || backing.alignment != RUNTIME_PRIVATE_HEAP_ALIGNMENT
        || !backing
            .range
            .start()
            .0
            .is_multiple_of(GUEST_PAGE_SIZE as u32)
        || capacity == 0
        || capacity % GUEST_PAGE_SIZE as u32 != 0
        || capacity > RUNTIME_PRIVATE_HEAP_ARENA_STRIDE
        || !backing
            .committed_bytes
            .is_multiple_of(GUEST_PAGE_SIZE as u32)
        || backing.committed_bytes > capacity
        || backing.high_water_mark > backing.committed_bytes
        || backing.next_allocation_id.0 == 0
    {
        return Err(RuntimeStateError::runtime_defect(
            "runtime-heap-backing",
            "heap-backing-metadata-invalid",
            "private heap backing violates its controlled address or size policy",
        ));
    }
    if backing.allocations.len() != heap.live_allocations as usize
        || backing.allocations.len() > RUNTIME_PRIVATE_HEAP_MAX_LIVE_ALLOCATIONS
    {
        return Err(RuntimeStateError::runtime_defect(
            "runtime-heap-backing",
            "heap-live-allocation-count-mismatch",
            "logical heap and backing allocation counts disagree",
        ));
    }

    let mut intervals = Vec::with_capacity(backing.allocations.len() + backing.free_blocks.len());
    let mut allocation_ids = BTreeSet::new();
    for (&address, allocation) in &backing.allocations {
        let offset = address
            .checked_sub(backing.range.start().0)
            .ok_or_else(|| {
                RuntimeStateError::runtime_defect(
                    "runtime-heap-backing",
                    "heap-allocation-outside-backing",
                    "private heap allocation precedes its backing range",
                )
            })?;
        let end = offset
            .checked_add(allocation.reserved_byte_length)
            .ok_or_else(|| {
                RuntimeStateError::runtime_defect(
                    "runtime-heap-backing",
                    "heap-allocation-range-overflow",
                    "private heap allocation range overflowed",
                )
            })?;
        if allocation.address.0 != address
            || allocation.id.0 == 0
            || allocation.id.0 >= backing.next_allocation_id.0
            || !allocation_ids.insert(allocation.id)
            || allocation.byte_length == 0
            || allocation.byte_length > allocation.reserved_byte_length
            || allocation.reserved_byte_length % backing.alignment != 0
            || offset % backing.alignment != 0
            || end > backing.high_water_mark
        {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-heap-backing",
                "heap-allocation-metadata-invalid",
                "private heap allocation violates identity, alignment, or range invariants",
            ));
        }
        intervals.push((offset, end));
    }
    for (&offset, &length) in &backing.free_blocks {
        let end = offset.checked_add(length).ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-heap-backing",
                "heap-free-block-range-overflow",
                "private heap free block range overflowed",
            )
        })?;
        if length == 0
            || offset % backing.alignment != 0
            || length % backing.alignment != 0
            || end > backing.high_water_mark
        {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-heap-backing",
                "heap-free-block-metadata-invalid",
                "private heap free block violates alignment or range invariants",
            ));
        }
        intervals.push((offset, end));
    }
    intervals.sort_unstable_by_key(|interval| interval.0);
    let mut cursor = 0u32;
    for (start, end) in intervals {
        if start != cursor || end <= start {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-heap-backing",
                "heap-backing-partition-invalid",
                "private heap live and free blocks do not partition the used arena",
            ));
        }
        cursor = end;
    }
    if cursor != backing.high_water_mark {
        return Err(RuntimeStateError::runtime_defect(
            "runtime-heap-backing",
            "heap-backing-high-water-mark-invalid",
            "private heap partition does not end at its high-water mark",
        ));
    }
    Ok(())
}

fn runtime_module_graph_error(error: RuntimeModuleGraphError) -> RuntimeStateError {
    match error.class {
        RuntimeModuleGraphErrorClass::InvalidInput => {
            RuntimeStateError::invalid_input("runtime-module-graph", error.code, error.message)
        }
        RuntimeModuleGraphErrorClass::NotFound => {
            RuntimeStateError::not_found("runtime-module-graph", error.code, error.message)
        }
        RuntimeModuleGraphErrorClass::Conflict => {
            RuntimeStateError::conflict("runtime-module-graph", error.code, error.message)
        }
        RuntimeModuleGraphErrorClass::ResourceLimit => {
            RuntimeStateError::resource_limit("runtime-module-graph", error.code, error.message)
        }
        RuntimeModuleGraphErrorClass::RuntimeDefect => {
            RuntimeStateError::runtime_defect("runtime-module-graph", error.code, error.message)
        }
    }
}

fn runtime_module_image_state_error(error: RuntimeStateError) -> RuntimeModuleImageError {
    let class = match error.class {
        RuntimeStateErrorClass::InvalidInput => RuntimeModuleImageErrorClass::InvalidInput,
        RuntimeStateErrorClass::NotFound => RuntimeModuleImageErrorClass::NotFound,
        RuntimeStateErrorClass::Conflict => RuntimeModuleImageErrorClass::Conflict,
        RuntimeStateErrorClass::ResourceLimit => RuntimeModuleImageErrorClass::ResourceLimit,
        RuntimeStateErrorClass::RuntimeDefect => RuntimeModuleImageErrorClass::RuntimeDefect,
    };
    RuntimeModuleImageError {
        class,
        code: error.code,
        message: error.message,
    }
}

fn runtime_module_image_error(error: RuntimeModuleImageError) -> RuntimeStateError {
    match error.class {
        RuntimeModuleImageErrorClass::InvalidInput => {
            RuntimeStateError::invalid_input("runtime-module-image", error.code, error.message)
        }
        RuntimeModuleImageErrorClass::NotFound => {
            RuntimeStateError::not_found("runtime-module-image", error.code, error.message)
        }
        RuntimeModuleImageErrorClass::Conflict => {
            RuntimeStateError::conflict("runtime-module-image", error.code, error.message)
        }
        RuntimeModuleImageErrorClass::ResourceLimit => {
            RuntimeStateError::resource_limit("runtime-module-image", error.code, error.message)
        }
        RuntimeModuleImageErrorClass::RuntimeDefect => {
            RuntimeStateError::runtime_defect("runtime-module-image", error.code, error.message)
        }
    }
}

fn runtime_export_matches_import(
    export: &RuntimeExport,
    module: &str,
    target: &ImportTarget,
) -> bool {
    match (export, target) {
        (
            RuntimeExport::Name {
                module: export_module,
                name: export_name,
            },
            ImportTarget::Name { name, .. },
        ) => export_module.eq_ignore_ascii_case(module) && export_name == name,
        (
            RuntimeExport::Ordinal {
                module: export_module,
                ordinal: export_ordinal,
            },
            ImportTarget::Ordinal { ordinal },
        ) => export_module.eq_ignore_ascii_case(module) && export_ordinal == ordinal,
        _ => false,
    }
}

fn validate_dll_return_sentinel(return_sentinel: GuestAddress) -> Result<(), RuntimeStateError> {
    let end = u64::from(CALLBACK_RETURN_ARENA_BASE.0) + CALLBACK_RETURN_ARENA_SIZE;
    if return_sentinel.0 < CALLBACK_RETURN_ARENA_BASE.0 || u64::from(return_sentinel.0) >= end {
        return Err(RuntimeStateError::invalid_input(
            "runtime-dll-lifecycle",
            "dll-return-sentinel-unowned",
            "DLL callback return sentinel is outside the runtime callback-control arena",
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeStateErrorClass {
    InvalidInput,
    NotFound,
    Conflict,
    ResourceLimit,
    RuntimeDefect,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeStateError {
    pub class: RuntimeStateErrorClass,
    pub stage: &'static str,
    pub code: &'static str,
    pub message: String,
}

impl RuntimeStateError {
    pub(crate) fn from_memory(error: GuestMemoryError) -> Self {
        let class = match error.class {
            GuestMemoryErrorClass::ResourceLimit => RuntimeStateErrorClass::ResourceLimit,
            GuestMemoryErrorClass::RuntimeDefect => RuntimeStateErrorClass::RuntimeDefect,
            GuestMemoryErrorClass::Conflict | GuestMemoryErrorClass::SynchronizationRequired => {
                RuntimeStateErrorClass::Conflict
            }
            GuestMemoryErrorClass::InvalidRequest | GuestMemoryErrorClass::Fault => {
                RuntimeStateErrorClass::InvalidInput
            }
        };
        Self::new(class, error.stage, error.code, error.message)
    }

    pub(crate) fn invalid_input(
        stage: &'static str,
        code: &'static str,
        message: impl Into<String>,
    ) -> Self {
        Self::new(RuntimeStateErrorClass::InvalidInput, stage, code, message)
    }

    pub(crate) fn not_found(
        stage: &'static str,
        code: &'static str,
        message: impl Into<String>,
    ) -> Self {
        Self::new(RuntimeStateErrorClass::NotFound, stage, code, message)
    }

    pub(crate) fn conflict(
        stage: &'static str,
        code: &'static str,
        message: impl Into<String>,
    ) -> Self {
        Self::new(RuntimeStateErrorClass::Conflict, stage, code, message)
    }

    pub(crate) fn resource_limit(
        stage: &'static str,
        code: &'static str,
        message: impl Into<String>,
    ) -> Self {
        Self::new(RuntimeStateErrorClass::ResourceLimit, stage, code, message)
    }

    pub(crate) fn runtime_defect(
        stage: &'static str,
        code: &'static str,
        message: impl Into<String>,
    ) -> Self {
        Self::new(RuntimeStateErrorClass::RuntimeDefect, stage, code, message)
    }

    fn new(
        class: RuntimeStateErrorClass,
        stage: &'static str,
        code: &'static str,
        message: impl Into<String>,
    ) -> Self {
        Self {
            class,
            stage,
            code,
            message: message.into(),
        }
    }
}

impl fmt::Display for RuntimeStateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} / {}: {}",
            self.stage, self.code, self.message
        )
    }
}

impl std::error::Error for RuntimeStateError {}

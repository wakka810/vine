use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;

use serde::Serialize;

use crate::callbacks::{CALLBACK_RETURN_ARENA_BASE, CALLBACK_RETURN_ARENA_SIZE};
use crate::dynamic_exports::{
    DYNAMIC_EXPORT_ARENA_BASE, DYNAMIC_EXPORT_ARENA_SIZE, DYNAMIC_EXPORT_ENDPOINT_NAMESPACE,
    DYNAMIC_EXPORT_SLOT_SIZE, DynamicExportThunk,
};
use crate::imports::{IMPORT_THUNK_ARENA_BASE, IMPORT_THUNK_ARENA_SIZE};
use crate::memory::GuestAddress;
use crate::module_graph::{
    RuntimeModuleClass, RuntimeModuleGraph, RuntimeModuleGraphError, RuntimeModuleGraphErrorClass,
    RuntimeModuleId, RuntimeModuleRegistration,
};

pub const LOGICAL_PRIVATE_HEAP_NAMESPACE: &str = "vine.logical-private-heap.v1";
pub const LOGICAL_LOADED_MODULE_NAMESPACE: &str = "vine.logical-loaded-module.v1";
pub const LOGICAL_TLS_INDEX_NAMESPACE: &str = "vine.logical-tls-index.v1";
pub const LOGICAL_ENCODED_POINTER_NAMESPACE: &str = "vine.logical-encoded-pointer.v1";
pub const RUNTIME_PROCESS_OBJECT_NAMESPACE: &str = "vine.runtime-process-object.v1";
pub const RUNTIME_THREAD_OBJECT_NAMESPACE: &str = "vine.runtime-thread-object.v1";
pub const TLS_OUT_OF_INDEXES: u32 = u32::MAX;
pub const X86_STARTUP_INFO_W_BYTE_LENGTH: usize = 68;

const LOGICAL_PRIVATE_HEAP_HANDLE_PREFIX: u32 = 0x6801_0000;
const RUNTIME_PRIVATE_HEAP_OWNER: &str = "runtime-process-private-heaps";
const RUNTIME_PRIVATE_HEAP_VARIANT: &str = "heap-create-metadata-v1";

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
    WaitingHost { request_id: u64 },
    WaitingObject { object: u32 },
    InCallback { depth: usize },
    Terminating { exit_code: u32 },
    Terminated { exit_code: u32 },
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
    pub scheduler_events_discarded: usize,
}

struct RuntimeThreadExitRequestPlan {
    thread_id: RuntimeThreadId,
    next_thread: RuntimeThreadState,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "event", content = "completion", rename_all = "kebab-case")]
pub enum RuntimeSchedulerEvent {
    HostCompleted(RuntimeHostCompletion),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeThreadState {
    pub id: RuntimeThreadId,
    pub lifecycle: RuntimeThreadLifecycleState,
    pub last_error: u32,
    pub tls_values: BTreeMap<u32, RuntimeTlsValue>,
    pub callback_frames: Vec<RuntimeCallbackFrame>,
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
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeScheduler {
    pub current_thread_id: RuntimeThreadId,
    pub threads: BTreeMap<RuntimeThreadId, SchedulerThreadState>,
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
    registries: RuntimeRegistries,
    encoded_pointer_namespace: String,
    encoded_pointer_token_prefix: u32,
    encoded_pointer_sources: BTreeMap<u32, u32>,
    next_encoded_pointer_id: u16,
    next_private_heap_slot: u16,
    next_tls_index: u32,
    next_callback_frame_id: u64,
    next_host_request_id: u64,
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
            callback_frames: Vec::new(),
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
            },
            threads: BTreeMap::from([(bootstrap.initial_thread_id, initial_thread)]),
            scheduler: RuntimeScheduler {
                current_thread_id: bootstrap.initial_thread_id,
                threads: BTreeMap::from([(
                    bootstrap.initial_thread_id,
                    SchedulerThreadState::Initialized,
                )]),
                events: VecDeque::new(),
            },
            objects,
            module_graph: RuntimeModuleGraph::default(),
            registries: RuntimeRegistries {
                dynamic_endpoints: BTreeMap::new(),
                encoded_pointers: BTreeMap::new(),
                host_requests: BTreeMap::new(),
            },
            encoded_pointer_namespace: bootstrap.encoded_pointer_namespace,
            encoded_pointer_token_prefix: bootstrap.encoded_pointer_token_prefix,
            encoded_pointer_sources: BTreeMap::new(),
            next_encoded_pointer_id: 1,
            next_private_heap_slot: 1,
            next_tls_index: 0,
            next_callback_frame_id: 1,
            next_host_request_id: 1,
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
                callback_frames: Vec::new(),
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
        self.module_graph
            .attach_all()
            .map_err(runtime_module_graph_error)
    }

    pub fn detach_modules(&mut self) -> Result<Vec<RuntimeModuleId>, RuntimeStateError> {
        self.module_graph
            .detach_all()
            .map_err(runtime_module_graph_error)
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
        self.objects.apply_wait_state_updates(wait_updates);
        self.commit_thread_exit_completion(completion);
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

    fn plan_thread_exit_request(
        &self,
        thread_id: RuntimeThreadId,
        exit_code: u32,
    ) -> Result<RuntimeThreadExitRequestPlan, RuntimeStateError> {
        let mut next_thread = self.thread_state(thread_id)?.clone();
        if next_thread.lifecycle != RuntimeThreadLifecycleState::Running
            || !matches!(
                self.scheduler_state_for_thread(thread_id)?,
                SchedulerThreadState::Runnable
                    | SchedulerThreadState::WaitingHost { .. }
                    | SchedulerThreadState::WaitingObject { .. }
                    | SchedulerThreadState::InCallback { .. }
            )
        {
            return Err(RuntimeStateError::conflict(
                "runtime-lifecycle",
                "thread-exit-request-transition-invalid",
                "thread exit request requires a running thread and a live scheduler state",
            ));
        }
        let callback_frames_cancelled = next_thread.callback_frames.len();
        next_thread.callback_frames.clear();
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
            .filter(|event| match event {
                RuntimeSchedulerEvent::HostCompleted(completion) => {
                    completion.thread_id == thread_id
                }
            })
            .count();
        Ok(RuntimeThreadExitRequestPlan {
            thread_id,
            next_thread,
            cancellation: RuntimeCancellation {
                thread_id,
                exit_code,
                host_requests_cancelled,
                callback_frames_cancelled,
                scheduler_events_discarded,
            },
        })
    }

    fn commit_thread_exit_request(&mut self, plan: RuntimeThreadExitRequestPlan) {
        self.registries
            .host_requests
            .retain(|_, request| request.thread_id != plan.thread_id);
        self.scheduler.events.retain(|event| match event {
            RuntimeSchedulerEvent::HostCompleted(completion) => {
                completion.thread_id != plan.thread_id
            }
        });
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
            || self.scheduler.events.iter().any(|event| match event {
                RuntimeSchedulerEvent::HostCompleted(completion) => {
                    completion.thread_id == thread_id
                }
            })
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
    fn invalid_input(stage: &'static str, code: &'static str, message: impl Into<String>) -> Self {
        Self::new(RuntimeStateErrorClass::InvalidInput, stage, code, message)
    }

    fn not_found(stage: &'static str, code: &'static str, message: impl Into<String>) -> Self {
        Self::new(RuntimeStateErrorClass::NotFound, stage, code, message)
    }

    fn conflict(stage: &'static str, code: &'static str, message: impl Into<String>) -> Self {
        Self::new(RuntimeStateErrorClass::Conflict, stage, code, message)
    }

    fn resource_limit(stage: &'static str, code: &'static str, message: impl Into<String>) -> Self {
        Self::new(RuntimeStateErrorClass::ResourceLimit, stage, code, message)
    }

    fn runtime_defect(stage: &'static str, code: &'static str, message: impl Into<String>) -> Self {
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

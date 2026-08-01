use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use super::{
    LOGICAL_LOADED_MODULE_NAMESPACE, LOGICAL_PRIVATE_HEAP_NAMESPACE, RUNTIME_FILE_SEARCH_NAMESPACE,
    RUNTIME_SOURCE_FILE_NAMESPACE, RuntimeStateError,
};

pub const RUNTIME_PROCESS_OBJECT_NAMESPACE: &str = "vine.runtime-process-object.v1";
pub const RUNTIME_THREAD_OBJECT_NAMESPACE: &str = "vine.runtime-thread-object.v1";
pub const RUNTIME_MUTEX_OBJECT_NAMESPACE: &str = "vine.runtime-mutex-object.v1";
pub const RUNTIME_WAITABLE_TIMER_OBJECT_NAMESPACE: &str = "vine.runtime-waitable-timer-object.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeObjectKind {
    Process,
    Thread,
    Mutex,
    WaitableTimer,
    File,
    FileSearch,
    PrivateHeap,
    Module,
}

impl RuntimeObjectKind {
    const fn handle_tag(self) -> u8 {
        match self {
            Self::Process => 0x50,
            Self::Thread => 0x54,
            Self::Mutex => 0x58,
            Self::WaitableTimer => 0x57,
            Self::File => 0x46,
            Self::FileSearch => 0x53,
            Self::PrivateHeap => 0x68,
            Self::Module => 0x4d,
        }
    }

    fn from_handle_tag(tag: u8) -> Option<Self> {
        match tag {
            0x50 => Some(Self::Process),
            0x54 => Some(Self::Thread),
            0x58 => Some(Self::Mutex),
            0x57 => Some(Self::WaitableTimer),
            0x46 => Some(Self::File),
            0x53 => Some(Self::FileSearch),
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
    Mutex { mutex_id: u64 },
    WaitableTimer { timer_id: u64 },
    File { file_id: u64 },
    FileSearch { search_id: u64 },
    PrivateHeap { handle: u32 },
    Module { handle: u32 },
}

impl RuntimeObjectKey {
    pub const fn kind(&self) -> RuntimeObjectKind {
        match self {
            Self::Process { .. } => RuntimeObjectKind::Process,
            Self::Thread { .. } => RuntimeObjectKind::Thread,
            Self::Mutex { .. } => RuntimeObjectKind::Mutex,
            Self::WaitableTimer { .. } => RuntimeObjectKind::WaitableTimer,
            Self::File { .. } => RuntimeObjectKind::File,
            Self::FileSearch { .. } => RuntimeObjectKind::FileSearch,
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
    FindClose,
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

struct RuntimeObjectRetirementPlan {
    id: RuntimeObjectId,
    key: RuntimeObjectKey,
    handles: Vec<RuntimeHandleEntry>,
    next_slots: Vec<(u32, RuntimeHandleSlot)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeCloseHandleObjectMutationPlan {
    previous: RuntimeObjectTable,
    next: RuntimeObjectTable,
    outcome: RuntimeCloseHandleObjectMutationOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeObjectRegistrationPlan {
    expected_next_object_id: u64,
    next_object_id: u64,
    record: RuntimeObjectRecord,
    handle_entry: RuntimeHandleEntry,
    slot_key: u32,
    expected_slot: Option<RuntimeHandleSlot>,
}

impl RuntimeObjectRegistrationPlan {
    pub(super) const fn handle(&self) -> RuntimeHandle {
        self.handle_entry.handle
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RuntimeCloseHandleObjectMutationOutcome {
    Opened(RuntimeHandle),
    Closed(RuntimeHandleEntry),
    Teardown { retired_handles: usize },
}

impl RuntimeCloseHandleObjectMutationPlan {
    pub(super) fn opened_handle(&self) -> RuntimeHandle {
        match self.outcome {
            RuntimeCloseHandleObjectMutationOutcome::Opened(handle) => handle,
            _ => unreachable!("close-handle object plan does not open a handle"),
        }
    }

    pub(super) fn closed_entry(&self) -> RuntimeHandleEntry {
        match self.outcome {
            RuntimeCloseHandleObjectMutationOutcome::Closed(entry) => entry,
            _ => unreachable!("close-handle object plan does not close a handle"),
        }
    }

    pub(super) fn retired_handles(&self) -> usize {
        match self.outcome {
            RuntimeCloseHandleObjectMutationOutcome::Teardown { retired_handles } => {
                retired_handles
            }
            _ => unreachable!("close-handle object plan is not a teardown"),
        }
    }
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
    pub(super) fn plan_object_and_handle_registration(
        &self,
        key: RuntimeObjectKey,
    ) -> Result<RuntimeObjectRegistrationPlan, RuntimeStateError> {
        if self.identity_index.contains_key(&key) {
            return Err(RuntimeStateError::conflict(
                "runtime-object",
                "runtime-object-identity-duplicate",
                "runtime object identity is already registered",
            ));
        }
        let id = RuntimeObjectId(self.next_object_id);
        let next_object_id = self.next_object_id.checked_add(1).ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-object",
                "runtime-object-id-space-exhausted",
                "runtime object ID space is exhausted",
            )
        })?;
        let kind = key.kind();
        let handle = self.next_available_handle(kind)?;
        self.preflight_exact_handle(kind, handle)?;
        if self.handles.contains_key(&handle.0) {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-object",
                "runtime-handle-entry-collision",
                "free runtime handle slot still has a table entry",
            ));
        }
        let slot_key = runtime_handle_slot_key(kind, handle.slot());
        Ok(RuntimeObjectRegistrationPlan {
            expected_next_object_id: self.next_object_id,
            next_object_id,
            record: runtime_object_record(id, key, 1),
            handle_entry: RuntimeHandleEntry {
                handle,
                object_id: id,
                kind,
            },
            slot_key,
            expected_slot: self.slots.get(&slot_key).copied(),
        })
    }

    pub(super) fn preflight_object_registration_commit(
        &self,
        plan: &RuntimeObjectRegistrationPlan,
    ) -> Result<(), RuntimeStateError> {
        if self.next_object_id != plan.expected_next_object_id
            || self.identity_index.contains_key(&plan.record.key)
            || self.objects.contains_key(&plan.record.id)
            || self.handles.contains_key(&plan.handle_entry.handle.0)
            || self.slots.get(&plan.slot_key).copied() != plan.expected_slot
        {
            return Err(RuntimeStateError::conflict(
                "runtime-object",
                "runtime-object-registration-plan-stale",
                "runtime object state changed after registration preflight",
            ));
        }
        Ok(())
    }

    pub(super) fn commit_object_registration(
        &mut self,
        plan: RuntimeObjectRegistrationPlan,
    ) -> (RuntimeObjectId, RuntimeHandle) {
        debug_assert_eq!(self.next_object_id, plan.expected_next_object_id);
        debug_assert!(!self.identity_index.contains_key(&plan.record.key));
        debug_assert!(!self.objects.contains_key(&plan.record.id));
        debug_assert!(!self.handles.contains_key(&plan.handle_entry.handle.0));
        debug_assert_eq!(self.slots.get(&plan.slot_key).copied(), plan.expected_slot);
        self.next_object_id = plan.next_object_id;
        let id = plan.record.id;
        let key = plan.record.key.clone();
        let handle = plan.handle_entry.handle;
        self.identity_index.insert(key, id);
        self.objects.insert(id, plan.record);
        self.handles.insert(handle.0, plan.handle_entry);
        self.slots.insert(
            plan.slot_key,
            RuntimeHandleSlot {
                generation: handle.generation(),
                live: true,
                retired: false,
            },
        );
        (id, handle)
    }

    pub(super) fn plan_close_handle_object_open(
        &self,
        key: RuntimeObjectKey,
        already_exists: bool,
    ) -> Result<RuntimeCloseHandleObjectMutationPlan, RuntimeStateError> {
        if !is_payload_close_handle_kind(key.kind()) {
            return Err(RuntimeStateError::invalid_input(
                "runtime-object",
                "runtime-close-handle-object-plan-kind-invalid",
                "payload handle planning requires a supported close-handle object identity",
            ));
        }
        let mut next = self.clone();
        let handle = if already_exists {
            next.open_handle(&key)?
        } else {
            next.register_object_and_open_handle(key)?.1
        };
        Ok(RuntimeCloseHandleObjectMutationPlan {
            previous: self.clone(),
            next,
            outcome: RuntimeCloseHandleObjectMutationOutcome::Opened(handle),
        })
    }

    pub(super) fn plan_close_handle_object_close(
        &self,
        handle: RuntimeHandle,
        retiring_key: Option<&RuntimeObjectKey>,
    ) -> Result<RuntimeCloseHandleObjectMutationPlan, RuntimeStateError> {
        let kind = handle.kind()?;
        if !is_payload_close_handle_kind(kind) || retiring_key.is_some_and(|key| key.kind() != kind)
        {
            return Err(RuntimeStateError::invalid_input(
                "runtime-object",
                "runtime-close-handle-object-plan-kind-invalid",
                "payload handle close planning requires matching supported object identities",
            ));
        }
        let mut next = self.clone();
        let closed = next.close_handle(handle)?;
        if let Some(key) = retiring_key {
            next.remove_closed_close_handle_object(key)?;
        }
        Ok(RuntimeCloseHandleObjectMutationPlan {
            previous: self.clone(),
            next,
            outcome: RuntimeCloseHandleObjectMutationOutcome::Closed(closed),
        })
    }

    pub(super) fn plan_find_close_object_close(
        &self,
        handle: RuntimeHandle,
        key: &RuntimeObjectKey,
    ) -> Result<RuntimeCloseHandleObjectMutationPlan, RuntimeStateError> {
        if key.kind() != RuntimeObjectKind::FileSearch
            || handle.kind()? != RuntimeObjectKind::FileSearch
        {
            return Err(RuntimeStateError::invalid_input(
                "runtime-object",
                "runtime-find-close-object-kind-invalid",
                "FindClose planning requires a matching file-search identity",
            ));
        }
        let record = self.object_for_handle(handle, RuntimeObjectKind::FileSearch)?;
        if &record.key != key || record.open_handles != 1 {
            return Err(RuntimeStateError::conflict(
                "runtime-object",
                "runtime-find-close-object-identity-mismatch",
                "FindClose requires the sole handle for its file-search identity",
            ));
        }
        let mut next = self.clone();
        let mut retired = next.retire_objects_with_policy(
            std::slice::from_ref(key),
            RuntimeHandleClosePolicy::FindClose,
        )?;
        let closed = retired.pop().ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-object",
                "runtime-find-close-object-handle-missing",
                "preflighted file-search retirement produced no handle",
            )
        })?;
        if !retired.is_empty() || closed.handle != handle {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-object",
                "runtime-find-close-object-handle-mismatch",
                "file-search retirement produced an unexpected handle set",
            ));
        }
        Ok(RuntimeCloseHandleObjectMutationPlan {
            previous: self.clone(),
            next,
            outcome: RuntimeCloseHandleObjectMutationOutcome::Closed(closed),
        })
    }

    pub(super) fn plan_payload_object_teardown(
        &self,
        close_handle_keys: &[RuntimeObjectKey],
        find_close_keys: &[RuntimeObjectKey],
    ) -> Result<RuntimeCloseHandleObjectMutationPlan, RuntimeStateError> {
        if find_close_keys
            .iter()
            .any(|key| key.kind() != RuntimeObjectKind::FileSearch)
        {
            return Err(RuntimeStateError::invalid_input(
                "runtime-object",
                "runtime-payload-object-teardown-kind-invalid",
                "payload FindClose teardown received a non-search identity",
            ));
        }
        let mut next = self.clone();
        let close_handle_count = next.retire_close_handle_objects(close_handle_keys)?;
        let find_close_count = next
            .retire_objects_with_policy(find_close_keys, RuntimeHandleClosePolicy::FindClose)?
            .len();
        let retired_handles = close_handle_count
            .checked_add(find_close_count)
            .ok_or_else(|| {
                RuntimeStateError::resource_limit(
                    "runtime-object",
                    "runtime-payload-retired-handle-count-overflow",
                    "payload retired handle count overflowed host accounting",
                )
            })?;
        Ok(RuntimeCloseHandleObjectMutationPlan {
            previous: self.clone(),
            next,
            outcome: RuntimeCloseHandleObjectMutationOutcome::Teardown { retired_handles },
        })
    }

    pub(super) fn commit_close_handle_object_mutation(
        &mut self,
        plan: RuntimeCloseHandleObjectMutationPlan,
    ) -> Result<(), RuntimeStateError> {
        if *self != plan.previous {
            return Err(RuntimeStateError::conflict(
                "runtime-object",
                "runtime-close-handle-object-plan-stale",
                "runtime object state changed after payload transaction preflight",
            ));
        }
        *self = plan.next;
        Ok(())
    }

    pub(super) fn register_object(
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
        let record = runtime_object_record(id, key.clone(), 0);
        self.objects.insert(id, record);
        self.identity_index.insert(key, id);
        Ok(id)
    }

    pub(super) fn register_object_with_handle(
        &mut self,
        key: RuntimeObjectKey,
        handle: RuntimeHandle,
    ) -> Result<RuntimeObjectId, RuntimeStateError> {
        self.preflight_exact_handle(key.kind(), handle)?;
        let previous_next_object_id = self.next_object_id;
        let id = self.register_object(key.clone())?;
        if let Err(error) = self.insert_exact_handle(id, handle) {
            self.objects.remove(&id);
            self.identity_index.remove(&key);
            self.next_object_id = previous_next_object_id;
            return Err(error);
        }
        Ok(id)
    }

    pub(super) fn register_object_and_open_handle(
        &mut self,
        key: RuntimeObjectKey,
    ) -> Result<(RuntimeObjectId, RuntimeHandle), RuntimeStateError> {
        let plan = self.plan_object_and_handle_registration(key)?;
        self.preflight_object_registration_commit(&plan)?;
        Ok(self.commit_object_registration(plan))
    }

    pub(super) fn open_handle(
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

    pub(super) fn handle_for_object(
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

    pub(super) fn object_for_handle(
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

    pub(super) fn close_handle(
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

    pub(super) fn remove_closed_close_handle_object(
        &mut self,
        key: &RuntimeObjectKey,
    ) -> Result<RuntimeObjectRecord, RuntimeStateError> {
        if !is_payload_close_handle_kind(key.kind()) {
            return Err(RuntimeStateError::invalid_input(
                "runtime-object",
                "runtime-close-handle-object-retirement-kind-invalid",
                "payload retirement requires a supported close-handle object identity",
            ));
        }
        let id = self.identity_index.get(key).copied().ok_or_else(|| {
            RuntimeStateError::not_found(
                "runtime-object",
                "runtime-object-identity-unknown",
                "close-handle object identity is not registered",
            )
        })?;
        let record = self.objects.get(&id).cloned().ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-object",
                "runtime-object-index-dangling",
                "close-handle object identity index references a missing object",
            )
        })?;
        if record.open_handles != 0
            || record.close_policy != RuntimeHandleClosePolicy::CloseHandle
            || self.handles.values().any(|entry| entry.object_id == id)
        {
            return Err(RuntimeStateError::conflict(
                "runtime-object",
                "runtime-close-handle-object-retirement-handles-live",
                "close-handle object cannot retire while a handle remains open",
            ));
        }
        self.identity_index.remove(key);
        self.objects.remove(&id).ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-object",
                "runtime-object-retirement-missing",
                "preflighted close-handle object disappeared during retirement",
            )
        })
    }

    pub(super) fn retire_close_handle_objects(
        &mut self,
        keys: &[RuntimeObjectKey],
    ) -> Result<usize, RuntimeStateError> {
        let mut unique_keys = BTreeSet::new();
        let mut retired_handles = 0;
        for key in keys {
            if !unique_keys.insert(key.clone()) {
                return Err(RuntimeStateError::invalid_input(
                    "runtime-object",
                    "runtime-close-handle-object-retirement-duplicate",
                    "process payload teardown contains a duplicate object identity",
                ));
            }
            if !is_payload_close_handle_kind(key.kind()) {
                return Err(RuntimeStateError::invalid_input(
                    "runtime-object",
                    "runtime-close-handle-object-retirement-kind-invalid",
                    "process payload teardown received an unsupported object identity",
                ));
            }
            let id = self.identity_index.get(key).copied().ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-object",
                    "runtime-object-identity-unknown",
                    "process payload teardown targets an unregistered object",
                )
            })?;
            let handles = self
                .handles
                .values()
                .filter(|entry| entry.object_id == id)
                .map(|entry| entry.handle)
                .collect::<Vec<_>>();
            for handle in handles {
                self.close_handle(handle)?;
                retired_handles += 1;
            }
            self.remove_closed_close_handle_object(key)?;
        }
        Ok(retired_handles)
    }

    pub(super) fn retire_subsystem_objects(
        &mut self,
        keys: &[RuntimeObjectKey],
    ) -> Result<Vec<RuntimeHandleEntry>, RuntimeStateError> {
        self.retire_objects_with_policy(keys, RuntimeHandleClosePolicy::SubsystemOwned)
    }

    fn retire_objects_with_policy(
        &mut self,
        keys: &[RuntimeObjectKey],
        expected_policy: RuntimeHandleClosePolicy,
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
            if record.close_policy != expected_policy {
                return Err(RuntimeStateError::conflict(
                    "runtime-object",
                    "runtime-object-retirement-policy",
                    "runtime object must be retired through its declared close policy",
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

    pub(super) fn wait_state(
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

    pub(super) fn contains_identity(&self, key: &RuntimeObjectKey) -> bool {
        self.identity_index.contains_key(key)
    }

    pub(super) fn object_for_identity(
        &self,
        key: &RuntimeObjectKey,
    ) -> Result<&RuntimeObjectRecord, RuntimeStateError> {
        let id = self.identity_index.get(key).copied().ok_or_else(|| {
            RuntimeStateError::not_found(
                "runtime-object",
                "runtime-object-identity-unknown",
                "runtime object identity is not registered",
            )
        })?;
        self.objects.get(&id).ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-object",
                "runtime-object-index-dangling",
                "runtime object identity index references a missing object",
            )
        })
    }

    pub fn objects(&self) -> &BTreeMap<RuntimeObjectId, RuntimeObjectRecord> {
        &self.objects
    }

    pub fn handles(&self) -> &BTreeMap<u32, RuntimeHandleEntry> {
        &self.handles
    }

    pub(super) fn plan_wait_state_updates(
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

    pub(super) fn commit_wait_state_updates(
        &mut self,
        updates: Vec<(RuntimeObjectId, RuntimeObjectRecord)>,
    ) {
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

fn runtime_object_record(
    id: RuntimeObjectId,
    key: RuntimeObjectKey,
    open_handles: usize,
) -> RuntimeObjectRecord {
    let kind = key.kind();
    let (namespace, close_policy, wait_state) = match kind {
        RuntimeObjectKind::Process
        | RuntimeObjectKind::Thread
        | RuntimeObjectKind::Mutex
        | RuntimeObjectKind::WaitableTimer
        | RuntimeObjectKind::File => (
            match kind {
                RuntimeObjectKind::Process => RUNTIME_PROCESS_OBJECT_NAMESPACE,
                RuntimeObjectKind::Thread => RUNTIME_THREAD_OBJECT_NAMESPACE,
                RuntimeObjectKind::Mutex => RUNTIME_MUTEX_OBJECT_NAMESPACE,
                RuntimeObjectKind::WaitableTimer => RUNTIME_WAITABLE_TIMER_OBJECT_NAMESPACE,
                RuntimeObjectKind::File => RUNTIME_SOURCE_FILE_NAMESPACE,
                RuntimeObjectKind::FileSearch
                | RuntimeObjectKind::PrivateHeap
                | RuntimeObjectKind::Module => unreachable!(),
            },
            RuntimeHandleClosePolicy::CloseHandle,
            matches!(
                kind,
                RuntimeObjectKind::Process
                    | RuntimeObjectKind::Thread
                    | RuntimeObjectKind::WaitableTimer
            )
            .then_some(RuntimeWaitState::Nonsignaled),
        ),
        RuntimeObjectKind::FileSearch => (
            RUNTIME_FILE_SEARCH_NAMESPACE,
            RuntimeHandleClosePolicy::FindClose,
            None,
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
    RuntimeObjectRecord {
        id,
        key,
        kind,
        namespace: namespace.to_owned(),
        close_policy,
        wait_state,
        open_handles,
    }
}

const fn runtime_handle_slot_key(kind: RuntimeObjectKind, slot: u16) -> u32 {
    (kind.handle_tag() as u32) << 16 | slot as u32
}

const fn is_payload_close_handle_kind(kind: RuntimeObjectKind) -> bool {
    matches!(
        kind,
        RuntimeObjectKind::Mutex | RuntimeObjectKind::WaitableTimer | RuntimeObjectKind::File
    )
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wait_state_plan_is_mutation_free_until_commit() {
        let mut objects = RuntimeObjectTable::default();
        let key = RuntimeObjectKey::Thread { thread_id: 0x2000 };
        let (_, handle) = objects
            .register_object_and_open_handle(key.clone())
            .expect("thread object and handle");
        let before = objects.clone();

        let plan = objects
            .plan_wait_state_updates(&[(key, RuntimeWaitState::Signaled { exit_code: 7 })])
            .expect("wait state plan");
        assert_eq!(objects, before, "planning must not mutate object state");

        objects.commit_wait_state_updates(plan);
        assert_eq!(
            objects.wait_state(handle, RuntimeObjectKind::Thread),
            Ok(RuntimeWaitState::Signaled { exit_code: 7 })
        );
    }

    #[test]
    fn failed_exact_handle_registration_restores_the_object_id_allocator() {
        let mut objects = RuntimeObjectTable::default();
        let handle =
            RuntimeHandle::from_parts(RuntimeObjectKind::Thread, 1, 1).expect("controlled handle");
        objects.handles.insert(
            handle.0,
            RuntimeHandleEntry {
                handle,
                object_id: RuntimeObjectId(999),
                kind: RuntimeObjectKind::Thread,
            },
        );
        let before = objects.clone();

        let error = objects
            .register_object_with_handle(RuntimeObjectKey::Thread { thread_id: 0x2000 }, handle)
            .expect_err("dangling handle collision must fail");
        assert_eq!(error.code, "runtime-handle-entry-collision");
        assert_eq!(objects, before, "failed registration must be atomic");
    }

    #[test]
    fn object_and_handle_registration_plan_is_pure_and_rejects_stale_commit() {
        let mut objects = RuntimeObjectTable::default();
        let before = objects.clone();
        let stale = objects
            .plan_object_and_handle_registration(RuntimeObjectKey::Thread { thread_id: 0x2000 })
            .expect("thread object registration plan");
        assert_eq!(
            objects, before,
            "registration planning must not mutate tables"
        );
        assert_eq!(stale.handle(), RuntimeHandle(0x5401_0001));

        objects
            .register_object_and_open_handle(RuntimeObjectKey::Process { process_id: 0x1000 })
            .expect("intervening process object registration");
        let before_stale_preflight = objects.clone();
        assert_eq!(
            objects
                .preflight_object_registration_commit(&stale)
                .expect_err("intervening object identity must stale the plan")
                .code,
            "runtime-object-registration-plan-stale"
        );
        assert_eq!(objects, before_stale_preflight);

        let fresh = objects
            .plan_object_and_handle_registration(RuntimeObjectKey::Thread { thread_id: 0x2000 })
            .expect("fresh thread registration plan");
        objects
            .preflight_object_registration_commit(&fresh)
            .expect("fresh registration preflight");
        let (object_id, handle) = objects.commit_object_registration(fresh);
        assert_eq!(object_id, RuntimeObjectId(2));
        assert_eq!(handle, RuntimeHandle(0x5401_0001));
    }

    #[test]
    fn close_handle_object_plans_are_atomic_and_reject_stale_commits() {
        let mut objects = RuntimeObjectTable::default();
        let key = RuntimeObjectKey::Mutex { mutex_id: 7 };
        let initial = objects.clone();
        let first = objects
            .plan_close_handle_object_open(key.clone(), false)
            .expect("new mutex object plan");
        let first_handle = first.opened_handle();
        assert_eq!(objects, initial, "open planning must not mutate tables");
        objects
            .commit_close_handle_object_mutation(first)
            .expect("commit first mutex handle");

        let stale_close = objects
            .plan_close_handle_object_close(first_handle, Some(&key))
            .expect("last-handle close plan");
        let second = objects
            .plan_close_handle_object_open(key.clone(), true)
            .expect("existing mutex handle plan");
        objects
            .commit_close_handle_object_mutation(second)
            .expect("commit second mutex handle");
        let before_stale_commit = objects.clone();
        assert_eq!(
            objects
                .commit_close_handle_object_mutation(stale_close)
                .expect_err("stale close plan must fail")
                .code,
            "runtime-close-handle-object-plan-stale"
        );
        assert_eq!(objects, before_stale_commit);

        let timer_key = RuntimeObjectKey::WaitableTimer { timer_id: 11 };
        let timer = objects
            .plan_close_handle_object_open(timer_key.clone(), false)
            .expect("new waitable timer object plan");
        assert_eq!(timer.opened_handle(), RuntimeHandle(0x5701_0001));
        objects
            .commit_close_handle_object_mutation(timer)
            .expect("commit waitable timer handle");
        let before_teardown = objects.clone();
        let teardown = objects
            .plan_payload_object_teardown(&[key, timer_key], &[])
            .expect("payload teardown plan");
        assert_eq!(teardown.retired_handles(), 3);
        assert_eq!(objects, before_teardown, "teardown planning must be pure");
        objects
            .commit_close_handle_object_mutation(teardown)
            .expect("commit mutex teardown");
        assert!(objects.objects().is_empty());
        assert!(objects.handles().is_empty());
    }

    #[test]
    fn subsystem_retirement_preflights_the_entire_batch() {
        let mut objects = RuntimeObjectTable::default();
        let first_key = RuntimeObjectKey::PrivateHeap {
            handle: 0x6801_0001,
        };
        let second_key = RuntimeObjectKey::PrivateHeap {
            handle: 0x6801_0002,
        };
        for (key, handle) in [
            (first_key.clone(), RuntimeHandle(0x6801_0001)),
            (second_key.clone(), RuntimeHandle(0x6801_0002)),
        ] {
            objects
                .register_object_with_handle(key, handle)
                .expect("subsystem-owned object");
        }
        let before = objects.clone();

        let error = objects
            .retire_subsystem_objects(&[
                first_key.clone(),
                RuntimeObjectKey::PrivateHeap {
                    handle: 0x6801_0003,
                },
            ])
            .expect_err("unknown second identity must reject the batch");
        assert_eq!(error.code, "runtime-object-identity-unknown");
        assert_eq!(
            objects, before,
            "failed batch preflight must not retire the first object"
        );

        let retired = objects
            .retire_subsystem_objects(&[first_key, second_key])
            .expect("complete retirement batch");
        assert_eq!(retired.len(), 2);
        assert!(objects.objects().is_empty());
        assert!(objects.handles().is_empty());
    }
}

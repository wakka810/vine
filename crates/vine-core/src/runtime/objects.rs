use std::collections::BTreeMap;

use super::RuntimeStateError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RuntimeObjectKind {
    Process,
    Thread,
    Mutex,
    Event,
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
            Self::Event => 0x45,
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
            0x45 => Some(Self::Event),
            0x57 => Some(Self::WaitableTimer),
            0x46 => Some(Self::File),
            0x53 => Some(Self::FileSearch),
            0x68 => Some(Self::PrivateHeap),
            0x4d => Some(Self::Module),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct RuntimeHandle(pub u32);

impl RuntimeHandle {
    pub fn from_parts(
        kind: RuntimeObjectKind,
        generation: u8,
        slot: u16,
    ) -> Result<Self, RuntimeStateError> {
        if generation == 0 || slot == 0 {
            return Err(RuntimeStateError::new(
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
            RuntimeStateError::new(format!(
                "runtime handle {:#010x} has unknown type tag {tag:#04x}",
                self.0
            ))
        })
    }

    pub const fn generation(self) -> u8 {
        ((self.0 >> 16) & 0xff) as u8
    }

    pub const fn slot(self) -> u16 {
        (self.0 & 0xffff) as u16
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum RuntimeObjectKey {
    Process { process_id: u32 },
    Thread { thread_id: u32 },
    Mutex { mutex_id: u64 },
    Event { event_id: u64, manual_reset: bool },
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
            Self::Event { .. } => RuntimeObjectKind::Event,
            Self::WaitableTimer { .. } => RuntimeObjectKind::WaitableTimer,
            Self::File { .. } => RuntimeObjectKind::File,
            Self::FileSearch { .. } => RuntimeObjectKind::FileSearch,
            Self::PrivateHeap { .. } => RuntimeObjectKind::PrivateHeap,
            Self::Module { .. } => RuntimeObjectKind::Module,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct RuntimeObjectId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeHandleClosePolicy {
    CloseHandle,
    FindClose,
    SubsystemOwned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeWaitState {
    Nonsignaled,
    Signaled { exit_code: u32 },
}

#[derive(Debug, Clone)]
#[cfg_attr(test, derive(PartialEq, Eq))]
pub struct RuntimeObjectRecord {
    pub id: RuntimeObjectId,
    pub key: RuntimeObjectKey,
    pub kind: RuntimeObjectKind,
    pub close_policy: RuntimeHandleClosePolicy,
    pub wait_state: Option<RuntimeWaitState>,
    pub open_handles: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeEvent {
    pub event_id: u64,
    pub manual_reset: bool,
    pub wait_state: RuntimeWaitState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeHandleEntry {
    pub handle: RuntimeHandle,
    pub object_id: RuntimeObjectId,
    pub kind: RuntimeObjectKind,
    pub inheritable: bool,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RuntimeHandleSlot {
    generation: u8,
    live: bool,
    retired: bool,
}

impl RuntimeHandleSlot {
    fn close(&mut self) {
        self.live = false;
        if self.generation == u8::MAX {
            self.retired = true;
        } else {
            self.generation += 1;
        }
    }
}

#[derive(Debug, Clone)]
#[cfg_attr(test, derive(PartialEq, Eq))]
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

#[derive(Debug, Clone)]
#[cfg_attr(test, derive(PartialEq, Eq))]
pub(super) struct RuntimeObjectTable {
    objects: BTreeMap<RuntimeObjectId, RuntimeObjectRecord>,
    handles: BTreeMap<u32, RuntimeHandleEntry>,
    slots: BTreeMap<u32, RuntimeHandleSlot>,
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
    pub(super) fn register_event(
        &mut self,
        manual_reset: bool,
        initial_state: bool,
    ) -> Result<RuntimeHandle, RuntimeStateError> {
        let event_id = self.next_object_id;
        let key = RuntimeObjectKey::Event {
            event_id,
            manual_reset,
        };
        let handle = self.next_available_handle(RuntimeObjectKind::Event)?;
        let (id, next_object_id) = self.validate_new_object_with_handle(&key, handle)?;
        let wait_state = if initial_state {
            RuntimeWaitState::Signaled { exit_code: 0 }
        } else {
            RuntimeWaitState::Nonsignaled
        };
        let mut record = runtime_object_record(id, key, 1);
        record.wait_state = Some(wait_state);
        self.publish_registration(record, handle, next_object_id);
        Ok(handle)
    }

    pub(super) fn plan_object_and_handle_registration(
        &self,
        key: RuntimeObjectKey,
    ) -> Result<RuntimeObjectRegistrationPlan, RuntimeStateError> {
        let kind = key.kind();
        let handle = self.next_available_handle(kind)?;
        let (id, next_object_id) = self.validate_new_object_with_handle(&key, handle)?;
        let slot_key = runtime_handle_slot_key(kind, handle.slot());
        Ok(RuntimeObjectRegistrationPlan {
            expected_next_object_id: self.next_object_id,
            next_object_id,
            record: runtime_object_record(id, key, 1),
            handle_entry: RuntimeHandleEntry {
                handle,
                object_id: id,
                kind,
                inheritable: false,
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
            return Err(RuntimeStateError::new(
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
        let id = plan.record.id;
        let handle = plan.handle_entry.handle;
        self.publish_registration(plan.record, handle, plan.next_object_id);
        (id, handle)
    }

    fn publish_registration(
        &mut self,
        record: RuntimeObjectRecord,
        handle: RuntimeHandle,
        next_object_id: u64,
    ) {
        let id = record.id;
        let kind = record.kind;
        self.next_object_id = next_object_id;
        self.identity_index.insert(record.key.clone(), id);
        self.objects.insert(id, record);
        self.handles.insert(
            handle.0,
            RuntimeHandleEntry {
                handle,
                object_id: id,
                kind,
                inheritable: false,
            },
        );
        self.slots.insert(
            runtime_handle_slot_key(kind, handle.slot()),
            RuntimeHandleSlot {
                generation: handle.generation(),
                live: true,
                retired: false,
            },
        );
    }

    pub(super) fn close_payload_handle(
        &mut self,
        handle: RuntimeHandle,
    ) -> Result<(), RuntimeStateError> {
        let kind = handle.kind()?;
        if !is_payload_close_handle_kind(kind) {
            return Err(RuntimeStateError::new(
                "payload close requires a supported object kind",
            ));
        }
        let entry = *self.lookup_entry(handle, kind)?;
        let record = self.object_for_handle(handle, kind)?;
        if record.close_policy != RuntimeHandleClosePolicy::CloseHandle
            || self
                .handles
                .values()
                .filter(|other| other.object_id == record.id)
                .count()
                != record.open_handles
        {
            return Err(RuntimeStateError::new(
                "payload close policy or live handle count is inconsistent",
            ));
        }
        let retiring_key = (record.open_handles == 1).then(|| record.key.clone());
        self.close_handle(handle)?;
        if let Some(key) = retiring_key {
            self.identity_index.remove(&key);
            self.objects.remove(&entry.object_id);
        }
        Ok(())
    }

    pub(super) fn retire_module(&mut self, handle: RuntimeHandle) -> Result<(), RuntimeStateError> {
        self.retire_unique_owned_object(handle, RuntimeObjectKind::Module)
    }

    pub(super) fn retire_private_heap(
        &mut self,
        handle: RuntimeHandle,
    ) -> Result<(), RuntimeStateError> {
        self.retire_unique_owned_object(handle, RuntimeObjectKind::PrivateHeap)
    }

    fn retire_unique_owned_object(
        &mut self,
        handle: RuntimeHandle,
        kind: RuntimeObjectKind,
    ) -> Result<(), RuntimeStateError> {
        let record = self.object_for_handle(handle, kind)?;
        if record.open_handles != 1 {
            return Err(RuntimeStateError::new(
                "owned object must have exactly one logical handle",
            ));
        }
        let id = record.id;
        let key = record.key.clone();
        self.slots
            .get_mut(&runtime_handle_slot_key(kind, handle.slot()))
            .expect("live owned object slot")
            .close();
        self.handles.remove(&handle.0);
        self.objects.remove(&id);
        self.identity_index.remove(&key);
        Ok(())
    }

    pub(super) fn find_close(&mut self, handle: RuntimeHandle) -> Result<(), RuntimeStateError> {
        if handle.kind()? != RuntimeObjectKind::FileSearch {
            return Err(RuntimeStateError::new(
                "FindClose requires a file-search handle",
            ));
        }
        let record = self.object_for_handle(handle, RuntimeObjectKind::FileSearch)?;
        if record.open_handles != 1 {
            return Err(RuntimeStateError::new(
                "FindClose requires the sole handle for its file-search identity",
            ));
        }
        if record.close_policy != RuntimeHandleClosePolicy::FindClose
            || self
                .handles
                .values()
                .filter(|entry| entry.object_id == record.id)
                .count()
                != 1
        {
            return Err(RuntimeStateError::new(
                "file-search close policy or live handle count is inconsistent",
            ));
        }
        let id = record.id;
        let key = record.key.clone();
        let slot_key = runtime_handle_slot_key(RuntimeObjectKind::FileSearch, handle.slot());
        let slot = self
            .slots
            .get_mut(&slot_key)
            .expect("validated search handle slot remains live");
        slot.close();
        self.handles.remove(&handle.0);
        self.objects.remove(&id);
        self.identity_index.remove(&key);
        Ok(())
    }

    pub(super) fn register_object_with_handle(
        &mut self,
        key: RuntimeObjectKey,
        handle: RuntimeHandle,
    ) -> Result<RuntimeObjectId, RuntimeStateError> {
        let (id, next_object_id) = self.validate_new_object_with_handle(&key, handle)?;
        self.publish_registration(runtime_object_record(id, key, 1), handle, next_object_id);
        Ok(id)
    }

    fn validate_new_object_with_handle(
        &self,
        key: &RuntimeObjectKey,
        handle: RuntimeHandle,
    ) -> Result<(RuntimeObjectId, u64), RuntimeStateError> {
        self.try_validate_new_object_with_handle(key, handle)?
            .ok_or_else(|| RuntimeStateError::new("runtime object ID space is exhausted"))
    }

    fn try_validate_new_object_with_handle(
        &self,
        key: &RuntimeObjectKey,
        handle: RuntimeHandle,
    ) -> Result<Option<(RuntimeObjectId, u64)>, RuntimeStateError> {
        self.preflight_exact_handle(key.kind(), handle)?;
        if self.identity_index.contains_key(key) {
            return Err(RuntimeStateError::new(
                "runtime object identity is already registered",
            ));
        }
        let id = RuntimeObjectId(self.next_object_id);
        if self.objects.contains_key(&id) {
            return Err(RuntimeStateError::new(
                "next runtime object identity is already occupied",
            ));
        }
        if self.handles.contains_key(&handle.0) {
            return Err(RuntimeStateError::new(
                "free runtime handle slot still has a table entry",
            ));
        }
        let Some(next_object_id) = self.next_object_id.checked_add(1) else {
            return Ok(None);
        };
        Ok(Some((id, next_object_id)))
    }

    pub(super) fn register_object_and_open_handle(
        &mut self,
        key: RuntimeObjectKey,
    ) -> Result<(RuntimeObjectId, RuntimeHandle), RuntimeStateError> {
        self.try_register_object_and_open_handle(key)?
            .ok_or_else(|| RuntimeStateError::new("runtime object or handle capacity is exhausted"))
    }

    pub(super) fn try_register_object_and_open_handle(
        &mut self,
        key: RuntimeObjectKey,
    ) -> Result<Option<(RuntimeObjectId, RuntimeHandle)>, RuntimeStateError> {
        let Some(handle) = self.try_next_available_handle(key.kind())? else {
            return Ok(None);
        };
        let Some((id, next_object_id)) = self.try_validate_new_object_with_handle(&key, handle)?
        else {
            return Ok(None);
        };
        self.publish_registration(runtime_object_record(id, key, 1), handle, next_object_id);
        Ok(Some((id, handle)))
    }

    pub(super) fn open_handle(
        &mut self,
        key: &RuntimeObjectKey,
    ) -> Result<RuntimeHandle, RuntimeStateError> {
        let object_id =
            self.identity_index.get(key).copied().ok_or_else(|| {
                RuntimeStateError::new("runtime object identity is not registered")
            })?;
        let record = self.objects.get(&object_id).ok_or_else(|| {
            RuntimeStateError::new("object identity index references a missing object")
        })?;
        let kind = key.kind();
        if record.id != object_id || &record.key != key || record.kind != kind {
            return Err(RuntimeStateError::new(
                "indexed object does not match its live identity",
            ));
        }
        let next_count = record
            .open_handles
            .checked_add(1)
            .ok_or_else(|| RuntimeStateError::new("runtime object handle count overflowed"))?;
        let handle = self.next_available_handle(kind)?;
        self.preflight_exact_handle(kind, handle)?;
        if self.handles.contains_key(&handle.0) {
            return Err(RuntimeStateError::new(
                "free runtime handle slot still has a table entry",
            ));
        }
        self.slots.insert(
            runtime_handle_slot_key(kind, handle.slot()),
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
                inheritable: false,
            },
        );
        self.objects
            .get_mut(&object_id)
            .expect("validated handle object remains live")
            .open_handles = next_count;
        Ok(handle)
    }

    pub(super) fn set_new_handle_inheritable(&mut self, handle: RuntimeHandle, inheritable: bool) {
        self.handles
            .get_mut(&handle.0)
            .expect("newly published live handle")
            .inheritable = inheritable;
    }

    #[cfg(test)]
    pub(crate) fn handle_for_object(
        &self,
        key: &RuntimeObjectKey,
    ) -> Result<RuntimeHandle, RuntimeStateError> {
        let object_id =
            self.identity_index.get(key).copied().ok_or_else(|| {
                RuntimeStateError::new("runtime object identity is not registered")
            })?;
        self.handles
            .values()
            .find(|entry| entry.object_id == object_id)
            .map(|entry| entry.handle)
            .ok_or_else(|| RuntimeStateError::new("runtime object has no open handle"))
    }

    pub(super) fn is_live_handle(
        &self,
        handle: RuntimeHandle,
        expected_kind: RuntimeObjectKind,
    ) -> Result<bool, RuntimeStateError> {
        self.live_object_for_handle(handle, expected_kind)
            .map(|object| object.is_some())
    }

    pub(super) fn live_object_for_handle(
        &self,
        handle: RuntimeHandle,
        expected_kind: RuntimeObjectKind,
    ) -> Result<Option<&RuntimeObjectRecord>, RuntimeStateError> {
        if !matches!(handle.kind(), Ok(kind) if kind == expected_kind)
            || handle.slot() == 0
            || handle.generation() == 0
        {
            return Ok(None);
        }
        let slot_key = runtime_handle_slot_key(expected_kind, handle.slot());
        let live = self.slots.get(&slot_key).is_some_and(|slot| {
            slot.live && !slot.retired && slot.generation == handle.generation()
        });
        if !live {
            if self.handles.contains_key(&handle.0) {
                return Err(RuntimeStateError::new(
                    "runtime handle entry has no matching live slot",
                ));
            }
            return Ok(None);
        }
        Ok(Some(self.object_for_handle(handle, expected_kind)?))
    }

    pub(super) fn object_for_handle(
        &self,
        handle: RuntimeHandle,
        expected_kind: RuntimeObjectKind,
    ) -> Result<&RuntimeObjectRecord, RuntimeStateError> {
        let entry = self.lookup_entry(handle, expected_kind)?;
        let record = self.objects.get(&entry.object_id).ok_or_else(|| {
            RuntimeStateError::new("live runtime handle references a missing object")
        })?;
        if entry.handle != handle
            || entry.kind != expected_kind
            || record.kind != expected_kind
            || record.key.kind() != expected_kind
            || record.id != entry.object_id
            || record.open_handles == 0
            || self.identity_index.get(&record.key) != Some(&record.id)
        {
            return Err(RuntimeStateError::new(
                "live handle, object, and identity indexes disagree",
            ));
        }
        Ok(record)
    }

    pub(super) fn close_handle(&mut self, handle: RuntimeHandle) -> Result<(), RuntimeStateError> {
        let kind = handle.kind()?;
        let entry = *self.lookup_entry(handle, kind)?;
        let record = self.object_for_handle(handle, kind)?;
        if record.close_policy != RuntimeHandleClosePolicy::CloseHandle {
            return Err(RuntimeStateError::new(
                "runtime handle must be retired by its owning subsystem",
            ));
        }
        let slot_key = runtime_handle_slot_key(kind, handle.slot());
        self.slots
            .get_mut(&slot_key)
            .expect("validated handle slot remains live")
            .close();
        self.objects
            .get_mut(&entry.object_id)
            .expect("validated handle object remains live")
            .open_handles -= 1;
        self.handles.remove(&handle.0);
        Ok(())
    }

    pub(super) fn wait_state(
        &self,
        handle: RuntimeHandle,
        expected_kind: RuntimeObjectKind,
    ) -> Result<RuntimeWaitState, RuntimeStateError> {
        self.object_for_handle(handle, expected_kind)?
            .wait_state
            .ok_or_else(|| {
                RuntimeStateError::new("runtime object type does not expose a wait state")
            })
    }

    pub(super) fn contains_identity(&self, key: &RuntimeObjectKey) -> bool {
        self.identity_index.contains_key(key)
    }

    pub(super) fn object_for_identity(
        &self,
        key: &RuntimeObjectKey,
    ) -> Result<&RuntimeObjectRecord, RuntimeStateError> {
        let id =
            self.identity_index.get(key).copied().ok_or_else(|| {
                RuntimeStateError::new("runtime object identity is not registered")
            })?;
        let record = self.objects.get(&id).ok_or_else(|| {
            RuntimeStateError::new("runtime object identity index references a missing object")
        })?;
        if record.id != id || record.key != *key || record.kind != key.kind() {
            return Err(RuntimeStateError::new(
                "runtime object identity index does not match its live record",
            ));
        }
        Ok(record)
    }

    #[cfg(test)]
    pub(super) fn objects(&self) -> &BTreeMap<RuntimeObjectId, RuntimeObjectRecord> {
        &self.objects
    }

    pub(super) fn set_wait_state(
        &mut self,
        handle: RuntimeHandle,
        kind: RuntimeObjectKind,
        state: RuntimeWaitState,
    ) -> Result<(), RuntimeStateError> {
        let record = self.object_for_handle(handle, kind)?;
        if record.wait_state.is_none() {
            return Err(RuntimeStateError::new(
                "runtime object type does not expose a wait state",
            ));
        }
        let id = record.id;
        self.objects
            .get_mut(&id)
            .expect("validated wait object remains live")
            .wait_state = Some(state);
        Ok(())
    }

    pub(super) fn plan_wait_state_updates(
        &self,
        updates: &[(RuntimeObjectKey, RuntimeWaitState)],
    ) -> Result<Vec<(RuntimeObjectId, RuntimeWaitState)>, RuntimeStateError> {
        let mut planned: Vec<(RuntimeObjectId, RuntimeWaitState)> =
            Vec::with_capacity(updates.len());
        for (key, state) in updates {
            let id = self.identity_index.get(key).copied().ok_or_else(|| {
                RuntimeStateError::new("runtime wait state targets an unregistered object")
            })?;
            if planned.iter().any(|(planned_id, _)| *planned_id == id) {
                return Err(RuntimeStateError::new(
                    "one runtime wait object has duplicate state updates",
                ));
            }
            let record = self.objects.get(&id).ok_or_else(|| {
                RuntimeStateError::new("runtime object identity index references a missing object")
            })?;
            if record.id != id || &record.key != key || record.kind != key.kind() {
                return Err(RuntimeStateError::new(
                    "runtime wait object does not match its live identity index",
                ));
            }
            if record.wait_state.is_none() {
                return Err(RuntimeStateError::new(
                    "runtime object type does not expose a wait state",
                ));
            }
            planned.push((id, *state));
        }
        Ok(planned)
    }

    pub(super) fn wait_state_mut_for_identity(
        &mut self,
        key: &RuntimeObjectKey,
    ) -> Result<(RuntimeObjectId, &mut RuntimeWaitState), RuntimeStateError> {
        let id = self.object_for_identity(key)?.id;
        let state = self
            .objects
            .get_mut(&id)
            .expect("validated wait object")
            .wait_state
            .as_mut()
            .ok_or_else(|| {
                RuntimeStateError::new("runtime object type does not expose a wait state")
            })?;
        Ok((id, state))
    }

    pub(super) fn commit_wait_state_updates(
        &mut self,
        updates: Vec<(RuntimeObjectId, RuntimeWaitState)>,
    ) {
        for (id, state) in updates {
            self.objects
                .get_mut(&id)
                .expect("validated wait object must remain live")
                .wait_state = Some(state);
        }
    }

    pub(super) fn next_available_handle(
        &self,
        kind: RuntimeObjectKind,
    ) -> Result<RuntimeHandle, RuntimeStateError> {
        self.try_next_available_handle(kind)?.ok_or_else(|| {
            RuntimeStateError::new("runtime handle slot space is exhausted for this object type")
        })
    }

    fn try_next_available_handle(
        &self,
        kind: RuntimeObjectKind,
    ) -> Result<Option<RuntimeHandle>, RuntimeStateError> {
        for slot in 1..=u16::MAX {
            if let Some(handle) = self.available_handle_in_slot(kind, slot)? {
                return Ok(Some(handle));
            }
        }
        Ok(None)
    }

    pub(super) fn available_handle_in_slot(
        &self,
        kind: RuntimeObjectKind,
        slot: u16,
    ) -> Result<Option<RuntimeHandle>, RuntimeStateError> {
        let generation = match self.slots.get(&runtime_handle_slot_key(kind, slot)) {
            Some(state) if !state.live && !state.retired => state.generation,
            Some(_) => return Ok(None),
            None => 1,
        };
        RuntimeHandle::from_parts(kind, generation, slot).map(Some)
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
            return Err(RuntimeStateError::new(
                "runtime handles require nonzero generation and slot fields",
            ));
        }
        let slot_key = runtime_handle_slot_key(kind, handle.slot());
        match self.slots.get(&slot_key) {
            Some(state) if state.live => Err(RuntimeStateError::new(
                "runtime handle slot is already live",
            )),
            Some(state) if state.retired || state.generation != handle.generation() => {
                Err(RuntimeStateError::new(
                    "runtime handle generation does not match the current slot generation",
                ))
            }
            None if handle.generation() != 1 => Err(RuntimeStateError::new(
                "a new runtime handle slot must begin at generation one",
            )),
            _ => Ok(()),
        }
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
            return Err(RuntimeStateError::new(
                "runtime handles require nonzero generation and slot fields",
            ));
        }
        let slot_key = runtime_handle_slot_key(kind, handle.slot());
        let slot = self.slots.get(&slot_key).ok_or_else(|| {
            RuntimeStateError::new(format!(
                "runtime handle {:#010x} was never allocated",
                handle.0
            ))
        })?;
        if slot.retired || slot.generation != handle.generation() || !slot.live {
            return Err(RuntimeStateError::new(format!(
                "runtime handle {:#010x} is closed or stale",
                handle.0
            )));
        }
        self.handles
            .get(&handle.0)
            .ok_or_else(|| RuntimeStateError::new("live runtime handle slot has no table entry"))
    }
}

fn runtime_object_record(
    id: RuntimeObjectId,
    key: RuntimeObjectKey,
    open_handles: usize,
) -> RuntimeObjectRecord {
    let kind = key.kind();
    let (close_policy, wait_state) = match kind {
        RuntimeObjectKind::Process
        | RuntimeObjectKind::Thread
        | RuntimeObjectKind::Mutex
        | RuntimeObjectKind::Event
        | RuntimeObjectKind::WaitableTimer
        | RuntimeObjectKind::File => (
            RuntimeHandleClosePolicy::CloseHandle,
            matches!(
                kind,
                RuntimeObjectKind::Process
                    | RuntimeObjectKind::Thread
                    | RuntimeObjectKind::Event
                    | RuntimeObjectKind::WaitableTimer
            )
            .then_some(RuntimeWaitState::Nonsignaled),
        ),
        RuntimeObjectKind::FileSearch => (RuntimeHandleClosePolicy::FindClose, None),
        RuntimeObjectKind::PrivateHeap | RuntimeObjectKind::Module => {
            (RuntimeHandleClosePolicy::SubsystemOwned, None)
        }
    };
    RuntimeObjectRecord {
        id,
        key,
        kind,
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
        RuntimeObjectKind::Mutex
            | RuntimeObjectKind::Event
            | RuntimeObjectKind::WaitableTimer
            | RuntimeObjectKind::File
    )
}

fn runtime_handle_type_mismatch(
    handle: RuntimeHandle,
    expected: RuntimeObjectKind,
    actual: RuntimeObjectKind,
) -> RuntimeStateError {
    RuntimeStateError::new(format!(
        "runtime handle {:#010x} has type {actual:?}, expected {expected:?}",
        handle.0
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closed_handles_cannot_be_revived_by_repeated_reuse() {
        let mut objects = RuntimeObjectTable::default();
        let key = RuntimeObjectKey::Thread { thread_id: 7 };
        let (_, first) = objects
            .register_object_and_open_handle(key.clone())
            .unwrap();
        let mut current = first;
        for _ in 0..300 {
            objects.close_handle(current).unwrap();
            assert!(objects.close_handle(current).is_err());
            current = objects.open_handle(&key).unwrap();
            assert!(
                objects
                    .object_for_handle(first, RuntimeObjectKind::Thread)
                    .is_err()
            );
            assert_eq!(
                objects.wait_state(current, RuntimeObjectKind::Thread),
                Ok(RuntimeWaitState::Nonsignaled)
            );
        }
    }

    #[test]
    fn invalid_duplicate_and_wrong_type_registration_keeps_live_handles_usable() {
        let mut objects = RuntimeObjectTable::default();
        let key = RuntimeObjectKey::Thread { thread_id: 7 };
        assert!(objects.open_handle(&key).is_err());
        let (_, handle) = objects
            .register_object_and_open_handle(key.clone())
            .unwrap();
        assert!(objects.register_object_and_open_handle(key).is_err());
        let other = RuntimeObjectKey::Thread { thread_id: 8 };
        for invalid in [RuntimeHandle(0), RuntimeHandle(u32::MAX), handle] {
            assert!(
                objects
                    .register_object_with_handle(other.clone(), invalid)
                    .is_err()
            );
            assert_eq!(
                objects.wait_state(handle, RuntimeObjectKind::Thread),
                Ok(RuntimeWaitState::Nonsignaled)
            );
        }
        assert!(
            !objects
                .is_live_handle(handle, RuntimeObjectKind::File)
                .unwrap()
        );
        assert!(
            !objects
                .is_live_handle(RuntimeHandle(0), RuntimeObjectKind::Thread)
                .unwrap()
        );
        let (_, next) = objects.register_object_and_open_handle(other).unwrap();
        assert_ne!(next, handle);
        objects.close_handle(next).unwrap();
        assert!(
            !objects
                .is_live_handle(next, RuntimeObjectKind::Thread)
                .unwrap()
        );
        assert!(
            objects
                .is_live_handle(handle, RuntimeObjectKind::Thread)
                .unwrap()
        );
    }

    #[test]
    fn wait_state_is_shared_and_failed_updates_keep_the_previous_signal() {
        let mut objects = RuntimeObjectTable::default();
        let key = RuntimeObjectKey::Thread { thread_id: 7 };
        let (_, first) = objects
            .register_object_and_open_handle(key.clone())
            .unwrap();
        let second = objects.open_handle(&key).unwrap();
        let signal = RuntimeWaitState::Signaled { exit_code: 23 };
        objects
            .set_wait_state(first, RuntimeObjectKind::Thread, signal)
            .unwrap();
        assert_eq!(
            objects.wait_state(second, RuntimeObjectKind::Thread),
            Ok(signal)
        );
        assert!(
            objects
                .set_wait_state(
                    first,
                    RuntimeObjectKind::Event,
                    RuntimeWaitState::Nonsignaled
                )
                .is_err()
        );
        objects.close_handle(first).unwrap();
        assert!(
            objects
                .set_wait_state(
                    first,
                    RuntimeObjectKind::Thread,
                    RuntimeWaitState::Nonsignaled
                )
                .is_err()
        );
        assert_eq!(
            objects.wait_state(second, RuntimeObjectKind::Thread),
            Ok(signal)
        );
        objects
            .set_wait_state(
                second,
                RuntimeObjectKind::Thread,
                RuntimeWaitState::Nonsignaled,
            )
            .unwrap();
        assert_eq!(
            objects.wait_state(second, RuntimeObjectKind::Thread),
            Ok(RuntimeWaitState::Nonsignaled)
        );
        let (_, file) = objects
            .register_object_and_open_handle(RuntimeObjectKey::File { file_id: 1 })
            .unwrap();
        assert!(
            objects
                .set_wait_state(file, RuntimeObjectKind::File, signal)
                .is_err()
        );
        assert!(objects.wait_state(file, RuntimeObjectKind::File).is_err());
        assert!(
            objects
                .is_live_handle(file, RuntimeObjectKind::File)
                .unwrap()
        );
    }

    #[test]
    fn events_expose_the_requested_initial_signal() {
        let mut objects = RuntimeObjectTable::default();
        for manual in [false, true] {
            for signaled in [false, true] {
                let handle = objects.register_event(manual, signaled).unwrap();
                let expected = if signaled {
                    RuntimeWaitState::Signaled { exit_code: 0 }
                } else {
                    RuntimeWaitState::Nonsignaled
                };
                assert_eq!(
                    objects.wait_state(handle, RuntimeObjectKind::Event),
                    Ok(expected)
                );
            }
        }
    }

    #[test]
    fn find_close_rejects_other_handle_kinds_and_close_handle_preserves_searches() {
        let mut objects = RuntimeObjectTable::default();
        let (_, search) = objects
            .register_object_and_open_handle(RuntimeObjectKey::FileSearch { search_id: 7 })
            .unwrap();
        let (_, file) = objects
            .register_object_and_open_handle(RuntimeObjectKey::File { file_id: 8 })
            .unwrap();
        assert!(objects.close_handle(search).is_err());
        assert!(objects.find_close(file).is_err());
        assert!(
            objects
                .is_live_handle(file, RuntimeObjectKind::File)
                .unwrap()
        );
        objects.find_close(search).unwrap();
        assert!(objects.find_close(search).is_err());
        assert!(
            !objects
                .is_live_handle(search, RuntimeObjectKind::FileSearch)
                .unwrap()
        );
        objects.close_handle(file).unwrap();
    }

    #[test]
    fn payload_close_retires_only_the_last_handle_for_that_object() {
        let mut objects = RuntimeObjectTable::default();
        let key = RuntimeObjectKey::Mutex { mutex_id: 7 };
        let (_, first) = objects
            .register_object_and_open_handle(key.clone())
            .unwrap();
        let second = objects.open_handle(&key).unwrap();
        let (_, other) = objects
            .register_object_and_open_handle(RuntimeObjectKey::Mutex { mutex_id: 8 })
            .unwrap();
        objects.close_payload_handle(first).unwrap();
        assert!(
            !objects
                .is_live_handle(first, RuntimeObjectKind::Mutex)
                .unwrap()
        );
        assert!(
            objects
                .is_live_handle(second, RuntimeObjectKind::Mutex)
                .unwrap()
        );
        objects.close_payload_handle(second).unwrap();
        assert!(objects.close_payload_handle(second).is_err());
        assert!(!objects.contains_identity(&key));
        assert!(
            objects
                .is_live_handle(other, RuntimeObjectKind::Mutex)
                .unwrap()
        );
        objects.close_payload_handle(other).unwrap();
    }
}

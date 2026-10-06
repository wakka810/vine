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

#[derive(Debug)]
#[cfg_attr(test, derive(Clone, PartialEq, Eq))]
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

#[derive(Debug)]
#[cfg_attr(test, derive(Clone, PartialEq, Eq))]
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

#[derive(Debug)]
#[cfg_attr(test, derive(Clone, PartialEq, Eq))]
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
            },
        );
        self.objects
            .get_mut(&object_id)
            .expect("validated handle object remains live")
            .open_handles = next_count;
        Ok(handle)
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

    fn next_available_handle(
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
            let slot_key = runtime_handle_slot_key(kind, slot);
            let generation = match self.slots.get(&slot_key) {
                Some(state) if !state.live && !state.retired => state.generation,
                Some(_) => continue,
                None => 1,
            };
            return RuntimeHandle::from_parts(kind, generation, slot).map(Some);
        }
        Ok(None)
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
    fn identity_lookup_rejects_wrong_record_key_kind_and_id_and_allows_repair() {
        let mut objects = RuntimeObjectTable::default();
        let key = RuntimeObjectKey::Thread { thread_id: 0x2000 };
        let (id, _) = objects
            .register_object_and_open_handle(key.clone())
            .unwrap();
        let (other_id, _) = objects
            .register_object_and_open_handle(RuntimeObjectKey::Thread { thread_id: 0x3000 })
            .unwrap();
        objects.identity_index.insert(key.clone(), other_id);
        assert!(objects.object_for_identity(&key).is_err());
        objects.identity_index.insert(key.clone(), id);
        objects.objects.get_mut(&id).unwrap().kind = RuntimeObjectKind::Event;
        assert!(objects.object_for_identity(&key).is_err());
        objects.objects.get_mut(&id).unwrap().kind = RuntimeObjectKind::Thread;
        objects.objects.get_mut(&id).unwrap().id = other_id;
        assert!(objects.object_for_identity(&key).is_err());
        objects.objects.get_mut(&id).unwrap().id = id;
        assert_eq!(objects.object_for_identity(&key).unwrap().id, id);
        assert_eq!(objects.object_for_identity(&key).unwrap().open_handles, 1);
        assert_eq!(
            objects
                .object_for_identity(&RuntimeObjectKey::Thread { thread_id: 0x3000 })
                .unwrap()
                .id,
            other_id
        );
    }

    #[test]
    fn open_handle_rejects_corrupt_key_without_consuming_handle_or_count() {
        let mut objects = RuntimeObjectTable::default();
        let key = RuntimeObjectKey::Thread { thread_id: 7 };
        let (id, first) = objects
            .register_object_and_open_handle(key.clone())
            .unwrap();
        objects.objects.get_mut(&id).unwrap().key = RuntimeObjectKey::Thread { thread_id: 8 };
        assert!(objects.open_handle(&key).is_err());
        assert_eq!(objects.objects.get(&id).unwrap().open_handles, 1);
        assert_eq!(objects.handles.len(), 1);
        assert_eq!(objects.slots.len(), 1);
        objects.objects.get_mut(&id).unwrap().key = key.clone();
        let second = objects.open_handle(&key).unwrap();
        assert_eq!(second.slot(), first.slot() + 1);
        assert_eq!(second.generation(), 1);
        assert_eq!(
            objects
                .object_for_handle(second, RuntimeObjectKind::Thread)
                .unwrap()
                .id,
            id
        );
        assert_eq!(
            objects
                .object_for_handle(first, RuntimeObjectKind::Thread)
                .unwrap()
                .open_handles,
            2
        );
    }

    #[test]
    fn close_handle_rejects_corrupt_identity_before_mutation_and_allows_retry() {
        let mut objects = RuntimeObjectTable::default();
        let key = RuntimeObjectKey::Thread { thread_id: 7 };
        let (id, handle) = objects
            .register_object_and_open_handle(key.clone())
            .unwrap();
        objects.identity_index.remove(&key);
        assert!(objects.close_handle(handle).is_err());
        assert_eq!(objects.objects.get(&id).unwrap().open_handles, 1);
        assert!(objects.handles.contains_key(&handle.0));
        let slot_key = runtime_handle_slot_key(RuntimeObjectKind::Thread, handle.slot());
        assert!(objects.slots.get(&slot_key).unwrap().live);
        assert_eq!(
            objects.slots.get(&slot_key).unwrap().generation,
            handle.generation()
        );
        objects.identity_index.insert(key, id);
        objects.close_handle(handle).unwrap();
        assert_eq!(objects.objects.get(&id).unwrap().open_handles, 0);
        assert!(!objects.handles.contains_key(&handle.0));
        assert!(!objects.slots.get(&slot_key).unwrap().live);
        assert!(objects.close_handle(handle).is_err());
    }

    #[test]
    fn close_handle_generation_exhaustion_retires_slot_without_wrapping() {
        let mut objects = RuntimeObjectTable::default();
        let key = RuntimeObjectKey::Thread { thread_id: 7 };
        let (id, first) = objects
            .register_object_and_open_handle(key.clone())
            .unwrap();
        let slot_key = runtime_handle_slot_key(RuntimeObjectKind::Thread, first.slot());
        let mut handle = first;
        for generation in 1..=u8::MAX {
            assert_eq!(handle.slot(), first.slot());
            assert_eq!(handle.generation(), generation);
            objects.close_handle(handle).unwrap();
            assert!(objects.close_handle(handle).is_err());
            if generation != u8::MAX {
                handle = objects.open_handle(&key).unwrap();
            }
        }
        let slot = objects.slots.get(&slot_key).unwrap();
        assert!(!slot.live);
        assert!(slot.retired);
        assert_eq!(slot.generation, u8::MAX);
        let next = objects.open_handle(&key).unwrap();
        assert_ne!(next.slot(), first.slot());
        assert_eq!(next.generation(), 1);
        assert!(objects.close_handle(first).is_err());
        let record = objects
            .object_for_handle(next, RuntimeObjectKind::Thread)
            .unwrap();
        assert_eq!(record.id, id);
        assert_eq!(record.open_handles, 1);
    }

    #[test]
    fn direct_wait_state_update_preserves_handle_identity_and_shares_state() {
        let mut objects = RuntimeObjectTable::default();
        let key = RuntimeObjectKey::Thread { thread_id: 0x2000 };
        let (id, handle) = objects
            .register_object_and_open_handle(key.clone())
            .unwrap();
        let second_handle = objects.open_handle(&key).unwrap();
        objects
            .set_wait_state(
                handle,
                RuntimeObjectKind::Thread,
                RuntimeWaitState::Signaled { exit_code: 7 },
            )
            .unwrap();
        for live_handle in [handle, second_handle] {
            let record = objects
                .object_for_handle(live_handle, RuntimeObjectKind::Thread)
                .unwrap();
            assert_eq!(record.id, id);
            assert_eq!(record.open_handles, 2);
            assert_eq!(
                record.wait_state,
                Some(RuntimeWaitState::Signaled { exit_code: 7 })
            );
        }
        objects.close_handle(handle).unwrap();
        assert!(
            objects
                .set_wait_state(
                    handle,
                    RuntimeObjectKind::Thread,
                    RuntimeWaitState::Nonsignaled
                )
                .is_err()
        );
        assert_eq!(
            objects
                .object_for_handle(second_handle, RuntimeObjectKind::Thread)
                .unwrap()
                .wait_state,
            Some(RuntimeWaitState::Signaled { exit_code: 7 })
        );
        objects
            .set_wait_state(
                second_handle,
                RuntimeObjectKind::Thread,
                RuntimeWaitState::Nonsignaled,
            )
            .unwrap();
        assert_eq!(
            objects
                .object_for_handle(second_handle, RuntimeObjectKind::Thread)
                .unwrap()
                .wait_state,
            Some(RuntimeWaitState::Nonsignaled)
        );
    }

    #[test]
    fn direct_wait_state_rejection_preserves_state_and_allows_repair_retry() {
        let mut objects = RuntimeObjectTable::default();
        let handle = objects.register_event(false, true).unwrap();
        let record = objects
            .object_for_handle(handle, RuntimeObjectKind::Event)
            .unwrap();
        let id = record.id;
        let key = record.key.clone();
        assert!(
            objects
                .set_wait_state(
                    handle,
                    RuntimeObjectKind::Thread,
                    RuntimeWaitState::Nonsignaled
                )
                .is_err()
        );
        objects.identity_index.remove(&key);
        assert!(
            objects
                .set_wait_state(
                    handle,
                    RuntimeObjectKind::Event,
                    RuntimeWaitState::Nonsignaled
                )
                .is_err()
        );
        assert_eq!(
            objects.objects.get(&id).unwrap().wait_state,
            Some(RuntimeWaitState::Signaled { exit_code: 0 })
        );
        objects.identity_index.insert(key, id);
        objects
            .set_wait_state(
                handle,
                RuntimeObjectKind::Event,
                RuntimeWaitState::Nonsignaled,
            )
            .unwrap();
        let record = objects
            .object_for_handle(handle, RuntimeObjectKind::Event)
            .unwrap();
        assert_eq!(record.id, id);
        assert_eq!(record.open_handles, 1);
        assert_eq!(record.wait_state, Some(RuntimeWaitState::Nonsignaled));
        let (_, file) = objects
            .register_object_and_open_handle(RuntimeObjectKey::File { file_id: 7 })
            .unwrap();
        assert!(
            objects
                .set_wait_state(file, RuntimeObjectKind::File, RuntimeWaitState::Nonsignaled)
                .is_err()
        );
        assert_eq!(
            objects
                .object_for_handle(file, RuntimeObjectKind::File)
                .unwrap()
                .wait_state,
            None
        );
    }

    #[test]
    fn wait_state_batch_rejection_leaves_live_targets_available_for_retry() {
        let mut objects = RuntimeObjectTable::default();
        let key = RuntimeObjectKey::Thread { thread_id: 0x2000 };
        let (id, handle) = objects
            .register_object_and_open_handle(key.clone())
            .unwrap();
        let file_key = RuntimeObjectKey::File { file_id: 7 };
        let (_, file) = objects
            .register_object_and_open_handle(file_key.clone())
            .unwrap();
        let state = RuntimeWaitState::Signaled { exit_code: 7 };
        for second_key in [
            key.clone(),
            RuntimeObjectKey::Thread { thread_id: 0x3000 },
            file_key,
        ] {
            assert!(
                objects
                    .plan_wait_state_updates(&[(key.clone(), state), (second_key, state)])
                    .is_err()
            );
            let record = objects
                .object_for_handle(handle, RuntimeObjectKind::Thread)
                .unwrap();
            assert_eq!(record.id, id);
            assert_eq!(record.open_handles, 1);
            assert_eq!(record.wait_state, Some(RuntimeWaitState::Nonsignaled));
            assert_eq!(
                objects
                    .object_for_handle(file, RuntimeObjectKind::File)
                    .unwrap()
                    .wait_state,
                None
            );
        }
        let updates = objects.plan_wait_state_updates(&[(key, state)]).unwrap();
        objects.commit_wait_state_updates(updates);
        assert_eq!(
            objects
                .object_for_handle(handle, RuntimeObjectKind::Thread)
                .unwrap()
                .wait_state,
            Some(state)
        );
    }

    #[test]
    fn wait_state_batch_corrupt_identity_rejects_then_recovers_after_repair() {
        for corruption in 0..3 {
            let mut objects = RuntimeObjectTable::default();
            let key = RuntimeObjectKey::Thread { thread_id: 0x2000 };
            let (id, handle) = objects
                .register_object_and_open_handle(key.clone())
                .unwrap();
            let record = objects.objects.get_mut(&id).unwrap();
            match corruption {
                0 => record.id = RuntimeObjectId(id.0 + 1),
                1 => record.key = RuntimeObjectKey::Thread { thread_id: 0x3000 },
                2 => record.kind = RuntimeObjectKind::Event,
                _ => unreachable!(),
            }
            let state = RuntimeWaitState::Signaled { exit_code: 7 };
            assert!(
                objects
                    .plan_wait_state_updates(&[(key.clone(), state)])
                    .is_err()
            );
            let record = objects.objects.get_mut(&id).unwrap();
            assert_eq!(record.wait_state, Some(RuntimeWaitState::Nonsignaled));
            assert_eq!(record.open_handles, 1);
            record.id = id;
            record.key = key.clone();
            record.kind = RuntimeObjectKind::Thread;
            let updates = objects.plan_wait_state_updates(&[(key, state)]).unwrap();
            objects.commit_wait_state_updates(updates);
            let record = objects
                .object_for_handle(handle, RuntimeObjectKind::Thread)
                .unwrap();
            assert_eq!(record.id, id);
            assert_eq!(record.wait_state, Some(state));
        }
    }

    #[test]
    fn live_handle_lookup_distinguishes_absence_from_corrupt_authority() {
        let mut objects = RuntimeObjectTable::default();
        let key = RuntimeObjectKey::File { file_id: 7 };
        let (id, handle) = objects
            .register_object_and_open_handle(key.clone())
            .unwrap();
        assert!(
            objects
                .is_live_handle(handle, RuntimeObjectKind::File)
                .unwrap()
        );
        assert!(
            !objects
                .is_live_handle(handle, RuntimeObjectKind::Thread)
                .unwrap()
        );
        assert!(
            !objects
                .is_live_handle(RuntimeHandle(0), RuntimeObjectKind::File)
                .unwrap()
        );
        for damage in 0..8 {
            let mut corrupt = objects.clone();
            match damage {
                0 => {
                    corrupt.slots.remove(&runtime_handle_slot_key(
                        RuntimeObjectKind::File,
                        handle.slot(),
                    ));
                }
                1 => {
                    corrupt.handles.remove(&handle.0);
                }
                2 => {
                    corrupt.objects.remove(&id);
                }
                3 => {
                    corrupt.identity_index.remove(&key);
                }
                4 => {
                    corrupt.objects.get_mut(&id).unwrap().kind = RuntimeObjectKind::Thread;
                }
                5 => {
                    corrupt.objects.get_mut(&id).unwrap().id = RuntimeObjectId(999);
                }
                6 => {
                    corrupt.handles.get_mut(&handle.0).unwrap().handle = RuntimeHandle(0);
                }
                _ => {
                    corrupt.objects.get_mut(&id).unwrap().open_handles = 0;
                }
            }
            let before = corrupt.clone();
            assert!(
                corrupt
                    .is_live_handle(handle, RuntimeObjectKind::File)
                    .is_err()
            );
            assert_eq!(corrupt, before);
        }
        objects.close_handle(handle).unwrap();
        let before = objects.clone();
        assert!(
            !objects
                .is_live_handle(handle, RuntimeObjectKind::File)
                .unwrap()
        );
        assert_eq!(objects, before);
    }

    #[test]
    fn exact_handle_registration_rejects_collisions_before_mutation() {
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

        objects
            .register_object_with_handle(RuntimeObjectKey::Thread { thread_id: 0x2000 }, handle)
            .expect_err("dangling handle collision must fail");
        assert_eq!(objects, before, "failed registration must be atomic");
    }

    #[test]
    fn optional_registration_reports_capacity_without_publishing_and_preserves_corrupt_authority() {
        let key = RuntimeObjectKey::Thread { thread_id: 0x2000 };
        let mut objects = RuntimeObjectTable {
            next_object_id: u64::MAX,
            ..Default::default()
        };
        let exhausted = objects.clone();
        assert!(
            objects
                .try_register_object_and_open_handle(key.clone())
                .unwrap()
                .is_none()
        );
        assert_eq!(objects, exhausted);
        objects.next_object_id = 1;
        let created = objects
            .try_register_object_and_open_handle(key.clone())
            .unwrap()
            .unwrap();
        assert_eq!(created.0, RuntimeObjectId(1));
        assert_eq!(
            created.1,
            RuntimeHandle::from_parts(RuntimeObjectKind::Thread, 1, 1).unwrap()
        );
        let before_duplicate = objects.clone();
        assert!(
            objects
                .try_register_object_and_open_handle(key.clone())
                .is_err()
        );
        assert_eq!(objects, before_duplicate);
        let mut occupied_identity = RuntimeObjectTable::default();
        occupied_identity.objects.insert(
            RuntimeObjectId(1),
            runtime_object_record(
                RuntimeObjectId(1),
                RuntimeObjectKey::Process { process_id: 0x1000 },
                0,
            ),
        );
        let before = occupied_identity.clone();
        assert!(
            occupied_identity
                .try_register_object_and_open_handle(key.clone())
                .is_err()
        );
        assert_eq!(occupied_identity, before);
        let mut handles_exhausted = RuntimeObjectTable::default();
        for slot in 1..=u16::MAX {
            handles_exhausted.slots.insert(
                runtime_handle_slot_key(RuntimeObjectKind::Thread, slot),
                RuntimeHandleSlot {
                    generation: u8::MAX,
                    live: false,
                    retired: true,
                },
            );
        }
        let before = handles_exhausted.clone();
        assert!(
            handles_exhausted
                .try_register_object_and_open_handle(key)
                .unwrap()
                .is_none()
        );
        assert_eq!(handles_exhausted, before);
    }

    #[test]
    fn direct_registration_rejects_exhaustion_and_corrupt_identity_without_mutation() {
        for exact_handle in [false, true] {
            let key = RuntimeObjectKey::Thread { thread_id: 0x2000 };
            let handle = RuntimeHandle::from_parts(RuntimeObjectKind::Thread, 1, 1).unwrap();
            let mut objects = RuntimeObjectTable {
                next_object_id: u64::MAX,
                ..Default::default()
            };
            let before = objects.clone();
            let result = if exact_handle {
                objects
                    .register_object_with_handle(key.clone(), handle)
                    .map(|id| (id, handle))
            } else {
                objects.register_object_and_open_handle(key.clone())
            };
            assert!(result.is_err());
            assert_eq!(objects, before);

            objects.next_object_id = 1;
            objects.objects.insert(
                RuntimeObjectId(1),
                runtime_object_record(
                    RuntimeObjectId(1),
                    RuntimeObjectKey::Process { process_id: 0x1000 },
                    0,
                ),
            );
            let before = objects.clone();
            let result = if exact_handle {
                objects
                    .register_object_with_handle(key.clone(), handle)
                    .map(|id| (id, handle))
            } else {
                objects.register_object_and_open_handle(key.clone())
            };
            assert!(result.is_err());
            assert_eq!(objects, before);
        }
    }

    #[test]
    fn direct_event_creation_publishes_wait_state_and_rejects_exhaustion_atomically() {
        for manual_reset in [false, true] {
            for initial_state in [false, true] {
                let mut objects = RuntimeObjectTable::default();
                let handle = objects.register_event(manual_reset, initial_state).unwrap();
                let expected = if initial_state {
                    RuntimeWaitState::Signaled { exit_code: 0 }
                } else {
                    RuntimeWaitState::Nonsignaled
                };
                let record = objects
                    .object_for_handle(handle, RuntimeObjectKind::Event)
                    .unwrap();
                assert_eq!(
                    record.key,
                    RuntimeObjectKey::Event {
                        event_id: 1,
                        manual_reset
                    }
                );
                assert_eq!(record.wait_state, Some(expected));
                assert_eq!(record.open_handles, 1);
                objects.next_object_id = u64::MAX;
                let before = objects.clone();
                assert!(objects.register_event(manual_reset, initial_state).is_err());
                assert_eq!(objects, before);
            }
        }
    }

    #[test]
    fn pending_registration_revalidates_identity_after_intervening_object_creation() {
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
        objects
            .preflight_object_registration_commit(&stale)
            .expect_err("intervening object identity must stale the plan");
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
    fn direct_handle_open_failures_preserve_all_tables_and_allocators() {
        let mut objects = RuntimeObjectTable::default();
        let key = RuntimeObjectKey::Mutex { mutex_id: 7 };
        let before_unknown = objects.clone();
        assert!(objects.open_handle(&key).is_err());
        assert_eq!(objects, before_unknown);

        let (id, first_handle) = objects
            .register_object_and_open_handle(key.clone())
            .unwrap();
        let before_duplicate = objects.clone();
        assert!(
            objects
                .register_object_and_open_handle(key.clone())
                .is_err()
        );
        assert_eq!(objects, before_duplicate);
        objects.objects.get_mut(&id).unwrap().open_handles = usize::MAX;
        let before_overflow = objects.clone();
        assert!(objects.open_handle(&key).is_err());
        assert_eq!(objects, before_overflow);
        objects.objects.get_mut(&id).unwrap().open_handles = 1;
        let second_handle = objects.open_handle(&key).unwrap();
        assert_ne!(first_handle, second_handle);
        assert_eq!(
            objects
                .object_for_handle(first_handle, RuntimeObjectKind::Mutex)
                .unwrap()
                .open_handles,
            2
        );
        assert_eq!(
            objects
                .object_for_handle(second_handle, RuntimeObjectKind::Mutex)
                .unwrap()
                .id,
            id
        );

        objects.next_object_id = u64::MAX;
        let before_exhaustion = objects.clone();
        assert!(
            objects
                .register_object_and_open_handle(RuntimeObjectKey::File { file_id: 1 })
                .is_err()
        );
        assert_eq!(objects, before_exhaustion);
        // Adding a handle to an existing object does not allocate another object identity.
        assert!(objects.open_handle(&key).is_ok());
    }

    #[test]
    fn find_close_rejects_bad_index_and_shared_handles_then_recovers() {
        let mut objects = RuntimeObjectTable::default();
        let key = RuntimeObjectKey::FileSearch { search_id: 7 };
        let other_key = RuntimeObjectKey::FileSearch { search_id: 8 };
        let (id, handle) = objects
            .register_object_and_open_handle(key.clone())
            .unwrap();
        let (other_id, other) = objects
            .register_object_and_open_handle(other_key.clone())
            .unwrap();
        objects.identity_index.insert(key.clone(), other_id);
        assert!(objects.find_close(handle).is_err());
        assert_eq!(objects.objects.get(&id).unwrap().open_handles, 1);
        assert!(objects.handles.contains_key(&handle.0));
        objects.identity_index.insert(key.clone(), id);
        objects.find_close(handle).unwrap();
        assert!(
            objects
                .lookup_entry(handle, RuntimeObjectKind::FileSearch)
                .is_err()
        );
        assert!(!objects.contains_identity(&key));
        assert!(objects.find_close(handle).is_err());
        assert_eq!(
            objects
                .object_for_handle(other, RuntimeObjectKind::FileSearch)
                .unwrap()
                .id,
            other_id
        );
        let duplicate = objects.open_handle(&other_key).unwrap();
        assert!(objects.find_close(other).is_err());
        assert_eq!(
            objects
                .object_for_handle(other, RuntimeObjectKind::FileSearch)
                .unwrap()
                .open_handles,
            2
        );
        assert!(objects.handles.contains_key(&duplicate.0));
    }

    #[test]
    fn find_close_count_and_policy_rejection_preserves_handle_for_retry() {
        let mut objects = RuntimeObjectTable::default();
        let key = RuntimeObjectKey::FileSearch { search_id: 7 };
        let (id, handle) = objects
            .register_object_and_open_handle(key.clone())
            .unwrap();
        for policy in [
            RuntimeHandleClosePolicy::CloseHandle,
            RuntimeHandleClosePolicy::SubsystemOwned,
        ] {
            objects.objects.get_mut(&id).unwrap().close_policy = policy;
            assert!(objects.find_close(handle).is_err());
            assert!(objects.handles.contains_key(&handle.0));
            assert!(objects.contains_identity(&key));
        }
        let record = objects.objects.get_mut(&id).unwrap();
        record.close_policy = RuntimeHandleClosePolicy::FindClose;
        record.open_handles = 2;
        assert!(objects.find_close(handle).is_err());
        assert!(objects.handles.contains_key(&handle.0));
        objects.objects.get_mut(&id).unwrap().open_handles = 1;
        objects.find_close(handle).unwrap();
        assert!(!objects.contains_identity(&key));
        assert!(!objects.handles.contains_key(&handle.0));
        let (_, next_handle) = objects
            .register_object_and_open_handle(RuntimeObjectKey::FileSearch { search_id: 8 })
            .unwrap();
        assert_ne!(next_handle, handle);
        assert!(objects.find_close(handle).is_err());
        objects.find_close(next_handle).unwrap();
    }

    #[test]
    fn payload_close_derives_retirement_from_live_handles() {
        let mut objects = RuntimeObjectTable::default();
        let key = RuntimeObjectKey::Mutex { mutex_id: 7 };
        let (id, first) = objects
            .register_object_and_open_handle(key.clone())
            .unwrap();
        let duplicate = objects.open_handle(&key).unwrap();
        let other_key = RuntimeObjectKey::Mutex { mutex_id: 8 };
        let (other_id, other) = objects
            .register_object_and_open_handle(other_key.clone())
            .unwrap();
        objects.close_payload_handle(first).unwrap();
        assert!(
            objects
                .lookup_entry(first, RuntimeObjectKind::Mutex)
                .is_err()
        );
        assert_eq!(
            objects
                .object_for_handle(duplicate, RuntimeObjectKind::Mutex)
                .unwrap()
                .id,
            id
        );
        assert_eq!(
            objects
                .object_for_handle(duplicate, RuntimeObjectKind::Mutex)
                .unwrap()
                .open_handles,
            1
        );
        objects.close_payload_handle(duplicate).unwrap();
        assert!(!objects.contains_identity(&key));
        assert!(objects.close_payload_handle(first).is_err());
        assert!(objects.close_payload_handle(duplicate).is_err());
        assert_eq!(
            objects
                .object_for_handle(other, RuntimeObjectKind::Mutex)
                .unwrap()
                .id,
            other_id
        );
        objects.identity_index.remove(&other_key);
        assert!(objects.close_payload_handle(other).is_err());
        assert_eq!(objects.objects.get(&other_id).unwrap().open_handles, 1);
        objects.identity_index.insert(other_key.clone(), other_id);
        objects.close_payload_handle(other).unwrap();
        assert!(!objects.contains_identity(&other_key));
    }

    #[test]
    fn payload_close_rejects_corrupt_handle_count_before_mutation() {
        let mut objects = RuntimeObjectTable::default();
        let key = RuntimeObjectKey::Mutex { mutex_id: 7 };
        let (id, first) = objects
            .register_object_and_open_handle(key.clone())
            .unwrap();
        let duplicate = objects.open_handle(&key).unwrap();
        objects.objects.get_mut(&id).unwrap().open_handles = 1;
        assert!(objects.close_payload_handle(first).is_err());
        assert!(objects.handles.contains_key(&first.0));
        assert!(objects.handles.contains_key(&duplicate.0));
        assert!(objects.contains_identity(&key));
        objects.objects.get_mut(&id).unwrap().open_handles = 2;
        objects.close_payload_handle(first).unwrap();
        objects.close_payload_handle(duplicate).unwrap();
        assert!(!objects.contains_identity(&key));
    }
}

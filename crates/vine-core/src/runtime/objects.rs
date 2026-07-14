use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use super::{
    LOGICAL_LOADED_MODULE_NAMESPACE, LOGICAL_PRIVATE_HEAP_NAMESPACE,
    RUNTIME_MUTEX_OBJECT_NAMESPACE, RUNTIME_PROCESS_OBJECT_NAMESPACE,
    RUNTIME_THREAD_OBJECT_NAMESPACE, RuntimeHandle, RuntimeHandleClosePolicy, RuntimeHandleEntry,
    RuntimeObjectId, RuntimeObjectKey, RuntimeObjectKind, RuntimeObjectRecord, RuntimeStateError,
    RuntimeWaitState,
};
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
        let kind = key.kind();
        let (namespace, close_policy, wait_state) = match kind {
            RuntimeObjectKind::Process | RuntimeObjectKind::Thread | RuntimeObjectKind::Mutex => (
                match kind {
                    RuntimeObjectKind::Process => RUNTIME_PROCESS_OBJECT_NAMESPACE,
                    RuntimeObjectKind::Thread => RUNTIME_THREAD_OBJECT_NAMESPACE,
                    RuntimeObjectKind::Mutex => RUNTIME_MUTEX_OBJECT_NAMESPACE,
                    RuntimeObjectKind::PrivateHeap | RuntimeObjectKind::Module => unreachable!(),
                },
                RuntimeHandleClosePolicy::CloseHandle,
                (kind != RuntimeObjectKind::Mutex).then_some(RuntimeWaitState::Nonsignaled),
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

    pub(super) fn remove_closed_mutex_object(
        &mut self,
        key: &RuntimeObjectKey,
    ) -> Result<RuntimeObjectRecord, RuntimeStateError> {
        if key.kind() != RuntimeObjectKind::Mutex {
            return Err(RuntimeStateError::invalid_input(
                "runtime-object",
                "runtime-mutex-retirement-kind-invalid",
                "closed-object retirement is restricted to mutex objects",
            ));
        }
        let id = self.identity_index.get(key).copied().ok_or_else(|| {
            RuntimeStateError::not_found(
                "runtime-object",
                "runtime-object-identity-unknown",
                "mutex object identity is not registered",
            )
        })?;
        let record = self.objects.get(&id).cloned().ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-object",
                "runtime-object-index-dangling",
                "mutex object identity index references a missing object",
            )
        })?;
        if record.open_handles != 0
            || record.close_policy != RuntimeHandleClosePolicy::CloseHandle
            || self.handles.values().any(|entry| entry.object_id == id)
        {
            return Err(RuntimeStateError::conflict(
                "runtime-object",
                "runtime-mutex-retirement-handles-live",
                "mutex object cannot retire while a handle remains open",
            ));
        }
        self.identity_index.remove(key);
        self.objects.remove(&id).ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-object",
                "runtime-object-retirement-missing",
                "preflighted mutex object disappeared during retirement",
            )
        })
    }

    pub(super) fn retire_mutex_objects(
        &mut self,
        keys: &[RuntimeObjectKey],
    ) -> Result<usize, RuntimeStateError> {
        let mut retired_handles = 0;
        for key in keys {
            if key.kind() != RuntimeObjectKind::Mutex {
                return Err(RuntimeStateError::invalid_input(
                    "runtime-object",
                    "runtime-mutex-retirement-kind-invalid",
                    "process mutex teardown received a non-mutex object identity",
                ));
            }
            let id = self.identity_index.get(key).copied().ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-object",
                    "runtime-object-identity-unknown",
                    "process mutex teardown targets an unregistered object",
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
            self.remove_closed_mutex_object(key)?;
        }
        Ok(retired_handles)
    }

    pub(super) fn retire_subsystem_objects(
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

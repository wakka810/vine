use std::collections::{BTreeMap, BTreeSet};

use crate::memory::{GuestAddress, MemoryOwnerId};

use super::{ProcessAddressAllocationId, RuntimeAddressRange, RuntimeStateError};

pub const RUNTIME_COM_IUNKNOWN_IID: RuntimeComIid = RuntimeComIid([
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xc0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46,
]);
pub const RUNTIME_COM_MAX_INTERFACES_PER_OBJECT: usize = 32;
pub const RUNTIME_COM_MAX_METHODS_PER_INTERFACE: usize = 256;
const RUNTIME_COM_MAX_NAME_BYTES: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct RuntimeComIid(pub [u8; 16]);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeComClass {
    D3d9,
    D3d9Device,
    D3d9Texture,
    D3d9Surface,
    D3d9VertexBuffer,
    DirectInput8A,
    DirectSound8,
    DirectSoundBuffer8,
    DirectShowGraph,
    DirectShowAllocator,
    DirectShowSample,
    DirectShowOutputPin,
    DirectShowStream,
}

impl RuntimeComIid {
    pub const fn from_windows_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    #[cfg(test)]
    pub const fn windows_bytes(self) -> [u8; 16] {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct RuntimeComObjectId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct RuntimeComInterfaceId(pub u64);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeComBacking {
    pub address_allocation_id: ProcessAddressAllocationId,
    pub memory_owner: MemoryOwnerId,
    pub range: RuntimeAddressRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeComMethodRecord {
    pub slot: u16,
    pub name: String,
    pub endpoint: GuestAddress,
    pub explicit_argument_words: Option<u16>,
}

impl RuntimeComMethodRecord {
    pub(crate) fn argument_words(&self) -> Option<u32> {
        self.explicit_argument_words
            .map(|words| u32::from(words) + 1)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeComInterfaceRecord {
    pub id: RuntimeComInterfaceId,
    pub object_id: RuntimeComObjectId,
    pub iid: RuntimeComIid,
    pub name: String,
    pub guest_pointer: GuestAddress,
    pub vtable: GuestAddress,
    pub methods: BTreeMap<u16, RuntimeComMethodRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeComObjectRecord {
    pub id: RuntimeComObjectId,
    pub class: RuntimeComClass,
    pub reference_count: u32,
    pub parent_interface_id: Option<RuntimeComInterfaceId>,
    pub canonical_iunknown_interface_id: RuntimeComInterfaceId,
    pub interface_ids: Vec<RuntimeComInterfaceId>,
    pub backing: RuntimeComBacking,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeComMethodRegistration {
    pub slot: u16,
    pub name: String,
    pub endpoint: GuestAddress,
    pub explicit_argument_words: Option<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeComInterfaceRegistration {
    pub iid: RuntimeComIid,
    pub name: String,
    pub guest_pointer: GuestAddress,
    pub vtable: GuestAddress,
    pub methods: Vec<RuntimeComMethodRegistration>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeComObjectRegistration {
    pub class: RuntimeComClass,
    pub initial_reference_count: u32,
    pub parent_interface_id: Option<RuntimeComInterfaceId>,
    pub backing: RuntimeComBacking,
    pub interfaces: Vec<RuntimeComInterfaceRegistration>,
}

#[derive(Debug, Clone)]
pub struct RuntimeComMethodTarget {
    pub object_id: RuntimeComObjectId,
    pub class: RuntimeComClass,
    pub(crate) interface_id: RuntimeComInterfaceId,
    pub(crate) iid: RuntimeComIid,
    pub interface_name: String,
    pub(crate) guest_pointer: GuestAddress,
    pub method: RuntimeComMethodRecord,
}

#[derive(Debug)]
pub(crate) struct RuntimeComRelease {
    reference_updates: Vec<(RuntimeComObjectId, u32)>,
    retired_objects: Vec<RuntimeComObjectId>,
    reference_count: u32,
}

impl RuntimeComRelease {
    pub(crate) fn reference_count(&self) -> u32 {
        self.reference_count
    }

    pub(crate) fn retired_objects(&self) -> &[RuntimeComObjectId] {
        &self.retired_objects
    }
}

#[derive(Debug)]
pub struct RuntimeCom {
    objects: BTreeMap<RuntimeComObjectId, RuntimeComObjectRecord>,
    interfaces: BTreeMap<RuntimeComInterfaceId, RuntimeComInterfaceRecord>,
    pointer_index: BTreeMap<GuestAddress, RuntimeComInterfaceId>,
    endpoint_index: BTreeMap<GuestAddress, (RuntimeComInterfaceId, u16)>,
    iid_index: BTreeMap<(RuntimeComObjectId, RuntimeComIid), RuntimeComInterfaceId>,
    next_object_id: u64,
    next_interface_id: u64,
}

impl Default for RuntimeCom {
    fn default() -> Self {
        Self {
            objects: BTreeMap::new(),
            interfaces: BTreeMap::new(),
            pointer_index: BTreeMap::new(),
            endpoint_index: BTreeMap::new(),
            iid_index: BTreeMap::new(),
            next_object_id: 1,
            next_interface_id: 1,
        }
    }
}

impl RuntimeCom {
    #[cfg(test)]
    pub(crate) fn is_empty(&self) -> bool {
        self.objects.is_empty()
    }

    #[cfg(test)]
    pub(crate) fn objects(&self) -> &BTreeMap<RuntimeComObjectId, RuntimeComObjectRecord> {
        &self.objects
    }

    #[cfg(test)]
    pub(crate) fn interfaces(&self) -> &BTreeMap<RuntimeComInterfaceId, RuntimeComInterfaceRecord> {
        &self.interfaces
    }

    pub fn object(
        &self,
        id: RuntimeComObjectId,
    ) -> Result<&RuntimeComObjectRecord, RuntimeStateError> {
        self.objects.get(&id).ok_or_else(|| {
            RuntimeStateError::new(format!("logical COM object ID {} is unknown", id.0))
        })
    }

    pub fn interface(
        &self,
        id: RuntimeComInterfaceId,
    ) -> Result<&RuntimeComInterfaceRecord, RuntimeStateError> {
        self.interfaces.get(&id).ok_or_else(|| {
            RuntimeStateError::new(format!("logical COM interface ID {} is unknown", id.0))
        })
    }

    pub fn interface_for_pointer(
        &self,
        pointer: GuestAddress,
    ) -> Result<Option<&RuntimeComInterfaceRecord>, RuntimeStateError> {
        let Some(id) = self.pointer_index.get(&pointer) else {
            return Ok(None);
        };
        let interface = self.interfaces.get(id).ok_or_else(|| {
            RuntimeStateError::new(
                "guest COM interface pointer index refers to a retired interface",
            )
        })?;
        if interface.guest_pointer != pointer {
            return Err(RuntimeStateError::new(
                "guest COM interface pointer index disagrees with the interface identity",
            ));
        }
        let object = self.objects.get(&interface.object_id).ok_or_else(|| {
            RuntimeStateError::new("live COM interface pointer refers to a retired object")
        })?;
        if object.reference_count == 0 || !object.interface_ids.contains(id) {
            return Err(RuntimeStateError::new(
                "live COM interface pointer does not belong to a referenced object",
            ));
        }
        Ok(Some(interface))
    }

    pub fn interface_for_object_iid(
        &self,
        object_id: RuntimeComObjectId,
        iid: RuntimeComIid,
    ) -> Result<&RuntimeComInterfaceRecord, RuntimeStateError> {
        self.object(object_id)?;
        let id = self
            .iid_index
            .get(&(object_id, iid))
            .copied()
            .ok_or_else(|| {
                RuntimeStateError::new(format!(
                    "logical COM object ID {} does not expose IID {:?}",
                    object_id.0, iid
                ))
            })?;
        self.interface(id)
    }

    pub fn resolve_method(
        &self,
        endpoint: GuestAddress,
    ) -> Result<Option<RuntimeComMethodTarget>, RuntimeStateError> {
        let Some((interface_id, slot)) = self.endpoint_index.get(&endpoint).copied() else {
            return Ok(None);
        };
        let interface = self.interface(interface_id)?;
        let object = self.object(interface.object_id)?;
        let method = interface.methods.get(&slot).ok_or_else(|| {
            RuntimeStateError::new("COM endpoint index references a missing vtable method")
        })?;
        if method.slot != slot
            || method.endpoint != endpoint
            || self
                .interface_for_pointer(interface.guest_pointer)?
                .map(|live| live.id)
                != Some(interface_id)
            || self.iid_index.get(&(object.id, interface.iid)) != Some(&interface_id)
        {
            return Err(RuntimeStateError::new(
                "COM endpoint does not match its live interface, method, or reverse index",
            ));
        }
        Ok(Some(RuntimeComMethodTarget {
            object_id: object.id,
            class: object.class,
            interface_id,
            iid: interface.iid,
            interface_name: interface.name.clone(),
            guest_pointer: interface.guest_pointer,
            method: method.clone(),
        }))
    }

    pub(crate) fn register_with<T, F>(
        &mut self,
        registration: RuntimeComObjectRegistration,
        write_guest: F,
    ) -> Result<T, RuntimeStateError>
    where
        F: FnOnce(
            &RuntimeComObjectRecord,
            &[RuntimeComInterfaceRecord],
        ) -> Result<T, RuntimeStateError>,
    {
        let parent_reference_update = registration
            .parent_interface_id
            .map(|parent_interface_id| {
                let parent_object_id = self.interface(parent_interface_id)?.object_id;
                self.checked_reference_increment(parent_object_id)
            })
            .transpose()?;
        let (object, interfaces, next_object_id, next_interface_id) =
            self.build_registration(registration)?;
        let result = write_guest(&object, &interfaces)?;
        // The closure only performs synchronous work in other owners. No COM
        // state changes before it succeeds, and publication cannot fail.
        if let Some(update) = parent_reference_update {
            self.commit_reference_update(update);
        }
        for interface in interfaces {
            for method in interface.methods.values() {
                let replaced = self
                    .endpoint_index
                    .insert(method.endpoint, (interface.id, method.slot));
                debug_assert!(replaced.is_none());
            }
            let replaced = self
                .pointer_index
                .insert(interface.guest_pointer, interface.id);
            debug_assert!(replaced.is_none());
            let replaced = self
                .iid_index
                .insert((object.id, interface.iid), interface.id);
            debug_assert!(replaced.is_none());
            let replaced = self.interfaces.insert(interface.id, interface);
            debug_assert!(replaced.is_none());
        }
        let replaced = self.objects.insert(object.id, object);
        debug_assert!(replaced.is_none());
        self.next_object_id = next_object_id;
        self.next_interface_id = next_interface_id;
        Ok(result)
    }

    pub(crate) fn query_interface_target(
        &self,
        source_interface_id: RuntimeComInterfaceId,
        iid: RuntimeComIid,
    ) -> Result<Option<&RuntimeComInterfaceRecord>, RuntimeStateError> {
        let source = self.interface(source_interface_id)?;
        let object = self.object(source.object_id)?;
        self.iid_index
            .get(&(object.id, iid))
            .map(|id| self.interface(*id))
            .transpose()
    }

    pub(crate) fn add_ref(
        &mut self,
        interface_id: RuntimeComInterfaceId,
    ) -> Result<u32, RuntimeStateError> {
        let object_id = self.interface(interface_id)?.object_id;
        let reference_count = self
            .object(object_id)?
            .reference_count
            .checked_add(1)
            .ok_or_else(|| {
                RuntimeStateError::new("COM reference count overflowed ULONG accounting")
            })?;
        self.objects
            .get_mut(&object_id)
            .expect("validated live COM object")
            .reference_count = reference_count;
        Ok(reference_count)
    }

    pub(crate) fn release_shared(
        &mut self,
        interface_id: RuntimeComInterfaceId,
    ) -> Result<Option<u32>, RuntimeStateError> {
        let object_id = self.interface(interface_id)?.object_id;
        let count = self.object(object_id)?.reference_count;
        if count == 0 {
            return Err(RuntimeStateError::new(
                "live COM object has zero references",
            ));
        }
        if count == 1 {
            return Ok(None);
        }
        let next = count - 1;
        self.objects
            .get_mut(&object_id)
            .expect("validated live COM object")
            .reference_count = next;
        Ok(Some(next))
    }

    pub(crate) fn validate_release(
        &self,
        interface_id: RuntimeComInterfaceId,
    ) -> Result<RuntimeComRelease, RuntimeStateError> {
        let source_object_id = self.interface(interface_id)?.object_id;
        let mut object_id = source_object_id;
        let mut reference_updates = Vec::new();
        let mut retired_objects = Vec::new();
        let mut source_reference_count = 0;
        loop {
            let object = self.object(object_id)?;
            let reference_count = object
                .reference_count
                .checked_sub(1)
                .ok_or_else(|| RuntimeStateError::new("live COM object has zero references"))?;
            if object_id == source_object_id {
                source_reference_count = reference_count;
            }
            if reference_count != 0 {
                reference_updates.push((object_id, reference_count));
                break;
            }
            self.validate_retirement(object)?;
            retired_objects.push(object_id);
            let Some(parent_interface_id) = object.parent_interface_id else {
                break;
            };
            let parent_object_id = self.interface(parent_interface_id)?.object_id;
            // Parents are existing, older objects at registration. This also
            // bounds traversal and rejects corrupted parent cycles.
            if parent_object_id >= object_id {
                return Err(RuntimeStateError::new(
                    "COM parent reference does not target an older live object",
                ));
            }
            object_id = parent_object_id;
        }
        Ok(RuntimeComRelease {
            reference_updates,
            retired_objects,
            reference_count: source_reference_count,
        })
    }

    pub(crate) fn remove_validated_objects(&mut self, release: RuntimeComRelease) {
        for update in release.reference_updates {
            self.commit_reference_update(update);
        }
        for object_id in release.retired_objects {
            let object = self
                .objects
                .remove(&object_id)
                .expect("validated live COM object");
            for interface_id in object.interface_ids {
                let interface = self
                    .interfaces
                    .remove(&interface_id)
                    .expect("validated live COM interface");
                self.pointer_index.remove(&interface.guest_pointer);
                self.iid_index.remove(&(object_id, interface.iid));
                for method in interface.methods.values() {
                    self.endpoint_index.remove(&method.endpoint);
                }
            }
        }
    }

    fn build_registration(
        &self,
        registration: RuntimeComObjectRegistration,
    ) -> Result<
        (
            RuntimeComObjectRecord,
            Vec<RuntimeComInterfaceRecord>,
            u64,
            u64,
        ),
        RuntimeStateError,
    > {
        validate_registration(self, &registration)?;
        let object_id = RuntimeComObjectId(self.next_object_id);
        let next_object_id = self
            .next_object_id
            .checked_add(1)
            .ok_or_else(|| RuntimeStateError::new("logical COM object ID space is exhausted"))?;
        let interface_count = u64::try_from(registration.interfaces.len()).map_err(|_| {
            RuntimeStateError::new("COM interface count does not fit logical ID accounting")
        })?;
        let next_interface_id = self
            .next_interface_id
            .checked_add(interface_count)
            .ok_or_else(|| RuntimeStateError::new("logical COM interface ID space is exhausted"))?;

        let mut interface_ids = Vec::with_capacity(registration.interfaces.len());
        let mut interfaces = Vec::with_capacity(registration.interfaces.len());
        let mut canonical_iunknown_interface_id = None;
        for (index, interface) in registration.interfaces.into_iter().enumerate() {
            let index = u64::try_from(index).expect("bounded COM interface index fits u64");
            let interface_id = RuntimeComInterfaceId(self.next_interface_id + index);
            let methods = interface
                .methods
                .into_iter()
                .map(|method| {
                    (
                        method.slot,
                        RuntimeComMethodRecord {
                            slot: method.slot,
                            name: method.name,
                            endpoint: method.endpoint,
                            explicit_argument_words: method.explicit_argument_words,
                        },
                    )
                })
                .collect::<BTreeMap<_, _>>();
            let record = RuntimeComInterfaceRecord {
                id: interface_id,
                object_id,
                iid: interface.iid,
                name: interface.name,
                guest_pointer: interface.guest_pointer,
                vtable: interface.vtable,
                methods,
            };
            if record.iid == RUNTIME_COM_IUNKNOWN_IID {
                canonical_iunknown_interface_id = Some(interface_id);
            }
            interface_ids.push(interface_id);
            interfaces.push(record);
        }
        let canonical_iunknown_interface_id = canonical_iunknown_interface_id.ok_or_else(|| {
            RuntimeStateError::new(
                "validated COM registration lost its canonical IUnknown interface",
            )
        })?;
        let object = RuntimeComObjectRecord {
            id: object_id,
            class: registration.class,
            reference_count: registration.initial_reference_count,
            parent_interface_id: registration.parent_interface_id,
            canonical_iunknown_interface_id,
            interface_ids,
            backing: registration.backing,
        };
        Ok((object, interfaces, next_object_id, next_interface_id))
    }

    fn checked_reference_increment(
        &self,
        object_id: RuntimeComObjectId,
    ) -> Result<(RuntimeComObjectId, u32), RuntimeStateError> {
        let object = self.object(object_id)?;
        let next = object.reference_count.checked_add(1).ok_or_else(|| {
            RuntimeStateError::new("COM reference count overflowed ULONG accounting")
        })?;
        Ok((object_id, next))
    }

    fn commit_reference_update(&mut self, (object_id, count): (RuntimeComObjectId, u32)) {
        self.objects
            .get_mut(&object_id)
            .expect("validated live COM reference target")
            .reference_count = count;
    }

    fn validate_retirement(
        &self,
        object: &RuntimeComObjectRecord,
    ) -> Result<(), RuntimeStateError> {
        let mut interface_ids = BTreeSet::new();
        for id in &object.interface_ids {
            let interface = self.interface(*id)?;
            if !interface_ids.insert(*id)
                || interface.id != *id
                || interface.object_id != object.id
                || self.pointer_index.get(&interface.guest_pointer) != Some(id)
                || self.iid_index.get(&(object.id, interface.iid)) != Some(id)
            {
                return Err(RuntimeStateError::new(
                    "retiring COM object has a corrupt interface relation or reverse index",
                ));
            }
            for (slot, method) in &interface.methods {
                if method.slot != *slot
                    || self.endpoint_index.get(&method.endpoint) != Some(&(*id, *slot))
                {
                    return Err(RuntimeStateError::new(
                        "retiring COM interface has a corrupt method or endpoint index",
                    ));
                }
            }
        }
        if !interface_ids.contains(&object.canonical_iunknown_interface_id)
            || self.interface(object.canonical_iunknown_interface_id)?.iid
                != RUNTIME_COM_IUNKNOWN_IID
        {
            return Err(RuntimeStateError::new(
                "retiring COM object has lost its canonical IUnknown identity",
            ));
        }
        Ok(())
    }
}

fn validate_registration(
    runtime: &RuntimeCom,
    registration: &RuntimeComObjectRegistration,
) -> Result<(), RuntimeStateError> {
    if registration.initial_reference_count == 0 {
        return Err(com_registration_error(
            "new COM objects require at least one owning reference",
        ));
    }
    if let Some(parent_interface_id) = registration.parent_interface_id {
        let parent = runtime.interface(parent_interface_id)?;
        if parent.object_id.0 >= runtime.next_object_id {
            return Err(com_registration_error(
                "COM parent reference must target an older live object",
            ));
        }
    }
    if registration.backing.address_allocation_id.0 == 0
        || registration.backing.memory_owner.0 == 0
        || registration.backing.range.byte_length == 0
    {
        return Err(com_registration_error(
            "COM backing requires nonzero allocation, memory owner, and range identities",
        ));
    }
    if registration.interfaces.is_empty()
        || registration.interfaces.len() > RUNTIME_COM_MAX_INTERFACES_PER_OBJECT
    {
        return Err(com_registration_error(
            "COM object interface count is empty or exceeds the bounded foundation",
        ));
    }

    let mut iids = BTreeSet::new();
    let mut pointers = BTreeSet::new();
    let mut vtables = BTreeSet::new();
    let mut endpoints = BTreeSet::new();
    let mut iunknown_count = 0;
    for interface in &registration.interfaces {
        if interface.name.is_empty() || interface.name.len() > RUNTIME_COM_MAX_NAME_BYTES {
            return Err(com_registration_error(
                "COM interface name must be nonempty and bounded",
            ));
        }
        if !iids.insert(interface.iid)
            || runtime
                .iid_index
                .contains_key(&(RuntimeComObjectId(runtime.next_object_id), interface.iid))
        {
            return Err(com_registration_error(
                "one COM object cannot register the same IID more than once",
            ));
        }
        iunknown_count += usize::from(interface.iid == RUNTIME_COM_IUNKNOWN_IID);
        if interface.guest_pointer.0 == 0
            || !interface.guest_pointer.0.is_multiple_of(4)
            || !pointers.insert(interface.guest_pointer)
            || runtime.pointer_index.contains_key(&interface.guest_pointer)
        {
            return Err(com_registration_error(
                "COM interface pointers must be nonzero, aligned, and globally unique",
            ));
        }
        if interface.vtable.0 == 0
            || !interface.vtable.0.is_multiple_of(4)
            || !vtables.insert(interface.vtable)
        {
            return Err(com_registration_error(
                "COM vtable pointers must be nonzero, aligned, and unique per object",
            ));
        }
        if interface.methods.len() < 3
            || interface.methods.len() > RUNTIME_COM_MAX_METHODS_PER_INTERFACE
        {
            return Err(com_registration_error(
                "COM vtable requires IUnknown slots and a bounded method count",
            ));
        }
        for (index, method) in interface.methods.iter().enumerate() {
            let expected_slot = u16::try_from(index).expect("bounded COM method index fits u16");
            if method.slot != expected_slot {
                return Err(com_registration_error(
                    "COM vtable slots must be contiguous and ordered from zero",
                ));
            }
            if method.name.is_empty() || method.name.len() > RUNTIME_COM_MAX_NAME_BYTES {
                return Err(com_registration_error(
                    "COM method name must be nonempty and bounded",
                ));
            }
            if method.endpoint.0 == 0
                || !method.endpoint.0.is_multiple_of(16)
                || !endpoints.insert(method.endpoint)
                || runtime.endpoint_index.contains_key(&method.endpoint)
            {
                return Err(com_registration_error(
                    "COM method endpoints must be nonzero, 16-byte aligned, and globally unique",
                ));
            }
            validate_iunknown_slot(method)?;
        }
        let vtable_bytes = u64::try_from(interface.methods.len())
            .ok()
            .and_then(|count| count.checked_mul(4))
            .ok_or_else(|| com_registration_error("COM vtable byte count overflowed"))?;
        require_backing_contains(
            registration.backing.range,
            RuntimeAddressRange::new(interface.guest_pointer, 4)?,
            "COM interface pointer leaves its object backing",
        )?;
        require_backing_contains(
            registration.backing.range,
            RuntimeAddressRange::new(interface.vtable, vtable_bytes)?,
            "COM vtable leaves its object backing",
        )?;
        for method in &interface.methods {
            require_backing_contains(
                registration.backing.range,
                RuntimeAddressRange::new(method.endpoint, 16)?,
                "COM method endpoint leaves its object backing",
            )?;
        }
    }
    if iunknown_count != 1 {
        return Err(com_registration_error(
            "COM object registration requires exactly one canonical IUnknown IID",
        ));
    }
    Ok(())
}

fn validate_iunknown_slot(method: &RuntimeComMethodRegistration) -> Result<(), RuntimeStateError> {
    let expected = match method.slot {
        0 => Some(2),
        1 | 2 => Some(0),
        _ => None,
    };
    if let Some(words) = expected
        && method.explicit_argument_words != Some(words)
    {
        return Err(com_registration_error(
            "COM slots 0..2 require the native QueryInterface/AddRef/Release ABI",
        ));
    }
    Ok(())
}

fn require_backing_contains(
    backing: RuntimeAddressRange,
    range: RuntimeAddressRange,
    message: &'static str,
) -> Result<(), RuntimeStateError> {
    if !backing.contains(range)? {
        return Err(com_registration_error(message));
    }
    Ok(())
}

fn com_registration_error(message: &'static str) -> RuntimeStateError {
    RuntimeStateError::new(message)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SYNTHETIC_IID: RuntimeComIid = RuntimeComIid([
        0x78, 0x56, 0x34, 0x12, 0x34, 0x12, 0x78, 0x56, 0x90, 0xab, 0xcd, 0xef, 0x01, 0x23, 0x45,
        0x67,
    ]);

    #[test]
    fn direct_references_and_iid_lookup_preserve_canonical_identity() {
        let mut runtime = RuntimeCom::default();
        let registration = registration();
        let object = register(&mut runtime, registration.clone()).unwrap();

        let interface_id = object.interface_ids[1];
        assert_eq!(runtime.add_ref(interface_id).unwrap(), 2);

        let object_before = runtime.object(object.id).unwrap().clone();
        let next_object_id = runtime.next_object_id;
        let next_interface_id = runtime.next_interface_id;
        assert!(register(&mut runtime, registration).is_err());
        assert_eq!(runtime.object(object.id).unwrap(), &object_before);
        assert_eq!(runtime.objects.len(), 1);
        assert_eq!(runtime.interfaces.len(), object.interface_ids.len());
        assert_eq!(runtime.next_object_id, next_object_id);
        assert_eq!(runtime.next_interface_id, next_interface_id);
        let queried = runtime
            .query_interface_target(interface_id, RUNTIME_COM_IUNKNOWN_IID)
            .unwrap()
            .unwrap();
        assert_eq!(queried.id, object.canonical_iunknown_interface_id);
        assert_eq!(runtime.object(object.id).unwrap(), &object_before);
        assert_eq!(runtime.objects.len(), 1);
        assert_eq!(runtime.interfaces.len(), object.interface_ids.len());
        assert_eq!(runtime.next_object_id, next_object_id);
        assert_eq!(runtime.next_interface_id, next_interface_id);
        assert_eq!(runtime.add_ref(interface_id).unwrap(), 3);
    }

    #[test]
    fn guest_pointer_and_endpoint_lookup_resolve_only_live_interfaces() {
        let mut runtime = RuntimeCom::default();
        let missing = GuestAddress(0x0100_ffff);
        let pointer = registration().interfaces[1].guest_pointer;
        assert!(runtime.resolve_method(missing).unwrap().is_none());
        assert!(runtime.interface_for_pointer(pointer).unwrap().is_none());
        let object = register(&mut runtime, registration()).unwrap();
        let interface = runtime.interface(object.interface_ids[1]).unwrap();
        let endpoint = interface.methods[&0].endpoint;
        let target = runtime.resolve_method(endpoint).unwrap().unwrap();
        assert_eq!(target.guest_pointer, pointer);
        assert_eq!(target.iid, SYNTHETIC_IID);
        assert_eq!(target.method.argument_words(), Some(3));
        assert_eq!(
            runtime.interface_for_pointer(pointer).unwrap().unwrap().id,
            target.interface_id
        );
        assert!(
            runtime
                .interface_for_pointer(GuestAddress(pointer.0 + 4))
                .unwrap()
                .is_none()
        );
        let release = runtime.validate_release(target.interface_id).unwrap();
        runtime.remove_validated_objects(release);
        assert!(runtime.interface_for_pointer(pointer).unwrap().is_none());
        assert!(runtime.resolve_method(endpoint).unwrap().is_none());
    }

    #[test]
    fn direct_add_ref_rejects_unknown_identity_without_mutation() {
        let mut runtime = RuntimeCom::default();
        let object = register(&mut runtime, registration()).unwrap();
        let interface_id = object.interface_ids[1];

        let object_before = runtime.object(object.id).unwrap().clone();
        assert!(runtime.add_ref(RuntimeComInterfaceId(u64::MAX)).is_err());
        assert_eq!(runtime.object(object.id).unwrap(), &object_before);
        assert_eq!(runtime.objects.len(), 1);
        assert_eq!(runtime.interfaces.len(), object.interface_ids.len());

        assert!(
            runtime
                .query_interface_target(interface_id, RuntimeComIid([0xff; 16]))
                .unwrap()
                .is_none()
        );
        assert_eq!(runtime.object(object.id).unwrap(), &object_before);
        assert_eq!(runtime.objects.len(), 1);
        assert_eq!(runtime.interfaces.len(), object.interface_ids.len());

        assert_eq!(runtime.add_ref(interface_id).unwrap(), 2);
        assert_eq!(runtime.object(object.id).unwrap().reference_count, 2);
    }

    #[test]
    fn shared_release_is_direct_and_final_release_remains_unpublished() {
        let mut runtime = RuntimeCom::default();
        let object = register(&mut runtime, registration()).unwrap();
        let interface_id = object.interface_ids[1];
        assert_eq!(runtime.add_ref(interface_id).unwrap(), 2);
        assert_eq!(runtime.release_shared(interface_id).unwrap(), Some(1));
        let object_before = runtime.object(object.id).unwrap().clone();
        assert_eq!(runtime.release_shared(interface_id).unwrap(), None);
        assert_eq!(runtime.object(object.id).unwrap(), &object_before);
        assert_eq!(runtime.objects.len(), 1);
        assert_eq!(runtime.interfaces.len(), object.interface_ids.len());
        assert!(
            runtime
                .release_shared(RuntimeComInterfaceId(u64::MAX))
                .is_err()
        );
        assert_eq!(runtime.object(object.id).unwrap(), &object_before);
        assert_eq!(runtime.objects.len(), 1);
        assert_eq!(runtime.interfaces.len(), object.interface_ids.len());
    }

    #[test]
    fn child_registration_and_final_release_hold_one_parent_reference_atomically() {
        let mut runtime = RuntimeCom::default();
        let parent_object = register(&mut runtime, registration()).unwrap();

        let parent_interface_id = parent_object.interface_ids[1];
        let mut child_registration = registration_at(0x0101_0000, 38, 5);
        child_registration.class = RuntimeComClass::D3d9Device;
        child_registration.parent_interface_id = Some(parent_interface_id);
        let child_object = register(&mut runtime, child_registration).unwrap();
        assert_eq!(child_object.parent_interface_id, Some(parent_interface_id));
        assert_eq!(runtime.object(parent_object.id).unwrap().reference_count, 2);

        let release_external_parent = runtime.validate_release(parent_interface_id).unwrap();
        assert_eq!(release_external_parent.reference_count(), 1);
        assert_eq!(release_external_parent.retired_objects().len(), 0);
        runtime.remove_validated_objects(release_external_parent);

        let child_interface_id = child_object.interface_ids[1];
        let release_child = runtime.validate_release(child_interface_id).unwrap();
        assert_eq!(release_child.reference_count(), 0);
        assert_eq!(
            release_child.retired_objects(),
            [child_object.id, parent_object.id]
        );
        runtime.remove_validated_objects(release_child);
        assert!(runtime.is_empty());
    }

    #[test]
    fn child_release_keeps_a_separate_live_parent_reference() {
        let mut runtime = RuntimeCom::default();
        let parent = register(&mut runtime, registration()).unwrap();
        let parent_id = parent.id;
        let parent_interface_id = parent.interface_ids[1];
        let mut child = registration_at(0x0101_0000, 38, 5);
        child.parent_interface_id = Some(parent_interface_id);
        let child = register(&mut runtime, child).unwrap();
        let child_id = child.id;
        let child_interface_id = child.interface_ids[1];
        assert_eq!(
            runtime.release_shared(parent_interface_id).unwrap(),
            Some(1)
        );

        // A separate live reference keeps the parent alive when its child dies.
        assert_eq!(runtime.add_ref(parent_interface_id).unwrap(), 2);
        let release = runtime.validate_release(child_interface_id).unwrap();
        assert_eq!(release.retired_objects(), [child_id]);
        runtime.remove_validated_objects(release);
        assert_eq!(runtime.object(parent_id).unwrap().reference_count, 1);
        assert!(runtime.object(child_id).is_err());
        assert_eq!(runtime.interfaces.len(), 2);
        assert_eq!(
            runtime.interface(parent_interface_id).unwrap().object_id,
            parent_id
        );
        let release = runtime.validate_release(parent_interface_id).unwrap();
        runtime.remove_validated_objects(release);
        assert!(runtime.is_empty());
    }

    #[test]
    fn failed_guest_publication_preserves_parent_references_and_registration_identity() {
        let mut runtime = RuntimeCom::default();
        let parent = register(&mut runtime, registration()).unwrap();
        let mut child = registration_at(0x0101_0000, 38, 5);
        child.parent_interface_id = Some(parent.interface_ids[1]);
        let objects_before = runtime.objects.clone();
        let interfaces_before = runtime.interfaces.clone();
        let pointers_before = runtime.pointer_index.clone();
        let endpoints_before = runtime.endpoint_index.clone();
        let iids_before = runtime.iid_index.clone();
        let ids_before = (runtime.next_object_id, runtime.next_interface_id);
        let failed: Result<(), RuntimeStateError> = runtime.register_with(child.clone(), |_, _| {
            Err(RuntimeStateError::new("fixture guest output write failed"))
        });
        assert!(failed.is_err());
        assert_eq!(runtime.objects, objects_before);
        assert_eq!(runtime.interfaces, interfaces_before);
        assert_eq!(runtime.pointer_index, pointers_before);
        assert_eq!(runtime.endpoint_index, endpoints_before);
        assert_eq!(runtime.iid_index, iids_before);
        assert_eq!(
            (runtime.next_object_id, runtime.next_interface_id),
            ids_before
        );
        let installed = register(&mut runtime, child).unwrap();
        assert_eq!(installed.id, RuntimeComObjectId(ids_before.0));
        assert_eq!(
            installed.interface_ids[0],
            RuntimeComInterfaceId(ids_before.1)
        );
        assert_eq!(runtime.object(parent.id).unwrap().reference_count, 2);
    }

    #[test]
    fn rejected_child_registration_does_not_increment_parent() {
        let mut runtime = RuntimeCom::default();
        let parent_object = register(&mut runtime, registration()).unwrap();
        let next_object_id = runtime.next_object_id;
        let next_interface_id = runtime.next_interface_id;

        let mut child_registration = registration();
        child_registration.parent_interface_id = Some(parent_object.interface_ids[1]);
        assert!(register(&mut runtime, child_registration).is_err());
        assert_eq!(runtime.object(parent_object.id).unwrap(), &parent_object);
        assert_eq!(runtime.objects.len(), 1);
        assert_eq!(runtime.interfaces.len(), parent_object.interface_ids.len());
        assert_eq!(runtime.next_object_id, next_object_id);
        assert_eq!(runtime.next_interface_id, next_interface_id);
    }

    fn register(
        runtime: &mut RuntimeCom,
        registration: RuntimeComObjectRegistration,
    ) -> Result<RuntimeComObjectRecord, RuntimeStateError> {
        runtime.register_with(registration, |object, _| Ok(object.clone()))
    }

    fn registration() -> RuntimeComObjectRegistration {
        registration_at(0x0100_0000, 37, 4)
    }

    fn registration_at(
        base: u32,
        address_allocation_id: u64,
        memory_owner: u32,
    ) -> RuntimeComObjectRegistration {
        RuntimeComObjectRegistration {
            class: RuntimeComClass::D3d9,
            initial_reference_count: 1,
            parent_interface_id: None,
            backing: RuntimeComBacking {
                address_allocation_id: ProcessAddressAllocationId(address_allocation_id),
                memory_owner: MemoryOwnerId(memory_owner),
                range: RuntimeAddressRange {
                    base: GuestAddress(base),
                    byte_length: 0x2000,
                },
            },
            interfaces: vec![
                interface(base, RUNTIME_COM_IUNKNOWN_IID, "IUnknown", 0, 0),
                interface(base, SYNTHETIC_IID, "ISynthetic", 4, 0x100),
            ],
        }
    }

    fn interface(
        base: u32,
        iid: RuntimeComIid,
        name: &str,
        pointer_offset: u32,
        endpoint_offset: u32,
    ) -> RuntimeComInterfaceRegistration {
        RuntimeComInterfaceRegistration {
            iid,
            name: name.to_owned(),
            guest_pointer: GuestAddress(base + pointer_offset),
            vtable: GuestAddress(base + 0x100 + pointer_offset * 4),
            methods: [("QueryInterface", 2), ("AddRef", 0), ("Release", 0)]
                .into_iter()
                .enumerate()
                .map(|(slot, (method, words))| RuntimeComMethodRegistration {
                    slot: slot as u16,
                    name: method.to_owned(),
                    endpoint: GuestAddress(base + 0x1000 + endpoint_offset + slot as u32 * 16),
                    explicit_argument_words: Some(words),
                })
                .collect(),
        }
    }
}

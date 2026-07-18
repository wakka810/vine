use std::collections::{BTreeMap, BTreeSet};

use serde::{Serialize, Serializer};

use crate::memory::{GuestAddress, MemoryOwnerId};

use super::{ProcessAddressAllocationId, RuntimeAddressRange, RuntimeStateError};

pub const RUNTIME_COM_IUNKNOWN_IID: RuntimeComIid = RuntimeComIid([
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xc0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46,
]);
pub const RUNTIME_COM_MAX_INTERFACES_PER_OBJECT: usize = 32;
pub const RUNTIME_COM_MAX_METHODS_PER_INTERFACE: usize = 256;
pub(crate) const RUNTIME_COM_ADDRESS_OWNER: &str = "runtime-com-object";
const RUNTIME_COM_MAX_IDENTITY_BYTES: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct RuntimeComIid(pub [u8; 16]);

impl RuntimeComIid {
    pub const fn from_windows_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    pub const fn windows_bytes(self) -> [u8; 16] {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct RuntimeComObjectId(pub u64);

impl Serialize for RuntimeComObjectId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0.to_string())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct RuntimeComInterfaceId(pub u64);

impl Serialize for RuntimeComInterfaceId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0.to_string())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeComReturnKind {
    HResult,
    U32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeComMethodImplementation {
    QueryInterface,
    AddRef,
    Release,
    Frontend,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeComBacking {
    pub address_allocation_id: ProcessAddressAllocationId,
    pub memory_owner: MemoryOwnerId,
    pub range: RuntimeAddressRange,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeComMethodRecord {
    pub slot: u16,
    pub name: String,
    pub endpoint: GuestAddress,
    pub explicit_argument_words: u16,
    pub return_kind: RuntimeComReturnKind,
    pub implementation: RuntimeComMethodImplementation,
}

impl RuntimeComMethodRecord {
    pub fn argument_words(&self) -> Result<u32, RuntimeStateError> {
        u32::from(self.explicit_argument_words)
            .checked_add(1)
            .ok_or_else(|| {
                RuntimeStateError::resource_limit(
                    "runtime-com",
                    "com-method-argument-count-overflow",
                    "COM method argument count overflowed x86 word accounting",
                )
            })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeComInterfaceRecord {
    pub id: RuntimeComInterfaceId,
    pub object_id: RuntimeComObjectId,
    pub iid: RuntimeComIid,
    pub name: String,
    pub guest_pointer: GuestAddress,
    pub vtable: GuestAddress,
    pub methods: BTreeMap<u16, RuntimeComMethodRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeComObjectRecord {
    pub id: RuntimeComObjectId,
    pub class_identity: String,
    pub reference_count: u32,
    pub canonical_iunknown_interface_id: RuntimeComInterfaceId,
    pub interface_ids: Vec<RuntimeComInterfaceId>,
    pub backing: RuntimeComBacking,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeComMethodRegistration {
    pub slot: u16,
    pub name: String,
    pub endpoint: GuestAddress,
    pub explicit_argument_words: u16,
    pub return_kind: RuntimeComReturnKind,
    pub implementation: RuntimeComMethodImplementation,
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
    pub class_identity: String,
    pub initial_reference_count: u32,
    pub backing: RuntimeComBacking,
    pub interfaces: Vec<RuntimeComInterfaceRegistration>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeComMethodTarget {
    pub object: RuntimeComObjectRecord,
    pub interface: RuntimeComInterfaceRecord,
    pub method: RuntimeComMethodRecord,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeComQueryResult {
    pub source_interface: RuntimeComInterfaceRecord,
    pub target_interface: Option<RuntimeComInterfaceRecord>,
    pub reference_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeComReferenceResult {
    pub interface: RuntimeComInterfaceRecord,
    pub reference_count: u32,
    pub retired_backing: Option<RuntimeComBacking>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeComRegistrationResult {
    pub object: RuntimeComObjectRecord,
    pub interfaces: Vec<RuntimeComInterfaceRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RuntimeComMutationOutcome {
    Registered(RuntimeComRegistrationResult),
    Queried(RuntimeComQueryResult),
    Referenced(RuntimeComReferenceResult),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeComMutationPlan {
    previous: RuntimeCom,
    next: RuntimeCom,
    outcome: RuntimeComMutationOutcome,
}

impl RuntimeComMutationPlan {
    pub(crate) fn registration_result(&self) -> RuntimeComRegistrationResult {
        match &self.outcome {
            RuntimeComMutationOutcome::Registered(result) => result.clone(),
            _ => unreachable!("COM mutation plan is not an object registration"),
        }
    }

    pub(crate) fn query_result(&self) -> RuntimeComQueryResult {
        match &self.outcome {
            RuntimeComMutationOutcome::Queried(result) => result.clone(),
            _ => unreachable!("COM mutation plan is not QueryInterface"),
        }
    }

    pub(crate) fn reference_result(&self) -> RuntimeComReferenceResult {
        match &self.outcome {
            RuntimeComMutationOutcome::Referenced(result) => result.clone(),
            _ => unreachable!("COM mutation plan is not a reference mutation"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeCom {
    objects: BTreeMap<RuntimeComObjectId, RuntimeComObjectRecord>,
    interfaces: BTreeMap<RuntimeComInterfaceId, RuntimeComInterfaceRecord>,
    #[serde(skip)]
    pointer_index: BTreeMap<GuestAddress, RuntimeComInterfaceId>,
    #[serde(skip)]
    endpoint_index: BTreeMap<GuestAddress, (RuntimeComInterfaceId, u16)>,
    #[serde(skip)]
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
    pub fn is_empty(&self) -> bool {
        self.objects.is_empty()
    }

    pub fn objects(&self) -> &BTreeMap<RuntimeComObjectId, RuntimeComObjectRecord> {
        &self.objects
    }

    pub fn interfaces(&self) -> &BTreeMap<RuntimeComInterfaceId, RuntimeComInterfaceRecord> {
        &self.interfaces
    }

    pub fn object(
        &self,
        id: RuntimeComObjectId,
    ) -> Result<&RuntimeComObjectRecord, RuntimeStateError> {
        self.objects.get(&id).ok_or_else(|| {
            RuntimeStateError::not_found(
                "runtime-com",
                "com-object-id-unknown",
                format!("logical COM object ID {} is unknown", id.0),
            )
        })
    }

    pub fn interface(
        &self,
        id: RuntimeComInterfaceId,
    ) -> Result<&RuntimeComInterfaceRecord, RuntimeStateError> {
        self.interfaces.get(&id).ok_or_else(|| {
            RuntimeStateError::not_found(
                "runtime-com",
                "com-interface-id-unknown",
                format!("logical COM interface ID {} is unknown", id.0),
            )
        })
    }

    pub fn interface_for_pointer(
        &self,
        pointer: GuestAddress,
    ) -> Result<&RuntimeComInterfaceRecord, RuntimeStateError> {
        let id = self.pointer_index.get(&pointer).copied().ok_or_else(|| {
            RuntimeStateError::not_found(
                "runtime-com",
                "com-interface-pointer-unknown",
                format!("guest COM interface pointer {:#010x} is unknown", pointer.0),
            )
        })?;
        self.interface(id)
    }

    pub fn resolve_method(
        &self,
        endpoint: GuestAddress,
    ) -> Result<RuntimeComMethodTarget, RuntimeStateError> {
        let (interface_id, slot) =
            self.endpoint_index.get(&endpoint).copied().ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-com",
                    "com-endpoint-unowned",
                    format!(
                        "COM endpoint {:#010x} is not owned by a live interface",
                        endpoint.0
                    ),
                )
            })?;
        let interface = self.interface(interface_id)?.clone();
        let object = self.object(interface.object_id)?.clone();
        let method = interface.methods.get(&slot).cloned().ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-com",
                "com-endpoint-method-missing",
                "COM endpoint index references a missing vtable method",
            )
        })?;
        Ok(RuntimeComMethodTarget {
            object,
            interface,
            method,
        })
    }

    pub(crate) fn plan_registration(
        &self,
        registration: RuntimeComObjectRegistration,
    ) -> Result<RuntimeComMutationPlan, RuntimeStateError> {
        let mut next = self.clone();
        let object = next.register_object(registration)?;
        let interfaces = object
            .interface_ids
            .iter()
            .map(|id| {
                next.interfaces
                    .get(id)
                    .expect("new COM object lost a registered interface")
                    .clone()
            })
            .collect();
        Ok(RuntimeComMutationPlan {
            previous: self.clone(),
            next,
            outcome: RuntimeComMutationOutcome::Registered(RuntimeComRegistrationResult {
                object,
                interfaces,
            }),
        })
    }

    pub(crate) fn plan_query_interface(
        &self,
        source_interface_id: RuntimeComInterfaceId,
        iid: RuntimeComIid,
    ) -> Result<RuntimeComMutationPlan, RuntimeStateError> {
        let source_interface = self.interface(source_interface_id)?.clone();
        let object = self.object(source_interface.object_id)?;
        let target_interface = self
            .iid_index
            .get(&(object.id, iid))
            .copied()
            .map(|id| self.interface(id).cloned())
            .transpose()?;
        let mut next = self.clone();
        let reference_count = if target_interface.is_some() {
            next.increment_reference_count(object.id)?
        } else {
            object.reference_count
        };
        Ok(RuntimeComMutationPlan {
            previous: self.clone(),
            next,
            outcome: RuntimeComMutationOutcome::Queried(RuntimeComQueryResult {
                source_interface,
                target_interface,
                reference_count,
            }),
        })
    }

    pub(crate) fn plan_add_ref(
        &self,
        interface_id: RuntimeComInterfaceId,
    ) -> Result<RuntimeComMutationPlan, RuntimeStateError> {
        let interface = self.interface(interface_id)?.clone();
        let mut next = self.clone();
        let reference_count = next.increment_reference_count(interface.object_id)?;
        Ok(RuntimeComMutationPlan {
            previous: self.clone(),
            next,
            outcome: RuntimeComMutationOutcome::Referenced(RuntimeComReferenceResult {
                interface,
                reference_count,
                retired_backing: None,
            }),
        })
    }

    pub(crate) fn plan_release(
        &self,
        interface_id: RuntimeComInterfaceId,
    ) -> Result<RuntimeComMutationPlan, RuntimeStateError> {
        let interface = self.interface(interface_id)?.clone();
        let object = self.object(interface.object_id)?.clone();
        if object.reference_count == 0 {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-com",
                "com-reference-count-underflow",
                "live COM object has zero references",
            ));
        }
        let reference_count = object.reference_count - 1;
        let mut next = self.clone();
        let retired_backing = if reference_count == 0 {
            next.retire_object(object.id)?;
            Some(object.backing)
        } else {
            next.objects
                .get_mut(&object.id)
                .expect("cloned COM object disappeared")
                .reference_count = reference_count;
            None
        };
        Ok(RuntimeComMutationPlan {
            previous: self.clone(),
            next,
            outcome: RuntimeComMutationOutcome::Referenced(RuntimeComReferenceResult {
                interface,
                reference_count,
                retired_backing,
            }),
        })
    }

    pub(crate) fn preflight(&self, plan: &RuntimeComMutationPlan) -> Result<(), RuntimeStateError> {
        if self != &plan.previous {
            return Err(RuntimeStateError::conflict(
                "runtime-com",
                "com-mutation-plan-stale",
                "COM identity or reference state changed after mutation planning",
            ));
        }
        Ok(())
    }

    pub(crate) fn commit(&mut self, plan: RuntimeComMutationPlan) {
        debug_assert_eq!(self, &plan.previous);
        *self = plan.next;
    }

    fn register_object(
        &mut self,
        registration: RuntimeComObjectRegistration,
    ) -> Result<RuntimeComObjectRecord, RuntimeStateError> {
        validate_registration(self, &registration)?;
        let object_id = RuntimeComObjectId(self.next_object_id);
        let next_object_id = self.next_object_id.checked_add(1).ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-com",
                "com-object-id-exhausted",
                "logical COM object ID space is exhausted",
            )
        })?;
        let interface_count = u64::try_from(registration.interfaces.len()).map_err(|_| {
            RuntimeStateError::resource_limit(
                "runtime-com",
                "com-interface-count-overflow",
                "COM interface count does not fit logical ID accounting",
            )
        })?;
        let next_interface_id = self
            .next_interface_id
            .checked_add(interface_count)
            .ok_or_else(|| {
                RuntimeStateError::resource_limit(
                    "runtime-com",
                    "com-interface-id-exhausted",
                    "logical COM interface ID space is exhausted",
                )
            })?;

        let mut interface_ids = Vec::with_capacity(registration.interfaces.len());
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
                            return_kind: method.return_kind,
                            implementation: method.implementation,
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
            for method in record.methods.values() {
                self.endpoint_index
                    .insert(method.endpoint, (interface_id, method.slot));
            }
            self.pointer_index
                .insert(record.guest_pointer, interface_id);
            self.iid_index.insert((object_id, record.iid), interface_id);
            self.interfaces.insert(interface_id, record);
            interface_ids.push(interface_id);
        }
        let canonical_iunknown_interface_id = canonical_iunknown_interface_id.ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-com",
                "com-iunknown-registration-lost",
                "validated COM registration lost its canonical IUnknown interface",
            )
        })?;
        let object = RuntimeComObjectRecord {
            id: object_id,
            class_identity: registration.class_identity,
            reference_count: registration.initial_reference_count,
            canonical_iunknown_interface_id,
            interface_ids,
            backing: registration.backing,
        };
        self.objects.insert(object_id, object.clone());
        self.next_object_id = next_object_id;
        self.next_interface_id = next_interface_id;
        Ok(object)
    }

    fn increment_reference_count(
        &mut self,
        object_id: RuntimeComObjectId,
    ) -> Result<u32, RuntimeStateError> {
        let object = self.objects.get_mut(&object_id).ok_or_else(|| {
            RuntimeStateError::not_found(
                "runtime-com",
                "com-object-id-unknown",
                "COM reference mutation targets an unknown logical object",
            )
        })?;
        object.reference_count = object.reference_count.checked_add(1).ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-com",
                "com-reference-count-overflow",
                "COM reference count overflowed ULONG accounting",
            )
        })?;
        Ok(object.reference_count)
    }

    fn retire_object(&mut self, object_id: RuntimeComObjectId) -> Result<(), RuntimeStateError> {
        let object = self.objects.remove(&object_id).ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-com",
                "com-object-retirement-missing",
                "preflighted COM object disappeared during retirement",
            )
        })?;
        for interface_id in object.interface_ids {
            let interface = self.interfaces.remove(&interface_id).ok_or_else(|| {
                RuntimeStateError::runtime_defect(
                    "runtime-com",
                    "com-interface-retirement-missing",
                    "COM object retirement found a missing interface",
                )
            })?;
            self.pointer_index.remove(&interface.guest_pointer);
            self.iid_index.remove(&(object_id, interface.iid));
            for method in interface.methods.values() {
                self.endpoint_index.remove(&method.endpoint);
            }
        }
        Ok(())
    }
}

fn validate_registration(
    runtime: &RuntimeCom,
    registration: &RuntimeComObjectRegistration,
) -> Result<(), RuntimeStateError> {
    if registration.class_identity.is_empty()
        || registration.class_identity.len() > RUNTIME_COM_MAX_IDENTITY_BYTES
    {
        return Err(com_registration_error(
            "com-class-identity-invalid",
            "COM class identity must be nonempty and bounded",
        ));
    }
    if registration.initial_reference_count == 0 {
        return Err(com_registration_error(
            "com-initial-reference-count-zero",
            "new COM objects require at least one owning reference",
        ));
    }
    if registration.backing.address_allocation_id.0 == 0
        || registration.backing.memory_owner.0 == 0
        || registration.backing.range.byte_length == 0
    {
        return Err(com_registration_error(
            "com-backing-identity-invalid",
            "COM backing requires nonzero allocation, memory owner, and range identities",
        ));
    }
    if registration.interfaces.is_empty()
        || registration.interfaces.len() > RUNTIME_COM_MAX_INTERFACES_PER_OBJECT
    {
        return Err(com_registration_error(
            "com-interface-count-invalid",
            "COM object interface count is empty or exceeds the bounded foundation",
        ));
    }

    let mut iids = BTreeSet::new();
    let mut pointers = BTreeSet::new();
    let mut vtables = BTreeSet::new();
    let mut endpoints = BTreeSet::new();
    let mut iunknown_count = 0;
    for interface in &registration.interfaces {
        if interface.name.is_empty() || interface.name.len() > RUNTIME_COM_MAX_IDENTITY_BYTES {
            return Err(com_registration_error(
                "com-interface-name-invalid",
                "COM interface name must be nonempty and bounded",
            ));
        }
        if !iids.insert(interface.iid)
            || runtime
                .iid_index
                .contains_key(&(RuntimeComObjectId(runtime.next_object_id), interface.iid))
        {
            return Err(com_registration_error(
                "com-interface-iid-duplicate",
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
                "com-interface-pointer-invalid",
                "COM interface pointers must be nonzero, aligned, and globally unique",
            ));
        }
        if interface.vtable.0 == 0
            || !interface.vtable.0.is_multiple_of(4)
            || !vtables.insert(interface.vtable)
        {
            return Err(com_registration_error(
                "com-vtable-pointer-invalid",
                "COM vtable pointers must be nonzero, aligned, and unique per object",
            ));
        }
        if interface.methods.len() < 3
            || interface.methods.len() > RUNTIME_COM_MAX_METHODS_PER_INTERFACE
        {
            return Err(com_registration_error(
                "com-vtable-method-count-invalid",
                "COM vtable requires IUnknown slots and a bounded method count",
            ));
        }
        for (index, method) in interface.methods.iter().enumerate() {
            let expected_slot = u16::try_from(index).expect("bounded COM method index fits u16");
            if method.slot != expected_slot {
                return Err(com_registration_error(
                    "com-vtable-slot-order-invalid",
                    "COM vtable slots must be contiguous and ordered from zero",
                ));
            }
            if method.name.is_empty() || method.name.len() > RUNTIME_COM_MAX_IDENTITY_BYTES {
                return Err(com_registration_error(
                    "com-method-name-invalid",
                    "COM method name must be nonempty and bounded",
                ));
            }
            if method.endpoint.0 == 0
                || !method.endpoint.0.is_multiple_of(16)
                || !endpoints.insert(method.endpoint)
                || runtime.endpoint_index.contains_key(&method.endpoint)
            {
                return Err(com_registration_error(
                    "com-method-endpoint-invalid",
                    "COM method endpoints must be nonzero, 16-byte aligned, and globally unique",
                ));
            }
            validate_iunknown_slot(method)?;
        }
        let vtable_bytes = u64::try_from(interface.methods.len())
            .ok()
            .and_then(|count| count.checked_mul(4))
            .ok_or_else(|| {
                com_registration_error(
                    "com-vtable-byte-count-overflow",
                    "COM vtable byte count overflowed",
                )
            })?;
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
            "com-canonical-iunknown-count-invalid",
            "COM object registration requires exactly one canonical IUnknown IID",
        ));
    }
    Ok(())
}

fn validate_iunknown_slot(method: &RuntimeComMethodRegistration) -> Result<(), RuntimeStateError> {
    let expected = match method.slot {
        0 => Some((
            "QueryInterface",
            2,
            RuntimeComReturnKind::HResult,
            RuntimeComMethodImplementation::QueryInterface,
        )),
        1 => Some((
            "AddRef",
            0,
            RuntimeComReturnKind::U32,
            RuntimeComMethodImplementation::AddRef,
        )),
        2 => Some((
            "Release",
            0,
            RuntimeComReturnKind::U32,
            RuntimeComMethodImplementation::Release,
        )),
        _ => None,
    };
    if let Some((name, words, return_kind, implementation)) = expected {
        if method.name != name
            || method.explicit_argument_words != words
            || method.return_kind != return_kind
            || method.implementation != implementation
        {
            return Err(com_registration_error(
                "com-iunknown-slot-contract-invalid",
                "COM slots 0..2 must use the canonical QueryInterface/AddRef/Release ABI",
            ));
        }
    } else if !matches!(
        method.implementation,
        RuntimeComMethodImplementation::Frontend | RuntimeComMethodImplementation::Unsupported
    ) {
        return Err(com_registration_error(
            "com-noniunknown-implementation-invalid",
            "non-IUnknown slots must be delegated to a versioned frontend or remain unsupported",
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
        return Err(com_registration_error(
            "com-backing-range-mismatch",
            message,
        ));
    }
    Ok(())
}

fn com_registration_error(code: &'static str, message: &'static str) -> RuntimeStateError {
    RuntimeStateError::invalid_input("runtime-com", code, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SYNTHETIC_IID: RuntimeComIid = RuntimeComIid([
        0x78, 0x56, 0x34, 0x12, 0x34, 0x12, 0x78, 0x56, 0x90, 0xab, 0xcd, 0xef, 0x01, 0x23, 0x45,
        0x67,
    ]);

    #[test]
    fn stale_or_duplicate_plans_preserve_com_identity_and_refcounts() {
        let runtime = RuntimeCom::default();
        let registration = registration();
        let plan = runtime.plan_registration(registration.clone()).unwrap();
        let object = plan.registration_result().object;
        let mut runtime = runtime;
        runtime.preflight(&plan).unwrap();
        runtime.commit(plan);

        let interface_id = object.interface_ids[1];
        let first = runtime.plan_add_ref(interface_id).unwrap();
        let stale = runtime.plan_add_ref(interface_id).unwrap();
        runtime.preflight(&first).unwrap();
        runtime.commit(first);
        let before_stale = runtime.clone();
        assert_eq!(
            runtime.preflight(&stale).unwrap_err().code,
            "com-mutation-plan-stale"
        );
        assert_eq!(runtime, before_stale);

        let before_duplicate = runtime.clone();
        assert_eq!(
            runtime.plan_registration(registration).unwrap_err().code,
            "com-interface-pointer-invalid"
        );
        assert_eq!(runtime, before_duplicate);

        let query = runtime
            .plan_query_interface(interface_id, RUNTIME_COM_IUNKNOWN_IID)
            .unwrap();
        let result = query.query_result();
        assert_eq!(result.source_interface.iid, SYNTHETIC_IID);
        assert_eq!(
            result.target_interface.unwrap().id,
            object.canonical_iunknown_interface_id
        );
    }

    fn registration() -> RuntimeComObjectRegistration {
        RuntimeComObjectRegistration {
            class_identity: "fixture.synthetic-com.v1".to_owned(),
            initial_reference_count: 1,
            backing: RuntimeComBacking {
                address_allocation_id: ProcessAddressAllocationId(37),
                memory_owner: MemoryOwnerId(4),
                range: RuntimeAddressRange {
                    base: GuestAddress(0x0100_0000),
                    byte_length: 0x2000,
                },
            },
            interfaces: vec![
                interface(RUNTIME_COM_IUNKNOWN_IID, "IUnknown", 0, 0),
                interface(SYNTHETIC_IID, "ISynthetic", 4, 0x100),
            ],
        }
    }

    fn interface(
        iid: RuntimeComIid,
        name: &str,
        pointer_offset: u32,
        endpoint_offset: u32,
    ) -> RuntimeComInterfaceRegistration {
        RuntimeComInterfaceRegistration {
            iid,
            name: name.to_owned(),
            guest_pointer: GuestAddress(0x0100_0000 + pointer_offset),
            vtable: GuestAddress(0x0100_0100 + pointer_offset * 4),
            methods: [
                (
                    "QueryInterface",
                    2,
                    RuntimeComReturnKind::HResult,
                    RuntimeComMethodImplementation::QueryInterface,
                ),
                (
                    "AddRef",
                    0,
                    RuntimeComReturnKind::U32,
                    RuntimeComMethodImplementation::AddRef,
                ),
                (
                    "Release",
                    0,
                    RuntimeComReturnKind::U32,
                    RuntimeComMethodImplementation::Release,
                ),
            ]
            .into_iter()
            .enumerate()
            .map(|(slot, (method, words, return_kind, implementation))| {
                RuntimeComMethodRegistration {
                    slot: slot as u16,
                    name: method.to_owned(),
                    endpoint: GuestAddress(0x0100_1000 + endpoint_offset + slot as u32 * 16),
                    explicit_argument_words: words,
                    return_kind,
                    implementation,
                }
            })
            .collect(),
        }
    }
}

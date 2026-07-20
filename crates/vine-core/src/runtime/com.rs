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
    Void,
    X87F32,
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
    #[serde(skip_serializing_if = "Option::is_none")]
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
    pub parent_interface_id: Option<RuntimeComInterfaceId>,
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
    pub retirements: Vec<RuntimeComObjectRetirement>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeComObjectRetirement {
    pub object_id: RuntimeComObjectId,
    pub backing: RuntimeComBacking,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeComRegistrationResult {
    pub object: RuntimeComObjectRecord,
    pub interfaces: Vec<RuntimeComInterfaceRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RuntimeComReferenceCountUpdate {
    object_id: RuntimeComObjectId,
    previous: u32,
    next: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RuntimeComRetirementMutation {
    object: RuntimeComObjectRecord,
    interfaces: Vec<RuntimeComInterfaceRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RuntimeComMutation {
    Registration {
        expected_generation: u64,
        next_generation: u64,
        next_object_id: u64,
        next_interface_id: u64,
        parent_reference_update: Option<RuntimeComReferenceCountUpdate>,
        object: RuntimeComObjectRecord,
        interfaces: Vec<RuntimeComInterfaceRecord>,
    },
    Query {
        expected_generation: u64,
        next_generation: u64,
        reference_update: Option<RuntimeComReferenceCountUpdate>,
        source_interface: RuntimeComInterfaceRecord,
        target_interface: Option<RuntimeComInterfaceRecord>,
        reference_count: u32,
    },
    Reference {
        expected_generation: u64,
        next_generation: u64,
        reference_updates: Vec<RuntimeComReferenceCountUpdate>,
        retirements: Vec<RuntimeComRetirementMutation>,
        interface: RuntimeComInterfaceRecord,
        reference_count: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeComMutationPlan {
    mutation: RuntimeComMutation,
}

impl RuntimeComMutationPlan {
    pub(crate) fn registration_result(&self) -> RuntimeComRegistrationResult {
        match &self.mutation {
            RuntimeComMutation::Registration {
                object, interfaces, ..
            } => RuntimeComRegistrationResult {
                object: object.clone(),
                interfaces: interfaces.clone(),
            },
            _ => unreachable!("COM mutation plan is not an object registration"),
        }
    }

    pub(crate) fn query_result(&self) -> RuntimeComQueryResult {
        match &self.mutation {
            RuntimeComMutation::Query {
                source_interface,
                target_interface,
                reference_count,
                ..
            } => RuntimeComQueryResult {
                source_interface: source_interface.clone(),
                target_interface: target_interface.clone(),
                reference_count: *reference_count,
            },
            _ => unreachable!("COM mutation plan is not QueryInterface"),
        }
    }

    pub(crate) fn reference_result(&self) -> RuntimeComReferenceResult {
        match &self.mutation {
            RuntimeComMutation::Reference {
                interface,
                reference_count,
                retirements,
                ..
            } => RuntimeComReferenceResult {
                interface: interface.clone(),
                reference_count: *reference_count,
                retirements: retirements
                    .iter()
                    .map(|retirement| RuntimeComObjectRetirement {
                        object_id: retirement.object.id,
                        backing: retirement.object.backing.clone(),
                    })
                    .collect(),
            },
            _ => unreachable!("COM mutation plan is not a reference mutation"),
        }
    }

    fn expected_generation(&self) -> u64 {
        match &self.mutation {
            RuntimeComMutation::Registration {
                expected_generation,
                ..
            }
            | RuntimeComMutation::Query {
                expected_generation,
                ..
            }
            | RuntimeComMutation::Reference {
                expected_generation,
                ..
            } => *expected_generation,
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
    #[serde(skip)]
    mutation_generation: u64,
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
            mutation_generation: 0,
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
                RuntimeStateError::not_found(
                    "runtime-com",
                    "com-object-interface-iid-unknown",
                    format!(
                        "logical COM object ID {} does not expose IID {:?}",
                        object_id.0, iid
                    ),
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
        let parent_reference_update = registration
            .parent_interface_id
            .map(|parent_interface_id| {
                let parent_object_id = self.interface(parent_interface_id)?.object_id;
                self.plan_reference_increment(parent_object_id)
            })
            .transpose()?;
        let (object, interfaces, next_object_id, next_interface_id) =
            self.build_registration(registration)?;
        Ok(RuntimeComMutationPlan {
            mutation: RuntimeComMutation::Registration {
                expected_generation: self.mutation_generation,
                next_generation: self.next_mutation_generation()?,
                next_object_id,
                next_interface_id,
                parent_reference_update,
                object,
                interfaces,
            },
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
        let reference_update = if target_interface.is_some() {
            Some(self.plan_reference_increment(object.id)?)
        } else {
            None
        };
        let reference_count = reference_update
            .as_ref()
            .map_or(object.reference_count, |update| update.next);
        Ok(RuntimeComMutationPlan {
            mutation: RuntimeComMutation::Query {
                expected_generation: self.mutation_generation,
                next_generation: if reference_update.is_some() {
                    self.next_mutation_generation()?
                } else {
                    self.mutation_generation
                },
                reference_update,
                source_interface,
                target_interface,
                reference_count,
            },
        })
    }

    pub(crate) fn plan_add_ref(
        &self,
        interface_id: RuntimeComInterfaceId,
    ) -> Result<RuntimeComMutationPlan, RuntimeStateError> {
        let interface = self.interface(interface_id)?.clone();
        let update = self.plan_reference_increment(interface.object_id)?;
        let reference_count = update.next;
        Ok(RuntimeComMutationPlan {
            mutation: RuntimeComMutation::Reference {
                expected_generation: self.mutation_generation,
                next_generation: self.next_mutation_generation()?,
                reference_updates: vec![update],
                retirements: Vec::new(),
                interface,
                reference_count,
            },
        })
    }

    pub(crate) fn plan_release(
        &self,
        interface_id: RuntimeComInterfaceId,
    ) -> Result<RuntimeComMutationPlan, RuntimeStateError> {
        let interface = self.interface(interface_id)?.clone();
        let mut reference_updates = Vec::new();
        let mut retirements = Vec::new();
        let reference_count = self.plan_reference_release(
            interface.object_id,
            &mut reference_updates,
            &mut retirements,
        )?;
        Ok(RuntimeComMutationPlan {
            mutation: RuntimeComMutation::Reference {
                expected_generation: self.mutation_generation,
                next_generation: self.next_mutation_generation()?,
                reference_updates,
                retirements,
                interface,
                reference_count,
            },
        })
    }

    pub(crate) fn preflight(&self, plan: &RuntimeComMutationPlan) -> Result<(), RuntimeStateError> {
        if self.mutation_generation != plan.expected_generation() {
            return Err(RuntimeStateError::conflict(
                "runtime-com",
                "com-mutation-plan-stale",
                "COM identity or reference state changed after mutation planning",
            ));
        }
        Ok(())
    }

    pub(crate) fn commit(&mut self, plan: RuntimeComMutationPlan) {
        debug_assert_eq!(self.mutation_generation, plan.expected_generation());
        match plan.mutation {
            RuntimeComMutation::Registration {
                next_generation,
                next_object_id,
                next_interface_id,
                parent_reference_update,
                object,
                interfaces,
                ..
            } => {
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
                    let pointer_replaced = self
                        .pointer_index
                        .insert(interface.guest_pointer, interface.id);
                    debug_assert!(pointer_replaced.is_none());
                    let iid_replaced = self
                        .iid_index
                        .insert((object.id, interface.iid), interface.id);
                    debug_assert!(iid_replaced.is_none());
                    let interface_replaced = self.interfaces.insert(interface.id, interface);
                    debug_assert!(interface_replaced.is_none());
                }
                let object_replaced = self.objects.insert(object.id, object);
                debug_assert!(object_replaced.is_none());
                self.next_object_id = next_object_id;
                self.next_interface_id = next_interface_id;
                self.mutation_generation = next_generation;
            }
            RuntimeComMutation::Query {
                next_generation,
                reference_update,
                ..
            } => {
                if let Some(update) = reference_update {
                    self.commit_reference_update(update);
                }
                self.mutation_generation = next_generation;
            }
            RuntimeComMutation::Reference {
                next_generation,
                reference_updates,
                retirements,
                ..
            } => {
                for update in reference_updates {
                    self.commit_reference_update(update);
                }
                for retirement in retirements {
                    self.commit_retirement(retirement);
                }
                self.mutation_generation = next_generation;
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
            interface_ids.push(interface_id);
            interfaces.push(record);
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
            parent_interface_id: registration.parent_interface_id,
            canonical_iunknown_interface_id,
            interface_ids,
            backing: registration.backing,
        };
        Ok((object, interfaces, next_object_id, next_interface_id))
    }

    fn plan_reference_increment(
        &self,
        object_id: RuntimeComObjectId,
    ) -> Result<RuntimeComReferenceCountUpdate, RuntimeStateError> {
        let object = self.object(object_id)?;
        let next = object.reference_count.checked_add(1).ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-com",
                "com-reference-count-overflow",
                "COM reference count overflowed ULONG accounting",
            )
        })?;
        Ok(RuntimeComReferenceCountUpdate {
            object_id,
            previous: object.reference_count,
            next,
        })
    }

    fn plan_reference_release(
        &self,
        object_id: RuntimeComObjectId,
        reference_updates: &mut Vec<RuntimeComReferenceCountUpdate>,
        retirements: &mut Vec<RuntimeComRetirementMutation>,
    ) -> Result<u32, RuntimeStateError> {
        let object = self.object(object_id)?.clone();
        if object.reference_count == 0 {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-com",
                "com-reference-count-underflow",
                "live COM object has zero references",
            ));
        }
        let reference_count = object.reference_count - 1;
        if reference_count != 0 {
            reference_updates.push(RuntimeComReferenceCountUpdate {
                object_id,
                previous: object.reference_count,
                next: reference_count,
            });
            return Ok(reference_count);
        }

        let interfaces = object
            .interface_ids
            .iter()
            .map(|interface_id| self.interface(*interface_id).cloned())
            .collect::<Result<Vec<_>, _>>()?;
        let parent_interface_id = object.parent_interface_id;
        retirements.push(RuntimeComRetirementMutation { object, interfaces });
        if let Some(parent_interface_id) = parent_interface_id {
            let parent_object_id = self.interface(parent_interface_id)?.object_id;
            self.plan_reference_release(parent_object_id, reference_updates, retirements)?;
        }
        Ok(0)
    }

    fn commit_reference_update(&mut self, update: RuntimeComReferenceCountUpdate) {
        let object = self
            .objects
            .get_mut(&update.object_id)
            .expect("preflighted COM reference target disappeared");
        debug_assert_eq!(object.reference_count, update.previous);
        object.reference_count = update.next;
    }

    fn commit_retirement(&mut self, retirement: RuntimeComRetirementMutation) {
        let object_id = retirement.object.id;
        let removed_object = self
            .objects
            .remove(&object_id)
            .expect("preflighted COM object disappeared during retirement");
        debug_assert_eq!(removed_object, retirement.object);
        for expected in retirement.interfaces {
            let interface = self
                .interfaces
                .remove(&expected.id)
                .expect("preflighted COM interface disappeared during retirement");
            debug_assert_eq!(interface, expected);
            let pointer = self.pointer_index.remove(&interface.guest_pointer);
            debug_assert_eq!(pointer, Some(interface.id));
            let iid = self.iid_index.remove(&(object_id, interface.iid));
            debug_assert_eq!(iid, Some(interface.id));
            for method in interface.methods.values() {
                let endpoint = self.endpoint_index.remove(&method.endpoint);
                debug_assert_eq!(endpoint, Some((interface.id, method.slot)));
            }
        }
    }

    fn next_mutation_generation(&self) -> Result<u64, RuntimeStateError> {
        self.mutation_generation.checked_add(1).ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-com",
                "com-mutation-generation-exhausted",
                "COM mutation generation is exhausted",
            )
        })
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
    if let Some(parent_interface_id) = registration.parent_interface_id {
        let parent = runtime.interface(parent_interface_id)?;
        if parent.object_id.0 >= runtime.next_object_id {
            return Err(com_registration_error(
                "com-parent-reference-order-invalid",
                "COM parent reference must target an older live object",
            ));
        }
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

    #[test]
    fn child_registration_and_final_release_hold_one_parent_reference_atomically() {
        let mut runtime = RuntimeCom::default();
        let parent = runtime.plan_registration(registration()).unwrap();
        let parent_object = parent.registration_result().object;
        runtime.commit(parent);

        let parent_interface_id = parent_object.interface_ids[1];
        let mut child_registration = registration_at(0x0101_0000, 38, 5);
        child_registration.class_identity = "fixture.synthetic-child.v1".to_owned();
        child_registration.parent_interface_id = Some(parent_interface_id);
        let child = runtime.plan_registration(child_registration).unwrap();
        let child_object = child.registration_result().object;
        assert_eq!(child_object.parent_interface_id, Some(parent_interface_id));
        assert_eq!(runtime.object(parent_object.id).unwrap().reference_count, 1);
        runtime.commit(child);
        assert_eq!(runtime.object(parent_object.id).unwrap().reference_count, 2);

        let release_external_parent = runtime.plan_release(parent_interface_id).unwrap();
        let external_result = release_external_parent.reference_result();
        assert_eq!(external_result.reference_count, 1);
        assert!(external_result.retirements.is_empty());
        runtime.commit(release_external_parent);

        let child_interface_id = child_object.interface_ids[1];
        let release_child = runtime.plan_release(child_interface_id).unwrap();
        let result = release_child.reference_result();
        assert_eq!(result.reference_count, 0);
        assert_eq!(
            result
                .retirements
                .iter()
                .map(|retirement| retirement.object_id)
                .collect::<Vec<_>>(),
            [child_object.id, parent_object.id]
        );
        runtime.commit(release_child);
        assert!(runtime.is_empty());
    }

    #[test]
    fn rejected_child_registration_does_not_increment_parent() {
        let mut runtime = RuntimeCom::default();
        let parent = runtime.plan_registration(registration()).unwrap();
        let parent_object = parent.registration_result().object;
        runtime.commit(parent);
        let before = runtime.clone();

        let mut child_registration = registration();
        child_registration.parent_interface_id = Some(parent_object.interface_ids[1]);
        assert_eq!(
            runtime
                .plan_registration(child_registration)
                .unwrap_err()
                .code,
            "com-interface-pointer-invalid"
        );
        assert_eq!(runtime, before);
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
            class_identity: "fixture.synthetic-com.v1".to_owned(),
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
                    endpoint: GuestAddress(base + 0x1000 + endpoint_offset + slot as u32 * 16),
                    explicit_argument_words: words,
                    return_kind,
                    implementation,
                }
            })
            .collect(),
        }
    }
}

use std::collections::BTreeMap;

use serde::Serialize;

use crate::dynamic_exports::{DYNAMIC_EXPORT_ENDPOINT_NAMESPACE, DYNAMIC_EXPORT_SLOT_SIZE};
use crate::memory::GuestAddress;

use super::{
    LOGICAL_ENCODED_POINTER_NAMESPACE, RuntimeAddressRange, RuntimeDynamicEndpoint,
    RuntimeEncodedPointer, RuntimeStateError,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeRegistries {
    dynamic_endpoints: BTreeMap<u32, RuntimeDynamicEndpoint>,
    encoded_pointers: BTreeMap<u32, RuntimeEncodedPointer>,
    #[serde(skip)]
    encoded_pointer_namespace: String,
    #[serde(skip)]
    encoded_pointer_token_prefix: u32,
    #[serde(skip)]
    encoded_pointer_sources: BTreeMap<u32, u32>,
    #[serde(skip)]
    next_encoded_pointer_id: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeDynamicEndpointPlan {
    endpoint: RuntimeDynamicEndpoint,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum RuntimeEncodedPointerPlan {
    Existing {
        token: u32,
    },
    Insert {
        previous_next_pointer_id: u16,
        next_pointer_id: u16,
        pointer: RuntimeEncodedPointer,
    },
}

impl RuntimeRegistries {
    pub(super) fn new(
        encoded_pointer_namespace: String,
        encoded_pointer_token_prefix: u32,
    ) -> Result<Self, RuntimeStateError> {
        if encoded_pointer_namespace != LOGICAL_ENCODED_POINTER_NAMESPACE
            || encoded_pointer_token_prefix == 0
            || encoded_pointer_token_prefix & 0xffff != 0
        {
            return Err(RuntimeStateError::invalid_input(
                "runtime-state",
                "encoded-pointer-policy-invalid",
                "encoded pointer policy requires the runtime namespace and a nonzero 16-bit-aligned token prefix",
            ));
        }
        Ok(Self {
            dynamic_endpoints: BTreeMap::new(),
            encoded_pointers: BTreeMap::new(),
            encoded_pointer_namespace,
            encoded_pointer_token_prefix,
            encoded_pointer_sources: BTreeMap::new(),
            next_encoded_pointer_id: 1,
        })
    }

    pub fn dynamic_endpoints(&self) -> &BTreeMap<u32, RuntimeDynamicEndpoint> {
        &self.dynamic_endpoints
    }

    pub fn encoded_pointers(&self) -> &BTreeMap<u32, RuntimeEncodedPointer> {
        &self.encoded_pointers
    }

    pub(super) fn encoded_pointer_namespace(&self) -> &str {
        &self.encoded_pointer_namespace
    }

    pub(super) const fn encoded_pointer_token_prefix(&self) -> u32 {
        self.encoded_pointer_token_prefix
    }

    pub(super) fn encoded_pointer_sources(&self) -> &BTreeMap<u32, u32> {
        &self.encoded_pointer_sources
    }

    pub(super) const fn next_encoded_pointer_id(&self) -> u16 {
        self.next_encoded_pointer_id
    }

    pub(super) fn plan_dynamic_endpoint(
        &self,
        dynamic_exports: RuntimeAddressRange,
        endpoint: RuntimeDynamicEndpoint,
    ) -> Result<RuntimeDynamicEndpointPlan, RuntimeStateError> {
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
        if self.dynamic_endpoints.contains_key(&endpoint.id)
            || self
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
        Ok(RuntimeDynamicEndpointPlan { endpoint })
    }

    pub(super) fn commit_dynamic_endpoint(
        &mut self,
        plan: RuntimeDynamicEndpointPlan,
    ) -> &RuntimeDynamicEndpoint {
        let endpoint = plan.endpoint;
        let id = endpoint.id;
        debug_assert!(!self.dynamic_endpoints.contains_key(&id));
        debug_assert!(
            !self
                .dynamic_endpoints
                .values()
                .any(|registered| registered.address == endpoint.address)
        );
        let previous = self.dynamic_endpoints.insert(id, endpoint);
        debug_assert!(previous.is_none());
        self.dynamic_endpoints
            .get(&id)
            .expect("committed dynamic endpoint disappeared")
    }

    pub(super) fn dynamic_endpoint(
        &self,
        id: u32,
    ) -> Result<&RuntimeDynamicEndpoint, RuntimeStateError> {
        self.dynamic_endpoints.get(&id).ok_or_else(|| {
            RuntimeStateError::not_found(
                "runtime-endpoint",
                "dynamic-endpoint-id-unknown",
                format!("dynamic endpoint ID {id} is not registered"),
            )
        })
    }

    pub(super) fn dynamic_endpoint_at(
        &self,
        address: GuestAddress,
    ) -> Result<&RuntimeDynamicEndpoint, RuntimeStateError> {
        self.dynamic_endpoints
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

    pub(super) fn dynamic_endpoint_by_name(
        &self,
        module: &str,
        name: &str,
    ) -> Result<&RuntimeDynamicEndpoint, RuntimeStateError> {
        self.dynamic_endpoints
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

    pub(super) fn plan_encode_pointer(
        &self,
        source: u32,
    ) -> Result<RuntimeEncodedPointerPlan, RuntimeStateError> {
        if let Some(encoded) = self.encoded_pointer_for_source_if_present(source)? {
            return Ok(RuntimeEncodedPointerPlan::Existing {
                token: encoded.token,
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
        if self.encoded_pointers.contains_key(&token) {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-pointer-encoding",
                "encoded-pointer-token-collision",
                "logical encoded pointer allocator selected a live token",
            ));
        }
        Ok(RuntimeEncodedPointerPlan::Insert {
            previous_next_pointer_id: token_id,
            next_pointer_id: token_id.checked_add(1).unwrap_or(0),
            pointer: RuntimeEncodedPointer {
                namespace: self.encoded_pointer_namespace.clone(),
                token_id,
                token,
                source,
            },
        })
    }

    pub(super) fn commit_encode_pointer(
        &mut self,
        plan: RuntimeEncodedPointerPlan,
    ) -> &RuntimeEncodedPointer {
        let token = match plan {
            RuntimeEncodedPointerPlan::Existing { token } => token,
            RuntimeEncodedPointerPlan::Insert {
                previous_next_pointer_id,
                next_pointer_id,
                pointer,
            } => {
                debug_assert_eq!(self.next_encoded_pointer_id, previous_next_pointer_id);
                debug_assert!(!self.encoded_pointer_sources.contains_key(&pointer.source));
                debug_assert!(!self.encoded_pointers.contains_key(&pointer.token));
                let token = pointer.token;
                let source = pointer.source;
                let previous_source = self.encoded_pointer_sources.insert(source, token);
                let previous_pointer = self.encoded_pointers.insert(token, pointer);
                debug_assert!(previous_source.is_none());
                debug_assert!(previous_pointer.is_none());
                self.next_encoded_pointer_id = next_pointer_id;
                token
            }
        };
        self.encoded_pointers
            .get(&token)
            .expect("planned encoded pointer disappeared before commit")
    }

    pub(super) fn encoded_pointer_for_source(
        &self,
        source: u32,
    ) -> Result<&RuntimeEncodedPointer, RuntimeStateError> {
        self.encoded_pointer_for_source_if_present(source)?
            .ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-pointer-encoding",
                    "encoded-pointer-source-unknown",
                    format!("guest pointer source {source:#010x} has not been encoded"),
                )
            })
    }

    fn encoded_pointer_for_source_if_present(
        &self,
        source: u32,
    ) -> Result<Option<&RuntimeEncodedPointer>, RuntimeStateError> {
        let Some(token) = self.encoded_pointer_sources.get(&source) else {
            return Ok(None);
        };
        let encoded = self.encoded_pointers.get(token).ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-pointer-encoding",
                "encoded-pointer-source-index-dangling",
                "encoded pointer source index references a missing token",
            )
        })?;
        if encoded.source != source || encoded.token != *token {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-pointer-encoding",
                "encoded-pointer-source-index-inconsistent",
                "encoded pointer source index does not match its token record",
            ));
        }
        Ok(Some(encoded))
    }

    pub(super) fn decode_pointer(
        &self,
        token: u32,
    ) -> Result<&RuntimeEncodedPointer, RuntimeStateError> {
        let token_id = token & 0xffff;
        if token_id == 0 || token & 0xffff_0000 != self.encoded_pointer_token_prefix {
            return Err(RuntimeStateError::invalid_input(
                "runtime-pointer-encoding",
                "encoded-pointer-token-invalid",
                format!("pointer token {token:#010x} is outside the runtime-owned token namespace"),
            ));
        }
        let encoded = self.encoded_pointers.get(&token).ok_or_else(|| {
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dynamic_exports::DYNAMIC_EXPORT_SLOT_SIZE;
    use crate::runtime::ProcessAddressSpaceLayout;

    fn registries() -> RuntimeRegistries {
        RuntimeRegistries::new(LOGICAL_ENCODED_POINTER_NAMESPACE.to_owned(), 0xec01_0000).unwrap()
    }

    fn endpoint(id: u32, name: &str) -> RuntimeDynamicEndpoint {
        let dynamic_exports = ProcessAddressSpaceLayout::vine_v1().dynamic_exports;
        RuntimeDynamicEndpoint {
            namespace: DYNAMIC_EXPORT_ENDPOINT_NAMESPACE.to_owned(),
            id,
            address: GuestAddress(dynamic_exports.base.0 + (id - 1) * DYNAMIC_EXPORT_SLOT_SIZE),
            module: "kernel32.dll".to_owned(),
            name: name.to_owned(),
            lifetime: "process-runtime".to_owned(),
        }
    }

    #[test]
    fn dynamic_endpoint_plan_is_mutation_free_and_commit_updates_all_lookups() {
        let mut registries = registries();
        let dynamic_exports = ProcessAddressSpaceLayout::vine_v1().dynamic_exports;
        let endpoint = endpoint(2, "FlsGetValue");
        let plan = registries
            .plan_dynamic_endpoint(dynamic_exports, endpoint.clone())
            .unwrap();
        assert!(registries.dynamic_endpoints().is_empty());

        assert_eq!(registries.commit_dynamic_endpoint(plan), &endpoint);
        assert_eq!(registries.dynamic_endpoint(2), Ok(&endpoint));
        assert_eq!(
            registries.dynamic_endpoint_at(endpoint.address),
            Ok(&endpoint)
        );
        assert_eq!(
            registries.dynamic_endpoint_by_name("KERNEL32.DLL", "FlsGetValue"),
            Ok(&endpoint)
        );
    }

    #[test]
    fn rejected_dynamic_endpoint_preserves_the_private_table() {
        let mut registries = registries();
        let dynamic_exports = ProcessAddressSpaceLayout::vine_v1().dynamic_exports;
        let endpoint = endpoint(1, "FlsAlloc");
        let plan = registries
            .plan_dynamic_endpoint(dynamic_exports, endpoint.clone())
            .unwrap();
        registries.commit_dynamic_endpoint(plan);
        let before = registries.clone();

        assert_eq!(
            registries
                .plan_dynamic_endpoint(dynamic_exports, endpoint)
                .unwrap_err()
                .code,
            "dynamic-endpoint-duplicate"
        );
        assert_eq!(registries, before);
    }

    #[test]
    fn encoded_pointer_plan_commits_both_indexes_and_reuses_identity() {
        let mut registries = registries();
        let plan = registries.plan_encode_pointer(0x7f02_0000).unwrap();
        assert!(registries.encoded_pointers().is_empty());
        assert!(registries.encoded_pointer_sources().is_empty());
        assert_eq!(registries.next_encoded_pointer_id(), 1);

        let encoded = registries.commit_encode_pointer(plan).clone();
        assert_eq!(encoded.token, 0xec01_0001);
        assert_eq!(registries.next_encoded_pointer_id(), 2);
        assert_eq!(
            registries.encoded_pointer_for_source(encoded.source),
            Ok(&encoded)
        );
        assert_eq!(registries.decode_pointer(encoded.token), Ok(&encoded));

        let reuse = registries.plan_encode_pointer(encoded.source).unwrap();
        let before = registries.clone();
        assert_eq!(registries.commit_encode_pointer(reuse), &encoded);
        assert_eq!(registries, before);
    }

    #[test]
    fn encoded_pointer_collision_and_exhaustion_are_mutation_free() {
        let mut collision = registries();
        collision.encoded_pointers.insert(
            0xec01_0001,
            RuntimeEncodedPointer {
                namespace: LOGICAL_ENCODED_POINTER_NAMESPACE.to_owned(),
                token_id: 1,
                token: 0xec01_0001,
                source: 7,
            },
        );
        let collision_before = collision.clone();
        assert_eq!(
            collision.plan_encode_pointer(8).unwrap_err().code,
            "encoded-pointer-token-collision"
        );
        assert_eq!(collision, collision_before);

        let mut exhausted = registries();
        exhausted.next_encoded_pointer_id = 0;
        let exhausted_before = exhausted.clone();
        assert_eq!(
            exhausted.plan_encode_pointer(8).unwrap_err().code,
            "encoded-pointer-token-space-exhausted"
        );
        assert_eq!(exhausted, exhausted_before);
    }

    #[test]
    fn serialization_exposes_only_compatible_public_registry_maps() {
        let registries = registries();
        assert_eq!(
            serde_json::to_value(registries).unwrap(),
            serde_json::json!({
                "dynamicEndpoints": {},
                "encodedPointers": {},
            })
        );
    }
}

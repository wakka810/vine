use std::collections::BTreeMap;

use serde::Serialize;

use crate::dynamic_exports::{
    DYNAMIC_EXPORT_ENDPOINT_NAMESPACE, DYNAMIC_EXPORT_SLOT_SIZE, DynamicExportThunk,
};
use crate::memory::GuestAddress;

use super::{RuntimeAddressRange, RuntimeStateError};

pub const LOGICAL_ENCODED_POINTER_NAMESPACE: &str = "vine.logical-encoded-pointer.v1";

/// The cookie bits that select the process permutation's rotation distance.
const ENCODED_POINTER_ROTATION_MASK: u32 = 0x1f;

const fn encoded_pointer_multiplier(cookie: u32) -> u32 {
    cookie.rotate_left(16) | 1
}

const fn encoded_pointer_addend(cookie: u32) -> u32 {
    cookie.rotate_right(7).wrapping_mul(0x9e37_79b9)
}

const fn inverse_odd_u32(value: u32) -> u32 {
    let mut inverse = value;
    let mut round = 0;
    while round < 5 {
        inverse = inverse.wrapping_mul(2_u32.wrapping_sub(value.wrapping_mul(inverse)));
        round += 1;
    }
    inverse
}

/// Obfuscates a guest pointer with a process-keyed permutation. The result is a bijection over
/// the entire 32-bit space, so distinct sources never share a token and no registration-order
/// prefix leaks through the encoded value.
const fn encode_with_cookie(source: u32, cookie: u32) -> u32 {
    (source ^ cookie)
        .wrapping_mul(encoded_pointer_multiplier(cookie))
        .wrapping_add(encoded_pointer_addend(cookie))
        .rotate_right(cookie & ENCODED_POINTER_ROTATION_MASK)
}

/// The exact inverse of [`encode_with_cookie`].
const fn decode_with_cookie(token: u32, cookie: u32) -> u32 {
    token
        .rotate_left(cookie & ENCODED_POINTER_ROTATION_MASK)
        .wrapping_sub(encoded_pointer_addend(cookie))
        .wrapping_mul(inverse_odd_u32(encoded_pointer_multiplier(cookie)))
        ^ cookie
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeDynamicEndpoint {
    pub namespace: String,
    pub id: u32,
    pub address: GuestAddress,
    pub module: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ordinal: Option<u16>,
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
            ordinal: None,
            lifetime: thunk.lifetime.to_owned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeEncodedPointer {
    pub namespace: String,
    /// Diagnostic registration order of this encoding. The token no longer derives from it.
    pub token_id: u64,
    pub token: u32,
    pub source: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeRegistries {
    dynamic_endpoints: BTreeMap<u32, RuntimeDynamicEndpoint>,
    encoded_pointers: BTreeMap<u32, RuntimeEncodedPointer>,
    #[serde(skip)]
    encoded_pointer_namespace: String,
    #[serde(skip)]
    encoded_pointer_cookie: u32,
    #[serde(skip)]
    encoded_pointer_sources: BTreeMap<u32, u32>,
    #[serde(skip)]
    next_encoded_pointer_id: u64,
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
        previous_next_pointer_id: u64,
        next_pointer_id: u64,
        pointer: RuntimeEncodedPointer,
    },
}

impl RuntimeRegistries {
    pub(super) fn new(
        encoded_pointer_namespace: String,
        encoded_pointer_cookie: u32,
    ) -> Result<Self, RuntimeStateError> {
        if encoded_pointer_namespace != LOGICAL_ENCODED_POINTER_NAMESPACE
            || encoded_pointer_cookie == 0
        {
            return Err(RuntimeStateError::invalid_input(
                "runtime-state",
                "encoded-pointer-policy-invalid",
                "encoded pointer policy requires the runtime namespace and a nonzero process cookie",
            ));
        }
        Ok(Self {
            dynamic_endpoints: BTreeMap::new(),
            encoded_pointers: BTreeMap::new(),
            encoded_pointer_namespace,
            encoded_pointer_cookie,
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

    pub(super) const fn encoded_pointer_cookie(&self) -> u32 {
        self.encoded_pointer_cookie
    }

    pub(super) fn encoded_pointer_sources(&self) -> &BTreeMap<u32, u32> {
        &self.encoded_pointer_sources
    }

    pub(super) const fn next_encoded_pointer_id(&self) -> u64 {
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
            || endpoint.ordinal == Some(0)
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
            || self.dynamic_endpoints.values().any(|registered| {
                registered.address == endpoint.address
                    || (registered.module.eq_ignore_ascii_case(&endpoint.module)
                        && (registered.name == endpoint.name
                            || (endpoint.ordinal.is_some()
                                && registered.ordinal == endpoint.ordinal)))
            })
        {
            return Err(RuntimeStateError::conflict(
                "runtime-endpoint",
                "dynamic-endpoint-duplicate",
                "dynamic endpoint ID, address, name, or ordinal is already registered",
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

    /// Resolves a runtime-provided export by ordinal.
    ///
    /// Resolves only an explicitly declared export ordinal. Endpoint registration order is an
    /// allocator detail and cannot stand in for a DLL export table.
    pub(super) fn dynamic_endpoint_by_ordinal(
        &self,
        module: &str,
        ordinal: u32,
    ) -> Result<&RuntimeDynamicEndpoint, RuntimeStateError> {
        self.dynamic_endpoints
            .values()
            .find(|endpoint| {
                endpoint.module.eq_ignore_ascii_case(module)
                    && endpoint.ordinal == u16::try_from(ordinal).ok()
            })
            .ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-endpoint",
                    "dynamic-endpoint-ordinal-unknown",
                    format!("dynamic endpoint {module}!#{ordinal} is not registered"),
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
        let token_id = self.next_encoded_pointer_id;
        let token = encode_with_cookie(source, self.encoded_pointer_cookie);
        if self.encoded_pointers.contains_key(&token) {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-pointer-encoding",
                "encoded-pointer-token-collision",
                "logical encoded pointer allocator selected a live token",
            ));
        }
        Ok(RuntimeEncodedPointerPlan::Insert {
            previous_next_pointer_id: token_id,
            next_pointer_id: token_id.checked_add(1).ok_or_else(|| {
                RuntimeStateError::resource_limit(
                    "runtime-pointer-encoding",
                    "encoded-pointer-diagnostic-id-space-exhausted",
                    "encoded pointer diagnostic registration ID space is exhausted",
                )
            })?,
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

    pub(super) fn decode_pointer(&self, token: u32) -> Result<u32, RuntimeStateError> {
        let source = decode_with_cookie(token, self.encoded_pointer_cookie);
        if let Some(encoded) = self.encoded_pointers.get(&token) {
            if encoded.namespace != self.encoded_pointer_namespace
                || encoded.token != token
                || encoded.source != source
            {
                return Err(RuntimeStateError::runtime_defect(
                    "runtime-pointer-encoding",
                    "encoded-pointer-record-inconsistent",
                    "registered encoded pointer does not match its process permutation",
                ));
            }
            match self.encoded_pointer_sources.get(&source) {
                Some(indexed_token) if *indexed_token == token => {}
                _ => {
                    return Err(RuntimeStateError::runtime_defect(
                        "runtime-pointer-encoding",
                        "encoded-pointer-source-index-inconsistent",
                        "registered encoded pointer does not match its process source index",
                    ));
                }
            }
        }
        Ok(source)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dynamic_exports::DYNAMIC_EXPORT_SLOT_SIZE;
    use crate::runtime::ProcessAddressSpaceLayout;

    const TEST_COOKIE: u32 = 0xec01_9d3b;

    fn registries() -> RuntimeRegistries {
        RuntimeRegistries::new(LOGICAL_ENCODED_POINTER_NAMESPACE.to_owned(), TEST_COOKIE).unwrap()
    }

    fn endpoint(id: u32, name: &str) -> RuntimeDynamicEndpoint {
        let dynamic_exports = ProcessAddressSpaceLayout::vine_v1().dynamic_exports;
        RuntimeDynamicEndpoint {
            namespace: DYNAMIC_EXPORT_ENDPOINT_NAMESPACE.to_owned(),
            id,
            address: GuestAddress(dynamic_exports.base.0 + (id - 1) * DYNAMIC_EXPORT_SLOT_SIZE),
            module: "kernel32.dll".to_owned(),
            name: name.to_owned(),
            ordinal: None,
            lifetime: "process-runtime".to_owned(),
        }
    }

    #[test]
    fn dynamic_endpoint_plan_is_mutation_free_and_commit_updates_all_lookups() {
        let mut registries = registries();
        let dynamic_exports = ProcessAddressSpaceLayout::vine_v1().dynamic_exports;
        let mut endpoint = endpoint(2, "FlsGetValue");
        endpoint.ordinal = Some(18);
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
        assert_eq!(
            registries.dynamic_endpoint_by_ordinal("KERNEL32.DLL", 18),
            Ok(&endpoint)
        );
        assert_eq!(
            registries
                .dynamic_endpoint_by_ordinal("kernel32.dll", 1)
                .unwrap_err()
                .code,
            "dynamic-endpoint-ordinal-unknown"
        );
    }

    #[test]
    fn rejected_dynamic_endpoint_preserves_the_private_table() {
        let mut registries = registries();
        let dynamic_exports = ProcessAddressSpaceLayout::vine_v1().dynamic_exports;
        let original = endpoint(1, "FlsAlloc");
        let plan = registries
            .plan_dynamic_endpoint(dynamic_exports, original.clone())
            .unwrap();
        registries.commit_dynamic_endpoint(plan);
        let before = registries.clone();

        assert_eq!(
            registries
                .plan_dynamic_endpoint(dynamic_exports, original)
                .unwrap_err()
                .code,
            "dynamic-endpoint-duplicate"
        );
        assert_eq!(registries, before);

        let mut duplicate_ordinal = endpoint(2, "FlsGetValue");
        duplicate_ordinal.ordinal = Some(11);
        let duplicate_ordinal_plan = registries
            .plan_dynamic_endpoint(dynamic_exports, duplicate_ordinal)
            .unwrap();
        registries.commit_dynamic_endpoint(duplicate_ordinal_plan);
        let mut conflicting = endpoint(3, "FlsSetValue");
        conflicting.ordinal = Some(11);
        let before_ordinal_conflict = registries.clone();
        assert_eq!(
            registries
                .plan_dynamic_endpoint(dynamic_exports, conflicting)
                .unwrap_err()
                .code,
            "dynamic-endpoint-duplicate"
        );
        assert_eq!(registries, before_ordinal_conflict);
    }

    #[test]
    fn encoded_pointer_plan_commits_both_indexes_and_reuses_identity() {
        let mut registries = registries();
        let plan = registries.plan_encode_pointer(0x7f02_0000).unwrap();
        assert!(registries.encoded_pointers().is_empty());
        assert!(registries.encoded_pointer_sources().is_empty());
        assert_eq!(registries.next_encoded_pointer_id(), 1);

        let encoded = registries.commit_encode_pointer(plan).clone();
        assert_eq!(encoded.token, encode_with_cookie(0x7f02_0000, TEST_COOKIE));
        assert_ne!(encoded.token, encode_with_cookie(0x7f02_0004, TEST_COOKIE));
        assert_eq!(decode_with_cookie(encoded.token, TEST_COOKIE), 0x7f02_0000);
        assert_eq!(registries.next_encoded_pointer_id(), 2);
        assert_eq!(
            registries.encoded_pointer_for_source(encoded.source),
            Ok(&encoded)
        );
        assert_eq!(registries.decode_pointer(encoded.token), Ok(encoded.source));
        let unregistered_source = 0x1234_5678;
        let unregistered_token = encode_with_cookie(unregistered_source, TEST_COOKIE);
        assert_eq!(
            registries.decode_pointer(unregistered_token),
            Ok(unregistered_source)
        );
        let other_process = RuntimeRegistries::new(
            LOGICAL_ENCODED_POINTER_NAMESPACE.to_owned(),
            TEST_COOKIE ^ 0x5a5a_a5a5,
        )
        .unwrap();
        assert_ne!(
            encoded.token,
            encode_with_cookie(encoded.source, other_process.encoded_pointer_cookie())
        );

        let reuse = registries.plan_encode_pointer(encoded.source).unwrap();
        let before = registries.clone();
        assert_eq!(registries.commit_encode_pointer(reuse), &encoded);
        assert_eq!(registries, before);
    }

    #[test]
    fn encoded_pointer_collision_and_diagnostic_exhaustion_are_mutation_free() {
        let mut collision = registries();
        let occupied_token = encode_with_cookie(8, TEST_COOKIE);
        collision.encoded_pointers.insert(
            occupied_token,
            RuntimeEncodedPointer {
                namespace: LOGICAL_ENCODED_POINTER_NAMESPACE.to_owned(),
                token_id: 1,
                token: occupied_token,
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
        exhausted.next_encoded_pointer_id = u64::MAX;
        let exhausted_before = exhausted.clone();
        assert_eq!(
            exhausted.plan_encode_pointer(8).unwrap_err().code,
            "encoded-pointer-diagnostic-id-space-exhausted"
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

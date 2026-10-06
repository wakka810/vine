use std::collections::BTreeMap;

use crate::dynamic_exports::{DYNAMIC_EXPORT_SLOT_SIZE, DynamicExportThunk};
use crate::memory::GuestAddress;

use super::{RuntimeAddressRange, RuntimeStateError};

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeDynamicEndpoint {
    pub id: u32,
    pub address: GuestAddress,
    pub module: String,
    pub name: String,
    pub ordinal: Option<u16>,
}

impl From<&DynamicExportThunk> for RuntimeDynamicEndpoint {
    fn from(thunk: &DynamicExportThunk) -> Self {
        Self {
            id: thunk.id.0,
            address: thunk.address,
            module: thunk.module.clone(),
            name: thunk.name.clone(),
            ordinal: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeRegistries {
    dynamic_endpoints: BTreeMap<u32, RuntimeDynamicEndpoint>,
    encoded_pointer_cookie: u32,
}

impl RuntimeRegistries {
    pub(super) fn new(encoded_pointer_cookie: u32) -> Result<Self, RuntimeStateError> {
        if encoded_pointer_cookie == 0 {
            return Err(RuntimeStateError::new(
                "encoded pointer permutation requires a nonzero process cookie",
            ));
        }
        Ok(Self {
            dynamic_endpoints: BTreeMap::new(),
            encoded_pointer_cookie,
        })
    }

    pub(super) fn register_dynamic_endpoint(
        &mut self,
        dynamic_exports: RuntimeAddressRange,
        endpoint: RuntimeDynamicEndpoint,
    ) -> Result<&RuntimeDynamicEndpoint, RuntimeStateError> {
        if endpoint.id == 0
            || endpoint.module.is_empty()
            || !endpoint.module.is_ascii()
            || endpoint.module.contains('\0')
            || endpoint.name.is_empty()
            || !endpoint.name.is_ascii()
            || endpoint.name.contains('\0')
            || endpoint.ordinal == Some(0)
        {
            return Err(RuntimeStateError::new(
                "dynamic endpoint requires a nonzero ID, NUL-free ASCII module and name, and valid ordinal",
            ));
        }
        let slot_offset = u64::from(endpoint.id - 1)
            .checked_mul(u64::from(DYNAMIC_EXPORT_SLOT_SIZE))
            .ok_or_else(|| {
                RuntimeStateError::new("dynamic endpoint slot offset overflowed runtime accounting")
            })?;
        let expected_address = u64::from(dynamic_exports.base.0)
            .checked_add(slot_offset)
            .filter(|_| slot_offset < dynamic_exports.byte_length)
            .and_then(|address| u32::try_from(address).ok())
            .ok_or_else(|| {
                RuntimeStateError::new(
                    "dynamic endpoint ID does not fit the process dynamic-export arena",
                )
            })?;
        if endpoint.address != GuestAddress(expected_address) {
            return Err(RuntimeStateError::new(
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
            return Err(RuntimeStateError::new(
                "dynamic endpoint ID, address, name, or ordinal is already registered",
            ));
        }
        let id = endpoint.id;
        self.dynamic_endpoints.insert(id, endpoint);
        Ok(&self.dynamic_endpoints[&id])
    }

    pub(super) fn dynamic_endpoint_by_name(
        &self,
        module: &str,
        name: &str,
    ) -> Option<&RuntimeDynamicEndpoint> {
        self.dynamic_endpoints
            .values()
            .find(|endpoint| endpoint.module.eq_ignore_ascii_case(module) && endpoint.name == name)
    }

    /// Resolves a runtime-provided export by ordinal.
    ///
    /// Resolves only an explicitly declared export ordinal. Endpoint registration order is an
    /// allocator detail and cannot stand in for a DLL export table.
    pub(super) fn dynamic_endpoint_by_ordinal(
        &self,
        module: &str,
        ordinal: u32,
    ) -> Option<&RuntimeDynamicEndpoint> {
        let ordinal = u16::try_from(ordinal).ok()?;
        self.dynamic_endpoints.values().find(|endpoint| {
            endpoint.module.eq_ignore_ascii_case(module) && endpoint.ordinal == Some(ordinal)
        })
    }

    pub(super) fn encode_pointer(&self, source: u32) -> u32 {
        encode_with_cookie(source, self.encoded_pointer_cookie)
    }

    pub(super) fn decode_pointer(&self, token: u32) -> u32 {
        decode_with_cookie(token, self.encoded_pointer_cookie)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dynamic_exports::{
        DYNAMIC_EXPORT_ARENA_BASE, DYNAMIC_EXPORT_ARENA_SIZE, DYNAMIC_EXPORT_SLOT_SIZE,
    };

    const TEST_COOKIE: u32 = 0xec01_9d3b;

    fn registries() -> RuntimeRegistries {
        RuntimeRegistries::new(TEST_COOKIE).unwrap()
    }

    fn endpoint(id: u32, name: &str) -> RuntimeDynamicEndpoint {
        let dynamic_exports =
            RuntimeAddressRange::new(DYNAMIC_EXPORT_ARENA_BASE, DYNAMIC_EXPORT_ARENA_SIZE).unwrap();
        RuntimeDynamicEndpoint {
            id,
            address: GuestAddress(dynamic_exports.base.0 + (id - 1) * DYNAMIC_EXPORT_SLOT_SIZE),
            module: "kernel32.dll".to_owned(),
            name: name.to_owned(),
            ordinal: None,
        }
    }

    #[test]
    fn dynamic_endpoint_registration_updates_all_lookups() {
        let mut registries = registries();
        let dynamic_exports =
            RuntimeAddressRange::new(DYNAMIC_EXPORT_ARENA_BASE, DYNAMIC_EXPORT_ARENA_SIZE).unwrap();
        let named_only = endpoint(3, "NamedOnly");
        let mut endpoint = endpoint(2, "FlsGetValue");
        endpoint.ordinal = Some(18);
        assert_eq!(
            registries.register_dynamic_endpoint(dynamic_exports, endpoint.clone()),
            Ok(&endpoint),
        );
        assert_eq!(registries.dynamic_endpoints.get(&2), Some(&endpoint));

        assert_eq!(
            registries.dynamic_endpoint_by_name("KERNEL32.DLL", "FlsGetValue"),
            Some(&endpoint)
        );
        assert_eq!(
            registries.dynamic_endpoint_by_ordinal("KERNEL32.DLL", 18),
            Some(&endpoint)
        );
        assert!(
            registries
                .dynamic_endpoint_by_name("kernel32.dll", "Missing")
                .is_none()
        );
        assert!(
            registries
                .dynamic_endpoint_by_name("user32.dll", "FlsGetValue")
                .is_none()
        );
        assert!(
            registries
                .dynamic_endpoint_by_ordinal("kernel32.dll", 1)
                .is_none()
        );
        registries
            .register_dynamic_endpoint(dynamic_exports, named_only)
            .unwrap();
        for ordinal in [u32::from(u16::MAX) + 1, u32::MAX] {
            assert!(
                registries
                    .dynamic_endpoint_by_ordinal("kernel32.dll", ordinal)
                    .is_none()
            );
        }
    }

    #[test]
    fn rejected_dynamic_endpoint_preserves_the_private_table() {
        let mut registries = registries();
        let dynamic_exports =
            RuntimeAddressRange::new(DYNAMIC_EXPORT_ARENA_BASE, DYNAMIC_EXPORT_ARENA_SIZE).unwrap();
        let original = endpoint(1, "FlsAlloc");
        registries
            .register_dynamic_endpoint(dynamic_exports, original.clone())
            .unwrap();
        let before = registries.clone();

        registries
            .register_dynamic_endpoint(dynamic_exports, original)
            .unwrap_err();
        assert_eq!(registries, before);

        let mut duplicate_ordinal = endpoint(2, "FlsGetValue");
        duplicate_ordinal.ordinal = Some(11);
        registries
            .register_dynamic_endpoint(dynamic_exports, duplicate_ordinal)
            .unwrap();
        let mut conflicting = endpoint(3, "FlsSetValue");
        conflicting.ordinal = Some(11);
        let before_ordinal_conflict = registries.clone();
        registries
            .register_dynamic_endpoint(dynamic_exports, conflicting)
            .unwrap_err();
        assert_eq!(registries, before_ordinal_conflict);

        let mut zero_id = endpoint(4, "ZeroId");
        zero_id.id = 0;
        let mut invalid_ordinal = endpoint(4, "ZeroOrdinal");
        invalid_ordinal.ordinal = Some(0);
        let mut invalid_module = endpoint(4, "BadModule");
        invalid_module.module = "kernel32.dll\0other.dll".to_owned();
        for bad in [
            zero_id,
            invalid_ordinal,
            invalid_module,
            endpoint(4, "Bad\0Name"),
            endpoint(4, "NonAscii名"),
            endpoint(4097, "OutsideArena"),
        ] {
            let before = registries.clone();
            registries
                .register_dynamic_endpoint(dynamic_exports, bad)
                .unwrap_err();
            assert_eq!(registries, before);
        }
        let mut wrong_slot = endpoint(4, "WrongSlot");
        wrong_slot.address = GuestAddress(dynamic_exports.base.0);
        let before = registries.clone();
        registries
            .register_dynamic_endpoint(dynamic_exports, wrong_slot)
            .unwrap_err();
        assert_eq!(registries, before);
    }

    #[test]
    fn pointer_encoding_is_a_pure_process_keyed_bijection() {
        let registries = registries();
        let other_process = RuntimeRegistries::new(TEST_COOKIE ^ 0x5a5a_a5a5).unwrap();
        let before = registries.clone();
        let sources = [0, 1, 8, 0x1234_5678, 0x7f02_0000, u32::MAX];
        for source in sources {
            let token = registries.encode_pointer(source);
            assert_eq!(registries.encode_pointer(source), token);
            assert_eq!(registries.decode_pointer(token), source);
            assert_ne!(token, other_process.encode_pointer(source));
        }
        for source in sources {
            for other in sources {
                if source != other {
                    assert_ne!(
                        registries.encode_pointer(source),
                        registries.encode_pointer(other)
                    );
                }
            }
        }
        assert_eq!(registries, before);
        assert_eq!(
            registries.decode_pointer(0xdead_beef),
            decode_with_cookie(0xdead_beef, TEST_COOKIE)
        );
    }
}

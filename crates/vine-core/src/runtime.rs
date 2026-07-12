use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;

use serde::Serialize;

use crate::callbacks::{CALLBACK_RETURN_ARENA_BASE, CALLBACK_RETURN_ARENA_SIZE};
use crate::dynamic_exports::{
    DYNAMIC_EXPORT_ARENA_BASE, DYNAMIC_EXPORT_ARENA_SIZE, DYNAMIC_EXPORT_ENDPOINT_NAMESPACE,
    DYNAMIC_EXPORT_SLOT_SIZE, DynamicExportThunk,
};
use crate::imports::{IMPORT_THUNK_ARENA_BASE, IMPORT_THUNK_ARENA_SIZE};
use crate::memory::GuestAddress;

pub const LOGICAL_PRIVATE_HEAP_NAMESPACE: &str = "vine.logical-private-heap.v1";
pub const LOGICAL_LOADED_MODULE_NAMESPACE: &str = "vine.logical-loaded-module.v1";
pub const LOGICAL_TLS_INDEX_NAMESPACE: &str = "vine.logical-tls-index.v1";
pub const LOGICAL_ENCODED_POINTER_NAMESPACE: &str = "vine.logical-encoded-pointer.v1";
pub const TLS_OUT_OF_INDEXES: u32 = u32::MAX;
pub const X86_STARTUP_INFO_W_BYTE_LENGTH: usize = 68;

const LOGICAL_PRIVATE_HEAP_HANDLE_PREFIX: u32 = 0x6801_0000;
const RUNTIME_PRIVATE_HEAP_OWNER: &str = "runtime-process-private-heaps";
const RUNTIME_PRIVATE_HEAP_VARIANT: &str = "heap-create-metadata-v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct RuntimeProcessId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct RuntimeThreadId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct RuntimeCallbackFrameId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeAddressRange {
    pub base: GuestAddress,
    pub byte_length: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessAddressSpaceLayout {
    pub import_thunks: RuntimeAddressRange,
    pub callback_returns: RuntimeAddressRange,
    pub dynamic_exports: RuntimeAddressRange,
}

impl ProcessAddressSpaceLayout {
    pub fn vine_v1() -> Self {
        Self {
            import_thunks: RuntimeAddressRange {
                base: IMPORT_THUNK_ARENA_BASE,
                byte_length: IMPORT_THUNK_ARENA_SIZE,
            },
            callback_returns: RuntimeAddressRange {
                base: CALLBACK_RETURN_ARENA_BASE,
                byte_length: CALLBACK_RETURN_ARENA_SIZE,
            },
            dynamic_exports: RuntimeAddressRange {
                base: DYNAMIC_EXPORT_ARENA_BASE,
                byte_length: DYNAMIC_EXPORT_ARENA_SIZE,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeClockState {
    pub wall_clock_file_time: u64,
    pub tick_count: u32,
    pub performance_counter: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeStartupInfo {
    pub profile_id: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeBootstrap {
    pub process_id: RuntimeProcessId,
    pub initial_thread_id: RuntimeThreadId,
    pub initial_last_error: u32,
    pub address_space: ProcessAddressSpaceLayout,
    pub clock: RuntimeClockState,
    pub startup_info: RuntimeStartupInfo,
    pub encoded_pointer_namespace: String,
    pub encoded_pointer_token_prefix: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeHeap {
    pub owner: String,
    pub variant_id: String,
    pub namespace: String,
    pub handle: u32,
    pub creation_flags: u32,
    pub initial_size: u32,
    pub maximum_size: u32,
    pub growable: bool,
    pub live_allocations: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeModule {
    pub owner: String,
    pub variant_id: String,
    pub namespace: String,
    pub handle: u32,
    pub requested_name: String,
    pub canonical_name: String,
    pub runtime_system_module: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeDynamicEndpoint {
    pub namespace: String,
    pub id: u32,
    pub address: GuestAddress,
    pub module: String,
    pub name: String,
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
            lifetime: thunk.lifetime.to_owned(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum RuntimeTlsValue {
    Raw {
        value: u32,
    },
    DynamicEndpoint {
        endpoint_id: u32,
        address: GuestAddress,
    },
}

impl RuntimeTlsValue {
    pub fn raw_value(&self) -> u32 {
        match self {
            Self::Raw { value } => *value,
            Self::DynamicEndpoint { address, .. } => address.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeEncodedPointer {
    pub namespace: String,
    pub token_id: u16,
    pub token: u32,
    pub source: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeCallbackFrame {
    pub id: RuntimeCallbackFrameId,
    pub thread_id: RuntimeThreadId,
    pub target: GuestAddress,
    pub return_sentinel: GuestAddress,
    pub entry_last_error: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum SchedulerThreadState {
    Runnable,
    WaitingHost { request_id: u64 },
    WaitingObject { object: u32 },
    InCallback { depth: usize },
    Terminated { exit_code: u32 },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeHostRequest {
    pub request_id: u64,
    pub thread_id: RuntimeThreadId,
    pub service: String,
    pub resume_state: SchedulerThreadState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeHostCompletion {
    pub request_id: u64,
    pub thread_id: RuntimeThreadId,
    pub service: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeCancellation {
    pub thread_id: RuntimeThreadId,
    pub exit_code: u32,
    pub host_requests_cancelled: usize,
    pub callback_frames_cancelled: usize,
    pub scheduler_events_discarded: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "event", content = "completion", rename_all = "kebab-case")]
pub enum RuntimeSchedulerEvent {
    HostCompleted(RuntimeHostCompletion),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeThreadState {
    pub id: RuntimeThreadId,
    pub last_error: u32,
    pub tls_values: BTreeMap<u32, RuntimeTlsValue>,
    pub callback_frames: Vec<RuntimeCallbackFrame>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeProcessState {
    pub id: RuntimeProcessId,
    pub address_space: ProcessAddressSpaceLayout,
    pub clock: RuntimeClockState,
    pub startup_info: RuntimeStartupInfo,
    pub heap_termination_on_corruption: bool,
    pub heaps: BTreeMap<u32, RuntimeHeap>,
    pub modules: BTreeMap<u32, RuntimeModule>,
    pub allocated_tls_indices: BTreeSet<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeScheduler {
    pub current_thread_id: RuntimeThreadId,
    pub threads: BTreeMap<RuntimeThreadId, SchedulerThreadState>,
    pub events: VecDeque<RuntimeSchedulerEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeRegistries {
    pub dynamic_endpoints: BTreeMap<u32, RuntimeDynamicEndpoint>,
    pub encoded_pointers: BTreeMap<u32, RuntimeEncodedPointer>,
    pub host_requests: BTreeMap<u64, RuntimeHostRequest>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeState {
    process: RuntimeProcessState,
    threads: BTreeMap<RuntimeThreadId, RuntimeThreadState>,
    scheduler: RuntimeScheduler,
    registries: RuntimeRegistries,
    encoded_pointer_namespace: String,
    encoded_pointer_token_prefix: u32,
    encoded_pointer_sources: BTreeMap<u32, u32>,
    next_encoded_pointer_id: u16,
    next_private_heap_slot: u16,
    next_tls_index: u32,
    next_callback_frame_id: u64,
    next_host_request_id: u64,
}

impl RuntimeState {
    pub fn new(bootstrap: RuntimeBootstrap) -> Result<Self, RuntimeStateError> {
        if bootstrap.process_id.0 == 0 || bootstrap.initial_thread_id.0 == 0 {
            return Err(RuntimeStateError::invalid_input(
                "runtime-state",
                "zero-process-or-thread-id",
                "runtime process and initial thread IDs must be nonzero",
            ));
        }
        if bootstrap.startup_info.profile_id.is_empty()
            || bootstrap.startup_info.bytes.len() != X86_STARTUP_INFO_W_BYTE_LENGTH
        {
            return Err(RuntimeStateError::invalid_input(
                "runtime-state",
                "startup-info-profile-invalid",
                "runtime startup profile requires a nonempty identity and one x86 STARTUPINFOW image",
            ));
        }
        validate_address_space(bootstrap.address_space)?;
        if bootstrap.encoded_pointer_namespace != LOGICAL_ENCODED_POINTER_NAMESPACE
            || bootstrap.encoded_pointer_token_prefix == 0
            || bootstrap.encoded_pointer_token_prefix & 0xffff != 0
        {
            return Err(RuntimeStateError::invalid_input(
                "runtime-state",
                "encoded-pointer-policy-invalid",
                "encoded pointer policy requires the runtime namespace and a nonzero 16-bit-aligned token prefix",
            ));
        }

        let initial_thread = RuntimeThreadState {
            id: bootstrap.initial_thread_id,
            last_error: bootstrap.initial_last_error,
            tls_values: BTreeMap::new(),
            callback_frames: Vec::new(),
        };
        Ok(Self {
            process: RuntimeProcessState {
                id: bootstrap.process_id,
                address_space: bootstrap.address_space,
                clock: bootstrap.clock,
                startup_info: bootstrap.startup_info,
                heap_termination_on_corruption: false,
                heaps: BTreeMap::new(),
                modules: BTreeMap::new(),
                allocated_tls_indices: BTreeSet::new(),
            },
            threads: BTreeMap::from([(bootstrap.initial_thread_id, initial_thread)]),
            scheduler: RuntimeScheduler {
                current_thread_id: bootstrap.initial_thread_id,
                threads: BTreeMap::from([(
                    bootstrap.initial_thread_id,
                    SchedulerThreadState::Runnable,
                )]),
                events: VecDeque::new(),
            },
            registries: RuntimeRegistries {
                dynamic_endpoints: BTreeMap::new(),
                encoded_pointers: BTreeMap::new(),
                host_requests: BTreeMap::new(),
            },
            encoded_pointer_namespace: bootstrap.encoded_pointer_namespace,
            encoded_pointer_token_prefix: bootstrap.encoded_pointer_token_prefix,
            encoded_pointer_sources: BTreeMap::new(),
            next_encoded_pointer_id: 1,
            next_private_heap_slot: 1,
            next_tls_index: 0,
            next_callback_frame_id: 1,
            next_host_request_id: 1,
        })
    }

    pub fn process_id(&self) -> RuntimeProcessId {
        self.process.id
    }

    pub fn process(&self) -> &RuntimeProcessState {
        &self.process
    }

    pub fn current_thread_id(&self) -> RuntimeThreadId {
        self.scheduler.current_thread_id
    }

    pub fn current_thread_state(&self) -> Result<&RuntimeThreadState, RuntimeStateError> {
        self.current_thread()
    }

    pub fn scheduler(&self) -> &RuntimeScheduler {
        &self.scheduler
    }

    pub fn registries(&self) -> &RuntimeRegistries {
        &self.registries
    }

    pub fn clock(&self) -> &RuntimeClockState {
        &self.process.clock
    }

    pub fn startup_info(&self) -> &RuntimeStartupInfo {
        &self.process.startup_info
    }

    pub fn current_last_error(&self) -> Result<u32, RuntimeStateError> {
        Ok(self.current_thread()?.last_error)
    }

    pub fn set_current_last_error(&mut self, value: u32) -> Result<(), RuntimeStateError> {
        self.current_thread_mut()?.last_error = value;
        Ok(())
    }

    pub fn scheduler_state(&self) -> Result<SchedulerThreadState, RuntimeStateError> {
        self.scheduler
            .threads
            .get(&self.scheduler.current_thread_id)
            .copied()
            .ok_or_else(|| {
                RuntimeStateError::runtime_defect(
                    "runtime-scheduler",
                    "current-thread-scheduler-state-missing",
                    "current runtime thread has no scheduler state",
                )
            })
    }

    pub fn set_heap_termination_on_corruption(&mut self, enabled: bool) {
        self.process.heap_termination_on_corruption = enabled;
    }

    pub fn create_heap(&mut self, heap: RuntimeHeap) -> Result<&RuntimeHeap, RuntimeStateError> {
        if heap.owner.is_empty()
            || heap.variant_id.is_empty()
            || heap.namespace != LOGICAL_PRIVATE_HEAP_NAMESPACE
            || heap.handle == 0
        {
            return Err(RuntimeStateError::invalid_input(
                "runtime-heap",
                "heap-identity-invalid",
                "runtime heap requires controlled owner, variant, namespace, and nonzero handle",
            ));
        }
        if self.process.heaps.contains_key(&heap.handle) {
            return Err(RuntimeStateError::conflict(
                "runtime-heap",
                "heap-handle-duplicate",
                "runtime heap handle is already live",
            ));
        }
        let handle = heap.handle;
        self.process.heaps.insert(handle, heap);
        self.process.heaps.get(&handle).ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-heap",
                "inserted-heap-missing",
                "runtime heap disappeared after insertion",
            )
        })
    }

    pub fn allocate_private_heap(
        &mut self,
        creation_flags: u32,
        initial_size: u32,
        maximum_size: u32,
    ) -> Result<&RuntimeHeap, RuntimeStateError> {
        loop {
            let slot = self.next_private_heap_slot;
            if slot == 0 {
                return Err(RuntimeStateError::resource_limit(
                    "runtime-heap",
                    "private-heap-handle-space-exhausted",
                    "logical private heap handle space is exhausted",
                ));
            }
            self.next_private_heap_slot = self.next_private_heap_slot.checked_add(1).unwrap_or(0);
            let handle = LOGICAL_PRIVATE_HEAP_HANDLE_PREFIX | u32::from(slot);
            if self.process.heaps.contains_key(&handle) {
                continue;
            }
            return self.create_heap(RuntimeHeap {
                owner: RUNTIME_PRIVATE_HEAP_OWNER.to_owned(),
                variant_id: RUNTIME_PRIVATE_HEAP_VARIANT.to_owned(),
                namespace: LOGICAL_PRIVATE_HEAP_NAMESPACE.to_owned(),
                handle,
                creation_flags,
                initial_size,
                maximum_size,
                growable: maximum_size == 0,
                live_allocations: 0,
            });
        }
    }

    pub fn heap(&self, handle: u32) -> Result<&RuntimeHeap, RuntimeStateError> {
        self.process.heaps.get(&handle).ok_or_else(|| {
            RuntimeStateError::not_found(
                "runtime-heap",
                "heap-handle-unknown",
                format!("logical heap handle {handle:#010x} is not live"),
            )
        })
    }

    pub fn load_module(
        &mut self,
        module: RuntimeModule,
    ) -> Result<&RuntimeModule, RuntimeStateError> {
        if module.owner.is_empty()
            || module.variant_id.is_empty()
            || module.namespace != LOGICAL_LOADED_MODULE_NAMESPACE
            || module.handle == 0
            || module.canonical_name.is_empty()
        {
            return Err(RuntimeStateError::invalid_input(
                "runtime-module",
                "module-identity-invalid",
                "runtime module requires controlled owner, variant, namespace, handle, and name",
            ));
        }
        if self.process.modules.contains_key(&module.handle)
            || self.process.modules.values().any(|loaded| {
                loaded
                    .canonical_name
                    .eq_ignore_ascii_case(&module.canonical_name)
            })
        {
            return Err(RuntimeStateError::conflict(
                "runtime-module",
                "module-identity-duplicate",
                "runtime module handle or canonical name is already loaded",
            ));
        }
        let handle = module.handle;
        self.process.modules.insert(handle, module);
        self.process.modules.get(&handle).ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-module",
                "inserted-module-missing",
                "runtime module disappeared after insertion",
            )
        })
    }

    pub fn module(&self, handle: u32) -> Result<&RuntimeModule, RuntimeStateError> {
        self.process.modules.get(&handle).ok_or_else(|| {
            RuntimeStateError::not_found(
                "runtime-module",
                "module-handle-unknown",
                format!("logical module handle {handle:#010x} is not loaded"),
            )
        })
    }

    pub fn module_by_name(&self, name: &str) -> Result<&RuntimeModule, RuntimeStateError> {
        self.process
            .modules
            .values()
            .find(|module| module.canonical_name.eq_ignore_ascii_case(name))
            .ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-module",
                    "module-name-unknown",
                    format!("module {name:?} is not loaded"),
                )
            })
    }

    pub fn register_dynamic_endpoint(
        &mut self,
        endpoint: RuntimeDynamicEndpoint,
    ) -> Result<&RuntimeDynamicEndpoint, RuntimeStateError> {
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
        let dynamic_exports = self.process.address_space.dynamic_exports;
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
        if self.registries.dynamic_endpoints.contains_key(&endpoint.id)
            || self
                .registries
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
        let id = endpoint.id;
        self.registries.dynamic_endpoints.insert(id, endpoint);
        self.registries.dynamic_endpoints.get(&id).ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-endpoint",
                "inserted-dynamic-endpoint-missing",
                "dynamic endpoint disappeared after insertion",
            )
        })
    }

    pub fn register_dynamic_export_thunk(
        &mut self,
        thunk: &DynamicExportThunk,
    ) -> Result<&RuntimeDynamicEndpoint, RuntimeStateError> {
        self.register_dynamic_endpoint(RuntimeDynamicEndpoint::from(thunk))
    }

    pub fn dynamic_endpoint(&self, id: u32) -> Result<&RuntimeDynamicEndpoint, RuntimeStateError> {
        self.registries.dynamic_endpoints.get(&id).ok_or_else(|| {
            RuntimeStateError::not_found(
                "runtime-endpoint",
                "dynamic-endpoint-id-unknown",
                format!("dynamic endpoint ID {id} is not registered"),
            )
        })
    }

    pub fn dynamic_endpoint_at(
        &self,
        address: GuestAddress,
    ) -> Result<&RuntimeDynamicEndpoint, RuntimeStateError> {
        self.registries
            .dynamic_endpoints
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

    pub fn dynamic_endpoint_by_name(
        &self,
        module: &str,
        name: &str,
    ) -> Result<&RuntimeDynamicEndpoint, RuntimeStateError> {
        self.registries
            .dynamic_endpoints
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

    pub fn allocate_tls_index(&mut self) -> Result<u32, RuntimeStateError> {
        if self.next_tls_index == TLS_OUT_OF_INDEXES {
            return Err(RuntimeStateError::resource_limit(
                "runtime-tls",
                "tls-index-space-exhausted",
                "logical TLS index space is exhausted",
            ));
        }
        let index = self.next_tls_index;
        if !self.process.allocated_tls_indices.insert(index) {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-tls",
                "tls-index-allocator-collision",
                "logical TLS allocator selected an already allocated index",
            ));
        }
        self.next_tls_index = self.next_tls_index.checked_add(1).ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-tls",
                "tls-index-counter-overflow",
                "logical TLS index counter overflowed",
            )
        })?;
        Ok(index)
    }

    pub fn set_tls_value(&mut self, index: u32, value: u32) -> Result<(), RuntimeStateError> {
        if !self.process.allocated_tls_indices.contains(&index) {
            return Err(RuntimeStateError::not_found(
                "runtime-tls",
                "tls-index-unallocated",
                format!("logical TLS index {index} is not allocated"),
            ));
        }
        let tls_value = self
            .dynamic_endpoint_at(GuestAddress(value))
            .map(|endpoint| RuntimeTlsValue::DynamicEndpoint {
                endpoint_id: endpoint.id,
                address: endpoint.address,
            })
            .unwrap_or(RuntimeTlsValue::Raw { value });
        self.current_thread_mut()?
            .tls_values
            .insert(index, tls_value);
        Ok(())
    }

    pub fn tls_value(&self, index: u32) -> Result<&RuntimeTlsValue, RuntimeStateError> {
        self.current_thread()?
            .tls_values
            .get(&index)
            .ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-tls",
                    "tls-value-uninitialized",
                    format!("current thread has no value for logical TLS index {index}"),
                )
            })
    }

    pub fn initialized_tls_thread_count(&self, index: u32) -> usize {
        self.threads
            .values()
            .filter(|thread| thread.tls_values.contains_key(&index))
            .count()
    }

    pub fn encoded_pointer_namespace(&self) -> &str {
        &self.encoded_pointer_namespace
    }

    pub fn encode_pointer(
        &mut self,
        source: u32,
    ) -> Result<&RuntimeEncodedPointer, RuntimeStateError> {
        if let Some(token) = self.encoded_pointer_sources.get(&source) {
            return self.registries.encoded_pointers.get(token).ok_or_else(|| {
                RuntimeStateError::runtime_defect(
                    "runtime-pointer-encoding",
                    "encoded-pointer-source-index-dangling",
                    "encoded pointer source index references a missing token",
                )
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
        if self.registries.encoded_pointers.contains_key(&token) {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-pointer-encoding",
                "encoded-pointer-token-collision",
                "logical encoded pointer allocator selected a live token",
            ));
        }
        self.next_encoded_pointer_id = self.next_encoded_pointer_id.checked_add(1).unwrap_or(0);
        self.encoded_pointer_sources.insert(source, token);
        self.registries.encoded_pointers.insert(
            token,
            RuntimeEncodedPointer {
                namespace: self.encoded_pointer_namespace.clone(),
                token_id,
                token,
                source,
            },
        );
        self.registries.encoded_pointers.get(&token).ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-pointer-encoding",
                "inserted-encoded-pointer-missing",
                "logical encoded pointer disappeared after insertion",
            )
        })
    }

    pub fn encoded_pointer_for_source(
        &self,
        source: u32,
    ) -> Result<&RuntimeEncodedPointer, RuntimeStateError> {
        let token = self.encoded_pointer_sources.get(&source).ok_or_else(|| {
            RuntimeStateError::not_found(
                "runtime-pointer-encoding",
                "encoded-pointer-source-unknown",
                format!("guest pointer source {source:#010x} has not been encoded"),
            )
        })?;
        self.registries.encoded_pointers.get(token).ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-pointer-encoding",
                "encoded-pointer-source-index-dangling",
                "encoded pointer source index references a missing token",
            )
        })
    }

    pub fn push_callback_frame(
        &mut self,
        target: GuestAddress,
        return_sentinel: GuestAddress,
    ) -> Result<RuntimeCallbackFrame, RuntimeStateError> {
        let current_depth = self.current_thread()?.callback_frames.len();
        match self.scheduler_state()? {
            SchedulerThreadState::Runnable if current_depth == 0 => {}
            SchedulerThreadState::InCallback { depth } if depth == current_depth => {}
            _ => {
                return Err(RuntimeStateError::conflict(
                    "runtime-callback",
                    "callback-entry-thread-state-invalid",
                    "callback entry requires a runnable thread or the matching nested callback state",
                ));
            }
        }
        let id = RuntimeCallbackFrameId(self.next_callback_frame_id);
        self.next_callback_frame_id =
            self.next_callback_frame_id.checked_add(1).ok_or_else(|| {
                RuntimeStateError::resource_limit(
                    "runtime-callback",
                    "callback-frame-id-overflow",
                    "callback frame ID space is exhausted",
                )
            })?;
        let frame = RuntimeCallbackFrame {
            id,
            thread_id: self.current_thread_id(),
            target,
            return_sentinel,
            entry_last_error: self.current_last_error()?,
        };
        let depth = {
            let thread = self.current_thread_mut()?;
            thread.callback_frames.push(frame);
            thread.callback_frames.len()
        };
        self.set_scheduler_state(SchedulerThreadState::InCallback { depth })?;
        Ok(frame)
    }

    pub fn pop_callback_frame(
        &mut self,
        expected: RuntimeCallbackFrameId,
    ) -> Result<RuntimeCallbackFrame, RuntimeStateError> {
        let current_depth = self.current_thread()?.callback_frames.len();
        if self.scheduler_state()?
            != (SchedulerThreadState::InCallback {
                depth: current_depth,
            })
        {
            return Err(RuntimeStateError::conflict(
                "runtime-callback",
                "callback-return-thread-state-invalid",
                "callback return requires the scheduler to identify the current callback depth",
            ));
        }
        let (frame, remaining_depth) = {
            let thread = self.current_thread_mut()?;
            let frame = thread.callback_frames.last().copied().ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-callback",
                    "callback-frame-stack-empty",
                    "current thread has no callback frame to return",
                )
            })?;
            if frame.id != expected {
                return Err(RuntimeStateError::conflict(
                    "runtime-callback",
                    "callback-frame-return-order-mismatch",
                    "callback frame return is not strict LIFO order",
                ));
            }
            thread.callback_frames.pop();
            (frame, thread.callback_frames.len())
        };
        let scheduler_state = if remaining_depth == 0 {
            SchedulerThreadState::Runnable
        } else {
            SchedulerThreadState::InCallback {
                depth: remaining_depth,
            }
        };
        self.set_scheduler_state(scheduler_state)?;
        Ok(frame)
    }

    pub fn begin_host_request(
        &mut self,
        service: impl Into<String>,
    ) -> Result<RuntimeHostRequest, RuntimeStateError> {
        let service = service.into();
        if service.is_empty() {
            return Err(RuntimeStateError::invalid_input(
                "runtime-host-request",
                "host-service-empty",
                "host request requires a nonempty service identity",
            ));
        }
        let resume_state = match self.scheduler_state()? {
            state @ (SchedulerThreadState::Runnable | SchedulerThreadState::InCallback { .. }) => {
                state
            }
            _ => {
                return Err(RuntimeStateError::conflict(
                    "runtime-host-request",
                    "thread-not-runnable",
                    "only a runnable guest thread or active callback can begin a host request",
                ));
            }
        };
        let request_id = self.next_host_request_id;
        self.next_host_request_id = self.next_host_request_id.checked_add(1).ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-host-request",
                "host-request-id-overflow",
                "host request ID space is exhausted",
            )
        })?;
        let request = RuntimeHostRequest {
            request_id,
            thread_id: self.current_thread_id(),
            service,
            resume_state,
        };
        self.registries
            .host_requests
            .insert(request_id, request.clone());
        self.set_scheduler_state(SchedulerThreadState::WaitingHost { request_id })?;
        Ok(request)
    }

    pub fn complete_host_request(
        &mut self,
        request_id: u64,
    ) -> Result<RuntimeHostCompletion, RuntimeStateError> {
        let request = self
            .registries
            .host_requests
            .remove(&request_id)
            .ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-host-request",
                    "host-request-id-unknown",
                    format!("host request ID {request_id} is not pending"),
                )
            })?;
        if self.scheduler_state()? != (SchedulerThreadState::WaitingHost { request_id }) {
            self.registries.host_requests.insert(request_id, request);
            return Err(RuntimeStateError::conflict(
                "runtime-host-request",
                "host-completion-thread-state-mismatch",
                "host completion does not match the current thread wait state",
            ));
        }
        if request.thread_id != self.current_thread_id() {
            self.registries.host_requests.insert(request_id, request);
            return Err(RuntimeStateError::conflict(
                "runtime-host-request",
                "host-completion-thread-mismatch",
                "host completion belongs to a different guest thread",
            ));
        }
        self.set_scheduler_state(request.resume_state)?;
        let completion = RuntimeHostCompletion {
            request_id,
            thread_id: request.thread_id,
            service: request.service,
        };
        self.scheduler
            .events
            .push_back(RuntimeSchedulerEvent::HostCompleted(completion.clone()));
        Ok(completion)
    }

    pub fn pop_scheduler_event(&mut self) -> Option<RuntimeSchedulerEvent> {
        self.scheduler.events.pop_front()
    }

    pub fn cancel_current_thread(
        &mut self,
        exit_code: u32,
    ) -> Result<RuntimeCancellation, RuntimeStateError> {
        let thread_id = self.current_thread_id();
        if matches!(
            self.scheduler_state()?,
            SchedulerThreadState::Terminated { .. }
        ) {
            return Err(RuntimeStateError::conflict(
                "runtime-cancellation",
                "thread-already-terminated",
                "the current guest thread is already terminated",
            ));
        }

        let host_requests_before = self.registries.host_requests.len();
        self.registries
            .host_requests
            .retain(|_, request| request.thread_id != thread_id);
        let host_requests_cancelled =
            host_requests_before.saturating_sub(self.registries.host_requests.len());
        let callback_frames_cancelled = {
            let thread = self.current_thread_mut()?;
            let count = thread.callback_frames.len();
            thread.callback_frames.clear();
            count
        };
        let scheduler_events_before = self.scheduler.events.len();
        self.scheduler.events.retain(|event| match event {
            RuntimeSchedulerEvent::HostCompleted(completion) => completion.thread_id != thread_id,
        });
        let scheduler_events_discarded =
            scheduler_events_before.saturating_sub(self.scheduler.events.len());
        self.set_scheduler_state(SchedulerThreadState::Terminated { exit_code })?;
        Ok(RuntimeCancellation {
            thread_id,
            exit_code,
            host_requests_cancelled,
            callback_frames_cancelled,
            scheduler_events_discarded,
        })
    }

    fn current_thread(&self) -> Result<&RuntimeThreadState, RuntimeStateError> {
        self.threads
            .get(&self.scheduler.current_thread_id)
            .ok_or_else(|| {
                RuntimeStateError::runtime_defect(
                    "runtime-thread",
                    "current-thread-state-missing",
                    "scheduler current thread has no runtime thread state",
                )
            })
    }

    fn current_thread_mut(&mut self) -> Result<&mut RuntimeThreadState, RuntimeStateError> {
        self.threads
            .get_mut(&self.scheduler.current_thread_id)
            .ok_or_else(|| {
                RuntimeStateError::runtime_defect(
                    "runtime-thread",
                    "current-thread-state-missing",
                    "scheduler current thread has no runtime thread state",
                )
            })
    }

    fn set_scheduler_state(
        &mut self,
        state: SchedulerThreadState,
    ) -> Result<(), RuntimeStateError> {
        let entry = self
            .scheduler
            .threads
            .get_mut(&self.scheduler.current_thread_id)
            .ok_or_else(|| {
                RuntimeStateError::runtime_defect(
                    "runtime-scheduler",
                    "current-thread-scheduler-state-missing",
                    "current runtime thread has no scheduler state",
                )
            })?;
        *entry = state;
        Ok(())
    }
}

fn validate_address_space(layout: ProcessAddressSpaceLayout) -> Result<(), RuntimeStateError> {
    let ranges = [
        ("import-thunks", layout.import_thunks),
        ("callback-returns", layout.callback_returns),
        ("dynamic-exports", layout.dynamic_exports),
    ];
    let mut checked = Vec::with_capacity(ranges.len());
    for (name, range) in ranges {
        let start = u64::from(range.base.0);
        let end = start
            .checked_add(range.byte_length)
            .filter(|end| *end <= u64::from(u32::MAX) + 1)
            .ok_or_else(|| {
                RuntimeStateError::invalid_input(
                    "runtime-state",
                    "address-space-range-invalid",
                    format!("process {name} range is empty or crosses the x86 address limit"),
                )
            })?;
        if range.byte_length == 0 {
            return Err(RuntimeStateError::invalid_input(
                "runtime-state",
                "address-space-range-invalid",
                format!("process {name} range is empty or crosses the x86 address limit"),
            ));
        }
        checked.push((name, start, end));
    }
    for left in 0..checked.len() {
        for right in (left + 1)..checked.len() {
            let (left_name, left_start, left_end) = checked[left];
            let (right_name, right_start, right_end) = checked[right];
            if left_start < right_end && right_start < left_end {
                return Err(RuntimeStateError::invalid_input(
                    "runtime-state",
                    "address-space-ranges-overlap",
                    format!("process {left_name} and {right_name} ranges overlap"),
                ));
            }
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeStateErrorClass {
    InvalidInput,
    NotFound,
    Conflict,
    ResourceLimit,
    RuntimeDefect,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeStateError {
    pub class: RuntimeStateErrorClass,
    pub stage: &'static str,
    pub code: &'static str,
    pub message: String,
}

impl RuntimeStateError {
    fn invalid_input(stage: &'static str, code: &'static str, message: impl Into<String>) -> Self {
        Self::new(RuntimeStateErrorClass::InvalidInput, stage, code, message)
    }

    fn not_found(stage: &'static str, code: &'static str, message: impl Into<String>) -> Self {
        Self::new(RuntimeStateErrorClass::NotFound, stage, code, message)
    }

    fn conflict(stage: &'static str, code: &'static str, message: impl Into<String>) -> Self {
        Self::new(RuntimeStateErrorClass::Conflict, stage, code, message)
    }

    fn resource_limit(stage: &'static str, code: &'static str, message: impl Into<String>) -> Self {
        Self::new(RuntimeStateErrorClass::ResourceLimit, stage, code, message)
    }

    fn runtime_defect(stage: &'static str, code: &'static str, message: impl Into<String>) -> Self {
        Self::new(RuntimeStateErrorClass::RuntimeDefect, stage, code, message)
    }

    fn new(
        class: RuntimeStateErrorClass,
        stage: &'static str,
        code: &'static str,
        message: impl Into<String>,
    ) -> Self {
        Self {
            class,
            stage,
            code,
            message: message.into(),
        }
    }
}

impl fmt::Display for RuntimeStateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} / {}: {}",
            self.stage, self.code, self.message
        )
    }
}

impl std::error::Error for RuntimeStateError {}

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::{Serialize, Serializer};

use crate::bootstrap::InitialThreadStack;
use crate::dynamic_exports::{
    DYNAMIC_EXPORT_ENDPOINT_NAMESPACE, DYNAMIC_EXPORT_SLOT_SIZE, DynamicExportThunk,
};
use crate::environment::{
    CONTROLLED_ANSI_COMMAND_LINE_ENCODING, CONTROLLED_WIDE_ENVIRONMENT_ENCODING,
    InitialProcessEnvironment,
};
use crate::imports::{ImportThunkArena, ImportThunkId, RuntimeExport};
use crate::materialize::ImageMaterialization;
use crate::memory::{
    GUEST_PAGE_SIZE, GuestAddress, GuestMemory, GuestMemoryError, GuestMemoryErrorClass,
    GuestRange, MemoryInvalidation, MemoryOwnerId, MemoryRegionKind, PageProtection,
};
use crate::module_graph::{RuntimeModuleClass, RuntimeModuleGraph, RuntimeModuleId};
use crate::module_images::{
    RuntimeModuleImage, RuntimeModuleImageError, RuntimeModuleImageRegistry, RuntimeResolvedExport,
};
use crate::pe32::{ExportLookup, ImportTarget, LoaderIngressReport};
use crate::snapshot::ModuleIdentity;

mod address_space;
mod heaps;
mod messages;
mod modules;
mod objects;
mod process;
mod scheduler;
mod synchronization;
mod thread;
mod waits;

pub use address_space::{
    ProcessAddressAllocationClassification, ProcessAddressAllocationId,
    ProcessAddressAllocationLifetime, ProcessAddressAllocationPurpose,
    ProcessAddressAllocationRecord, ProcessAddressAllocationReleasePolicy,
    ProcessAddressAllocationRequest, ProcessAddressSpaceAllocator, ProcessAddressSpaceLayout,
    RuntimeAddressRange,
};
use heaps::RuntimeHeaps;
use messages::{RuntimeMessageClearPlan, RuntimeMessageReceivePlan, RuntimeMessages};
pub use messages::{
    RuntimeMessageId, RuntimeMessageReceiveDisposition, RuntimeMessageWake, RuntimeThreadMessage,
};
use modules::{
    RuntimeDllLifecyclePlan, RuntimeGuestImportBindingPlan, RuntimeModuleRecords, RuntimeModules,
    runtime_module_image_state_error,
};
pub use objects::RuntimeObjectTable;
use process::RuntimeProcessDirectories;
pub use process::{RuntimeCurrentDirectoryA, RuntimeSetCurrentDirectoryAPlan};
pub use scheduler::{
    RuntimeHostCompletion, RuntimeHostRequest, RuntimeScheduler, RuntimeSchedulerEvent,
    SchedulerThreadState,
};
use scheduler::{RuntimeHostRequestCancellationPlan, RuntimeSchedulerLanePlan};
use synchronization::RuntimeSynchronization;
pub use synchronization::{
    RUNTIME_CRITICAL_SECTION_LIMIT, RUNTIME_MUTEX_LIMIT, RuntimeCriticalSection, RuntimeMutex,
    RuntimeMutexId, RuntimeMutexNameA,
};
use thread::{RuntimeThreadMutationPlan, RuntimeThreads};

pub const LOGICAL_PRIVATE_HEAP_NAMESPACE: &str = "vine.logical-private-heap.v1";
pub const LOGICAL_LOADED_MODULE_NAMESPACE: &str = "vine.logical-loaded-module.v1";
pub const LOGICAL_TLS_INDEX_NAMESPACE: &str = "vine.logical-tls-index.v1";
pub const LOGICAL_FLS_INDEX_NAMESPACE: &str = "vine.logical-fls-index.v1";
pub const LOGICAL_ENCODED_POINTER_NAMESPACE: &str = "vine.logical-encoded-pointer.v1";
pub const LOGICAL_STANDARD_HANDLE_NAMESPACE: &str = "vine.logical-standard-handle.v1";
pub const RUNTIME_PROCESS_OBJECT_NAMESPACE: &str = "vine.runtime-process-object.v1";
pub const RUNTIME_THREAD_OBJECT_NAMESPACE: &str = "vine.runtime-thread-object.v1";
pub const RUNTIME_MUTEX_OBJECT_NAMESPACE: &str = "vine.runtime-mutex-object.v1";
pub const RUNTIME_PRIVATE_HEAP_ARENA_BASE: GuestAddress = GuestAddress(0x2000_0000);
pub const RUNTIME_PRIVATE_HEAP_ARENA_STRIDE: u32 = 0x0100_0000;
pub const RUNTIME_PRIVATE_HEAP_BACKING_LIMIT: u16 = 32;
pub const RUNTIME_PRIVATE_HEAP_ALIGNMENT: u32 = 8;
pub const RUNTIME_PRIVATE_HEAP_UNINITIALIZED_BYTE: u8 = 0xa5;
pub const RUNTIME_PRIVATE_HEAP_FREED_BYTE: u8 = 0xdd;
pub const RUNTIME_PRIVATE_HEAP_MAX_LIVE_ALLOCATIONS: usize = 65_536;
pub const TLS_OUT_OF_INDEXES: u32 = u32::MAX;
pub const FLS_OUT_OF_INDEXES: u32 = u32::MAX;
pub const RUNTIME_FLS_INDEX_LIMIT: u32 = 128;
pub const X86_STARTUP_INFO_W_BYTE_LENGTH: usize = 68;
pub const X86_CRITICAL_SECTION_BYTE_LENGTH: usize = 24;
pub const RUNTIME_STANDARD_INPUT_SELECTOR: u32 = 0xffff_fff6;
pub const RUNTIME_STANDARD_INPUT_HANDLE: u32 = 0x5301_0001;
pub const RUNTIME_STANDARD_OUTPUT_SELECTOR: u32 = 0xffff_fff5;
pub const RUNTIME_STANDARD_OUTPUT_HANDLE: u32 = 0x5301_0002;
pub const RUNTIME_STANDARD_ERROR_SELECTOR: u32 = 0xffff_fff4;
pub const RUNTIME_STANDARD_ERROR_HANDLE: u32 = 0x5301_0003;
pub const RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING: &str = "windows-932-v1";
pub const RUNTIME_PROCESS_IMAGE_PATH_A_MAX_BYTES: usize = 32 * 1024;

const LOGICAL_PRIVATE_HEAP_HANDLE_PREFIX: u32 = 0x6801_0000;
const RUNTIME_PRIVATE_HEAP_OWNER: &str = "runtime-process-private-heaps";
const RUNTIME_PRIVATE_HEAP_VARIANT: &str = "heap-create-metadata-v1";
const RUNTIME_STANDARD_HANDLE_OWNER: &str = "runtime-process-standard-handles";
const RUNTIME_STANDARD_INPUT_VARIANT: &str = "standard-input-logical-v1";
const RUNTIME_STANDARD_OUTPUT_VARIANT: &str = "standard-output-logical-v1";
const RUNTIME_STANDARD_ERROR_VARIANT: &str = "standard-error-logical-v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct RuntimeProcessId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct RuntimeThreadId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeObjectKind {
    Process,
    Thread,
    Mutex,
    PrivateHeap,
    Module,
}

impl RuntimeObjectKind {
    const fn handle_tag(self) -> u8 {
        match self {
            Self::Process => 0x50,
            Self::Thread => 0x54,
            Self::Mutex => 0x58,
            Self::PrivateHeap => 0x68,
            Self::Module => 0x4d,
        }
    }

    fn from_handle_tag(tag: u8) -> Option<Self> {
        match tag {
            0x50 => Some(Self::Process),
            0x54 => Some(Self::Thread),
            0x58 => Some(Self::Mutex),
            0x68 => Some(Self::PrivateHeap),
            0x4d => Some(Self::Module),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct RuntimeHandle(pub u32);

impl RuntimeHandle {
    pub fn from_parts(
        kind: RuntimeObjectKind,
        generation: u8,
        slot: u16,
    ) -> Result<Self, RuntimeStateError> {
        if generation == 0 || slot == 0 {
            return Err(RuntimeStateError::invalid_input(
                "runtime-object",
                "runtime-handle-parts-invalid",
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
            RuntimeStateError::invalid_input(
                "runtime-object",
                "runtime-handle-type-unknown",
                format!(
                    "runtime handle {:#010x} has unknown type tag {tag:#04x}",
                    self.0
                ),
            )
        })
    }

    pub const fn generation(self) -> u8 {
        ((self.0 >> 16) & 0xff) as u8
    }

    pub const fn slot(self) -> u16 {
        (self.0 & 0xffff) as u16
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum RuntimeObjectKey {
    Process { process_id: u32 },
    Thread { thread_id: u32 },
    Mutex { mutex_id: u64 },
    PrivateHeap { handle: u32 },
    Module { handle: u32 },
}

impl RuntimeObjectKey {
    pub const fn kind(&self) -> RuntimeObjectKind {
        match self {
            Self::Process { .. } => RuntimeObjectKind::Process,
            Self::Thread { .. } => RuntimeObjectKind::Thread,
            Self::Mutex { .. } => RuntimeObjectKind::Mutex,
            Self::PrivateHeap { .. } => RuntimeObjectKind::PrivateHeap,
            Self::Module { .. } => RuntimeObjectKind::Module,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct RuntimeObjectId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeHandleClosePolicy {
    CloseHandle,
    SubsystemOwned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum RuntimeWaitState {
    Nonsignaled,
    Signaled { exit_code: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct RuntimeWaitRegistrationId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeWaitRegistration {
    pub id: RuntimeWaitRegistrationId,
    pub thread_id: RuntimeThreadId,
    pub object_id: RuntimeObjectId,
    pub handle: RuntimeHandle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeWaitCompletion {
    pub registration: RuntimeWaitRegistration,
    pub wait_state: RuntimeWaitState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "outcome", rename_all = "kebab-case")]
pub enum RuntimeWaitDisposition {
    AlreadySignaled {
        object_id: RuntimeObjectId,
        wait_state: RuntimeWaitState,
    },
    Pending {
        registration: RuntimeWaitRegistration,
    },
    MutexAcquired {
        object_id: RuntimeObjectId,
        mutex_id: RuntimeMutexId,
        recursion_count: u32,
        abandoned: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeCreateMutexAResult {
    pub mutex_id: RuntimeMutexId,
    pub handle: RuntimeHandle,
    pub already_exists: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeObjectRecord {
    pub id: RuntimeObjectId,
    pub key: RuntimeObjectKey,
    pub kind: RuntimeObjectKind,
    pub namespace: String,
    pub close_policy: RuntimeHandleClosePolicy,
    pub wait_state: Option<RuntimeWaitState>,
    pub open_handles: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeHandleEntry {
    pub handle: RuntimeHandle,
    pub object_id: RuntimeObjectId,
    pub kind: RuntimeObjectKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct RuntimeCallbackFrameId(pub u64);

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
pub struct RuntimeCommandLine {
    pub source_authority: String,
    pub wide_buffer_address: GuestAddress,
    pub wide_length_bytes: u16,
    pub ansi_buffer_address: GuestAddress,
    pub ansi_byte_length_with_nul: u32,
    pub ansi_encoding: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeProcessImagePathA {
    pub source_authority: String,
    pub bytes_with_nul: Vec<u8>,
    pub encoding: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeEnvironmentStringCopy {
    pub acquisition_id: u64,
    pub buffer_address: GuestAddress,
    pub byte_length_with_double_nul: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeEnvironmentStrings {
    pub source_authority: String,
    pub authority_buffer_address: GuestAddress,
    pub copy_buffer_address: GuestAddress,
    pub byte_length_with_double_nul: u32,
    pub encoding: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_copy: Option<RuntimeEnvironmentStringCopy>,
    pub next_acquisition_id: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeProcessEnvironment {
    pub address_allocation_id: ProcessAddressAllocationId,
    pub range: RuntimeAddressRange,
    pub command_line: RuntimeCommandLine,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment_strings: Option<RuntimeEnvironmentStrings>,
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
    pub process_image_path_a: RuntimeProcessImagePathA,
    pub initial_current_directory_a: RuntimeCurrentDirectoryA,
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
pub struct RuntimeStandardHandle {
    pub owner: String,
    pub variant_id: String,
    pub namespace: String,
    pub handle: u32,
    pub selector: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(transparent)]
pub struct RuntimeHeapAllocationId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeHeapInitialization {
    Zeroed,
    DeterministicUninitialized,
}

impl RuntimeHeapInitialization {
    pub const fn fill_byte(self) -> u8 {
        match self {
            Self::Zeroed => 0,
            Self::DeterministicUninitialized => RUNTIME_PRIVATE_HEAP_UNINITIALIZED_BYTE,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeHeapAllocation {
    pub id: RuntimeHeapAllocationId,
    pub address: GuestAddress,
    pub byte_length: u32,
    pub reserved_byte_length: u32,
    pub initialization: RuntimeHeapInitialization,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeHeapBacking {
    pub memory_owner: MemoryOwnerId,
    pub address_allocation_id: ProcessAddressAllocationId,
    pub range: GuestRange,
    pub alignment: u32,
    pub committed_bytes: u32,
    pub high_water_mark: u32,
    pub next_allocation_id: RuntimeHeapAllocationId,
    pub allocations: BTreeMap<u32, RuntimeHeapAllocation>,
    pub free_blocks: BTreeMap<u32, u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeHeapTeardown {
    pub handles: Vec<u32>,
    pub address_allocation_ids: Vec<ProcessAddressAllocationId>,
    pub allocations_discarded: usize,
    pub reserved_bytes_released: u64,
    pub committed_bytes_released: u64,
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
pub struct RuntimeGuestImport {
    pub requested_module_id: RuntimeModuleId,
    pub requested_module: String,
    #[serde(flatten)]
    pub target: ImportTarget,
    pub iat_address: GuestAddress,
    pub original_iat_value: u32,
    pub resolution: RuntimeGuestImportResolution,
}

impl RuntimeGuestImport {
    pub fn resolved_address(&self) -> GuestAddress {
        match &self.resolution {
            RuntimeGuestImportResolution::GuestExport { export } => export.address,
            RuntimeGuestImportResolution::RuntimeExport { endpoint } => endpoint.address,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum RuntimeGuestImportResolution {
    GuestExport {
        export: RuntimeResolvedExport,
    },
    RuntimeExport {
        endpoint: RuntimeSystemExportEndpoint,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeSystemExportEndpoint {
    pub id: ImportThunkId,
    pub address: GuestAddress,
    pub export: RuntimeExport,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeSystemExportTable {
    pub arena: ImportThunkArena,
    pub endpoints: Vec<RuntimeSystemExportEndpoint>,
}

impl RuntimeSystemExportTable {
    pub fn endpoint(
        &self,
        module: &str,
        target: &ImportTarget,
    ) -> Option<&RuntimeSystemExportEndpoint> {
        self.endpoints
            .iter()
            .find(|endpoint| runtime_export_matches_import(&endpoint.export, module, target))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeModuleDependencyEdge {
    pub requester: RuntimeModuleId,
    pub dependency: RuntimeModuleId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeGuestImportBinding {
    pub requester_module_id: RuntimeModuleId,
    pub requester_handle: u32,
    pub source: ModuleIdentity,
    pub image_owner: MemoryOwnerId,
    pub image_load_base: GuestAddress,
    pub imports: Vec<RuntimeGuestImport>,
    pub dependency_edges: Vec<RuntimeModuleDependencyEdge>,
    pub image_owner_sealed: bool,
    pub invalidation: MemoryInvalidation,
}

pub const DLL_PROCESS_DETACH: u32 = 0;
pub const DLL_PROCESS_ATTACH: u32 = 1;
pub const DLL_PROCESS_STATIC_RESERVED: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeDllLifecyclePhase {
    Idle,
    ProcessAttaching,
    AttachUnwinding,
    ProcessAttached,
    ProcessDetaching,
    ProcessDetached,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeDllCallbackReason {
    ProcessAttach,
    ProcessDetach,
}

impl RuntimeDllCallbackReason {
    pub const fn value(self) -> u32 {
        match self {
            Self::ProcessAttach => DLL_PROCESS_ATTACH,
            Self::ProcessDetach => DLL_PROCESS_DETACH,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeDllCallbackRequest {
    pub frame: RuntimeCallbackFrame,
    pub module_id: RuntimeModuleId,
    pub module_handle: u32,
    pub module_name: String,
    pub module_base: GuestAddress,
    pub entry_point: GuestAddress,
    pub reason: RuntimeDllCallbackReason,
    pub reserved: u32,
}

impl RuntimeDllCallbackRequest {
    pub fn arguments(&self) -> [u32; 3] {
        [self.module_base.0, self.reason.value(), self.reserved]
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeDllLifecycleCompletion {
    pub phase: RuntimeDllLifecyclePhase,
    pub graph_order: Vec<RuntimeModuleId>,
    pub callback_order: Vec<RuntimeModuleId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeDllAttachFailure {
    pub failed_module_id: RuntimeModuleId,
    pub failed_module: String,
    pub unwound_modules: Vec<RuntimeModuleId>,
    pub retryable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "action", rename_all = "kebab-case")]
pub enum RuntimeDllLifecycleAction {
    Callback {
        request: RuntimeDllCallbackRequest,
    },
    Complete {
        completion: RuntimeDllLifecycleCompletion,
    },
    AttachFailed {
        failure: RuntimeDllAttachFailure,
    },
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeFlsSlot {
    pub namespace: String,
    pub index: u32,
    pub callback: Option<GuestAddress>,
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
pub enum RuntimeProcessLifecycleState {
    Bootstrapping,
    Running,
    ExitRequested {
        exit_code: u32,
        initiating_thread_id: RuntimeThreadId,
    },
    Exited {
        exit_code: u32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum RuntimeThreadLifecycleState {
    Initialized,
    Running,
    ExitRequested { exit_code: u32 },
    Exited { exit_code: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeCancellation {
    pub thread_id: RuntimeThreadId,
    pub exit_code: u32,
    pub host_requests_cancelled: usize,
    pub callback_frames_cancelled: usize,
    pub wait_registrations_cancelled: usize,
    pub messages_discarded: usize,
    pub scheduler_events_discarded: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeProcessCancellation {
    pub initiating_thread_id: RuntimeThreadId,
    pub exit_code: u32,
    pub thread_cancellations: Vec<RuntimeCancellation>,
    pub host_requests_cancelled: usize,
    pub callback_frames_cancelled: usize,
    pub wait_registrations_cancelled: usize,
    pub messages_discarded: usize,
    pub scheduler_events_discarded: usize,
}

struct RuntimeThreadExitRequestPlan {
    thread_id: RuntimeThreadId,
    thread_plan: RuntimeThreadMutationPlan,
    scheduler_plan: RuntimeSchedulerLanePlan,
    cancelled_wait: Option<RuntimeWaitRegistration>,
    host_request_cancellation: RuntimeHostRequestCancellationPlan,
    message_clear: RuntimeMessageClearPlan,
    cancellation: RuntimeCancellation,
}

struct RuntimeThreadExitCompletionPlan {
    thread_id: RuntimeThreadId,
    exit_code: u32,
    thread_plan: RuntimeThreadMutationPlan,
    scheduler_plan: RuntimeSchedulerLanePlan,
}

struct RuntimeWaitWakePlan {
    completion: RuntimeWaitCompletion,
    scheduler_plan: RuntimeSchedulerLanePlan,
}

struct RuntimeProcessExitRequestPlan {
    initiating_thread_id: RuntimeThreadId,
    exit_code: u32,
    thread_plans: Vec<RuntimeThreadExitRequestPlan>,
    cancellation: RuntimeProcessCancellation,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum RuntimeHeapBackingPlan {
    New {
        address_allocation_id: ProcessAddressAllocationId,
        range: GuestRange,
        initial_committed_bytes: u32,
    },
    Existing {
        memory_owner: MemoryOwnerId,
        address_allocation_id: ProcessAddressAllocationId,
        range: GuestRange,
        committed_bytes: u32,
    },
}

impl RuntimeHeapBackingPlan {
    pub(crate) const fn range(self) -> GuestRange {
        match self {
            Self::New { range, .. } | Self::Existing { range, .. } => range,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct RuntimeHeapAllocationPlan {
    pub handle: u32,
    pub allocation: RuntimeHeapAllocation,
    pub allocation_offset: u32,
    pub backing: RuntimeHeapBackingPlan,
    pub source_free_block: Option<(u32, u32)>,
    pub previous_high_water_mark: u32,
    pub next_committed_bytes: u32,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct RuntimeHeapFreePlan {
    pub handle: u32,
    pub allocation: RuntimeHeapAllocation,
    pub memory_owner: MemoryOwnerId,
    pub merged_free_block: (u32, u32),
    pub predecessor: Option<(u32, u32)>,
    pub successor: Option<(u32, u32)>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct RuntimeHeapDestructionPlan {
    pub handle: u32,
    pub backing: Option<(ProcessAddressAllocationId, MemoryOwnerId, GuestRange, u32)>,
    pub live_allocations: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeThreadState {
    pub id: RuntimeThreadId,
    pub lifecycle: RuntimeThreadLifecycleState,
    pub last_error: u32,
    pub tls_values: BTreeMap<u32, RuntimeTlsValue>,
    pub fls_storage_initialized: bool,
    pub fls_values: BTreeMap<u32, u32>,
    pub callback_frames: Vec<RuntimeCallbackFrame>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stack: Option<RuntimeThreadStack>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeThreadStack {
    pub thread_id: RuntimeThreadId,
    pub address_allocation_id: ProcessAddressAllocationId,
    pub memory_owner: MemoryOwnerId,
    pub reserved_range: GuestRange,
    pub committed_range: GuestRange,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub guard_page: Option<GuestRange>,
    pub committed_pages: usize,
    pub stack_top: GuestAddress,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeThreadStackGrowth {
    pub thread_id: RuntimeThreadId,
    pub address_allocation_id: ProcessAddressAllocationId,
    pub memory_owner: MemoryOwnerId,
    pub fault_address: GuestAddress,
    pub page_fault_error_code: u32,
    pub committed_page: GuestRange,
    pub previous_guard_page: GuestRange,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_guard_page: Option<GuestRange>,
    pub committed_pages: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeProcessState {
    pub id: RuntimeProcessId,
    pub lifecycle: RuntimeProcessLifecycleState,
    pub address_space: ProcessAddressSpaceLayout,
    pub clock: RuntimeClockState,
    pub startup_info: RuntimeStartupInfo,
    pub image_path_a: RuntimeProcessImagePathA,
    #[serde(flatten)]
    directories: RuntimeProcessDirectories,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment: Option<RuntimeProcessEnvironment>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unhandled_exception_filter: Option<GuestAddress>,
    pub heap_termination_on_corruption: bool,
    #[serde(flatten)]
    heaps: RuntimeHeaps,
    #[serde(flatten)]
    module_records: RuntimeModuleRecords,
    pub allocated_tls_indices: BTreeSet<u32>,
    pub fls_slots: BTreeMap<u32, RuntimeFlsSlot>,
    #[serde(flatten)]
    synchronization: RuntimeSynchronization,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub standard_handles: BTreeMap<u32, RuntimeStandardHandle>,
}

impl RuntimeProcessState {
    pub fn heaps(&self) -> &BTreeMap<u32, RuntimeHeap> {
        self.heaps.heaps()
    }

    pub fn modules(&self) -> &BTreeMap<u32, RuntimeModule> {
        self.module_records.modules()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeRegistries {
    pub dynamic_endpoints: BTreeMap<u32, RuntimeDynamicEndpoint>,
    pub encoded_pointers: BTreeMap<u32, RuntimeEncodedPointer>,
}

struct RuntimeCallbackMutationPlan {
    thread: RuntimeThreadMutationPlan,
    scheduler: RuntimeSchedulerLanePlan,
    next_callback_frame_id: u64,
    entered: Option<RuntimeCallbackFrame>,
    exited: Option<RuntimeCallbackFrame>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeState {
    process: RuntimeProcessState,
    address_space_allocator: ProcessAddressSpaceAllocator,
    threads: RuntimeThreads,
    scheduler: RuntimeScheduler,
    messages: RuntimeMessages,
    objects: RuntimeObjectTable,
    modules: RuntimeModules,
    registries: RuntimeRegistries,
    encoded_pointer_namespace: String,
    encoded_pointer_token_prefix: u32,
    encoded_pointer_sources: BTreeMap<u32, u32>,
    next_encoded_pointer_id: u16,
    next_private_heap_slot: u16,
    next_tls_index: u32,
    next_fls_index: u32,
    next_callback_frame_id: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeRegistriesSnapshot<'a> {
    dynamic_endpoints: &'a BTreeMap<u32, RuntimeDynamicEndpoint>,
    encoded_pointers: &'a BTreeMap<u32, RuntimeEncodedPointer>,
    host_requests: &'a BTreeMap<u64, RuntimeHostRequest>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeStateSnapshot<'a> {
    process: &'a RuntimeProcessState,
    address_space_allocator: &'a ProcessAddressSpaceAllocator,
    threads: &'a RuntimeThreads,
    scheduler: &'a RuntimeScheduler,
    messages: &'a RuntimeMessages,
    objects: &'a RuntimeObjectTable,
    #[serde(flatten)]
    modules: &'a RuntimeModules,
    registries: RuntimeRegistriesSnapshot<'a>,
    encoded_pointer_namespace: &'a str,
    encoded_pointer_token_prefix: u32,
    encoded_pointer_sources: &'a BTreeMap<u32, u32>,
    next_encoded_pointer_id: u16,
    next_private_heap_slot: u16,
    next_tls_index: u32,
    next_fls_index: u32,
    next_callback_frame_id: u64,
    next_host_request_id: u64,
    next_wait_registration_id: u64,
}

impl Serialize for RuntimeState {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        RuntimeStateSnapshot {
            process: &self.process,
            address_space_allocator: &self.address_space_allocator,
            threads: &self.threads,
            scheduler: &self.scheduler,
            messages: &self.messages,
            objects: &self.objects,
            modules: &self.modules,
            registries: RuntimeRegistriesSnapshot {
                dynamic_endpoints: &self.registries.dynamic_endpoints,
                encoded_pointers: &self.registries.encoded_pointers,
                host_requests: self.scheduler.host_requests(),
            },
            encoded_pointer_namespace: &self.encoded_pointer_namespace,
            encoded_pointer_token_prefix: self.encoded_pointer_token_prefix,
            encoded_pointer_sources: &self.encoded_pointer_sources,
            next_encoded_pointer_id: self.next_encoded_pointer_id,
            next_private_heap_slot: self.next_private_heap_slot,
            next_tls_index: self.next_tls_index,
            next_fls_index: self.next_fls_index,
            next_callback_frame_id: self.next_callback_frame_id,
            next_host_request_id: self.scheduler.next_host_request_id(),
            next_wait_registration_id: self.scheduler.next_wait_registration_id(),
        }
        .serialize(serializer)
    }
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
        if bootstrap.process_image_path_a.source_authority.is_empty()
            || bootstrap.process_image_path_a.encoding != RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING
            || bootstrap.process_image_path_a.bytes_with_nul.len() < 2
            || bootstrap.process_image_path_a.bytes_with_nul.len()
                > RUNTIME_PROCESS_IMAGE_PATH_A_MAX_BYTES
            || bootstrap.process_image_path_a.bytes_with_nul.last() != Some(&0)
            || bootstrap.process_image_path_a.bytes_with_nul
                [..bootstrap.process_image_path_a.bytes_with_nul.len() - 1]
                .contains(&0)
        {
            return Err(RuntimeStateError::invalid_input(
                "runtime-process-image-path",
                "process-image-path-a-invalid",
                "runtime process image path requires bounded Windows-932 bytes with exactly one trailing NUL",
            ));
        }
        let address_space_allocator =
            ProcessAddressSpaceAllocator::vine_v1(bootstrap.address_space)?;
        let process_directories = RuntimeProcessDirectories::new(
            bootstrap.initial_current_directory_a.clone(),
            &bootstrap.process_image_path_a,
        )?;
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

        let process_key = RuntimeObjectKey::Process {
            process_id: bootstrap.process_id.0,
        };
        let thread_key = RuntimeObjectKey::Thread {
            thread_id: bootstrap.initial_thread_id.0,
        };
        let mut objects = RuntimeObjectTable::default();
        objects.register_object_and_open_handle(process_key)?;
        objects.register_object_and_open_handle(thread_key)?;

        let threads =
            RuntimeThreads::new(bootstrap.initial_thread_id, bootstrap.initial_last_error);
        Ok(Self {
            process: RuntimeProcessState {
                id: bootstrap.process_id,
                lifecycle: RuntimeProcessLifecycleState::Bootstrapping,
                address_space: bootstrap.address_space,
                clock: bootstrap.clock,
                startup_info: bootstrap.startup_info,
                image_path_a: bootstrap.process_image_path_a,
                directories: process_directories,
                environment: None,
                unhandled_exception_filter: None,
                heap_termination_on_corruption: false,
                heaps: RuntimeHeaps::default(),
                module_records: RuntimeModuleRecords::default(),
                allocated_tls_indices: BTreeSet::new(),
                fls_slots: BTreeMap::new(),
                synchronization: RuntimeSynchronization::default(),
                standard_handles: BTreeMap::new(),
            },
            address_space_allocator,
            threads,
            scheduler: RuntimeScheduler::new(bootstrap.initial_thread_id),
            messages: RuntimeMessages::new(bootstrap.initial_thread_id),
            objects,
            modules: RuntimeModules::default(),
            registries: RuntimeRegistries {
                dynamic_endpoints: BTreeMap::new(),
                encoded_pointers: BTreeMap::new(),
            },
            encoded_pointer_namespace: bootstrap.encoded_pointer_namespace,
            encoded_pointer_token_prefix: bootstrap.encoded_pointer_token_prefix,
            encoded_pointer_sources: BTreeMap::new(),
            next_encoded_pointer_id: 1,
            next_private_heap_slot: 1,
            next_tls_index: 0,
            next_fls_index: 0,
            next_callback_frame_id: 1,
        })
    }

    pub fn process_id(&self) -> RuntimeProcessId {
        self.process.id
    }

    pub fn process(&self) -> &RuntimeProcessState {
        &self.process
    }

    pub fn process_image_path_a(&self) -> &RuntimeProcessImagePathA {
        &self.process.image_path_a
    }

    pub fn current_directory_a(&self) -> &RuntimeCurrentDirectoryA {
        self.process.directories.current_directory_a()
    }

    pub fn current_directory_generation(&self) -> u64 {
        self.process.directories.current_directory_generation()
    }

    pub fn is_process_image_directory_a(
        &self,
        requested: &[u8],
    ) -> Result<bool, RuntimeStateError> {
        self.process
            .directories
            .is_process_image_directory(requested)
    }

    pub fn plan_set_current_directory_a(
        &self,
        requested: &[u8],
    ) -> Result<RuntimeSetCurrentDirectoryAPlan, RuntimeStateError> {
        if self.process.lifecycle != RuntimeProcessLifecycleState::Running {
            return Err(RuntimeStateError::conflict(
                "runtime-process-directory",
                "current-directory-process-state-invalid",
                "current-directory replacement requires a running process",
            ));
        }
        self.process
            .directories
            .plan_process_image_directory(requested)
    }

    pub fn commit_set_current_directory_a(
        &mut self,
        plan: RuntimeSetCurrentDirectoryAPlan,
    ) -> Result<&RuntimeCurrentDirectoryA, RuntimeStateError> {
        if self.process.lifecycle != RuntimeProcessLifecycleState::Running {
            return Err(RuntimeStateError::conflict(
                "runtime-process-directory",
                "current-directory-process-state-invalid",
                "current-directory replacement requires a running process",
            ));
        }
        self.process.directories.commit(plan)
    }

    pub fn unhandled_exception_filter(&self) -> Option<GuestAddress> {
        self.process.unhandled_exception_filter
    }

    pub fn set_unhandled_exception_filter(
        &mut self,
        filter: Option<GuestAddress>,
    ) -> Result<Option<GuestAddress>, RuntimeStateError> {
        if self.process.lifecycle != RuntimeProcessLifecycleState::Running {
            return Err(RuntimeStateError::conflict(
                "runtime-process-exception-filter",
                "unhandled-exception-filter-process-state-invalid",
                "unhandled-exception filter replacement requires a running process",
            ));
        }
        let previous = self.process.unhandled_exception_filter;
        self.process.unhandled_exception_filter = filter;
        Ok(previous)
    }

    pub fn process_address_space_allocator(&self) -> &ProcessAddressSpaceAllocator {
        &self.address_space_allocator
    }

    pub fn register_initial_process_environment(
        &mut self,
        environment: &InitialProcessEnvironment,
    ) -> Result<&RuntimeProcessEnvironment, RuntimeStateError> {
        if self.process.environment.is_some() {
            return Err(RuntimeStateError::conflict(
                "runtime-process-environment",
                "process-environment-already-registered",
                "runtime process environment may be registered only once",
            ));
        }
        if matches!(
            self.process.lifecycle,
            RuntimeProcessLifecycleState::ExitRequested { .. }
                | RuntimeProcessLifecycleState::Exited { .. }
        ) {
            return Err(RuntimeStateError::conflict(
                "runtime-process-environment",
                "process-environment-lifecycle-state-invalid",
                "runtime process environment cannot be registered during process exit",
            ));
        }
        if !environment.owners_sealed {
            return Err(RuntimeStateError::invalid_input(
                "runtime-process-environment",
                "process-environment-owner-unsealed",
                "runtime process environment requires sealed guest-memory owners",
            ));
        }
        let parameters = environment.process_parameters.as_ref().ok_or_else(|| {
            RuntimeStateError::invalid_input(
                "runtime-process-environment",
                "process-parameters-missing",
                "runtime process command line requires initial process parameters",
            )
        })?;
        if !parameters.owners_sealed {
            return Err(RuntimeStateError::invalid_input(
                "runtime-process-environment",
                "process-parameter-owner-unsealed",
                "runtime process parameters require sealed guest-memory owners",
            ));
        }
        let ansi = parameters.ansi.as_ref().ok_or_else(|| {
            RuntimeStateError::invalid_input(
                "runtime-process-environment",
                "ansi-command-line-unavailable",
                "the controlled process command line has no deterministic ANSI view",
            )
        })?;
        if ansi.encoding != CONTROLLED_ANSI_COMMAND_LINE_ENCODING
            || !parameters.command_line.is_ascii()
            || usize::try_from(ansi.byte_length_with_nul).ok()
                != parameters.command_line.len().checked_add(1)
        {
            return Err(RuntimeStateError::invalid_input(
                "runtime-process-environment",
                "ansi-command-line-metadata-invalid",
                "initial ANSI command-line metadata does not match the controlled ASCII authority",
            ));
        }
        let ansi_end = u64::from(ansi.buffer_address.0)
            .checked_add(u64::from(ansi.byte_length_with_nul))
            .ok_or_else(|| {
                RuntimeStateError::invalid_input(
                    "runtime-process-environment",
                    "ansi-command-line-range-overflow",
                    "initial ANSI command-line range overflowed x86 accounting",
                )
            })?;
        if ansi.buffer_address == parameters.command_line_buffer_address
            || u64::from(ansi.buffer_address.0) < u64::from(parameters.command_line_range.start().0)
            || ansi_end > parameters.command_line_range.end()
        {
            return Err(RuntimeStateError::invalid_input(
                "runtime-process-environment",
                "ansi-command-line-range-invalid",
                "initial ANSI command-line view must be distinct and contained by the command-line allocation",
            ));
        }
        let wide_start = u64::from(parameters.command_line_buffer_address.0);
        let wide_end = wide_start
            .checked_add(u64::from(parameters.command_line_maximum_length_bytes))
            .ok_or_else(|| {
                RuntimeStateError::invalid_input(
                    "runtime-process-environment",
                    "wide-command-line-range-overflow",
                    "initial wide command-line range overflowed x86 accounting",
                )
            })?;
        if wide_start < u64::from(parameters.command_line_range.start().0)
            || wide_end > parameters.command_line_range.end()
            || ranges_overlap(
                wide_start,
                wide_end,
                u64::from(ansi.buffer_address.0),
                ansi_end,
            )
        {
            return Err(RuntimeStateError::invalid_input(
                "runtime-process-environment",
                "command-line-view-range-invalid",
                "initial wide and ANSI command-line views must be disjoint and contained",
            ));
        }
        let environment_strings = parameters
            .environment_strings
            .as_ref()
            .map(|environment| {
                if environment.encoding != CONTROLLED_WIDE_ENVIRONMENT_ENCODING
                    || environment.byte_length_with_double_nul != 4
                    || environment.authority_buffer_address == environment.copy_buffer_address
                {
                    return Err(RuntimeStateError::invalid_input(
                        "runtime-process-environment",
                        "wide-environment-metadata-invalid",
                        "initial wide environment metadata does not match the controlled empty block",
                    ));
                }
                let authority_start = u64::from(environment.authority_buffer_address.0);
                let authority_end = authority_start
                    .checked_add(u64::from(environment.byte_length_with_double_nul))
                    .ok_or_else(|| {
                        RuntimeStateError::invalid_input(
                            "runtime-process-environment",
                            "wide-environment-authority-range-overflow",
                            "wide environment authority range overflowed x86 accounting",
                        )
                    })?;
                let copy_start = u64::from(environment.copy_buffer_address.0);
                let copy_end = copy_start
                    .checked_add(u64::from(environment.byte_length_with_double_nul))
                    .ok_or_else(|| {
                        RuntimeStateError::invalid_input(
                            "runtime-process-environment",
                            "wide-environment-copy-range-overflow",
                            "wide environment copy range overflowed x86 accounting",
                        )
                    })?;
                let command_line_start = u64::from(parameters.command_line_range.start().0);
                let command_line_end = parameters.command_line_range.end();
                if authority_start < command_line_start
                    || authority_end > command_line_end
                    || copy_start < command_line_start
                    || copy_end > command_line_end
                    || ranges_overlap(authority_start, authority_end, copy_start, copy_end)
                    || ranges_overlap(authority_start, authority_end, wide_start, wide_end)
                    || ranges_overlap(authority_start, authority_end, u64::from(ansi.buffer_address.0), ansi_end)
                    || ranges_overlap(copy_start, copy_end, wide_start, wide_end)
                    || ranges_overlap(copy_start, copy_end, u64::from(ansi.buffer_address.0), ansi_end)
                {
                    return Err(RuntimeStateError::invalid_input(
                        "runtime-process-environment",
                        "wide-environment-range-invalid",
                        "wide environment authority and copy must be disjoint from all process string views",
                    ));
                }
                Ok(RuntimeEnvironmentStrings {
                    source_authority: "rtl-user-process-parameters-environment".to_owned(),
                    authority_buffer_address: environment.authority_buffer_address,
                    copy_buffer_address: environment.copy_buffer_address,
                    byte_length_with_double_nul: environment.byte_length_with_double_nul,
                    encoding: environment.encoding.clone(),
                    active_copy: None,
                    next_acquisition_id: 1,
                })
            })
            .transpose()?;

        let mut ranges = [
            parameters.process_parameters_range,
            parameters.command_line_range,
            environment.teb_range,
            environment.peb_range,
        ];
        ranges.sort_by_key(|range| range.start().0);
        if ranges
            .windows(2)
            .any(|pair| pair[0].end() != u64::from(pair[1].start().0))
        {
            return Err(RuntimeStateError::invalid_input(
                "runtime-process-environment",
                "process-environment-range-discontiguous",
                "initial process-environment pages must form one contiguous allocation",
            ));
        }
        let base = ranges[0].start();
        let byte_length = ranges[3]
            .end()
            .checked_sub(u64::from(base.0))
            .ok_or_else(|| {
                RuntimeStateError::runtime_defect(
                    "runtime-process-environment",
                    "process-environment-range-order-invalid",
                    "sorted process-environment ranges have an invalid order",
                )
            })?;
        let range = RuntimeAddressRange::new(base, byte_length)?;
        let mut allocator = self.address_space_allocator.clone();
        let allocation = allocator.reserve(ProcessAddressAllocationRequest {
            purpose: ProcessAddressAllocationPurpose::ProcessEnvironment,
            owner: "runtime-initial-process-environment".to_owned(),
            preferred_address: Some(base),
            search_range: range,
            reserve_size: byte_length,
            alignment: GUEST_PAGE_SIZE as u32,
            guard_pages: 0,
            classification: ProcessAddressAllocationClassification::ProcessEnvironment,
            lifetime: ProcessAddressAllocationLifetime::Process,
            release_policy: ProcessAddressAllocationReleasePolicy::Permanent,
            executable: false,
        })?;
        if allocation.range != range {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-process-environment",
                "process-environment-preferred-address-lost",
                "process address allocator did not preserve the materialized environment range",
            ));
        }
        let registered = RuntimeProcessEnvironment {
            address_allocation_id: allocation.id,
            range,
            command_line: RuntimeCommandLine {
                source_authority: "rtl-user-process-parameters-command-line".to_owned(),
                wide_buffer_address: parameters.command_line_buffer_address,
                wide_length_bytes: parameters.command_line_length_bytes,
                ansi_buffer_address: ansi.buffer_address,
                ansi_byte_length_with_nul: ansi.byte_length_with_nul,
                ansi_encoding: ansi.encoding.clone(),
            },
            environment_strings,
        };
        self.address_space_allocator = allocator;
        self.process.environment = Some(registered);
        Ok(self
            .process
            .environment
            .as_ref()
            .expect("registered process environment"))
    }

    pub fn process_command_line(&self) -> Result<&RuntimeCommandLine, RuntimeStateError> {
        self.process
            .environment
            .as_ref()
            .map(|environment| &environment.command_line)
            .ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-process-environment",
                    "process-command-line-unregistered",
                    "runtime process has no registered command-line authority",
                )
            })
    }

    pub fn process_environment_strings(
        &self,
    ) -> Result<&RuntimeEnvironmentStrings, RuntimeStateError> {
        self.process
            .environment
            .as_ref()
            .and_then(|environment| environment.environment_strings.as_ref())
            .ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-process-environment",
                    "wide-environment-strings-unregistered",
                    "runtime process has no registered wide environment-string authority",
                )
            })
    }

    pub fn acquire_environment_strings_w(
        &mut self,
    ) -> Result<RuntimeEnvironmentStringCopy, RuntimeStateError> {
        if matches!(
            self.process.lifecycle,
            RuntimeProcessLifecycleState::ExitRequested { .. }
                | RuntimeProcessLifecycleState::Exited { .. }
        ) {
            return Err(RuntimeStateError::conflict(
                "runtime-process-environment",
                "wide-environment-acquire-lifecycle-state-invalid",
                "wide environment strings cannot be acquired during process exit",
            ));
        }
        let environment_strings = self
            .process
            .environment
            .as_mut()
            .and_then(|environment| environment.environment_strings.as_mut())
            .ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-process-environment",
                    "wide-environment-strings-unregistered",
                    "runtime process has no registered wide environment-string authority",
                )
            })?;
        if environment_strings.active_copy.is_some() {
            return Err(RuntimeStateError::resource_limit(
                "runtime-process-environment",
                "wide-environment-copy-limit-reached",
                "the controlled profile permits one active wide environment-string copy",
            ));
        }
        let next_acquisition_id = environment_strings
            .next_acquisition_id
            .checked_add(1)
            .ok_or_else(|| {
                RuntimeStateError::resource_limit(
                    "runtime-process-environment",
                    "wide-environment-acquisition-id-exhausted",
                    "wide environment-string acquisition ID space is exhausted",
                )
            })?;
        let copy = RuntimeEnvironmentStringCopy {
            acquisition_id: environment_strings.next_acquisition_id,
            buffer_address: environment_strings.copy_buffer_address,
            byte_length_with_double_nul: environment_strings.byte_length_with_double_nul,
        };
        environment_strings.next_acquisition_id = next_acquisition_id;
        environment_strings.active_copy = Some(copy.clone());
        Ok(copy)
    }

    pub fn free_environment_strings_w(
        &mut self,
        buffer_address: GuestAddress,
    ) -> Result<RuntimeEnvironmentStringCopy, RuntimeStateError> {
        let environment_strings = self
            .process
            .environment
            .as_mut()
            .and_then(|environment| environment.environment_strings.as_mut())
            .ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-process-environment",
                    "wide-environment-strings-unregistered",
                    "runtime process has no registered wide environment-string authority",
                )
            })?;
        let copy = environment_strings.active_copy.as_ref().ok_or_else(|| {
            RuntimeStateError::not_found(
                "runtime-process-environment",
                "wide-environment-copy-not-active",
                "no wide environment-string copy is active",
            )
        })?;
        if copy.buffer_address != buffer_address {
            return Err(RuntimeStateError::invalid_input(
                "runtime-process-environment",
                "wide-environment-copy-pointer-invalid",
                "FreeEnvironmentStringsW requires the active process-owned copy pointer",
            ));
        }
        let copy = copy.clone();
        environment_strings.active_copy = None;
        Ok(copy)
    }

    pub fn current_thread_id(&self) -> RuntimeThreadId {
        self.scheduler.current_thread_id()
    }

    pub fn current_thread_state(&self) -> Result<&RuntimeThreadState, RuntimeStateError> {
        self.current_thread()
    }

    pub fn thread_stack(
        &self,
        thread_id: RuntimeThreadId,
    ) -> Result<&RuntimeThreadStack, RuntimeStateError> {
        self.thread_state(thread_id)?.stack.as_ref().ok_or_else(|| {
            RuntimeStateError::not_found(
                "runtime-thread-stack",
                "thread-stack-not-registered",
                format!("runtime thread ID {} has no registered stack", thread_id.0),
            )
        })
    }

    pub fn register_initial_thread_stack(
        &mut self,
        stack: &InitialThreadStack,
    ) -> Result<&RuntimeThreadStack, RuntimeStateError> {
        let thread_id = self.current_thread_id();
        if self.current_thread()?.stack.is_some() {
            return Err(RuntimeStateError::conflict(
                "runtime-thread-stack",
                "thread-stack-already-registered",
                "runtime initial thread stack may be registered only once",
            ));
        }
        if matches!(
            self.process.lifecycle,
            RuntimeProcessLifecycleState::ExitRequested { .. }
                | RuntimeProcessLifecycleState::Exited { .. }
        ) {
            return Err(RuntimeStateError::conflict(
                "runtime-thread-stack",
                "thread-stack-process-state-invalid",
                "runtime thread stack cannot be registered during process exit",
            ));
        }
        validate_initial_thread_stack(stack)?;

        let guard_pages = u32::from(stack.guard_page.is_some());
        let usable_byte_length = stack
            .stack_range
            .byte_length()
            .checked_sub(u64::from(guard_pages) * GUEST_PAGE_SIZE)
            .ok_or_else(|| {
                RuntimeStateError::runtime_defect(
                    "runtime-thread-stack",
                    "thread-stack-usable-size-underflow",
                    "initial thread stack guard exceeds its reserved range",
                )
            })?;
        let reserved_range =
            RuntimeAddressRange::new(stack.stack_range.start(), stack.stack_range.byte_length())?;
        let mut allocator = self.address_space_allocator.clone();
        let allocation = allocator.reserve(ProcessAddressAllocationRequest {
            purpose: ProcessAddressAllocationPurpose::ThreadStack,
            owner: format!("runtime-thread-{}-initial-stack", thread_id.0),
            preferred_address: Some(reserved_range.base),
            search_range: reserved_range,
            reserve_size: usable_byte_length,
            alignment: GUEST_PAGE_SIZE as u32,
            guard_pages,
            classification: ProcessAddressAllocationClassification::Stack,
            lifetime: ProcessAddressAllocationLifetime::Thread,
            release_policy: ProcessAddressAllocationReleasePolicy::Explicit,
            executable: false,
        })?;
        if allocation.range != reserved_range {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-thread-stack",
                "thread-stack-preferred-address-lost",
                "process address allocator did not preserve the materialized stack range",
            ));
        }
        let registered = RuntimeThreadStack {
            thread_id,
            address_allocation_id: allocation.id,
            memory_owner: stack.owner,
            reserved_range: stack.stack_range,
            committed_range: stack.committed_range,
            guard_page: stack.guard_page,
            committed_pages: stack.committed_pages,
            stack_top: stack.stack_top,
        };
        let thread_plan = self
            .threads
            .plan_stack_registration(thread_id, registered)?;
        self.address_space_allocator = allocator;
        self.threads.commit(thread_plan);
        self.thread_stack(thread_id)
    }

    pub fn grow_initial_thread_stack(
        &mut self,
        memory: &mut GuestMemory,
        fault_address: GuestAddress,
        page_fault_error_code: u32,
    ) -> Result<RuntimeThreadStackGrowth, RuntimeStateError> {
        if self.process.lifecycle != RuntimeProcessLifecycleState::Running
            || self.current_thread()?.lifecycle != RuntimeThreadLifecycleState::Running
        {
            return Err(RuntimeStateError::conflict(
                "runtime-thread-stack",
                "thread-stack-growth-lifecycle-state-invalid",
                "initial thread stack growth requires a running process and thread",
            ));
        }
        let thread_id = self.current_thread_id();
        let stack = self.thread_stack(thread_id)?.clone();
        let guard_page = stack.guard_page.ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-thread-stack",
                "thread-stack-reserve-exhausted",
                "initial thread stack has committed its entire reserved range",
            )
        })?;
        let fault_page = GuestAddress(fault_address.0 & !(GUEST_PAGE_SIZE as u32 - 1));
        if fault_page != guard_page.start() {
            return Err(RuntimeStateError::invalid_input(
                "runtime-thread-stack",
                "thread-stack-fault-outside-guard",
                "page fault does not fall within the current initial-thread stack guard page",
            ));
        }
        if page_fault_error_code & 1 != 0 {
            return Err(RuntimeStateError::invalid_input(
                "runtime-thread-stack",
                "thread-stack-protection-fault",
                "present-page protection faults cannot grow the initial thread stack",
            ));
        }
        let stats = memory.stats();
        if stats.generation != stats.acknowledged_generation || stats.pending_invalidation.is_some()
        {
            return Err(RuntimeStateError::conflict(
                "runtime-thread-stack",
                "thread-stack-growth-memory-unsynchronized",
                "initial thread stack growth requires fully acknowledged guest memory",
            ));
        }
        validate_runtime_thread_stack_allocation(&self.address_space_allocator, &stack)?;
        let stack_region_matches = memory.region_snapshots().iter().any(|region| {
            region.owner == stack.memory_owner
                && region.kind == MemoryRegionKind::Stack
                && region.range == stack.reserved_range
        });
        if !stack_region_matches {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-thread-stack",
                "thread-stack-memory-region-mismatch",
                "runtime stack metadata does not match the Rust guest-memory reservation",
            ));
        }
        memory
            .preflight_growth(0, 0, 1, "runtime-thread-stack")
            .map_err(RuntimeStateError::from_memory)?;

        let next_committed_byte_length = stack
            .committed_range
            .byte_length()
            .checked_add(GUEST_PAGE_SIZE)
            .ok_or_else(|| {
                RuntimeStateError::runtime_defect(
                    "runtime-thread-stack",
                    "thread-stack-committed-size-overflow",
                    "initial thread stack committed size overflowed",
                )
            })?;
        let committed_range = GuestRange::new(guard_page.start(), next_committed_byte_length)
            .map_err(RuntimeStateError::from_memory)?;
        let next_guard_page = (guard_page.start().0 > stack.reserved_range.start().0)
            .then(|| {
                guard_page
                    .start()
                    .0
                    .checked_sub(GUEST_PAGE_SIZE as u32)
                    .map(GuestAddress)
                    .ok_or_else(|| {
                        RuntimeStateError::runtime_defect(
                            "runtime-thread-stack",
                            "thread-stack-next-guard-underflow",
                            "initial thread stack guard underflowed its reserved range",
                        )
                    })
            })
            .transpose()?
            .map(|base| {
                GuestRange::new(base, GUEST_PAGE_SIZE).map_err(RuntimeStateError::from_memory)
            })
            .transpose()?;
        let committed_pages = stack.committed_pages.checked_add(1).ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-thread-stack",
                "thread-stack-committed-page-count-overflow",
                "initial thread stack committed page count overflowed host accounting",
            )
        })?;
        let mut next_stack = stack.clone();
        next_stack.committed_range = committed_range;
        next_stack.guard_page = next_guard_page;
        next_stack.committed_pages = committed_pages;
        let thread_plan = self.threads.plan_stack_growth(thread_id, next_stack)?;

        let mut transaction = memory.begin_transaction();
        transaction
            .commit(
                stack.memory_owner,
                guard_page,
                PageProtection {
                    read: true,
                    write: true,
                    execute: false,
                },
            )
            .map_err(RuntimeStateError::from_memory)?;
        transaction.commit_transaction();
        self.threads.commit(thread_plan);

        Ok(RuntimeThreadStackGrowth {
            thread_id,
            address_allocation_id: stack.address_allocation_id,
            memory_owner: stack.memory_owner,
            fault_address,
            page_fault_error_code,
            committed_page: guard_page,
            previous_guard_page: guard_page,
            next_guard_page,
            committed_pages,
        })
    }

    pub fn thread_state(
        &self,
        thread_id: RuntimeThreadId,
    ) -> Result<&RuntimeThreadState, RuntimeStateError> {
        self.threads.state(thread_id)
    }

    pub fn scheduler(&self) -> &RuntimeScheduler {
        &self.scheduler
    }

    pub fn objects(&self) -> &RuntimeObjectTable {
        &self.objects
    }

    pub fn module_graph(&self) -> &RuntimeModuleGraph {
        self.modules.graph()
    }

    pub fn module_images(&self) -> &RuntimeModuleImageRegistry {
        self.modules.images()
    }

    pub fn resolve_guest_export(
        &self,
        module: &str,
        symbol: ExportLookup,
    ) -> Result<RuntimeResolvedExport, RuntimeModuleImageError> {
        self.modules.resolve(module, symbol)
    }

    pub(crate) fn register_module_image(
        &mut self,
        memory: &GuestMemory,
        handle: u32,
        loader: &LoaderIngressReport,
        materialization: &ImageMaterialization,
    ) -> Result<&RuntimeModuleImage, RuntimeModuleImageError> {
        self.module(handle)
            .map_err(runtime_module_image_state_error)?;
        self.modules
            .register_image(memory, handle, loader, materialization)
    }

    pub fn module_import_bindings(&self) -> &BTreeMap<RuntimeModuleId, RuntimeGuestImportBinding> {
        self.modules.bindings()
    }

    pub fn runtime_system_exports(&self) -> Option<&RuntimeSystemExportTable> {
        self.modules.runtime_system_exports()
    }

    pub fn guest_import_binding(
        &self,
        handle: u32,
    ) -> Result<&RuntimeGuestImportBinding, RuntimeStateError> {
        self.module(handle)?;
        self.modules.guest_import_binding(handle)
    }

    pub(crate) fn plan_guest_import_binding(
        &self,
        requester_handle: u32,
        source: ModuleIdentity,
        image_owner: MemoryOwnerId,
        image_load_base: GuestAddress,
        imports: Vec<RuntimeGuestImport>,
        dependency_edges: Vec<RuntimeModuleDependencyEdge>,
    ) -> Result<RuntimeGuestImportBindingPlan, RuntimeStateError> {
        self.module(requester_handle)?;
        self.modules.plan_guest_import_binding(
            requester_handle,
            source,
            image_owner,
            image_load_base,
            imports,
            dependency_edges,
        )
    }

    pub(crate) fn commit_guest_import_binding(
        &mut self,
        plan: RuntimeGuestImportBindingPlan,
        invalidation: MemoryInvalidation,
        runtime_system_exports: Option<RuntimeSystemExportTable>,
    ) -> Result<&RuntimeGuestImportBinding, RuntimeStateError> {
        self.modules
            .commit_guest_import_binding(plan, invalidation, runtime_system_exports)
    }

    pub fn process_handle(&self) -> Result<RuntimeHandle, RuntimeStateError> {
        self.objects.handle_for_object(&RuntimeObjectKey::Process {
            process_id: self.process.id.0,
        })
    }

    pub fn current_thread_handle(&self) -> Result<RuntimeHandle, RuntimeStateError> {
        self.thread_handle(self.current_thread_id())
    }

    pub fn thread_handle(
        &self,
        thread_id: RuntimeThreadId,
    ) -> Result<RuntimeHandle, RuntimeStateError> {
        self.thread_state(thread_id)?;
        self.objects.handle_for_object(&RuntimeObjectKey::Thread {
            thread_id: thread_id.0,
        })
    }

    pub fn open_process_handle(&mut self) -> Result<RuntimeHandle, RuntimeStateError> {
        self.objects.open_handle(&RuntimeObjectKey::Process {
            process_id: self.process.id.0,
        })
    }

    pub fn open_current_thread_handle(&mut self) -> Result<RuntimeHandle, RuntimeStateError> {
        self.open_thread_handle(self.current_thread_id())
    }

    pub fn open_thread_handle(
        &mut self,
        thread_id: RuntimeThreadId,
    ) -> Result<RuntimeHandle, RuntimeStateError> {
        self.thread_state(thread_id)?;
        self.objects.open_handle(&RuntimeObjectKey::Thread {
            thread_id: thread_id.0,
        })
    }

    pub fn object_for_handle(
        &self,
        handle: RuntimeHandle,
        expected_kind: RuntimeObjectKind,
    ) -> Result<&RuntimeObjectRecord, RuntimeStateError> {
        self.objects.object_for_handle(handle, expected_kind)
    }

    pub fn close_handle(
        &mut self,
        handle: RuntimeHandle,
    ) -> Result<RuntimeHandleEntry, RuntimeStateError> {
        if handle.kind()? != RuntimeObjectKind::Mutex {
            return self.objects.close_handle(handle);
        }
        let record = self
            .objects
            .object_for_handle(handle, RuntimeObjectKind::Mutex)?
            .clone();
        let RuntimeObjectKey::Mutex { mutex_id } = record.key.clone() else {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-mutex",
                "mutex-object-key-invalid",
                "mutex handle resolved to a non-mutex object identity",
            ));
        };
        let retirement = (record.open_handles == 1)
            .then(|| {
                self.process
                    .synchronization
                    .plan_mutex_retirement(RuntimeMutexId(mutex_id))
            })
            .transpose()?;
        let mut next_objects = self.objects.clone();
        let removed = next_objects.close_handle(handle)?;
        if retirement.is_some() {
            next_objects.remove_closed_mutex_object(&record.key)?;
        }
        let mut next_synchronization = self.process.synchronization.clone();
        if let Some(retirement) = retirement {
            next_synchronization.commit_mutex_retirement(retirement);
        }
        self.objects = next_objects;
        self.process.synchronization = next_synchronization;
        Ok(removed)
    }

    pub fn object_wait_state(
        &self,
        handle: RuntimeHandle,
        expected_kind: RuntimeObjectKind,
    ) -> Result<RuntimeWaitState, RuntimeStateError> {
        self.objects.wait_state(handle, expected_kind)
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
        self.threads.last_error(self.current_thread_id())
    }

    pub fn set_current_last_error(&mut self, value: u32) -> Result<(), RuntimeStateError> {
        let plan = self
            .threads
            .plan_last_error(self.current_thread_id(), value)?;
        self.threads.commit(plan);
        Ok(())
    }

    pub fn scheduler_state(&self) -> Result<SchedulerThreadState, RuntimeStateError> {
        self.scheduler.current_state()
    }

    pub fn scheduler_state_for_thread(
        &self,
        thread_id: RuntimeThreadId,
    ) -> Result<SchedulerThreadState, RuntimeStateError> {
        self.scheduler.state(thread_id)
    }

    pub fn start_initial_thread(&mut self) -> Result<(), RuntimeStateError> {
        let thread_id = self.current_thread_id();
        let thread_plan = self.threads.plan_start(thread_id)?;
        if self.process.lifecycle != RuntimeProcessLifecycleState::Bootstrapping
            || self.scheduler_state()? != SchedulerThreadState::Initialized
        {
            return Err(RuntimeStateError::conflict(
                "runtime-lifecycle",
                "initial-thread-start-transition-invalid",
                "initial thread start requires matching process, thread, and scheduler bootstrap states",
            ));
        }
        let scheduler_plan = self
            .scheduler
            .plan_transition(thread_id, SchedulerThreadState::Runnable)?;
        self.process.lifecycle = RuntimeProcessLifecycleState::Running;
        self.threads.commit(thread_plan);
        self.scheduler.commit_lane(scheduler_plan);
        Ok(())
    }

    pub fn register_thread(
        &mut self,
        thread_id: RuntimeThreadId,
        initial_last_error: u32,
    ) -> Result<RuntimeHandle, RuntimeStateError> {
        if self.process.lifecycle != RuntimeProcessLifecycleState::Running {
            return Err(RuntimeStateError::conflict(
                "runtime-thread-registry",
                "thread-registration-process-state-invalid",
                "thread registration requires a running process",
            ));
        }
        let thread_registered = self.threads.contains(thread_id);
        let scheduler_registered = self.scheduler.contains_thread(thread_id);
        if thread_registered != scheduler_registered {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-thread-registry",
                "thread-registry-scheduler-mismatch",
                "thread and scheduler registries disagree about a thread identity",
            ));
        }
        let thread_plan = self
            .threads
            .plan_registration(thread_id, initial_last_error)?;
        let scheduler_plan = self
            .scheduler
            .plan_registration(thread_id, SchedulerThreadState::Runnable)?;
        self.messages.preflight_register_thread(thread_id)?;
        let thread_key = RuntimeObjectKey::Thread {
            thread_id: thread_id.0,
        };
        if self.objects.contains_identity(&thread_key) {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-thread-registry",
                "thread-registry-object-orphan",
                "thread object identity exists without a thread registry record",
            ));
        }
        let (_, handle) = self.objects.register_object_and_open_handle(thread_key)?;
        self.threads.commit(thread_plan);
        self.scheduler.commit_lane(scheduler_plan);
        self.messages.commit_register_thread(thread_id);
        Ok(handle)
    }

    pub fn select_thread(&mut self, thread_id: RuntimeThreadId) -> Result<(), RuntimeStateError> {
        if self.process.lifecycle != RuntimeProcessLifecycleState::Running {
            return Err(RuntimeStateError::conflict(
                "runtime-thread-registry",
                "thread-selection-process-state-invalid",
                "thread selection requires a running process",
            ));
        }
        let thread = self.thread_state(thread_id)?;
        let scheduler_state = self.scheduler_state_for_thread(thread_id)?;
        if thread.lifecycle != RuntimeThreadLifecycleState::Running
            || !matches!(
                scheduler_state,
                SchedulerThreadState::Runnable
                    | SchedulerThreadState::WaitingHost { .. }
                    | SchedulerThreadState::WaitingObject { .. }
                    | SchedulerThreadState::WaitingMessage
                    | SchedulerThreadState::InCallback { .. }
            )
        {
            return Err(RuntimeStateError::conflict(
                "runtime-thread-registry",
                "thread-selection-state-invalid",
                "thread selection requires matching live thread and scheduler states",
            ));
        }
        let thread_key = RuntimeObjectKey::Thread {
            thread_id: thread_id.0,
        };
        if !self.objects.contains_identity(&thread_key) {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-thread-registry",
                "thread-registry-object-missing",
                "registered thread has no runtime object identity",
            ));
        }
        let scheduler_plan = self.scheduler.plan_selection(thread_id)?;
        self.scheduler.commit_selection(scheduler_plan);
        Ok(())
    }

    pub fn wait_current_thread_on(
        &mut self,
        handle: RuntimeHandle,
    ) -> Result<RuntimeWaitDisposition, RuntimeStateError> {
        if self.process.lifecycle != RuntimeProcessLifecycleState::Running
            || self.current_thread()?.lifecycle != RuntimeThreadLifecycleState::Running
            || self.scheduler_state()? != SchedulerThreadState::Runnable
        {
            return Err(RuntimeStateError::conflict(
                "runtime-wait",
                "wait-registration-thread-state-invalid",
                "wait registration requires a running process and runnable current thread",
            ));
        }
        let thread_id = self.current_thread_id();
        if self
            .scheduler
            .wait_registration_for_thread(thread_id)?
            .is_some()
        {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-wait",
                "wait-registration-thread-duplicate",
                "runnable thread already owns a wait registration",
            ));
        }
        let kind = handle.kind()?;
        let object = self.objects.object_for_handle(handle, kind)?.clone();
        if kind == RuntimeObjectKind::Mutex {
            let RuntimeObjectKey::Mutex { mutex_id } = &object.key else {
                return Err(RuntimeStateError::runtime_defect(
                    "runtime-mutex",
                    "mutex-object-key-invalid",
                    "mutex wait resolved to a non-mutex object identity",
                ));
            };
            let mutex_id = RuntimeMutexId(*mutex_id);
            let plan = self
                .process
                .synchronization
                .plan_mutex_wait(mutex_id, thread_id)?;
            let recursion_count = plan.recursion_count();
            let abandoned = plan.abandoned_acquisition();
            self.process.synchronization.commit_mutex_transition(plan);
            return Ok(RuntimeWaitDisposition::MutexAcquired {
                object_id: object.id,
                mutex_id,
                recursion_count,
                abandoned,
            });
        }
        let wait_state = object.wait_state.ok_or_else(|| {
            RuntimeStateError::invalid_input(
                "runtime-wait",
                "runtime-object-not-waitable",
                "runtime object type does not expose a wait state",
            )
        })?;
        if matches!(wait_state, RuntimeWaitState::Signaled { .. }) {
            return Ok(RuntimeWaitDisposition::AlreadySignaled {
                object_id: object.id,
                wait_state,
            });
        }
        let wait_plan = self
            .scheduler
            .plan_wait_registration(thread_id, object.id, handle)?;
        let registration = wait_plan.registration();
        let scheduler_plan = self.scheduler.plan_transition(
            thread_id,
            SchedulerThreadState::WaitingObject {
                registration_id: registration.id,
                object_id: object.id,
            },
        )?;
        self.scheduler.commit_wait_registration(wait_plan);
        self.scheduler.commit_lane(scheduler_plan);
        Ok(RuntimeWaitDisposition::Pending { registration })
    }

    pub fn post_thread_message(
        &mut self,
        target_thread_id: RuntimeThreadId,
        hwnd: u32,
        message: u32,
        w_param: u32,
        l_param: u32,
    ) -> Result<RuntimeThreadMessage, RuntimeStateError> {
        if self.process.lifecycle != RuntimeProcessLifecycleState::Running
            || self.current_thread()?.lifecycle != RuntimeThreadLifecycleState::Running
            || !matches!(
                self.scheduler_state()?,
                SchedulerThreadState::Runnable | SchedulerThreadState::InCallback { .. }
            )
        {
            return Err(RuntimeStateError::conflict(
                "runtime-message",
                "message-post-source-state-invalid",
                "message post requires a running process and executing source thread",
            ));
        }
        let source_thread_id = self.current_thread_id();
        let target_thread = self.thread_state(target_thread_id)?;
        let target_scheduler_state = self.scheduler_state_for_thread(target_thread_id)?;
        if target_thread.lifecycle != RuntimeThreadLifecycleState::Running
            || !matches!(
                target_scheduler_state,
                SchedulerThreadState::Runnable
                    | SchedulerThreadState::WaitingHost { .. }
                    | SchedulerThreadState::WaitingObject { .. }
                    | SchedulerThreadState::WaitingMessage
                    | SchedulerThreadState::InCallback { .. }
            )
        {
            return Err(RuntimeStateError::conflict(
                "runtime-message",
                "message-post-target-state-invalid",
                "message target requires matching live thread and scheduler states",
            ));
        }
        let plan = self.messages.plan_post(
            source_thread_id,
            target_thread_id,
            hwnd,
            message,
            w_param,
            l_param,
            target_scheduler_state == SchedulerThreadState::WaitingMessage,
        )?;
        let posted = plan.message;
        let scheduler_plan = plan
            .wake_target
            .then(|| {
                self.scheduler
                    .plan_transition(target_thread_id, SchedulerThreadState::Runnable)
            })
            .transpose()?;
        self.messages.commit_post(plan);
        if let Some(scheduler_plan) = scheduler_plan {
            self.scheduler.commit_lane(scheduler_plan);
            self.scheduler
                .commit_event(RuntimeSchedulerEvent::MessageReady(RuntimeMessageWake {
                    thread_id: target_thread_id,
                    message_id: posted.id,
                    queue_depth: plan.queue_depth,
                }));
        }
        Ok(posted)
    }

    pub fn receive_current_thread_message(
        &mut self,
    ) -> Result<RuntimeMessageReceiveDisposition, RuntimeStateError> {
        if self.process.lifecycle != RuntimeProcessLifecycleState::Running
            || self.current_thread()?.lifecycle != RuntimeThreadLifecycleState::Running
            || self.scheduler_state()? != SchedulerThreadState::Runnable
        {
            return Err(RuntimeStateError::conflict(
                "runtime-message",
                "message-receive-thread-state-invalid",
                "message receive requires a running process and runnable owner thread",
            ));
        }
        let thread_id = self.current_thread_id();
        let plan = self.messages.plan_receive(thread_id)?;
        if let RuntimeMessageReceivePlan::Message { message, .. } = plan {
            self.messages.commit_receive(thread_id, message.id);
            return Ok(RuntimeMessageReceiveDisposition::Message { message });
        }
        let scheduler_plan = self
            .scheduler
            .plan_transition(thread_id, SchedulerThreadState::WaitingMessage)?;
        self.scheduler.commit_lane(scheduler_plan);
        Ok(RuntimeMessageReceiveDisposition::Blocked)
    }

    pub fn queued_thread_messages(
        &self,
        thread_id: RuntimeThreadId,
    ) -> Result<Vec<RuntimeThreadMessage>, RuntimeStateError> {
        self.thread_state(thread_id)?;
        self.messages.snapshot(thread_id)
    }

    pub fn set_heap_termination_on_corruption(&mut self, enabled: bool) {
        self.process.heap_termination_on_corruption = enabled;
    }

    pub fn ensure_standard_handle(
        &mut self,
        selector: u32,
    ) -> Result<&RuntimeStandardHandle, RuntimeStateError> {
        let (handle, variant_id) = match selector {
            RUNTIME_STANDARD_INPUT_SELECTOR => (
                RUNTIME_STANDARD_INPUT_HANDLE,
                RUNTIME_STANDARD_INPUT_VARIANT,
            ),
            RUNTIME_STANDARD_OUTPUT_SELECTOR => (
                RUNTIME_STANDARD_OUTPUT_HANDLE,
                RUNTIME_STANDARD_OUTPUT_VARIANT,
            ),
            RUNTIME_STANDARD_ERROR_SELECTOR => (
                RUNTIME_STANDARD_ERROR_HANDLE,
                RUNTIME_STANDARD_ERROR_VARIANT,
            ),
            _ => {
                return Err(RuntimeStateError::invalid_input(
                    "runtime-standard-handle",
                    "standard-handle-selector-unsupported",
                    "only the reached standard input, output, and error selectors are supported",
                ));
            }
        };
        if self.process.lifecycle != RuntimeProcessLifecycleState::Running
            || self.current_thread()?.lifecycle != RuntimeThreadLifecycleState::Running
        {
            return Err(RuntimeStateError::conflict(
                "runtime-standard-handle",
                "standard-handle-lifecycle-state-invalid",
                "standard handle lookup requires a running process and thread",
            ));
        }
        Ok(self
            .process
            .standard_handles
            .entry(selector)
            .or_insert_with(|| RuntimeStandardHandle {
                owner: RUNTIME_STANDARD_HANDLE_OWNER.to_owned(),
                variant_id: variant_id.to_owned(),
                namespace: LOGICAL_STANDARD_HANDLE_NAMESPACE.to_owned(),
                handle,
                selector,
            }))
    }

    pub fn standard_handle_by_value(
        &self,
        handle: u32,
    ) -> Result<&RuntimeStandardHandle, RuntimeStateError> {
        self.process
            .standard_handles
            .values()
            .find(|standard_handle| standard_handle.handle == handle)
            .ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-standard-handle",
                    "standard-handle-unknown",
                    format!("logical standard handle {handle:#010x} is not registered"),
                )
            })
    }

    pub fn create_heap(&mut self, heap: RuntimeHeap) -> Result<&RuntimeHeap, RuntimeStateError> {
        let handle = self.commit_heap_creation(heap)?;
        Ok(self
            .process
            .heaps
            .heap(handle)
            .expect("committed runtime heap disappeared"))
    }

    fn commit_heap_creation(&mut self, heap: RuntimeHeap) -> Result<u32, RuntimeStateError> {
        let handle = heap.handle;
        let plan = self.process.heaps.plan_creation(heap)?;
        self.objects.register_object_with_handle(
            RuntimeObjectKey::PrivateHeap { handle },
            RuntimeHandle(handle),
        )?;
        self.process.heaps.commit_creation(plan);
        Ok(handle)
    }

    pub fn allocate_private_heap(
        &mut self,
        creation_flags: u32,
        initial_size: u32,
        maximum_size: u32,
    ) -> Result<&RuntimeHeap, RuntimeStateError> {
        let mut next_slot = self.next_private_heap_slot;
        loop {
            let slot = next_slot;
            if slot == 0 {
                return Err(RuntimeStateError::resource_limit(
                    "runtime-heap",
                    "private-heap-handle-space-exhausted",
                    "logical private heap handle space is exhausted",
                ));
            }
            next_slot = slot.checked_add(1).unwrap_or(0);
            let handle = LOGICAL_PRIVATE_HEAP_HANDLE_PREFIX | u32::from(slot);
            if self.process.heaps.contains(handle) {
                continue;
            }
            self.commit_heap_creation(RuntimeHeap {
                owner: RUNTIME_PRIVATE_HEAP_OWNER.to_owned(),
                variant_id: RUNTIME_PRIVATE_HEAP_VARIANT.to_owned(),
                namespace: LOGICAL_PRIVATE_HEAP_NAMESPACE.to_owned(),
                handle,
                creation_flags,
                initial_size,
                maximum_size,
                growable: maximum_size == 0,
                live_allocations: 0,
            })?;
            self.next_private_heap_slot = next_slot;
            return Ok(self
                .process
                .heaps
                .heap(handle)
                .expect("committed private heap disappeared"));
        }
    }

    pub fn heap(&self, handle: u32) -> Result<&RuntimeHeap, RuntimeStateError> {
        self.objects
            .object_for_handle(RuntimeHandle(handle), RuntimeObjectKind::PrivateHeap)?;
        self.process.heaps.heap(handle)
    }

    pub fn heap_backing(
        &self,
        handle: u32,
    ) -> Result<Option<&RuntimeHeapBacking>, RuntimeStateError> {
        self.heap(handle)?;
        Ok(self.process.heaps.backing(handle))
    }

    pub fn private_heap_allocation(
        &self,
        handle: u32,
        address: GuestAddress,
    ) -> Result<RuntimeHeapAllocation, RuntimeStateError> {
        self.require_private_heap_operation_state()?;
        self.heap(handle)?;
        self.process.heaps.allocation(handle, address)
    }

    pub(crate) fn plan_private_heap_allocation(
        &self,
        handle: u32,
        byte_length: u32,
        initialization: RuntimeHeapInitialization,
    ) -> Result<RuntimeHeapAllocationPlan, RuntimeStateError> {
        self.require_private_heap_operation_state()?;
        if byte_length == 0 {
            return Err(RuntimeStateError::invalid_input(
                "runtime-heap-backing",
                "heap-allocation-size-zero",
                "private heap backing does not accept zero-byte allocations",
            ));
        }
        self.heap(handle)?;
        self.process.heaps.plan_allocation(
            &self.address_space_allocator,
            handle,
            byte_length,
            initialization,
        )
    }

    pub(crate) fn commit_private_heap_allocation(
        &mut self,
        plan: RuntimeHeapAllocationPlan,
        new_memory_owner: Option<MemoryOwnerId>,
    ) -> Result<RuntimeHeapAllocation, RuntimeStateError> {
        self.process.heaps.commit_allocation(plan, new_memory_owner)
    }

    pub(crate) fn plan_private_heap_free(
        &self,
        handle: u32,
        address: GuestAddress,
    ) -> Result<RuntimeHeapFreePlan, RuntimeStateError> {
        self.require_private_heap_operation_state()?;
        self.heap(handle)?;
        self.process.heaps.plan_free(handle, address)
    }

    pub(crate) fn commit_private_heap_free(
        &mut self,
        plan: RuntimeHeapFreePlan,
    ) -> Result<RuntimeHeapAllocation, RuntimeStateError> {
        self.process.heaps.commit_free(plan)
    }

    pub(crate) fn plan_private_heap_destruction(
        &self,
        handle: u32,
    ) -> Result<RuntimeHeapDestructionPlan, RuntimeStateError> {
        self.require_private_heap_operation_state()?;
        self.heap(handle)?;
        self.process
            .heaps
            .plan_destruction(&self.address_space_allocator, handle)
    }

    pub(crate) fn plan_process_heap_teardown(
        &self,
    ) -> Result<Vec<RuntimeHeapDestructionPlan>, RuntimeStateError> {
        if !matches!(
            self.process.lifecycle,
            RuntimeProcessLifecycleState::ExitRequested { .. }
        ) {
            return Err(RuntimeStateError::conflict(
                "runtime-heap-backing",
                "process-heap-teardown-state-invalid",
                "process heap teardown requires an active process exit request",
            ));
        }
        for handle in self.process.heaps.handles() {
            self.objects
                .object_for_handle(RuntimeHandle(handle), RuntimeObjectKind::PrivateHeap)?;
        }
        self.process
            .heaps
            .plan_all_destructions(&self.address_space_allocator)
    }

    pub(crate) fn commit_private_heap_destruction(
        &mut self,
        plans: &[RuntimeHeapDestructionPlan],
    ) -> Result<RuntimeHeapTeardown, RuntimeStateError> {
        let teardown = self.process.heaps.preflight_destruction(plans)?;
        let keys: Vec<_> = teardown
            .handles
            .iter()
            .map(|handle| RuntimeObjectKey::PrivateHeap { handle: *handle })
            .collect();
        self.objects.retire_subsystem_objects(&keys)?;
        self.process.heaps.commit_destruction(&teardown);
        Ok(teardown)
    }

    fn require_private_heap_operation_state(&self) -> Result<(), RuntimeStateError> {
        if self.process.lifecycle != RuntimeProcessLifecycleState::Running
            || self.current_thread()?.lifecycle != RuntimeThreadLifecycleState::Running
        {
            return Err(RuntimeStateError::conflict(
                "runtime-heap-backing",
                "heap-operation-lifecycle-state-invalid",
                "private heap allocation operations require a running process and thread",
            ));
        }
        Ok(())
    }

    pub fn load_module(
        &mut self,
        module: RuntimeModule,
    ) -> Result<&RuntimeModule, RuntimeStateError> {
        let class = if module.runtime_system_module {
            RuntimeModuleClass::RuntimeSystemModule
        } else {
            RuntimeModuleClass::GuestDynamicLibrary
        };
        self.register_module_record(module, class)
    }

    pub fn register_main_module(
        &mut self,
        module: RuntimeModule,
    ) -> Result<&RuntimeModule, RuntimeStateError> {
        if module.runtime_system_module {
            return Err(RuntimeStateError::invalid_input(
                "runtime-module",
                "main-module-class-invalid",
                "main executable cannot be registered as a runtime system module",
            ));
        }
        self.register_module_record(module, RuntimeModuleClass::MainExecutable)
    }

    fn register_module_record(
        &mut self,
        module: RuntimeModule,
        class: RuntimeModuleClass,
    ) -> Result<&RuntimeModule, RuntimeStateError> {
        self.require_dll_operation_inactive_for_module_graph_mutation()?;
        let handle = module.handle;
        let plan = self
            .modules
            .plan_registration(&self.process.module_records, module, class)?;
        self.objects.register_object_with_handle(
            RuntimeObjectKey::Module { handle },
            RuntimeHandle(handle),
        )?;
        self.modules
            .commit_registration(&mut self.process.module_records, plan);
        Ok(self
            .process
            .module_records
            .module(handle)
            .expect("committed runtime module disappeared"))
    }

    pub fn module(&self, handle: u32) -> Result<&RuntimeModule, RuntimeStateError> {
        self.objects
            .object_for_handle(RuntimeHandle(handle), RuntimeObjectKind::Module)?;
        self.process.module_records.module(handle)
    }

    pub fn module_by_name(&self, name: &str) -> Result<&RuntimeModule, RuntimeStateError> {
        let handle = self.modules.module_by_name_handle(name)?;
        self.module(handle)
    }

    pub fn add_module_dependency(
        &mut self,
        requester_handle: u32,
        dependency_handle: u32,
    ) -> Result<(), RuntimeStateError> {
        self.require_dll_operation_inactive_for_module_graph_mutation()?;
        self.module(requester_handle)?;
        self.module(dependency_handle)?;
        self.modules
            .add_dependency(requester_handle, dependency_handle)
    }

    pub fn add_current_thread_module_reference(
        &mut self,
        handle: u32,
    ) -> Result<u32, RuntimeStateError> {
        self.require_dll_operation_inactive_for_module_graph_mutation()?;
        self.module(handle)?;
        self.modules
            .add_dynamic_reference(handle, self.current_thread_id().0)
    }

    pub fn release_current_thread_module_reference(
        &mut self,
        handle: u32,
    ) -> Result<u32, RuntimeStateError> {
        self.require_dll_operation_inactive_for_module_graph_mutation()?;
        self.module(handle)?;
        self.modules
            .release_dynamic_reference(handle, self.current_thread_id().0)
    }

    pub fn mark_module_ready(&mut self, handle: u32) -> Result<(), RuntimeStateError> {
        self.require_dll_operation_inactive_for_module_graph_mutation()?;
        self.module(handle)?;
        self.modules.mark_ready(handle)
    }

    pub fn attach_modules(&mut self) -> Result<Vec<RuntimeModuleId>, RuntimeStateError> {
        self.modules.attach_all()
    }

    pub fn detach_modules(&mut self) -> Result<Vec<RuntimeModuleId>, RuntimeStateError> {
        self.modules.detach_all()
    }

    fn require_dll_operation_inactive_for_module_graph_mutation(
        &self,
    ) -> Result<(), RuntimeStateError> {
        self.modules.require_graph_mutation_allowed()
    }

    pub fn dll_lifecycle_phase(&self) -> RuntimeDllLifecyclePhase {
        self.modules.dll_lifecycle_phase()
    }

    pub fn begin_dll_process_attach(
        &mut self,
        return_sentinel: GuestAddress,
    ) -> Result<RuntimeDllLifecycleAction, RuntimeStateError> {
        self.modules.require_process_attach_start()?;
        self.require_dll_callback_thread_state()?;
        let plan = self.modules.plan_process_attach(return_sentinel)?;
        self.commit_dll_lifecycle_plan(None, plan)
    }

    pub fn begin_dll_process_detach(
        &mut self,
        return_sentinel: GuestAddress,
    ) -> Result<RuntimeDllLifecycleAction, RuntimeStateError> {
        self.modules.require_process_detach_start()?;
        self.require_dll_callback_thread_state()?;
        let plan = self.modules.plan_process_detach(return_sentinel)?;
        self.commit_dll_lifecycle_plan(None, plan)
    }

    pub fn complete_dll_callback(
        &mut self,
        frame_id: RuntimeCallbackFrameId,
        result: u32,
    ) -> Result<RuntimeDllLifecycleAction, RuntimeStateError> {
        let plan = self.modules.plan_callback_completion(frame_id, result)?;
        self.commit_dll_lifecycle_plan(Some(frame_id), plan)
    }

    fn require_dll_callback_thread_state(&self) -> Result<(), RuntimeStateError> {
        if self.process.lifecycle != RuntimeProcessLifecycleState::Running
            || self.current_thread()?.lifecycle != RuntimeThreadLifecycleState::Running
            || self.scheduler_state()? != SchedulerThreadState::Runnable
        {
            return Err(RuntimeStateError::conflict(
                "runtime-dll-lifecycle",
                "dll-callback-thread-state-invalid",
                "DLL process callbacks require a running process and runnable selected thread",
            ));
        }
        Ok(())
    }

    fn commit_dll_lifecycle_plan(
        &mut self,
        completed_frame: Option<RuntimeCallbackFrameId>,
        plan: RuntimeDllLifecyclePlan,
    ) -> Result<RuntimeDllLifecycleAction, RuntimeStateError> {
        self.modules.validate_lifecycle_plan(&plan)?;
        let callback_plan = match (completed_frame, plan.callback_entry()) {
            (None, Some((target, return_sentinel))) => {
                Some(self.plan_callback_push(target, return_sentinel)?)
            }
            (Some(expected), Some((target, return_sentinel))) => {
                Some(self.plan_callback_replace(expected, target, return_sentinel)?)
            }
            (Some(expected), None) => Some(self.plan_callback_pop(expected)?),
            (None, None) => None,
        };
        self.modules.validate_lifecycle_plan(&plan)?;
        let entered =
            callback_plan.and_then(|callback_plan| self.commit_callback_mutation(callback_plan).0);
        Ok(self.modules.commit_lifecycle_plan(plan, entered))
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

    pub fn allocate_fls_slot(
        &mut self,
        callback: Option<GuestAddress>,
    ) -> Result<&RuntimeFlsSlot, RuntimeStateError> {
        if self.process.lifecycle != RuntimeProcessLifecycleState::Running
            || self.current_thread()?.lifecycle != RuntimeThreadLifecycleState::Running
            || !matches!(
                self.scheduler_state()?,
                SchedulerThreadState::Runnable | SchedulerThreadState::InCallback { .. }
            )
        {
            return Err(RuntimeStateError::conflict(
                "runtime-fls",
                "fls-allocation-lifecycle-state-invalid",
                "FLS allocation requires a running process and active current thread",
            ));
        }
        if self.next_fls_index >= RUNTIME_FLS_INDEX_LIMIT {
            return Err(RuntimeStateError::resource_limit(
                "runtime-fls",
                "fls-index-space-exhausted",
                format!("logical FLS allocation is limited to {RUNTIME_FLS_INDEX_LIMIT} slots"),
            ));
        }
        let index = self.next_fls_index;
        if self.process.fls_slots.contains_key(&index) {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-fls",
                "fls-index-allocator-collision",
                "logical FLS allocator selected an already allocated index",
            ));
        }
        let thread_plan = self
            .threads
            .plan_initialize_fls_storage(self.current_thread_id())?;
        self.next_fls_index = index.checked_add(1).ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-fls",
                "fls-index-counter-overflow",
                "logical FLS index counter overflowed",
            )
        })?;
        self.process.fls_slots.insert(
            index,
            RuntimeFlsSlot {
                namespace: LOGICAL_FLS_INDEX_NAMESPACE.to_owned(),
                index,
                callback,
            },
        );
        self.threads.commit(thread_plan);
        self.process.fls_slots.get(&index).ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-fls",
                "inserted-fls-slot-missing",
                "logical FLS slot disappeared after insertion",
            )
        })
    }

    pub fn fls_slot(&self, index: u32) -> Result<&RuntimeFlsSlot, RuntimeStateError> {
        self.process.fls_slots.get(&index).ok_or_else(|| {
            RuntimeStateError::not_found(
                "runtime-fls",
                "fls-index-unallocated",
                format!("logical FLS index {index} is not allocated"),
            )
        })
    }

    pub fn set_fls_value(
        &mut self,
        index: u32,
        value: u32,
    ) -> Result<Option<u32>, RuntimeStateError> {
        if self.process.lifecycle != RuntimeProcessLifecycleState::Running
            || self.current_thread()?.lifecycle != RuntimeThreadLifecycleState::Running
            || !matches!(
                self.scheduler_state()?,
                SchedulerThreadState::Runnable | SchedulerThreadState::InCallback { .. }
            )
        {
            return Err(RuntimeStateError::conflict(
                "runtime-fls",
                "fls-value-lifecycle-state-invalid",
                "FLS value storage requires a running process and active current thread",
            ));
        }
        if !self.process.fls_slots.contains_key(&index) {
            return Err(RuntimeStateError::not_found(
                "runtime-fls",
                "fls-index-unallocated",
                format!("logical FLS index {index} is not allocated"),
            ));
        }
        let (thread_plan, replaced) =
            self.threads
                .plan_fls_value(self.current_thread_id(), index, value)?;
        self.threads.commit(thread_plan);
        Ok(replaced)
    }

    pub fn fls_value(&self, index: u32) -> Result<u32, RuntimeStateError> {
        self.fls_value_or_unset(index)?.ok_or_else(|| {
            RuntimeStateError::not_found(
                "runtime-fls",
                "fls-value-uninitialized",
                format!("logical FLS index {index} has no value for the current thread"),
            )
        })
    }

    pub fn fls_value_or_unset(&self, index: u32) -> Result<Option<u32>, RuntimeStateError> {
        self.fls_slot(index)?;
        self.threads
            .fls_value_or_unset(self.current_thread_id(), index)
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
        let plan = self
            .threads
            .plan_tls_value(self.current_thread_id(), index, tls_value)?;
        self.threads.commit(plan);
        Ok(())
    }

    pub fn tls_value(&self, index: u32) -> Result<&RuntimeTlsValue, RuntimeStateError> {
        self.tls_value_or_unset(index)?.ok_or_else(|| {
            RuntimeStateError::not_found(
                "runtime-tls",
                "tls-value-uninitialized",
                format!("current thread has no value for logical TLS index {index}"),
            )
        })
    }

    pub fn tls_value_or_unset(
        &self,
        index: u32,
    ) -> Result<Option<&RuntimeTlsValue>, RuntimeStateError> {
        if !self.process.allocated_tls_indices.contains(&index) {
            return Err(RuntimeStateError::not_found(
                "runtime-tls",
                "tls-index-unallocated",
                format!("logical TLS index {index} is not allocated"),
            ));
        }
        self.threads
            .tls_value_or_unset(self.current_thread_id(), index)
    }

    pub fn initialized_tls_thread_count(&self, index: u32) -> usize {
        self.threads.initialized_tls_thread_count(index)
    }

    pub fn create_mutex_a(
        &mut self,
        name_a: Option<RuntimeMutexNameA>,
        initial_owner: bool,
    ) -> Result<RuntimeCreateMutexAResult, RuntimeStateError> {
        self.require_mutex_operation_state()?;
        let current_thread_id = self.current_thread_id();
        let plan = self.process.synchronization.plan_mutex_creation(
            name_a,
            initial_owner,
            current_thread_id,
        )?;
        let mutex_id = plan.id();
        let already_exists = plan.already_exists();
        let key = RuntimeObjectKey::Mutex {
            mutex_id: mutex_id.0,
        };
        let mut next_objects = self.objects.clone();
        let handle = if already_exists {
            next_objects.open_handle(&key)?
        } else {
            next_objects.register_object_and_open_handle(key)?.1
        };
        let mut next_synchronization = self.process.synchronization.clone();
        next_synchronization.commit_mutex_creation(plan);
        self.objects = next_objects;
        self.process.synchronization = next_synchronization;
        Ok(RuntimeCreateMutexAResult {
            mutex_id,
            handle,
            already_exists,
        })
    }

    pub fn mutex_for_handle(
        &self,
        handle: RuntimeHandle,
    ) -> Result<&RuntimeMutex, RuntimeStateError> {
        let record = self
            .objects
            .object_for_handle(handle, RuntimeObjectKind::Mutex)?;
        let RuntimeObjectKey::Mutex { mutex_id } = &record.key else {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-mutex",
                "mutex-object-key-invalid",
                "mutex handle resolved to a non-mutex object identity",
            ));
        };
        self.process
            .synchronization
            .mutex(RuntimeMutexId(*mutex_id))
    }

    pub fn release_mutex(
        &mut self,
        handle: RuntimeHandle,
    ) -> Result<&RuntimeMutex, RuntimeStateError> {
        self.require_mutex_operation_state()?;
        let id = self.mutex_for_handle(handle)?.id;
        let plan = self
            .process
            .synchronization
            .plan_mutex_release(id, self.current_thread_id())?;
        Ok(self.process.synchronization.commit_mutex_transition(plan))
    }

    fn require_mutex_operation_state(&self) -> Result<(), RuntimeStateError> {
        if self.process.lifecycle != RuntimeProcessLifecycleState::Running
            || self.current_thread()?.lifecycle != RuntimeThreadLifecycleState::Running
            || !matches!(
                self.scheduler_state()?,
                SchedulerThreadState::Runnable | SchedulerThreadState::InCallback { .. }
            )
        {
            return Err(RuntimeStateError::conflict(
                "runtime-mutex",
                "mutex-operation-state-invalid",
                "mutex operation requires a running process and active current thread",
            ));
        }
        Ok(())
    }

    fn teardown_mutexes(&mut self) -> Result<usize, RuntimeStateError> {
        let ids = self.process.synchronization.mutex_ids();
        let keys = ids
            .iter()
            .map(|id| RuntimeObjectKey::Mutex { mutex_id: id.0 })
            .collect::<Vec<_>>();
        let plans = ids
            .iter()
            .map(|id| self.process.synchronization.plan_mutex_retirement(*id))
            .collect::<Result<Vec<_>, _>>()?;
        let mut next_objects = self.objects.clone();
        let retired_handles = next_objects.retire_mutex_objects(&keys)?;
        let mut next_synchronization = self.process.synchronization.clone();
        for plan in plans {
            next_synchronization.commit_mutex_retirement(plan);
        }
        self.objects = next_objects;
        self.process.synchronization = next_synchronization;
        Ok(retired_handles)
    }

    pub fn initialize_critical_section(
        &mut self,
        address: GuestAddress,
        spin_count: u32,
    ) -> Result<&RuntimeCriticalSection, RuntimeStateError> {
        self.require_critical_section_operation_state()?;
        let plan = self
            .process
            .synchronization
            .plan_initialization(address, spin_count)?;
        if spin_count > 0x00ff_ffff {
            return Err(RuntimeStateError::invalid_input(
                "runtime-critical-section",
                "critical-section-spin-count-invalid",
                "critical section spin count exceeds the Win32 24-bit range",
            ));
        }
        Ok(self.process.synchronization.commit(plan))
    }

    pub fn initialize_critical_section_with_spin_state(
        &mut self,
        address: GuestAddress,
        spin_count_field: u32,
    ) -> Result<&RuntimeCriticalSection, RuntimeStateError> {
        self.require_critical_section_operation_state()?;
        let plan = self
            .process
            .synchronization
            .plan_initialization(address, spin_count_field)?;
        Ok(self.process.synchronization.commit(plan))
    }

    pub fn enter_critical_section(
        &mut self,
        address: GuestAddress,
    ) -> Result<&RuntimeCriticalSection, RuntimeStateError> {
        self.require_critical_section_operation_state()?;
        let current_thread_id = self.current_thread_id();
        let plan = self
            .process
            .synchronization
            .plan_enter(address, current_thread_id)?;
        Ok(self.process.synchronization.commit(plan))
    }

    pub fn leave_critical_section(
        &mut self,
        address: GuestAddress,
    ) -> Result<&RuntimeCriticalSection, RuntimeStateError> {
        self.require_critical_section_operation_state()?;
        let current_thread_id = self.current_thread_id();
        let plan = self
            .process
            .synchronization
            .plan_leave(address, current_thread_id)?;
        Ok(self.process.synchronization.commit(plan))
    }

    pub fn critical_section(
        &self,
        address: GuestAddress,
    ) -> Result<&RuntimeCriticalSection, RuntimeStateError> {
        self.process.synchronization.critical_section(address)
    }

    fn require_critical_section_operation_state(&self) -> Result<(), RuntimeStateError> {
        if self.process.lifecycle != RuntimeProcessLifecycleState::Running
            || self.current_thread()?.lifecycle != RuntimeThreadLifecycleState::Running
            || !matches!(
                self.scheduler_state()?,
                SchedulerThreadState::Runnable | SchedulerThreadState::InCallback { .. }
            )
        {
            return Err(RuntimeStateError::conflict(
                "runtime-critical-section",
                "critical-section-lifecycle-state-invalid",
                "critical section operations require a running process and active current thread",
            ));
        }
        Ok(())
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

    pub fn decode_pointer(&self, token: u32) -> Result<&RuntimeEncodedPointer, RuntimeStateError> {
        let token_id = token & 0xffff;
        if token_id == 0 || token & 0xffff_0000 != self.encoded_pointer_token_prefix {
            return Err(RuntimeStateError::invalid_input(
                "runtime-pointer-encoding",
                "encoded-pointer-token-invalid",
                format!("pointer token {token:#010x} is outside the runtime-owned token namespace"),
            ));
        }
        let encoded = self
            .registries
            .encoded_pointers
            .get(&token)
            .ok_or_else(|| {
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

    pub fn push_callback_frame(
        &mut self,
        target: GuestAddress,
        return_sentinel: GuestAddress,
    ) -> Result<RuntimeCallbackFrame, RuntimeStateError> {
        let plan = self.plan_callback_push(target, return_sentinel)?;
        Ok(self
            .commit_callback_mutation(plan)
            .0
            .expect("callback push plan did not enter a frame"))
    }

    fn plan_callback_push(
        &self,
        target: GuestAddress,
        return_sentinel: GuestAddress,
    ) -> Result<RuntimeCallbackMutationPlan, RuntimeStateError> {
        let thread_id = self.current_thread_id();
        let current_depth = self.threads.callback_depth(thread_id)?;
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
        let next_callback_frame_id =
            self.next_callback_frame_id.checked_add(1).ok_or_else(|| {
                RuntimeStateError::resource_limit(
                    "runtime-callback",
                    "callback-frame-id-overflow",
                    "callback frame ID space is exhausted",
                )
            })?;
        let frame = RuntimeCallbackFrame {
            id,
            thread_id,
            target,
            return_sentinel,
            entry_last_error: self.current_last_error()?,
        };
        let (thread_plan, depth) = self.threads.plan_callback_push(thread_id, frame)?;
        let scheduler_plan = self
            .scheduler
            .plan_transition(thread_id, SchedulerThreadState::InCallback { depth })?;
        Ok(RuntimeCallbackMutationPlan {
            thread: thread_plan,
            scheduler: scheduler_plan,
            next_callback_frame_id,
            entered: Some(frame),
            exited: None,
        })
    }

    pub fn pop_callback_frame(
        &mut self,
        expected: RuntimeCallbackFrameId,
    ) -> Result<RuntimeCallbackFrame, RuntimeStateError> {
        let plan = self.plan_callback_pop(expected)?;
        Ok(self
            .commit_callback_mutation(plan)
            .1
            .expect("callback pop plan did not exit a frame"))
    }

    fn plan_callback_pop(
        &self,
        expected: RuntimeCallbackFrameId,
    ) -> Result<RuntimeCallbackMutationPlan, RuntimeStateError> {
        let thread_id = self.current_thread_id();
        let current_depth = self.threads.callback_depth(thread_id)?;
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
        let (thread_plan, frame, remaining_depth) =
            self.threads.plan_callback_pop(thread_id, expected)?;
        let scheduler_state = if remaining_depth == 0 {
            SchedulerThreadState::Runnable
        } else {
            SchedulerThreadState::InCallback {
                depth: remaining_depth,
            }
        };
        let scheduler_plan = self.scheduler.plan_transition(thread_id, scheduler_state)?;
        Ok(RuntimeCallbackMutationPlan {
            thread: thread_plan,
            scheduler: scheduler_plan,
            next_callback_frame_id: self.next_callback_frame_id,
            entered: None,
            exited: Some(frame),
        })
    }

    fn plan_callback_replace(
        &self,
        expected: RuntimeCallbackFrameId,
        target: GuestAddress,
        return_sentinel: GuestAddress,
    ) -> Result<RuntimeCallbackMutationPlan, RuntimeStateError> {
        let thread_id = self.current_thread_id();
        let current_depth = self.threads.callback_depth(thread_id)?;
        if self.scheduler_state()?
            != (SchedulerThreadState::InCallback {
                depth: current_depth,
            })
        {
            return Err(RuntimeStateError::conflict(
                "runtime-callback",
                "callback-return-thread-state-invalid",
                "callback replacement requires the scheduler to identify the current callback depth",
            ));
        }
        let id = RuntimeCallbackFrameId(self.next_callback_frame_id);
        let next_callback_frame_id =
            self.next_callback_frame_id.checked_add(1).ok_or_else(|| {
                RuntimeStateError::resource_limit(
                    "runtime-callback",
                    "callback-frame-id-overflow",
                    "callback frame ID space is exhausted",
                )
            })?;
        let frame = RuntimeCallbackFrame {
            id,
            thread_id,
            target,
            return_sentinel,
            entry_last_error: self.current_last_error()?,
        };
        let (thread_plan, exited, depth) = self
            .threads
            .plan_callback_replace(thread_id, expected, frame)?;
        let scheduler_plan = self
            .scheduler
            .plan_transition(thread_id, SchedulerThreadState::InCallback { depth })?;
        Ok(RuntimeCallbackMutationPlan {
            thread: thread_plan,
            scheduler: scheduler_plan,
            next_callback_frame_id,
            entered: Some(frame),
            exited: Some(exited),
        })
    }

    fn commit_callback_mutation(
        &mut self,
        plan: RuntimeCallbackMutationPlan,
    ) -> (Option<RuntimeCallbackFrame>, Option<RuntimeCallbackFrame>) {
        self.next_callback_frame_id = plan.next_callback_frame_id;
        self.threads.commit(plan.thread);
        self.scheduler.commit_lane(plan.scheduler);
        (plan.entered, plan.exited)
    }

    pub fn begin_host_request(
        &mut self,
        service: impl Into<String>,
    ) -> Result<RuntimeHostRequest, RuntimeStateError> {
        let plan = self.scheduler.plan_host_request(service.into())?;
        Ok(self.scheduler.commit_host_request(plan))
    }

    pub fn complete_host_request(
        &mut self,
        request_id: u64,
    ) -> Result<RuntimeHostCompletion, RuntimeStateError> {
        let plan = self.scheduler.plan_host_completion(request_id)?;
        Ok(self.scheduler.commit_host_completion(plan))
    }

    pub fn pop_scheduler_event(&mut self) -> Option<RuntimeSchedulerEvent> {
        self.scheduler.pop_event()
    }

    pub fn request_current_thread_exit(
        &mut self,
        exit_code: u32,
    ) -> Result<RuntimeCancellation, RuntimeStateError> {
        if self.process.lifecycle != RuntimeProcessLifecycleState::Running {
            return Err(RuntimeStateError::conflict(
                "runtime-lifecycle",
                "thread-exit-request-transition-invalid",
                "thread exit request requires a running process",
            ));
        }
        let plan = self.plan_thread_exit_request(self.current_thread_id(), exit_code)?;
        let cancellation = plan.cancellation;
        self.commit_thread_exit_request(plan);
        Ok(cancellation)
    }

    pub fn complete_current_thread_exit(&mut self) -> Result<(), RuntimeStateError> {
        if self.process.lifecycle != RuntimeProcessLifecycleState::Running {
            return Err(RuntimeStateError::conflict(
                "runtime-lifecycle",
                "thread-exit-completion-transition-invalid",
                "thread exit completion requires a running process",
            ));
        }
        let completion = self.plan_thread_exit_completion(self.current_thread_id())?;
        let wait_updates = self.objects.plan_wait_state_updates(&[(
            RuntimeObjectKey::Thread {
                thread_id: completion.thread_id.0,
            },
            RuntimeWaitState::Signaled {
                exit_code: completion.exit_code,
            },
        )])?;
        let wake_completions = self.plan_waiter_wakes(&wait_updates, &[completion.thread_id])?;
        self.objects.commit_wait_state_updates(wait_updates);
        self.commit_thread_exit_completion(completion);
        self.commit_waiter_wakes(wake_completions);
        Ok(())
    }

    pub fn cancel_current_thread(
        &mut self,
        exit_code: u32,
    ) -> Result<RuntimeCancellation, RuntimeStateError> {
        if self.process.lifecycle != RuntimeProcessLifecycleState::Running {
            return Err(RuntimeStateError::conflict(
                "runtime-lifecycle",
                "thread-exit-request-transition-invalid",
                "thread cancellation requires a running process",
            ));
        }
        let plan = self.plan_thread_exit_request(self.current_thread_id(), exit_code)?;
        let wait_updates = self.objects.plan_wait_state_updates(&[(
            RuntimeObjectKey::Thread {
                thread_id: plan.thread_id.0,
            },
            RuntimeWaitState::Signaled { exit_code },
        )])?;
        let wake_completions = self.plan_waiter_wakes(&wait_updates, &[plan.thread_id])?;
        let cancellation = plan.cancellation;
        let terminal_thread_plan = plan.thread_plan.terminal_after_request(exit_code)?;
        let terminal_scheduler_plan = plan
            .scheduler_plan
            .transition_after(SchedulerThreadState::Terminated { exit_code });
        self.objects.commit_wait_state_updates(wait_updates);
        self.commit_thread_exit_request(plan);
        self.threads.commit(terminal_thread_plan);
        self.scheduler.commit_lane(terminal_scheduler_plan);
        self.commit_waiter_wakes(wake_completions);
        Ok(cancellation)
    }

    pub fn request_current_process_exit(
        &mut self,
        exit_code: u32,
    ) -> Result<RuntimeProcessCancellation, RuntimeStateError> {
        if self.process.lifecycle != RuntimeProcessLifecycleState::Running {
            return Err(RuntimeStateError::conflict(
                "runtime-lifecycle",
                "process-exit-request-transition-invalid",
                "process exit request requires a running process",
            ));
        }
        let plan = self.plan_process_exit_request(exit_code)?;
        let cancellation = plan.cancellation.clone();
        self.commit_process_exit_request(plan);
        Ok(cancellation)
    }

    pub fn complete_current_process_exit(&mut self) -> Result<(), RuntimeStateError> {
        let exit_code = match self.process.lifecycle {
            RuntimeProcessLifecycleState::ExitRequested {
                exit_code,
                initiating_thread_id,
            } if initiating_thread_id == self.current_thread_id() => exit_code,
            _ => {
                return Err(RuntimeStateError::conflict(
                    "runtime-lifecycle",
                    "process-exit-completion-transition-invalid",
                    "process exit completion requires a matching prior exit request",
                ));
            }
        };
        if !self.process.heaps.is_empty() {
            return Err(RuntimeStateError::conflict(
                "runtime-heap-backing",
                "process-heaps-live-at-exit-completion",
                "process exit completion requires private heap teardown",
            ));
        }
        let completions = self.plan_process_exit_completion(exit_code)?;
        let mut wait_states = Vec::with_capacity(completions.len() + 1);
        for completion in &completions {
            wait_states.push((
                RuntimeObjectKey::Thread {
                    thread_id: completion.thread_id.0,
                },
                RuntimeWaitState::Signaled { exit_code },
            ));
        }
        wait_states.push((
            RuntimeObjectKey::Process {
                process_id: self.process.id.0,
            },
            RuntimeWaitState::Signaled { exit_code },
        ));
        let wait_updates = self.objects.plan_wait_state_updates(&wait_states)?;
        let excluded_threads: Vec<_> = completions
            .iter()
            .map(|completion| completion.thread_id)
            .collect();
        let wake_completions = self.plan_waiter_wakes(&wait_updates, &excluded_threads)?;
        if !wake_completions.is_empty() {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-wait",
                "process-exit-surviving-waiter",
                "process exit cannot leave a registered waiter to resume",
            ));
        }
        self.teardown_mutexes()?;
        self.objects.commit_wait_state_updates(wait_updates);
        for completion in completions {
            self.commit_thread_exit_completion(completion);
        }
        self.process.unhandled_exception_filter = None;
        self.process.lifecycle = RuntimeProcessLifecycleState::Exited { exit_code };
        Ok(())
    }

    pub fn terminate_current_process(
        &mut self,
        exit_code: u32,
    ) -> Result<RuntimeProcessCancellation, RuntimeStateError> {
        if self.process.lifecycle != RuntimeProcessLifecycleState::Running {
            return Err(RuntimeStateError::conflict(
                "runtime-lifecycle",
                "process-exit-request-transition-invalid",
                "process termination requires a running process",
            ));
        }
        if !self.process.heaps.is_empty() {
            return Err(RuntimeStateError::conflict(
                "runtime-heap-backing",
                "process-heaps-live-at-direct-termination",
                "direct process termination cannot bypass private heap teardown",
            ));
        }
        let plan = self.plan_process_exit_request(exit_code)?;
        let terminal_thread_plans = plan
            .thread_plans
            .iter()
            .map(|plan| plan.thread_plan.terminal_after_request(exit_code))
            .collect::<Result<Vec<_>, _>>()?;
        let terminal_scheduler_plans = plan
            .thread_plans
            .iter()
            .map(|plan| {
                plan.scheduler_plan
                    .transition_after(SchedulerThreadState::Terminated { exit_code })
            })
            .collect::<Vec<_>>();
        let mut wait_states = Vec::with_capacity(plan.thread_plans.len() + 1);
        for thread_plan in &plan.thread_plans {
            wait_states.push((
                RuntimeObjectKey::Thread {
                    thread_id: thread_plan.thread_id.0,
                },
                RuntimeWaitState::Signaled { exit_code },
            ));
        }
        wait_states.push((
            RuntimeObjectKey::Process {
                process_id: self.process.id.0,
            },
            RuntimeWaitState::Signaled { exit_code },
        ));
        let wait_updates = self.objects.plan_wait_state_updates(&wait_states)?;
        let excluded_threads: Vec<_> = plan
            .thread_plans
            .iter()
            .map(|thread_plan| thread_plan.thread_id)
            .collect();
        let wake_completions = self.plan_waiter_wakes(&wait_updates, &excluded_threads)?;
        if !wake_completions.is_empty() {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-wait",
                "process-exit-surviving-waiter",
                "process termination cannot leave a registered waiter to resume",
            ));
        }
        self.teardown_mutexes()?;
        let cancellation = plan.cancellation.clone();
        self.objects.commit_wait_state_updates(wait_updates);
        for ((thread_plan, terminal_thread_plan), terminal_scheduler_plan) in plan
            .thread_plans
            .into_iter()
            .zip(terminal_thread_plans)
            .zip(terminal_scheduler_plans)
        {
            self.commit_thread_exit_request(thread_plan);
            self.threads.commit(terminal_thread_plan);
            self.scheduler.commit_lane(terminal_scheduler_plan);
        }
        self.process.unhandled_exception_filter = None;
        self.process.lifecycle = RuntimeProcessLifecycleState::Exited { exit_code };
        Ok(cancellation)
    }

    fn plan_thread_wait_cancellation(
        &self,
        thread_id: RuntimeThreadId,
        scheduler_state: SchedulerThreadState,
    ) -> Result<Option<RuntimeWaitRegistration>, RuntimeStateError> {
        let registration = self.scheduler.wait_registration_for_thread(thread_id)?;
        match scheduler_state {
            SchedulerThreadState::WaitingObject {
                registration_id,
                object_id,
            } => {
                let registration = registration.ok_or_else(|| {
                    RuntimeStateError::runtime_defect(
                        "runtime-wait",
                        "waiting-thread-registration-count-invalid",
                        "waiting thread must own exactly one wait registration",
                    )
                })?;
                if registration.id != registration_id || registration.object_id != object_id {
                    return Err(RuntimeStateError::runtime_defect(
                        "runtime-wait",
                        "waiting-thread-registration-mismatch",
                        "scheduler wait state does not match the thread registration",
                    ));
                }
                let queue = self
                    .scheduler
                    .object_waiters()
                    .get(&object_id)
                    .ok_or_else(|| {
                        RuntimeStateError::runtime_defect(
                            "runtime-wait",
                            "wait-object-queue-missing",
                            "wait registration has no object queue",
                        )
                    })?;
                if queue
                    .iter()
                    .filter(|&&queued_id| queued_id == registration_id)
                    .count()
                    != 1
                {
                    return Err(RuntimeStateError::runtime_defect(
                        "runtime-wait",
                        "wait-object-queue-registration-mismatch",
                        "wait registration must occur exactly once in its object queue",
                    ));
                }
                Ok(Some(registration))
            }
            _ if registration.is_none() => Ok(None),
            _ => Err(RuntimeStateError::runtime_defect(
                "runtime-wait",
                "nonwaiting-thread-registration-present",
                "nonwaiting thread owns a wait registration",
            )),
        }
    }

    fn plan_waiter_wakes(
        &self,
        wait_updates: &[(RuntimeObjectId, RuntimeObjectRecord)],
        excluded_threads: &[RuntimeThreadId],
    ) -> Result<Vec<RuntimeWaitWakePlan>, RuntimeStateError> {
        let wait_states = wait_updates
            .iter()
            .filter_map(|(object_id, record)| {
                record.wait_state.map(|wait_state| (*object_id, wait_state))
            })
            .collect::<Vec<_>>();
        let candidates = self
            .scheduler
            .plan_waiter_wakes(&wait_states, excluded_threads)?;
        let mut plans = Vec::with_capacity(candidates.len());
        for candidate in candidates {
            let completion = candidate.completion;
            let registration = completion.registration;
            if self.thread_state(registration.thread_id)?.lifecycle
                != RuntimeThreadLifecycleState::Running
                || self.scheduler_state_for_thread(registration.thread_id)?
                    != (SchedulerThreadState::WaitingObject {
                        registration_id: registration.id,
                        object_id: registration.object_id,
                    })
            {
                return Err(RuntimeStateError::runtime_defect(
                    "runtime-wait",
                    "wait-registration-thread-state-mismatch",
                    "wait registration does not match a running waiting thread",
                ));
            }
            if candidate.excluded {
                continue;
            }
            let scheduler_plan = self
                .scheduler
                .plan_transition(registration.thread_id, SchedulerThreadState::Runnable)?;
            plans.push(RuntimeWaitWakePlan {
                completion,
                scheduler_plan,
            });
        }
        Ok(plans)
    }

    fn commit_waiter_wakes(&mut self, plans: Vec<RuntimeWaitWakePlan>) {
        for plan in plans {
            let completion = plan.completion;
            self.commit_remove_wait_registration(completion.registration);
            self.scheduler.commit_lane(plan.scheduler_plan);
            self.scheduler
                .commit_event(RuntimeSchedulerEvent::WaitSatisfied(completion));
        }
    }

    fn commit_remove_wait_registration(&mut self, registration: RuntimeWaitRegistration) {
        self.scheduler.commit_remove_wait_registration(registration);
    }

    fn plan_thread_exit_request(
        &self,
        thread_id: RuntimeThreadId,
        exit_code: u32,
    ) -> Result<RuntimeThreadExitRequestPlan, RuntimeStateError> {
        let (thread_plan, callback_frames_cancelled) =
            self.threads.plan_exit_request(thread_id, exit_code)?;
        let scheduler_state = self.scheduler_state_for_thread(thread_id)?;
        if !matches!(
            scheduler_state,
            SchedulerThreadState::Runnable
                | SchedulerThreadState::WaitingHost { .. }
                | SchedulerThreadState::WaitingObject { .. }
                | SchedulerThreadState::WaitingMessage
                | SchedulerThreadState::InCallback { .. }
        ) {
            return Err(RuntimeStateError::conflict(
                "runtime-lifecycle",
                "thread-exit-request-transition-invalid",
                "thread exit request requires a running thread and a live scheduler state",
            ));
        }
        let scheduler_plan = self
            .scheduler
            .plan_transition(thread_id, SchedulerThreadState::Terminating { exit_code })?;
        let cancelled_wait = self.plan_thread_wait_cancellation(thread_id, scheduler_state)?;
        let host_request_cancellation = self.scheduler.plan_host_request_cancellation(thread_id);
        let message_clear = self.messages.plan_clear(thread_id)?;
        let messages_discarded = message_clear.message_count;
        let host_requests_cancelled = host_request_cancellation.request_count();
        let scheduler_events_discarded = self.scheduler.event_count_for_thread(thread_id);
        Ok(RuntimeThreadExitRequestPlan {
            thread_id,
            thread_plan,
            scheduler_plan,
            cancelled_wait,
            host_request_cancellation,
            message_clear,
            cancellation: RuntimeCancellation {
                thread_id,
                exit_code,
                host_requests_cancelled,
                callback_frames_cancelled,
                wait_registrations_cancelled: usize::from(cancelled_wait.is_some()),
                messages_discarded,
                scheduler_events_discarded,
            },
        })
    }

    fn commit_thread_exit_request(&mut self, plan: RuntimeThreadExitRequestPlan) {
        self.messages.commit_clear(plan.message_clear);
        self.scheduler
            .commit_host_request_cancellation(plan.host_request_cancellation);
        if let Some(registration) = plan.cancelled_wait {
            self.commit_remove_wait_registration(registration);
        }
        self.scheduler
            .commit_remove_events_for_thread(plan.thread_id);
        self.threads.commit(plan.thread_plan);
        self.scheduler.commit_lane(plan.scheduler_plan);
    }

    fn plan_thread_exit_completion(
        &self,
        thread_id: RuntimeThreadId,
    ) -> Result<RuntimeThreadExitCompletionPlan, RuntimeStateError> {
        let (thread_plan, exit_code) = self.threads.plan_exit_completion(thread_id)?;
        if self.scheduler_state_for_thread(thread_id)?
            != (SchedulerThreadState::Terminating { exit_code })
            || self.scheduler.has_host_request_for_thread(thread_id)
            || !thread_plan.next().callback_frames.is_empty()
            || !self.messages.is_empty(thread_id)?
            || self.scheduler.has_wait_registration_for_thread(thread_id)
            || self.scheduler.has_event_for_thread(thread_id)
        {
            return Err(RuntimeStateError::conflict(
                "runtime-lifecycle",
                "thread-exit-completion-state-invalid",
                "thread exit completion requires terminating scheduler state and no retained work",
            ));
        }
        let scheduler_plan = self
            .scheduler
            .plan_transition(thread_id, SchedulerThreadState::Terminated { exit_code })?;
        Ok(RuntimeThreadExitCompletionPlan {
            thread_id,
            exit_code,
            thread_plan,
            scheduler_plan,
        })
    }

    fn commit_thread_exit_completion(&mut self, plan: RuntimeThreadExitCompletionPlan) {
        self.threads.commit(plan.thread_plan);
        self.scheduler.commit_lane(plan.scheduler_plan);
    }

    fn plan_process_exit_request(
        &self,
        exit_code: u32,
    ) -> Result<RuntimeProcessExitRequestPlan, RuntimeStateError> {
        let initiating_thread_id = self.current_thread_id();
        if self.thread_state(initiating_thread_id)?.lifecycle
            != RuntimeThreadLifecycleState::Running
        {
            return Err(RuntimeStateError::conflict(
                "runtime-lifecycle",
                "process-exit-initiating-thread-state-invalid",
                "process exit requires a running initiating thread",
            ));
        }
        let mut thread_plans = Vec::new();
        for (&thread_id, thread) in self.threads.iter() {
            match thread.lifecycle {
                RuntimeThreadLifecycleState::Running => {
                    thread_plans.push(self.plan_thread_exit_request(thread_id, exit_code)?);
                }
                RuntimeThreadLifecycleState::Exited { exit_code } => {
                    if self.scheduler_state_for_thread(thread_id)?
                        != (SchedulerThreadState::Terminated { exit_code })
                    {
                        return Err(RuntimeStateError::runtime_defect(
                            "runtime-lifecycle",
                            "exited-thread-scheduler-state-mismatch",
                            "exited thread does not have the matching terminated scheduler state",
                        ));
                    }
                }
                RuntimeThreadLifecycleState::Initialized
                | RuntimeThreadLifecycleState::ExitRequested { .. } => {
                    return Err(RuntimeStateError::conflict(
                        "runtime-lifecycle",
                        "process-exit-thread-state-invalid",
                        "process exit requires every non-exited thread to be running",
                    ));
                }
            }
        }
        if !thread_plans
            .iter()
            .any(|plan| plan.thread_id == initiating_thread_id)
        {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-lifecycle",
                "process-exit-initiating-thread-plan-missing",
                "process exit did not plan the initiating thread",
            ));
        }
        let thread_cancellations: Vec<_> =
            thread_plans.iter().map(|plan| plan.cancellation).collect();
        let cancellation = RuntimeProcessCancellation {
            initiating_thread_id,
            exit_code,
            host_requests_cancelled: thread_cancellations
                .iter()
                .map(|cancellation| cancellation.host_requests_cancelled)
                .sum(),
            callback_frames_cancelled: thread_cancellations
                .iter()
                .map(|cancellation| cancellation.callback_frames_cancelled)
                .sum(),
            wait_registrations_cancelled: thread_cancellations
                .iter()
                .map(|cancellation| cancellation.wait_registrations_cancelled)
                .sum(),
            messages_discarded: thread_cancellations
                .iter()
                .map(|cancellation| cancellation.messages_discarded)
                .sum(),
            scheduler_events_discarded: thread_cancellations
                .iter()
                .map(|cancellation| cancellation.scheduler_events_discarded)
                .sum(),
            thread_cancellations,
        };
        Ok(RuntimeProcessExitRequestPlan {
            initiating_thread_id,
            exit_code,
            thread_plans,
            cancellation,
        })
    }

    fn commit_process_exit_request(&mut self, plan: RuntimeProcessExitRequestPlan) {
        for thread_plan in plan.thread_plans {
            self.commit_thread_exit_request(thread_plan);
        }
        self.process.lifecycle = RuntimeProcessLifecycleState::ExitRequested {
            exit_code: plan.exit_code,
            initiating_thread_id: plan.initiating_thread_id,
        };
    }

    fn plan_process_exit_completion(
        &self,
        exit_code: u32,
    ) -> Result<Vec<RuntimeThreadExitCompletionPlan>, RuntimeStateError> {
        let mut completions = Vec::new();
        for (&thread_id, thread) in self.threads.iter() {
            match thread.lifecycle {
                RuntimeThreadLifecycleState::ExitRequested {
                    exit_code: thread_exit_code,
                } if thread_exit_code == exit_code => {
                    completions.push(self.plan_thread_exit_completion(thread_id)?);
                }
                RuntimeThreadLifecycleState::Exited {
                    exit_code: thread_exit_code,
                } => {
                    if self.scheduler_state_for_thread(thread_id)?
                        != (SchedulerThreadState::Terminated {
                            exit_code: thread_exit_code,
                        })
                    {
                        return Err(RuntimeStateError::runtime_defect(
                            "runtime-lifecycle",
                            "exited-thread-scheduler-state-mismatch",
                            "exited thread does not have the matching terminated scheduler state",
                        ));
                    }
                }
                _ => {
                    return Err(RuntimeStateError::runtime_defect(
                        "runtime-lifecycle",
                        "process-thread-exit-state-mismatch",
                        "process exit request does not match every live thread exit state",
                    ));
                }
            }
        }
        Ok(completions)
    }

    fn current_thread(&self) -> Result<&RuntimeThreadState, RuntimeStateError> {
        self.threads
            .current_state(self.scheduler.current_thread_id())
    }
}

fn runtime_export_matches_import(
    export: &RuntimeExport,
    module: &str,
    target: &ImportTarget,
) -> bool {
    match (export, target) {
        (
            RuntimeExport::Name {
                module: export_module,
                name: export_name,
            },
            ImportTarget::Name { name, .. },
        ) => export_module.eq_ignore_ascii_case(module) && export_name == name,
        (
            RuntimeExport::Ordinal {
                module: export_module,
                ordinal: export_ordinal,
            },
            ImportTarget::Ordinal { ordinal },
        ) => export_module.eq_ignore_ascii_case(module) && export_ordinal == ordinal,
        _ => false,
    }
}

fn validate_initial_thread_stack(stack: &InitialThreadStack) -> Result<(), RuntimeStateError> {
    let page_size = GUEST_PAGE_SIZE as u32;
    if !stack.owner_sealed
        || !stack.stack_range.start().0.is_multiple_of(page_size)
        || !stack
            .stack_range
            .byte_length()
            .is_multiple_of(GUEST_PAGE_SIZE)
        || !stack.committed_range.start().0.is_multiple_of(page_size)
        || !stack
            .committed_range
            .byte_length()
            .is_multiple_of(GUEST_PAGE_SIZE)
        || stack.committed_range.byte_length() == 0
        || stack.committed_pages != stack.committed_range.page_count()
        || stack.stack_range.start().0 > stack.committed_range.start().0
        || stack.committed_range.end() != stack.stack_range.end()
        || stack.stack_range.end() != u64::from(stack.stack_top.0)
        || stack.initial_esp.0.checked_add(4) != Some(stack.stack_top.0)
    {
        return Err(RuntimeStateError::invalid_input(
            "runtime-thread-stack",
            "initial-thread-stack-layout-invalid",
            "initial thread stack requires sealed, page-aligned, top-contiguous reserve and commit metadata",
        ));
    }
    let expected_guard = if stack.committed_range.start() == stack.stack_range.start() {
        None
    } else {
        Some(
            GuestRange::new(
                GuestAddress(
                    stack
                        .committed_range
                        .start()
                        .0
                        .checked_sub(page_size)
                        .ok_or_else(|| {
                            RuntimeStateError::invalid_input(
                                "runtime-thread-stack",
                                "initial-thread-stack-guard-underflow",
                                "initial thread stack guard underflows the x86 address space",
                            )
                        })?,
                ),
                GUEST_PAGE_SIZE,
            )
            .map_err(RuntimeStateError::from_memory)?,
        )
    };
    if stack.guard_page != expected_guard {
        return Err(RuntimeStateError::invalid_input(
            "runtime-thread-stack",
            "initial-thread-stack-guard-invalid",
            "initial thread stack guard must be the page immediately below committed storage",
        ));
    }
    Ok(())
}

fn validate_runtime_thread_stack_allocation(
    allocator: &ProcessAddressSpaceAllocator,
    stack: &RuntimeThreadStack,
) -> Result<(), RuntimeStateError> {
    let allocation = allocator.allocation(stack.address_allocation_id)?;
    let expected_range = RuntimeAddressRange::new(
        stack.reserved_range.start(),
        stack.reserved_range.byte_length(),
    )?;
    if allocation.purpose != ProcessAddressAllocationPurpose::ThreadStack
        || allocation.range != expected_range
        || allocation.classification != ProcessAddressAllocationClassification::Stack
        || allocation.lifetime != ProcessAddressAllocationLifetime::Thread
        || allocation.release_policy != ProcessAddressAllocationReleasePolicy::Explicit
        || allocation.executable
        || allocation.guard_pages != 1
    {
        return Err(RuntimeStateError::runtime_defect(
            "runtime-thread-stack",
            "thread-stack-address-allocation-mismatch",
            "initial thread stack is not backed by its thread-lifetime process allocation record",
        ));
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
    pub(crate) fn from_memory(error: GuestMemoryError) -> Self {
        let class = match error.class {
            GuestMemoryErrorClass::ResourceLimit => RuntimeStateErrorClass::ResourceLimit,
            GuestMemoryErrorClass::RuntimeDefect => RuntimeStateErrorClass::RuntimeDefect,
            GuestMemoryErrorClass::Conflict | GuestMemoryErrorClass::SynchronizationRequired => {
                RuntimeStateErrorClass::Conflict
            }
            GuestMemoryErrorClass::InvalidRequest | GuestMemoryErrorClass::Fault => {
                RuntimeStateErrorClass::InvalidInput
            }
        };
        Self::new(class, error.stage, error.code, error.message)
    }

    pub(crate) fn invalid_input(
        stage: &'static str,
        code: &'static str,
        message: impl Into<String>,
    ) -> Self {
        Self::new(RuntimeStateErrorClass::InvalidInput, stage, code, message)
    }

    pub(crate) fn not_found(
        stage: &'static str,
        code: &'static str,
        message: impl Into<String>,
    ) -> Self {
        Self::new(RuntimeStateErrorClass::NotFound, stage, code, message)
    }

    pub(crate) fn conflict(
        stage: &'static str,
        code: &'static str,
        message: impl Into<String>,
    ) -> Self {
        Self::new(RuntimeStateErrorClass::Conflict, stage, code, message)
    }

    pub(crate) fn resource_limit(
        stage: &'static str,
        code: &'static str,
        message: impl Into<String>,
    ) -> Self {
        Self::new(RuntimeStateErrorClass::ResourceLimit, stage, code, message)
    }

    pub(crate) fn runtime_defect(
        stage: &'static str,
        code: &'static str,
        message: impl Into<String>,
    ) -> Self {
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

const fn ranges_overlap(left_start: u64, left_end: u64, right_start: u64, right_end: u64) -> bool {
    left_start < right_end && right_start < left_end
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_private_heap_registration_preserves_the_slot_allocator() {
        let mut runtime = RuntimeState::new(RuntimeBootstrap {
            process_id: RuntimeProcessId(0x1000),
            initial_thread_id: RuntimeThreadId(0x2000),
            initial_last_error: 0,
            address_space: ProcessAddressSpaceLayout::vine_v1(),
            clock: RuntimeClockState {
                wall_clock_file_time: 134_282_016_000_000_000,
                tick_count: 3_600_000,
                performance_counter: 12_345_678_901,
            },
            startup_info: RuntimeStartupInfo {
                profile_id: "runtime-heap-slot-atomicity-test".to_owned(),
                bytes: vec![0; X86_STARTUP_INFO_W_BYTE_LENGTH],
            },
            process_image_path_a: RuntimeProcessImagePathA {
                source_authority: "runtime-heap-slot-atomicity-fixture".to_owned(),
                bytes_with_nul: b"C:\\Vine\\fixture.exe\0".to_vec(),
                encoding: RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING.to_owned(),
            },
            initial_current_directory_a: RuntimeCurrentDirectoryA {
                source_authority: "runtime-heap-slot-atomicity-fixture".to_owned(),
                bytes_with_nul: b"C:\\Vine\0".to_vec(),
                encoding: RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING.to_owned(),
            },
            encoded_pointer_namespace: LOGICAL_ENCODED_POINTER_NAMESPACE.to_owned(),
            encoded_pointer_token_prefix: 0xec01_0000,
        })
        .expect("controlled runtime");
        let handle = LOGICAL_PRIVATE_HEAP_HANDLE_PREFIX | 1;
        runtime
            .objects
            .register_object_with_handle(
                RuntimeObjectKey::PrivateHeap { handle },
                RuntimeHandle(handle),
            )
            .expect("controlled object-table conflict");
        let before = runtime.clone();

        let error = runtime
            .allocate_private_heap(0, 0x1000, 0)
            .expect_err("object-table conflict must reject heap creation");

        assert_eq!(error.code, "runtime-handle-slot-live");
        assert_eq!(runtime, before, "failed creation must not consume a slot");
    }

    #[test]
    fn host_request_serialization_preserves_the_runtime_compatibility_shape() {
        let mut runtime = RuntimeState::new(RuntimeBootstrap {
            process_id: RuntimeProcessId(0x1000),
            initial_thread_id: RuntimeThreadId(0x2000),
            initial_last_error: 0,
            address_space: ProcessAddressSpaceLayout::vine_v1(),
            clock: RuntimeClockState {
                wall_clock_file_time: 134_282_016_000_000_000,
                tick_count: 3_600_000,
                performance_counter: 12_345_678_901,
            },
            startup_info: RuntimeStartupInfo {
                profile_id: "runtime-host-request-serialization-test".to_owned(),
                bytes: vec![0; X86_STARTUP_INFO_W_BYTE_LENGTH],
            },
            process_image_path_a: RuntimeProcessImagePathA {
                source_authority: "runtime-host-request-serialization-fixture".to_owned(),
                bytes_with_nul: b"C:\\Vine\\fixture.exe\0".to_vec(),
                encoding: RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING.to_owned(),
            },
            initial_current_directory_a: RuntimeCurrentDirectoryA {
                source_authority: "runtime-host-request-serialization-fixture".to_owned(),
                bytes_with_nul: b"C:\\Vine\0".to_vec(),
                encoding: RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING.to_owned(),
            },
            encoded_pointer_namespace: LOGICAL_ENCODED_POINTER_NAMESPACE.to_owned(),
            encoded_pointer_token_prefix: 0xec01_0000,
        })
        .expect("controlled runtime");
        runtime.start_initial_thread().unwrap();
        let request = runtime.begin_host_request("browser.storage.read").unwrap();

        let serialized = serde_json::to_value(&runtime).unwrap();
        assert_eq!(
            serialized["registries"]["hostRequests"]["1"]["requestId"],
            serde_json::json!(request.request_id)
        );
        assert_eq!(serialized["nextHostRequestId"], serde_json::json!(2));
        assert!(serialized["scheduler"].get("hostRequests").is_none());
        assert!(serialized["scheduler"].get("nextHostRequestId").is_none());
        assert_eq!(
            runtime.scheduler().host_requests().get(&request.request_id),
            Some(&request)
        );
    }
}

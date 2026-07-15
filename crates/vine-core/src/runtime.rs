//! Public runtime facade.
//!
//! Subsystems own their mutable collections. `RuntimeKernel` coordinates
//! transactions that cross those subsystem boundaries; `RuntimeState` remains
//! its compatibility name for existing callers.

mod address_space;
mod heaps;
mod kernel;
mod messages;
mod modules;
mod objects;
mod process;
mod registries;
mod scheduler;
mod synchronization;
mod thread;
mod waits;

pub use address_space::{
    ProcessAddressAllocationClassification, ProcessAddressAllocationId,
    ProcessAddressAllocationLifetime, ProcessAddressAllocationPurpose,
    ProcessAddressAllocationRecord, ProcessAddressAllocationReleasePolicy,
    ProcessAddressAllocationRequest, ProcessAddressMappingRequirements,
    ProcessAddressSpaceAllocator, ProcessAddressSpaceLayout, RuntimeAddressRange,
    validate_process_address_mapping_allocation,
};
pub use heaps::{
    LOGICAL_PRIVATE_HEAP_NAMESPACE, RUNTIME_PRIVATE_HEAP_ALIGNMENT,
    RUNTIME_PRIVATE_HEAP_ARENA_BASE, RUNTIME_PRIVATE_HEAP_ARENA_STRIDE,
    RUNTIME_PRIVATE_HEAP_BACKING_LIMIT, RUNTIME_PRIVATE_HEAP_FREED_BYTE,
    RUNTIME_PRIVATE_HEAP_MAX_LIVE_ALLOCATIONS, RUNTIME_PRIVATE_HEAP_UNINITIALIZED_BYTE,
    RuntimeHeap, RuntimeHeapAllocation, RuntimeHeapAllocationId, RuntimeHeapBacking,
    RuntimeHeapInitialization, RuntimeHeapTeardown,
};
pub use kernel::*;
pub use messages::{
    RuntimeMessageId, RuntimeMessageReceiveDisposition, RuntimeMessageWake, RuntimeThreadMessage,
};
pub use modules::{
    DLL_PROCESS_ATTACH, DLL_PROCESS_DETACH, DLL_PROCESS_STATIC_RESERVED,
    LOGICAL_LOADED_MODULE_NAMESPACE, RuntimeDllAttachFailure, RuntimeDllCallbackReason,
    RuntimeDllCallbackRequest, RuntimeDllLifecycleAction, RuntimeDllLifecycleCompletion,
    RuntimeDllLifecyclePhase, RuntimeGuestImport, RuntimeGuestImportBinding,
    RuntimeGuestImportResolution, RuntimeModule, RuntimeModuleDependencyEdge,
    RuntimeSystemExportEndpoint, RuntimeSystemExportTable,
};
pub use objects::{
    RUNTIME_MUTEX_OBJECT_NAMESPACE, RUNTIME_PROCESS_OBJECT_NAMESPACE,
    RUNTIME_THREAD_OBJECT_NAMESPACE, RuntimeHandle, RuntimeHandleClosePolicy, RuntimeHandleEntry,
    RuntimeObjectId, RuntimeObjectKey, RuntimeObjectKind, RuntimeObjectRecord, RuntimeObjectTable,
    RuntimeWaitState,
};
pub use process::{
    LOGICAL_STANDARD_HANDLE_NAMESPACE, RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING,
    RUNTIME_PROCESS_IMAGE_PATH_A_MAX_BYTES, RUNTIME_STANDARD_ERROR_HANDLE,
    RUNTIME_STANDARD_ERROR_SELECTOR, RUNTIME_STANDARD_INPUT_HANDLE,
    RUNTIME_STANDARD_INPUT_SELECTOR, RUNTIME_STANDARD_OUTPUT_HANDLE,
    RUNTIME_STANDARD_OUTPUT_SELECTOR, RuntimeClockState, RuntimeCommandLine,
    RuntimeCurrentDirectoryA, RuntimeEnvironmentStringCopy, RuntimeEnvironmentStrings,
    RuntimeProcessCancellation, RuntimeProcessEnvironment, RuntimeProcessId,
    RuntimeProcessImagePathA, RuntimeProcessLifecycleState, RuntimeProcessState,
    RuntimeSetCurrentDirectoryAPlan, RuntimeStandardHandle, RuntimeStartupInfo,
    X86_STARTUP_INFO_W_BYTE_LENGTH,
};
pub use registries::{
    LOGICAL_ENCODED_POINTER_NAMESPACE, RuntimeDynamicEndpoint, RuntimeEncodedPointer,
    RuntimeRegistries,
};
pub use scheduler::{
    RuntimeHostCompletion, RuntimeHostRequest, RuntimeScheduler, RuntimeSchedulerEvent,
    SchedulerThreadState,
};
pub use synchronization::{
    RUNTIME_CRITICAL_SECTION_LIMIT, RUNTIME_MUTEX_LIMIT, RuntimeCreateMutexAResult,
    RuntimeCriticalSection, RuntimeMutex, RuntimeMutexId, RuntimeMutexNameA,
    X86_CRITICAL_SECTION_BYTE_LENGTH,
};
pub use thread::{
    FLS_OUT_OF_INDEXES, LOGICAL_FLS_INDEX_NAMESPACE, LOGICAL_TLS_INDEX_NAMESPACE,
    RUNTIME_FLS_INDEX_LIMIT, RuntimeCallbackFrame, RuntimeCallbackFrameId, RuntimeCancellation,
    RuntimeFlsSlot, RuntimeThreadId, RuntimeThreadLifecycleState, RuntimeThreadStack,
    RuntimeThreadStackGrowth, RuntimeThreadState, RuntimeTlsValue, TLS_OUT_OF_INDEXES,
};
pub use waits::{
    RuntimeWaitCompletion, RuntimeWaitDisposition, RuntimeWaitRegistration,
    RuntimeWaitRegistrationId,
};

pub type RuntimeState = RuntimeKernel;

pub(crate) use address_space::validate_process_address_mapping_allocation_for_memory;
pub(crate) use heaps::{
    RuntimeHeapAllocationPlan, RuntimeHeapBackingPlan, RuntimeHeapDestructionPlan,
};

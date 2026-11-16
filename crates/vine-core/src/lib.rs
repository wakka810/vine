#![forbid(unsafe_code)]

mod api;
mod bootstrap;
mod callbacks;
mod com;
mod d3d9;
mod dinput8;
pub mod directshow;
mod dll;
mod dsound;
mod dynamic_exports;
mod environment;
mod gdi32;
mod gdiplus;
mod heap;
mod image_code;
mod imm32;
mod imports;
mod materialize;
mod memory;
mod module_graph;
mod module_images;
mod ole32;
mod pe32;
mod runtime;
mod shell32;
mod snapshot;
mod user32;
mod win32;
mod winmm;

pub use api::{
    ApiDeferredCall, ApiDispatcher, ApiEndpointIdentity, ApiFault, ApiGuestCallbackCall,
    ApiImmediate, ApiImmediateCompletion, ApiOwnedTrap, ApiSourceLookup, DispatchOutcome,
    GuestCallbackReturn, HostCompletion, X86CpuContext,
};
pub use bootstrap::install_initial_thread_stack;
pub use callbacks::install_runtime_return_sentinels;
pub use com::{ComAudioDeferred, ComCallTrap, ComDispatchOutcome};
pub use dynamic_exports::DynamicExportThunk;
pub use gdi32::{
    Gdi32TextRasterHostResponse, RUNTIME_GDI_TEXT_RASTER_SERVICE, complete_gdi32_text_raster,
};
pub use image_code::{CodePreparationEntries, code_preparation_entries};
pub use imports::{RuntimeExport, RuntimeExportCatalog};
pub use materialize::{ImageMaterialization, materialize_pe32_image};
pub use memory::{
    GUEST_PAGE_SIZE, GuestAddress, GuestMemory, GuestRange, MemoryDeltaBuffers, MemoryLimits,
    MemoryOwnerId, MemoryOwnerKind, MemoryReflectionBuffers, MemoryRegionKind, PageProtection,
};
pub use module_graph::{RuntimeModuleId, RuntimeModuleLoadState};
pub(crate) use pe32::MappingProtection;
pub use pe32::{ImageKind, ImportTarget, Pe32Limits, Pe32LoadPlan, Rva, build_pe32_load_plan};
pub(crate) use runtime::RuntimePathResolution;
pub use runtime::{
    ProcessAddressAllocationPurpose, ProcessAddressAllocationRecord,
    RUNTIME_AUDIO_NOTIFICATION_PACING_SERVICE, RUNTIME_GUEST_THREAD_STACK_ARENA_BASE,
    RUNTIME_GUEST_THREAD_STACK_ARENA_SIZE, RUNTIME_MESSAGE_BOX_SERVICE,
    RUNTIME_OBJECT_WAIT_SERVICE, RUNTIME_OPFS_READ_SERVICE, RUNTIME_PROCESS_IMAGE_PATH_A_MAX_BYTES,
    RUNTIME_SOURCE_FILE_LIMIT, RUNTIME_SOURCE_READ_MAX_BYTES,
    RUNTIME_SOURCE_READ_RESPONSE_CONTROL_BYTES, RUNTIME_WAITABLE_TIMER_PACING_SERVICE, Runtime,
    RuntimeApiBoundaryDisposition, RuntimeBootstrap, RuntimeClockState, RuntimeCurrentDirectoryA,
    RuntimeCursorPositionProfile, RuntimeDisplayProfile, RuntimeDllLifecycleAction,
    RuntimeDllLifecyclePhase, RuntimeGuestImportBinding, RuntimeInputProfile,
    RuntimeKeyboardSourceEvent, RuntimeKeyboardSourceEventKind, RuntimeMessageBox,
    RuntimeMessageBoxButton, RuntimeMessageBoxIcon, RuntimeModule, RuntimePointerButton,
    RuntimePointerButtonSourceEvent, RuntimePointerButtonSourceEventKind,
    RuntimePointerPositionSourceEvent, RuntimeProcessId, RuntimeProcessImagePathA, RuntimeRect,
    RuntimeSchedulerEvent, RuntimeSourceFileManifest, RuntimeSourceMountManifest,
    RuntimeSourceReadHostResponse, RuntimeSourceSnapshotManifest, RuntimeStartupInfo,
    RuntimeThreadId, RuntimeThreadLifecycleState, SchedulerThreadState,
};
pub(crate) use snapshot::ModuleIdentity;
pub use snapshot::ModuleSnapshot;
pub use user32::complete_user32_message_box;
pub use win32::{
    OpfsFileOperation, RUNTIME_SLEEP_DELAY_SERVICE, complete_kernel32_audio_notification_wait,
    complete_kernel32_object_wait, complete_kernel32_opfs_open, complete_kernel32_opfs_write,
    complete_kernel32_source_read, complete_kernel32_waitable_timer_pacing,
    complete_opfs_file_operation,
};

#[cfg(test)]
mod address_allocation_mapping_tests;
#[cfg(test)]
mod guest_memory_ingress_tests;
#[cfg(test)]
mod guest_memory_property_tests;
#[cfg(test)]
mod module_graph_tests;
#[cfg(test)]
mod pe32_load_plan_tests;
#[cfg(test)]
mod pe32_resources_tests;
#[cfg(test)]
mod runtime_dll_lifecycle_tests;
#[cfg(test)]
mod runtime_module_images_tests;
#[cfg(test)]
mod source_file_gateway_tests;

#[cfg(test)]
mod process_environment_transaction_tests;

#[cfg(test)]
mod com_iunknown_foundation_tests;
#[cfg(test)]
mod dinput8_dispatcher_tests;

#[cfg(test)]
mod d3d9_dispatcher_tests;

#[cfg(test)]
mod dsound_dispatcher_tests;

#[cfg(test)]
mod gdi32_dispatcher_tests;

#[cfg(test)]
mod gdiplus_dispatcher_tests;

#[cfg(test)]
mod kernel32_thread_dispatcher_tests;

#[cfg(test)]
mod kernel32_virtual_memory_dispatcher_tests;

#[cfg(test)]
mod directshow_dispatcher_tests;
#[cfg(test)]
mod ole32_dispatcher_tests;

#[cfg(test)]
mod user32_dispatcher_tests;

#[cfg(test)]
mod winmm_dispatcher_tests;

#[cfg(test)]
mod runtime_message_queue_tests;

#[cfg(test)]
mod runtime_synchronization_tests;

#[cfg(test)]
mod memory_writeback_tests;

#[cfg(test)]
mod memory_delta_tests;

#[cfg(test)]
mod pe32_materialization_tests;

#[cfg(test)]
mod runtime_initial_thread_stack_tests;

#[cfg(test)]
mod runtime_lifecycle_tests;

#[cfg(test)]
mod runtime_mutex_tests;

#[cfg(test)]
mod runtime_objects_tests;

#[cfg(test)]
mod runtime_registries_tests;

#[cfg(test)]
mod runtime_thread_registry_tests;

#[cfg(test)]
mod runtime_waitable_timer_tests;

#[cfg(test)]
mod runtime_wait_queue_tests;

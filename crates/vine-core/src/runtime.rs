//! Public runtime facade.
//!
//! Subsystems own their mutable collections. `RuntimeKernel` coordinates
//! transactions that cross those subsystem boundaries; `RuntimeState` remains
//! its compatibility name for existing callers.

mod address_space;
mod audio;
mod com;
mod displays;
mod files;
mod gdi;
mod graphics;
mod heaps;
mod input;
mod kernel;
mod messages;
mod modules;
mod objects;
mod process;
mod registries;
mod scheduler;
mod synchronization;
mod thread;
mod timers;
mod user32;
mod waits;

pub use address_space::{
    ProcessAddressAllocationClassification, ProcessAddressAllocationId,
    ProcessAddressAllocationLifetime, ProcessAddressAllocationPurpose,
    ProcessAddressAllocationRecord, ProcessAddressAllocationReleasePolicy,
    ProcessAddressAllocationRequest, ProcessAddressMappingRequirements,
    ProcessAddressSpaceAllocator, ProcessAddressSpaceLayout, RuntimeAddressRange,
    validate_process_address_mapping_allocation,
};
pub use audio::{
    RUNTIME_AUDIO_BUFFER_ID_NAMESPACE, RUNTIME_AUDIO_BUFFER_LIMIT,
    RUNTIME_AUDIO_DEVICE_ID_NAMESPACE, RUNTIME_AUDIO_DEVICE_LIMIT, RuntimeAudioBuffer,
    RuntimeAudioBufferDescriptor, RuntimeAudioBufferFormatResult, RuntimeAudioBufferId,
    RuntimeAudioBufferKind, RuntimeAudioBufferPlayResult, RuntimeAudioCooperativeLevel,
    RuntimeAudioCooperativeLevelResult, RuntimeAudioCooperativeState, RuntimeAudioDevice,
    RuntimeAudioDeviceId, RuntimeAudioDeviceSelector, RuntimeAudioFrontend, RuntimeAudioPcmFormat,
    RuntimeAudioPlaybackState,
};
pub(crate) use audio::{
    RuntimeAudioBufferCreateRequest, RuntimeAudioBufferFormatRequest,
    RuntimeAudioBufferPlayRequest, RuntimeAudioCooperativeLevelRequest,
    RuntimeAudioDeviceCreateRequest,
};
pub use com::{
    RUNTIME_COM_IUNKNOWN_IID, RUNTIME_COM_MAX_INTERFACES_PER_OBJECT,
    RUNTIME_COM_MAX_METHODS_PER_INTERFACE, RuntimeCom, RuntimeComBacking, RuntimeComIid,
    RuntimeComInterfaceId, RuntimeComInterfaceRecord, RuntimeComMethodImplementation,
    RuntimeComMethodRecord, RuntimeComMethodTarget, RuntimeComObjectId, RuntimeComObjectRecord,
    RuntimeComReturnKind,
};
pub use displays::{
    LOGICAL_MONITOR_HANDLE_NAMESPACE, RUNTIME_PRIMARY_MONITOR_HANDLE, RuntimeDisplayProfile,
    RuntimeLogicalMonitor, RuntimeRect,
};
pub use files::{
    RUNTIME_SOURCE_FILE_LIMIT, RUNTIME_SOURCE_FILE_NAMESPACE, RUNTIME_SOURCE_READ_MAX_BYTES,
    RUNTIME_SOURCE_READ_REQUEST_BYTES, RUNTIME_SOURCE_READ_RESPONSE_CONTROL_BYTES,
    RUNTIME_SOURCE_READ_SERVICE, RUNTIME_SOURCE_READ_TRANSPORT_VERSION,
    RUNTIME_SOURCE_SNAPSHOT_SCHEMA, RUNTIME_SOURCE_SNAPSHOT_VERSION, RuntimeOpenSourceFileResult,
    RuntimeSourceFile, RuntimeSourceFileDescriptor, RuntimeSourceFileId, RuntimeSourceFileManifest,
    RuntimeSourceMountManifest, RuntimeSourceReadCompletion, RuntimeSourceReadHostResponse,
    RuntimeSourceReadRequest, RuntimeSourceSnapshot, RuntimeSourceSnapshotManifest,
};
pub use gdi::{
    LOGICAL_DEVICE_CONTEXT_HANDLE_NAMESPACE, LOGICAL_GDI_OBJECT_HANDLE_NAMESPACE,
    RUNTIME_DEVICE_CONTEXT_HANDLE_BASE, RUNTIME_GDI_BLACK_BRUSH_INDEX,
    RUNTIME_GDI_DEVICE_CONTEXT_LIMIT, RUNTIME_GDI_OBJECT_HANDLE_BASE, RUNTIME_GDI_OBJECT_LIMIT,
    RUNTIME_GDI_SURFACE_FILL_LIMIT, RUNTIME_GDI_SURFACE_LIMIT, RuntimeDeviceContext,
    RuntimeDeviceContextId, RuntimeDeviceContextTarget, RuntimeFillRectResult, RuntimeGdiObject,
    RuntimeGdiObjectId, RuntimeGdiObjectKind, RuntimeGdiSurface, RuntimeGdiSurfaceFill,
    RuntimeGdiSurfaceId, RuntimeGetDcResult, RuntimeGetGraphicsSurfaceDcResult,
    RuntimeGetStockObjectResult,
};
pub use graphics::{
    RUNTIME_BROWSER_RENDERER_ID_NAMESPACE, RUNTIME_GRAPHICS_DEVICE_ID_NAMESPACE,
    RUNTIME_GRAPHICS_DEVICE_LIMIT, RUNTIME_GRAPHICS_RESOURCE_ARENA_BASE,
    RUNTIME_GRAPHICS_RESOURCE_ARENA_SIZE, RUNTIME_GRAPHICS_SURFACE_FILL_LIMIT,
    RUNTIME_GRAPHICS_SURFACE_ID_NAMESPACE, RUNTIME_GRAPHICS_SURFACE_LIMIT,
    RUNTIME_GRAPHICS_TEXTURE_ID_NAMESPACE, RUNTIME_GRAPHICS_TEXTURE_LIMIT,
    RUNTIME_GRAPHICS_VERTEX_BUFFER_ID_NAMESPACE, RUNTIME_GRAPHICS_VERTEX_BUFFER_LIMIT,
    RuntimeBrowserRendererResourceId, RuntimeGraphicsColorFormat, RuntimeGraphicsCullMode,
    RuntimeGraphicsDevice, RuntimeGraphicsDeviceId, RuntimeGraphicsFrontend,
    RuntimeGraphicsPresentMode, RuntimeGraphicsPresentation, RuntimeGraphicsRenderStateUpdate,
    RuntimeGraphicsRenderStateUpdateResult, RuntimeGraphicsRenderStates,
    RuntimeGraphicsRendererAssociation, RuntimeGraphicsRendererOwnership, RuntimeGraphicsSurface,
    RuntimeGraphicsSurfaceDescriptor, RuntimeGraphicsSurfaceFill, RuntimeGraphicsSurfaceId,
    RuntimeGraphicsTexture, RuntimeGraphicsTextureDescriptor, RuntimeGraphicsTextureId,
    RuntimeGraphicsTexturePool, RuntimeGraphicsTextureUsage, RuntimeGraphicsVertexBuffer,
    RuntimeGraphicsVertexBufferBacking, RuntimeGraphicsVertexBufferDescriptor,
    RuntimeGraphicsVertexBufferId, RuntimeGraphicsVertexBufferLock,
    RuntimeGraphicsVertexBufferPool, RuntimeGraphicsVertexBufferUnlockResult,
    RuntimeGraphicsVertexBufferUsage,
};
pub(crate) use graphics::{
    RuntimeGraphicsDeviceCreateRequest, RuntimeGraphicsRenderStateUpdateRequest,
    RuntimeGraphicsSurfaceCreateRequest, RuntimeGraphicsTextureCreateRequest,
    RuntimeGraphicsVertexBufferCreateRequest, RuntimeGraphicsVertexBufferLockRequest,
};
pub(crate) use heaps::RuntimeHeapFreeBackingPlan;
pub use heaps::{
    LOGICAL_PRIVATE_HEAP_NAMESPACE, RUNTIME_PRIVATE_HEAP_ALIGNMENT,
    RUNTIME_PRIVATE_HEAP_ARENA_BASE, RUNTIME_PRIVATE_HEAP_ARENA_STRIDE,
    RUNTIME_PRIVATE_HEAP_BACKING_LIMIT, RUNTIME_PRIVATE_HEAP_FREED_BYTE,
    RUNTIME_PRIVATE_HEAP_LARGE_SLOT_BASE, RUNTIME_PRIVATE_HEAP_LARGE_SLOT_SIZE,
    RUNTIME_PRIVATE_HEAP_MAX_LIVE_ALLOCATIONS, RUNTIME_PRIVATE_HEAP_UNINITIALIZED_BYTE,
    RuntimeHeap, RuntimeHeapAllocation, RuntimeHeapAllocationId, RuntimeHeapBacking,
    RuntimeHeapInitialization, RuntimeHeapPageMaterialization, RuntimeHeapTeardown,
    RuntimeLargeHeapAllocationBacking,
};
pub use input::{RuntimeInputProfile, RuntimeMouseButtonMapping, RuntimeSwapMouseButtonResult};
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
    RuntimeSystemExportEndpoint, RuntimeSystemExportTable, RuntimeSystemModuleLoad,
};
pub use objects::{
    RUNTIME_MUTEX_OBJECT_NAMESPACE, RUNTIME_PROCESS_OBJECT_NAMESPACE,
    RUNTIME_THREAD_OBJECT_NAMESPACE, RUNTIME_WAITABLE_TIMER_OBJECT_NAMESPACE, RuntimeHandle,
    RuntimeHandleClosePolicy, RuntimeHandleEntry, RuntimeObjectId, RuntimeObjectKey,
    RuntimeObjectKind, RuntimeObjectRecord, RuntimeObjectTable, RuntimeWaitState,
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
    RuntimeThreadExecutionSelection, SchedulerThreadState,
};
pub use synchronization::{
    RUNTIME_CRITICAL_SECTION_LIMIT, RUNTIME_MUTEX_LIMIT, RuntimeCreateMutexAResult,
    RuntimeCriticalSection, RuntimeMutex, RuntimeMutexId, RuntimeMutexNameA,
    X86_CRITICAL_SECTION_BYTE_LENGTH,
};
pub use thread::{
    FLS_OUT_OF_INDEXES, LOGICAL_FLS_INDEX_NAMESPACE, LOGICAL_TLS_INDEX_NAMESPACE,
    RUNTIME_FLS_INDEX_LIMIT, RuntimeCallbackFrame, RuntimeCallbackFrameId, RuntimeCallbackScratch,
    RuntimeCancellation, RuntimeComApartmentModel, RuntimeComApartmentState,
    RuntimeComInitializationDisposition, RuntimeComInitializationOutcome,
    RuntimeComUninitializationOutcome, RuntimeFlsSlot, RuntimeThreadId,
    RuntimeThreadLifecycleState, RuntimeThreadStack, RuntimeThreadStackGrowth, RuntimeThreadState,
    RuntimeTlsValue, TLS_OUT_OF_INDEXES,
};
pub(crate) use timers::RUNTIME_TIMER_RESOLUTION_DISTINCT_PERIOD_LIMIT;
pub use timers::{
    RUNTIME_WAITABLE_TIMER_LIMIT, RuntimeCreateWaitableTimerAResult,
    RuntimeTimerResolutionMutationResult, RuntimeTimerResolutionState, RuntimeWaitableTimer,
    RuntimeWaitableTimerId,
};
pub use user32::{
    LOGICAL_CURSOR_HANDLE_NAMESPACE, LOGICAL_ICON_HANDLE_NAMESPACE,
    LOGICAL_WINDOW_CLASS_ATOM_NAMESPACE, LOGICAL_WINDOW_HANDLE_NAMESPACE,
    RUNTIME_CURSOR_HANDLE_BASE, RUNTIME_ICON_HANDLE_BASE, RUNTIME_SHARED_CURSOR_LIMIT,
    RUNTIME_SHARED_ICON_LIMIT, RUNTIME_SYSTEM_CURSOR_ARROW_RESOURCE_ID,
    RUNTIME_WINDOW_CLASS_ATOM_BASE, RUNTIME_WINDOW_CLASS_LIMIT,
    RUNTIME_WINDOW_CLASS_NAME_A_ENCODING, RUNTIME_WINDOW_CLASS_NAME_A_LIMIT,
    RUNTIME_WINDOW_HANDLE_BASE, RUNTIME_WINDOW_LIMIT, RUNTIME_WINDOW_NAME_A_ENCODING,
    RUNTIME_WINDOW_NAME_A_LIMIT, RUNTIME_WINDOW_PROCEDURE_ARGUMENT_BYTES,
    RUNTIME_WINDOW_STYLE_VISIBLE, RuntimeCreateWindowExAResult, RuntimeCursor, RuntimeCursorId,
    RuntimeCursorResource, RuntimeCursorSource, RuntimeIcon, RuntimeIconId, RuntimeIconResource,
    RuntimeLoadCursorAResult, RuntimeLoadIconAResult, RuntimeRegisterClassExAResult,
    RuntimeSetWindowPosCallbackAction, RuntimeSetWindowPosCallbackMessage,
    RuntimeSetWindowPosDefaultCallbackAction, RuntimeSetWindowPosDefaultCallbackMessage,
    RuntimeSetWindowPosDefaultOperation, RuntimeSetWindowPosOperation, RuntimeSetWindowPosRequest,
    RuntimeSetWindowPosVariant, RuntimeSetWindowStyleResult, RuntimeSystemCursorKind,
    RuntimeWindow, RuntimeWindowClass, RuntimeWindowClassCursorReference,
    RuntimeWindowClassIconReference, RuntimeWindowClassId, RuntimeWindowClassModule,
    RuntimeWindowClassNameA, RuntimeWindowClassReference, RuntimeWindowClassRegistration,
    RuntimeWindowCreationCallbackAction, RuntimeWindowCreationMessage,
    RuntimeWindowCreationRequest, RuntimeWindowGeometry, RuntimeWindowIconReference,
    RuntimeWindowId, RuntimeWindowLifecycle, RuntimeWindowNameA, RuntimeWindowProcedure,
    RuntimeWindowProcedureCallingConvention,
};
pub use waits::{
    RuntimeWaitCompletion, RuntimeWaitDisposition, RuntimeWaitRegistration,
    RuntimeWaitRegistrationId,
};

pub type RuntimeState = RuntimeKernel;

pub(crate) use address_space::validate_process_address_mapping_allocation_for_memory;
pub(crate) use com::{
    RUNTIME_COM_ADDRESS_OWNER, RuntimeComInterfaceRegistration, RuntimeComMethodRegistration,
    RuntimeComObjectRegistration,
};
pub(crate) use heaps::{
    RuntimeHeapAllocationPlan, RuntimeHeapBackingPlan, RuntimeHeapDestructionPlan,
};

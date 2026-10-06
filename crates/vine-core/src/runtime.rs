//! Windows userspace subsystem ownership and coordination.

mod address_space;
mod audio;
mod clock;
mod com;
mod displays;
mod files;
mod gdi;
mod gdiplus;
mod graphics;
mod heaps;
mod imm32;
mod input;
mod kernel;
mod messages;
pub(crate) use messages::RUNTIME_QUEUE_STATUS_VALID_FLAGS;
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
    ProcessAddressAllocationPurpose, ProcessAddressAllocationRecord,
    ProcessAddressAllocationReleasePolicy, ProcessAddressAllocationRequest,
    ProcessAddressMappingRequirements, ProcessAddressSpaceAllocator, RuntimeAddressRange,
    validate_process_address_mapping_allocation,
};
pub use audio::RUNTIME_AUDIO_NOTIFICATION_PACING_SERVICE;
pub(crate) use audio::RuntimeAudioBufferControlUpdate;
pub(crate) use audio::{BrowserAudioPreparation, PreparedBrowserAudio};
pub(crate) use audio::{
    RUNTIME_AUDIO_NOTIFICATION_LIMIT, RUNTIME_AUDIO_PAN_MAX_HUNDREDTHS_DB,
    RUNTIME_AUDIO_PAN_MIN_HUNDREDTHS_DB, RUNTIME_AUDIO_PCM_MAX_CHANNELS,
    RUNTIME_AUDIO_PCM_MAX_SAMPLES_PER_SECOND, RUNTIME_AUDIO_PCM_MIN_SAMPLES_PER_SECOND,
    RUNTIME_AUDIO_PLAYBACK_FREQUENCY_MAX_HZ, RUNTIME_AUDIO_PLAYBACK_FREQUENCY_MIN_HZ,
    RUNTIME_AUDIO_RESOURCE_ARENA_BASE, RUNTIME_AUDIO_RESOURCE_ARENA_SIZE,
    RUNTIME_AUDIO_VOLUME_MAX_HUNDREDTHS_DB, RUNTIME_AUDIO_VOLUME_MIN_HUNDREDTHS_DB,
    RuntimeAudioBufferDescriptor, RuntimeAudioBufferKind, RuntimeAudioBufferSegment,
    RuntimeAudioNotification, RuntimeAudioPcmFormat, RuntimeAudioPlaybackState,
};
pub use clock::RuntimeClockState;
pub use com::{
    RUNTIME_COM_IUNKNOWN_IID, RUNTIME_COM_MAX_INTERFACES_PER_OBJECT, RuntimeComIid,
    RuntimeComInterfaceId, RuntimeComObjectId,
};
#[cfg(test)]
pub(crate) use displays::RUNTIME_PRIMARY_MONITOR_HANDLE;
pub use displays::{RuntimeDisplayProfile, RuntimeRect};
pub use files::{
    RUNTIME_OPFS_READ_SERVICE, RUNTIME_SOURCE_FILE_LIMIT, RUNTIME_SOURCE_READ_MAX_BYTES,
    RUNTIME_SOURCE_READ_RESPONSE_CONTROL_BYTES, RUNTIME_SOURCE_READ_SERVICE,
    RUNTIME_WRITABLE_FILE_MAX_IO_BYTES, RuntimePathResolution, RuntimeSourceFileManifest,
    RuntimeSourceMountManifest, RuntimeSourceReadHostResponse, RuntimeSourceSnapshotManifest,
};
pub(crate) use files::{RuntimeFileBacking, RuntimeSourceFileId};
#[cfg(test)]
pub(crate) use gdi::RuntimeGdiObjectKind;
pub use gdi::{
    RUNTIME_GDI_BLACK_BRUSH_INDEX, RUNTIME_GDI_DIB_BI_RGB, RUNTIME_GDI_DIB_RGB_COLORS,
    RUNTIME_LOG_FONT_A_BYTE_LENGTH, RUNTIME_LOG_FONT_A_FACE_CAPACITY, RuntimeDeviceContextTarget,
    RuntimeGdiDibSectionDescriptor, RuntimeGdiRgbQuad, RuntimeGdiTextRasterTarget, RuntimeLogFontA,
};
#[cfg(test)]
pub(crate) use gdiplus::{RUNTIME_GDIPLUS_SESSION_LIMIT, RUNTIME_GDIPLUS_SESSION_TOKEN_PREFIX};
pub(crate) use graphics::RuntimeGraphicsDeviceId;
pub use graphics::{
    RUNTIME_GRAPHICS_RESOURCE_ARENA_BASE, RUNTIME_GRAPHICS_RESOURCE_ARENA_SIZE,
    RuntimeGraphicsBlendFactor, RuntimeGraphicsBlendOperation, RuntimeGraphicsColorFormat,
    RuntimeGraphicsCullMode, RuntimeGraphicsFlexibleVertexFormat, RuntimeGraphicsPresentMode,
    RuntimeGraphicsRenderStateUpdate, RuntimeGraphicsSamplerStateUpdate,
    RuntimeGraphicsTextureArgument, RuntimeGraphicsTextureDescriptor, RuntimeGraphicsTextureFilter,
    RuntimeGraphicsTextureOperation, RuntimeGraphicsTexturePool,
    RuntimeGraphicsTextureStageStateUpdate, RuntimeGraphicsTextureUsage,
    RuntimeGraphicsVertexBufferDescriptor, RuntimeGraphicsVertexBufferPool,
    RuntimeGraphicsVertexBufferUsage,
};
#[cfg(test)]
pub(crate) use heaps::{
    RUNTIME_PRIVATE_HEAP_ALIGNMENT, RUNTIME_PRIVATE_HEAP_FREED_BYTE,
    RUNTIME_PRIVATE_HEAP_UNINITIALIZED_BYTE, RuntimeHeapAllocationId,
};
pub use heaps::{
    RUNTIME_PRIVATE_HEAP_ARENA_BASE, RUNTIME_PRIVATE_HEAP_ARENA_STRIDE,
    RUNTIME_PRIVATE_HEAP_BACKING_LIMIT, RuntimeHeapInitialization,
};
pub use input::{
    RuntimeCursorPositionProfile, RuntimeInputProfile, RuntimeKeyboardSourceEvent,
    RuntimeKeyboardSourceEventKind, RuntimePointerButton, RuntimePointerButtonSourceEvent,
    RuntimePointerButtonSourceEventKind, RuntimePointerPositionSourceEvent,
};
pub(crate) use kernel::RuntimeFindFirstFileOutcome;
pub use kernel::*;
#[cfg(test)]
pub(crate) use messages::RuntimeMessageReceiveDisposition;
pub(crate) use messages::RuntimeThreadMessage;
pub(crate) use modules::RuntimeSystemExportTable;
pub use modules::{
    RuntimeDllLifecycleAction, RuntimeDllLifecyclePhase, RuntimeGuestImport,
    RuntimeGuestImportBinding, RuntimeGuestImportResolution, RuntimeModule,
    RuntimeModuleDependencyEdge, RuntimeSystemExportEndpoint,
};
pub use objects::{
    RuntimeHandle, RuntimeObjectId, RuntimeObjectKey, RuntimeObjectKind, RuntimeWaitState,
};
pub use process::{
    RUNTIME_PROCESS_IMAGE_PATH_A_MAX_BYTES, RuntimeCurrentDirectoryA, RuntimeProcessId,
    RuntimeProcessImagePathA, RuntimeStartupInfo,
};
#[cfg(test)]
pub(crate) use process::{
    RUNTIME_STANDARD_ERROR_HANDLE, RUNTIME_STANDARD_ERROR_SELECTOR, RUNTIME_STANDARD_INPUT_HANDLE,
    RUNTIME_STANDARD_INPUT_SELECTOR, RUNTIME_STANDARD_OUTPUT_HANDLE,
    RUNTIME_STANDARD_OUTPUT_SELECTOR, RuntimeProcessLifecycleState, X86_STARTUP_INFO_W_BYTE_LENGTH,
};
pub use registries::RuntimeDynamicEndpoint;
#[cfg(test)]
pub(crate) use scheduler::RUNTIME_API_BOUNDARY_QUANTUM;
pub use scheduler::{
    RuntimeApiBoundaryDisposition, RuntimeHostCompletion, RuntimeSchedulerEvent,
    SchedulerThreadState,
};
pub use synchronization::{
    RuntimeCriticalSection, RuntimeMutexId, RuntimeMutexNameA, X86_CRITICAL_SECTION_BYTE_LENGTH,
};
#[cfg(test)]
pub(crate) use thread::RUNTIME_FLS_INDEX_LIMIT;
pub(crate) use thread::RuntimeCreateThreadRequest;
pub(crate) use thread::TLS_OUT_OF_INDEXES;
pub use thread::{
    FLS_OUT_OF_INDEXES, RUNTIME_GUEST_THREAD_LIMIT, RUNTIME_GUEST_THREAD_STACK_ARENA_BASE,
    RUNTIME_GUEST_THREAD_STACK_ARENA_SIZE, RuntimeCallbackFrame, RuntimeCallbackFrameId,
    RuntimeCallbackScratch, RuntimeComApartmentModel, RuntimeThreadId, RuntimeThreadLifecycleState,
    RuntimeThreadPriority,
};
pub use timers::RuntimeWaitableTimer;
#[cfg(test)]
pub(crate) use timers::{
    RuntimeWaitableTimerArm, RuntimeWaitableTimerExpiration, RuntimeWaitableTimerId,
};
#[cfg(test)]
pub(crate) use user32::RUNTIME_MENU_HANDLE_BASE;
pub use user32::{
    RUNTIME_MESSAGE_BOX_SERVICE, RUNTIME_SYSTEM_CURSOR_ARROW_RESOURCE_ID,
    RUNTIME_WINDOW_CLASS_NAME_A_LIMIT, RUNTIME_WINDOW_NAME_A_LIMIT, RUNTIME_WINDOW_STYLE_VISIBLE,
    RUNTIME_X86_PAINT_STRUCT_BYTE_LENGTH, RuntimeCursorResource, RuntimeCursorSource,
    RuntimeIconResource, RuntimeImeStatusWindowRequest, RuntimeMessageBox, RuntimeMessageBoxButton,
    RuntimeMessageBoxIcon, RuntimeSetWindowPosCallbackAction, RuntimeSetWindowPosCallbackMessage,
    RuntimeSetWindowPosDefaultCallbackAction, RuntimeSetWindowPosDefaultCallbackMessage,
    RuntimeSetWindowPosDefaultOperation, RuntimeSetWindowPosOperation, RuntimeSetWindowPosRequest,
    RuntimeSetWindowPosVariant, RuntimeSystemCursorKind, RuntimeUpdateWindowOperation,
    RuntimeWindow, RuntimeWindowClass, RuntimeWindowClassCursorReference,
    RuntimeWindowClassIconReference, RuntimeWindowClassModule, RuntimeWindowClassReference,
    RuntimeWindowClassRegistration, RuntimeWindowCreationCallbackAction,
    RuntimeWindowCreationMessage, RuntimeWindowCreationRequest, RuntimeWindowGeometry,
    RuntimeWindowId, RuntimeWindowLifecycle, RuntimeWindowNameA, RuntimeWindowProcedure,
};
pub use waits::{
    RUNTIME_WAITABLE_TIMER_PACING_SERVICE, RuntimeWaitCompletion, RuntimeWaitDisposition,
    RuntimeWaitForSingleObjectExBegin, RuntimeWaitForSingleObjectExOutcome,
    RuntimeWaitRegistration, RuntimeWaitRegistrationId,
};

pub(crate) use com::{
    RuntimeComClass, RuntimeComInterfaceRegistration, RuntimeComMethodRegistration,
    RuntimeComMethodTarget,
};

#[cfg(test)]
pub(crate) use com::RuntimeComInterfaceRecord;
#[cfg(test)]
pub(crate) use modules::{
    DLL_PROCESS_ATTACH, DLL_PROCESS_DETACH, DLL_PROCESS_STATIC_RESERVED, RuntimeDllCallbackReason,
};

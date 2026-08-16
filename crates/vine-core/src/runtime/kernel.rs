//! Runtime aggregate root and cross-subsystem transaction coordinator.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::ops::Deref;

use serde::{Serialize, Serializer};

use crate::bootstrap::InitialThreadStack;
use crate::callbacks::{
    CALLBACK_SCRATCH_ARENA_BASE, CALLBACK_SCRATCH_ARENA_SIZE, CALLBACK_SCRATCH_SLOT_SIZE,
};
use crate::dynamic_exports::DynamicExportThunk;
use crate::environment::InitialProcessEnvironment;
use crate::materialize::ImageMaterialization;
use crate::memory::{
    GUEST_PAGE_SIZE, GuestAddress, GuestMemory, GuestMemoryError, GuestMemoryErrorClass,
    GuestRange, GuestWriteResult, MemoryAccess, MemoryInvalidation, MemoryOwnerId, MemoryOwnerKind,
    MemoryRegionKind, PageProtection,
};
use crate::module_graph::{RuntimeModuleClass, RuntimeModuleGraph, RuntimeModuleId};
use crate::module_images::{
    RuntimeModuleImage, RuntimeModuleImageError, RuntimeModuleImageRegistry, RuntimeResolvedExport,
};
use crate::pe32::{ExportLookup, LoaderIngressReport, ResourceIdentifier};
use crate::snapshot::ModuleIdentity;

use super::address_space::{
    ProcessAddressAllocationClassification, ProcessAddressAllocationLifetime,
    ProcessAddressAllocationPurpose, ProcessAddressAllocationRecord,
    ProcessAddressAllocationReleasePolicy, ProcessAddressAllocationRequest,
    ProcessAddressReleasePlan, ProcessAddressReservationBatchPlan, ProcessAddressReservationPlan,
    ProcessAddressSpaceAllocator, ProcessAddressSpaceLayout, RuntimeAddressRange,
};
use super::audio::{
    RUNTIME_AUDIO_NOTIFICATION_LIMIT, RUNTIME_AUDIO_NOTIFICATION_PACING_SERVICE,
    RUNTIME_AUDIO_RESOURCE_ADDRESS_OWNER, RuntimeAudio, RuntimeAudioBuffer,
    RuntimeAudioBufferBackingPlan, RuntimeAudioBufferControlPlan, RuntimeAudioBufferControlRequest,
    RuntimeAudioBufferControlResult, RuntimeAudioBufferCreateRequest,
    RuntimeAudioBufferCreationPlan, RuntimeAudioBufferFormatPlan, RuntimeAudioBufferFormatRequest,
    RuntimeAudioBufferFormatResult, RuntimeAudioBufferId, RuntimeAudioBufferLock,
    RuntimeAudioBufferLockPlan, RuntimeAudioBufferLockRequest, RuntimeAudioBufferNotificationPlan,
    RuntimeAudioBufferNotificationRequest, RuntimeAudioBufferPlayPlan,
    RuntimeAudioBufferPlayRequest, RuntimeAudioBufferPlayResult, RuntimeAudioBufferUnlockPlan,
    RuntimeAudioBufferUnlockRequest, RuntimeAudioBufferUnlockResult, RuntimeAudioComRetirementPlan,
    RuntimeAudioCooperativeLevelPlan, RuntimeAudioCooperativeLevelRequest,
    RuntimeAudioCooperativeLevelResult, RuntimeAudioCooperativeLevelUpdateRequest,
    RuntimeAudioDevice, RuntimeAudioDeviceCreateRequest, RuntimeAudioDeviceCreationPlan,
    RuntimeAudioDeviceId, RuntimeAudioNotification, RuntimeAudioNotificationWaitPlan,
    RuntimeAudioWindowTarget,
};
use super::clock::{
    RuntimeClockAdvancePlan, RuntimeClockProfile, RuntimeClockState,
    RuntimeScheduledExecutionClockState, RuntimeTimerClockState,
};
use super::com::{
    RUNTIME_COM_ADDRESS_OWNER, RuntimeCom, RuntimeComIid, RuntimeComInterfaceId,
    RuntimeComInterfaceRecord, RuntimeComMethodTarget, RuntimeComMutationPlan, RuntimeComObjectId,
    RuntimeComObjectRegistration, RuntimeComReferenceResult, RuntimeComRegistrationResult,
};
use super::displays::{RuntimeDisplayProfile, RuntimeDisplays, RuntimeLogicalMonitor};
use super::files::{
    RUNTIME_SOURCE_READ_SERVICE, RUNTIME_WRITABLE_COMMIT_SERVICE,
    RUNTIME_WRITABLE_DIRECTORY_CREATE_SERVICE, RUNTIME_WRITABLE_READ_SERVICE, RuntimeFileSearch,
    RuntimeFileSearchAdvancePlan, RuntimeFileSearchEntry, RuntimeFileSearchId,
    RuntimeFileSearchOpenPlan, RuntimeFileSearchRetirementPlan, RuntimeFiles,
    RuntimeFindFirstFileResult, RuntimeSourceFile, RuntimeSourceFileRetirementPlan,
    RuntimeSourceReadBeginPlan, RuntimeSourceReadCancellationPlan, RuntimeSourceReadCompletion,
    RuntimeSourceReadCompletionPlan, RuntimeSourceReadHostResponse, RuntimeSourceReadRequest,
    RuntimeSourceSnapshot, RuntimeSourceSnapshotManifest, RuntimeWritableCommitBeginPlan,
    RuntimeWritableCommitCancellationPlan, RuntimeWritableCommitCompletion,
    RuntimeWritableCommitCompletionPlan, RuntimeWritableCommitHostResponse,
    RuntimeWritableCommitRequest, RuntimeWritableDirectoryCreateBeginPlan,
    RuntimeWritableDirectoryCreateCancellationPlan, RuntimeWritableDirectoryCreateCompletion,
    RuntimeWritableDirectoryCreateCompletionPlan, RuntimeWritableDirectoryCreateHostResponse,
    RuntimeWritableDirectoryCreateRequest, RuntimeWritableProfile, RuntimeWritableProfileManifest,
    RuntimeWritableReadPlan, RuntimeWritableReadResult, RuntimeWritableWritePlan,
    RuntimeWritableWriteResult,
};
use super::gdi::{
    RUNTIME_GDI_RESOURCE_ADDRESS_OWNER, RuntimeCreateCompatibleDcResult,
    RuntimeCreateDibSectionResult, RuntimeCreateFontAResult, RuntimeDeleteGdiObjectPlan,
    RuntimeDeviceContext, RuntimeDeviceContextId, RuntimeDeviceContextTarget, RuntimeGdi,
    RuntimeGdiComRetirementGuard, RuntimeGdiDibSectionDescriptor, RuntimeGdiGraphicsSurfaceTarget,
    RuntimeGdiObject, RuntimeGdiObjectId, RuntimeGdiScreenTarget, RuntimeGdiSurface,
    RuntimeGdiSurfaceId, RuntimeGdiTextRasterResult, RuntimeGdiTextRasterTarget,
    RuntimeGdiWindowTarget, RuntimeGetDcResult, RuntimeGetGraphicsSurfaceDcPlan,
    RuntimeGetGraphicsSurfaceDcResult, RuntimeGetScreenDcResult, RuntimeGetStockObjectResult,
    RuntimeLogFontA, RuntimeSelectGdiObjectResult,
};
use super::gdiplus::{RuntimeGdiplus, RuntimeGdiplusSession, RuntimeGdiplusStartupRequest};
use super::graphics::{
    RUNTIME_GRAPHICS_RESOURCE_ADDRESS_OWNER, RuntimeGraphics, RuntimeGraphicsBackBufferClearPlan,
    RuntimeGraphicsBackBufferClearRequest, RuntimeGraphicsBackBufferClearResult,
    RuntimeGraphicsComRetirementPlan, RuntimeGraphicsDevice, RuntimeGraphicsDeviceCreateRequest,
    RuntimeGraphicsDeviceCreationPlan, RuntimeGraphicsDeviceCreationRequest,
    RuntimeGraphicsDeviceId, RuntimeGraphicsDrawCapturePlan, RuntimeGraphicsDrawCaptureRequest,
    RuntimeGraphicsDrawSubmissionPlan, RuntimeGraphicsDrawSubmissionResult,
    RuntimeGraphicsFlexibleVertexFormatUpdatePlan,
    RuntimeGraphicsFlexibleVertexFormatUpdateRequest,
    RuntimeGraphicsFlexibleVertexFormatUpdateResult, RuntimeGraphicsPresentPlan,
    RuntimeGraphicsPresentRequest, RuntimeGraphicsPresentResult,
    RuntimeGraphicsRenderStateUpdatePlan, RuntimeGraphicsRenderStateUpdateRequest,
    RuntimeGraphicsRenderStateUpdateResult, RuntimeGraphicsSamplerStateUpdatePlan,
    RuntimeGraphicsSamplerStateUpdateRequest, RuntimeGraphicsSamplerStateUpdateResult,
    RuntimeGraphicsSceneBeginPlan, RuntimeGraphicsSceneBeginRequest,
    RuntimeGraphicsSceneBeginResult, RuntimeGraphicsSceneEndPlan, RuntimeGraphicsSceneEndRequest,
    RuntimeGraphicsSceneEndResult, RuntimeGraphicsStreamSourceUpdatePlan,
    RuntimeGraphicsStreamSourceUpdateRequest, RuntimeGraphicsStreamSourceUpdateResult,
    RuntimeGraphicsSurface, RuntimeGraphicsSurfaceCreateRequest,
    RuntimeGraphicsSurfaceCreationPlan, RuntimeGraphicsSurfaceFillRequest,
    RuntimeGraphicsSurfaceId, RuntimeGraphicsTexture, RuntimeGraphicsTextureBackingPlan,
    RuntimeGraphicsTextureBindingUpdatePlan, RuntimeGraphicsTextureBindingUpdateRequest,
    RuntimeGraphicsTextureBindingUpdateResult, RuntimeGraphicsTextureCreateRequest,
    RuntimeGraphicsTextureCreationPlan, RuntimeGraphicsTextureDirtyRegionMarkPlan,
    RuntimeGraphicsTextureDirtyRegionMarkRequest, RuntimeGraphicsTextureDirtyRegionMarkResult,
    RuntimeGraphicsTextureId, RuntimeGraphicsTextureLock, RuntimeGraphicsTextureLockPlan,
    RuntimeGraphicsTextureLockRequest, RuntimeGraphicsTextureStageStateUpdatePlan,
    RuntimeGraphicsTextureStageStateUpdateRequest, RuntimeGraphicsTextureStageStateUpdateResult,
    RuntimeGraphicsTextureUnlockPlan, RuntimeGraphicsTextureUnlockResult,
    RuntimeGraphicsTextureUpdatePlan, RuntimeGraphicsTextureUpdateRequest,
    RuntimeGraphicsTextureUpdateResult, RuntimeGraphicsVertexBuffer,
    RuntimeGraphicsVertexBufferCreateRequest, RuntimeGraphicsVertexBufferCreationPlan,
    RuntimeGraphicsVertexBufferId, RuntimeGraphicsVertexBufferLock,
    RuntimeGraphicsVertexBufferLockPlan, RuntimeGraphicsVertexBufferLockRequest,
    RuntimeGraphicsVertexBufferUnlockPlan, RuntimeGraphicsVertexBufferUnlockResult,
    RuntimeGraphicsWindowTarget, texture_byte_layout,
};
use super::heaps::{
    RuntimeHeap, RuntimeHeapAllocation, RuntimeHeapAllocationPlan, RuntimeHeapBacking,
    RuntimeHeapCreationPlan, RuntimeHeapDestructionPlan, RuntimeHeapFreePlan,
    RuntimeHeapInitialization, RuntimeHeapPageMaterialization, RuntimeHeapTeardown, RuntimeHeaps,
    RuntimeLargeHeapAllocationBacking,
};
use super::imm32::{
    RuntimeImeAssociationResult, RuntimeImeContext, RuntimeImeContextAssociation,
    RuntimeImeContextId, RuntimeImm32,
};
use super::input::{
    RuntimeAsyncKeyStateResult, RuntimeCursorPosition, RuntimeInput, RuntimeInputProfile,
    RuntimeKeyboardEventReceipt, RuntimeKeyboardSourceEvent, RuntimeKeyboardState,
    RuntimeMouseButtonMapping, RuntimePointerButtonEventReceipt,
    RuntimePointerButtonMessageProfile, RuntimePointerButtonSourceEvent,
    RuntimePointerPositionEventReceipt, RuntimePointerPositionSourceEvent,
    RuntimeSwapMouseButtonResult,
};
use super::messages::{
    RuntimeMessageClearPlan, RuntimeMessagePeekDisposition, RuntimeMessagePeekPlan,
    RuntimeMessageReceiveDisposition, RuntimeMessageReceivePlan, RuntimeMessageSource,
    RuntimeMessageWake, RuntimeMessages, RuntimeThreadMessage,
};
#[cfg(test)]
use super::modules::LOGICAL_LOADED_MODULE_NAMESPACE;
use super::modules::{
    RuntimeDllLifecycleAction, RuntimeDllLifecyclePhase, RuntimeDllLifecyclePlan,
    RuntimeGuestImport, RuntimeGuestImportBinding, RuntimeGuestImportBindingPlan, RuntimeModule,
    RuntimeModuleDependencyEdge, RuntimeModuleRecords, RuntimeModules, RuntimeSystemExportTable,
    RuntimeSystemModuleLoad, runtime_module_image_error, runtime_module_image_state_error,
};
use super::objects::{
    RuntimeCloseHandleObjectMutationPlan, RuntimeEvent, RuntimeHandle, RuntimeHandleEntry,
    RuntimeObjectId, RuntimeObjectKey, RuntimeObjectKind, RuntimeObjectRecord,
    RuntimeObjectRegistrationPlan, RuntimeObjectTable, RuntimeWaitState,
};
use super::process::{
    RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING, RUNTIME_PROCESS_IMAGE_PATH_A_MAX_BYTES,
    RuntimeCommandLine, RuntimeCurrentDirectoryA, RuntimeEnvironmentStringCopy,
    RuntimeEnvironmentStrings, RuntimeProcessCancellation, RuntimeProcessEnvironment,
    RuntimeProcessId, RuntimeProcessImagePathA, RuntimeProcessLifecycleState, RuntimeProcessState,
    RuntimeSetCurrentDirectoryAPlan, RuntimeStandardHandle, RuntimeStartupInfo,
    X86_STARTUP_INFO_W_BYTE_LENGTH,
};
use super::process::{RuntimeProcessDirectories, RuntimeProcessMutationPlan};
#[cfg(test)]
use super::registries::LOGICAL_ENCODED_POINTER_NAMESPACE;
use super::registries::{RuntimeDynamicEndpoint, RuntimeEncodedPointer, RuntimeRegistries};
use super::scheduler::{
    RuntimeApiBoundaryDisposition, RuntimeHostCompletion, RuntimeHostRequest, RuntimeScheduler,
    RuntimeSchedulerEvent, RuntimeThreadExecutionSelection, SchedulerThreadState,
};
use super::scheduler::{
    RuntimeHostCompletionPlan, RuntimeHostRequestCancellationPlan, RuntimeHostRequestPlan,
    RuntimeSchedulerLanePlan,
};
use super::synchronization::RuntimeSynchronization;
use super::synchronization::{
    RuntimeCreateMutexAResult, RuntimeCriticalSection, RuntimeMutex, RuntimeMutexId,
    RuntimeMutexNameA, RuntimeMutexRetirementPlan,
};
use super::thread::{
    RUNTIME_GUEST_THREAD_ENVIRONMENT_ARENA_BASE, RUNTIME_GUEST_THREAD_ENVIRONMENT_ARENA_SIZE,
    RUNTIME_GUEST_THREAD_STACK_ARENA_BASE, RUNTIME_GUEST_THREAD_STACK_ARENA_SIZE,
    RuntimeCallbackFrame, RuntimeCallbackFrameId, RuntimeCallbackFrameIdPlan,
    RuntimeCallbackScratch, RuntimeCancellation, RuntimeComApartmentModel,
    RuntimeComApartmentState, RuntimeComInitializationOutcome, RuntimeComUninitializationOutcome,
    RuntimeCreateThreadRequest, RuntimeCreateThreadResult, RuntimeFlsSlot,
    RuntimeGuestThreadBootstrap, RuntimeResumeThreadResult, RuntimeThreadEnvironment,
    RuntimeThreadId, RuntimeThreadLifecycleState, RuntimeThreadMutationPlan, RuntimeThreadPriority,
    RuntimeThreadStack, RuntimeThreadStackGrowth, RuntimeThreadState, RuntimeThreads,
    RuntimeTlsValue,
};
use super::timers::{
    RuntimeCreateWaitableTimerAResult, RuntimeTimerResolutionMutationResult,
    RuntimeTimerResolutionState, RuntimeTimers, RuntimeTimersTeardownPlan, RuntimeWaitableTimer,
    RuntimeWaitableTimerExpiration, RuntimeWaitableTimerExpirationPlan, RuntimeWaitableTimerId,
    RuntimeWaitableTimerRetirementPlan, RuntimeWindowTimer, RuntimeWindowTimerId,
    RuntimeWindowTimerRegistrationRequest, RuntimeWindowTimerRegistrationResult,
};
use super::user32::{
    RuntimeBeginPaintRequest, RuntimeCreateWindowExAResult, RuntimeCursor, RuntimeCursorId,
    RuntimeCursorSource, RuntimeDefaultImeWindowResult, RuntimeEnableMenuItemResult,
    RuntimeEndPaintRequest, RuntimeGetSystemMenuResult, RuntimeIcon, RuntimeIconId,
    RuntimeIconResource, RuntimeImeStatusWindowRequest, RuntimeImeWindow, RuntimeImeWindowId,
    RuntimeLoadCursorAResult, RuntimeLoadIconAResult, RuntimeMenuId, RuntimePaintSession,
    RuntimeRegisterClassExAResult, RuntimeSetWindowPosCallbackAction,
    RuntimeSetWindowPosCallbackMessage, RuntimeSetWindowPosDefaultCallbackAction,
    RuntimeSetWindowPosDefaultCallbackMessage, RuntimeSetWindowPosDefaultOperation,
    RuntimeSetWindowPosOperation, RuntimeSetWindowPosRequest, RuntimeSetWindowPosVariant,
    RuntimeSetWindowStyleResult, RuntimeSystemMenu, RuntimeUpdateWindowCallbackAction,
    RuntimeUpdateWindowOperation, RuntimeUpdateWindowRequest, RuntimeUser32, RuntimeWindow,
    RuntimeWindowClass, RuntimeWindowClassId, RuntimeWindowClassRegistration,
    RuntimeWindowCreationCallbackAction, RuntimeWindowCreationMessage,
    RuntimeWindowCreationRequest, RuntimeWindowFileDropAcceptanceResult, RuntimeWindowGeometry,
    RuntimeWindowId, RuntimeWindowLifecycle, RuntimeWindowNameA, RuntimeWindowProcedure,
};
use super::virtual_memory::{RuntimeVirtualMemoryReservation, RuntimeVirtualMemoryReserveRequest};
use super::waits::{
    RUNTIME_WAITABLE_TIMER_PACING_SERVICE, RuntimeWaitCompletion, RuntimeWaitDisposition,
    RuntimeWaitForSingleObjectExBegin, RuntimeWaitForSingleObjectExOutcome,
    RuntimeWaitForSingleObjectExResult, RuntimeWaitRegistration, RuntimeWaitRegistrationPlan,
    RuntimeWaitableTimerPacing,
};

const WM_MOUSEMOVE: u32 = 0x0200;

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
pub struct RuntimePointerButtonIngressReceipt {
    pub profile_id: String,
    pub window_id: RuntimeWindowId,
    pub window_handle: u32,
    pub target_thread_id: RuntimeThreadId,
    pub client_x: i32,
    pub client_y: i32,
    pub input: RuntimePointerButtonEventReceipt,
    pub message: RuntimeThreadMessage,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimePointerPositionIngressReceipt {
    pub profile_id: String,
    pub window_id: RuntimeWindowId,
    pub window_handle: u32,
    pub target_thread_id: RuntimeThreadId,
    pub client_x: i32,
    pub client_y: i32,
    pub input: RuntimePointerPositionEventReceipt,
    pub message: RuntimeThreadMessage,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum RuntimePageFaultResolution {
    ThreadStackGrowth {
        growth: RuntimeThreadStackGrowth,
    },
    PrivateHeapPageMaterialization {
        materialization: RuntimeHeapPageMaterialization,
    },
}

struct RuntimeThreadExitRequestPlan {
    thread_id: RuntimeThreadId,
    thread_plan: RuntimeThreadMutationPlan,
    scheduler_plan: RuntimeSchedulerLanePlan,
    cancelled_wait: Option<RuntimeWaitRegistration>,
    host_request_cancellation: RuntimeHostRequestCancellationPlan,
    source_read_cancellation: RuntimeSourceReadCancellationPlan,
    writable_commit_cancellation: RuntimeWritableCommitCancellationPlan,
    writable_directory_create_cancellation: RuntimeWritableDirectoryCreateCancellationPlan,
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
    process_plan: RuntimeProcessMutationPlan,
    thread_plans: Vec<RuntimeThreadExitRequestPlan>,
    timer_teardown: Option<RuntimeTimersTeardownPlan>,
    cancellation: RuntimeProcessCancellation,
}

struct RuntimePayloadObjectTeardownPlan {
    object_plan: Option<RuntimeCloseHandleObjectMutationPlan>,
    mutex_plans: Vec<RuntimeMutexRetirementPlan>,
    waitable_timer_plans: Vec<RuntimeWaitableTimerRetirementPlan>,
    source_file_plans: Vec<RuntimeSourceFileRetirementPlan>,
    file_search_plans: Vec<RuntimeFileSearchRetirementPlan>,
    retired_handles: usize,
}

pub(crate) struct RuntimeSourceReadAggregateCompletionPlan {
    scheduler: super::scheduler::RuntimeHostCompletionPlan,
    source_read: RuntimeSourceReadCompletionPlan,
}

pub(crate) struct RuntimeWritableCommitAggregateCompletionPlan {
    scheduler: super::scheduler::RuntimeHostCompletionPlan,
    writable_commit: RuntimeWritableCommitCompletionPlan,
    object: Option<RuntimeCloseHandleObjectMutationPlan>,
    retirement: Option<RuntimeSourceFileRetirementPlan>,
}

pub(crate) struct RuntimeWritableDirectoryCreateAggregateCompletionPlan {
    scheduler: super::scheduler::RuntimeHostCompletionPlan,
    directory_create: RuntimeWritableDirectoryCreateCompletionPlan,
}

pub(crate) struct RuntimeWaitableTimerPacingCompletionPlan {
    scheduler: RuntimeHostCompletionPlan,
    registration: RuntimeWaitRegistration,
    clock: RuntimeClockAdvancePlan,
    expiration: Option<RuntimeWaitableTimerExpirationPlan>,
    wait_updates: Option<Vec<(RuntimeObjectId, RuntimeObjectRecord)>>,
    result: RuntimeWaitForSingleObjectExResult,
}

pub(crate) struct RuntimeAudioNotificationWaitCompletionPlan {
    scheduler: RuntimeHostCompletionPlan,
    audio: Option<RuntimeAudioNotificationWaitPlan>,
    result_index: Option<u32>,
    payload: Vec<u8>,
}

impl RuntimeAudioNotificationWaitCompletionPlan {
    pub(crate) fn encode_host_payload(&self) -> Vec<u8> {
        self.payload.clone()
    }

    pub(crate) const fn result_index(&self) -> Option<u32> {
        self.result_index
    }

    pub(crate) fn accepts_host_payload(&self, payload: &[u8]) -> bool {
        match self.audio.as_ref() {
            Some(audio) => audio.matches_rebased_host_payload(payload),
            None => self.encode_host_payload() == payload,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeAudioNotificationWaitBegin {
    pub request: RuntimeHostRequest,
    pub delay_milliseconds: u32,
    pub result_index: Option<u32>,
    pub payload: Vec<u8>,
}

fn encode_inactive_audio_notification_wait_payload(
    handles: &[RuntimeHandle],
    timeout_milliseconds: u32,
) -> Vec<u8> {
    let words = [
        2,
        timeout_milliseconds,
        timeout_milliseconds,
        handles.len() as u32,
    ];
    let mut payload = Vec::with_capacity((words.len() + handles.len()) * 4);
    for word in words
        .into_iter()
        .chain(handles.iter().map(|handle| handle.0))
    {
        payload.extend_from_slice(&word.to_le_bytes());
    }
    payload
}

fn decode_audio_notification_wait_handles(
    payload: &[u8],
    timeout_milliseconds: u32,
) -> Result<Vec<RuntimeHandle>, RuntimeStateError> {
    if !payload.len().is_multiple_of(4) {
        return Err(RuntimeStateError::invalid_input(
            "runtime-audio",
            "audio-notification-wait-payload-invalid",
            "audio notification wait payload is not DWORD aligned",
        ));
    }
    let words = payload
        .chunks_exact(4)
        .map(|bytes| u32::from_le_bytes(bytes.try_into().unwrap()))
        .collect::<Vec<_>>();
    let (payload_timeout, handle_count, handles_start) = match words.first() {
        Some(1) => (words.get(11), words.get(12), 13),
        Some(2) => (words.get(2), words.get(3), 4),
        _ => {
            return Err(RuntimeStateError::invalid_input(
                "runtime-audio",
                "audio-notification-wait-payload-invalid",
                "audio notification wait payload has an unknown version",
            ));
        }
    };
    let handle_count = handle_count.copied().unwrap_or(0) as usize;
    if payload_timeout != Some(&timeout_milliseconds)
        || handle_count == 0
        || handle_count > RUNTIME_AUDIO_NOTIFICATION_LIMIT
        || words.len() != handles_start + handle_count
    {
        return Err(RuntimeStateError::invalid_input(
            "runtime-audio",
            "audio-notification-wait-payload-invalid",
            "audio notification wait payload has inconsistent timeout or handle bounds",
        ));
    }
    Ok(words[handles_start..]
        .iter()
        .copied()
        .map(RuntimeHandle)
        .collect())
}

impl RuntimeWaitableTimerPacingCompletionPlan {
    pub(crate) const fn result(&self) -> RuntimeWaitForSingleObjectExResult {
        self.result
    }
}

impl RuntimeWritableCommitAggregateCompletionPlan {
    pub(crate) fn completion(&self) -> &RuntimeWritableCommitCompletion {
        self.writable_commit.completion()
    }
}

impl RuntimeWritableDirectoryCreateAggregateCompletionPlan {
    pub(crate) fn completion(&self) -> &RuntimeWritableDirectoryCreateCompletion {
        self.directory_create.completion()
    }
}

impl RuntimeSourceReadAggregateCompletionPlan {
    pub(crate) fn response(&self) -> &RuntimeSourceReadHostResponse {
        self.source_read.response()
    }

    pub(crate) const fn buffer(&self) -> GuestAddress {
        self.source_read.buffer()
    }

    pub(crate) const fn bytes_read(&self) -> GuestAddress {
        self.source_read.bytes_read()
    }

    pub(crate) fn completion(&self) -> &RuntimeSourceReadCompletion {
        self.source_read.completion()
    }
}

pub(crate) struct RuntimeCallbackMutationPlan {
    thread: RuntimeThreadMutationPlan,
    scheduler: RuntimeSchedulerLanePlan,
    callback_frame_id: Option<RuntimeCallbackFrameIdPlan>,
    entered: Option<RuntimeCallbackFrame>,
    exited: Option<RuntimeCallbackFrame>,
}

impl RuntimeCallbackMutationPlan {
    pub(crate) fn entered_frame(&self) -> Option<RuntimeCallbackFrame> {
        self.entered
    }
}

pub(crate) struct RuntimeComReleasePlan {
    com: RuntimeComMutationPlan,
    address_space: Option<ProcessAddressReleasePlan>,
    audio: Option<RuntimeAudioComRetirementPlan>,
    graphics: Option<RuntimeGraphicsComRetirementPlan>,
    gdi: Option<RuntimeGdiComRetirementGuard>,
    memory_retirements: Vec<(MemoryOwnerId, GuestRange)>,
}

impl RuntimeComReleasePlan {
    pub(crate) fn result(&self) -> RuntimeComReferenceResult {
        self.com.reference_result()
    }

    pub(crate) fn memory_retirements(&self) -> Vec<(MemoryOwnerId, GuestRange)> {
        self.memory_retirements.clone()
    }
}

pub(crate) struct RuntimeGraphicsVertexBufferInstallationPlan {
    address_space: ProcessAddressReservationBatchPlan,
    graphics: RuntimeGraphicsVertexBufferCreationPlan,
}

pub(crate) struct RuntimeAudioBufferLockInstallationPlan {
    address_space: Option<ProcessAddressReservationBatchPlan>,
    audio: RuntimeAudioBufferLockPlan,
}

pub(crate) struct RuntimeGraphicsTextureLockInstallationPlan {
    address_space: Option<ProcessAddressReservationBatchPlan>,
    graphics: RuntimeGraphicsTextureLockPlan,
}

impl RuntimeGraphicsTextureLockInstallationPlan {
    pub(crate) fn resource_allocation(&self) -> Option<&ProcessAddressAllocationRecord> {
        self.address_space
            .as_ref()
            .and_then(|plan| plan.allocations().first())
    }

    pub(crate) fn texture_id(&self) -> RuntimeGraphicsTextureId {
        self.graphics.texture_id()
    }

    pub(crate) fn lock(&self) -> RuntimeGraphicsTextureLock {
        self.graphics.lock()
    }
}

impl RuntimeAudioBufferLockInstallationPlan {
    pub(crate) fn resource_allocation(&self) -> Option<&ProcessAddressAllocationRecord> {
        self.address_space
            .as_ref()
            .and_then(|plan| plan.allocations().first())
    }

    pub(crate) fn buffer_id(&self) -> RuntimeAudioBufferId {
        self.audio.buffer_id()
    }

    pub(crate) fn lock(&self) -> RuntimeAudioBufferLock {
        self.audio.lock()
    }
}

impl RuntimeGraphicsVertexBufferInstallationPlan {
    pub(crate) fn com_allocation(&self) -> &ProcessAddressAllocationRecord {
        &self.address_space.allocations()[0]
    }

    pub(crate) fn resource_allocation(&self) -> &ProcessAddressAllocationRecord {
        &self.address_space.allocations()[1]
    }
}

pub(crate) struct RuntimeGraphicsSurfaceDcAcquisitionPlan {
    target: RuntimeGdiGraphicsSurfaceTarget,
    gdi: RuntimeGetGraphicsSurfaceDcPlan,
}

impl RuntimeGraphicsSurfaceDcAcquisitionPlan {
    pub(crate) fn result(&self) -> RuntimeGetGraphicsSurfaceDcResult {
        self.gdi.result()
    }
}

pub const RUNTIME_CURRENT_THREAD_PSEUDO_HANDLE: u32 = 0xffff_fffe;
const X86_TEB_EXCEPTION_LIST_OFFSET: usize = 0x00;
const X86_TEB_STACK_BASE_OFFSET: usize = 0x04;
const X86_TEB_STACK_LIMIT_OFFSET: usize = 0x08;
const X86_TEB_SELF_POINTER_OFFSET: usize = 0x18;
const X86_TEB_PROCESS_ID_OFFSET: usize = 0x20;
const X86_TEB_THREAD_ID_OFFSET: usize = 0x24;
const X86_TEB_PEB_POINTER_OFFSET: usize = 0x30;
const X86_TEB_INITIALIZATION_BYTES: usize = X86_TEB_PEB_POINTER_OFFSET + 4;

fn validate_guest_thread_stack_sizes(
    reserve_bytes: u64,
    commit_bytes: u64,
) -> Result<(), RuntimeStateError> {
    if reserve_bytes == 0
        || commit_bytes == 0
        || commit_bytes > reserve_bytes
        || !reserve_bytes.is_multiple_of(GUEST_PAGE_SIZE)
        || !commit_bytes.is_multiple_of(GUEST_PAGE_SIZE)
        || reserve_bytes > u64::from(u32::MAX)
    {
        return Err(RuntimeStateError::invalid_input(
            "runtime-thread-stack",
            "guest-thread-stack-layout-invalid",
            "guest thread reserve and commit must be page-aligned, nonzero x86 sizes with commit no larger than reserve",
        ));
    }
    if reserve_bytes > commit_bytes && reserve_bytes - commit_bytes < GUEST_PAGE_SIZE {
        return Err(RuntimeStateError::invalid_input(
            "runtime-thread-stack",
            "guest-thread-stack-guard-unavailable",
            "a growable guest thread stack requires one page between its commit and reserve limits",
        ));
    }
    Ok(())
}

fn pointer_input_coordinates(
    normalized_x: u16,
    normalized_y: u16,
    geometry: RuntimeWindowGeometry,
    profile: &RuntimePointerButtonMessageProfile,
) -> Result<(i32, i32, i32, i32, u32), RuntimeStateError> {
    if geometry.client_width == 0
        || geometry.client_height == 0
        || profile.normalized_position_denominator != 65_536
    {
        return Err(RuntimeStateError::runtime_defect(
            "runtime-pointer-input",
            "pointer-input-client-geometry-invalid",
            "pointer input requires bounded nonempty client geometry",
        ));
    }
    let denominator = u64::from(profile.normalized_position_denominator);
    let client_x = i32::try_from(
        u64::from(normalized_x)
            .checked_mul(u64::from(geometry.client_width))
            .ok_or_else(|| {
                RuntimeStateError::resource_limit(
                    "runtime-pointer-input",
                    "pointer-input-client-x-overflow",
                    "normalized pointer x position overflowed logical client accounting",
                )
            })?
            / denominator,
    )
    .map_err(|_| {
        RuntimeStateError::resource_limit(
            "runtime-pointer-input",
            "pointer-input-client-x-unrepresentable",
            "logical pointer x position does not fit x86 client coordinates",
        )
    })?;
    let client_y = i32::try_from(
        u64::from(normalized_y)
            .checked_mul(u64::from(geometry.client_height))
            .ok_or_else(|| {
                RuntimeStateError::resource_limit(
                    "runtime-pointer-input",
                    "pointer-input-client-y-overflow",
                    "normalized pointer y position overflowed logical client accounting",
                )
            })?
            / denominator,
    )
    .map_err(|_| {
        RuntimeStateError::resource_limit(
            "runtime-pointer-input",
            "pointer-input-client-y-unrepresentable",
            "logical pointer y position does not fit x86 client coordinates",
        )
    })?;
    let client_x_i16 = i16::try_from(client_x).map_err(|_| {
        RuntimeStateError::invalid_input(
            "runtime-pointer-input",
            "pointer-input-client-x-out-of-range",
            "logical pointer x position is outside the signed x86 LPARAM range",
        )
    })?;
    let client_y_i16 = i16::try_from(client_y).map_err(|_| {
        RuntimeStateError::invalid_input(
            "runtime-pointer-input",
            "pointer-input-client-y-out-of-range",
            "logical pointer y position is outside the signed x86 LPARAM range",
        )
    })?;
    let screen_x = geometry.client_x.checked_add(client_x).ok_or_else(|| {
        RuntimeStateError::resource_limit(
            "runtime-pointer-input",
            "pointer-input-screen-x-overflow",
            "logical pointer screen x position overflowed i32 accounting",
        )
    })?;
    let screen_y = geometry.client_y.checked_add(client_y).ok_or_else(|| {
        RuntimeStateError::resource_limit(
            "runtime-pointer-input",
            "pointer-input-screen-y-overflow",
            "logical pointer screen y position overflowed i32 accounting",
        )
    })?;
    let l_param = u32::from(client_x_i16 as u16) | (u32::from(client_y_i16 as u16) << 16);
    Ok((client_x, client_y, screen_x, screen_y, l_param))
}

fn callback_scratch_for_depth(
    depth: usize,
    byte_length: u32,
) -> Result<Option<RuntimeCallbackScratch>, RuntimeStateError> {
    if byte_length == 0 {
        return Ok(None);
    }
    if byte_length > CALLBACK_SCRATCH_SLOT_SIZE {
        return Err(RuntimeStateError::resource_limit(
            "runtime-callback",
            "callback-scratch-slot-capacity-exceeded",
            format!(
                "callback scratch requires {byte_length} bytes; slot capacity is {CALLBACK_SCRATCH_SLOT_SIZE}"
            ),
        ));
    }
    let slot = depth.checked_sub(1).ok_or_else(|| {
        RuntimeStateError::runtime_defect(
            "runtime-callback",
            "callback-scratch-depth-zero",
            "live callback scratch cannot be assigned to depth zero",
        )
    })?;
    let slot_count = usize::try_from(CALLBACK_SCRATCH_ARENA_SIZE)
        .ok()
        .and_then(|bytes| bytes.checked_div(CALLBACK_SCRATCH_SLOT_SIZE as usize))
        .ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-callback",
                "callback-scratch-arena-layout-invalid",
                "callback scratch arena does not contain a complete slot",
            )
        })?;
    if slot >= slot_count {
        return Err(RuntimeStateError::resource_limit(
            "runtime-callback",
            "callback-scratch-depth-limit-exceeded",
            format!("callback scratch depth {depth} exceeds {slot_count} arena slots"),
        ));
    }
    let offset = u32::try_from(slot)
        .ok()
        .and_then(|slot| slot.checked_mul(CALLBACK_SCRATCH_SLOT_SIZE))
        .ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-callback",
                "callback-scratch-slot-offset-overflow",
                "validated callback scratch slot offset overflowed x86 accounting",
            )
        })?;
    let address = CALLBACK_SCRATCH_ARENA_BASE
        .0
        .checked_add(offset)
        .map(GuestAddress)
        .ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-callback",
                "callback-scratch-slot-address-overflow",
                "validated callback scratch slot address overflowed x86 accounting",
            )
        })?;
    Ok(Some(RuntimeCallbackScratch {
        slot: u16::try_from(slot).map_err(|_| {
            RuntimeStateError::runtime_defect(
                "runtime-callback",
                "callback-scratch-slot-unrepresentable",
                "validated callback scratch slot does not fit its identity type",
            )
        })?,
        address,
        byte_length,
        capacity: CALLBACK_SCRATCH_SLOT_SIZE,
    }))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeCurrentThreadPseudoHandleResolution {
    pub value: u32,
    pub selected_thread_id: RuntimeThreadId,
    pub object_id: RuntimeObjectId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeKernel {
    process: RuntimeProcessState,
    address_space_allocator: ProcessAddressSpaceAllocator,
    threads: RuntimeThreads,
    scheduler: RuntimeScheduler,
    messages: RuntimeMessages,
    displays: RuntimeDisplays,
    input: RuntimeInput,
    audio: RuntimeAudio,
    user32: RuntimeUser32,
    imm32: RuntimeImm32,
    gdi: RuntimeGdi,
    gdiplus: RuntimeGdiplus,
    graphics: RuntimeGraphics,
    timers: RuntimeTimers,
    objects: RuntimeObjectTable,
    com: RuntimeCom,
    heaps: RuntimeHeaps,
    files: RuntimeFiles,
    module_records: RuntimeModuleRecords,
    synchronization: RuntimeSynchronization,
    modules: RuntimeModules,
    registries: RuntimeRegistries,
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
struct RuntimeProcessStateSnapshot<'a> {
    #[serde(flatten)]
    process: &'a RuntimeProcessState,
    #[serde(flatten)]
    heaps: &'a RuntimeHeaps,
    #[serde(flatten)]
    module_records: &'a RuntimeModuleRecords,
    #[serde(flatten)]
    synchronization: &'a RuntimeSynchronization,
    allocated_tls_indices: &'a BTreeSet<u32>,
    fls_slots: &'a BTreeMap<u32, RuntimeFlsSlot>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    waitable_timers: &'a BTreeMap<RuntimeWaitableTimerId, RuntimeWaitableTimer>,
    timer_clock: RuntimeTimerClockState,
    #[serde(skip_serializing_if = "Option::is_none")]
    clock_profile: Option<&'a RuntimeClockProfile>,
    #[serde(skip_serializing_if = "Option::is_none")]
    scheduled_execution_clock: Option<RuntimeScheduledExecutionClockState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    timer_resolution: Option<RuntimeTimerResolutionState>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    window_timers: &'a BTreeMap<RuntimeWindowTimerId, RuntimeWindowTimer>,
}

#[derive(Clone, Copy)]
pub struct RuntimeProcessView<'a> {
    process: &'a RuntimeProcessState,
    heaps: &'a RuntimeHeaps,
    module_records: &'a RuntimeModuleRecords,
    synchronization: &'a RuntimeSynchronization,
    allocated_tls_indices: &'a BTreeSet<u32>,
    fls_slots: &'a BTreeMap<u32, RuntimeFlsSlot>,
    timers: &'a RuntimeTimers,
}

impl Deref for RuntimeProcessView<'_> {
    type Target = RuntimeProcessState;

    fn deref(&self) -> &Self::Target {
        self.process
    }
}

impl Serialize for RuntimeProcessView<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        RuntimeProcessStateSnapshot {
            process: self.process,
            heaps: self.heaps,
            module_records: self.module_records,
            synchronization: self.synchronization,
            allocated_tls_indices: self.allocated_tls_indices,
            fls_slots: self.fls_slots,
            waitable_timers: self.timers.waitable_timers(),
            timer_clock: self.process.timer_clock_state(),
            clock_profile: self
                .process
                .uses_scheduled_shared_clock()
                .then(|| self.process.clock_profile()),
            scheduled_execution_clock: self.process.scheduled_execution_clock_state(),
            timer_resolution: self.timers.state(),
            window_timers: self.timers.window_timers(),
        }
        .serialize(serializer)
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeStateSnapshot<'a> {
    process: RuntimeProcessStateSnapshot<'a>,
    address_space_allocator: &'a ProcessAddressSpaceAllocator,
    threads: &'a RuntimeThreads,
    scheduler: &'a RuntimeScheduler,
    messages: &'a RuntimeMessages,
    displays: &'a RuntimeDisplays,
    input: &'a RuntimeInput,
    #[serde(skip_serializing_if = "Option::is_none")]
    audio: Option<&'a RuntimeAudio>,
    #[serde(skip_serializing_if = "Option::is_none")]
    user32: Option<&'a RuntimeUser32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    imm32: Option<&'a RuntimeImm32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    gdi: Option<&'a RuntimeGdi>,
    #[serde(skip_serializing_if = "Option::is_none")]
    gdiplus: Option<&'a RuntimeGdiplus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    graphics: Option<&'a RuntimeGraphics>,
    objects: &'a RuntimeObjectTable,
    #[serde(skip_serializing_if = "Option::is_none")]
    com: Option<&'a RuntimeCom>,
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
    next_thread_id: u64,
    next_host_request_id: u64,
    next_wait_registration_id: u64,
}

impl Serialize for RuntimeKernel {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        RuntimeStateSnapshot {
            process: RuntimeProcessStateSnapshot {
                process: &self.process,
                heaps: &self.heaps,
                module_records: &self.module_records,
                synchronization: &self.synchronization,
                allocated_tls_indices: self.threads.allocated_tls_indices(),
                fls_slots: self.threads.fls_slots(),
                waitable_timers: self.timers.waitable_timers(),
                timer_clock: self.process.timer_clock_state(),
                clock_profile: self
                    .process
                    .uses_scheduled_shared_clock()
                    .then(|| self.process.clock_profile()),
                scheduled_execution_clock: self.process.scheduled_execution_clock_state(),
                timer_resolution: self.timers.state(),
                window_timers: self.timers.window_timers(),
            },
            address_space_allocator: &self.address_space_allocator,
            threads: &self.threads,
            scheduler: &self.scheduler,
            messages: &self.messages,
            displays: &self.displays,
            input: &self.input,
            audio: (!self.audio.is_empty()).then_some(&self.audio),
            user32: (!self.user32.is_empty()).then_some(&self.user32),
            imm32: (!self.imm32.is_empty()).then_some(&self.imm32),
            gdi: (!self.gdi.is_empty()).then_some(&self.gdi),
            gdiplus: (!self.gdiplus.is_empty()).then_some(&self.gdiplus),
            graphics: (!self.graphics.is_empty()).then_some(&self.graphics),
            objects: &self.objects,
            com: (!self.com.is_empty()).then_some(&self.com),
            modules: &self.modules,
            registries: RuntimeRegistriesSnapshot {
                dynamic_endpoints: self.registries.dynamic_endpoints(),
                encoded_pointers: self.registries.encoded_pointers(),
                host_requests: self.scheduler.host_requests(),
            },
            encoded_pointer_namespace: self.registries.encoded_pointer_namespace(),
            encoded_pointer_token_prefix: self.registries.encoded_pointer_token_prefix(),
            encoded_pointer_sources: self.registries.encoded_pointer_sources(),
            next_encoded_pointer_id: self.registries.next_encoded_pointer_id(),
            next_private_heap_slot: self.heaps.next_private_heap_slot(),
            next_tls_index: self.threads.next_tls_index(),
            next_fls_index: self.threads.next_fls_index(),
            next_callback_frame_id: self.threads.next_callback_frame_id(),
            next_thread_id: self.threads.next_thread_id(),
            next_host_request_id: self.scheduler.next_host_request_id(),
            next_wait_registration_id: self.scheduler.next_wait_registration_id(),
        }
        .serialize(serializer)
    }
}

impl RuntimeKernel {
    pub fn new(bootstrap: RuntimeBootstrap) -> Result<Self, RuntimeStateError> {
        if bootstrap.process_id.0 == 0 || bootstrap.initial_thread_id.0 == 0 {
            return Err(RuntimeStateError::invalid_input(
                "runtime-state",
                "zero-process-or-thread-id",
                "runtime process and initial thread IDs must be nonzero",
            ));
        }
        if bootstrap.clock.performance_frequency == 0 {
            return Err(RuntimeStateError::invalid_input(
                "runtime-clock",
                "performance-frequency-zero",
                "runtime performance-counter frequency must be nonzero",
            ));
        }
        if bootstrap.clock.performance_frequency > i64::MAX as u64 {
            return Err(RuntimeStateError::invalid_input(
                "runtime-clock",
                "performance-frequency-range-invalid",
                "runtime performance-counter frequency must fit a nonnegative LARGE_INTEGER",
            ));
        }
        if bootstrap.clock.performance_counter > i64::MAX as u64 {
            return Err(RuntimeStateError::invalid_input(
                "runtime-clock",
                "performance-counter-range-invalid",
                "runtime performance counter must fit a nonnegative LARGE_INTEGER",
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
        let registries = RuntimeRegistries::new(
            bootstrap.encoded_pointer_namespace,
            bootstrap.encoded_pointer_token_prefix,
        )?;

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
            process: RuntimeProcessState::new(
                bootstrap.process_id,
                bootstrap.address_space,
                bootstrap.clock,
                bootstrap.startup_info,
                bootstrap.process_image_path_a,
                process_directories,
            ),
            address_space_allocator,
            threads,
            scheduler: RuntimeScheduler::new(bootstrap.initial_thread_id),
            messages: RuntimeMessages::new(bootstrap.initial_thread_id),
            displays: RuntimeDisplays::default(),
            input: RuntimeInput::default(),
            audio: RuntimeAudio::default(),
            user32: RuntimeUser32::default(),
            imm32: RuntimeImm32::default(),
            gdi: RuntimeGdi::default(),
            gdiplus: RuntimeGdiplus::default(),
            graphics: RuntimeGraphics::default(),
            timers: RuntimeTimers::default(),
            objects,
            com: RuntimeCom::default(),
            heaps: RuntimeHeaps::default(),
            files: RuntimeFiles::default(),
            module_records: RuntimeModuleRecords::default(),
            synchronization: RuntimeSynchronization::default(),
            modules: RuntimeModules::default(),
            registries,
        })
    }

    pub fn process_id(&self) -> RuntimeProcessId {
        self.process.id()
    }

    pub fn configure_display_profile(
        &mut self,
        profile: RuntimeDisplayProfile,
    ) -> Result<(), RuntimeStateError> {
        let plan = self.displays.plan_configuration(profile)?;
        self.displays.commit_configuration(plan)
    }

    pub fn display_profile_id(&self) -> Option<&str> {
        self.displays.profile_id()
    }

    pub fn primary_monitor(&self) -> Option<&RuntimeLogicalMonitor> {
        self.displays.primary_monitor()
    }

    pub fn monitor(&self, handle: u32) -> Result<&RuntimeLogicalMonitor, RuntimeStateError> {
        self.displays.monitor(handle)
    }

    pub fn configure_input_profile(
        &mut self,
        profile: RuntimeInputProfile,
    ) -> Result<(), RuntimeStateError> {
        let plan = self.input.plan_configuration(profile)?;
        self.input.commit_configuration(plan)
    }

    pub fn mouse_button_mapping(&self) -> Option<&RuntimeMouseButtonMapping> {
        self.input.mouse_button_mapping()
    }

    pub fn swap_mouse_buttons(
        &mut self,
        swapped: bool,
    ) -> Result<RuntimeSwapMouseButtonResult, RuntimeStateError> {
        let plan = self.input.plan_mouse_button_swap(swapped)?;
        self.input.commit_mouse_button_swap(plan)
    }

    pub fn keyboard_state(&self) -> Option<&RuntimeKeyboardState> {
        self.input.keyboard_state()
    }

    pub fn cursor_position(&self) -> Option<&RuntimeCursorPosition> {
        self.input.cursor_position()
    }

    pub fn ingest_keyboard_event(
        &mut self,
        event: RuntimeKeyboardSourceEvent,
    ) -> Result<RuntimeKeyboardEventReceipt, RuntimeStateError> {
        let plan = self.input.plan_keyboard_event(event)?;
        self.input.commit_keyboard_event(plan)
    }

    pub fn ingest_pointer_button_event(
        &mut self,
        event: RuntimePointerButtonSourceEvent,
        profile: &RuntimePointerButtonMessageProfile,
    ) -> Result<RuntimePointerButtonIngressReceipt, RuntimeStateError> {
        let (window, target_thread_id, target_scheduler_state, geometry) =
            self.pointer_input_target(profile)?;
        let window_handle = window.handle;
        let (client_x, client_y, screen_x, screen_y, l_param) =
            pointer_input_coordinates(event.normalized_x, event.normalized_y, geometry, profile)?;
        let input_plan = self
            .input
            .plan_pointer_button_event(event, screen_x, screen_y)?;
        let (message, w_param) = input_plan.message_and_key_state(profile);
        let message_plan = self.messages.plan_post(
            RuntimeMessageSource::PointerInput {
                event_sequence: event.sequence,
            },
            target_thread_id,
            window_handle,
            message,
            w_param,
            l_param,
            self.process.clock().tick_count,
            screen_x,
            screen_y,
            target_scheduler_state == SchedulerThreadState::WaitingMessage,
        )?;
        let posted = message_plan.message;
        let scheduler_plan = message_plan
            .wake_target
            .then(|| {
                self.scheduler
                    .plan_transition(target_thread_id, SchedulerThreadState::Runnable)
            })
            .transpose()?;

        self.input.preflight_pointer_button_event(&input_plan)?;
        self.messages.preflight_post(&message_plan)?;
        self.preflight_pointer_input_target(&window, target_thread_id, geometry)?;
        let input = self
            .input
            .commit_preflighted_pointer_button_event(input_plan);
        self.messages.commit_post(message_plan);
        if let Some(scheduler_plan) = scheduler_plan {
            self.scheduler.commit_lane(scheduler_plan);
            self.scheduler
                .commit_event(RuntimeSchedulerEvent::MessageReady(RuntimeMessageWake {
                    thread_id: target_thread_id,
                    message_id: posted.id,
                    queue_depth: message_plan.queue_depth,
                }));
        }
        Ok(RuntimePointerButtonIngressReceipt {
            profile_id: profile.id.clone(),
            window_id: window.id,
            window_handle,
            target_thread_id,
            client_x,
            client_y,
            input,
            message: posted,
        })
    }

    pub fn ingest_pointer_position_event(
        &mut self,
        event: RuntimePointerPositionSourceEvent,
        profile: &RuntimePointerButtonMessageProfile,
    ) -> Result<RuntimePointerPositionIngressReceipt, RuntimeStateError> {
        let (window, target_thread_id, target_scheduler_state, geometry) =
            self.pointer_input_target(profile)?;
        let (client_x, client_y, screen_x, screen_y, l_param) =
            pointer_input_coordinates(event.normalized_x, event.normalized_y, geometry, profile)?;
        let input_plan = self
            .input
            .plan_pointer_position_event(event, screen_x, screen_y)?;
        let message_plan = self.messages.plan_post(
            RuntimeMessageSource::PointerInput {
                event_sequence: event.sequence,
            },
            target_thread_id,
            window.handle,
            WM_MOUSEMOVE,
            input_plan.message_key_state(profile),
            l_param,
            self.process.clock().tick_count,
            screen_x,
            screen_y,
            target_scheduler_state == SchedulerThreadState::WaitingMessage,
        )?;
        let posted = message_plan.message;
        let scheduler_plan = message_plan
            .wake_target
            .then(|| {
                self.scheduler
                    .plan_transition(target_thread_id, SchedulerThreadState::Runnable)
            })
            .transpose()?;

        self.input.preflight_pointer_position_event(&input_plan)?;
        self.messages.preflight_post(&message_plan)?;
        self.preflight_pointer_input_target(&window, target_thread_id, geometry)?;

        let input = self
            .input
            .commit_preflighted_pointer_position_event(input_plan);
        self.messages.commit_post(message_plan);
        if let Some(scheduler_plan) = scheduler_plan {
            self.scheduler.commit_lane(scheduler_plan);
            self.scheduler
                .commit_event(RuntimeSchedulerEvent::MessageReady(RuntimeMessageWake {
                    thread_id: target_thread_id,
                    message_id: posted.id,
                    queue_depth: message_plan.queue_depth,
                }));
        }
        Ok(RuntimePointerPositionIngressReceipt {
            profile_id: profile.id.clone(),
            window_id: window.id,
            window_handle: window.handle,
            target_thread_id,
            client_x,
            client_y,
            input,
            message: posted,
        })
    }

    fn pointer_input_target(
        &self,
        profile: &RuntimePointerButtonMessageProfile,
    ) -> Result<
        (
            RuntimeWindow,
            RuntimeThreadId,
            SchedulerThreadState,
            RuntimeWindowGeometry,
        ),
        RuntimeStateError,
    > {
        if profile.id.is_empty()
            || profile.id.len() > 128
            || !profile.id.is_ascii()
            || profile.id.as_bytes().contains(&0)
            || profile.normalized_position_denominator != 65_536
            || profile.left_button_down_message == 0
            || profile.left_button_up_message == 0
            || profile.right_button_down_message == 0
            || profile.right_button_up_message == 0
            || profile.left_button_key_mask == 0
            || profile.right_button_key_mask == 0
            || profile.shift_key_mask == 0
            || profile.control_key_mask == 0
            || !profile.require_visible_active_focused_foreground_window
        {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-pointer-input",
                "pointer-message-profile-invalid",
                "pointer message ingress requires the bounded normalized foreground-window profile",
            ));
        }
        if !self.process.is_running() {
            return Err(RuntimeStateError::conflict(
                "runtime-pointer-input",
                "pointer-input-process-state-invalid",
                "pointer input requires a running process",
            ));
        }
        let window_handle = self.user32.foreground_window_handle()?.ok_or_else(|| {
            RuntimeStateError::conflict(
                "runtime-pointer-input",
                "pointer-input-foreground-window-missing",
                "pointer input requires a live logical foreground window",
            )
        })?;
        let window = self.user32.window(window_handle)?.clone();
        let target_thread_id = window.creation.owner_thread_id;
        if window.lifecycle != RuntimeWindowLifecycle::Created
            || !window.visible
            || self.user32.active_window_handle(target_thread_id)? != Some(window_handle)
            || self.user32.focused_window_handle(target_thread_id)? != Some(window_handle)
        {
            return Err(RuntimeStateError::conflict(
                "runtime-pointer-input",
                "pointer-input-window-state-invalid",
                "pointer input requires one visible active focused foreground logical window",
            ));
        }
        self.user32
            .resolve_window_procedure(window_handle, target_thread_id)?;
        let target_thread = self.thread_state(target_thread_id)?;
        let target_scheduler_state = self.scheduler_state_for_thread(target_thread_id)?;
        if target_thread.lifecycle != RuntimeThreadLifecycleState::Running
            || !matches!(
                target_scheduler_state,
                SchedulerThreadState::Runnable
                    | SchedulerThreadState::WaitingHost { .. }
                    | SchedulerThreadState::WaitingObject { .. }
                    | SchedulerThreadState::WaitingCriticalSection { .. }
                    | SchedulerThreadState::WaitingMessage
                    | SchedulerThreadState::InCallback { .. }
            )
        {
            return Err(RuntimeStateError::conflict(
                "runtime-pointer-input",
                "pointer-input-target-state-invalid",
                "pointer input target requires matching live thread and scheduler states",
            ));
        }
        let geometry = window.creation.geometry;
        if geometry.client_width == 0 || geometry.client_height == 0 {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-pointer-input",
                "pointer-input-client-geometry-invalid",
                "pointer input target has an empty logical client area",
            ));
        }
        Ok((window, target_thread_id, target_scheduler_state, geometry))
    }

    fn preflight_pointer_input_target(
        &self,
        window: &RuntimeWindow,
        target_thread_id: RuntimeThreadId,
        geometry: RuntimeWindowGeometry,
    ) -> Result<(), RuntimeStateError> {
        let current_window = self.user32.window(window.handle)?;
        if current_window.id != window.id
            || current_window.lifecycle != RuntimeWindowLifecycle::Created
            || !current_window.visible
            || current_window.creation.owner_thread_id != target_thread_id
            || current_window.creation.geometry != geometry
            || self.user32.active_window_handle(target_thread_id)? != Some(window.handle)
            || self.user32.focused_window_handle(target_thread_id)? != Some(window.handle)
            || self.user32.foreground_window_handle()? != Some(window.handle)
        {
            return Err(RuntimeStateError::conflict(
                "runtime-pointer-input",
                "pointer-input-window-plan-stale",
                "logical pointer target changed after input and message planning",
            ));
        }
        Ok(())
    }

    pub fn query_async_key_state(
        &mut self,
        virtual_key: u32,
    ) -> Result<Option<RuntimeAsyncKeyStateResult>, RuntimeStateError> {
        let Some(plan) = self.input.plan_async_key_state_query(virtual_key)? else {
            return Ok(None);
        };
        self.input.commit_async_key_state_query(plan).map(Some)
    }

    pub fn begin_timer_period(
        &mut self,
        period_milliseconds: u32,
    ) -> Result<RuntimeTimerResolutionMutationResult, RuntimeStateError> {
        self.require_timer_operation_state()?;
        let plan = self.timers.plan_resolution_begin(period_milliseconds)?;
        self.timers.commit_resolution_mutation(plan)
    }

    pub fn end_timer_period(
        &mut self,
        period_milliseconds: u32,
    ) -> Result<Option<RuntimeTimerResolutionMutationResult>, RuntimeStateError> {
        self.require_timer_operation_state()?;
        let Some(plan) = self.timers.plan_resolution_end(period_milliseconds)? else {
            return Ok(None);
        };
        self.timers.commit_resolution_mutation(plan).map(Some)
    }

    pub fn timer_resolution_state(&self) -> Option<RuntimeTimerResolutionState> {
        self.timers.state()
    }

    pub fn timer_clock_state(&self) -> RuntimeTimerClockState {
        self.process.timer_clock_state()
    }

    pub fn scheduled_execution_clock_state(&self) -> Option<RuntimeScheduledExecutionClockState> {
        self.process.scheduled_execution_clock_state()
    }

    fn require_timer_operation_state(&self) -> Result<(), RuntimeStateError> {
        if !self.process.is_running()
            || self.current_thread()?.lifecycle != RuntimeThreadLifecycleState::Running
            || !matches!(
                self.scheduler_state()?,
                SchedulerThreadState::Runnable | SchedulerThreadState::InCallback { .. }
            )
        {
            return Err(RuntimeStateError::conflict(
                "runtime-timers",
                "timer-resolution-operation-state-invalid",
                "timer resolution requests require a running process and active current thread",
            ));
        }
        Ok(())
    }

    pub fn process(&self) -> RuntimeProcessView<'_> {
        RuntimeProcessView {
            process: &self.process,
            heaps: &self.heaps,
            module_records: &self.module_records,
            synchronization: &self.synchronization,
            allocated_tls_indices: self.threads.allocated_tls_indices(),
            fls_slots: self.threads.fls_slots(),
            timers: &self.timers,
        }
    }

    pub fn heaps(&self) -> &BTreeMap<u32, RuntimeHeap> {
        self.heaps.heaps()
    }

    pub fn modules(&self) -> &BTreeMap<u32, RuntimeModule> {
        self.module_records.modules()
    }

    pub fn process_image_path_a(&self) -> &RuntimeProcessImagePathA {
        self.process.image_path_a()
    }

    pub fn current_directory_a(&self) -> &RuntimeCurrentDirectoryA {
        self.process.current_directory_a()
    }

    pub fn current_directory_generation(&self) -> u64 {
        self.process.current_directory_generation()
    }

    pub fn is_process_image_directory_a(
        &self,
        requested: &[u8],
    ) -> Result<bool, RuntimeStateError> {
        self.process.is_process_image_directory_a(requested)
    }

    pub fn is_process_image_drive_root_a(&self, requested: Option<&[u8]>) -> bool {
        self.process.is_process_image_drive_root_a(requested)
    }

    pub fn plan_set_current_directory_a(
        &self,
        requested: &[u8],
    ) -> Result<RuntimeSetCurrentDirectoryAPlan, RuntimeStateError> {
        self.process.plan_set_current_directory_a(requested)
    }

    pub fn commit_set_current_directory_a(
        &mut self,
        plan: RuntimeSetCurrentDirectoryAPlan,
    ) -> Result<&RuntimeCurrentDirectoryA, RuntimeStateError> {
        self.process.commit_set_current_directory_a(plan)
    }

    pub fn register_source_snapshot(
        &mut self,
        manifest: RuntimeSourceSnapshotManifest,
    ) -> Result<&RuntimeSourceSnapshot, RuntimeStateError> {
        let plan = self.files.plan_snapshot_registration(manifest)?;
        self.files.commit_snapshot_registration(plan)?;
        self.files.snapshot().ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-vfs",
                "source-snapshot-registration-missing",
                "source snapshot disappeared after registration commit",
            )
        })
    }

    pub fn source_snapshot(&self) -> Option<&RuntimeSourceSnapshot> {
        self.files.snapshot()
    }

    pub fn register_writable_profile(
        &mut self,
        manifest: RuntimeWritableProfileManifest,
    ) -> Result<&RuntimeWritableProfile, RuntimeStateError> {
        let plan = self.files.plan_writable_profile_registration(manifest)?;
        self.files.commit_writable_profile_registration(plan)?;
        self.files.writable_profile().ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-vfs",
                "writable-profile-registration-missing",
                "writable profile disappeared after registration commit",
            )
        })
    }

    pub fn writable_profile(&self) -> Option<&RuntimeWritableProfile> {
        self.files.writable_profile()
    }

    pub fn resolve_file_system_path_a(
        &self,
        requested_path: &[u8],
    ) -> Result<super::RuntimePathResolution, RuntimeStateError> {
        self.require_source_file_operation_state()?;
        self.files.resolve_path_kind(
            requested_path,
            &self.process.current_directory_a().bytes_with_nul,
        )
    }

    pub fn begin_writable_directory_create(
        &mut self,
        requested_path: &[u8],
    ) -> Result<RuntimeWritableDirectoryCreateRequest, RuntimeStateError> {
        self.require_source_file_operation_state()?;
        let current_directory = self.process.current_directory_a().bytes_with_nul.clone();
        let scheduler = self
            .scheduler
            .plan_host_request(RUNTIME_WRITABLE_DIRECTORY_CREATE_SERVICE.to_owned())?;
        let directory: RuntimeWritableDirectoryCreateBeginPlan =
            self.files.plan_writable_directory_create(
                requested_path,
                &current_directory,
                scheduler.request().request_id,
            )?;
        self.files.preflight_writable_directory_create(&directory)?;
        let runtime_request = self.scheduler.commit_host_request(scheduler);
        let request = self.files.commit_writable_directory_create_begin(directory);
        debug_assert_eq!(runtime_request.request_id, request.request_id);
        debug_assert_eq!(
            runtime_request.service,
            RUNTIME_WRITABLE_DIRECTORY_CREATE_SERVICE
        );
        Ok(request)
    }

    pub fn writable_directory_create_request(
        &self,
        request_id: u64,
    ) -> Result<&RuntimeWritableDirectoryCreateRequest, RuntimeStateError> {
        self.files.writable_directory_create_request(request_id)
    }

    pub(crate) fn plan_writable_directory_create_completion(
        &self,
        response: RuntimeWritableDirectoryCreateHostResponse,
    ) -> Result<RuntimeWritableDirectoryCreateAggregateCompletionPlan, RuntimeStateError> {
        let scheduler = self.scheduler.plan_host_completion(response.request_id)?;
        if scheduler.request().service != RUNTIME_WRITABLE_DIRECTORY_CREATE_SERVICE {
            return Err(RuntimeStateError::invalid_input(
                "runtime-vfs",
                "writable-directory-create-host-service-mismatch",
                "directory create completion belongs to another host service",
            ));
        }
        let directory_create = self
            .files
            .plan_writable_directory_create_completion(response)?;
        self.files
            .preflight_writable_directory_create_completion(&directory_create)?;
        Ok(RuntimeWritableDirectoryCreateAggregateCompletionPlan {
            scheduler,
            directory_create,
        })
    }

    pub(crate) fn commit_writable_directory_create_completion(
        &mut self,
        plan: RuntimeWritableDirectoryCreateAggregateCompletionPlan,
    ) -> RuntimeWritableDirectoryCreateCompletion {
        debug_assert!(
            self.files
                .preflight_writable_directory_create_completion(&plan.directory_create)
                .is_ok()
        );
        let scheduler_completion = self.scheduler.commit_host_completion(plan.scheduler);
        debug_assert_eq!(
            scheduler_completion.request_id,
            plan.directory_create.completion().request.request_id
        );
        self.files
            .commit_writable_directory_create_completion(plan.directory_create)
    }

    pub fn hydrate_writable_profile_object(
        &mut self,
        object_id: u32,
        generation: u64,
        bytes: &[u8],
    ) -> Result<&super::RuntimeWritableFile, RuntimeStateError> {
        let plan = self
            .files
            .plan_writable_profile_hydration(object_id, generation, bytes)?;
        self.files.commit_writable_profile_hydration(plan)
    }

    pub fn open_source_file_a(
        &mut self,
        requested_path: &[u8],
        desired_access: u32,
        share_mode: u32,
        flags_and_attributes: u32,
    ) -> Result<Option<super::RuntimeOpenSourceFileResult>, RuntimeStateError> {
        self.open_file_a(
            requested_path,
            desired_access,
            share_mode,
            3,
            flags_and_attributes,
        )
    }

    pub fn open_file_a(
        &mut self,
        requested_path: &[u8],
        desired_access: u32,
        share_mode: u32,
        creation_disposition: u32,
        flags_and_attributes: u32,
    ) -> Result<Option<super::RuntimeOpenSourceFileResult>, RuntimeStateError> {
        self.require_source_file_operation_state()?;
        let Some(plan) = self.files.plan_open_with_disposition(
            requested_path,
            &self.process.current_directory_a().bytes_with_nul,
            desired_access,
            share_mode,
            creation_disposition,
            flags_and_attributes,
        )?
        else {
            return Ok(None);
        };
        let key = RuntimeObjectKey::File {
            file_id: plan.file_id().0,
        };
        let object_plan = self.objects.plan_close_handle_object_open(key, false)?;
        self.files.preflight_open(&plan)?;
        let handle = object_plan.opened_handle();
        let existed = plan.existed();
        self.objects
            .commit_close_handle_object_mutation(object_plan)?;
        let file = self.files.commit_open(plan);
        Ok(Some(super::RuntimeOpenSourceFileResult {
            file,
            handle,
            existed,
        }))
    }

    pub fn find_first_file_a<F>(
        &mut self,
        memory: &mut GuestMemory,
        requested_pattern: &[u8],
        output: GuestAddress,
        maximum_matches: usize,
        expected_output_bytes: usize,
        encode_entry: F,
    ) -> Result<Option<(RuntimeFindFirstFileResult, Vec<u8>, GuestWriteResult)>, RuntimeStateError>
    where
        F: FnOnce(&RuntimeFileSearchEntry) -> Result<Vec<u8>, RuntimeStateError>,
    {
        self.require_source_file_operation_state()?;
        let Some(search_plan): Option<RuntimeFileSearchOpenPlan> = self.files.plan_find_first(
            requested_pattern,
            &self.process.current_directory_a().bytes_with_nul,
            maximum_matches,
        )?
        else {
            return Ok(None);
        };
        let entry = search_plan.first_entry().clone();
        let bytes = encode_entry(&entry)?;
        if bytes.len() != expected_output_bytes {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-vfs",
                "file-search-output-size-invalid",
                "file search entry encoder returned an unexpected ABI byte length",
            ));
        }
        memory
            .validate_access(output, bytes.len(), MemoryAccess::Write)
            .map_err(RuntimeStateError::from_memory)?;
        let key = RuntimeObjectKey::FileSearch {
            search_id: search_plan.search_id().0,
        };
        let object_plan = self.objects.plan_object_and_handle_registration(key)?;
        self.files.preflight_find_first(&search_plan)?;
        self.objects
            .preflight_object_registration_commit(&object_plan)?;
        let handle = object_plan.handle();
        let mut transaction = memory.begin_transaction();
        let write = transaction
            .write(output, &bytes)
            .map_err(RuntimeStateError::from_memory)?;
        let (_, committed_handle) = self.objects.commit_object_registration(object_plan);
        debug_assert_eq!(committed_handle, handle);
        let search = self.files.commit_find_first(search_plan);
        transaction.commit_transaction();
        Ok(Some((
            RuntimeFindFirstFileResult {
                search,
                handle,
                entry,
            },
            bytes,
            write,
        )))
    }

    pub fn find_next_file_a<F>(
        &mut self,
        memory: &mut GuestMemory,
        handle: RuntimeHandle,
        output: GuestAddress,
        expected_output_bytes: usize,
        encode_entry: F,
    ) -> Result<Option<(RuntimeFileSearchEntry, Vec<u8>, GuestWriteResult)>, RuntimeStateError>
    where
        F: FnOnce(&RuntimeFileSearchEntry) -> Result<Vec<u8>, RuntimeStateError>,
    {
        self.require_source_file_operation_state()?;
        let search_id = self.file_search_for_handle(handle)?.id;
        let Some(plan): Option<RuntimeFileSearchAdvancePlan> =
            self.files.plan_find_next(search_id)?
        else {
            return Ok(None);
        };
        let entry = plan.entry().clone();
        let bytes = encode_entry(&entry)?;
        if bytes.len() != expected_output_bytes {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-vfs",
                "file-search-output-size-invalid",
                "file search entry encoder returned an unexpected ABI byte length",
            ));
        }
        memory
            .validate_access(output, bytes.len(), MemoryAccess::Write)
            .map_err(RuntimeStateError::from_memory)?;
        self.files.preflight_find_next(&plan)?;
        let mut transaction = memory.begin_transaction();
        let write = transaction
            .write(output, &bytes)
            .map_err(RuntimeStateError::from_memory)?;
        let committed = self.files.commit_find_next(plan);
        debug_assert_eq!(committed, entry);
        transaction.commit_transaction();
        Ok(Some((entry, bytes, write)))
    }

    pub fn file_search_for_handle(
        &self,
        handle: RuntimeHandle,
    ) -> Result<&RuntimeFileSearch, RuntimeStateError> {
        let record = self
            .objects
            .object_for_handle(handle, RuntimeObjectKind::FileSearch)?;
        let RuntimeObjectKey::FileSearch { search_id } = record.key else {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-vfs",
                "file-search-object-key-invalid",
                "file search handle resolved to a different object identity",
            ));
        };
        self.files.file_search(RuntimeFileSearchId(search_id))
    }

    pub fn find_close(
        &mut self,
        handle: RuntimeHandle,
    ) -> Result<RuntimeHandleEntry, RuntimeStateError> {
        self.require_source_file_operation_state()?;
        let record = self
            .objects
            .object_for_handle(handle, RuntimeObjectKind::FileSearch)?
            .clone();
        let RuntimeObjectKey::FileSearch { search_id } = record.key.clone() else {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-vfs",
                "file-search-object-key-invalid",
                "file search handle resolved to a different object identity",
            ));
        };
        let retirement = self
            .files
            .plan_file_search_retirement(RuntimeFileSearchId(search_id))?;
        let object_plan = self
            .objects
            .plan_find_close_object_close(handle, &record.key)?;
        self.files.preflight_file_search_retirement(&retirement)?;
        let removed = object_plan.closed_entry();
        self.objects
            .commit_close_handle_object_mutation(object_plan)?;
        self.files.commit_file_search_retirement(retirement);
        Ok(removed)
    }

    pub fn source_file_for_handle(
        &self,
        handle: RuntimeHandle,
    ) -> Result<&RuntimeSourceFile, RuntimeStateError> {
        let record = self
            .objects
            .object_for_handle(handle, RuntimeObjectKind::File)?;
        let RuntimeObjectKey::File { file_id } = record.key else {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-vfs",
                "source-file-object-key-invalid",
                "source file handle resolved to a different object identity",
            ));
        };
        self.files.open_file(super::RuntimeSourceFileId(file_id))
    }

    pub fn source_file_byte_length(&self, handle: RuntimeHandle) -> Result<u64, RuntimeStateError> {
        let file_id = self.source_file_for_handle(handle)?.id;
        self.files.file_byte_length(file_id)
    }

    pub fn read_writable_file(
        &mut self,
        memory: &mut GuestMemory,
        handle: RuntimeHandle,
        buffer: GuestAddress,
        bytes_read: GuestAddress,
        requested_bytes: u32,
    ) -> Result<
        (
            RuntimeWritableReadResult,
            Vec<u8>,
            Option<GuestWriteResult>,
            GuestWriteResult,
        ),
        RuntimeStateError,
    > {
        self.require_source_file_operation_state()?;
        let file_id = self.source_file_for_handle(handle)?.id;
        let plan: RuntimeWritableReadPlan =
            self.files.plan_writable_read(file_id, requested_bytes)?;
        memory
            .validate_access(bytes_read, 4, MemoryAccess::Write)
            .map_err(RuntimeStateError::from_memory)?;
        if !plan.bytes().is_empty() {
            memory
                .validate_access(buffer, plan.bytes().len(), MemoryAccess::Write)
                .map_err(RuntimeStateError::from_memory)?;
        }
        self.files.preflight_writable_read(&plan)?;
        let bytes = plan.bytes().to_vec();
        let result = plan.result();
        let mut transaction = memory.begin_transaction();
        let buffer_write = (!bytes.is_empty())
            .then(|| {
                transaction
                    .write(buffer, &bytes)
                    .map_err(RuntimeStateError::from_memory)
            })
            .transpose()?;
        let count_write = transaction
            .write(bytes_read, &result.transferred_bytes.to_le_bytes())
            .map_err(RuntimeStateError::from_memory)?;
        let committed = self.files.commit_writable_read(plan);
        debug_assert_eq!(committed, result);
        transaction.commit_transaction();
        Ok((result, bytes, buffer_write, count_write))
    }

    pub fn write_writable_file(
        &mut self,
        memory: &mut GuestMemory,
        handle: RuntimeHandle,
        bytes_written: GuestAddress,
        bytes: &[u8],
    ) -> Result<(RuntimeWritableWriteResult, GuestWriteResult), RuntimeStateError> {
        self.require_source_file_operation_state()?;
        let file_id = self.source_file_for_handle(handle)?.id;
        let plan: RuntimeWritableWritePlan = self.files.plan_writable_write(file_id, bytes)?;
        memory
            .validate_access(bytes_written, 4, MemoryAccess::Write)
            .map_err(RuntimeStateError::from_memory)?;
        self.files.preflight_writable_write(&plan)?;
        let result = plan.result();
        let mut transaction = memory.begin_transaction();
        let count_write = transaction
            .write(bytes_written, &result.transferred_bytes.to_le_bytes())
            .map_err(RuntimeStateError::from_memory)?;
        let committed = self.files.commit_writable_write(plan);
        debug_assert_eq!(committed, result);
        transaction.commit_transaction();
        Ok((result, count_write))
    }

    pub fn writable_file_needs_commit(
        &self,
        handle: RuntimeHandle,
    ) -> Result<bool, RuntimeStateError> {
        let file_id = self.source_file_for_handle(handle)?.id;
        self.files.writable_file_needs_commit(file_id)
    }

    pub fn begin_writable_file_commit(
        &mut self,
        handle: RuntimeHandle,
        close_after_commit: bool,
    ) -> Result<RuntimeWritableCommitRequest, RuntimeStateError> {
        self.require_source_file_operation_state()?;
        let file_id = self.source_file_for_handle(handle)?.id;
        let scheduler = self
            .scheduler
            .plan_host_request(RUNTIME_WRITABLE_COMMIT_SERVICE.to_owned())?;
        let writable: RuntimeWritableCommitBeginPlan = self.files.plan_writable_commit_begin(
            file_id,
            scheduler.request().request_id,
            close_after_commit,
        )?;
        self.files.preflight_writable_commit_begin(&writable)?;
        let runtime_request = self.scheduler.commit_host_request(scheduler);
        let request = self.files.commit_writable_commit_begin(writable);
        debug_assert_eq!(runtime_request.request_id, request.request_id);
        debug_assert_eq!(runtime_request.service, RUNTIME_WRITABLE_COMMIT_SERVICE);
        Ok(request)
    }

    pub fn writable_file_commit_bytes(&self, request_id: u64) -> Result<&[u8], RuntimeStateError> {
        self.files.writable_commit_bytes(request_id)
    }

    pub fn writable_file_commit_request(
        &self,
        request_id: u64,
    ) -> Result<&RuntimeWritableCommitRequest, RuntimeStateError> {
        self.files.writable_commit_request(request_id)
    }

    pub(crate) fn plan_writable_file_commit_completion(
        &self,
        response: RuntimeWritableCommitHostResponse,
    ) -> Result<RuntimeWritableCommitAggregateCompletionPlan, RuntimeStateError> {
        let scheduler = self.scheduler.plan_host_completion(response.request_id)?;
        if scheduler.request().service != RUNTIME_WRITABLE_COMMIT_SERVICE {
            return Err(RuntimeStateError::invalid_input(
                "runtime-vfs",
                "writable-commit-host-service-mismatch",
                "writable commit completion belongs to another host service",
            ));
        }
        let writable_commit = self.files.plan_writable_commit_completion(response)?;
        self.files
            .preflight_writable_commit_completion(&writable_commit)?;
        let (object, retirement) = if writable_commit.closes_file() {
            let file_id = writable_commit.file_id();
            let key = RuntimeObjectKey::File { file_id: file_id.0 };
            let handle = self.objects.handle_for_object(&key)?;
            let record = self
                .objects
                .object_for_handle(handle, RuntimeObjectKind::File)?;
            if record.open_handles != 1 {
                return Err(RuntimeStateError::conflict(
                    "runtime-vfs",
                    "writable-close-handle-count-invalid",
                    "writable close commit requires one open logical handle",
                ));
            }
            let retirement = self.files.plan_retirement_after_commit(&writable_commit)?;
            self.files.preflight_retirement(&retirement)?;
            let object = self
                .objects
                .plan_close_handle_object_close(handle, Some(&key))?;
            (Some(object), Some(retirement))
        } else {
            (None, None)
        };
        Ok(RuntimeWritableCommitAggregateCompletionPlan {
            scheduler,
            writable_commit,
            object,
            retirement,
        })
    }

    pub(crate) fn commit_writable_file_commit_completion(
        &mut self,
        plan: RuntimeWritableCommitAggregateCompletionPlan,
    ) -> RuntimeWritableCommitCompletion {
        debug_assert!(
            self.files
                .preflight_writable_commit_completion(&plan.writable_commit)
                .is_ok()
        );
        if let Some(retirement) = plan.retirement.as_ref() {
            debug_assert!(self.files.preflight_retirement(retirement).is_ok());
        }
        if let Some(object) = plan.object {
            self.objects
                .commit_close_handle_object_mutation(object)
                .expect("preflighted writable close object plan must commit");
        }
        let scheduler_completion = self.scheduler.commit_host_completion(plan.scheduler);
        debug_assert_eq!(
            scheduler_completion.request_id,
            plan.writable_commit.completion().request.request_id
        );
        let completion = self
            .files
            .commit_writable_commit_completion(plan.writable_commit);
        if let Some(retirement) = plan.retirement {
            self.files.commit_retirement(retirement);
        }
        completion
    }

    pub fn begin_source_file_read(
        &mut self,
        handle: RuntimeHandle,
        buffer: GuestAddress,
        bytes_read: GuestAddress,
        requested_bytes: u32,
    ) -> Result<RuntimeSourceReadRequest, RuntimeStateError> {
        self.require_source_file_operation_state()?;
        let file_id = self.source_file_for_handle(handle)?.id;
        let service = self.files.file_read_service(file_id)?;
        let scheduler_plan = self.scheduler.plan_host_request(service.to_owned())?;
        let source_plan: RuntimeSourceReadBeginPlan = self.files.plan_read_begin(
            file_id,
            scheduler_plan.request().request_id,
            buffer,
            bytes_read,
            requested_bytes,
        )?;
        self.files.preflight_read_begin(&source_plan)?;
        let runtime_request = self.scheduler.commit_host_request(scheduler_plan);
        let source_request = self.files.commit_read_begin(source_plan);
        debug_assert_eq!(runtime_request.request_id, source_request.request_id);
        debug_assert_eq!(runtime_request.service, service);
        Ok(source_request)
    }

    pub(crate) fn plan_source_file_read_completion(
        &self,
        response: RuntimeSourceReadHostResponse,
    ) -> Result<RuntimeSourceReadAggregateCompletionPlan, RuntimeStateError> {
        let scheduler = self.scheduler.plan_host_completion(response.request_id)?;
        if !matches!(
            scheduler.request().service.as_str(),
            RUNTIME_SOURCE_READ_SERVICE | RUNTIME_WRITABLE_READ_SERVICE
        ) {
            return Err(RuntimeStateError::invalid_input(
                "runtime-vfs",
                "source-read-host-service-mismatch",
                "source read completion belongs to another host service",
            ));
        }
        let source_read = self.files.plan_read_completion(response)?;
        if scheduler.request().service != source_read.service() {
            return Err(RuntimeStateError::conflict(
                "runtime-vfs",
                "file-read-host-service-mismatch",
                "file read completion service does not match its backing authority",
            ));
        }
        self.files.preflight_read_completion(&source_read)?;
        Ok(RuntimeSourceReadAggregateCompletionPlan {
            scheduler,
            source_read,
        })
    }

    pub(crate) fn commit_source_file_read_completion(
        &mut self,
        plan: RuntimeSourceReadAggregateCompletionPlan,
    ) -> RuntimeSourceReadCompletion {
        debug_assert!(
            self.files
                .preflight_read_completion(&plan.source_read)
                .is_ok()
        );
        let scheduler_completion = self.scheduler.commit_host_completion(plan.scheduler);
        debug_assert_eq!(
            scheduler_completion.request_id,
            plan.source_read.completion().request.request_id
        );
        self.files.commit_read_completion(plan.source_read)
    }

    pub fn seek_source_file(
        &mut self,
        handle: RuntimeHandle,
        distance: i64,
        origin: u32,
    ) -> Result<u64, RuntimeStateError> {
        self.require_source_file_operation_state()?;
        let file_id = self.source_file_for_handle(handle)?.id;
        let plan = self.files.plan_seek(file_id, distance, origin)?;
        self.files.commit_seek(plan)
    }

    fn require_source_file_operation_state(&self) -> Result<(), RuntimeStateError> {
        if !self.process.is_running()
            || self.current_thread()?.lifecycle != RuntimeThreadLifecycleState::Running
            || !matches!(
                self.scheduler_state()?,
                SchedulerThreadState::Runnable | SchedulerThreadState::InCallback { .. }
            )
        {
            return Err(RuntimeStateError::conflict(
                "runtime-vfs",
                "source-file-operation-state-invalid",
                "source file operation requires a running process and active current thread",
            ));
        }
        Ok(())
    }

    pub fn unhandled_exception_filter(&self) -> Option<GuestAddress> {
        self.process.unhandled_exception_filter()
    }

    pub fn set_unhandled_exception_filter(
        &mut self,
        filter: Option<GuestAddress>,
    ) -> Result<Option<GuestAddress>, RuntimeStateError> {
        self.process.set_unhandled_exception_filter(filter)
    }

    pub fn process_address_space_allocator(&self) -> &ProcessAddressSpaceAllocator {
        &self.address_space_allocator
    }

    pub fn reserve_virtual_memory(
        &mut self,
        memory: &mut GuestMemory,
        request: RuntimeVirtualMemoryReserveRequest,
    ) -> Result<RuntimeVirtualMemoryReservation, RuntimeStateError> {
        self.require_virtual_memory_operation_state()?;
        if request.preferred_address.is_some() || !request.top_down {
            return Err(RuntimeStateError::invalid_input(
                "runtime-virtual-memory",
                "virtual-memory-placement-unsupported",
                "the active virtual memory contract requires a null preferred address and top-down placement",
            ));
        }
        let address_request = request.address_request()?;
        let reservation = self
            .address_space_allocator
            .plan_top_down_reservation(address_request)?;
        let allocation = &reservation.allocation;
        if allocation.usable_byte_length != u64::from(request.byte_length)
            || allocation.guard_pages != 0
            || allocation.purpose != ProcessAddressAllocationPurpose::VirtualMemory
            || allocation.classification != ProcessAddressAllocationClassification::VirtualMemory
            || allocation.lifetime != ProcessAddressAllocationLifetime::Process
            || allocation.release_policy != ProcessAddressAllocationReleasePolicy::Explicit
            || allocation.executable != request.protection.execute
        {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-virtual-memory",
                "virtual-memory-address-plan-invalid",
                "planned virtual memory reservation does not satisfy its runtime contract",
            ));
        }
        let range = GuestRange::new(allocation.range.base, allocation.range.byte_length)
            .map_err(RuntimeStateError::from_memory)?;
        memory
            .preflight_growth(1, 1, 0, "runtime-virtual-memory")
            .map_err(RuntimeStateError::from_memory)?;
        memory
            .preflight_fixed_reservation(
                allocation.range.base,
                allocation.range.byte_length,
                "runtime-virtual-memory",
            )
            .map_err(RuntimeStateError::from_memory)?;
        self.address_space_allocator
            .preflight_reservation_commit(&reservation)?;

        let mut transaction = memory.begin_transaction();
        let memory_owner = transaction
            .create_owner(MemoryOwnerKind::Anonymous)
            .map_err(RuntimeStateError::from_memory)?;
        let reserved = transaction
            .reserve_fixed_for_address_allocation(
                allocation.id.0,
                memory_owner,
                MemoryRegionKind::Anonymous,
                allocation.range.base,
                allocation.range.byte_length,
            )
            .map_err(RuntimeStateError::from_memory)?;
        if reserved != range {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-virtual-memory",
                "virtual-memory-region-range-mismatch",
                "guest memory reserved a different range than the process address plan",
            ));
        }
        self.address_space_allocator
            .preflight_reservation_commit(&reservation)?;
        let result = RuntimeVirtualMemoryReservation {
            address_allocation_id: allocation.id,
            memory_owner,
            range: allocation.range,
            usable_byte_length: request.byte_length,
            top_down: request.top_down,
            protection: request.protection,
        };
        self.address_space_allocator.commit_reservation(reservation);
        transaction.commit_transaction();
        Ok(result)
    }

    fn require_virtual_memory_operation_state(&self) -> Result<(), RuntimeStateError> {
        if !self.process.is_running()
            || self.current_thread()?.lifecycle != RuntimeThreadLifecycleState::Running
            || !matches!(
                self.scheduler_state()?,
                SchedulerThreadState::Runnable | SchedulerThreadState::InCallback { .. }
            )
        {
            return Err(RuntimeStateError::conflict(
                "runtime-virtual-memory",
                "virtual-memory-operation-state-invalid",
                "virtual memory reservation requires a running process and active current thread",
            ));
        }
        Ok(())
    }

    pub fn com(&self) -> &RuntimeCom {
        &self.com
    }

    pub fn resolve_com_method(
        &self,
        endpoint: GuestAddress,
    ) -> Result<RuntimeComMethodTarget, RuntimeStateError> {
        self.com.resolve_method(endpoint)
    }

    pub fn com_interface_for_pointer(
        &self,
        pointer: GuestAddress,
    ) -> Result<&RuntimeComInterfaceRecord, RuntimeStateError> {
        self.com.interface_for_pointer(pointer)
    }

    pub fn com_interface_for_object_iid(
        &self,
        object_id: RuntimeComObjectId,
        iid: RuntimeComIid,
    ) -> Result<&RuntimeComInterfaceRecord, RuntimeStateError> {
        self.com.interface_for_object_iid(object_id, iid)
    }

    pub(crate) fn plan_com_address_reservation(
        &self,
        request: ProcessAddressAllocationRequest,
    ) -> Result<ProcessAddressReservationPlan, RuntimeStateError> {
        validate_com_address_allocation_request(&request)?;
        self.address_space_allocator.plan_reservation(request)
    }

    pub(crate) fn plan_com_registration(
        &self,
        registration: RuntimeComObjectRegistration,
    ) -> Result<RuntimeComMutationPlan, RuntimeStateError> {
        self.com.plan_registration(registration)
    }

    pub(crate) fn commit_com_installation(
        &mut self,
        reservation: ProcessAddressReservationPlan,
        com: RuntimeComMutationPlan,
    ) -> Result<RuntimeComRegistrationResult, RuntimeStateError> {
        self.address_space_allocator
            .preflight_reservation_commit(&reservation)?;
        self.com.preflight(&com)?;
        let result = com.registration_result();
        validate_com_installation_allocation(&reservation.allocation, &result)?;
        self.address_space_allocator.commit_reservation(reservation);
        self.com.commit(com);
        Ok(result)
    }

    pub(crate) fn plan_audio_device_creation(
        &self,
        request: RuntimeAudioDeviceCreateRequest,
    ) -> Result<RuntimeAudioDeviceCreationPlan, RuntimeStateError> {
        self.require_audio_operation_state()?;
        self.audio.plan_device_creation(request)
    }

    pub(crate) fn commit_com_audio_device_installation(
        &mut self,
        reservation: ProcessAddressReservationPlan,
        com: RuntimeComMutationPlan,
        audio: RuntimeAudioDeviceCreationPlan,
    ) -> Result<(RuntimeComRegistrationResult, RuntimeAudioDevice), RuntimeStateError> {
        self.address_space_allocator
            .preflight_reservation_commit(&reservation)?;
        self.com.preflight(&com)?;
        let result = com.registration_result();
        validate_com_installation_allocation(&reservation.allocation, &result)?;
        self.audio
            .preflight_device_creation(&audio, result.object.id)?;
        self.address_space_allocator.commit_reservation(reservation);
        self.com.commit(com);
        let device = self.audio.commit_device_creation(audio, result.object.id);
        Ok((result, device))
    }

    pub(crate) fn plan_com_query_interface(
        &self,
        interface_id: RuntimeComInterfaceId,
        iid: super::RuntimeComIid,
    ) -> Result<RuntimeComMutationPlan, RuntimeStateError> {
        self.com.plan_query_interface(interface_id, iid)
    }

    pub(crate) fn plan_com_add_ref(
        &self,
        interface_id: RuntimeComInterfaceId,
    ) -> Result<RuntimeComMutationPlan, RuntimeStateError> {
        self.com.plan_add_ref(interface_id)
    }

    pub(crate) fn commit_com_mutation(
        &mut self,
        plan: RuntimeComMutationPlan,
    ) -> Result<(), RuntimeStateError> {
        self.com.preflight(&plan)?;
        self.com.commit(plan);
        Ok(())
    }

    pub(crate) fn plan_com_release(
        &self,
        interface_id: RuntimeComInterfaceId,
    ) -> Result<RuntimeComReleasePlan, RuntimeStateError> {
        let com = self.com.plan_release(interface_id)?;
        let result = com.reference_result();
        let retired_object_ids = result
            .retirements
            .iter()
            .map(|retirement| retirement.object_id)
            .collect::<BTreeSet<_>>();
        let audio = self
            .audio
            .plan_com_object_retirements(&retired_object_ids)?;
        let audio_buffer_backings = audio
            .as_ref()
            .map(|plan| self.audio.buffer_backings_for_retirement(plan))
            .transpose()?
            .unwrap_or_default();
        let graphics = self
            .graphics
            .plan_com_object_retirements(&retired_object_ids)?;
        let texture_backings = graphics
            .as_ref()
            .map(|plan| self.graphics.texture_backings_for_retirement(plan))
            .transpose()?
            .unwrap_or_default();
        let vertex_buffer_backings = graphics
            .as_ref()
            .map(|plan| self.graphics.vertex_buffer_backings_for_retirement(plan))
            .transpose()?
            .unwrap_or_default();
        let mut allocation_ids = result
            .retirements
            .iter()
            .map(|retirement| retirement.backing.address_allocation_id)
            .collect::<Vec<_>>();
        allocation_ids.extend(
            audio_buffer_backings
                .iter()
                .map(|(backing, _)| backing.address_allocation_id),
        );
        allocation_ids.extend(
            texture_backings
                .iter()
                .map(|(backing, _)| backing.address_allocation_id),
        );
        allocation_ids.extend(
            vertex_buffer_backings
                .iter()
                .map(|(backing, _)| backing.address_allocation_id),
        );
        let address_space = if allocation_ids.is_empty() {
            None
        } else {
            let plan = self
                .address_space_allocator
                .plan_releases(&allocation_ids)?;
            let (com_allocations, resource_allocations) =
                plan.allocations().split_at(result.retirements.len());
            let (audio_allocations, graphics_allocations) =
                resource_allocations.split_at(audio_buffer_backings.len());
            let (texture_allocations, vertex_buffer_allocations) =
                graphics_allocations.split_at(texture_backings.len());
            for (retirement, released) in result.retirements.iter().zip(com_allocations) {
                let backing = &retirement.backing;
                if released.range != backing.range
                    || released.purpose != ProcessAddressAllocationPurpose::ComEndpoint
                    || released.owner != RUNTIME_COM_ADDRESS_OWNER
                    || released.classification != ProcessAddressAllocationClassification::Com
                    || released.lifetime != ProcessAddressAllocationLifetime::Object
                    || released.release_policy != ProcessAddressAllocationReleasePolicy::Explicit
                    || !released.executable
                {
                    return Err(RuntimeStateError::runtime_defect(
                        "runtime-com",
                        "com-retirement-allocation-mismatch",
                        "retiring COM object does not own the expected process allocation",
                    ));
                }
            }
            let audio_arena = self
                .address_space_allocator
                .allocation_for_purpose(&ProcessAddressAllocationPurpose::AudioResourceArena)?;
            for ((backing, usable_byte_length), released) in
                audio_buffer_backings.iter().zip(audio_allocations)
            {
                if released.parent_id != Some(audio_arena.id)
                    || released.range.base != backing.range.start()
                    || released.range.byte_length != backing.range.byte_length()
                    || released.usable_byte_length != u64::from(*usable_byte_length)
                    || released.purpose != ProcessAddressAllocationPurpose::AudioResource
                    || released.owner != RUNTIME_AUDIO_RESOURCE_ADDRESS_OWNER
                    || released.classification != ProcessAddressAllocationClassification::Audio
                    || released.lifetime != ProcessAddressAllocationLifetime::Object
                    || released.release_policy != ProcessAddressAllocationReleasePolicy::Explicit
                    || released.executable
                {
                    return Err(RuntimeStateError::runtime_defect(
                        "runtime-audio",
                        "audio-buffer-retirement-allocation-mismatch",
                        "retiring audio buffer does not own its planned guest sample backing",
                    ));
                }
            }
            let graphics_arena = self
                .address_space_allocator
                .allocation_for_purpose(&ProcessAddressAllocationPurpose::GraphicsResourceArena)?;
            for ((backing, usable_byte_length), released) in
                texture_backings.iter().zip(texture_allocations)
            {
                if released.parent_id != Some(graphics_arena.id)
                    || released.range.base != backing.range.start()
                    || released.range.byte_length != backing.range.byte_length()
                    || released.usable_byte_length != u64::from(*usable_byte_length)
                    || released.purpose != ProcessAddressAllocationPurpose::GraphicsResource
                    || released.owner != RUNTIME_GRAPHICS_RESOURCE_ADDRESS_OWNER
                    || released.classification != ProcessAddressAllocationClassification::Graphics
                    || released.lifetime != ProcessAddressAllocationLifetime::Object
                    || released.release_policy != ProcessAddressAllocationReleasePolicy::Explicit
                    || released.executable
                {
                    return Err(RuntimeStateError::runtime_defect(
                        "runtime-graphics",
                        "graphics-texture-retirement-allocation-mismatch",
                        "retiring graphics texture does not own its planned guest backing",
                    ));
                }
            }
            for ((backing, usable_byte_length), released) in
                vertex_buffer_backings.iter().zip(vertex_buffer_allocations)
            {
                if released.parent_id != Some(graphics_arena.id)
                    || released.range.base != backing.range.start()
                    || released.range.byte_length != backing.range.byte_length()
                    || released.usable_byte_length != u64::from(*usable_byte_length)
                    || released.purpose != ProcessAddressAllocationPurpose::GraphicsResource
                    || released.owner != RUNTIME_GRAPHICS_RESOURCE_ADDRESS_OWNER
                    || released.classification != ProcessAddressAllocationClassification::Graphics
                    || released.lifetime != ProcessAddressAllocationLifetime::Object
                    || released.release_policy != ProcessAddressAllocationReleasePolicy::Explicit
                    || released.executable
                {
                    return Err(RuntimeStateError::runtime_defect(
                        "runtime-graphics",
                        "graphics-vertex-buffer-retirement-allocation-mismatch",
                        "retiring graphics vertex buffer does not own its planned guest backing",
                    ));
                }
            }
            Some(plan)
        };
        let mut memory_retirements = result
            .retirements
            .iter()
            .map(|retirement| {
                GuestRange::new(
                    retirement.backing.range.base,
                    retirement.backing.range.byte_length,
                )
                .map(|range| (retirement.backing.memory_owner, range))
                .map_err(RuntimeStateError::from_memory)
            })
            .collect::<Result<Vec<_>, _>>()?;
        memory_retirements.extend(
            audio_buffer_backings
                .iter()
                .map(|(backing, _)| (backing.memory_owner, backing.range)),
        );
        memory_retirements.extend(
            texture_backings
                .iter()
                .map(|(backing, _)| (backing.memory_owner, backing.range)),
        );
        memory_retirements.extend(
            vertex_buffer_backings
                .iter()
                .map(|(backing, _)| (backing.memory_owner, backing.range)),
        );
        let gdi = self.gdi.plan_com_object_retirements(&retired_object_ids)?;
        Ok(RuntimeComReleasePlan {
            com,
            address_space,
            audio,
            graphics,
            gdi,
            memory_retirements,
        })
    }

    pub(crate) fn preflight_com_release(
        &self,
        plan: &RuntimeComReleasePlan,
    ) -> Result<(), RuntimeStateError> {
        self.com.preflight(&plan.com)?;
        if let Some(address_space) = &plan.address_space {
            self.address_space_allocator
                .preflight_releases(address_space)
                .map_err(|error| {
                    if error.code == "address-allocation-release-plan-stale" {
                        RuntimeStateError::conflict(
                            "runtime-com",
                            "com-release-allocation-plan-stale",
                            "process address allocations changed after COM release planning",
                        )
                    } else {
                        error
                    }
                })?;
        }
        if let Some(graphics) = &plan.graphics {
            self.graphics.preflight_com_object_retirement(graphics)?;
        }
        if let Some(audio) = &plan.audio {
            self.audio.preflight_com_object_retirement(audio)?;
        }
        if let Some(gdi) = &plan.gdi {
            self.gdi.preflight_com_object_retirements(gdi)?;
        }
        Ok(())
    }

    pub(crate) fn commit_com_release(
        &mut self,
        plan: RuntimeComReleasePlan,
    ) -> Result<RuntimeComReferenceResult, RuntimeStateError> {
        self.preflight_com_release(&plan)?;
        let result = plan.result();
        self.com.commit(plan.com);
        if let Some(address_space) = plan.address_space {
            self.address_space_allocator.commit_releases(address_space);
        }
        if let Some(graphics) = plan.graphics {
            self.graphics.commit_com_object_retirement(graphics);
        }
        if let Some(audio) = plan.audio {
            self.audio.commit_com_object_retirement(audio);
        }
        Ok(result)
    }

    pub(crate) fn plan_initial_process_environment_reservation(
        &self,
        request: ProcessAddressAllocationRequest,
    ) -> Result<ProcessAddressReservationPlan, RuntimeStateError> {
        if request.purpose != ProcessAddressAllocationPurpose::ProcessEnvironment {
            return Err(RuntimeStateError::invalid_input(
                "runtime-process-environment",
                "process-environment-allocation-purpose-invalid",
                "initial process environment requires a process-environment allocation request",
            ));
        }
        self.address_space_allocator.plan_reservation(request)
    }

    pub(crate) fn commit_initial_process_environment_reservation(
        &mut self,
        reservation: ProcessAddressReservationPlan,
        environment: &InitialProcessEnvironment,
    ) -> Result<&RuntimeProcessEnvironment, RuntimeStateError> {
        self.address_space_allocator
            .preflight_reservation_commit(&reservation)?;
        if reservation.allocation.id != environment.address_allocation_id {
            return Err(RuntimeStateError::conflict(
                "runtime-process-environment",
                "process-environment-address-allocation-id-mismatch",
                "materialized process environment belongs to a different planned allocation",
            ));
        }
        let process_plan = self
            .process
            .preflight_environment_registration(environment)?;
        self.process
            .preflight_environment_commit(&process_plan, &reservation.allocation)?;
        let allocation = reservation.allocation.clone();
        self.address_space_allocator.commit_reservation(reservation);
        Ok(self
            .process
            .commit_environment_registration(process_plan, allocation))
    }

    pub fn process_command_line(&self) -> Result<&RuntimeCommandLine, RuntimeStateError> {
        self.process.command_line()
    }

    pub fn process_environment_strings(
        &self,
    ) -> Result<&RuntimeEnvironmentStrings, RuntimeStateError> {
        self.process.environment_strings()
    }

    pub fn acquire_environment_strings_w(
        &mut self,
    ) -> Result<RuntimeEnvironmentStringCopy, RuntimeStateError> {
        self.process.acquire_environment_strings_w()
    }

    pub fn free_environment_strings_w(
        &mut self,
        buffer_address: GuestAddress,
    ) -> Result<RuntimeEnvironmentStringCopy, RuntimeStateError> {
        self.process.free_environment_strings_w(buffer_address)
    }

    pub fn current_thread_id(&self) -> RuntimeThreadId {
        self.scheduler.current_thread_id()
    }

    pub fn current_thread_state(&self) -> Result<&RuntimeThreadState, RuntimeStateError> {
        self.current_thread()
    }

    pub fn current_thread_teb_address(&self) -> Result<GuestAddress, RuntimeStateError> {
        let thread = self.current_thread()?;
        if let Some(environment) = &thread.environment {
            return Ok(environment.teb_address);
        }
        if thread.guest_bootstrap.is_none() {
            return self
                .process
                .environment()
                .map(|environment| environment.initial_teb_address)
                .ok_or_else(|| {
                    RuntimeStateError::not_found(
                        "runtime-thread-environment",
                        "initial-thread-environment-not-registered",
                        "initial runtime thread has no registered process TEB",
                    )
                });
        }
        Err(RuntimeStateError::runtime_defect(
            "runtime-thread-environment",
            "guest-thread-environment-missing",
            "registered guest thread has no thread environment",
        ))
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
        self.register_thread_stack(self.current_thread_id(), stack)
    }

    pub fn register_thread_stack(
        &mut self,
        thread_id: RuntimeThreadId,
        stack: &InitialThreadStack,
    ) -> Result<&RuntimeThreadStack, RuntimeStateError> {
        let thread = self.thread_state(thread_id)?;
        if !matches!(
            thread.lifecycle,
            RuntimeThreadLifecycleState::Initialized | RuntimeThreadLifecycleState::Running
        ) {
            return Err(RuntimeStateError::conflict(
                "runtime-thread-stack",
                "thread-stack-lifecycle-state-invalid",
                "runtime thread stack registration requires an initialized or running thread",
            ));
        }
        if thread.stack.is_some() {
            return Err(RuntimeStateError::conflict(
                "runtime-thread-stack",
                "thread-stack-already-registered",
                "runtime thread stack may be registered only once",
            ));
        }
        if matches!(
            self.process.lifecycle(),
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
                    "thread stack guard exceeds its reserved range",
                )
            })?;
        let reserved_range =
            RuntimeAddressRange::new(stack.stack_range.start(), stack.stack_range.byte_length())?;
        let mut allocator = self.address_space_allocator.clone();
        let allocation = allocator.reserve(ProcessAddressAllocationRequest {
            purpose: ProcessAddressAllocationPurpose::ThreadStack,
            owner: format!("runtime-thread-{}-stack", thread_id.0),
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
            initial_stack_pointer: stack.initial_esp,
            return_sentinel: stack.entry_return_address,
        };
        let thread_plan = self
            .threads
            .plan_stack_registration(thread_id, registered)?;
        self.address_space_allocator = allocator;
        self.threads.commit(thread_plan);
        self.thread_stack(thread_id)
    }

    pub fn grow_current_thread_stack(
        &mut self,
        memory: &mut GuestMemory,
        fault_address: GuestAddress,
        page_fault_error_code: u32,
    ) -> Result<RuntimeThreadStackGrowth, RuntimeStateError> {
        if !self.process.is_running()
            || self.current_thread()?.lifecycle != RuntimeThreadLifecycleState::Running
        {
            return Err(RuntimeStateError::conflict(
                "runtime-thread-stack",
                "thread-stack-growth-lifecycle-state-invalid",
                "thread stack growth requires a running process and current thread",
            ));
        }
        let thread_id = self.current_thread_id();
        let stack = self.thread_stack(thread_id)?.clone();
        let guard_page = stack.guard_page.ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-thread-stack",
                "thread-stack-reserve-exhausted",
                "current thread stack has committed its entire reserved range",
            )
        })?;
        let fault_page = GuestAddress(fault_address.0 & !(GUEST_PAGE_SIZE as u32 - 1));
        if fault_page != guard_page.start() {
            return Err(RuntimeStateError::invalid_input(
                "runtime-thread-stack",
                "thread-stack-fault-outside-guard",
                "page fault does not fall within the current thread stack guard page",
            ));
        }
        if page_fault_error_code & 1 != 0 {
            return Err(RuntimeStateError::invalid_input(
                "runtime-thread-stack",
                "thread-stack-protection-fault",
                "present-page protection faults cannot grow the current thread stack",
            ));
        }
        let stats = memory.stats();
        if stats.generation != stats.acknowledged_generation || stats.pending_invalidation.is_some()
        {
            return Err(RuntimeStateError::conflict(
                "runtime-thread-stack",
                "thread-stack-growth-memory-unsynchronized",
                "thread stack growth requires fully acknowledged guest memory",
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
                    "current thread stack committed size overflowed",
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
                            "current thread stack guard underflowed its reserved range",
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
                "current thread stack committed page count overflowed host accounting",
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

    pub fn service_page_fault(
        &mut self,
        memory: &mut GuestMemory,
        fault_address: GuestAddress,
        page_fault_error_code: u32,
    ) -> Result<RuntimePageFaultResolution, RuntimeStateError> {
        let fault_page = GuestAddress(fault_address.0 & !(GUEST_PAGE_SIZE as u32 - 1));
        let stack_guard_matches = self
            .current_thread()?
            .stack
            .as_ref()
            .and_then(|stack| stack.guard_page)
            .is_some_and(|guard| guard.start() == fault_page);
        if stack_guard_matches {
            return self
                .grow_current_thread_stack(memory, fault_address, page_fault_error_code)
                .map(|growth| RuntimePageFaultResolution::ThreadStackGrowth { growth });
        }
        if self.heaps.large_allocation_contains(fault_address) {
            return self
                .materialize_private_heap_page(memory, fault_address, page_fault_error_code)
                .map(|materialization| {
                    RuntimePageFaultResolution::PrivateHeapPageMaterialization { materialization }
                });
        }
        Err(RuntimeStateError::invalid_input(
            "runtime-page-fault",
            "guest-page-fault-unhandled",
            "page fault does not match a current stack guard or live lazy heap allocation",
        ))
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

    pub fn next_wait_registration_id(&self) -> u64 {
        self.scheduler.next_wait_registration_id()
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

    pub fn module_image_by_load_base(
        &self,
        load_base: GuestAddress,
    ) -> Result<&RuntimeModuleImage, RuntimeModuleImageError> {
        self.modules.images().image_by_load_base(load_base)
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
            process_id: self.process.id().0,
        })
    }

    pub fn current_thread_handle(&self) -> Result<RuntimeHandle, RuntimeStateError> {
        self.thread_handle(self.current_thread_id())
    }

    pub fn resolve_current_thread_pseudo_handle(
        &self,
    ) -> Result<RuntimeCurrentThreadPseudoHandleResolution, RuntimeStateError> {
        let selected_thread_id = self.current_thread_id();
        self.thread_state(selected_thread_id)?;
        let object = self
            .objects
            .object_for_identity(&RuntimeObjectKey::Thread {
                thread_id: selected_thread_id.0,
            })?;
        if object.kind != RuntimeObjectKind::Thread {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-thread-pseudo-handle",
                "current-thread-object-kind-mismatch",
                "selected thread identity resolved to a non-thread runtime object",
            ));
        }
        Ok(RuntimeCurrentThreadPseudoHandleResolution {
            value: RUNTIME_CURRENT_THREAD_PSEUDO_HANDLE,
            selected_thread_id,
            object_id: object.id,
        })
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
            process_id: self.process.id().0,
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
        match handle.kind()? {
            RuntimeObjectKind::Mutex => {
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
                        self.synchronization
                            .plan_mutex_retirement(RuntimeMutexId(mutex_id))
                    })
                    .transpose()?;
                let object_plan = self.objects.plan_close_handle_object_close(
                    handle,
                    retirement.as_ref().map(|_| &record.key),
                )?;
                let removed = object_plan.closed_entry();
                self.objects
                    .commit_close_handle_object_mutation(object_plan)?;
                if let Some(retirement) = retirement {
                    self.synchronization.commit_mutex_retirement(retirement);
                }
                Ok(removed)
            }
            RuntimeObjectKind::WaitableTimer => {
                let record = self
                    .objects
                    .object_for_handle(handle, RuntimeObjectKind::WaitableTimer)?
                    .clone();
                if self
                    .scheduler
                    .object_waiters()
                    .get(&record.id)
                    .is_some_and(|waiters| !waiters.is_empty())
                {
                    return Err(RuntimeStateError::conflict(
                        "runtime-waitable-timer",
                        "waitable-timer-close-with-pending-wait-unsupported",
                        "bounded waitable timer pacing does not close a timer with a pending wait",
                    ));
                }
                let RuntimeObjectKey::WaitableTimer { timer_id } = record.key.clone() else {
                    return Err(RuntimeStateError::runtime_defect(
                        "runtime-waitable-timer",
                        "waitable-timer-object-key-invalid",
                        "waitable timer handle resolved to a different object identity",
                    ));
                };
                let retirement = (record.open_handles == 1)
                    .then(|| {
                        self.timers
                            .plan_waitable_timer_retirement(RuntimeWaitableTimerId(timer_id))
                    })
                    .transpose()?;
                let object_plan = self.objects.plan_close_handle_object_close(
                    handle,
                    retirement.as_ref().map(|_| &record.key),
                )?;
                let removed = object_plan.closed_entry();
                self.objects
                    .commit_close_handle_object_mutation(object_plan)?;
                if let Some(retirement) = retirement {
                    self.timers.commit_waitable_timer_retirement(retirement);
                }
                Ok(removed)
            }
            RuntimeObjectKind::Event => {
                let record = self
                    .objects
                    .object_for_handle(handle, RuntimeObjectKind::Event)?
                    .clone();
                if self
                    .scheduler
                    .object_waiters()
                    .get(&record.id)
                    .is_some_and(|waiters| !waiters.is_empty())
                {
                    return Err(RuntimeStateError::conflict(
                        "runtime-event",
                        "event-close-with-pending-wait-unsupported",
                        "bounded event handling does not close an event with a pending wait",
                    ));
                }
                let retiring_key = (record.open_handles == 1).then_some(&record.key);
                let plan = self
                    .objects
                    .plan_close_handle_object_close(handle, retiring_key)?;
                let removed = plan.closed_entry();
                self.objects.commit_close_handle_object_mutation(plan)?;
                Ok(removed)
            }
            RuntimeObjectKind::File => {
                let record = self
                    .objects
                    .object_for_handle(handle, RuntimeObjectKind::File)?
                    .clone();
                let RuntimeObjectKey::File { file_id } = record.key.clone() else {
                    return Err(RuntimeStateError::runtime_defect(
                        "runtime-vfs",
                        "source-file-object-key-invalid",
                        "source file handle resolved to a different object identity",
                    ));
                };
                let retirement = (record.open_handles == 1)
                    .then(|| {
                        self.files
                            .plan_retirement(super::RuntimeSourceFileId(file_id))
                    })
                    .transpose()?;
                let object_plan = self.objects.plan_close_handle_object_close(
                    handle,
                    retirement.as_ref().map(|_| &record.key),
                )?;
                if let Some(retirement) = retirement.as_ref() {
                    self.files.preflight_retirement(retirement)?;
                }
                let removed = object_plan.closed_entry();
                self.objects
                    .commit_close_handle_object_mutation(object_plan)?;
                if let Some(retirement) = retirement {
                    self.files.commit_retirement(retirement);
                }
                Ok(removed)
            }
            RuntimeObjectKind::Process
            | RuntimeObjectKind::Thread
            | RuntimeObjectKind::FileSearch
            | RuntimeObjectKind::PrivateHeap
            | RuntimeObjectKind::Module => self.objects.close_handle(handle),
        }
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

    pub fn allocated_tls_indices(&self) -> &BTreeSet<u32> {
        self.threads.allocated_tls_indices()
    }

    pub fn fls_slots(&self) -> &BTreeMap<u32, RuntimeFlsSlot> {
        self.threads.fls_slots()
    }

    pub fn clock(&self) -> &RuntimeClockState {
        self.process.clock()
    }

    pub fn clock_profile(&self) -> &RuntimeClockProfile {
        self.process.clock_profile()
    }

    pub fn configure_clock_profile(
        &mut self,
        profile: RuntimeClockProfile,
    ) -> Result<&RuntimeClockProfile, RuntimeStateError> {
        let plan = self.process.plan_clock_profile_configuration(profile)?;
        self.process.commit_clock_profile_configuration(plan)
    }

    pub fn advance_scheduled_clock_to(
        &mut self,
        target_time_100ns: u64,
    ) -> Result<RuntimeTimerClockState, RuntimeStateError> {
        self.require_timer_operation_state()?;
        if !self.process.uses_scheduled_shared_clock() {
            return Err(RuntimeStateError::conflict(
                "runtime-clock",
                "scheduled-clock-profile-inactive",
                "explicit scheduled clock advancement requires the scheduled shared profile",
            ));
        }
        let plan = self.process.plan_clock_advance(target_time_100ns)?;
        self.process.preflight_clock_advance(&plan)?;
        self.process.commit_clock_advance(plan)
    }

    pub fn advance_scheduled_clock_for_execution(
        &mut self,
        retired_instructions: u64,
    ) -> Result<Option<RuntimeTimerClockState>, RuntimeStateError> {
        self.require_timer_operation_state()?;
        if !self.process.uses_scheduled_shared_clock() {
            return Ok(None);
        }
        let plan = self
            .process
            .plan_scheduled_execution_clock_advance(retired_instructions)?;
        self.process
            .preflight_scheduled_execution_clock_advance(&plan)?;
        self.process
            .commit_scheduled_execution_clock_advance(plan)
            .map(Some)
    }

    pub fn synchronize_scheduled_clock_for_host_execution(
        &mut self,
        retired_instructions: u64,
        host_elapsed_time_100ns: u64,
    ) -> Result<RuntimeTimerClockState, RuntimeStateError> {
        self.require_timer_operation_state()?;
        if !self.process.uses_scheduled_shared_clock() {
            return Err(RuntimeStateError::conflict(
                "runtime-clock",
                "scheduled-host-clock-profile-inactive",
                "host-monotonic clock synchronization requires the scheduled shared profile",
            ));
        }
        let plan = self.process.plan_scheduled_host_execution_clock_advance(
            retired_instructions,
            host_elapsed_time_100ns,
        )?;
        self.process
            .preflight_scheduled_execution_clock_advance(&plan)?;
        self.process.commit_scheduled_execution_clock_advance(plan)
    }

    pub fn sample_tick_count(&mut self, increment_per_read: u32) -> Result<u32, RuntimeStateError> {
        let plan = self.process.plan_tick_count_sample(increment_per_read)?;
        self.process.commit_tick_count_sample(plan)
    }

    pub fn sample_performance_counter(
        &mut self,
        increment_per_read: u64,
    ) -> Result<u64, RuntimeStateError> {
        let plan = self
            .process
            .plan_performance_counter_sample(increment_per_read)?;
        self.process.commit_performance_counter_sample(plan)
    }

    pub fn startup_info(&self) -> &RuntimeStartupInfo {
        self.process.startup_info()
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

    pub fn current_com_apartment(
        &self,
    ) -> Result<Option<&RuntimeComApartmentState>, RuntimeStateError> {
        self.threads.com_apartment(self.current_thread_id())
    }

    pub fn initialize_current_com_apartment(
        &mut self,
        model: RuntimeComApartmentModel,
    ) -> Result<RuntimeComInitializationOutcome, RuntimeStateError> {
        let plan = self
            .threads
            .plan_com_initialization(self.current_thread_id(), model)?;
        self.threads.commit_com_initialization(plan)
    }

    pub fn uninitialize_current_com_apartment(
        &mut self,
    ) -> Result<RuntimeComUninitializationOutcome, RuntimeStateError> {
        let plan = self
            .threads
            .plan_com_uninitialization(self.current_thread_id())?;
        self.threads.commit_com_uninitialization(plan)
    }

    pub fn current_thread_affinity_mask(
        &self,
        process_mask: u32,
    ) -> Result<u32, RuntimeStateError> {
        self.threads
            .effective_affinity_mask(self.current_thread_id(), process_mask)
    }

    pub fn set_current_thread_affinity_mask(
        &mut self,
        process_mask: u32,
        requested_mask: u32,
    ) -> Result<u32, RuntimeStateError> {
        let (plan, previous_mask) = self.threads.plan_affinity_mask(
            self.current_thread_id(),
            process_mask,
            requested_mask,
        )?;
        self.threads.commit_affinity_mask(plan)?;
        Ok(previous_mask)
    }

    pub fn thread_priority(
        &self,
        thread_id: RuntimeThreadId,
    ) -> Result<RuntimeThreadPriority, RuntimeStateError> {
        self.threads.effective_priority(thread_id)
    }

    pub fn set_thread_priority(
        &mut self,
        handle: RuntimeHandle,
        requested_priority: RuntimeThreadPriority,
    ) -> Result<RuntimeThreadPriority, RuntimeStateError> {
        let object = self
            .objects
            .object_for_handle(handle, RuntimeObjectKind::Thread)?;
        let RuntimeObjectKey::Thread { thread_id } = object.key else {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-thread-priority",
                "thread-priority-object-key-invalid",
                "thread handle resolved to a different object identity",
            ));
        };
        let thread_id = RuntimeThreadId(thread_id);
        let (plan, previous_priority) =
            self.threads.plan_priority(thread_id, requested_priority)?;
        self.threads.commit_priority(plan)?;
        Ok(previous_priority)
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
        let process_plan = self.process.plan_start()?;
        let thread_plan = self.threads.plan_start(thread_id)?;
        if self.scheduler_state()? != SchedulerThreadState::Initialized {
            return Err(RuntimeStateError::conflict(
                "runtime-lifecycle",
                "initial-thread-start-transition-invalid",
                "initial thread start requires matching process, thread, and scheduler bootstrap states",
            ));
        }
        let scheduler_plan = self
            .scheduler
            .plan_transition(thread_id, SchedulerThreadState::Runnable)?;
        self.process.preflight_mutation_commit(&process_plan)?;
        self.process.commit_mutation(process_plan);
        self.threads.commit(thread_plan);
        self.scheduler.commit_lane(scheduler_plan);
        Ok(())
    }

    pub fn register_thread(
        &mut self,
        thread_id: RuntimeThreadId,
        initial_last_error: u32,
    ) -> Result<RuntimeHandle, RuntimeStateError> {
        if !self.process.is_running() {
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

    pub fn create_guest_thread(
        &mut self,
        memory: &mut GuestMemory,
        request: RuntimeCreateThreadRequest,
    ) -> Result<RuntimeCreateThreadResult, RuntimeStateError> {
        if !self.process.is_running()
            || self.current_thread()?.lifecycle != RuntimeThreadLifecycleState::Running
            || !self.scheduler.current_state()?.is_executable()
        {
            return Err(RuntimeStateError::conflict(
                "runtime-thread-bootstrap",
                "guest-thread-creation-lifecycle-state-invalid",
                "guest thread creation requires a running process and executable caller thread",
            ));
        }
        validate_guest_thread_stack_sizes(request.stack_reserve_bytes, request.stack_commit_bytes)?;
        memory
            .validate_access(request.entry_point, 1, MemoryAccess::Execute)
            .map_err(RuntimeStateError::from_memory)?;
        if let Some(output) = request.thread_id_output {
            memory
                .validate_access(output, 4, MemoryAccess::Write)
                .map_err(RuntimeStateError::from_memory)?;
        }

        let process_environment = self.process.environment().cloned().ok_or_else(|| {
            RuntimeStateError::not_found(
                "runtime-thread-bootstrap",
                "process-environment-unregistered",
                "guest thread creation requires the registered process PEB authority",
            )
        })?;
        let return_sentinel = self
            .current_thread()?
            .stack
            .as_ref()
            .map(|stack| stack.return_sentinel)
            .ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-thread-bootstrap",
                    "thread-return-sentinel-unregistered",
                    "guest thread creation requires the process entry return sentinel",
                )
            })?;
        memory
            .validate_access(return_sentinel, 1, MemoryAccess::Execute)
            .map_err(RuntimeStateError::from_memory)?;

        let thread_id_plan = self.threads.plan_thread_id_allocation()?;
        let thread_id = thread_id_plan.thread_id();
        let guard_pages = u32::from(request.stack_reserve_bytes > request.stack_commit_bytes);
        let stack_usable_bytes = request
            .stack_reserve_bytes
            .checked_sub(u64::from(guard_pages) * GUEST_PAGE_SIZE)
            .ok_or_else(|| {
                RuntimeStateError::invalid_input(
                    "runtime-thread-stack",
                    "guest-thread-stack-guard-underflow",
                    "guest thread stack reserve cannot contain its growth guard",
                )
            })?;
        let address_space = self.address_space_allocator.plan_reservations(vec![
            (
                None,
                ProcessAddressAllocationRequest {
                    purpose: ProcessAddressAllocationPurpose::ThreadStack,
                    owner: format!("runtime-thread-{}-stack", thread_id.0),
                    preferred_address: None,
                    search_range: RuntimeAddressRange::new(
                        RUNTIME_GUEST_THREAD_STACK_ARENA_BASE,
                        RUNTIME_GUEST_THREAD_STACK_ARENA_SIZE,
                    )?,
                    reserve_size: stack_usable_bytes,
                    alignment: GUEST_PAGE_SIZE as u32,
                    guard_pages,
                    classification: ProcessAddressAllocationClassification::Stack,
                    lifetime: ProcessAddressAllocationLifetime::Thread,
                    release_policy: ProcessAddressAllocationReleasePolicy::Explicit,
                    executable: false,
                },
            ),
            (
                None,
                ProcessAddressAllocationRequest {
                    purpose: ProcessAddressAllocationPurpose::ThreadEnvironment,
                    owner: format!("runtime-thread-{}-environment", thread_id.0),
                    preferred_address: None,
                    search_range: RuntimeAddressRange::new(
                        RUNTIME_GUEST_THREAD_ENVIRONMENT_ARENA_BASE,
                        RUNTIME_GUEST_THREAD_ENVIRONMENT_ARENA_SIZE,
                    )?,
                    reserve_size: GUEST_PAGE_SIZE,
                    alignment: GUEST_PAGE_SIZE as u32,
                    guard_pages: 0,
                    classification: ProcessAddressAllocationClassification::ProcessEnvironment,
                    lifetime: ProcessAddressAllocationLifetime::Thread,
                    release_policy: ProcessAddressAllocationReleasePolicy::Explicit,
                    executable: false,
                },
            ),
        ])?;
        let [stack_allocation, environment_allocation] = address_space.allocations() else {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-thread-bootstrap",
                "guest-thread-address-plan-shape-invalid",
                "guest thread address plan did not contain stack and TEB allocations",
            ));
        };
        let stack_allocation = stack_allocation.clone();
        let environment_allocation = environment_allocation.clone();
        if stack_allocation.range.byte_length != request.stack_reserve_bytes
            || environment_allocation.range.byte_length != GUEST_PAGE_SIZE
        {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-thread-bootstrap",
                "guest-thread-address-plan-size-invalid",
                "guest thread address allocator changed the requested stack or TEB size",
            ));
        }

        let stack_top = GuestAddress(
            u32::try_from(stack_allocation.range.end_exclusive()?).map_err(|_| {
                RuntimeStateError::runtime_defect(
                    "runtime-thread-stack",
                    "guest-thread-stack-top-unrepresentable",
                    "planned guest thread stack top does not fit an x86 address",
                )
            })?,
        );
        let commit_bytes_u32 = u32::try_from(request.stack_commit_bytes).map_err(|_| {
            RuntimeStateError::invalid_input(
                "runtime-thread-stack",
                "guest-thread-stack-commit-unrepresentable",
                "guest thread stack commit does not fit x86 address accounting",
            )
        })?;
        let committed_base =
            GuestAddress(stack_top.0.checked_sub(commit_bytes_u32).ok_or_else(|| {
                RuntimeStateError::runtime_defect(
                    "runtime-thread-stack",
                    "guest-thread-stack-commit-underflow",
                    "planned guest thread stack commit underflowed its top",
                )
            })?);
        let committed_range = GuestRange::new(committed_base, request.stack_commit_bytes)
            .map_err(RuntimeStateError::from_memory)?;
        let reserved_range = GuestRange::new(
            stack_allocation.range.base,
            stack_allocation.range.byte_length,
        )
        .map_err(RuntimeStateError::from_memory)?;
        let environment_range = GuestRange::new(
            environment_allocation.range.base,
            environment_allocation.range.byte_length,
        )
        .map_err(RuntimeStateError::from_memory)?;
        let initial_stack_pointer = GuestAddress(stack_top.0.checked_sub(8).ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-thread-stack",
                "guest-thread-initial-frame-underflow",
                "guest thread stack cannot contain its entry frame",
            )
        })?);
        let guard_page = (guard_pages != 0)
            .then(|| {
                committed_base
                    .0
                    .checked_sub(GUEST_PAGE_SIZE as u32)
                    .map(GuestAddress)
                    .ok_or_else(|| {
                        RuntimeStateError::runtime_defect(
                            "runtime-thread-stack",
                            "guest-thread-stack-guard-underflow",
                            "guest thread stack guard underflowed x86 address accounting",
                        )
                    })
            })
            .transpose()?
            .map(|base| GuestRange::new(base, GUEST_PAGE_SIZE))
            .transpose()
            .map_err(RuntimeStateError::from_memory)?;
        let committed_pages = usize::try_from(request.stack_commit_bytes / GUEST_PAGE_SIZE)
            .map_err(|_| {
                RuntimeStateError::resource_limit(
                    "runtime-thread-stack",
                    "guest-thread-stack-page-count-unrepresentable",
                    "guest thread committed stack pages do not fit host accounting",
                )
            })?;

        memory
            .preflight_growth(
                2,
                2,
                committed_pages.checked_add(1).ok_or_else(|| {
                    RuntimeStateError::resource_limit(
                        "runtime-thread-bootstrap",
                        "guest-thread-physical-page-count-overflow",
                        "guest thread physical page count overflowed host accounting",
                    )
                })?,
                "runtime-thread-bootstrap",
            )
            .map_err(RuntimeStateError::from_memory)?;
        memory
            .preflight_fixed_reservation(
                reserved_range.start(),
                reserved_range.byte_length(),
                "runtime-thread-bootstrap",
            )
            .map_err(RuntimeStateError::from_memory)?;
        memory
            .preflight_fixed_reservation(
                environment_range.start(),
                environment_range.byte_length(),
                "runtime-thread-bootstrap",
            )
            .map_err(RuntimeStateError::from_memory)?;

        let mut transaction = memory.begin_transaction();
        let stack_owner = transaction
            .create_owner(MemoryOwnerKind::Anonymous)
            .map_err(RuntimeStateError::from_memory)?;
        transaction
            .reserve_fixed_for_address_allocation(
                stack_allocation.id.0,
                stack_owner,
                MemoryRegionKind::Stack,
                reserved_range.start(),
                reserved_range.byte_length(),
            )
            .map_err(RuntimeStateError::from_memory)?;
        transaction
            .commit(
                stack_owner,
                committed_range,
                PageProtection {
                    read: true,
                    write: true,
                    execute: false,
                },
            )
            .map_err(RuntimeStateError::from_memory)?;
        let mut initial_frame = Vec::with_capacity(8);
        initial_frame.extend_from_slice(&return_sentinel.0.to_le_bytes());
        initial_frame.extend_from_slice(&request.parameter.to_le_bytes());
        transaction
            .write_initialization(stack_owner, initial_stack_pointer, &initial_frame)
            .map_err(RuntimeStateError::from_memory)?;
        transaction
            .seal_owner(stack_owner)
            .map_err(RuntimeStateError::from_memory)?;

        let environment_owner = transaction
            .create_owner(MemoryOwnerKind::Anonymous)
            .map_err(RuntimeStateError::from_memory)?;
        transaction
            .reserve_fixed_for_address_allocation(
                environment_allocation.id.0,
                environment_owner,
                MemoryRegionKind::ThreadEnvironmentBlock,
                environment_range.start(),
                environment_range.byte_length(),
            )
            .map_err(RuntimeStateError::from_memory)?;
        transaction
            .commit(
                environment_owner,
                environment_range,
                PageProtection {
                    read: true,
                    write: true,
                    execute: false,
                },
            )
            .map_err(RuntimeStateError::from_memory)?;
        let teb_address = environment_range.start();
        let mut teb = [0_u8; X86_TEB_INITIALIZATION_BYTES];
        teb[X86_TEB_EXCEPTION_LIST_OFFSET..X86_TEB_EXCEPTION_LIST_OFFSET + 4]
            .copy_from_slice(&u32::MAX.to_le_bytes());
        teb[X86_TEB_STACK_BASE_OFFSET..X86_TEB_STACK_BASE_OFFSET + 4]
            .copy_from_slice(&stack_top.0.to_le_bytes());
        teb[X86_TEB_STACK_LIMIT_OFFSET..X86_TEB_STACK_LIMIT_OFFSET + 4]
            .copy_from_slice(&committed_base.0.to_le_bytes());
        teb[X86_TEB_SELF_POINTER_OFFSET..X86_TEB_SELF_POINTER_OFFSET + 4]
            .copy_from_slice(&teb_address.0.to_le_bytes());
        teb[X86_TEB_PROCESS_ID_OFFSET..X86_TEB_PROCESS_ID_OFFSET + 4]
            .copy_from_slice(&self.process.id().0.to_le_bytes());
        teb[X86_TEB_THREAD_ID_OFFSET..X86_TEB_THREAD_ID_OFFSET + 4]
            .copy_from_slice(&thread_id.0.to_le_bytes());
        teb[X86_TEB_PEB_POINTER_OFFSET..X86_TEB_PEB_POINTER_OFFSET + 4]
            .copy_from_slice(&process_environment.peb_address.0.to_le_bytes());
        transaction
            .write_initialization(environment_owner, teb_address, &teb)
            .map_err(RuntimeStateError::from_memory)?;
        transaction
            .seal_owner(environment_owner)
            .map_err(RuntimeStateError::from_memory)?;

        let stack = RuntimeThreadStack {
            thread_id,
            address_allocation_id: stack_allocation.id,
            memory_owner: stack_owner,
            reserved_range,
            committed_range,
            guard_page,
            committed_pages,
            stack_top,
            initial_stack_pointer,
            return_sentinel,
        };
        let environment = RuntimeThreadEnvironment {
            thread_id,
            address_allocation_id: environment_allocation.id,
            memory_owner: environment_owner,
            range: environment_range,
            teb_address,
            peb_address: process_environment.peb_address,
        };
        let bootstrap = RuntimeGuestThreadBootstrap {
            thread_id,
            entry_point: request.entry_point,
            parameter: request.parameter,
            initial_stack_pointer,
            fs_base: teb_address,
            return_sentinel,
        };
        let thread_plan = self.threads.plan_guest_registration(
            thread_id_plan,
            0,
            stack.clone(),
            environment.clone(),
            bootstrap.clone(),
        )?;
        let scheduler_state = if request.initial_suspend_count == 0 {
            SchedulerThreadState::Runnable
        } else {
            SchedulerThreadState::Suspended {
                count: request.initial_suspend_count,
            }
        };
        let scheduler_plan = self
            .scheduler
            .plan_registration(thread_id, scheduler_state)?;
        self.messages.preflight_register_thread(thread_id)?;
        let object_plan: RuntimeObjectRegistrationPlan = self
            .objects
            .plan_object_and_handle_registration(RuntimeObjectKey::Thread {
                thread_id: thread_id.0,
            })?;
        let handle = object_plan.handle();
        let thread_id_write = request
            .thread_id_output
            .map(|output| transaction.write(output, &thread_id.0.to_le_bytes()))
            .transpose()
            .map_err(RuntimeStateError::from_memory)?;

        self.address_space_allocator
            .preflight_reservations_commit(&address_space)?;
        self.threads
            .preflight_guest_registration_commit(&thread_id_plan, &thread_plan)?;
        self.scheduler.preflight_lane_commit(&scheduler_plan)?;
        self.objects
            .preflight_object_registration_commit(&object_plan)?;

        self.address_space_allocator
            .commit_reservations(address_space);
        self.threads
            .commit_guest_registration(thread_id_plan, thread_plan);
        self.scheduler.commit_lane(scheduler_plan);
        let (_, committed_handle) = self.objects.commit_object_registration(object_plan);
        debug_assert_eq!(committed_handle, handle);
        self.messages.commit_register_thread(thread_id);
        transaction.commit_transaction();

        Ok(RuntimeCreateThreadResult {
            thread_id,
            handle,
            stack,
            environment,
            bootstrap,
            thread_id_output: request.thread_id_output,
            thread_id_write,
        })
    }

    pub fn resume_thread(
        &mut self,
        handle: RuntimeHandle,
    ) -> Result<RuntimeResumeThreadResult, RuntimeStateError> {
        let object = self
            .objects
            .object_for_handle(handle, RuntimeObjectKind::Thread)?;
        let RuntimeObjectKey::Thread { thread_id } = object.key else {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-thread-suspend",
                "thread-handle-object-key-invalid",
                "thread handle resolved to a different object identity",
            ));
        };
        let thread_id = RuntimeThreadId(thread_id);
        let thread = self.threads.state(thread_id)?;
        if thread.lifecycle != RuntimeThreadLifecycleState::Running {
            return Err(RuntimeStateError::conflict(
                "runtime-thread-suspend",
                "thread-resume-lifecycle-state-invalid",
                "ResumeThread requires a live runtime thread",
            ));
        }
        let state = self.scheduler.state(thread_id)?;
        let (previous_suspend_count, remaining_suspend_count, became_runnable, plan) = match state {
            SchedulerThreadState::Suspended { count } if count != 0 => {
                let remaining = count - 1;
                let next = if remaining == 0 {
                    SchedulerThreadState::Runnable
                } else {
                    SchedulerThreadState::Suspended { count: remaining }
                };
                (
                    count,
                    remaining,
                    remaining == 0,
                    Some(self.scheduler.plan_transition(thread_id, next)?),
                )
            }
            SchedulerThreadState::Suspended { .. } => {
                return Err(RuntimeStateError::runtime_defect(
                    "runtime-thread-suspend",
                    "thread-suspend-count-zero",
                    "suspended scheduler state contains a zero suspend count",
                ));
            }
            SchedulerThreadState::Runnable
            | SchedulerThreadState::WaitingHost { .. }
            | SchedulerThreadState::WaitingObject { .. }
            | SchedulerThreadState::WaitingCriticalSection { .. }
            | SchedulerThreadState::WaitingMessage
            | SchedulerThreadState::InCallback { .. } => (0, 0, false, None),
            SchedulerThreadState::Initialized
            | SchedulerThreadState::Terminating { .. }
            | SchedulerThreadState::Terminated { .. } => {
                return Err(RuntimeStateError::conflict(
                    "runtime-thread-suspend",
                    "thread-resume-scheduler-state-invalid",
                    "ResumeThread requires an initialized live execution lane",
                ));
            }
        };
        if let Some(plan) = plan {
            self.scheduler.preflight_lane_commit(&plan)?;
            self.scheduler.commit_lane(plan);
        }
        Ok(RuntimeResumeThreadResult {
            thread_id,
            handle,
            previous_suspend_count,
            remaining_suspend_count,
            became_runnable,
        })
    }

    pub fn select_thread(&mut self, thread_id: RuntimeThreadId) -> Result<(), RuntimeStateError> {
        if !self.process.is_running() {
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
                    | SchedulerThreadState::WaitingCriticalSection { .. }
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

    pub fn select_next_executable_thread(
        &mut self,
    ) -> Result<Option<RuntimeThreadExecutionSelection>, RuntimeStateError> {
        if !self.process.is_running() {
            return Err(RuntimeStateError::conflict(
                "runtime-thread-registry",
                "thread-selection-process-state-invalid",
                "thread selection requires a running process",
            ));
        }
        let Some((thread_id, _)) = self.preferred_executable_thread(true)? else {
            return Ok(None);
        };
        let plan = self.scheduler.plan_selection(thread_id)?;
        let state = self.scheduler_state_for_thread(thread_id)?;
        if !state.is_executable()
            || self.thread_state(thread_id)?.lifecycle != RuntimeThreadLifecycleState::Running
        {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-thread-registry",
                "executable-thread-state-mismatch",
                "scheduler selected a thread without matching executable runtime state",
            ));
        }
        let thread_key = RuntimeObjectKey::Thread {
            thread_id: thread_id.0,
        };
        if !self.objects.contains_identity(&thread_key) {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-thread-registry",
                "thread-registry-object-missing",
                "selected executable thread has no runtime object identity",
            ));
        }
        let selection = RuntimeThreadExecutionSelection {
            previous_thread_id: plan.previous(),
            thread_id,
            state,
        };
        self.scheduler.commit_selection(plan);
        Ok(Some(selection))
    }

    pub fn complete_api_execution_boundary(
        &mut self,
    ) -> Result<Option<RuntimeThreadExecutionSelection>, RuntimeStateError> {
        self.complete_api_execution_boundary_with_disposition(
            RuntimeApiBoundaryDisposition::ContinueQuantum,
        )
    }

    pub fn complete_api_execution_boundary_with_disposition(
        &mut self,
        disposition: RuntimeApiBoundaryDisposition,
    ) -> Result<Option<RuntimeThreadExecutionSelection>, RuntimeStateError> {
        if disposition == RuntimeApiBoundaryDisposition::TerminateCurrentThread {
            return Err(RuntimeStateError::invalid_input(
                "runtime-thread-registry",
                "terminating-api-boundary-requires-thread-exit",
                "a terminating API boundary must commit through the thread lifecycle owner",
            ));
        }
        if !self.process.is_running()
            || self.current_thread()?.lifecycle != RuntimeThreadLifecycleState::Running
            || !self.scheduler_state()?.is_executable()
        {
            return Err(RuntimeStateError::conflict(
                "runtime-thread-registry",
                "api-boundary-process-state-invalid",
                "an API execution boundary requires a running process and executable current thread",
            ));
        }
        let current_key = RuntimeObjectKey::Thread {
            thread_id: self.current_thread_id().0,
        };
        if !self.objects.contains_identity(&current_key) {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-thread-registry",
                "thread-registry-object-missing",
                "current API boundary thread has no runtime object identity",
            ));
        }
        let preferred_competitor = self.preferred_executable_thread(false)?;
        let current_priority = self.threads.effective_priority(self.current_thread_id())?;
        let (competitor, force_competitor) = match (disposition, preferred_competitor) {
            (_, None) => (None, false),
            (RuntimeApiBoundaryDisposition::YieldCurrentThread, Some((thread_id, _))) => {
                (Some(thread_id), false)
            }
            (RuntimeApiBoundaryDisposition::ContinueQuantum, Some((thread_id, priority)))
                if priority.scheduling_rank() > current_priority.scheduling_rank() =>
            {
                (Some(thread_id), true)
            }
            (RuntimeApiBoundaryDisposition::ContinueQuantum, Some((thread_id, priority)))
                if priority == current_priority =>
            {
                (Some(thread_id), false)
            }
            (RuntimeApiBoundaryDisposition::ContinueQuantum, Some(_)) => (None, false),
            (RuntimeApiBoundaryDisposition::BlockCurrentThread, Some((thread_id, _))) => {
                (Some(thread_id), false)
            }
            (RuntimeApiBoundaryDisposition::TerminateCurrentThread, _) => unreachable!(),
        };
        let plan = self.scheduler.plan_completed_api_boundary_with_competitor(
            disposition,
            competitor,
            force_competitor,
            true,
        )?;
        let selection = if let Some(selection) = plan.selection() {
            let thread_id = selection.next();
            let state = self.scheduler_state_for_thread(thread_id)?;
            if !state.is_executable()
                || self.thread_state(thread_id)?.lifecycle != RuntimeThreadLifecycleState::Running
            {
                return Err(RuntimeStateError::runtime_defect(
                    "runtime-thread-registry",
                    "api-boundary-thread-state-mismatch",
                    "API boundary selected a thread without matching executable runtime state",
                ));
            }
            let thread_key = RuntimeObjectKey::Thread {
                thread_id: thread_id.0,
            };
            if !self.objects.contains_identity(&thread_key) {
                return Err(RuntimeStateError::runtime_defect(
                    "runtime-thread-registry",
                    "thread-registry-object-missing",
                    "API boundary selected a thread without runtime object identity",
                ));
            }
            Some(RuntimeThreadExecutionSelection {
                previous_thread_id: selection.previous(),
                thread_id,
                state,
            })
        } else {
            None
        };
        self.scheduler.preflight_completed_api_boundary(&plan)?;
        self.scheduler.commit_completed_api_boundary(plan);
        Ok(selection)
    }

    fn preferred_executable_thread(
        &self,
        include_current: bool,
    ) -> Result<Option<(RuntimeThreadId, RuntimeThreadPriority)>, RuntimeStateError> {
        let thread_ids = self
            .scheduler
            .thread_states()
            .keys()
            .copied()
            .collect::<Vec<_>>();
        let current_thread_id = self.current_thread_id();
        let current_index = thread_ids
            .iter()
            .position(|thread_id| *thread_id == current_thread_id)
            .ok_or_else(|| {
                RuntimeStateError::runtime_defect(
                    "runtime-scheduler",
                    "current-thread-scheduler-state-missing",
                    "current runtime thread has no scheduler lane",
                )
            })?;
        let mut preferred = None;
        for offset in 1..=thread_ids.len() {
            let thread_id = thread_ids[(current_index + offset) % thread_ids.len()];
            if (!include_current && thread_id == current_thread_id)
                || !self.scheduler.state(thread_id)?.is_executable()
            {
                continue;
            }
            let priority = self.threads.effective_priority(thread_id)?;
            if preferred.is_none_or(|(_, selected_priority): (_, RuntimeThreadPriority)| {
                priority.scheduling_rank() > selected_priority.scheduling_rank()
            }) {
                preferred = Some((thread_id, priority));
            }
        }
        Ok(preferred)
    }

    pub fn wait_current_thread_on(
        &mut self,
        handle: RuntimeHandle,
    ) -> Result<RuntimeWaitDisposition, RuntimeStateError> {
        if !self.process.is_running()
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
            let plan = self.synchronization.plan_mutex_wait(mutex_id, thread_id)?;
            let recursion_count = plan.recursion_count();
            let abandoned = plan.abandoned_acquisition();
            self.synchronization.commit_mutex_transition(plan);
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
            if kind == RuntimeObjectKind::Event
                && matches!(
                    object.key,
                    RuntimeObjectKey::Event {
                        manual_reset: false,
                        ..
                    }
                )
            {
                let updates = self.objects.plan_wait_state_updates(&[(
                    object.key.clone(),
                    RuntimeWaitState::Nonsignaled,
                )])?;
                self.objects.commit_wait_state_updates(updates);
            }
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
        if !self.process.is_running()
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
        let cursor = self.input.cursor_position().ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-message",
                "message-cursor-position-unconfigured",
                "message posting requires a configured logical cursor position for MSG metadata",
            )
        })?;
        let message_time = self.process.clock().tick_count;
        let point_x = cursor.screen_x;
        let point_y = cursor.screen_y;
        let target_thread = self.thread_state(target_thread_id)?;
        let target_scheduler_state = self.scheduler_state_for_thread(target_thread_id)?;
        if target_thread.lifecycle != RuntimeThreadLifecycleState::Running
            || !matches!(
                target_scheduler_state,
                SchedulerThreadState::Runnable
                    | SchedulerThreadState::WaitingHost { .. }
                    | SchedulerThreadState::WaitingObject { .. }
                    | SchedulerThreadState::WaitingCriticalSection { .. }
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
            RuntimeMessageSource::Thread {
                thread_id: source_thread_id,
            },
            target_thread_id,
            hwnd,
            message,
            w_param,
            l_param,
            message_time,
            point_x,
            point_y,
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
        self.messages.preflight_post(&plan)?;
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
        if !self.process.is_running()
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

    pub(crate) fn peek_current_thread_message(
        &mut self,
        memory: &mut GuestMemory,
        output: GuestAddress,
        remove: bool,
        window_timer_message: Option<u32>,
    ) -> Result<(RuntimeMessagePeekDisposition, Option<GuestWriteResult>), RuntimeStateError> {
        let scheduler_state = self.scheduler_state()?;
        if !self.process.is_running()
            || self.current_thread()?.lifecycle != RuntimeThreadLifecycleState::Running
            || !scheduler_state.is_executable()
        {
            return Err(RuntimeStateError::conflict(
                "runtime-message",
                "message-peek-thread-state-invalid",
                format!(
                    "message peek requires a running process and executable owner thread; current thread {} is {scheduler_state:?}",
                    self.current_thread_id().0
                ),
            ));
        }
        let thread_id = self.current_thread_id();
        let timer_clock = self.process.timer_clock_state();
        if let Some(window_timer_message) = window_timer_message
            && self.messages.is_empty(thread_id)?
            && let Some(timer_plan) = self
                .timers
                .plan_due_window_timer_delivery(thread_id, timer_clock)?
        {
            let timer = timer_plan.timer();
            let window = self.user32.window(timer.window_handle)?;
            if window.id != timer.window_id
                || window.lifecycle != RuntimeWindowLifecycle::Created
                || window.creation.owner_thread_id != timer.owner_thread_id
                || window_timer_message == 0
            {
                return Err(RuntimeStateError::conflict(
                    "runtime-window-timer",
                    "window-timer-delivery-target-stale",
                    "logical window timer target changed before message delivery",
                ));
            }
            let cursor = self.input.cursor_position().ok_or_else(|| {
                RuntimeStateError::runtime_defect(
                    "runtime-message",
                    "message-cursor-position-unconfigured",
                    "timer message delivery requires a configured logical cursor position",
                )
            })?;
            let post_plan = self.messages.plan_post(
                RuntimeMessageSource::WindowTimer,
                thread_id,
                timer.window_handle,
                window_timer_message,
                timer.timer_id,
                timer.callback_address.0,
                self.process.clock().tick_count,
                cursor.screen_x,
                cursor.screen_y,
                false,
            )?;
            let message = post_plan.message;
            let bytes = message.to_x86_msg_bytes();
            memory
                .validate_access(output, bytes.len(), MemoryAccess::Write)
                .map_err(RuntimeStateError::from_memory)?;
            let mut transaction = memory.begin_transaction();
            let write = transaction
                .write(output, &bytes)
                .map_err(RuntimeStateError::from_memory)?;
            self.timers
                .preflight_window_timer_delivery(&timer_plan, timer_clock)?;
            self.messages.preflight_post(&post_plan)?;
            let current_window = self.user32.window(timer.window_handle)?;
            if current_window.id != timer.window_id
                || current_window.lifecycle != RuntimeWindowLifecycle::Created
                || current_window.creation.owner_thread_id != timer.owner_thread_id
            {
                return Err(RuntimeStateError::conflict(
                    "runtime-window-timer",
                    "window-timer-delivery-target-stale",
                    "logical window timer target changed after delivery planning",
                ));
            }
            self.timers
                .commit_window_timer_delivery(timer_plan, timer_clock);
            self.messages.commit_post(post_plan);
            if remove {
                self.messages.commit_receive(thread_id, message.id);
            }
            transaction.commit_transaction();
            return Ok((
                RuntimeMessagePeekDisposition::Message {
                    message,
                    removed: remove,
                },
                Some(write),
            ));
        }
        let plan = self.messages.plan_peek(thread_id, remove)?;
        let RuntimeMessagePeekPlan::Message { message, .. } = plan else {
            self.messages.preflight_peek(&plan)?;
            return Ok((self.messages.commit_preflighted_peek(plan), None));
        };
        let bytes = message.to_x86_msg_bytes();
        memory
            .validate_access(output, bytes.len(), MemoryAccess::Write)
            .map_err(RuntimeStateError::from_memory)?;
        let mut transaction = memory.begin_transaction();
        let write = transaction
            .write(output, &bytes)
            .map_err(RuntimeStateError::from_memory)?;
        self.messages.preflight_peek(&plan)?;
        let disposition = self.messages.commit_preflighted_peek(plan);
        transaction.commit_transaction();
        Ok((disposition, Some(write)))
    }

    pub fn queued_thread_messages(
        &self,
        thread_id: RuntimeThreadId,
    ) -> Result<Vec<RuntimeThreadMessage>, RuntimeStateError> {
        self.thread_state(thread_id)?;
        self.messages.snapshot(thread_id)
    }

    pub fn set_heap_termination_on_corruption(&mut self, enabled: bool) {
        self.process.set_heap_termination_on_corruption(enabled);
    }

    pub fn ensure_standard_handle(
        &mut self,
        selector: u32,
    ) -> Result<&RuntimeStandardHandle, RuntimeStateError> {
        if self.current_thread()?.lifecycle != RuntimeThreadLifecycleState::Running {
            return Err(RuntimeStateError::conflict(
                "runtime-standard-handle",
                "standard-handle-lifecycle-state-invalid",
                "standard handle lookup requires a running thread",
            ));
        }
        self.process.ensure_standard_handle(selector)
    }

    pub fn standard_handle_by_value(
        &self,
        handle: u32,
    ) -> Result<&RuntimeStandardHandle, RuntimeStateError> {
        self.process.standard_handle_by_value(handle)
    }

    pub fn create_heap(&mut self, heap: RuntimeHeap) -> Result<&RuntimeHeap, RuntimeStateError> {
        let handle = self.commit_heap_creation(heap)?;
        Ok(self
            .heaps
            .heap(handle)
            .expect("committed runtime heap disappeared"))
    }

    fn commit_heap_creation(&mut self, heap: RuntimeHeap) -> Result<u32, RuntimeStateError> {
        let plan = self.heaps.plan_creation(heap)?;
        self.commit_heap_creation_plan(plan)
    }

    fn commit_heap_creation_plan(
        &mut self,
        plan: RuntimeHeapCreationPlan,
    ) -> Result<u32, RuntimeStateError> {
        let handle = plan.handle();
        self.objects.register_object_with_handle(
            RuntimeObjectKey::PrivateHeap { handle },
            RuntimeHandle(handle),
        )?;
        self.heaps.commit_creation(plan);
        Ok(handle)
    }

    pub fn allocate_private_heap(
        &mut self,
        creation_flags: u32,
        initial_size: u32,
        maximum_size: u32,
    ) -> Result<&RuntimeHeap, RuntimeStateError> {
        let plan = self
            .heaps
            .plan_private_creation(creation_flags, initial_size, maximum_size)?;
        let handle = self.commit_heap_creation_plan(plan)?;
        Ok(self
            .heaps
            .heap(handle)
            .expect("committed private heap disappeared"))
    }

    pub fn heap(&self, handle: u32) -> Result<&RuntimeHeap, RuntimeStateError> {
        self.objects
            .object_for_handle(RuntimeHandle(handle), RuntimeObjectKind::PrivateHeap)?;
        self.heaps.heap(handle)
    }

    pub fn heap_backing(
        &self,
        handle: u32,
    ) -> Result<Option<&RuntimeHeapBacking>, RuntimeStateError> {
        self.heap(handle)?;
        Ok(self.heaps.backing(handle))
    }

    pub fn heap_extension_backing(
        &self,
        handle: u32,
    ) -> Result<Option<&RuntimeHeapBacking>, RuntimeStateError> {
        self.heap(handle)?;
        Ok(self.heaps.extension_backing(handle))
    }

    pub fn large_heap_allocation_backing(&self) -> Option<&RuntimeLargeHeapAllocationBacking> {
        self.heaps.large_backing()
    }

    pub fn private_heap_allocation(
        &self,
        handle: u32,
        address: GuestAddress,
    ) -> Result<RuntimeHeapAllocation, RuntimeStateError> {
        self.require_private_heap_operation_state()?;
        self.heap(handle)?;
        self.heaps.allocation(handle, address)
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
        self.heaps.plan_allocation(
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
        self.heaps.commit_allocation(plan, new_memory_owner)
    }

    pub(crate) fn plan_private_heap_free(
        &self,
        handle: u32,
        address: GuestAddress,
    ) -> Result<RuntimeHeapFreePlan, RuntimeStateError> {
        self.require_private_heap_operation_state()?;
        self.heap(handle)?;
        self.heaps.plan_free(handle, address)
    }

    pub(crate) fn commit_private_heap_free(
        &mut self,
        plan: RuntimeHeapFreePlan,
    ) -> Result<RuntimeHeapAllocation, RuntimeStateError> {
        self.heaps.commit_free(plan)
    }

    pub fn materialize_private_heap_page(
        &mut self,
        memory: &mut GuestMemory,
        fault_address: GuestAddress,
        page_fault_error_code: u32,
    ) -> Result<RuntimeHeapPageMaterialization, RuntimeStateError> {
        self.require_private_heap_operation_state()?;
        if page_fault_error_code & 1 != 0 {
            return Err(RuntimeStateError::invalid_input(
                "runtime-heap-backing",
                "large-heap-protection-fault",
                "present-page protection faults cannot materialize a lazy heap page",
            ));
        }
        let stats = memory.stats();
        if stats.generation != stats.acknowledged_generation || stats.pending_invalidation.is_some()
        {
            return Err(RuntimeStateError::conflict(
                "runtime-heap-backing",
                "large-heap-materialization-memory-unsynchronized",
                "large heap page materialization requires fully acknowledged guest memory",
            ));
        }
        let plan = self.heaps.plan_page_materialization(
            &self.address_space_allocator,
            fault_address,
            page_fault_error_code,
        )?;
        let region_matches = memory.region_snapshots().iter().any(|region| {
            region.owner == plan.memory_owner
                && region.kind == MemoryRegionKind::Heap
                && region.range.start() == plan.allocation.address
                && region.range.byte_length() == u64::from(plan.allocation.reserved_byte_length)
        });
        if !region_matches {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-heap-backing",
                "large-heap-memory-region-mismatch",
                "large heap metadata does not match the Rust guest-memory reservation",
            ));
        }
        memory
            .preflight_growth(0, 0, 1, "runtime-heap-backing")
            .map_err(RuntimeStateError::from_memory)?;

        let mut transaction = memory.begin_transaction();
        transaction
            .commit(
                plan.memory_owner,
                plan.committed_page,
                PageProtection {
                    read: true,
                    write: true,
                    execute: false,
                },
            )
            .map_err(RuntimeStateError::from_memory)?;
        let fill_byte = plan.allocation.initialization.fill_byte();
        if fill_byte != 0 {
            let bytes = vec![fill_byte; GUEST_PAGE_SIZE as usize];
            transaction
                .write(plan.committed_page.start(), &bytes)
                .map_err(RuntimeStateError::from_memory)?;
        }
        let materialization = self.heaps.commit_page_materialization(plan)?;
        transaction.commit_transaction();
        Ok(materialization)
    }

    pub(crate) fn plan_private_heap_destruction(
        &self,
        handle: u32,
    ) -> Result<RuntimeHeapDestructionPlan, RuntimeStateError> {
        self.require_private_heap_operation_state()?;
        self.heap(handle)?;
        self.heaps
            .plan_destruction(&self.address_space_allocator, handle)
    }

    pub(crate) fn plan_process_heap_teardown(
        &self,
    ) -> Result<Vec<RuntimeHeapDestructionPlan>, RuntimeStateError> {
        if !matches!(
            self.process.lifecycle(),
            RuntimeProcessLifecycleState::ExitRequested { .. }
        ) {
            return Err(RuntimeStateError::conflict(
                "runtime-heap-backing",
                "process-heap-teardown-state-invalid",
                "process heap teardown requires an active process exit request",
            ));
        }
        for handle in self.heaps.handles() {
            self.objects
                .object_for_handle(RuntimeHandle(handle), RuntimeObjectKind::PrivateHeap)?;
        }
        self.heaps
            .plan_all_destructions(&self.address_space_allocator)
    }

    pub(crate) fn commit_private_heap_destruction(
        &mut self,
        plans: &[RuntimeHeapDestructionPlan],
    ) -> Result<RuntimeHeapTeardown, RuntimeStateError> {
        let teardown = self.heaps.preflight_destruction(plans)?;
        let keys: Vec<_> = teardown
            .handles
            .iter()
            .map(|handle| RuntimeObjectKey::PrivateHeap { handle: *handle })
            .collect();
        self.objects.retire_subsystem_objects(&keys)?;
        self.heaps.commit_destruction(&teardown);
        Ok(teardown)
    }

    fn require_private_heap_operation_state(&self) -> Result<(), RuntimeStateError> {
        if !self.process.is_running()
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

    pub fn load_current_thread_runtime_system_module(
        &mut self,
        requested_name: String,
        canonical_name: String,
    ) -> Result<RuntimeSystemModuleLoad, RuntimeStateError> {
        if !self.process.is_running()
            || self.current_thread()?.lifecycle != RuntimeThreadLifecycleState::Running
        {
            return Err(RuntimeStateError::conflict(
                "runtime-module-loader",
                "runtime-system-module-load-thread-state-invalid",
                "dynamic runtime-system loading requires a running process and current thread",
            ));
        }
        let plan = self.modules.plan_runtime_system_module_load(
            &self.module_records,
            requested_name,
            canonical_name,
            self.current_thread_id().0,
        )?;
        self.modules
            .validate_runtime_system_module_load_plan(&self.module_records, &plan)?;
        if let Some(handle) = plan.new_module_handle() {
            self.objects.register_object_with_handle(
                RuntimeObjectKey::Module { handle },
                RuntimeHandle(handle),
            )?;
        } else {
            self.objects.object_for_handle(
                RuntimeHandle(plan.result_handle()),
                RuntimeObjectKind::Module,
            )?;
        }
        Ok(self
            .modules
            .commit_runtime_system_module_load(&mut self.module_records, plan))
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
            .plan_registration(&self.module_records, module, class)?;
        self.objects.register_object_with_handle(
            RuntimeObjectKey::Module { handle },
            RuntimeHandle(handle),
        )?;
        self.modules
            .commit_registration(&mut self.module_records, plan);
        Ok(self
            .module_records
            .module(handle)
            .expect("committed runtime module disappeared"))
    }

    pub fn module(&self, handle: u32) -> Result<&RuntimeModule, RuntimeStateError> {
        self.objects
            .object_for_handle(RuntimeHandle(handle), RuntimeObjectKind::Module)?;
        self.module_records.module(handle)
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
        let plan = self.modules.plan_process_detach(return_sentinel)?;
        self.require_dll_process_detach_thread_state(plan.callback_entry().is_some())?;
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
        if !self.process.is_running()
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

    fn require_dll_process_detach_thread_state(
        &self,
        requires_callback: bool,
    ) -> Result<(), RuntimeStateError> {
        if !self.process.is_running()
            || self.current_thread()?.lifecycle != RuntimeThreadLifecycleState::Running
            || (requires_callback
                && !matches!(
                    self.scheduler_state()?,
                    SchedulerThreadState::Runnable | SchedulerThreadState::InCallback { .. }
                ))
        {
            return Err(RuntimeStateError::conflict(
                "runtime-dll-lifecycle",
                "dll-callback-thread-state-invalid",
                "DLL process detach requires a running process and an active selected thread when guest callbacks are pending",
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
                Some(self.plan_callback_push(target, return_sentinel, 0)?)
            }
            (Some(expected), Some((target, return_sentinel))) => {
                Some(self.plan_callback_replace(expected, target, return_sentinel, 0)?)
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
        let plan = self
            .registries
            .plan_dynamic_endpoint(self.process.address_space().dynamic_exports, endpoint)?;
        Ok(self.registries.commit_dynamic_endpoint(plan))
    }

    pub fn register_dynamic_export_thunk(
        &mut self,
        thunk: &DynamicExportThunk,
    ) -> Result<&RuntimeDynamicEndpoint, RuntimeStateError> {
        self.register_dynamic_endpoint(RuntimeDynamicEndpoint::from(thunk))
    }

    pub fn dynamic_endpoint(&self, id: u32) -> Result<&RuntimeDynamicEndpoint, RuntimeStateError> {
        self.registries.dynamic_endpoint(id)
    }

    pub fn dynamic_endpoint_at(
        &self,
        address: GuestAddress,
    ) -> Result<&RuntimeDynamicEndpoint, RuntimeStateError> {
        self.registries.dynamic_endpoint_at(address)
    }

    pub fn dynamic_endpoint_by_name(
        &self,
        module: &str,
        name: &str,
    ) -> Result<&RuntimeDynamicEndpoint, RuntimeStateError> {
        self.registries.dynamic_endpoint_by_name(module, name)
    }

    pub fn allocate_tls_index(&mut self) -> Result<u32, RuntimeStateError> {
        let plan = self.threads.plan_tls_index_allocation()?;
        Ok(self.threads.commit_tls_index_allocation(plan))
    }

    pub fn allocate_fls_slot(
        &mut self,
        callback: Option<GuestAddress>,
    ) -> Result<&RuntimeFlsSlot, RuntimeStateError> {
        if !self.process.is_running()
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
        let slot_plan = self.threads.plan_fls_slot_allocation(callback)?;
        let thread_plan = self
            .threads
            .plan_initialize_fls_storage(self.current_thread_id())?;
        Ok(self
            .threads
            .commit_fls_slot_allocation(slot_plan, thread_plan))
    }

    pub fn fls_slot(&self, index: u32) -> Result<&RuntimeFlsSlot, RuntimeStateError> {
        self.threads.fls_slot(index)
    }

    pub fn set_fls_value(
        &mut self,
        index: u32,
        value: u32,
    ) -> Result<Option<u32>, RuntimeStateError> {
        if !self.process.is_running()
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
        self.threads.fls_slot(index)?;
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
        self.threads.require_tls_index(index)?;
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
        self.threads.require_tls_index(index)?;
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
        let plan =
            self.synchronization
                .plan_mutex_creation(name_a, initial_owner, current_thread_id)?;
        let mutex_id = plan.id();
        let already_exists = plan.already_exists();
        let key = RuntimeObjectKey::Mutex {
            mutex_id: mutex_id.0,
        };
        let object_plan = self
            .objects
            .plan_close_handle_object_open(key, already_exists)?;
        let handle = object_plan.opened_handle();
        self.objects
            .commit_close_handle_object_mutation(object_plan)?;
        self.synchronization.commit_mutex_creation(plan);
        Ok(RuntimeCreateMutexAResult {
            mutex_id,
            handle,
            already_exists,
        })
    }

    pub fn create_event(
        &mut self,
        manual_reset: bool,
        initial_state: bool,
    ) -> Result<(RuntimeHandle, RuntimeEvent), RuntimeStateError> {
        self.require_event_operation_state()?;
        let plan = self
            .objects
            .plan_event_and_handle_registration(manual_reset, initial_state)?;
        self.objects.preflight_object_registration_commit(&plan)?;
        let handle = plan.handle();
        self.objects.commit_object_registration(plan);
        Ok((handle, self.event_for_handle(handle)?))
    }

    pub fn event_for_handle(
        &self,
        handle: RuntimeHandle,
    ) -> Result<RuntimeEvent, RuntimeStateError> {
        let record = self
            .objects
            .object_for_handle(handle, RuntimeObjectKind::Event)?;
        let RuntimeObjectKey::Event {
            event_id,
            manual_reset,
        } = &record.key
        else {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-event",
                "event-object-key-invalid",
                "event handle resolved to a different object identity",
            ));
        };
        let wait_state = record.wait_state.ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-event",
                "event-object-not-waitable",
                "event object has no wait state",
            )
        })?;
        Ok(RuntimeEvent {
            event_id: *event_id,
            manual_reset: *manual_reset,
            wait_state,
        })
    }

    pub fn set_event(&mut self, handle: RuntimeHandle) -> Result<RuntimeEvent, RuntimeStateError> {
        self.require_event_operation_state()?;
        let record = self
            .objects
            .object_for_handle(handle, RuntimeObjectKind::Event)?
            .clone();
        let RuntimeObjectKey::Event { manual_reset, .. } = &record.key else {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-event",
                "event-object-key-invalid",
                "event handle resolved to a different object identity",
            ));
        };
        let signaled = self.objects.plan_wait_state_updates(&[(
            record.key.clone(),
            RuntimeWaitState::Signaled { exit_code: 0 },
        )])?;
        let mut wakes = self.plan_waiter_wakes(&signaled, &[])?;
        if !*manual_reset {
            wakes.truncate(1);
        }
        let final_state = if !*manual_reset && !wakes.is_empty() {
            self.objects
                .plan_wait_state_updates(&[(record.key, RuntimeWaitState::Nonsignaled)])?
        } else {
            signaled
        };
        self.objects.commit_wait_state_updates(final_state);
        self.commit_waiter_wakes(wakes);
        self.event_for_handle(handle)
    }

    pub fn reset_event(
        &mut self,
        handle: RuntimeHandle,
    ) -> Result<RuntimeEvent, RuntimeStateError> {
        self.require_event_operation_state()?;
        let record = self
            .objects
            .object_for_handle(handle, RuntimeObjectKind::Event)?
            .clone();
        let updates = self
            .objects
            .plan_wait_state_updates(&[(record.key, RuntimeWaitState::Nonsignaled)])?;
        self.objects.commit_wait_state_updates(updates);
        self.event_for_handle(handle)
    }

    pub fn try_wait_event(&mut self, handle: RuntimeHandle) -> Result<bool, RuntimeStateError> {
        self.require_event_operation_state()?;
        let event = self.event_for_handle(handle)?;
        if !matches!(event.wait_state, RuntimeWaitState::Signaled { .. }) {
            return Ok(false);
        }
        if !event.manual_reset {
            let record = self
                .objects
                .object_for_handle(handle, RuntimeObjectKind::Event)?
                .clone();
            let updates = self
                .objects
                .plan_wait_state_updates(&[(record.key, RuntimeWaitState::Nonsignaled)])?;
            self.objects.commit_wait_state_updates(updates);
        }
        Ok(true)
    }

    pub fn create_waitable_timer_a(
        &mut self,
        manual_reset: bool,
    ) -> Result<RuntimeCreateWaitableTimerAResult, RuntimeStateError> {
        self.require_waitable_timer_operation_state()?;
        let plan = self.timers.plan_waitable_timer_creation(manual_reset)?;
        let timer_id = plan.id();
        let key = RuntimeObjectKey::WaitableTimer {
            timer_id: timer_id.0,
        };
        let object_plan = self.objects.plan_close_handle_object_open(key, false)?;
        self.timers.preflight_waitable_timer_creation(&plan)?;
        let handle = object_plan.opened_handle();
        self.objects
            .commit_close_handle_object_mutation(object_plan)?;
        self.timers.commit_waitable_timer_creation(plan);
        Ok(RuntimeCreateWaitableTimerAResult { timer_id, handle })
    }

    pub fn waitable_timer_for_handle(
        &self,
        handle: RuntimeHandle,
    ) -> Result<&RuntimeWaitableTimer, RuntimeStateError> {
        let record = self
            .objects
            .object_for_handle(handle, RuntimeObjectKind::WaitableTimer)?;
        let RuntimeObjectKey::WaitableTimer { timer_id } = &record.key else {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-waitable-timer",
                "waitable-timer-object-key-invalid",
                "waitable timer handle resolved to a different object identity",
            ));
        };
        self.timers
            .waitable_timer(RuntimeWaitableTimerId(*timer_id))
    }

    pub fn set_waitable_timer(
        &mut self,
        handle: RuntimeHandle,
        relative_due_time_100ns: u64,
    ) -> Result<&RuntimeWaitableTimer, RuntimeStateError> {
        self.require_waitable_timer_operation_state()?;
        let record = self
            .objects
            .object_for_handle(handle, RuntimeObjectKind::WaitableTimer)?
            .clone();
        let RuntimeObjectKey::WaitableTimer { timer_id } = record.key else {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-waitable-timer",
                "waitable-timer-object-key-invalid",
                "waitable timer handle resolved to a different object identity",
            ));
        };
        let timer_id = RuntimeWaitableTimerId(timer_id);
        if self
            .scheduler
            .object_waiters()
            .get(&record.id)
            .is_some_and(|waiters| !waiters.is_empty())
        {
            return Err(RuntimeStateError::conflict(
                "runtime-waitable-timer",
                "waitable-timer-rearm-with-pending-wait-unsupported",
                "bounded waitable timer pacing does not rearm a timer with a pending wait",
            ));
        }
        let timer_clock = self.process.timer_clock_state();
        let timer_plan = self.timers.plan_waitable_timer_one_shot_arm(
            timer_id,
            relative_due_time_100ns,
            timer_clock,
        )?;
        let wait_updates = self.objects.plan_wait_state_updates(&[(
            RuntimeObjectKey::WaitableTimer {
                timer_id: timer_id.0,
            },
            RuntimeWaitState::Nonsignaled,
        )])?;
        self.timers
            .preflight_waitable_timer_arm(&timer_plan, timer_clock)?;
        self.objects.commit_wait_state_updates(wait_updates);
        Ok(self
            .timers
            .commit_waitable_timer_arm(timer_plan, timer_clock))
    }

    pub fn begin_wait_for_single_object_ex(
        &mut self,
        handle: RuntimeHandle,
        timeout_milliseconds: u32,
        alertable: bool,
    ) -> Result<RuntimeWaitForSingleObjectExBegin, RuntimeStateError> {
        self.require_waitable_timer_operation_state()?;
        let timer_clock_before = self.process.timer_clock_state();
        let object = self
            .objects
            .object_for_handle(handle, RuntimeObjectKind::WaitableTimer)?
            .clone();
        let RuntimeObjectKey::WaitableTimer { timer_id } = object.key else {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-waitable-timer",
                "waitable-timer-object-key-invalid",
                "waitable timer handle resolved to a different object identity",
            ));
        };
        if self
            .scheduler
            .object_waiters()
            .get(&object.id)
            .is_some_and(|waiters| !waiters.is_empty())
        {
            return Err(RuntimeStateError::conflict(
                "runtime-wait",
                "waitable-timer-existing-waiters-unsupported",
                "bounded waitable timer pacing does not interleave existing object waiters",
            ));
        }
        let timer_id = RuntimeWaitableTimerId(timer_id);
        let timer = *self.timers.waitable_timer(timer_id)?;
        let object_wait_state = object.wait_state.ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-waitable-timer",
                "waitable-timer-object-not-waitable",
                "waitable timer object has no wait state",
            )
        })?;
        if matches!(object_wait_state, RuntimeWaitState::Signaled { .. }) {
            return self
                .wait_for_single_object_ex(handle, timeout_milliseconds, alertable)
                .map(|result| RuntimeWaitForSingleObjectExBegin::Immediate { result });
        }

        let timeout_100ns = u64::from(timeout_milliseconds)
            .checked_mul(10_000)
            .ok_or_else(|| {
                RuntimeStateError::resource_limit(
                    "runtime-wait",
                    "wait-timeout-conversion-overflow",
                    "wait timeout exceeds the logical timer clock unit range",
                )
            })?;
        let timeout_deadline_time_100ns = timer_clock_before
            .current_time_100ns
            .checked_add(timeout_100ns)
            .ok_or_else(|| {
                RuntimeStateError::resource_limit(
                    "runtime-wait",
                    "wait-timeout-deadline-overflow",
                    "wait timeout deadline exceeds the logical timer clock",
                )
            })?;
        let expiration_arm = timer
            .arm
            .filter(|arm| arm.deadline_time_100ns <= timeout_deadline_time_100ns);
        let target_time_100ns = expiration_arm.map_or(timeout_deadline_time_100ns, |arm| {
            arm.deadline_time_100ns
                .max(timer_clock_before.current_time_100ns)
        });
        if target_time_100ns == timer_clock_before.current_time_100ns {
            return self
                .wait_for_single_object_ex(handle, timeout_milliseconds, alertable)
                .map(|result| RuntimeWaitForSingleObjectExBegin::Immediate { result });
        }

        let delay_100ns = target_time_100ns - timer_clock_before.current_time_100ns;
        let delay_milliseconds = u32::try_from(delay_100ns.div_ceil(10_000)).map_err(|_| {
            RuntimeStateError::resource_limit(
                "runtime-wait",
                "wait-pacing-delay-not-representable",
                "waitable timer pacing delay exceeds the bounded browser transport",
            )
        })?;
        let host_plan: RuntimeHostRequestPlan = self
            .scheduler
            .plan_host_request(RUNTIME_WAITABLE_TIMER_PACING_SERVICE.to_owned())?;
        let pacing = RuntimeWaitableTimerPacing {
            request_id: host_plan.request().request_id,
            delay_milliseconds,
            target_time_100ns,
            timer_clock_before,
            timer_before: timer,
            outcome: if expiration_arm.is_some() {
                RuntimeWaitForSingleObjectExOutcome::ObjectSignaled
            } else {
                RuntimeWaitForSingleObjectExOutcome::TimedOut
            },
        };
        let wait_plan: RuntimeWaitRegistrationPlan = self.scheduler.plan_host_wait_registration(
            self.current_thread_id(),
            object.id,
            handle,
            timeout_deadline_time_100ns,
            alertable,
            pacing,
        )?;

        self.scheduler.preflight_host_request(&host_plan)?;
        self.scheduler.preflight_wait_registration(&wait_plan)?;
        if self.process.timer_clock_state() != timer_clock_before
            || self.timers.waitable_timer(timer_id)? != &timer
            || self
                .objects
                .object_for_handle(handle, RuntimeObjectKind::WaitableTimer)?
                != &object
        {
            return Err(RuntimeStateError::conflict(
                "runtime-waitable-timer",
                "waitable-timer-pacing-begin-plan-stale",
                "timer, object, or logical clock changed after pacing request planning",
            ));
        }

        self.scheduler.commit_wait_registration(wait_plan);
        let host_request = self.scheduler.commit_host_request(host_plan);
        let registration = self
            .scheduler
            .wait_registration_for_thread(host_request.thread_id)?
            .ok_or_else(|| {
                RuntimeStateError::runtime_defect(
                    "runtime-wait",
                    "waitable-timer-pacing-registration-missing",
                    "committed timer pacing host request has no wait registration",
                )
            })?;
        debug_assert_eq!(
            registration
                .waitable_timer_pacing
                .map(|pending| pending.request_id),
            Some(host_request.request_id)
        );
        Ok(RuntimeWaitForSingleObjectExBegin::Deferred { registration })
    }

    pub fn wait_for_single_object_ex(
        &mut self,
        handle: RuntimeHandle,
        timeout_milliseconds: u32,
        alertable: bool,
    ) -> Result<RuntimeWaitForSingleObjectExResult, RuntimeStateError> {
        self.require_waitable_timer_operation_state()?;
        let timer_clock_before = self.process.timer_clock_state();
        let object = self
            .objects
            .object_for_handle(handle, RuntimeObjectKind::WaitableTimer)?
            .clone();
        let RuntimeObjectKey::WaitableTimer { timer_id } = object.key else {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-waitable-timer",
                "waitable-timer-object-key-invalid",
                "waitable timer handle resolved to a different object identity",
            ));
        };
        if self
            .scheduler
            .object_waiters()
            .get(&object.id)
            .is_some_and(|waiters| !waiters.is_empty())
        {
            return Err(RuntimeStateError::conflict(
                "runtime-wait",
                "waitable-timer-existing-waiters-unsupported",
                "bounded waitable timer completion does not interleave existing object waiters",
            ));
        }
        let timer_id = RuntimeWaitableTimerId(timer_id);
        let timer = *self.timers.waitable_timer(timer_id)?;
        let object_wait_state = object.wait_state.ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-waitable-timer",
                "waitable-timer-object-not-waitable",
                "waitable timer object has no wait state",
            )
        })?;
        if matches!(object_wait_state, RuntimeWaitState::Signaled { .. }) {
            if timer.arm.is_some() {
                return Err(RuntimeStateError::runtime_defect(
                    "runtime-waitable-timer",
                    "waitable-timer-signaled-with-live-arm",
                    "signaled waitable timer retains a live one-shot arm",
                ));
            }
            let final_wait_state = if timer.manual_reset {
                object_wait_state
            } else {
                let wait_updates = self.objects.plan_wait_state_updates(&[(
                    RuntimeObjectKey::WaitableTimer {
                        timer_id: timer_id.0,
                    },
                    RuntimeWaitState::Nonsignaled,
                )])?;
                self.objects.commit_wait_state_updates(wait_updates);
                RuntimeWaitState::Nonsignaled
            };
            return Ok(RuntimeWaitForSingleObjectExResult {
                outcome: RuntimeWaitForSingleObjectExOutcome::ObjectSignaled,
                registration: None,
                object_id: object.id,
                object_wait_state: final_wait_state,
                timer,
                timer_clock_before,
                timer_clock_after: timer_clock_before,
            });
        }

        let timeout_100ns = u64::from(timeout_milliseconds)
            .checked_mul(10_000)
            .ok_or_else(|| {
                RuntimeStateError::resource_limit(
                    "runtime-wait",
                    "wait-timeout-conversion-overflow",
                    "wait timeout exceeds the logical timer clock unit range",
                )
            })?;
        let timeout_deadline_time_100ns = timer_clock_before
            .current_time_100ns
            .checked_add(timeout_100ns)
            .ok_or_else(|| {
                RuntimeStateError::resource_limit(
                    "runtime-wait",
                    "wait-timeout-deadline-overflow",
                    "wait timeout deadline exceeds the logical timer clock",
                )
            })?;
        let expiration_arm = timer
            .arm
            .filter(|arm| arm.deadline_time_100ns <= timeout_deadline_time_100ns);
        let target_time_100ns = expiration_arm.map_or(timeout_deadline_time_100ns, |arm| {
            arm.deadline_time_100ns
                .max(timer_clock_before.current_time_100ns)
        });
        let clock_plan = self.process.plan_clock_advance(target_time_100ns)?;
        let wait_plan = (target_time_100ns > timer_clock_before.current_time_100ns)
            .then(|| {
                self.scheduler.plan_immediate_wait(
                    self.current_thread_id(),
                    object.id,
                    handle,
                    timeout_deadline_time_100ns,
                    alertable,
                )
            })
            .transpose()?;
        let expiration_plan = expiration_arm
            .map(|arm| {
                self.timers
                    .plan_waitable_timer_expiration(timer_id, arm.generation)
            })
            .transpose()?;
        let next_object_wait_state = if expiration_arm.is_some() && timer.manual_reset {
            RuntimeWaitState::Signaled { exit_code: 0 }
        } else {
            RuntimeWaitState::Nonsignaled
        };
        let wait_updates = expiration_arm
            .map(|_| {
                self.objects.plan_wait_state_updates(&[(
                    RuntimeObjectKey::WaitableTimer {
                        timer_id: timer_id.0,
                    },
                    next_object_wait_state,
                )])
            })
            .transpose()?;

        self.process.preflight_clock_advance(&clock_plan)?;
        if let Some(plan) = expiration_plan.as_ref() {
            self.timers.preflight_waitable_timer_expiration(plan)?;
        }
        if let Some(plan) = wait_plan.as_ref() {
            self.scheduler.preflight_immediate_wait(plan)?;
        }
        if self
            .objects
            .object_for_handle(handle, RuntimeObjectKind::WaitableTimer)?
            != &object
        {
            return Err(RuntimeStateError::conflict(
                "runtime-waitable-timer",
                "waitable-timer-wait-object-plan-stale",
                "waitable timer object changed after wait planning",
            ));
        }

        let registration = wait_plan.map(|plan| self.scheduler.commit_immediate_wait_begin(plan));
        let timer_clock_after = self
            .process
            .commit_clock_advance(clock_plan)
            .expect("preflighted logical clock advance");
        let timer_after = expiration_plan.map_or(timer, |plan| {
            *self.timers.commit_waitable_timer_expiration(plan)
        });
        if let Some(updates) = wait_updates {
            self.objects.commit_wait_state_updates(updates);
        }
        if let Some(plan) = wait_plan {
            let completed = self.scheduler.commit_immediate_wait_complete(plan);
            debug_assert_eq!(registration, Some(completed));
        }

        Ok(RuntimeWaitForSingleObjectExResult {
            outcome: if expiration_arm.is_some() {
                RuntimeWaitForSingleObjectExOutcome::ObjectSignaled
            } else {
                RuntimeWaitForSingleObjectExOutcome::TimedOut
            },
            registration,
            object_id: object.id,
            object_wait_state: next_object_wait_state,
            timer: timer_after,
            timer_clock_before,
            timer_clock_after,
        })
    }

    pub(crate) fn plan_waitable_timer_pacing_completion(
        &self,
        request_id: u64,
    ) -> Result<RuntimeWaitableTimerPacingCompletionPlan, RuntimeStateError> {
        let scheduler = self.scheduler.plan_host_completion(request_id)?;
        if scheduler.request().service != RUNTIME_WAITABLE_TIMER_PACING_SERVICE {
            return Err(RuntimeStateError::invalid_input(
                "runtime-wait",
                "waitable-timer-pacing-host-service-mismatch",
                "waitable timer pacing completion belongs to another host service",
            ));
        }
        let registration = self
            .scheduler
            .wait_registration_for_thread(scheduler.request().thread_id)?
            .ok_or_else(|| {
                RuntimeStateError::runtime_defect(
                    "runtime-wait",
                    "waitable-timer-pacing-registration-missing",
                    "timer pacing host request has no wait registration",
                )
            })?;
        let pacing = registration.waitable_timer_pacing.ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-wait",
                "waitable-timer-pacing-contract-missing",
                "timer pacing wait registration has no pacing contract",
            )
        })?;
        if pacing.request_id != request_id {
            return Err(RuntimeStateError::conflict(
                "runtime-wait",
                "waitable-timer-pacing-request-mismatch",
                "timer pacing wait registration belongs to another host request",
            ));
        }
        if pacing.target_time_100ns <= pacing.timer_clock_before.current_time_100ns
            || pacing.delay_milliseconds == 0
        {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-wait",
                "waitable-timer-pacing-delay-invalid",
                "pending timer pacing wait must advance logical time by a positive delay",
            ));
        }
        let object = self
            .objects
            .object_for_handle(registration.handle, RuntimeObjectKind::WaitableTimer)?
            .clone();
        if object.id != registration.object_id
            || object.wait_state != Some(RuntimeWaitState::Nonsignaled)
        {
            return Err(RuntimeStateError::conflict(
                "runtime-wait",
                "waitable-timer-pacing-object-stale",
                "timer pacing object identity or wait state changed before completion",
            ));
        }
        let RuntimeObjectKey::WaitableTimer { timer_id } = object.key else {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-waitable-timer",
                "waitable-timer-object-key-invalid",
                "waitable timer pacing handle resolved to another object identity",
            ));
        };
        let timer_id = RuntimeWaitableTimerId(timer_id);
        if timer_id != pacing.timer_before.id
            || self.timers.waitable_timer(timer_id)? != &pacing.timer_before
        {
            return Err(RuntimeStateError::conflict(
                "runtime-wait",
                "waitable-timer-pacing-generation-stale",
                "timer arm changed before pacing completion",
            ));
        }
        let completion_clock_before = self.process.timer_clock_state();
        if completion_clock_before.current_time_100ns < pacing.timer_clock_before.current_time_100ns
        {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-wait",
                "waitable-timer-pacing-clock-regressed",
                "process-wide logical clock regressed while timer pacing was pending",
            ));
        }

        let timeout_deadline_time_100ns =
            registration.timeout_deadline_time_100ns.ok_or_else(|| {
                RuntimeStateError::runtime_defect(
                    "runtime-wait",
                    "waitable-timer-pacing-timeout-missing",
                    "timer pacing wait registration has no timeout deadline",
                )
            })?;
        let expiration_arm = match pacing.outcome {
            RuntimeWaitForSingleObjectExOutcome::ObjectSignaled => {
                let arm = pacing.timer_before.arm.ok_or_else(|| {
                    RuntimeStateError::conflict(
                        "runtime-wait",
                        "waitable-timer-pacing-arm-missing",
                        "signaled timer pacing completion has no matching live arm",
                    )
                })?;
                if arm.deadline_time_100ns != pacing.target_time_100ns
                    || arm.deadline_time_100ns > timeout_deadline_time_100ns
                {
                    return Err(RuntimeStateError::conflict(
                        "runtime-wait",
                        "waitable-timer-pacing-arm-deadline-mismatch",
                        "timer arm deadline does not match the pending pacing target",
                    ));
                }
                Some(arm)
            }
            RuntimeWaitForSingleObjectExOutcome::TimedOut => {
                if pacing.target_time_100ns != timeout_deadline_time_100ns
                    || pacing
                        .timer_before
                        .arm
                        .is_some_and(|arm| arm.deadline_time_100ns <= timeout_deadline_time_100ns)
                {
                    return Err(RuntimeStateError::conflict(
                        "runtime-wait",
                        "waitable-timer-pacing-timeout-mismatch",
                        "timer pacing timeout no longer precedes the live arm deadline",
                    ));
                }
                None
            }
        };
        // Other runnable guest threads continue to advance the process-wide
        // scheduled clock while this thread is waiting for the browser pacing
        // acknowledgement. A late acknowledgement therefore completes at the
        // current clock without moving it backwards; the timer arm identity,
        // rather than the clock generation captured at wait entry, protects
        // the deferred wait from a stale rearm.
        let clock = self.process.plan_clock_advance(
            completion_clock_before
                .current_time_100ns
                .max(pacing.target_time_100ns),
        )?;
        let expiration = expiration_arm
            .map(|arm| {
                self.timers
                    .plan_waitable_timer_expiration(timer_id, arm.generation)
            })
            .transpose()?;
        let object_wait_state = if expiration_arm.is_some() && pacing.timer_before.manual_reset {
            RuntimeWaitState::Signaled { exit_code: 0 }
        } else {
            RuntimeWaitState::Nonsignaled
        };
        let wait_updates = expiration_arm
            .map(|_| {
                self.objects.plan_wait_state_updates(&[(
                    RuntimeObjectKey::WaitableTimer {
                        timer_id: timer_id.0,
                    },
                    object_wait_state,
                )])
            })
            .transpose()?;
        self.process.preflight_clock_advance(&clock)?;
        if let Some(plan) = expiration.as_ref() {
            self.timers.preflight_waitable_timer_expiration(plan)?;
        }

        let mut timer_after = pacing.timer_before;
        if let Some(arm) = expiration_arm {
            timer_after.arm = None;
            timer_after.last_expiration = Some(RuntimeWaitableTimerExpiration {
                arm_generation: arm.generation,
                deadline_time_100ns: arm.deadline_time_100ns,
            });
        }
        let result = RuntimeWaitForSingleObjectExResult {
            outcome: pacing.outcome,
            registration: Some(registration),
            object_id: registration.object_id,
            object_wait_state,
            timer: timer_after,
            timer_clock_before: pacing.timer_clock_before,
            timer_clock_after: clock.timer_after(),
        };
        Ok(RuntimeWaitableTimerPacingCompletionPlan {
            scheduler,
            registration,
            clock,
            expiration,
            wait_updates,
            result,
        })
    }

    pub(crate) fn commit_waitable_timer_pacing_completion(
        &mut self,
        plan: RuntimeWaitableTimerPacingCompletionPlan,
    ) -> RuntimeWaitForSingleObjectExResult {
        debug_assert_eq!(
            self.scheduler
                .wait_registration_for_thread(plan.registration.thread_id),
            Ok(Some(plan.registration))
        );
        debug_assert!(self.process.preflight_clock_advance(&plan.clock).is_ok());
        if let Some(expiration) = plan.expiration.as_ref() {
            debug_assert!(
                self.timers
                    .preflight_waitable_timer_expiration(expiration)
                    .is_ok()
            );
        }
        self.scheduler
            .commit_remove_wait_registration(plan.registration);
        let clock = self
            .process
            .commit_clock_advance(plan.clock)
            .expect("preflighted logical clock advance");
        debug_assert_eq!(clock, plan.result.timer_clock_after);
        if let Some(expiration) = plan.expiration {
            let timer = *self.timers.commit_waitable_timer_expiration(expiration);
            debug_assert_eq!(timer, plan.result.timer);
        }
        if let Some(wait_updates) = plan.wait_updates {
            self.objects.commit_wait_state_updates(wait_updates);
        }
        let completion = self.scheduler.commit_host_completion(plan.scheduler);
        debug_assert_eq!(
            completion.request_id,
            plan.result
                .registration
                .unwrap()
                .waitable_timer_pacing
                .unwrap()
                .request_id
        );
        plan.result
    }

    pub fn load_shared_icon(
        &mut self,
        resource: RuntimeIconResource,
    ) -> Result<RuntimeLoadIconAResult, RuntimeStateError> {
        self.require_user32_operation_state()?;
        let image = self
            .modules
            .images()
            .image(resource.module_id)
            .map_err(runtime_module_image_error)?;
        if image.load_base != resource.module_load_base
            || image.source.sha256 != resource.module_source_sha256
        {
            return Err(RuntimeStateError::invalid_input(
                "runtime-user32",
                "icon-module-identity-mismatch",
                "shared icon resource does not match its registered module image",
            ));
        }
        let resources = image.resources.as_ref().ok_or_else(|| {
            RuntimeStateError::not_found(
                "runtime-user32",
                "icon-module-resource-catalog-missing",
                "registered module image has no PE resource catalog",
            )
        })?;
        let language_matches =
            |entry: &&crate::pe32::ResourceDataEntryPlan, type_id, name_id, rva, size| {
                matches!(
                    &entry.resource_type,
                    ResourceIdentifier::Id { id } if *id == type_id
                ) && matches!(&entry.name, ResourceIdentifier::Id { id } if *id == name_id)
                    && matches!(
                        &entry.language,
                        ResourceIdentifier::Id { id } if *id == resource.language_id
                    )
                    && entry.data_rva == rva
                    && entry.size == size
            };
        if resources
            .entries
            .iter()
            .filter(|entry| {
                language_matches(
                    entry,
                    14,
                    resource.group_resource_id,
                    resource.group_data_rva,
                    resource.group_data_size,
                )
            })
            .count()
            != 1
            || resources
                .entries
                .iter()
                .filter(|entry| {
                    language_matches(
                        entry,
                        3,
                        resource.icon_resource_id,
                        resource.icon_data_rva,
                        resource.icon_data_size,
                    )
                })
                .count()
                != 1
        {
            return Err(RuntimeStateError::invalid_input(
                "runtime-user32",
                "icon-resource-catalog-mismatch",
                "shared icon resource does not match unique group-icon and icon catalog entries",
            ));
        }
        let plan = self.user32.plan_shared_icon_load(resource)?;
        self.user32.commit_shared_icon_load(plan)
    }

    pub fn icon(&self, handle: u32) -> Result<&RuntimeIcon, RuntimeStateError> {
        self.user32.icon(handle)
    }

    pub fn icons(&self) -> &BTreeMap<RuntimeIconId, RuntimeIcon> {
        self.user32.icons()
    }

    pub(crate) fn load_shared_cursor(
        &mut self,
        source: RuntimeCursorSource,
    ) -> Result<RuntimeLoadCursorAResult, RuntimeStateError> {
        self.require_user32_operation_state()?;
        if let RuntimeCursorSource::ModuleResource { resource } = &source {
            let image = self
                .modules
                .images()
                .image(resource.module_id)
                .map_err(runtime_module_image_error)?;
            if image.load_base != resource.module_load_base
                || image.source.sha256 != resource.module_source_sha256
            {
                return Err(RuntimeStateError::invalid_input(
                    "runtime-user32",
                    "cursor-module-identity-mismatch",
                    "shared cursor resource does not match its registered module image",
                ));
            }
            let resources = image.resources.as_ref().ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-user32",
                    "cursor-module-resource-catalog-missing",
                    "registered module image has no PE resource catalog",
                )
            })?;
            let language_matches =
                |entry: &&crate::pe32::ResourceDataEntryPlan, type_id, name_id, rva, size| {
                    matches!(
                        &entry.resource_type,
                        ResourceIdentifier::Id { id } if *id == type_id
                    ) && matches!(&entry.name, ResourceIdentifier::Id { id } if *id == name_id)
                        && matches!(
                            &entry.language,
                            ResourceIdentifier::Id { id } if *id == resource.language_id
                        )
                        && entry.data_rva == rva
                        && entry.size == size
                };
            if resources
                .entries
                .iter()
                .filter(|entry| {
                    language_matches(
                        entry,
                        12,
                        resource.group_resource_id,
                        resource.group_data_rva,
                        resource.group_data_size,
                    )
                })
                .count()
                != 1
                || resources
                    .entries
                    .iter()
                    .filter(|entry| {
                        language_matches(
                            entry,
                            1,
                            resource.cursor_resource_id,
                            resource.cursor_data_rva,
                            resource.cursor_data_size,
                        )
                    })
                    .count()
                    != 1
            {
                return Err(RuntimeStateError::invalid_input(
                    "runtime-user32",
                    "cursor-resource-catalog-mismatch",
                    "shared cursor resource does not match unique group-cursor and cursor catalog entries",
                ));
            }
        }
        let plan = self.user32.plan_shared_cursor_load(source)?;
        self.user32.commit_shared_cursor_load(plan)
    }

    pub fn cursor(&self, handle: u32) -> Result<&RuntimeCursor, RuntimeStateError> {
        self.user32.cursor(handle)
    }

    pub fn cursor_for_source(
        &self,
        source: &RuntimeCursorSource,
    ) -> Result<&RuntimeCursor, RuntimeStateError> {
        self.user32.cursor_for_source(source)
    }

    pub fn cursors(&self) -> &BTreeMap<RuntimeCursorId, RuntimeCursor> {
        self.user32.cursors()
    }

    pub(crate) fn show_cursor(&mut self, show: bool) -> Result<i32, RuntimeStateError> {
        self.require_user32_operation_state()?;
        let thread_id = self.current_thread_id();
        let (plan, count) = self
            .threads
            .plan_cursor_display_count_change(thread_id, show)?;
        self.threads.commit_cursor_display_count_change(plan)?;
        Ok(count)
    }

    pub fn current_thread_cursor_display_count(&self) -> Result<i32, RuntimeStateError> {
        self.threads.cursor_display_count(self.current_thread_id())
    }

    pub(crate) fn register_window_class(
        &mut self,
        registration: RuntimeWindowClassRegistration,
    ) -> Result<RuntimeRegisterClassExAResult, RuntimeStateError> {
        self.require_user32_operation_state()?;
        let image = self
            .modules
            .images()
            .image(registration.module.module_id)
            .map_err(runtime_module_image_error)?;
        if image.load_base != registration.module.load_base
            || image.source.sha256 != registration.module.source_sha256
        {
            return Err(RuntimeStateError::invalid_input(
                "runtime-user32",
                "window-class-module-identity-mismatch",
                "window class module identity does not match its registered image",
            ));
        }
        let procedure_address = u64::from(registration.window_procedure.address.0);
        if registration.window_procedure.module_id != image.module_id
            || procedure_address < u64::from(image.image_range.start().0)
            || procedure_address >= image.image_range.end()
        {
            return Err(RuntimeStateError::invalid_input(
                "runtime-user32",
                "window-class-procedure-module-mismatch",
                "window class WndProc does not belong to its registered module image",
            ));
        }
        let plan = self.user32.plan_window_class_registration(registration)?;
        self.user32.commit_window_class_registration(plan)
    }

    pub fn window_class(&self, atom: u16) -> Result<&RuntimeWindowClass, RuntimeStateError> {
        self.user32.window_class(atom)
    }

    pub fn window_class_by_name(
        &self,
        module_id: RuntimeModuleId,
        name: &str,
    ) -> Result<&RuntimeWindowClass, RuntimeStateError> {
        self.user32.window_class_by_name(module_id, name)
    }

    pub fn window_classes(&self) -> &BTreeMap<RuntimeWindowClassId, RuntimeWindowClass> {
        self.user32.window_classes()
    }

    pub(crate) fn preview_window_creation(
        &self,
        request: &RuntimeWindowCreationRequest,
    ) -> Result<RuntimeWindow, RuntimeStateError> {
        self.require_user32_operation_state()?;
        self.validate_window_creation_authority(request)?;
        let plan = self.user32.plan_window_creation(request.clone())?;
        Ok(self.user32.window_creation_preview(&plan).clone())
    }

    pub(crate) fn create_window(
        &mut self,
        request: RuntimeWindowCreationRequest,
    ) -> Result<RuntimeCreateWindowExAResult, RuntimeStateError> {
        self.require_user32_operation_state()?;
        self.validate_window_creation_authority(&request)?;
        let plan = self.user32.plan_window_creation(request)?;
        self.user32.commit_window_creation(plan)
    }

    pub(crate) fn preview_window_creation_callback(
        &self,
        window_id: RuntimeWindowId,
        message: RuntimeWindowCreationMessage,
        result: u32,
    ) -> Result<RuntimeWindowCreationCallbackAction, RuntimeStateError> {
        self.require_user32_operation_state()?;
        self.require_window_owner_thread(window_id)?;
        let plan = self
            .user32
            .plan_window_creation_callback(window_id, message, result)?;
        Ok(self.user32.window_creation_callback_action(&plan).clone())
    }

    pub(crate) fn complete_window_creation_callback(
        &mut self,
        window_id: RuntimeWindowId,
        message: RuntimeWindowCreationMessage,
        result: u32,
    ) -> Result<RuntimeWindowCreationCallbackAction, RuntimeStateError> {
        self.require_user32_operation_state()?;
        self.require_window_owner_thread(window_id)?;
        let plan = self
            .user32
            .plan_window_creation_callback(window_id, message, result)?;
        self.user32.commit_window_creation_callback(plan)
    }

    pub fn window(&self, handle: u32) -> Result<&RuntimeWindow, RuntimeStateError> {
        self.user32.window(handle)
    }

    pub fn window_is_iconic(&self, handle: u32) -> Result<bool, RuntimeStateError> {
        self.user32.window_is_iconic(handle)
    }

    pub fn window_by_id(&self, id: RuntimeWindowId) -> Result<&RuntimeWindow, RuntimeStateError> {
        self.user32.window_by_id(id)
    }

    pub fn windows(&self) -> &BTreeMap<RuntimeWindowId, RuntimeWindow> {
        self.user32.windows()
    }

    pub fn paint_sessions(&self) -> &BTreeMap<RuntimeWindowId, RuntimePaintSession> {
        self.user32.paint_sessions()
    }

    pub(crate) fn get_system_menu(
        &mut self,
        window_handle: u32,
        close_item_command: u32,
    ) -> Result<RuntimeGetSystemMenuResult, RuntimeStateError> {
        self.require_user32_operation_state()?;
        let plan = self
            .user32
            .plan_system_menu(window_handle, close_item_command)?;
        self.user32.commit_system_menu(plan)
    }

    pub(crate) fn enable_system_menu_close_item(
        &mut self,
        menu_handle: u32,
        close_item_command: u32,
    ) -> Result<RuntimeEnableMenuItemResult, RuntimeStateError> {
        self.require_user32_operation_state()?;
        let plan = self
            .user32
            .plan_enable_system_menu_close_item(menu_handle, close_item_command)?;
        self.user32.commit_enable_system_menu_close_item(plan)
    }

    pub fn system_menu(&self, handle: u32) -> Result<&RuntimeSystemMenu, RuntimeStateError> {
        self.user32.system_menu(handle)
    }

    pub fn system_menus(&self) -> &BTreeMap<RuntimeMenuId, RuntimeSystemMenu> {
        self.user32.system_menus()
    }

    pub(crate) fn get_default_ime_window(
        &mut self,
        window_handle: u32,
    ) -> Result<RuntimeDefaultImeWindowResult, RuntimeStateError> {
        self.require_user32_operation_state()?;
        self.require_imm32_operation_state()?;
        let user32 = self
            .user32
            .plan_default_ime_window(window_handle, self.current_thread_id())?;
        let target = self.user32.default_ime_window_target(&user32);
        let imm32 = self.imm32.plan_ensure_window(target)?;
        self.user32.preflight_default_ime_window(&user32)?;
        self.imm32.preflight_ensure_window(&imm32)?;
        let result = self.user32.commit_default_ime_window_preflighted(user32);
        self.imm32.commit_ensure_window_preflighted(imm32);
        Ok(result)
    }

    pub(crate) fn associate_ime_context(
        &mut self,
        window_handle: u32,
        context_handle: u32,
    ) -> Result<RuntimeImeAssociationResult, RuntimeStateError> {
        self.require_user32_operation_state()?;
        self.require_imm32_operation_state()?;
        let target = self
            .user32
            .resolve_ime_app_window_target(window_handle, self.current_thread_id())?;
        let plan = self.imm32.plan_association(target, context_handle)?;
        self.imm32.commit_association(plan)
    }

    pub(crate) fn control_ime_status_window(
        &mut self,
        window_handle: u32,
        request: RuntimeImeStatusWindowRequest,
    ) -> Result<RuntimeImeWindow, RuntimeStateError> {
        self.require_user32_operation_state()?;
        let plan =
            self.user32
                .plan_ime_control(window_handle, self.current_thread_id(), request)?;
        self.user32.commit_ime_control(plan)
    }

    pub fn ime_window(&self, handle: u32) -> Result<&RuntimeImeWindow, RuntimeStateError> {
        self.user32.ime_window(handle)
    }

    pub fn ime_windows(&self) -> &BTreeMap<RuntimeImeWindowId, RuntimeImeWindow> {
        self.user32.ime_windows()
    }

    pub fn ime_context(&self, handle: u32) -> Result<&RuntimeImeContext, RuntimeStateError> {
        self.imm32.context(handle)
    }

    pub fn ime_contexts(&self) -> &BTreeMap<RuntimeImeContextId, RuntimeImeContext> {
        self.imm32.contexts()
    }

    pub fn ime_context_associations(
        &self,
    ) -> &BTreeMap<RuntimeWindowId, RuntimeImeContextAssociation> {
        self.imm32.associations()
    }

    pub(crate) fn register_window_timer(
        &mut self,
        window_handle: u32,
        timer_id: u32,
        interval_milliseconds: u32,
        callback_address: GuestAddress,
    ) -> Result<RuntimeWindowTimerRegistrationResult, RuntimeStateError> {
        self.require_user32_operation_state()?;
        let (window_id, owner_thread_id) = self.resolve_window_timer_target(window_handle)?;
        let timer_clock = self.process.timer_clock_state();
        let plan = self.timers.plan_window_timer_registration(
            RuntimeWindowTimerRegistrationRequest {
                window_id,
                window_handle,
                timer_id,
                owner_thread_id,
                interval_milliseconds,
                callback_address,
            },
            timer_clock,
        )?;
        self.preflight_window_timer_registration(&plan)?;
        Ok(self
            .timers
            .commit_window_timer_registration(plan, timer_clock))
    }

    pub fn window_timers(&self) -> &BTreeMap<RuntimeWindowTimerId, RuntimeWindowTimer> {
        self.timers.window_timers()
    }

    pub fn window_timer(
        &self,
        id: RuntimeWindowTimerId,
    ) -> Result<&RuntimeWindowTimer, RuntimeStateError> {
        self.timers.window_timer(id)
    }

    pub(crate) fn resolve_window_timer_callback(
        &self,
        window_handle: u32,
        timer_id: u32,
        callback_address: GuestAddress,
    ) -> Result<GuestAddress, RuntimeStateError> {
        self.require_user32_operation_state()?;
        let timer = self
            .timers
            .window_timers()
            .values()
            .find(|timer| timer.window_handle == window_handle && timer.timer_id == timer_id)
            .ok_or_else(|| {
                RuntimeStateError::not_found(
                    "runtime-window-timer",
                    "window-timer-message-unknown",
                    "timer message does not identify a live logical window timer",
                )
            })?;
        let window = self.user32.window(window_handle)?;
        if timer.owner_thread_id != self.current_thread_id()
            || timer.callback_address != callback_address
            || timer.delivery_count == 0
            || timer.last_delivery.is_none()
            || window.id != timer.window_id
            || window.lifecycle != RuntimeWindowLifecycle::Created
            || window.creation.owner_thread_id != timer.owner_thread_id
        {
            return Err(RuntimeStateError::conflict(
                "runtime-window-timer",
                "window-timer-message-stale",
                "timer message no longer matches its live window, owner, registration, or callback",
            ));
        }
        Ok(timer.callback_address)
    }

    fn preflight_window_timer_registration(
        &self,
        plan: &super::timers::RuntimeWindowTimerRegistrationPlan,
    ) -> Result<(), RuntimeStateError> {
        self.require_user32_operation_state()?;
        let (window_id, window_handle, _timer_id, owner_thread_id, _callback_address) =
            plan.request_identity();
        let (current_window_id, current_owner_thread_id) =
            self.resolve_window_timer_target(window_handle)?;
        if current_window_id != window_id || current_owner_thread_id != owner_thread_id {
            return Err(RuntimeStateError::conflict(
                "runtime-window-timer",
                "window-timer-target-stale",
                "logical window timer target changed after registration planning",
            ));
        }
        self.timers
            .preflight_window_timer_registration(plan, self.process.timer_clock_state())
    }

    fn resolve_window_timer_target(
        &self,
        window_handle: u32,
    ) -> Result<(RuntimeWindowId, RuntimeThreadId), RuntimeStateError> {
        let window = self.user32.window(window_handle)?;
        if window.lifecycle != RuntimeWindowLifecycle::Created {
            return Err(RuntimeStateError::conflict(
                "runtime-window-timer",
                "window-timer-lifecycle-state-invalid",
                "logical window timer registration requires a created window",
            ));
        }
        if window.creation.owner_thread_id != self.current_thread_id() {
            return Err(RuntimeStateError::conflict(
                "runtime-window-timer",
                "window-timer-owner-thread-mismatch",
                "logical window timer registration requires the selected window owner thread",
            ));
        }
        Ok((window.id, window.creation.owner_thread_id))
    }

    pub(crate) fn set_window_style(
        &mut self,
        window_handle: u32,
        requested_style: u32,
        retained_style_mask: u32,
    ) -> Result<RuntimeSetWindowStyleResult, RuntimeStateError> {
        self.require_user32_operation_state()?;
        let plan = self.user32.plan_window_style_update(
            window_handle,
            self.current_thread_id(),
            requested_style,
            retained_style_mask,
        )?;
        self.user32.commit_window_style_update(plan)
    }

    pub(crate) fn set_window_file_drop_acceptance(
        &mut self,
        window_handle: u32,
        accepts_files: bool,
    ) -> Result<RuntimeWindowFileDropAcceptanceResult, RuntimeStateError> {
        self.require_user32_operation_state()?;
        let plan = self
            .user32
            .plan_window_file_drop_acceptance(window_handle, accepts_files)?;
        self.user32.commit_window_file_drop_acceptance(plan)
    }

    pub(crate) fn acquire_window_dc(
        &mut self,
        window_handle: u32,
    ) -> Result<RuntimeGetDcResult, RuntimeStateError> {
        self.require_gdi_operation_state()?;
        let target = self.resolve_gdi_window_target(window_handle)?;
        let plan = self.gdi.plan_get_dc(target)?;
        if self.resolve_gdi_window_target(window_handle)? != target {
            return Err(RuntimeStateError::conflict(
                "runtime-gdi",
                "get-dc-window-target-stale",
                "logical window changed after GetDC was planned",
            ));
        }
        self.gdi.commit_get_dc(plan)
    }

    pub(crate) fn begin_window_paint(
        &mut self,
        memory: &mut GuestMemory,
        request: RuntimeBeginPaintRequest,
    ) -> Result<(RuntimePaintSession, RuntimeGetDcResult, GuestWriteResult), RuntimeStateError>
    {
        self.require_user32_operation_state()?;
        self.require_gdi_operation_state()?;
        memory
            .validate_access(
                request.paint_struct_address,
                super::user32::RUNTIME_X86_PAINT_STRUCT_BYTE_LENGTH,
                MemoryAccess::Write,
            )
            .map_err(RuntimeStateError::from_memory)?;
        let target = self.resolve_gdi_window_target(request.window_handle)?;
        let gdi_plan = self.gdi.plan_get_dc(target)?;
        let gdi_result = gdi_plan.result();
        let user32_plan = self.user32.plan_begin_paint(
            self.current_thread_id(),
            request,
            gdi_result.device_context_id,
            gdi_result.handle,
        )?;
        let planned_session = user32_plan.session().clone();
        if planned_session.window_id != target.window_id
            || planned_session.window_handle != target.window_handle
            || planned_session.owner_thread_id != target.owner_thread_id
            || planned_session.device_context_id != gdi_result.device_context_id
            || planned_session.device_context_handle != gdi_result.handle
        {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-user32",
                "begin-paint-cross-subsystem-plan-mismatch",
                "BeginPaint User32 session and GDI HDC plans do not identify the same logical window",
            ));
        }
        let mut transaction = memory.begin_transaction();
        let write_result = transaction
            .write(
                request.paint_struct_address,
                &planned_session.paint_struct_bytes,
            )
            .map_err(RuntimeStateError::from_memory)?;
        if self.resolve_gdi_window_target(request.window_handle)? != target {
            return Err(RuntimeStateError::conflict(
                "runtime-user32",
                "begin-paint-window-target-stale",
                "logical window changed after BeginPaint planning",
            ));
        }
        self.gdi.preflight_get_dc(&gdi_plan)?;
        self.user32.preflight_begin_paint(&user32_plan)?;
        let committed_gdi = self.gdi.commit_preflighted_get_dc(gdi_plan);
        debug_assert_eq!(committed_gdi, gdi_result);
        let committed_session = self.user32.commit_preflighted_begin_paint(user32_plan);
        transaction.commit_transaction();
        Ok((committed_session, committed_gdi, write_result))
    }

    pub(crate) fn end_window_paint(
        &mut self,
        request: RuntimeEndPaintRequest,
    ) -> Result<RuntimePaintSession, RuntimeStateError> {
        self.require_user32_operation_state()?;
        self.require_gdi_operation_state()?;
        let user32_plan = self
            .user32
            .plan_end_paint(self.current_thread_id(), request.clone())?;
        let session = user32_plan.session().clone();
        let target = self.resolve_gdi_window_target(request.window_handle)?;
        let device_context = self
            .gdi
            .device_context_by_handle(session.device_context_handle)?
            .clone();
        validate_device_context_target(&device_context, target)?;
        if device_context.id != session.device_context_id {
            return Err(RuntimeStateError::conflict(
                "runtime-user32",
                "end-paint-device-context-identity-mismatch",
                "EndPaint session and logical HDC do not identify the same GDI object",
            ));
        }
        let gdi_plan = self.gdi.plan_release_dc(
            request.window_handle,
            session.device_context_handle,
            self.current_thread_id(),
        )?;
        if self.resolve_gdi_window_target(request.window_handle)? != target {
            return Err(RuntimeStateError::conflict(
                "runtime-user32",
                "end-paint-window-target-stale",
                "logical window changed after EndPaint planning",
            ));
        }
        self.user32.preflight_end_paint(&user32_plan)?;
        self.gdi.preflight_release_dc(&gdi_plan)?;
        self.gdi.commit_preflighted_release_dc(gdi_plan);
        Ok(self.user32.commit_preflighted_end_paint(user32_plan))
    }

    pub(crate) fn acquire_screen_dc(
        &mut self,
    ) -> Result<RuntimeGetScreenDcResult, RuntimeStateError> {
        self.require_gdi_operation_state()?;
        let target = self.resolve_gdi_screen_target()?;
        let plan = self.gdi.plan_get_screen_dc(target)?;
        if self.resolve_gdi_screen_target()? != target {
            return Err(RuntimeStateError::conflict(
                "runtime-gdi",
                "screen-get-dc-target-stale",
                "logical display changed after screen GetDC was planned",
            ));
        }
        self.gdi.commit_get_screen_dc(plan)
    }

    pub(crate) fn create_compatible_memory_dc(
        &mut self,
    ) -> Result<RuntimeCreateCompatibleDcResult, RuntimeStateError> {
        self.require_gdi_operation_state()?;
        let target = self.resolve_gdi_screen_target()?;
        let plan = self.gdi.plan_create_compatible_dc(target)?;
        if self.resolve_gdi_screen_target()? != target {
            return Err(RuntimeStateError::conflict(
                "runtime-gdi",
                "create-compatible-dc-target-stale",
                "logical display changed after CreateCompatibleDC was planned",
            ));
        }
        self.gdi.commit_create_compatible_dc(plan)
    }

    pub(crate) fn delete_compatible_memory_dc(
        &mut self,
        device_context_handle: u32,
    ) -> Result<(), RuntimeStateError> {
        self.require_gdi_operation_state()?;
        let plan = self
            .gdi
            .plan_delete_compatible_dc(device_context_handle, self.current_thread_id())?;
        self.gdi.commit_release_dc(plan)
    }

    pub(crate) fn validate_screen_dc_operation(
        &self,
        device_context_handle: u32,
    ) -> Result<RuntimeDeviceContext, RuntimeStateError> {
        self.require_gdi_operation_state()?;
        let target = self.resolve_gdi_screen_target()?;
        let device_context = self.gdi.validate_screen_dc_access(
            target,
            device_context_handle,
            self.current_thread_id(),
        )?;
        if self.resolve_gdi_screen_target()? != target {
            return Err(RuntimeStateError::conflict(
                "runtime-gdi",
                "screen-dc-operation-target-stale",
                "logical display changed while screen GDI access was being validated",
            ));
        }
        Ok(device_context)
    }

    pub(crate) fn plan_graphics_surface_dc_acquisition(
        &self,
        surface_com_object_id: RuntimeComObjectId,
    ) -> Result<RuntimeGraphicsSurfaceDcAcquisitionPlan, RuntimeStateError> {
        self.require_gdi_operation_state()?;
        self.require_graphics_operation_state()?;
        let target = self.resolve_gdi_graphics_surface_target(surface_com_object_id)?;
        let gdi = self.gdi.plan_get_graphics_surface_dc(target)?;
        Ok(RuntimeGraphicsSurfaceDcAcquisitionPlan { target, gdi })
    }

    pub(crate) fn commit_graphics_surface_dc_acquisition(
        &mut self,
        plan: RuntimeGraphicsSurfaceDcAcquisitionPlan,
    ) -> Result<RuntimeGetGraphicsSurfaceDcResult, RuntimeStateError> {
        if self.resolve_gdi_graphics_surface_target(plan.target.surface_com_object_id)?
            != plan.target
        {
            return Err(RuntimeStateError::conflict(
                "runtime-gdi",
                "graphics-surface-get-dc-target-stale",
                "logical graphics surface changed after GetDC was planned",
            ));
        }
        self.gdi.commit_get_graphics_surface_dc(plan.gdi)
    }

    pub(crate) fn release_graphics_surface_dc(
        &mut self,
        surface_com_object_id: RuntimeComObjectId,
        device_context_handle: u32,
    ) -> Result<(), RuntimeStateError> {
        self.require_gdi_operation_state()?;
        self.require_graphics_operation_state()?;
        let target = self.resolve_gdi_graphics_surface_target(surface_com_object_id)?;
        let plan = self.gdi.plan_release_graphics_surface_dc(
            target,
            device_context_handle,
            self.current_thread_id(),
        )?;
        if self.resolve_gdi_graphics_surface_target(surface_com_object_id)? != target {
            return Err(RuntimeStateError::conflict(
                "runtime-gdi",
                "graphics-surface-release-dc-target-stale",
                "logical graphics surface changed after ReleaseDC was planned",
            ));
        }
        self.gdi.commit_release_dc(plan)
    }

    pub(crate) fn get_gdi_stock_object(
        &mut self,
        index: i32,
    ) -> Result<RuntimeGetStockObjectResult, RuntimeStateError> {
        self.require_gdi_operation_state()?;
        let plan = self.gdi.plan_get_stock_object(index)?;
        self.gdi.commit_get_stock_object(plan)
    }

    pub(crate) fn create_gdi_font_a(
        &mut self,
        log_font: RuntimeLogFontA,
    ) -> Result<RuntimeCreateFontAResult, RuntimeStateError> {
        self.require_gdi_operation_state()?;
        let plan = self.gdi.plan_create_font_a(log_font)?;
        self.gdi.commit_create_font_a(plan)
    }

    pub(crate) fn create_gdi_dib_section(
        &mut self,
        memory: &mut GuestMemory,
        device_context_handle: u32,
        descriptor: RuntimeGdiDibSectionDescriptor,
        bits_output: GuestAddress,
    ) -> Result<(RuntimeCreateDibSectionResult, GuestWriteResult), RuntimeStateError> {
        self.require_gdi_operation_state()?;
        memory
            .validate_access(bits_output, 4, MemoryAccess::Write)
            .map_err(RuntimeStateError::from_memory)?;
        let arena = self
            .address_space_allocator
            .allocation_for_purpose(&ProcessAddressAllocationPurpose::GraphicsResourceArena)?;
        if arena.range.base != super::RUNTIME_GRAPHICS_RESOURCE_ARENA_BASE
            || arena.range.byte_length != super::RUNTIME_GRAPHICS_RESOURCE_ARENA_SIZE
            || arena.classification != ProcessAddressAllocationClassification::Graphics
            || arena.lifetime != ProcessAddressAllocationLifetime::Process
            || arena.release_policy != ProcessAddressAllocationReleasePolicy::Permanent
            || arena.executable
        {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-gdi",
                "gdi-resource-arena-invalid",
                "GDI resource arena does not match the controlled process layout",
            ));
        }
        let address_space = self.address_space_allocator.plan_reservations(vec![(
            Some(arena.id),
            ProcessAddressAllocationRequest {
                purpose: ProcessAddressAllocationPurpose::GdiResource,
                owner: RUNTIME_GDI_RESOURCE_ADDRESS_OWNER.to_owned(),
                preferred_address: None,
                search_range: arena.range,
                reserve_size: u64::from(descriptor.pixel_byte_length),
                alignment: GUEST_PAGE_SIZE as u32,
                guard_pages: 0,
                classification: ProcessAddressAllocationClassification::Graphics,
                lifetime: ProcessAddressAllocationLifetime::Object,
                release_policy: ProcessAddressAllocationReleasePolicy::Explicit,
                executable: false,
            },
        )])?;
        let [allocation] = address_space.allocations() else {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-gdi",
                "dib-section-address-plan-shape-invalid",
                "DIB section installation did not plan exactly one address allocation",
            ));
        };
        let allocation = allocation.clone();
        let backing_range = GuestRange::new(allocation.range.base, allocation.range.byte_length)
            .map_err(RuntimeStateError::from_memory)?;
        let gdi = self.gdi.plan_create_dib_section(
            device_context_handle,
            self.current_thread_id(),
            descriptor,
            allocation.id,
            backing_range,
        )?;
        let page_count = backing_range.page_count();
        memory
            .preflight_growth(1, 1, page_count, "runtime-gdi")
            .map_err(RuntimeStateError::from_memory)?;
        memory
            .preflight_fixed_reservation(
                backing_range.start(),
                backing_range.byte_length(),
                "runtime-gdi",
            )
            .map_err(RuntimeStateError::from_memory)?;

        let mut transaction = memory.begin_transaction();
        let memory_owner = transaction
            .create_owner(MemoryOwnerKind::Runtime)
            .map_err(RuntimeStateError::from_memory)?;
        transaction
            .reserve_fixed_for_address_allocation(
                allocation.id.0,
                memory_owner,
                MemoryRegionKind::GdiResource,
                backing_range.start(),
                backing_range.byte_length(),
            )
            .map_err(RuntimeStateError::from_memory)?;
        transaction
            .commit(
                memory_owner,
                backing_range,
                PageProtection {
                    read: true,
                    write: true,
                    execute: false,
                },
            )
            .map_err(RuntimeStateError::from_memory)?;
        let bits_write = transaction
            .write(bits_output, &backing_range.start().0.to_le_bytes())
            .map_err(RuntimeStateError::from_memory)?;

        self.address_space_allocator
            .preflight_reservations_commit(&address_space)?;
        self.gdi.preflight_create_dib_section(&gdi, memory_owner)?;
        if allocation.parent_id != Some(arena.id)
            || allocation.id != gdi.address_allocation_id()
            || allocation.range.base != gdi.backing_range().start()
            || allocation.range.byte_length != gdi.backing_range().byte_length()
            || allocation.usable_byte_length != u64::from(gdi.descriptor().pixel_byte_length)
            || allocation.purpose != ProcessAddressAllocationPurpose::GdiResource
            || allocation.owner != RUNTIME_GDI_RESOURCE_ADDRESS_OWNER
            || allocation.classification != ProcessAddressAllocationClassification::Graphics
            || allocation.lifetime != ProcessAddressAllocationLifetime::Object
            || allocation.release_policy != ProcessAddressAllocationReleasePolicy::Explicit
            || allocation.executable
        {
            return Err(RuntimeStateError::conflict(
                "runtime-gdi",
                "dib-section-backing-allocation-mismatch",
                "logical DIB section backing does not match its planned process allocation",
            ));
        }
        self.address_space_allocator
            .commit_reservations(address_space);
        let result = self.gdi.commit_create_dib_section(gdi, memory_owner)?;
        transaction.commit_transaction();
        Ok((result, bits_write))
    }

    pub(crate) fn get_gdi_object_bytes(
        &self,
        object_handle: u32,
        requested_byte_length: u32,
    ) -> Result<Vec<u8>, RuntimeStateError> {
        self.require_gdi_operation_state()?;
        self.gdi.object_bytes(object_handle, requested_byte_length)
    }

    pub(crate) fn get_gdi_selected_font_a(
        &self,
        device_context_handle: u32,
    ) -> Result<RuntimeLogFontA, RuntimeStateError> {
        self.require_gdi_operation_state()?;
        self.gdi
            .selected_font_a(device_context_handle, self.current_thread_id())
    }

    pub(crate) fn get_gdi_text_raster_target(
        &self,
        device_context_handle: u32,
    ) -> Result<RuntimeGdiTextRasterTarget, RuntimeStateError> {
        self.require_gdi_operation_state()?;
        self.gdi
            .text_raster_target(device_context_handle, self.current_thread_id())
    }

    pub(crate) fn commit_gdi_text_raster(
        &mut self,
        memory: &mut GuestMemory,
        target: RuntimeGdiTextRasterTarget,
        byte_offset: u32,
        expected_bytes: &[u8],
        replacement_bytes: &[u8],
    ) -> Result<(RuntimeGdiTextRasterResult, GuestWriteResult), RuntimeStateError> {
        self.require_gdi_operation_state()?;
        if expected_bytes.is_empty() || expected_bytes.len() != replacement_bytes.len() {
            return Err(RuntimeStateError::invalid_input(
                "runtime-gdi",
                "text-raster-byte-plan-invalid",
                "text rasterization requires equally sized nonempty before and after bytes",
            ));
        }
        let byte_length = u64::try_from(expected_bytes.len()).map_err(|_| {
            RuntimeStateError::resource_limit(
                "runtime-gdi",
                "text-raster-byte-length-unrepresentable",
                "text raster byte length does not fit guest range accounting",
            )
        })?;
        let pixel_end = u64::from(byte_offset)
            .checked_add(byte_length)
            .ok_or_else(|| {
                RuntimeStateError::resource_limit(
                    "runtime-gdi",
                    "text-raster-byte-range-overflow",
                    "text raster byte range overflowed x86 accounting",
                )
            })?;
        if pixel_end > u64::from(target.descriptor.pixel_byte_length) {
            return Err(RuntimeStateError::invalid_input(
                "runtime-gdi",
                "text-raster-byte-range-outside-dib",
                "text raster byte range extends beyond the selected DIB pixel plane",
            ));
        }
        let address = target
            .backing
            .range
            .start()
            .0
            .checked_add(byte_offset)
            .map(GuestAddress)
            .ok_or_else(|| {
                RuntimeStateError::resource_limit(
                    "runtime-gdi",
                    "text-raster-address-overflow",
                    "text raster guest address overflowed x86 accounting",
                )
            })?;
        memory
            .validate_access(address, expected_bytes.len(), MemoryAccess::Read)
            .map_err(RuntimeStateError::from_memory)?;
        memory
            .validate_access(address, replacement_bytes.len(), MemoryAccess::Write)
            .map_err(RuntimeStateError::from_memory)?;
        if memory
            .read(address, expected_bytes.len())
            .map_err(RuntimeStateError::from_memory)?
            != expected_bytes
        {
            return Err(RuntimeStateError::conflict(
                "runtime-gdi",
                "text-raster-backing-stale",
                "selected DIB bytes changed after text rasterization was planned",
            ));
        }
        let gdi = self.gdi.plan_text_raster_commit(target)?;
        let mut transaction = memory.begin_transaction();
        let write_result = transaction
            .write(address, replacement_bytes)
            .map_err(RuntimeStateError::from_memory)?;
        self.gdi.preflight_text_raster_commit(&gdi)?;
        let result = self.gdi.commit_text_raster(gdi)?;
        transaction.commit_transaction();
        Ok((result, write_result))
    }

    pub(crate) fn pending_gdi_text_raster_target(
        &self,
        request_id: u64,
        expected_service: &str,
        device_context_handle: u32,
    ) -> Result<RuntimeGdiTextRasterTarget, RuntimeStateError> {
        if !self.process.is_running()
            || self.current_thread()?.lifecycle != RuntimeThreadLifecycleState::Running
        {
            return Err(RuntimeStateError::conflict(
                "runtime-gdi",
                "text-raster-host-process-state-invalid",
                "browser text raster completion requires a running process and thread",
            ));
        }
        let scheduler = self.scheduler.plan_host_completion(request_id)?;
        if scheduler.request().service != expected_service
            || scheduler.request().thread_id != self.current_thread_id()
        {
            return Err(RuntimeStateError::conflict(
                "runtime-gdi",
                "text-raster-host-request-mismatch",
                "browser text raster completion does not own the current waiting thread",
            ));
        }
        self.gdi
            .text_raster_target(device_context_handle, self.current_thread_id())
    }

    pub(crate) fn commit_pending_gdi_text_raster(
        &mut self,
        memory: &mut GuestMemory,
        request_id: u64,
        expected_service: &str,
        target: RuntimeGdiTextRasterTarget,
        byte_offset: u32,
        expected_bytes: &[u8],
        replacement_bytes: &[u8],
    ) -> Result<
        (
            RuntimeGdiTextRasterResult,
            GuestWriteResult,
            RuntimeHostCompletion,
        ),
        RuntimeStateError,
    > {
        let scheduler = self.scheduler.plan_host_completion(request_id)?;
        if scheduler.request().service != expected_service
            || scheduler.request().thread_id != self.current_thread_id()
        {
            return Err(RuntimeStateError::conflict(
                "runtime-gdi",
                "text-raster-host-request-mismatch",
                "browser text raster completion does not own the current waiting thread",
            ));
        }
        if expected_bytes.is_empty() || expected_bytes.len() != replacement_bytes.len() {
            return Err(RuntimeStateError::invalid_input(
                "runtime-gdi",
                "text-raster-byte-plan-invalid",
                "text rasterization requires equally sized nonempty before and after bytes",
            ));
        }
        let byte_length = u64::try_from(expected_bytes.len()).map_err(|_| {
            RuntimeStateError::resource_limit(
                "runtime-gdi",
                "text-raster-byte-length-unrepresentable",
                "text raster byte length does not fit guest range accounting",
            )
        })?;
        let pixel_end = u64::from(byte_offset)
            .checked_add(byte_length)
            .ok_or_else(|| {
                RuntimeStateError::resource_limit(
                    "runtime-gdi",
                    "text-raster-byte-range-overflow",
                    "text raster byte range overflowed x86 accounting",
                )
            })?;
        if pixel_end > u64::from(target.descriptor.pixel_byte_length) {
            return Err(RuntimeStateError::invalid_input(
                "runtime-gdi",
                "text-raster-byte-range-outside-dib",
                "text raster byte range extends beyond the selected DIB pixel plane",
            ));
        }
        let address = target
            .backing
            .range
            .start()
            .0
            .checked_add(byte_offset)
            .map(GuestAddress)
            .ok_or_else(|| {
                RuntimeStateError::resource_limit(
                    "runtime-gdi",
                    "text-raster-address-overflow",
                    "text raster guest address overflowed x86 accounting",
                )
            })?;
        memory
            .validate_access(address, expected_bytes.len(), MemoryAccess::Read)
            .map_err(RuntimeStateError::from_memory)?;
        memory
            .validate_access(address, replacement_bytes.len(), MemoryAccess::Write)
            .map_err(RuntimeStateError::from_memory)?;
        if memory
            .read(address, expected_bytes.len())
            .map_err(RuntimeStateError::from_memory)?
            != expected_bytes
        {
            return Err(RuntimeStateError::conflict(
                "runtime-gdi",
                "text-raster-backing-stale",
                "selected DIB bytes changed while browser rasterization was pending",
            ));
        }
        let gdi = self.gdi.plan_text_raster_commit(target)?;
        let mut transaction = memory.begin_transaction();
        let write_result = transaction
            .write(address, replacement_bytes)
            .map_err(RuntimeStateError::from_memory)?;
        self.gdi.preflight_text_raster_commit(&gdi)?;
        let result = self.gdi.commit_text_raster(gdi)?;
        transaction.commit_transaction();
        let completion = self.scheduler.commit_host_completion(scheduler);
        Ok((result, write_result, completion))
    }

    pub(crate) fn start_gdiplus_session(
        &mut self,
        memory: &mut GuestMemory,
        request: RuntimeGdiplusStartupRequest,
        token_output: GuestAddress,
    ) -> Result<(RuntimeGdiplusSession, GuestWriteResult), RuntimeStateError> {
        self.require_gdiplus_operation_state()?;
        memory
            .validate_access(token_output, 4, MemoryAccess::Write)
            .map_err(RuntimeStateError::from_memory)?;
        let plan = self.gdiplus.plan_startup(request)?;
        let token_bytes = plan.session().token.to_le_bytes();
        let mut transaction = memory.begin_transaction();
        let token_write = transaction
            .write(token_output, &token_bytes)
            .map_err(RuntimeStateError::from_memory)?;
        self.gdiplus.preflight_startup(&plan)?;
        let session = self.gdiplus.commit_startup(plan)?;
        transaction.commit_transaction();
        Ok((session, token_write))
    }

    pub(crate) fn shutdown_gdiplus_session(
        &mut self,
        token: u32,
    ) -> Result<RuntimeGdiplusSession, RuntimeStateError> {
        self.require_gdiplus_operation_state()?;
        let plan = self.gdiplus.plan_shutdown(token)?;
        self.gdiplus.commit_shutdown(plan)
    }

    pub(crate) fn select_gdi_object(
        &mut self,
        device_context_handle: u32,
        object_handle: u32,
    ) -> Result<RuntimeSelectGdiObjectResult, RuntimeStateError> {
        self.require_gdi_operation_state()?;
        let plan = self.gdi.plan_select_object(
            device_context_handle,
            object_handle,
            self.current_thread_id(),
        )?;
        self.gdi.commit_select_object(plan)
    }

    pub(crate) fn delete_gdi_object(
        &mut self,
        memory: &mut GuestMemory,
        object_handle: u32,
    ) -> Result<RuntimeGdiObject, RuntimeStateError> {
        self.require_gdi_operation_state()?;
        let gdi: RuntimeDeleteGdiObjectPlan = self.gdi.plan_delete_object(object_handle)?;
        let Some(backing) = gdi.backing() else {
            return self.gdi.commit_delete_object(gdi);
        };
        let address_space = self
            .address_space_allocator
            .plan_releases(&[backing.address_allocation_id])?;
        let [allocation] = address_space.allocations() else {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-gdi",
                "delete-dib-section-address-plan-shape-invalid",
                "DIB section deletion did not plan exactly one address release",
            ));
        };
        if allocation.range.base != backing.range.start()
            || allocation.range.byte_length != backing.range.byte_length()
            || allocation.usable_byte_length
                != match &gdi.object().object {
                    super::gdi::RuntimeGdiObjectKind::DibSection { descriptor, .. } => {
                        u64::from(descriptor.pixel_byte_length)
                    }
                    _ => 0,
                }
            || allocation.purpose != ProcessAddressAllocationPurpose::GdiResource
            || allocation.owner != RUNTIME_GDI_RESOURCE_ADDRESS_OWNER
            || allocation.classification != ProcessAddressAllocationClassification::Graphics
            || allocation.lifetime != ProcessAddressAllocationLifetime::Object
            || allocation.release_policy != ProcessAddressAllocationReleasePolicy::Explicit
            || allocation.executable
        {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-gdi",
                "delete-dib-section-allocation-mismatch",
                "retiring logical DIB section does not own its process address allocation",
            ));
        }

        let mut transaction = memory.begin_transaction();
        transaction
            .release_region(backing.memory_owner, backing.range.start())
            .map_err(RuntimeStateError::from_memory)?;
        transaction
            .destroy_owner(backing.memory_owner)
            .map_err(RuntimeStateError::from_memory)?;
        self.address_space_allocator
            .preflight_releases(&address_space)?;
        self.gdi.preflight_delete_object(&gdi)?;
        self.address_space_allocator.commit_releases(address_space);
        let object = self.gdi.commit_delete_object(gdi)?;
        transaction.commit_transaction();
        Ok(object)
    }

    pub(crate) fn fill_gdi_rect(
        &mut self,
        device_context_handle: u32,
        rect: super::displays::RuntimeRect,
        brush_handle: u32,
    ) -> Result<(), RuntimeStateError> {
        self.require_gdi_operation_state()?;
        let device_context = self
            .gdi
            .device_context_by_handle(device_context_handle)?
            .clone();
        match device_context.target {
            RuntimeDeviceContextTarget::Window { window_handle, .. } => {
                let target = self.resolve_gdi_window_target(window_handle)?;
                validate_device_context_target(&device_context, target)?;
                let plan = self.gdi.plan_fill_rect(
                    device_context_handle,
                    rect,
                    brush_handle,
                    self.current_thread_id(),
                )?;
                let current_target = self.resolve_gdi_window_target(window_handle)?;
                validate_device_context_target(&device_context, current_target)?;
                self.gdi.commit_fill_rect(plan).map(|_| ())
            }
            RuntimeDeviceContextTarget::GraphicsSurface {
                surface_com_object_id,
                ..
            } => {
                self.require_graphics_operation_state()?;
                let target = self.resolve_gdi_graphics_surface_target(surface_com_object_id)?;
                validate_graphics_device_context_target(&device_context, target)?;
                let gdi_plan = self.gdi.plan_graphics_surface_fill_rect(
                    device_context_handle,
                    rect,
                    brush_handle,
                    self.current_thread_id(),
                )?;
                let graphics_plan =
                    self.graphics
                        .plan_surface_fill(RuntimeGraphicsSurfaceFillRequest {
                            surface_com_object_id: gdi_plan.surface_com_object_id(),
                            rect: gdi_plan.rect(),
                            color_ref: gdi_plan.color_ref(),
                        })?;
                let current_target =
                    self.resolve_gdi_graphics_surface_target(surface_com_object_id)?;
                validate_graphics_device_context_target(&device_context, current_target)?;
                if gdi_plan.surface_id() != target.surface_id {
                    return Err(RuntimeStateError::conflict(
                        "runtime-gdi",
                        "graphics-fill-rect-target-stale",
                        "logical HDC and graphics surface fill targets disagree",
                    ));
                }
                self.gdi.preflight_graphics_surface_fill_rect(&gdi_plan)?;
                self.graphics.preflight_surface_fill(&graphics_plan)?;
                self.graphics.commit_surface_fill(graphics_plan);
                Ok(())
            }
            RuntimeDeviceContextTarget::Screen { .. } => Err(RuntimeStateError::conflict(
                "runtime-gdi",
                "screen-dc-drawing-unsupported",
                "drawing through a logical screen HDC is outside the reached GDI support band",
            )),
            RuntimeDeviceContextTarget::Memory { .. } => Err(RuntimeStateError::conflict(
                "runtime-gdi",
                "memory-dc-drawing-unsupported",
                "drawing through a logical memory DC requires a selected DIB surface",
            )),
        }
    }

    pub(crate) fn release_window_dc(
        &mut self,
        window_handle: u32,
        device_context_handle: u32,
    ) -> Result<(), RuntimeStateError> {
        self.require_gdi_operation_state()?;
        let target = self.resolve_gdi_window_target(window_handle)?;
        let device_context = self
            .gdi
            .device_context_by_handle(device_context_handle)?
            .clone();
        validate_device_context_target(&device_context, target)?;
        self.user32
            .require_non_paint_device_context(device_context.id, device_context.handle)?;
        let plan = self.gdi.plan_release_dc(
            window_handle,
            device_context_handle,
            self.current_thread_id(),
        )?;
        let current_target = self.resolve_gdi_window_target(window_handle)?;
        validate_device_context_target(&device_context, current_target)?;
        self.user32
            .require_non_paint_device_context(device_context.id, device_context.handle)?;
        self.gdi.commit_release_dc(plan)
    }

    pub(crate) fn release_screen_dc(
        &mut self,
        device_context_handle: u32,
    ) -> Result<(), RuntimeStateError> {
        self.require_gdi_operation_state()?;
        let target = self.resolve_gdi_screen_target()?;
        let device_context = self
            .gdi
            .device_context_by_handle(device_context_handle)?
            .clone();
        validate_screen_device_context_target(&device_context, target)?;
        let plan = self.gdi.plan_release_screen_dc(
            target,
            device_context_handle,
            self.current_thread_id(),
        )?;
        let current_target = self.resolve_gdi_screen_target()?;
        validate_screen_device_context_target(&device_context, current_target)?;
        self.gdi.commit_release_dc(plan)
    }

    pub fn gdi_device_context(
        &self,
        handle: u32,
    ) -> Result<&RuntimeDeviceContext, RuntimeStateError> {
        self.gdi.device_context_by_handle(handle)
    }

    pub fn gdi_device_contexts(&self) -> &BTreeMap<RuntimeDeviceContextId, RuntimeDeviceContext> {
        self.gdi.device_contexts()
    }

    pub fn gdi_objects(&self) -> &BTreeMap<RuntimeGdiObjectId, RuntimeGdiObject> {
        self.gdi.objects()
    }

    pub fn gdi_surfaces(&self) -> &BTreeMap<RuntimeGdiSurfaceId, RuntimeGdiSurface> {
        self.gdi.surfaces()
    }

    pub fn gdiplus_sessions(
        &self,
    ) -> &BTreeMap<super::RuntimeGdiplusSessionId, RuntimeGdiplusSession> {
        self.gdiplus.sessions()
    }

    pub(crate) fn plan_audio_buffer_creation(
        &self,
        request: RuntimeAudioBufferCreateRequest,
    ) -> Result<RuntimeAudioBufferCreationPlan, RuntimeStateError> {
        self.require_audio_operation_state()?;
        self.audio.plan_buffer_creation(request)
    }

    pub(crate) fn preflight_audio_buffer_creation(
        &self,
        plan: &RuntimeAudioBufferCreationPlan,
    ) -> Result<(), RuntimeStateError> {
        self.require_audio_operation_state()?;
        self.audio.preflight_buffer_creation(plan)
    }

    pub(crate) fn commit_audio_buffer_creation(
        &mut self,
        plan: RuntimeAudioBufferCreationPlan,
        com_object_id: RuntimeComObjectId,
    ) -> RuntimeAudioBuffer {
        debug_assert!(self.preflight_audio_buffer_creation(&plan).is_ok());
        self.audio.commit_buffer_creation(plan, com_object_id)
    }

    pub(crate) fn plan_audio_buffer_format(
        &self,
        request: RuntimeAudioBufferFormatRequest,
    ) -> Result<RuntimeAudioBufferFormatPlan, RuntimeStateError> {
        self.require_audio_operation_state()?;
        self.audio.plan_buffer_format(request)
    }

    pub(crate) fn commit_audio_buffer_format(
        &mut self,
        plan: RuntimeAudioBufferFormatPlan,
    ) -> Result<RuntimeAudioBufferFormatResult, RuntimeStateError> {
        self.require_audio_operation_state()?;
        self.audio.preflight_buffer_format(&plan)?;
        Ok(self.audio.commit_buffer_format(plan))
    }

    pub(crate) fn plan_audio_buffer_lock(
        &self,
        request: RuntimeAudioBufferLockRequest,
    ) -> Result<RuntimeAudioBufferLockInstallationPlan, RuntimeStateError> {
        self.require_audio_operation_state()?;
        let buffer = self
            .audio
            .buffer_by_com_object(request.buffer_com_object_id)?;
        let address_space = if buffer.backing.is_none() {
            let arena = self
                .address_space_allocator
                .allocation_for_purpose(&ProcessAddressAllocationPurpose::AudioResourceArena)?;
            if arena.range.base != super::RUNTIME_AUDIO_RESOURCE_ARENA_BASE
                || arena.range.byte_length != super::RUNTIME_AUDIO_RESOURCE_ARENA_SIZE
                || arena.classification != ProcessAddressAllocationClassification::Audio
                || arena.lifetime != ProcessAddressAllocationLifetime::Process
                || arena.release_policy != ProcessAddressAllocationReleasePolicy::Permanent
                || arena.executable
            {
                return Err(RuntimeStateError::runtime_defect(
                    "runtime-audio",
                    "audio-resource-arena-invalid",
                    "audio resource arena does not match the controlled process layout",
                ));
            }
            Some(self.address_space_allocator.plan_reservations(vec![(
                Some(arena.id),
                ProcessAddressAllocationRequest {
                    purpose: ProcessAddressAllocationPurpose::AudioResource,
                    owner: RUNTIME_AUDIO_RESOURCE_ADDRESS_OWNER.to_owned(),
                    preferred_address: None,
                    search_range: arena.range,
                    reserve_size: u64::from(buffer.descriptor.byte_length),
                    alignment: GUEST_PAGE_SIZE as u32,
                    guard_pages: 0,
                    classification: ProcessAddressAllocationClassification::Audio,
                    lifetime: ProcessAddressAllocationLifetime::Object,
                    release_policy: ProcessAddressAllocationReleasePolicy::Explicit,
                    executable: false,
                },
            )])?)
        } else {
            None
        };
        let backing_installation = address_space
            .as_ref()
            .and_then(|plan| plan.allocations().first())
            .map(|allocation| {
                GuestRange::new(allocation.range.base, allocation.range.byte_length)
                    .map(|range| RuntimeAudioBufferBackingPlan {
                        address_allocation_id: allocation.id,
                        range,
                    })
                    .map_err(RuntimeStateError::from_memory)
            })
            .transpose()?;
        let audio = self.audio.plan_buffer_lock(request, backing_installation)?;
        Ok(RuntimeAudioBufferLockInstallationPlan {
            address_space,
            audio,
        })
    }

    pub(crate) fn preflight_audio_buffer_lock(
        &self,
        plan: &RuntimeAudioBufferLockInstallationPlan,
        backing_memory_owner: Option<MemoryOwnerId>,
    ) -> Result<(), RuntimeStateError> {
        self.require_audio_operation_state()?;
        if let Some(address_space) = &plan.address_space {
            self.address_space_allocator
                .preflight_reservations_commit(address_space)?;
            let [allocation] = address_space.allocations() else {
                return Err(RuntimeStateError::runtime_defect(
                    "runtime-audio",
                    "audio-buffer-lock-allocation-count-invalid",
                    "initial audio Lock did not plan exactly one sample-backing allocation",
                ));
            };
            let arena = self
                .address_space_allocator
                .allocation_for_purpose(&ProcessAddressAllocationPurpose::AudioResourceArena)?;
            let backing = plan.audio.backing_installation().ok_or_else(|| {
                RuntimeStateError::runtime_defect(
                    "runtime-audio",
                    "audio-buffer-lock-backing-plan-missing",
                    "initial audio Lock omitted its sample-backing installation",
                )
            })?;
            let buffer = self.audio.buffer(plan.audio.buffer_id())?;
            if allocation.parent_id != Some(arena.id)
                || allocation.id != backing.address_allocation_id
                || allocation.range.base != backing.range.start()
                || allocation.range.byte_length != backing.range.byte_length()
                || allocation.usable_byte_length != u64::from(buffer.descriptor.byte_length)
                || allocation.purpose != ProcessAddressAllocationPurpose::AudioResource
                || allocation.owner != RUNTIME_AUDIO_RESOURCE_ADDRESS_OWNER
                || allocation.classification != ProcessAddressAllocationClassification::Audio
                || allocation.lifetime != ProcessAddressAllocationLifetime::Object
                || allocation.release_policy != ProcessAddressAllocationReleasePolicy::Explicit
                || allocation.executable
            {
                return Err(RuntimeStateError::conflict(
                    "runtime-audio",
                    "audio-buffer-backing-allocation-mismatch",
                    "logical audio buffer backing does not match its planned process allocation",
                ));
            }
        } else if plan.audio.backing_installation().is_some() {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-audio",
                "audio-buffer-lock-backing-plan-unexpected",
                "existing audio backing cannot install another process allocation",
            ));
        }
        self.audio
            .preflight_buffer_lock(&plan.audio, backing_memory_owner)
    }

    pub(crate) fn commit_audio_buffer_lock(
        &mut self,
        plan: RuntimeAudioBufferLockInstallationPlan,
        backing_memory_owner: Option<MemoryOwnerId>,
    ) -> Result<RuntimeAudioBufferLock, RuntimeStateError> {
        self.preflight_audio_buffer_lock(&plan, backing_memory_owner)?;
        if let Some(address_space) = plan.address_space {
            self.address_space_allocator
                .commit_reservations(address_space);
        }
        Ok(self
            .audio
            .commit_buffer_lock(plan.audio, backing_memory_owner))
    }

    pub(crate) fn plan_audio_buffer_unlock(
        &self,
        request: RuntimeAudioBufferUnlockRequest,
    ) -> Result<RuntimeAudioBufferUnlockPlan, RuntimeStateError> {
        self.require_audio_operation_state()?;
        self.audio.plan_buffer_unlock(request)
    }

    pub(crate) fn preflight_audio_buffer_unlock(
        &self,
        plan: &RuntimeAudioBufferUnlockPlan,
    ) -> Result<(), RuntimeStateError> {
        self.require_audio_operation_state()?;
        self.audio.preflight_buffer_unlock(plan)
    }

    pub(crate) fn commit_audio_buffer_unlock(
        &mut self,
        plan: RuntimeAudioBufferUnlockPlan,
    ) -> Result<RuntimeAudioBufferUnlockResult, RuntimeStateError> {
        self.preflight_audio_buffer_unlock(&plan)?;
        Ok(self.audio.commit_buffer_unlock(plan))
    }

    pub(crate) fn plan_audio_buffer_notifications(
        &self,
        request: RuntimeAudioBufferNotificationRequest,
    ) -> Result<RuntimeAudioBufferNotificationPlan, RuntimeStateError> {
        self.require_audio_operation_state()?;
        for notification in &request.notifications {
            let event = self.event_for_handle(notification.event_handle)?;
            if event.manual_reset {
                return Err(RuntimeStateError::invalid_input(
                    "runtime-audio",
                    "audio-notification-manual-reset-event-unsupported",
                    "reached DirectSound notifications require auto-reset event handles",
                ));
            }
        }
        self.audio.plan_buffer_notifications(request)
    }

    pub(crate) fn commit_audio_buffer_notifications(
        &mut self,
        plan: RuntimeAudioBufferNotificationPlan,
    ) -> Result<Vec<RuntimeAudioNotification>, RuntimeStateError> {
        self.require_audio_operation_state()?;
        self.audio.preflight_buffer_notifications(&plan)?;
        Ok(self.audio.commit_buffer_notifications(plan))
    }

    pub fn begin_audio_notification_wait(
        &mut self,
        handles: &[RuntimeHandle],
        timeout_milliseconds: u32,
    ) -> Result<RuntimeAudioNotificationWaitBegin, RuntimeStateError> {
        self.require_audio_operation_state()?;
        if timeout_milliseconds == 0 || timeout_milliseconds > 100 {
            return Err(RuntimeStateError::invalid_input(
                "runtime-audio",
                "audio-notification-wait-timeout-unsupported",
                "audio notification pacing supports finite waits from 1 through 100 milliseconds",
            ));
        }
        self.validate_audio_notification_wait_events(handles)?;
        let audio = match self
            .audio
            .plan_notification_wait(handles, timeout_milliseconds)
        {
            Ok(audio) => {
                self.audio.preflight_notification_wait(&audio)?;
                Some(audio)
            }
            Err(error) if error.code == "audio-notification-wait-buffer-not-playing" => None,
            Err(error) => return Err(error),
        };
        let scheduler = self
            .scheduler
            .plan_host_request(RUNTIME_AUDIO_NOTIFICATION_PACING_SERVICE.to_owned())?;
        let payload = audio.as_ref().map_or_else(
            || encode_inactive_audio_notification_wait_payload(handles, timeout_milliseconds),
            RuntimeAudioNotificationWaitPlan::encode_host_payload,
        );
        let delay_milliseconds = audio.as_ref().map_or(
            timeout_milliseconds,
            RuntimeAudioNotificationWaitPlan::delay_milliseconds,
        );
        let result_index = audio
            .as_ref()
            .and_then(RuntimeAudioNotificationWaitPlan::result_index);
        let request = self.scheduler.commit_host_request(scheduler);
        Ok(RuntimeAudioNotificationWaitBegin {
            request,
            delay_milliseconds,
            result_index,
            payload,
        })
    }

    pub(crate) fn plan_audio_notification_wait_completion(
        &self,
        request_id: u64,
        timeout_milliseconds: u32,
        payload: &[u8],
    ) -> Result<RuntimeAudioNotificationWaitCompletionPlan, RuntimeStateError> {
        // WaitForMultipleObjects captures its handle array when the call begins.
        // The guest may reuse or clear the source array while another thread runs,
        // so completion must use the bounded Rust-owned request payload instead of
        // rereading mutable guest memory.
        let handles = decode_audio_notification_wait_handles(payload, timeout_milliseconds)?;
        let scheduler = self.scheduler.plan_host_completion(request_id)?;
        if scheduler.request().service != RUNTIME_AUDIO_NOTIFICATION_PACING_SERVICE {
            return Err(RuntimeStateError::invalid_input(
                "runtime-audio",
                "audio-notification-host-service-mismatch",
                "audio notification completion does not match its pacing service",
            ));
        }
        let inactive_payload =
            encode_inactive_audio_notification_wait_payload(&handles, timeout_milliseconds);
        let (audio, result_index, expected_payload) = if payload == inactive_payload {
            (None, None, inactive_payload)
        } else {
            match self
                .audio
                .plan_notification_wait(&handles, timeout_milliseconds)
            {
                Ok(audio) => {
                    self.audio.preflight_notification_wait(&audio)?;
                    let result_index = audio.result_index();
                    let expected_payload = audio.encode_host_payload();
                    (Some(audio), result_index, expected_payload)
                }
                Err(error) if error.code == "audio-notification-wait-buffer-not-playing" => {
                    let result_index = self.audio.stopped_notification_result(
                        &handles,
                        timeout_milliseconds,
                        payload,
                    )?;
                    (None, Some(result_index), payload.to_vec())
                }
                Err(error) => return Err(error),
            }
        };
        Ok(RuntimeAudioNotificationWaitCompletionPlan {
            scheduler,
            audio,
            result_index,
            payload: expected_payload,
        })
    }

    pub(crate) fn commit_audio_notification_wait_completion(
        &mut self,
        plan: RuntimeAudioNotificationWaitCompletionPlan,
    ) -> (RuntimeHostCompletion, Option<u32>) {
        debug_assert!(
            plan.audio
                .as_ref()
                .is_none_or(|audio| self.audio.preflight_notification_wait(audio).is_ok())
        );
        let planned_result_index = plan.result_index;
        let result_index = plan
            .audio
            .map(|audio| self.audio.commit_notification_wait(audio))
            .unwrap_or(planned_result_index);
        let completion = self.scheduler.commit_host_completion(plan.scheduler);
        (completion, result_index)
    }

    fn validate_audio_notification_wait_events(
        &self,
        handles: &[RuntimeHandle],
    ) -> Result<(), RuntimeStateError> {
        for &handle in handles {
            let event = self.event_for_handle(handle)?;
            if event.manual_reset {
                return Err(RuntimeStateError::invalid_input(
                    "runtime-audio",
                    "audio-notification-wait-manual-reset-event-unsupported",
                    "reached DirectSound wait requires auto-reset event handles",
                ));
            }
        }
        Ok(())
    }

    pub(crate) fn plan_audio_buffer_play(
        &self,
        request: RuntimeAudioBufferPlayRequest,
    ) -> Result<RuntimeAudioBufferPlayPlan, RuntimeStateError> {
        self.require_audio_operation_state()?;
        self.audio.plan_buffer_play(request)
    }

    pub(crate) fn commit_audio_buffer_play(
        &mut self,
        plan: RuntimeAudioBufferPlayPlan,
    ) -> Result<RuntimeAudioBufferPlayResult, RuntimeStateError> {
        self.require_audio_operation_state()?;
        self.audio.preflight_buffer_play(&plan)?;
        Ok(self.audio.commit_buffer_play(plan))
    }

    pub(crate) fn plan_audio_buffer_control(
        &self,
        request: RuntimeAudioBufferControlRequest,
    ) -> Result<RuntimeAudioBufferControlPlan, RuntimeStateError> {
        self.require_audio_operation_state()?;
        self.audio.plan_buffer_control(request)
    }

    pub(crate) fn commit_audio_buffer_control(
        &mut self,
        plan: RuntimeAudioBufferControlPlan,
    ) -> Result<RuntimeAudioBufferControlResult, RuntimeStateError> {
        self.require_audio_operation_state()?;
        self.audio.preflight_buffer_control(&plan)?;
        Ok(self.audio.commit_buffer_control(plan))
    }

    pub(crate) fn plan_audio_cooperative_level(
        &self,
        request: RuntimeAudioCooperativeLevelRequest,
    ) -> Result<RuntimeAudioCooperativeLevelPlan, RuntimeStateError> {
        self.require_audio_operation_state()?;
        let target = self.resolve_audio_window_target(request.window_handle)?;
        self.audio
            .plan_cooperative_level(RuntimeAudioCooperativeLevelUpdateRequest {
                device_com_object_id: request.device_com_object_id,
                target,
                level: request.level,
            })
    }

    pub(crate) fn preflight_audio_cooperative_level(
        &self,
        plan: &RuntimeAudioCooperativeLevelPlan,
    ) -> Result<(), RuntimeStateError> {
        self.require_audio_operation_state()?;
        self.audio.preflight_cooperative_level(plan)?;
        let state = plan.next_state();
        let target = self.resolve_audio_window_target(state.window_handle)?;
        if target.window_id != state.window_id || target.owner_thread_id != state.owner_thread_id {
            return Err(RuntimeStateError::conflict(
                "runtime-audio",
                "audio-cooperative-window-plan-stale",
                "logical audio cooperative window changed after planning",
            ));
        }
        Ok(())
    }

    pub(crate) fn commit_audio_cooperative_level(
        &mut self,
        plan: RuntimeAudioCooperativeLevelPlan,
    ) -> Result<RuntimeAudioCooperativeLevelResult, RuntimeStateError> {
        self.preflight_audio_cooperative_level(&plan)?;
        Ok(self.audio.commit_cooperative_level(plan))
    }

    pub fn audio_device(
        &self,
        id: RuntimeAudioDeviceId,
    ) -> Result<&RuntimeAudioDevice, RuntimeStateError> {
        self.audio.device(id)
    }

    pub fn audio_devices(&self) -> &BTreeMap<RuntimeAudioDeviceId, RuntimeAudioDevice> {
        self.audio.devices()
    }

    pub fn audio_buffer(
        &self,
        id: RuntimeAudioBufferId,
    ) -> Result<&RuntimeAudioBuffer, RuntimeStateError> {
        self.audio.buffer(id)
    }

    pub fn audio_buffer_for_com_object(
        &self,
        com_object_id: RuntimeComObjectId,
    ) -> Result<&RuntimeAudioBuffer, RuntimeStateError> {
        self.audio.buffer_by_com_object(com_object_id)
    }

    pub fn audio_buffers(&self) -> &BTreeMap<RuntimeAudioBufferId, RuntimeAudioBuffer> {
        self.audio.buffers()
    }

    pub(crate) fn plan_graphics_device_creation(
        &self,
        request: RuntimeGraphicsDeviceCreateRequest,
    ) -> Result<RuntimeGraphicsDeviceCreationPlan, RuntimeStateError> {
        self.require_graphics_operation_state()?;
        let target = self.resolve_graphics_window_target(request.window_handle)?;
        self.graphics
            .plan_device_creation(RuntimeGraphicsDeviceCreationRequest {
                frontend: request.frontend,
                target,
                color_format: request.color_format,
                back_buffer_count: request.back_buffer_count,
                sample_count: request.sample_count,
                present_mode: request.present_mode,
                windowed: request.windowed,
            })
    }

    pub(crate) fn preflight_graphics_device_creation(
        &self,
        plan: &RuntimeGraphicsDeviceCreationPlan,
    ) -> Result<(), RuntimeStateError> {
        self.graphics.preflight_device_creation(plan)
    }

    pub(crate) fn commit_graphics_device_creation(
        &mut self,
        plan: RuntimeGraphicsDeviceCreationPlan,
        com_object_id: RuntimeComObjectId,
    ) -> RuntimeGraphicsDevice {
        self.graphics.commit_device_creation(plan, com_object_id)
    }

    pub fn graphics_device(
        &self,
        id: RuntimeGraphicsDeviceId,
    ) -> Result<&RuntimeGraphicsDevice, RuntimeStateError> {
        self.graphics.device(id)
    }

    pub fn graphics_devices(&self) -> &BTreeMap<RuntimeGraphicsDeviceId, RuntimeGraphicsDevice> {
        self.graphics.devices()
    }

    pub(crate) fn graphics_device_by_com_object(
        &self,
        com_object_id: RuntimeComObjectId,
    ) -> Result<&RuntimeGraphicsDevice, RuntimeStateError> {
        self.require_graphics_operation_state()?;
        self.graphics.device_by_com_object(com_object_id)
    }

    pub(crate) fn plan_graphics_render_state_update(
        &self,
        request: RuntimeGraphicsRenderStateUpdateRequest,
    ) -> Result<RuntimeGraphicsRenderStateUpdatePlan, RuntimeStateError> {
        self.require_graphics_operation_state()?;
        self.graphics.plan_render_state_update(request)
    }

    pub(crate) fn preflight_graphics_render_state_update(
        &self,
        plan: &RuntimeGraphicsRenderStateUpdatePlan,
    ) -> Result<(), RuntimeStateError> {
        self.graphics.preflight_render_state_update(plan)
    }

    pub(crate) fn commit_graphics_render_state_update(
        &mut self,
        plan: RuntimeGraphicsRenderStateUpdatePlan,
    ) -> RuntimeGraphicsRenderStateUpdateResult {
        self.graphics.commit_render_state_update(plan)
    }

    pub(crate) fn plan_graphics_sampler_state_update(
        &self,
        request: RuntimeGraphicsSamplerStateUpdateRequest,
    ) -> Result<RuntimeGraphicsSamplerStateUpdatePlan, RuntimeStateError> {
        self.require_graphics_operation_state()?;
        self.graphics.plan_sampler_state_update(request)
    }

    pub(crate) fn preflight_graphics_sampler_state_update(
        &self,
        plan: &RuntimeGraphicsSamplerStateUpdatePlan,
    ) -> Result<(), RuntimeStateError> {
        self.graphics.preflight_sampler_state_update(plan)
    }

    pub(crate) fn commit_graphics_sampler_state_update(
        &mut self,
        plan: RuntimeGraphicsSamplerStateUpdatePlan,
    ) -> RuntimeGraphicsSamplerStateUpdateResult {
        self.graphics.commit_sampler_state_update(plan)
    }

    pub(crate) fn plan_graphics_texture_stage_state_update(
        &self,
        request: RuntimeGraphicsTextureStageStateUpdateRequest,
    ) -> Result<RuntimeGraphicsTextureStageStateUpdatePlan, RuntimeStateError> {
        self.require_graphics_operation_state()?;
        self.graphics.plan_texture_stage_state_update(request)
    }

    pub(crate) fn preflight_graphics_texture_stage_state_update(
        &self,
        plan: &RuntimeGraphicsTextureStageStateUpdatePlan,
    ) -> Result<(), RuntimeStateError> {
        self.graphics.preflight_texture_stage_state_update(plan)
    }

    pub(crate) fn commit_graphics_texture_stage_state_update(
        &mut self,
        plan: RuntimeGraphicsTextureStageStateUpdatePlan,
    ) -> RuntimeGraphicsTextureStageStateUpdateResult {
        self.graphics.commit_texture_stage_state_update(plan)
    }

    pub(crate) fn plan_graphics_texture_binding_update(
        &self,
        request: RuntimeGraphicsTextureBindingUpdateRequest,
    ) -> Result<RuntimeGraphicsTextureBindingUpdatePlan, RuntimeStateError> {
        self.require_graphics_operation_state()?;
        self.graphics.plan_texture_binding_update(request)
    }

    pub(crate) fn preflight_graphics_texture_binding_update(
        &self,
        plan: &RuntimeGraphicsTextureBindingUpdatePlan,
    ) -> Result<(), RuntimeStateError> {
        self.graphics.preflight_texture_binding_update(plan)
    }

    pub(crate) fn commit_graphics_texture_binding_update(
        &mut self,
        plan: RuntimeGraphicsTextureBindingUpdatePlan,
    ) -> RuntimeGraphicsTextureBindingUpdateResult {
        self.graphics.commit_texture_binding_update(plan)
    }

    pub(crate) fn plan_graphics_flexible_vertex_format_update(
        &self,
        request: RuntimeGraphicsFlexibleVertexFormatUpdateRequest,
    ) -> Result<RuntimeGraphicsFlexibleVertexFormatUpdatePlan, RuntimeStateError> {
        self.require_graphics_operation_state()?;
        self.graphics.plan_flexible_vertex_format_update(request)
    }

    pub(crate) fn preflight_graphics_flexible_vertex_format_update(
        &self,
        plan: &RuntimeGraphicsFlexibleVertexFormatUpdatePlan,
    ) -> Result<(), RuntimeStateError> {
        self.graphics.preflight_flexible_vertex_format_update(plan)
    }

    pub(crate) fn commit_graphics_flexible_vertex_format_update(
        &mut self,
        plan: RuntimeGraphicsFlexibleVertexFormatUpdatePlan,
    ) -> RuntimeGraphicsFlexibleVertexFormatUpdateResult {
        self.graphics.commit_flexible_vertex_format_update(plan)
    }

    pub(crate) fn plan_graphics_stream_source_update(
        &self,
        request: RuntimeGraphicsStreamSourceUpdateRequest,
    ) -> Result<RuntimeGraphicsStreamSourceUpdatePlan, RuntimeStateError> {
        self.require_graphics_operation_state()?;
        self.graphics.plan_stream_source_update(request)
    }

    pub(crate) fn preflight_graphics_stream_source_update(
        &self,
        plan: &RuntimeGraphicsStreamSourceUpdatePlan,
    ) -> Result<(), RuntimeStateError> {
        self.graphics.preflight_stream_source_update(plan)
    }

    pub(crate) fn commit_graphics_stream_source_update(
        &mut self,
        plan: RuntimeGraphicsStreamSourceUpdatePlan,
    ) -> RuntimeGraphicsStreamSourceUpdateResult {
        self.graphics.commit_stream_source_update(plan)
    }

    pub(crate) fn plan_graphics_draw_capture(
        &self,
        request: RuntimeGraphicsDrawCaptureRequest,
    ) -> Result<RuntimeGraphicsDrawCapturePlan, RuntimeStateError> {
        self.require_graphics_operation_state()?;
        self.graphics.plan_draw_capture(request)
    }

    pub(crate) fn prepare_graphics_draw_submission(
        &self,
        capture: RuntimeGraphicsDrawCapturePlan,
        vertex_data: Vec<u8>,
    ) -> Result<RuntimeGraphicsDrawSubmissionPlan, RuntimeStateError> {
        self.require_graphics_operation_state()?;
        self.graphics.prepare_draw_submission(capture, vertex_data)
    }

    pub(crate) fn preflight_graphics_draw_submission(
        &self,
        plan: &RuntimeGraphicsDrawSubmissionPlan,
    ) -> Result<(), RuntimeStateError> {
        self.graphics.preflight_draw_submission(plan)
    }

    pub(crate) fn commit_graphics_draw_submission(
        &mut self,
        plan: RuntimeGraphicsDrawSubmissionPlan,
    ) -> RuntimeGraphicsDrawSubmissionResult {
        self.graphics.commit_draw_submission(plan)
    }

    pub(crate) fn plan_graphics_back_buffer_clear(
        &self,
        request: RuntimeGraphicsBackBufferClearRequest,
    ) -> Result<RuntimeGraphicsBackBufferClearPlan, RuntimeStateError> {
        self.require_graphics_operation_state()?;
        self.graphics.plan_back_buffer_clear(request)
    }

    pub(crate) fn preflight_graphics_back_buffer_clear(
        &self,
        plan: &RuntimeGraphicsBackBufferClearPlan,
    ) -> Result<(), RuntimeStateError> {
        self.graphics.preflight_back_buffer_clear(plan)
    }

    pub(crate) fn commit_graphics_back_buffer_clear(
        &mut self,
        plan: RuntimeGraphicsBackBufferClearPlan,
    ) -> RuntimeGraphicsBackBufferClearResult {
        self.graphics.commit_back_buffer_clear(plan)
    }

    pub(crate) fn plan_graphics_scene_begin(
        &self,
        request: RuntimeGraphicsSceneBeginRequest,
    ) -> Result<RuntimeGraphicsSceneBeginPlan, RuntimeStateError> {
        self.require_graphics_operation_state()?;
        self.graphics.plan_scene_begin(request)
    }

    pub(crate) fn preflight_graphics_scene_begin(
        &self,
        plan: &RuntimeGraphicsSceneBeginPlan,
    ) -> Result<(), RuntimeStateError> {
        self.graphics.preflight_scene_begin(plan)
    }

    pub(crate) fn commit_graphics_scene_begin(
        &mut self,
        plan: RuntimeGraphicsSceneBeginPlan,
    ) -> RuntimeGraphicsSceneBeginResult {
        self.graphics.commit_scene_begin(plan)
    }

    pub(crate) fn plan_graphics_scene_end(
        &self,
        request: RuntimeGraphicsSceneEndRequest,
    ) -> Result<RuntimeGraphicsSceneEndPlan, RuntimeStateError> {
        self.require_graphics_operation_state()?;
        self.graphics.plan_scene_end(request)
    }

    pub(crate) fn preflight_graphics_scene_end(
        &self,
        plan: &RuntimeGraphicsSceneEndPlan,
    ) -> Result<(), RuntimeStateError> {
        self.graphics.preflight_scene_end(plan)
    }

    pub(crate) fn commit_graphics_scene_end(
        &mut self,
        plan: RuntimeGraphicsSceneEndPlan,
    ) -> RuntimeGraphicsSceneEndResult {
        self.graphics.commit_scene_end(plan)
    }

    pub(crate) fn plan_graphics_present(
        &self,
        request: RuntimeGraphicsPresentRequest,
    ) -> Result<RuntimeGraphicsPresentPlan, RuntimeStateError> {
        self.require_graphics_operation_state()?;
        self.graphics.plan_present(request)
    }

    pub(crate) fn preflight_graphics_present(
        &self,
        plan: &RuntimeGraphicsPresentPlan,
    ) -> Result<(), RuntimeStateError> {
        self.graphics.preflight_present(plan)
    }

    pub(crate) fn commit_graphics_present(
        &mut self,
        plan: RuntimeGraphicsPresentPlan,
    ) -> RuntimeGraphicsPresentResult {
        self.graphics.commit_present(plan)
    }

    pub(crate) fn plan_graphics_texture_creation(
        &self,
        request: RuntimeGraphicsTextureCreateRequest,
    ) -> Result<RuntimeGraphicsTextureCreationPlan, RuntimeStateError> {
        self.require_graphics_operation_state()?;
        self.graphics.plan_texture_creation(request)
    }

    pub(crate) fn preflight_graphics_texture_creation(
        &self,
        plan: &RuntimeGraphicsTextureCreationPlan,
    ) -> Result<(), RuntimeStateError> {
        self.graphics.preflight_texture_creation(plan)
    }

    pub(crate) fn commit_graphics_texture_creation(
        &mut self,
        plan: RuntimeGraphicsTextureCreationPlan,
        com_object_id: RuntimeComObjectId,
    ) -> RuntimeGraphicsTexture {
        self.graphics.commit_texture_creation(plan, com_object_id)
    }

    pub(crate) fn plan_graphics_texture_lock(
        &self,
        request: RuntimeGraphicsTextureLockRequest,
    ) -> Result<RuntimeGraphicsTextureLockInstallationPlan, RuntimeStateError> {
        self.require_graphics_operation_state()?;
        let texture = self
            .graphics
            .texture_for_com_object(request.texture_com_object_id)?;
        let (row_pitch, byte_length) = texture_byte_layout(texture.descriptor)?;
        let address_space = if texture.backing.is_none() {
            let arena = self
                .address_space_allocator
                .allocation_for_purpose(&ProcessAddressAllocationPurpose::GraphicsResourceArena)?;
            if arena.range.base != super::RUNTIME_GRAPHICS_RESOURCE_ARENA_BASE
                || arena.range.byte_length != super::RUNTIME_GRAPHICS_RESOURCE_ARENA_SIZE
                || arena.classification != ProcessAddressAllocationClassification::Graphics
                || arena.lifetime != ProcessAddressAllocationLifetime::Process
                || arena.release_policy != ProcessAddressAllocationReleasePolicy::Permanent
                || arena.executable
            {
                return Err(RuntimeStateError::runtime_defect(
                    "runtime-graphics",
                    "graphics-resource-arena-invalid",
                    "graphics resource arena does not match the controlled process layout",
                ));
            }
            Some(self.address_space_allocator.plan_reservations(vec![(
                Some(arena.id),
                ProcessAddressAllocationRequest {
                    purpose: ProcessAddressAllocationPurpose::GraphicsResource,
                    owner: RUNTIME_GRAPHICS_RESOURCE_ADDRESS_OWNER.to_owned(),
                    preferred_address: None,
                    search_range: arena.range,
                    reserve_size: u64::from(byte_length),
                    alignment: GUEST_PAGE_SIZE as u32,
                    guard_pages: 0,
                    classification: ProcessAddressAllocationClassification::Graphics,
                    lifetime: ProcessAddressAllocationLifetime::Object,
                    release_policy: ProcessAddressAllocationReleasePolicy::Explicit,
                    executable: false,
                },
            )])?)
        } else {
            None
        };
        let backing_installation = address_space
            .as_ref()
            .and_then(|plan| plan.allocations().first())
            .map(|allocation| {
                GuestRange::new(allocation.range.base, allocation.range.byte_length)
                    .map(|range| RuntimeGraphicsTextureBackingPlan {
                        address_allocation_id: allocation.id,
                        range,
                        row_pitch,
                    })
                    .map_err(RuntimeStateError::from_memory)
            })
            .transpose()?;
        let graphics = self
            .graphics
            .plan_texture_lock(request, backing_installation)?;
        Ok(RuntimeGraphicsTextureLockInstallationPlan {
            address_space,
            graphics,
        })
    }

    pub(crate) fn preflight_graphics_texture_lock(
        &self,
        plan: &RuntimeGraphicsTextureLockInstallationPlan,
        backing_memory_owner: Option<MemoryOwnerId>,
    ) -> Result<(), RuntimeStateError> {
        self.require_graphics_operation_state()?;
        if let Some(address_space) = &plan.address_space {
            self.address_space_allocator
                .preflight_reservations_commit(address_space)?;
            let [allocation] = address_space.allocations() else {
                return Err(RuntimeStateError::runtime_defect(
                    "runtime-graphics",
                    "graphics-texture-lock-allocation-count-invalid",
                    "initial texture LockRect did not plan exactly one texel-backing allocation",
                ));
            };
            let arena = self
                .address_space_allocator
                .allocation_for_purpose(&ProcessAddressAllocationPurpose::GraphicsResourceArena)?;
            let backing = plan.graphics.backing_installation().ok_or_else(|| {
                RuntimeStateError::runtime_defect(
                    "runtime-graphics",
                    "graphics-texture-lock-backing-plan-missing",
                    "initial texture LockRect omitted its texel-backing installation",
                )
            })?;
            let texture = self.graphics.texture(plan.texture_id())?;
            let (_, byte_length) = texture_byte_layout(texture.descriptor)?;
            if allocation.parent_id != Some(arena.id)
                || allocation.id != backing.address_allocation_id
                || allocation.range.base != backing.range.start()
                || allocation.range.byte_length != backing.range.byte_length()
                || allocation.usable_byte_length != u64::from(byte_length)
                || allocation.purpose != ProcessAddressAllocationPurpose::GraphicsResource
                || allocation.owner != RUNTIME_GRAPHICS_RESOURCE_ADDRESS_OWNER
                || allocation.classification != ProcessAddressAllocationClassification::Graphics
                || allocation.lifetime != ProcessAddressAllocationLifetime::Object
                || allocation.release_policy != ProcessAddressAllocationReleasePolicy::Explicit
                || allocation.executable
            {
                return Err(RuntimeStateError::conflict(
                    "runtime-graphics",
                    "graphics-texture-backing-allocation-mismatch",
                    "logical texture backing does not match its planned process allocation",
                ));
            }
        } else if plan.graphics.backing_installation().is_some() {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-graphics",
                "graphics-texture-lock-backing-plan-unexpected",
                "existing texture backing cannot install another process allocation",
            ));
        }
        self.graphics
            .preflight_texture_lock(&plan.graphics, backing_memory_owner)
    }

    pub(crate) fn commit_graphics_texture_lock(
        &mut self,
        plan: RuntimeGraphicsTextureLockInstallationPlan,
        backing_memory_owner: Option<MemoryOwnerId>,
    ) -> Result<RuntimeGraphicsTextureLock, RuntimeStateError> {
        self.preflight_graphics_texture_lock(&plan, backing_memory_owner)?;
        if let Some(address_space) = plan.address_space {
            self.address_space_allocator
                .commit_reservations(address_space);
        }
        Ok(self
            .graphics
            .commit_texture_lock(plan.graphics, backing_memory_owner))
    }

    pub(crate) fn plan_graphics_texture_unlock(
        &self,
        texture_com_object_id: RuntimeComObjectId,
    ) -> Result<RuntimeGraphicsTextureUnlockPlan, RuntimeStateError> {
        self.require_graphics_operation_state()?;
        self.graphics.plan_texture_unlock(texture_com_object_id)
    }

    pub(crate) fn preflight_graphics_texture_unlock(
        &self,
        plan: &RuntimeGraphicsTextureUnlockPlan,
    ) -> Result<(), RuntimeStateError> {
        self.require_graphics_operation_state()?;
        self.graphics.preflight_texture_unlock(plan)
    }

    pub(crate) fn commit_graphics_texture_unlock(
        &mut self,
        plan: RuntimeGraphicsTextureUnlockPlan,
    ) -> RuntimeGraphicsTextureUnlockResult {
        self.graphics.commit_texture_unlock(plan)
    }

    pub(crate) fn plan_graphics_texture_dirty_region_mark(
        &self,
        request: RuntimeGraphicsTextureDirtyRegionMarkRequest,
    ) -> Result<RuntimeGraphicsTextureDirtyRegionMarkPlan, RuntimeStateError> {
        self.require_graphics_operation_state()?;
        self.graphics.plan_texture_dirty_region_mark(request)
    }

    pub(crate) fn preflight_graphics_texture_dirty_region_mark(
        &self,
        plan: &RuntimeGraphicsTextureDirtyRegionMarkPlan,
    ) -> Result<(), RuntimeStateError> {
        self.graphics.preflight_texture_dirty_region_mark(plan)
    }

    pub(crate) fn commit_graphics_texture_dirty_region_mark(
        &mut self,
        plan: RuntimeGraphicsTextureDirtyRegionMarkPlan,
    ) -> RuntimeGraphicsTextureDirtyRegionMarkResult {
        self.graphics.commit_texture_dirty_region_mark(plan)
    }

    pub(crate) fn plan_graphics_texture_update(
        &self,
        request: RuntimeGraphicsTextureUpdateRequest,
    ) -> Result<RuntimeGraphicsTextureUpdatePlan, RuntimeStateError> {
        self.require_graphics_operation_state()?;
        self.graphics.plan_texture_update(request)
    }

    pub(crate) fn preflight_graphics_texture_update(
        &self,
        plan: &RuntimeGraphicsTextureUpdatePlan,
    ) -> Result<(), RuntimeStateError> {
        self.graphics.preflight_texture_update(plan)
    }

    pub(crate) fn commit_graphics_texture_update(
        &mut self,
        plan: RuntimeGraphicsTextureUpdatePlan,
    ) -> RuntimeGraphicsTextureUpdateResult {
        self.graphics.commit_texture_update(plan)
    }

    pub fn graphics_texture(
        &self,
        id: RuntimeGraphicsTextureId,
    ) -> Result<&RuntimeGraphicsTexture, RuntimeStateError> {
        self.graphics.texture(id)
    }

    pub fn graphics_textures(&self) -> &BTreeMap<RuntimeGraphicsTextureId, RuntimeGraphicsTexture> {
        self.graphics.textures()
    }

    pub(crate) fn graphics_texture_for_com_object(
        &self,
        com_object_id: RuntimeComObjectId,
    ) -> Result<&RuntimeGraphicsTexture, RuntimeStateError> {
        self.require_graphics_operation_state()?;
        self.graphics.texture_for_com_object(com_object_id)
    }

    pub(crate) fn plan_graphics_vertex_buffer_installation(
        &self,
        com_address_request: ProcessAddressAllocationRequest,
        request: RuntimeGraphicsVertexBufferCreateRequest,
    ) -> Result<RuntimeGraphicsVertexBufferInstallationPlan, RuntimeStateError> {
        self.require_graphics_operation_state()?;
        validate_com_address_allocation_request(&com_address_request)?;
        let arena = self
            .address_space_allocator
            .allocation_for_purpose(&ProcessAddressAllocationPurpose::GraphicsResourceArena)?;
        if arena.range.base != super::RUNTIME_GRAPHICS_RESOURCE_ARENA_BASE
            || arena.range.byte_length != super::RUNTIME_GRAPHICS_RESOURCE_ARENA_SIZE
            || arena.classification != ProcessAddressAllocationClassification::Graphics
            || arena.lifetime != ProcessAddressAllocationLifetime::Process
            || arena.release_policy != ProcessAddressAllocationReleasePolicy::Permanent
            || arena.executable
        {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-graphics",
                "graphics-resource-arena-invalid",
                "graphics resource arena does not match the controlled process layout",
            ));
        }
        let address_space = self.address_space_allocator.plan_reservations(vec![
            (None, com_address_request),
            (
                Some(arena.id),
                ProcessAddressAllocationRequest {
                    purpose: ProcessAddressAllocationPurpose::GraphicsResource,
                    owner: RUNTIME_GRAPHICS_RESOURCE_ADDRESS_OWNER.to_owned(),
                    preferred_address: None,
                    search_range: arena.range,
                    reserve_size: u64::from(request.descriptor.byte_length),
                    alignment: GUEST_PAGE_SIZE as u32,
                    guard_pages: 0,
                    classification: ProcessAddressAllocationClassification::Graphics,
                    lifetime: ProcessAddressAllocationLifetime::Object,
                    release_policy: ProcessAddressAllocationReleasePolicy::Explicit,
                    executable: false,
                },
            ),
        ])?;
        let allocations = address_space.allocations();
        if allocations.len() != 2 {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-graphics",
                "graphics-vertex-buffer-installation-allocation-count-invalid",
                "vertex-buffer installation did not plan exactly two address allocations",
            ));
        }
        let resource_allocation = &allocations[1];
        let backing_range = GuestRange::new(
            resource_allocation.range.base,
            resource_allocation.range.byte_length,
        )
        .map_err(RuntimeStateError::from_memory)?;
        let graphics = self.graphics.plan_vertex_buffer_creation(
            request,
            resource_allocation.id,
            backing_range,
        )?;
        Ok(RuntimeGraphicsVertexBufferInstallationPlan {
            address_space,
            graphics,
        })
    }

    pub(crate) fn preflight_graphics_vertex_buffer_installation(
        &self,
        plan: &RuntimeGraphicsVertexBufferInstallationPlan,
        com: &RuntimeComMutationPlan,
    ) -> Result<(), RuntimeStateError> {
        self.address_space_allocator
            .preflight_reservations_commit(&plan.address_space)?;
        self.com.preflight(com)?;
        let result = com.registration_result();
        validate_com_installation_allocation(plan.com_allocation(), &result)?;
        self.graphics
            .preflight_vertex_buffer_creation(&plan.graphics)?;
        let allocation = plan.resource_allocation();
        let arena = self
            .address_space_allocator
            .allocation_for_purpose(&ProcessAddressAllocationPurpose::GraphicsResourceArena)?;
        if allocation.parent_id != Some(arena.id)
            || allocation.id != plan.graphics.address_allocation_id()
            || allocation.range.base != plan.graphics.backing_range().start()
            || allocation.range.byte_length != plan.graphics.backing_range().byte_length()
            || allocation.usable_byte_length != u64::from(plan.graphics.descriptor().byte_length)
            || allocation.purpose != ProcessAddressAllocationPurpose::GraphicsResource
            || allocation.owner != RUNTIME_GRAPHICS_RESOURCE_ADDRESS_OWNER
            || allocation.classification != ProcessAddressAllocationClassification::Graphics
            || allocation.lifetime != ProcessAddressAllocationLifetime::Object
            || allocation.release_policy != ProcessAddressAllocationReleasePolicy::Explicit
            || allocation.executable
        {
            return Err(RuntimeStateError::conflict(
                "runtime-graphics",
                "graphics-vertex-buffer-backing-allocation-mismatch",
                "logical vertex-buffer backing does not match its planned process allocation",
            ));
        }
        Ok(())
    }

    pub(crate) fn commit_graphics_vertex_buffer_installation(
        &mut self,
        plan: RuntimeGraphicsVertexBufferInstallationPlan,
        com: RuntimeComMutationPlan,
        memory_owner: MemoryOwnerId,
    ) -> Result<(RuntimeComRegistrationResult, RuntimeGraphicsVertexBuffer), RuntimeStateError>
    {
        self.preflight_graphics_vertex_buffer_installation(&plan, &com)?;
        if memory_owner.0 == 0 {
            return Err(RuntimeStateError::invalid_input(
                "runtime-graphics",
                "graphics-vertex-buffer-memory-owner-invalid",
                "logical vertex-buffer installation requires a nonzero guest-memory owner",
            ));
        }
        let result = com.registration_result();
        let com_object_id = result.object.id;
        self.address_space_allocator
            .commit_reservations(plan.address_space);
        self.com.commit(com);
        let vertex_buffer =
            self.graphics
                .commit_vertex_buffer_creation(plan.graphics, com_object_id, memory_owner);
        Ok((result, vertex_buffer))
    }

    pub(crate) fn plan_graphics_vertex_buffer_lock(
        &self,
        request: RuntimeGraphicsVertexBufferLockRequest,
    ) -> Result<RuntimeGraphicsVertexBufferLockPlan, RuntimeStateError> {
        self.require_graphics_operation_state()?;
        self.graphics.plan_vertex_buffer_lock(request)
    }

    pub(crate) fn preflight_graphics_vertex_buffer_lock(
        &self,
        plan: &RuntimeGraphicsVertexBufferLockPlan,
    ) -> Result<(), RuntimeStateError> {
        self.graphics.preflight_vertex_buffer_lock(plan)
    }

    pub(crate) fn commit_graphics_vertex_buffer_lock(
        &mut self,
        plan: RuntimeGraphicsVertexBufferLockPlan,
    ) -> RuntimeGraphicsVertexBufferLock {
        self.graphics.commit_vertex_buffer_lock(plan)
    }

    pub(crate) fn plan_graphics_vertex_buffer_unlock(
        &self,
        vertex_buffer_com_object_id: RuntimeComObjectId,
    ) -> Result<RuntimeGraphicsVertexBufferUnlockPlan, RuntimeStateError> {
        self.require_graphics_operation_state()?;
        self.graphics
            .plan_vertex_buffer_unlock(vertex_buffer_com_object_id)
    }

    pub(crate) fn preflight_graphics_vertex_buffer_unlock(
        &self,
        plan: &RuntimeGraphicsVertexBufferUnlockPlan,
    ) -> Result<(), RuntimeStateError> {
        self.graphics.preflight_vertex_buffer_unlock(plan)
    }

    pub(crate) fn commit_graphics_vertex_buffer_unlock(
        &mut self,
        plan: RuntimeGraphicsVertexBufferUnlockPlan,
    ) -> RuntimeGraphicsVertexBufferUnlockResult {
        self.graphics.commit_vertex_buffer_unlock(plan)
    }

    pub fn graphics_vertex_buffer(
        &self,
        id: RuntimeGraphicsVertexBufferId,
    ) -> Result<&RuntimeGraphicsVertexBuffer, RuntimeStateError> {
        self.graphics.vertex_buffer(id)
    }

    pub fn graphics_vertex_buffers(
        &self,
    ) -> &BTreeMap<RuntimeGraphicsVertexBufferId, RuntimeGraphicsVertexBuffer> {
        self.graphics.vertex_buffers()
    }

    pub(crate) fn plan_graphics_surface_creation(
        &self,
        request: RuntimeGraphicsSurfaceCreateRequest,
    ) -> Result<RuntimeGraphicsSurfaceCreationPlan, RuntimeStateError> {
        self.require_graphics_operation_state()?;
        self.graphics.plan_surface_creation(request)
    }

    pub(crate) fn preflight_graphics_surface_creation(
        &self,
        plan: &RuntimeGraphicsSurfaceCreationPlan,
    ) -> Result<(), RuntimeStateError> {
        self.graphics.preflight_surface_creation(plan)
    }

    pub(crate) fn commit_graphics_surface_creation(
        &mut self,
        plan: RuntimeGraphicsSurfaceCreationPlan,
        com_object_id: RuntimeComObjectId,
    ) -> RuntimeGraphicsSurface {
        self.graphics.commit_surface_creation(plan, com_object_id)
    }

    pub(crate) fn graphics_surface_for_texture_level(
        &self,
        texture_com_object_id: RuntimeComObjectId,
        level: u32,
    ) -> Result<Option<&RuntimeGraphicsSurface>, RuntimeStateError> {
        self.require_graphics_operation_state()?;
        self.graphics
            .surface_for_texture_level(texture_com_object_id, level)
    }

    pub fn graphics_surface(
        &self,
        id: RuntimeGraphicsSurfaceId,
    ) -> Result<&RuntimeGraphicsSurface, RuntimeStateError> {
        self.graphics.surface(id)
    }

    pub fn graphics_surfaces(&self) -> &BTreeMap<RuntimeGraphicsSurfaceId, RuntimeGraphicsSurface> {
        self.graphics.surfaces()
    }

    fn resolve_audio_window_target(
        &self,
        window_handle: u32,
    ) -> Result<RuntimeAudioWindowTarget, RuntimeStateError> {
        let window = self.user32.window(window_handle).map_err(|_| {
            RuntimeStateError::not_found(
                "runtime-audio",
                "audio-window-handle-unknown",
                format!("logical audio HWND {window_handle:#010x} is not live"),
            )
        })?;
        if window.lifecycle != RuntimeWindowLifecycle::Created {
            return Err(RuntimeStateError::conflict(
                "runtime-audio",
                "audio-window-lifecycle-state-invalid",
                "logical audio cooperative state requires a created window",
            ));
        }
        if window.creation.parent != 0 {
            return Err(RuntimeStateError::invalid_input(
                "runtime-audio",
                "audio-window-not-top-level",
                "logical audio cooperative state requires a top-level window",
            ));
        }
        if window.creation.owner_thread_id != self.current_thread_id() {
            return Err(RuntimeStateError::conflict(
                "runtime-audio",
                "audio-window-thread-mismatch",
                "logical audio cooperative state requires the window owner guest thread",
            ));
        }
        Ok(RuntimeAudioWindowTarget {
            window_id: window.id,
            window_handle: window.handle,
            owner_thread_id: window.creation.owner_thread_id,
        })
    }

    fn resolve_graphics_window_target(
        &self,
        window_handle: u32,
    ) -> Result<RuntimeGraphicsWindowTarget, RuntimeStateError> {
        let window = self.user32.window(window_handle).map_err(|_| {
            RuntimeStateError::not_found(
                "runtime-graphics",
                "graphics-window-handle-unknown",
                format!("logical graphics HWND {window_handle:#010x} is not live"),
            )
        })?;
        if window.lifecycle != RuntimeWindowLifecycle::Created {
            return Err(RuntimeStateError::conflict(
                "runtime-graphics",
                "graphics-window-lifecycle-state-invalid",
                "logical graphics device creation requires a created window",
            ));
        }
        if window.creation.owner_thread_id != self.current_thread_id() {
            return Err(RuntimeStateError::conflict(
                "runtime-graphics",
                "graphics-window-thread-mismatch",
                "logical graphics device creation requires the window owner guest thread",
            ));
        }
        Ok(RuntimeGraphicsWindowTarget {
            window_id: window.id,
            window_handle: window.handle,
            owner_thread_id: window.creation.owner_thread_id,
            client_width: window.creation.geometry.client_width,
            client_height: window.creation.geometry.client_height,
        })
    }

    fn resolve_gdi_window_target(
        &self,
        window_handle: u32,
    ) -> Result<RuntimeGdiWindowTarget, RuntimeStateError> {
        let window = self.user32.window(window_handle)?;
        if window.lifecycle != RuntimeWindowLifecycle::Created {
            return Err(RuntimeStateError::conflict(
                "runtime-gdi",
                "window-dc-lifecycle-state-invalid",
                "logical window DC access requires a created window",
            ));
        }
        if window.creation.owner_thread_id != self.current_thread_id() {
            return Err(RuntimeStateError::conflict(
                "runtime-gdi",
                "window-dc-thread-mismatch",
                "bounded logical window DC access requires the window owner guest thread",
            ));
        }
        Ok(RuntimeGdiWindowTarget {
            window_id: window.id,
            window_handle: window.handle,
            owner_thread_id: window.creation.owner_thread_id,
            client_width: window.creation.geometry.client_width,
            client_height: window.creation.geometry.client_height,
        })
    }

    fn resolve_gdi_screen_target(&self) -> Result<RuntimeGdiScreenTarget, RuntimeStateError> {
        let monitor = self.displays.primary_monitor().ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-gdi",
                "screen-dc-display-authority-missing",
                "screen DC access requires a configured logical primary monitor",
            )
        })?;
        if !monitor.primary {
            return Err(RuntimeStateError::runtime_defect(
                "runtime-gdi",
                "screen-dc-primary-monitor-invalid",
                "configured screen DC target is not the logical primary monitor",
            ));
        }
        Ok(RuntimeGdiScreenTarget {
            monitor_handle: monitor.handle,
            owner_thread_id: self.current_thread_id(),
            rect: monitor.rect,
        })
    }

    fn resolve_gdi_graphics_surface_target(
        &self,
        surface_com_object_id: RuntimeComObjectId,
    ) -> Result<RuntimeGdiGraphicsSurfaceTarget, RuntimeStateError> {
        let surface = self
            .graphics
            .surface_for_com_object(surface_com_object_id)?;
        Ok(RuntimeGdiGraphicsSurfaceTarget {
            surface_id: surface.id,
            surface_com_object_id: surface.com_object_id,
            owner_thread_id: self.current_thread_id(),
            width: surface.descriptor.width,
            height: surface.descriptor.height,
            color_format: surface.descriptor.color_format,
        })
    }

    pub(crate) fn resolve_small_icon_window_procedure(
        &self,
        window_handle: u32,
        icon_handle: u32,
        icon_width: u16,
        icon_height: u16,
    ) -> Result<RuntimeWindowProcedure, RuntimeStateError> {
        self.require_user32_operation_state()?;
        self.user32.resolve_small_icon_window_procedure(
            window_handle,
            self.current_thread_id(),
            icon_handle,
            icon_width,
            icon_height,
        )
    }

    pub(crate) fn set_window_small_icon(
        &mut self,
        window_handle: u32,
        icon_handle: u32,
        icon_width: u16,
        icon_height: u16,
    ) -> Result<u32, RuntimeStateError> {
        self.require_user32_operation_state()?;
        let plan = self.user32.plan_window_small_icon(
            window_handle,
            self.current_thread_id(),
            icon_handle,
            icon_width,
            icon_height,
        )?;
        self.user32.commit_window_small_icon(plan)
    }

    pub(crate) fn set_window_title(
        &mut self,
        window_handle: u32,
        title: RuntimeWindowNameA,
    ) -> Result<RuntimeWindow, RuntimeStateError> {
        self.require_user32_operation_state()?;
        let plan = self
            .user32
            .plan_window_title(window_handle, self.current_thread_id(), title)?;
        self.user32.commit_window_title(plan)
    }

    pub(crate) fn resolve_window_procedure(
        &self,
        window_handle: u32,
    ) -> Result<RuntimeWindowProcedure, RuntimeStateError> {
        self.require_user32_operation_state()?;
        self.user32
            .resolve_window_procedure(window_handle, self.current_thread_id())
    }

    pub(crate) fn preview_set_window_pos(
        &self,
        request: RuntimeSetWindowPosRequest,
    ) -> Result<RuntimeSetWindowPosOperation, RuntimeStateError> {
        self.require_user32_operation_state()?;
        self.user32
            .preview_set_window_pos(self.current_thread_id(), request)
    }

    pub(crate) fn preview_set_window_pos_callback(
        &self,
        operation: RuntimeSetWindowPosOperation,
        message: RuntimeSetWindowPosCallbackMessage,
        result: u32,
    ) -> Result<RuntimeSetWindowPosCallbackAction, RuntimeStateError> {
        self.require_user32_operation_state()?;
        self.require_window_owner_thread(operation.window_id)?;
        let plan = self
            .user32
            .plan_set_window_pos_callback(operation, message, result)?;
        if operation.variant == RuntimeSetWindowPosVariant::Resize
            && message == RuntimeSetWindowPosCallbackMessage::NonClientCalculate
        {
            self.gdi.plan_window_surface_resize(
                operation.window_id,
                operation.previous_geometry.client_width,
                operation.previous_geometry.client_height,
                operation.next_geometry.client_width,
                operation.next_geometry.client_height,
            )?;
        }
        Ok(self.user32.set_window_pos_callback_action(&plan).clone())
    }

    pub(crate) fn complete_set_window_pos_callback(
        &mut self,
        operation: RuntimeSetWindowPosOperation,
        message: RuntimeSetWindowPosCallbackMessage,
        result: u32,
    ) -> Result<RuntimeSetWindowPosCallbackAction, RuntimeStateError> {
        self.require_user32_operation_state()?;
        self.require_window_owner_thread(operation.window_id)?;
        let plan = self
            .user32
            .plan_set_window_pos_callback(operation, message, result)?;
        if operation.variant != RuntimeSetWindowPosVariant::Resize
            || message != RuntimeSetWindowPosCallbackMessage::NonClientCalculate
        {
            return self.user32.commit_set_window_pos_callback(plan);
        }

        let gdi_plan = self.gdi.plan_window_surface_resize(
            operation.window_id,
            operation.previous_geometry.client_width,
            operation.previous_geometry.client_height,
            operation.next_geometry.client_width,
            operation.next_geometry.client_height,
        )?;
        self.user32.preflight_set_window_pos_callback(&plan)?;
        self.gdi.preflight_window_surface_resize(&gdi_plan)?;
        self.gdi.commit_preflighted_window_surface_resize(gdi_plan);
        Ok(self.user32.commit_preflighted_set_window_pos_callback(plan))
    }

    pub(crate) fn preview_update_window(
        &self,
        request: RuntimeUpdateWindowRequest,
    ) -> Result<RuntimeUpdateWindowOperation, RuntimeStateError> {
        self.require_user32_operation_state()?;
        self.user32
            .preview_update_window(self.current_thread_id(), request)
    }

    pub(crate) fn preview_update_window_callback(
        &self,
        operation: RuntimeUpdateWindowOperation,
        result: u32,
    ) -> Result<RuntimeUpdateWindowCallbackAction, RuntimeStateError> {
        self.require_user32_operation_state()?;
        self.require_window_owner_thread(operation.window_id)?;
        let plan = self.user32.plan_update_window_callback(operation, result)?;
        Ok(self.user32.update_window_callback_action(&plan).clone())
    }

    pub(crate) fn complete_update_window_callback(
        &mut self,
        operation: RuntimeUpdateWindowOperation,
        result: u32,
    ) -> Result<RuntimeUpdateWindowCallbackAction, RuntimeStateError> {
        self.require_user32_operation_state()?;
        self.require_window_owner_thread(operation.window_id)?;
        let plan = self.user32.plan_update_window_callback(operation, result)?;
        self.user32.commit_update_window_callback(plan)
    }

    pub(crate) fn preview_set_window_pos_focus(
        &self,
        window_handle: u32,
    ) -> Result<RuntimeSetWindowPosDefaultOperation, RuntimeStateError> {
        self.require_user32_operation_state()?;
        let plan = self
            .user32
            .plan_set_window_pos_focus(window_handle, self.current_thread_id())?;
        Ok(self.user32.set_window_pos_focus_operation(&plan))
    }

    pub(crate) fn focus_set_window_pos_window(
        &mut self,
        window_handle: u32,
    ) -> Result<RuntimeSetWindowPosDefaultOperation, RuntimeStateError> {
        self.require_user32_operation_state()?;
        let plan = self
            .user32
            .plan_set_window_pos_focus(window_handle, self.current_thread_id())?;
        self.user32.commit_set_window_pos_focus(plan)
    }

    pub(crate) fn preview_set_window_pos_default_callback(
        &self,
        operation: RuntimeSetWindowPosDefaultOperation,
        message: RuntimeSetWindowPosDefaultCallbackMessage,
        result: u32,
    ) -> Result<RuntimeSetWindowPosDefaultCallbackAction, RuntimeStateError> {
        self.require_user32_operation_state()?;
        self.user32
            .set_window_pos_default_callback_action(operation, message, result)
    }

    pub(crate) fn complete_set_window_pos_default_callback(
        &mut self,
        operation: RuntimeSetWindowPosDefaultOperation,
        message: RuntimeSetWindowPosDefaultCallbackMessage,
        result: u32,
    ) -> Result<RuntimeSetWindowPosDefaultCallbackAction, RuntimeStateError> {
        self.require_user32_operation_state()?;
        self.user32
            .set_window_pos_default_callback_action(operation, message, result)
    }

    pub fn active_window_handle(&self) -> Result<Option<u32>, RuntimeStateError> {
        self.user32.active_window_handle(self.current_thread_id())
    }

    pub fn focused_window_handle(&self) -> Result<Option<u32>, RuntimeStateError> {
        self.user32.focused_window_handle(self.current_thread_id())
    }

    pub fn foreground_window_handle(&self) -> Result<Option<u32>, RuntimeStateError> {
        self.user32.foreground_window_handle()
    }

    pub(crate) fn set_focus_same_window(
        &self,
        window_handle: u32,
    ) -> Result<u32, RuntimeStateError> {
        self.require_user32_operation_state()?;
        self.user32
            .resolve_same_window_focus(window_handle, self.current_thread_id())
    }

    fn validate_window_creation_authority(
        &self,
        request: &RuntimeWindowCreationRequest,
    ) -> Result<(), RuntimeStateError> {
        if request.owner_thread_id != self.current_thread_id() {
            return Err(RuntimeStateError::invalid_input(
                "runtime-user32",
                "window-owner-thread-mismatch",
                "logical window owner must be the selected creating guest thread",
            ));
        }
        let image = self
            .modules
            .images()
            .image(request.instance.module_id)
            .map_err(runtime_module_image_error)?;
        if image.load_base != request.instance.load_base
            || image.source.sha256 != request.instance.source_sha256
        {
            return Err(RuntimeStateError::invalid_input(
                "runtime-user32",
                "window-module-identity-mismatch",
                "logical window instance does not match its registered module image",
            ));
        }
        Ok(())
    }

    fn require_window_owner_thread(
        &self,
        window_id: RuntimeWindowId,
    ) -> Result<(), RuntimeStateError> {
        if self
            .user32
            .window_by_id(window_id)?
            .creation
            .owner_thread_id
            != self.current_thread_id()
        {
            return Err(RuntimeStateError::conflict(
                "runtime-user32",
                "window-callback-thread-mismatch",
                "logical window creation callback returned on a different guest thread",
            ));
        }
        Ok(())
    }

    fn require_user32_operation_state(&self) -> Result<(), RuntimeStateError> {
        if !self.process.is_running()
            || self.current_thread()?.lifecycle != RuntimeThreadLifecycleState::Running
            || !matches!(
                self.scheduler_state()?,
                SchedulerThreadState::Runnable | SchedulerThreadState::InCallback { .. }
            )
        {
            return Err(RuntimeStateError::conflict(
                "runtime-user32",
                "user32-operation-state-invalid",
                "User32 resource operation requires a running process and active current thread",
            ));
        }
        Ok(())
    }

    fn require_gdi_operation_state(&self) -> Result<(), RuntimeStateError> {
        if !self.process.is_running()
            || self.current_thread()?.lifecycle != RuntimeThreadLifecycleState::Running
            || !matches!(
                self.scheduler_state()?,
                SchedulerThreadState::Runnable | SchedulerThreadState::InCallback { .. }
            )
        {
            return Err(RuntimeStateError::conflict(
                "runtime-gdi",
                "gdi-operation-state-invalid",
                "GDI operation requires a running process and active current thread",
            ));
        }
        Ok(())
    }

    fn require_imm32_operation_state(&self) -> Result<(), RuntimeStateError> {
        if !self.process.is_running()
            || self.current_thread()?.lifecycle != RuntimeThreadLifecycleState::Running
            || !matches!(
                self.scheduler_state()?,
                SchedulerThreadState::Runnable | SchedulerThreadState::InCallback { .. }
            )
        {
            return Err(RuntimeStateError::conflict(
                "runtime-imm32",
                "imm32-operation-state-invalid",
                "IMM32 operation requires a running process and active current thread",
            ));
        }
        Ok(())
    }

    fn require_gdiplus_operation_state(&self) -> Result<(), RuntimeStateError> {
        if !self.process.is_running()
            || self.current_thread()?.lifecycle != RuntimeThreadLifecycleState::Running
            || !matches!(
                self.scheduler_state()?,
                SchedulerThreadState::Runnable | SchedulerThreadState::InCallback { .. }
            )
        {
            return Err(RuntimeStateError::conflict(
                "runtime-gdiplus",
                "gdiplus-operation-state-invalid",
                "GDI+ operation requires a running process and active current thread",
            ));
        }
        Ok(())
    }

    fn require_audio_operation_state(&self) -> Result<(), RuntimeStateError> {
        if !self.process.is_running()
            || self.current_thread()?.lifecycle != RuntimeThreadLifecycleState::Running
            || !matches!(
                self.scheduler_state()?,
                SchedulerThreadState::Runnable | SchedulerThreadState::InCallback { .. }
            )
        {
            return Err(RuntimeStateError::conflict(
                "runtime-audio",
                "audio-operation-state-invalid",
                "audio operation requires a running process and active current thread",
            ));
        }
        Ok(())
    }

    fn require_graphics_operation_state(&self) -> Result<(), RuntimeStateError> {
        if !self.process.is_running()
            || self.current_thread()?.lifecycle != RuntimeThreadLifecycleState::Running
            || !matches!(
                self.scheduler_state()?,
                SchedulerThreadState::Runnable | SchedulerThreadState::InCallback { .. }
            )
        {
            return Err(RuntimeStateError::conflict(
                "runtime-graphics",
                "graphics-operation-state-invalid",
                "graphics operation requires a running process and active current thread",
            ));
        }
        Ok(())
    }

    fn require_waitable_timer_operation_state(&self) -> Result<(), RuntimeStateError> {
        if !self.process.is_running()
            || self.current_thread()?.lifecycle != RuntimeThreadLifecycleState::Running
            || !matches!(
                self.scheduler_state()?,
                SchedulerThreadState::Runnable | SchedulerThreadState::InCallback { .. }
            )
        {
            return Err(RuntimeStateError::conflict(
                "runtime-waitable-timer",
                "waitable-timer-operation-state-invalid",
                "waitable timer operation requires a running process and active current thread",
            ));
        }
        Ok(())
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
        self.synchronization.mutex(RuntimeMutexId(*mutex_id))
    }

    pub fn release_mutex(
        &mut self,
        handle: RuntimeHandle,
    ) -> Result<&RuntimeMutex, RuntimeStateError> {
        self.require_mutex_operation_state()?;
        let id = self.mutex_for_handle(handle)?.id;
        let plan = self
            .synchronization
            .plan_mutex_release(id, self.current_thread_id())?;
        Ok(self.synchronization.commit_mutex_transition(plan))
    }

    fn require_mutex_operation_state(&self) -> Result<(), RuntimeStateError> {
        if !self.process.is_running()
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

    fn require_event_operation_state(&self) -> Result<(), RuntimeStateError> {
        if !self.process.is_running()
            || self.current_thread()?.lifecycle != RuntimeThreadLifecycleState::Running
            || !matches!(
                self.scheduler_state()?,
                SchedulerThreadState::Runnable | SchedulerThreadState::InCallback { .. }
            )
        {
            return Err(RuntimeStateError::conflict(
                "runtime-event",
                "event-operation-state-invalid",
                "event operation requires a running process and active current thread",
            ));
        }
        Ok(())
    }

    fn plan_payload_object_teardown(
        &self,
        cancelling_source_reads: &BTreeSet<u64>,
    ) -> Result<RuntimePayloadObjectTeardownPlan, RuntimeStateError> {
        let mutex_ids = self.synchronization.mutex_ids();
        let waitable_timer_ids = self.timers.waitable_timer_ids();
        let source_file_ids = self.files.open_file_ids().collect::<Vec<_>>();
        let file_search_ids = self.files.file_search_ids().collect::<Vec<_>>();
        let mut keys = mutex_ids
            .iter()
            .map(|id| RuntimeObjectKey::Mutex { mutex_id: id.0 })
            .collect::<Vec<_>>();
        keys.extend(
            waitable_timer_ids
                .iter()
                .map(|id| RuntimeObjectKey::WaitableTimer { timer_id: id.0 }),
        );
        keys.extend(
            self.objects
                .objects()
                .values()
                .filter(|record| record.kind == RuntimeObjectKind::Event)
                .map(|record| record.key.clone()),
        );
        keys.extend(
            source_file_ids
                .iter()
                .map(|id| RuntimeObjectKey::File { file_id: id.0 }),
        );
        let file_search_keys = file_search_ids
            .iter()
            .map(|id| RuntimeObjectKey::FileSearch { search_id: id.0 })
            .collect::<Vec<_>>();
        let mutex_plans = mutex_ids
            .iter()
            .map(|id| self.synchronization.plan_mutex_retirement(*id))
            .collect::<Result<Vec<_>, _>>()?;
        let waitable_timer_plans = waitable_timer_ids
            .iter()
            .map(|id| self.timers.plan_waitable_timer_retirement(*id))
            .collect::<Result<Vec<_>, _>>()?;
        let source_file_plans = source_file_ids
            .iter()
            .map(|id| {
                self.files
                    .plan_process_retirement(*id, cancelling_source_reads)
            })
            .collect::<Result<Vec<_>, _>>()?;
        for plan in &source_file_plans {
            self.files.preflight_retirement(plan)?;
        }
        let file_search_plans = file_search_ids
            .iter()
            .map(|id| self.files.plan_file_search_retirement(*id))
            .collect::<Result<Vec<_>, _>>()?;
        for plan in &file_search_plans {
            self.files.preflight_file_search_retirement(plan)?;
        }
        let object_plan = (!keys.is_empty() || !file_search_keys.is_empty())
            .then(|| {
                self.objects
                    .plan_payload_object_teardown(&keys, &file_search_keys)
            })
            .transpose()?;
        let retired_handles = object_plan
            .as_ref()
            .map_or(0, RuntimeCloseHandleObjectMutationPlan::retired_handles);
        Ok(RuntimePayloadObjectTeardownPlan {
            object_plan,
            mutex_plans,
            waitable_timer_plans,
            source_file_plans,
            file_search_plans,
            retired_handles,
        })
    }

    fn commit_payload_object_teardown(
        &mut self,
        plan: RuntimePayloadObjectTeardownPlan,
    ) -> Result<usize, RuntimeStateError> {
        if let Some(object_plan) = plan.object_plan {
            self.objects
                .commit_close_handle_object_mutation(object_plan)?;
        }
        for plan in plan.mutex_plans {
            self.synchronization.commit_mutex_retirement(plan);
        }
        for plan in plan.waitable_timer_plans {
            self.timers.commit_waitable_timer_retirement(plan);
        }
        for plan in plan.source_file_plans {
            self.files.commit_retirement(plan);
        }
        for plan in plan.file_search_plans {
            self.files.commit_file_search_retirement(plan);
        }
        Ok(plan.retired_handles)
    }

    pub fn initialize_critical_section(
        &mut self,
        address: GuestAddress,
        spin_count: u32,
    ) -> Result<&RuntimeCriticalSection, RuntimeStateError> {
        self.require_critical_section_operation_state()?;
        let plan = self
            .synchronization
            .plan_initialization(address, spin_count)?;
        if spin_count > 0x00ff_ffff {
            return Err(RuntimeStateError::invalid_input(
                "runtime-critical-section",
                "critical-section-spin-count-invalid",
                "critical section spin count exceeds the Win32 24-bit range",
            ));
        }
        self.synchronization.commit(plan)
    }

    pub fn initialize_critical_section_with_spin_state(
        &mut self,
        address: GuestAddress,
        spin_count_field: u32,
    ) -> Result<&RuntimeCriticalSection, RuntimeStateError> {
        self.require_critical_section_operation_state()?;
        let plan = self
            .synchronization
            .plan_initialization(address, spin_count_field)?;
        self.synchronization.commit(plan)
    }

    pub fn enter_critical_section(
        &mut self,
        address: GuestAddress,
    ) -> Result<&RuntimeCriticalSection, RuntimeStateError> {
        self.require_critical_section_operation_state()?;
        let current_thread_id = self.current_thread_id();
        let plan = self
            .synchronization
            .plan_enter(address, current_thread_id)?;
        self.synchronization.commit(plan)
    }

    pub fn wait_current_thread_for_critical_section(
        &mut self,
        address: GuestAddress,
    ) -> Result<(), RuntimeStateError> {
        self.require_critical_section_operation_state()?;
        let current_thread_id = self.current_thread_id();
        let executable_competitor = self
            .scheduler
            .plan_next_other_executable_selection()?
            .is_some();
        let owner_thread_id = self
            .synchronization
            .critical_section(address)?
            .owning_thread_id
            .ok_or_else(|| {
                RuntimeStateError::conflict(
                    "runtime-critical-section",
                    "critical-section-wait-not-contended",
                    "critical section wait requires a live owner",
                )
            })?;
        let owner_host_wait = match self.scheduler.state(owner_thread_id)? {
            SchedulerThreadState::WaitingHost { request_id } => self
                .scheduler
                .host_requests()
                .get(&request_id)
                .is_some_and(|request| request.thread_id == owner_thread_id),
            _ => false,
        };
        if !executable_competitor && !owner_host_wait {
            return Err(RuntimeStateError::conflict(
                "runtime-critical-section",
                "critical-section-no-executable-owner",
                "contended critical section has neither an executable lane nor a host-waiting owner",
            ));
        }
        let critical_section_plan = self.synchronization.plan_wait(address, current_thread_id)?;
        let scheduler_plan = self.scheduler.plan_transition(
            current_thread_id,
            SchedulerThreadState::WaitingCriticalSection { address: address.0 },
        )?;
        self.synchronization.preflight(&critical_section_plan)?;
        self.scheduler.preflight_lane_commit(&scheduler_plan)?;
        self.synchronization.commit(critical_section_plan)?;
        self.scheduler.commit_lane(scheduler_plan);
        Ok(())
    }

    pub fn leave_critical_section(
        &mut self,
        address: GuestAddress,
    ) -> Result<&RuntimeCriticalSection, RuntimeStateError> {
        self.require_critical_section_operation_state()?;
        let current_thread_id = self.current_thread_id();
        let plan = self
            .synchronization
            .plan_leave(address, current_thread_id)?;
        let awakened_thread_id = plan.awakened_thread_id();
        let scheduler_plan = awakened_thread_id
            .map(|thread_id| {
                if self.thread_state(thread_id)?.lifecycle != RuntimeThreadLifecycleState::Running
                    || self.scheduler_state_for_thread(thread_id)?
                        != (SchedulerThreadState::WaitingCriticalSection { address: address.0 })
                {
                    return Err(RuntimeStateError::runtime_defect(
                        "runtime-critical-section",
                        "critical-section-waiter-state-mismatch",
                        "critical section waiter does not have a matching live scheduler lane",
                    ));
                }
                self.scheduler
                    .plan_transition(thread_id, SchedulerThreadState::Runnable)
            })
            .transpose()?;
        self.synchronization.preflight(&plan)?;
        if let Some(scheduler_plan) = scheduler_plan.as_ref() {
            self.scheduler.preflight_lane_commit(scheduler_plan)?;
        }
        self.synchronization.commit(plan)?;
        if let Some(scheduler_plan) = scheduler_plan {
            self.scheduler.commit_lane(scheduler_plan);
        }
        self.synchronization.critical_section(address)
    }

    pub fn delete_critical_section(
        &mut self,
        address: GuestAddress,
    ) -> Result<RuntimeCriticalSection, RuntimeStateError> {
        self.require_critical_section_operation_state()?;
        let plan = self.synchronization.plan_retirement(address)?;
        self.synchronization.commit_retirement(plan)
    }

    pub fn critical_section(
        &self,
        address: GuestAddress,
    ) -> Result<&RuntimeCriticalSection, RuntimeStateError> {
        self.synchronization.critical_section(address)
    }

    pub fn critical_section_waiters(&self, address: GuestAddress) -> Vec<RuntimeThreadId> {
        self.synchronization.critical_section_waiters(address)
    }

    fn require_critical_section_operation_state(&self) -> Result<(), RuntimeStateError> {
        if !self.process.is_running()
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
        self.registries.encoded_pointer_namespace()
    }

    pub fn encode_pointer(
        &mut self,
        source: u32,
    ) -> Result<&RuntimeEncodedPointer, RuntimeStateError> {
        let plan = self.registries.plan_encode_pointer(source)?;
        Ok(self.registries.commit_encode_pointer(plan))
    }

    pub fn encoded_pointer_for_source(
        &self,
        source: u32,
    ) -> Result<&RuntimeEncodedPointer, RuntimeStateError> {
        self.registries.encoded_pointer_for_source(source)
    }

    pub fn decode_pointer(&self, token: u32) -> Result<&RuntimeEncodedPointer, RuntimeStateError> {
        self.registries.decode_pointer(token)
    }

    pub fn push_callback_frame(
        &mut self,
        target: GuestAddress,
        return_sentinel: GuestAddress,
    ) -> Result<RuntimeCallbackFrame, RuntimeStateError> {
        let plan = self.plan_callback_push(target, return_sentinel, 0)?;
        Ok(self
            .commit_callback_mutation(plan)
            .0
            .expect("callback push plan did not enter a frame"))
    }

    pub(crate) fn callback_scratch_allocation(
        &self,
    ) -> Result<&ProcessAddressAllocationRecord, RuntimeStateError> {
        self.address_space_allocator
            .allocation_for_purpose(&ProcessAddressAllocationPurpose::CallbackScratch)
    }

    pub fn validate_callback_frame_entry(
        &self,
        target: GuestAddress,
        return_sentinel: GuestAddress,
    ) -> Result<(), RuntimeStateError> {
        self.plan_callback_push(target, return_sentinel, 0)
            .map(drop)
    }

    pub(crate) fn plan_callback_push(
        &self,
        target: GuestAddress,
        return_sentinel: GuestAddress,
        scratch_bytes: u32,
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
        let callback_frame_id = self.threads.plan_callback_frame_id()?;
        let depth = current_depth.checked_add(1).ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-callback",
                "callback-depth-overflow",
                "callback depth overflowed host accounting",
            )
        })?;
        let frame = RuntimeCallbackFrame {
            id: callback_frame_id.id(),
            thread_id,
            target,
            return_sentinel,
            entry_last_error: self.current_last_error()?,
            scratch: callback_scratch_for_depth(depth, scratch_bytes)?,
        };
        let (thread_plan, depth) = self.threads.plan_callback_push(thread_id, frame)?;
        let scheduler_plan = self
            .scheduler
            .plan_transition(thread_id, SchedulerThreadState::InCallback { depth })?;
        Ok(RuntimeCallbackMutationPlan {
            thread: thread_plan,
            scheduler: scheduler_plan,
            callback_frame_id: Some(callback_frame_id),
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

    pub fn validate_callback_frame_return(
        &self,
        expected: RuntimeCallbackFrameId,
    ) -> Result<(), RuntimeStateError> {
        self.plan_callback_pop(expected).map(drop)
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
            callback_frame_id: None,
            entered: None,
            exited: Some(frame),
        })
    }

    pub(crate) fn plan_callback_replace(
        &self,
        expected: RuntimeCallbackFrameId,
        target: GuestAddress,
        return_sentinel: GuestAddress,
        scratch_bytes: u32,
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
        let callback_frame_id = self.threads.plan_callback_frame_id()?;
        let frame = RuntimeCallbackFrame {
            id: callback_frame_id.id(),
            thread_id,
            target,
            return_sentinel,
            entry_last_error: self.current_last_error()?,
            scratch: callback_scratch_for_depth(current_depth, scratch_bytes)?,
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
            callback_frame_id: Some(callback_frame_id),
            entered: Some(frame),
            exited: Some(exited),
        })
    }

    pub fn validate_callback_frame_replacement(
        &self,
        expected: RuntimeCallbackFrameId,
        target: GuestAddress,
        return_sentinel: GuestAddress,
    ) -> Result<(), RuntimeStateError> {
        self.plan_callback_replace(expected, target, return_sentinel, 0)
            .map(drop)
    }

    pub fn replace_callback_frame(
        &mut self,
        expected: RuntimeCallbackFrameId,
        target: GuestAddress,
        return_sentinel: GuestAddress,
    ) -> Result<RuntimeCallbackFrame, RuntimeStateError> {
        let plan = self.plan_callback_replace(expected, target, return_sentinel, 0)?;
        Ok(self
            .commit_callback_mutation(plan)
            .0
            .expect("callback replacement plan did not enter a frame"))
    }

    pub(crate) fn commit_callback_mutation(
        &mut self,
        plan: RuntimeCallbackMutationPlan,
    ) -> (Option<RuntimeCallbackFrame>, Option<RuntimeCallbackFrame>) {
        let RuntimeCallbackMutationPlan {
            thread,
            scheduler,
            callback_frame_id,
            entered,
            exited,
        } = plan;
        self.threads
            .commit_callback_mutation(callback_frame_id, thread);
        self.scheduler.commit_lane(scheduler);
        (entered, exited)
    }

    pub fn begin_host_request(
        &mut self,
        service: impl Into<String>,
    ) -> Result<RuntimeHostRequest, RuntimeStateError> {
        let service = service.into();
        if service == RUNTIME_WAITABLE_TIMER_PACING_SERVICE {
            return Err(RuntimeStateError::invalid_input(
                "runtime-host-service",
                "waitable-timer-pacing-service-reserved",
                "timer pacing requests must be created with their authoritative wait registration",
            ));
        }
        let plan = self.scheduler.plan_host_request(service)?;
        Ok(self.scheduler.commit_host_request(plan))
    }

    pub fn complete_host_request(
        &mut self,
        request_id: u64,
    ) -> Result<RuntimeHostCompletion, RuntimeStateError> {
        let plan = self.scheduler.plan_host_completion(request_id)?;
        if matches!(
            plan.request().service.as_str(),
            RUNTIME_WAITABLE_TIMER_PACING_SERVICE | RUNTIME_AUDIO_NOTIFICATION_PACING_SERVICE
        ) {
            return Err(RuntimeStateError::invalid_input(
                "runtime-host-service",
                "specialized-pacing-completion-required",
                "pacing completion must atomically commit its subsystem and scheduler state",
            ));
        }
        Ok(self.scheduler.commit_host_completion(plan))
    }

    pub fn pop_scheduler_event(&mut self) -> Option<RuntimeSchedulerEvent> {
        self.scheduler.pop_event()
    }

    pub fn request_current_thread_exit(
        &mut self,
        exit_code: u32,
    ) -> Result<RuntimeCancellation, RuntimeStateError> {
        if !self.process.is_running() {
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
        if !self.process.is_running() {
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

    pub fn complete_current_guest_thread_exit(
        &mut self,
        memory: &mut GuestMemory,
    ) -> Result<(), RuntimeStateError> {
        let thread_id = self.current_thread_id();
        let thread = self.thread_state(thread_id)?.clone();
        if thread.guest_bootstrap.is_none() {
            return self.complete_current_thread_exit();
        }
        let stack = thread.stack.ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-thread-bootstrap",
                "guest-thread-stack-missing-at-exit",
                "dynamically created guest thread lost its stack authority before exit",
            )
        })?;
        let environment = thread.environment.ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-thread-bootstrap",
                "guest-thread-environment-missing-at-exit",
                "dynamically created guest thread lost its TEB authority before exit",
            )
        })?;
        let completion = self.plan_thread_exit_completion(thread_id)?;
        let wait_updates = self.objects.plan_wait_state_updates(&[(
            RuntimeObjectKey::Thread {
                thread_id: completion.thread_id.0,
            },
            RuntimeWaitState::Signaled {
                exit_code: completion.exit_code,
            },
        )])?;
        let wake_completions = self.plan_waiter_wakes(&wait_updates, &[completion.thread_id])?;
        let address_releases = self.address_space_allocator.plan_releases(&[
            stack.address_allocation_id,
            environment.address_allocation_id,
        ])?;

        let mut transaction = memory.begin_transaction();
        transaction
            .release_region(stack.memory_owner, stack.reserved_range.start())
            .map_err(RuntimeStateError::from_memory)?;
        transaction
            .destroy_owner(stack.memory_owner)
            .map_err(RuntimeStateError::from_memory)?;
        transaction
            .release_region(environment.memory_owner, environment.range.start())
            .map_err(RuntimeStateError::from_memory)?;
        transaction
            .destroy_owner(environment.memory_owner)
            .map_err(RuntimeStateError::from_memory)?;
        self.address_space_allocator
            .preflight_releases(&address_releases)?;

        self.objects.commit_wait_state_updates(wait_updates);
        self.commit_thread_exit_completion(completion);
        self.commit_waiter_wakes(wake_completions);
        self.address_space_allocator
            .commit_releases(address_releases);
        transaction.commit_transaction();
        Ok(())
    }

    pub fn cancel_current_thread(
        &mut self,
        exit_code: u32,
    ) -> Result<RuntimeCancellation, RuntimeStateError> {
        if !self.process.is_running() {
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
        let plan = self.plan_process_exit_request(exit_code)?;
        let cancellation = plan.cancellation.clone();
        self.commit_process_exit_request(plan)?;
        Ok(cancellation)
    }

    pub fn complete_current_process_exit(&mut self) -> Result<(), RuntimeStateError> {
        let initiating_thread_id = self.current_thread_id();
        let exit_code = self.process.exit_request_code(initiating_thread_id)?;
        let process_plan = self.process.plan_exit_completion(initiating_thread_id)?;
        if !self.heaps.is_empty() {
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
                process_id: self.process.id().0,
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
        let payload_teardown = self.plan_payload_object_teardown(&BTreeSet::new())?;
        let graphics_teardown = self.graphics.plan_process_teardown()?;
        let gdi_teardown = self.gdi.plan_process_teardown()?;
        let gdiplus_teardown = self.gdiplus.plan_process_teardown()?;
        let imm32_teardown = self.imm32.plan_process_teardown()?;
        let user32_teardown = self.user32.plan_process_teardown()?;
        self.process.preflight_mutation_commit(&process_plan)?;
        if let Some(gdi_plan) = gdi_teardown.as_ref() {
            self.gdi.preflight_process_teardown(gdi_plan)?;
        }
        if let Some(gdiplus_plan) = gdiplus_teardown.as_ref() {
            self.gdiplus.preflight_process_teardown(gdiplus_plan)?;
        }
        if let Some(imm32_plan) = imm32_teardown.as_ref() {
            self.imm32.preflight_process_teardown(imm32_plan)?;
        }
        if let Some(graphics_plan) = graphics_teardown.as_ref() {
            self.graphics.preflight_process_teardown(graphics_plan)?;
        }
        if let Some(user32_plan) = user32_teardown.as_ref() {
            self.user32.preflight_process_teardown(user32_plan)?;
        }
        self.commit_payload_object_teardown(payload_teardown)?;
        if let Some(gdi_plan) = gdi_teardown {
            self.gdi.commit_process_teardown(gdi_plan);
        }
        if let Some(gdiplus_plan) = gdiplus_teardown {
            self.gdiplus.commit_process_teardown(gdiplus_plan);
        }
        if let Some(imm32_plan) = imm32_teardown {
            self.imm32.commit_process_teardown(imm32_plan);
        }
        if let Some(graphics_plan) = graphics_teardown {
            self.graphics.commit_process_teardown(graphics_plan);
        }
        if let Some(user32_plan) = user32_teardown {
            self.user32.commit_process_teardown(user32_plan);
        }
        self.objects.commit_wait_state_updates(wait_updates);
        for completion in completions {
            self.commit_thread_exit_completion(completion);
        }
        self.process.commit_mutation(process_plan);
        Ok(())
    }

    pub fn terminate_current_process(
        &mut self,
        exit_code: u32,
    ) -> Result<RuntimeProcessCancellation, RuntimeStateError> {
        let process_plan = self.process.plan_direct_termination(exit_code)?;
        if !self.heaps.is_empty() {
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
                process_id: self.process.id().0,
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
        let cancelling_source_reads = plan
            .thread_plans
            .iter()
            .flat_map(|thread| {
                thread
                    .source_read_cancellation
                    .request_ids()
                    .chain(thread.writable_commit_cancellation.request_ids())
            })
            .collect::<BTreeSet<_>>();
        let payload_teardown = self.plan_payload_object_teardown(&cancelling_source_reads)?;
        let graphics_teardown = self.graphics.plan_process_teardown()?;
        let gdi_teardown = self.gdi.plan_process_teardown()?;
        let gdiplus_teardown = self.gdiplus.plan_process_teardown()?;
        let imm32_teardown = self.imm32.plan_process_teardown()?;
        let user32_teardown = self.user32.plan_process_teardown()?;
        self.process.preflight_mutation_commit(&process_plan)?;
        if let Some(timer_plan) = plan.timer_teardown.as_ref() {
            self.timers.preflight_process_teardown(timer_plan)?;
        }
        if let Some(gdi_plan) = gdi_teardown.as_ref() {
            self.gdi.preflight_process_teardown(gdi_plan)?;
        }
        if let Some(gdiplus_plan) = gdiplus_teardown.as_ref() {
            self.gdiplus.preflight_process_teardown(gdiplus_plan)?;
        }
        if let Some(imm32_plan) = imm32_teardown.as_ref() {
            self.imm32.preflight_process_teardown(imm32_plan)?;
        }
        if let Some(graphics_plan) = graphics_teardown.as_ref() {
            self.graphics.preflight_process_teardown(graphics_plan)?;
        }
        if let Some(user32_plan) = user32_teardown.as_ref() {
            self.user32.preflight_process_teardown(user32_plan)?;
        }
        let cancellation = plan.cancellation.clone();
        if let Some(timer_plan) = plan.timer_teardown {
            self.timers.commit_process_teardown(timer_plan)?;
        }
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
        self.commit_payload_object_teardown(payload_teardown)?;
        if let Some(gdi_plan) = gdi_teardown {
            self.gdi.commit_process_teardown(gdi_plan);
        }
        if let Some(gdiplus_plan) = gdiplus_teardown {
            self.gdiplus.commit_process_teardown(gdiplus_plan);
        }
        if let Some(imm32_plan) = imm32_teardown {
            self.imm32.commit_process_teardown(imm32_plan);
        }
        if let Some(graphics_plan) = graphics_teardown {
            self.graphics.commit_process_teardown(graphics_plan);
        }
        if let Some(user32_plan) = user32_teardown {
            self.user32.commit_process_teardown(user32_plan);
        }
        self.objects.commit_wait_state_updates(wait_updates);
        self.process.commit_mutation(process_plan);
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
            SchedulerThreadState::WaitingHost { request_id } if registration.is_some() => {
                let registration = registration.expect("checked timer pacing registration");
                let pacing = registration.waitable_timer_pacing.ok_or_else(|| {
                    RuntimeStateError::runtime_defect(
                        "runtime-wait",
                        "waiting-host-registration-pacing-missing",
                        "host-waiting thread registration has no timer pacing contract",
                    )
                })?;
                let request = self
                    .scheduler
                    .host_requests()
                    .get(&request_id)
                    .ok_or_else(|| {
                        RuntimeStateError::runtime_defect(
                            "runtime-wait",
                            "waiting-host-request-missing",
                            "host-waiting timer registration has no scheduler request",
                        )
                    })?;
                if pacing.request_id != request_id
                    || request.thread_id != thread_id
                    || request.service != RUNTIME_WAITABLE_TIMER_PACING_SERVICE
                {
                    return Err(RuntimeStateError::runtime_defect(
                        "runtime-wait",
                        "waiting-host-registration-request-mismatch",
                        "timer pacing wait registration does not match its scheduler request",
                    ));
                }
                let queue = self
                    .scheduler
                    .object_waiters()
                    .get(&registration.object_id)
                    .ok_or_else(|| {
                        RuntimeStateError::runtime_defect(
                            "runtime-wait",
                            "wait-object-queue-missing",
                            "timer pacing wait registration has no object queue",
                        )
                    })?;
                if queue
                    .iter()
                    .filter(|&&queued_id| queued_id == registration.id)
                    .count()
                    != 1
                {
                    return Err(RuntimeStateError::runtime_defect(
                        "runtime-wait",
                        "wait-object-queue-registration-mismatch",
                        "timer pacing wait registration must occur exactly once in its object queue",
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
                | SchedulerThreadState::WaitingCriticalSection { .. }
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
        let source_read_cancellation = self
            .files
            .plan_read_cancellation(host_request_cancellation.request_ids());
        let writable_commit_cancellation = self
            .files
            .plan_writable_commit_cancellation(host_request_cancellation.request_ids());
        let writable_directory_create_cancellation = self
            .files
            .plan_writable_directory_create_cancellation(host_request_cancellation.request_ids());
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
            source_read_cancellation,
            writable_commit_cancellation,
            writable_directory_create_cancellation,
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
        self.files
            .commit_read_cancellation(plan.source_read_cancellation);
        self.files
            .commit_writable_commit_cancellation(plan.writable_commit_cancellation);
        self.files.commit_writable_directory_create_cancellation(
            plan.writable_directory_create_cancellation,
        );
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
        let process_plan = self
            .process
            .plan_exit_request(initiating_thread_id, exit_code)?;
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
            process_plan,
            thread_plans,
            timer_teardown: self.timers.plan_process_teardown()?,
            cancellation,
        })
    }

    fn commit_process_exit_request(
        &mut self,
        plan: RuntimeProcessExitRequestPlan,
    ) -> Result<(), RuntimeStateError> {
        self.process.preflight_mutation_commit(&plan.process_plan)?;
        if let Some(timer_plan) = plan.timer_teardown.as_ref() {
            self.timers.preflight_process_teardown(timer_plan)?;
        }
        if let Some(timer_plan) = plan.timer_teardown {
            self.timers.commit_process_teardown(timer_plan)?;
        }
        self.process.commit_mutation(plan.process_plan);
        for thread_plan in plan.thread_plans {
            self.commit_thread_exit_request(thread_plan);
        }
        Ok(())
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

fn validate_device_context_target(
    device_context: &RuntimeDeviceContext,
    target: RuntimeGdiWindowTarget,
) -> Result<(), RuntimeStateError> {
    if !matches!(
        device_context.target,
        RuntimeDeviceContextTarget::Window {
            window_id,
            window_handle,
            ..
        } if window_id == target.window_id && window_handle == target.window_handle
    ) || device_context.owner_thread_id != target.owner_thread_id
    {
        return Err(RuntimeStateError::conflict(
            "runtime-gdi",
            "device-context-window-target-mismatch",
            "logical HDC does not reference the current window identity and owner thread",
        ));
    }
    Ok(())
}

fn validate_screen_device_context_target(
    device_context: &RuntimeDeviceContext,
    target: RuntimeGdiScreenTarget,
) -> Result<(), RuntimeStateError> {
    if device_context.owner_thread_id != target.owner_thread_id
        || device_context.target
            != (RuntimeDeviceContextTarget::Screen {
                monitor_handle: target.monitor_handle,
                rect: target.rect,
            })
    {
        return Err(RuntimeStateError::conflict(
            "runtime-gdi",
            "device-context-screen-target-mismatch",
            "logical HDC does not reference the current primary monitor and owner thread",
        ));
    }
    Ok(())
}

fn validate_graphics_device_context_target(
    device_context: &RuntimeDeviceContext,
    target: RuntimeGdiGraphicsSurfaceTarget,
) -> Result<(), RuntimeStateError> {
    if device_context.owner_thread_id != target.owner_thread_id
        || device_context.target
            != (RuntimeDeviceContextTarget::GraphicsSurface {
                surface_id: target.surface_id,
                surface_com_object_id: target.surface_com_object_id,
                width: target.width,
                height: target.height,
                color_format: target.color_format,
            })
    {
        return Err(RuntimeStateError::conflict(
            "runtime-gdi",
            "device-context-graphics-target-mismatch",
            "logical HDC does not reference the current graphics surface and owner thread",
        ));
    }
    Ok(())
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

fn validate_com_address_allocation_request(
    request: &ProcessAddressAllocationRequest,
) -> Result<(), RuntimeStateError> {
    if request.purpose != ProcessAddressAllocationPurpose::ComEndpoint
        || request.owner != RUNTIME_COM_ADDRESS_OWNER
        || request.classification != ProcessAddressAllocationClassification::Com
        || request.lifetime != ProcessAddressAllocationLifetime::Object
        || request.release_policy != ProcessAddressAllocationReleasePolicy::Explicit
        || !request.executable
    {
        return Err(RuntimeStateError::invalid_input(
            "runtime-com",
            "com-address-allocation-request-invalid",
            "COM backing requires an executable, explicit-release, object-lifetime COM allocation",
        ));
    }
    Ok(())
}

fn validate_com_installation_allocation(
    allocation: &ProcessAddressAllocationRecord,
    result: &RuntimeComRegistrationResult,
) -> Result<(), RuntimeStateError> {
    let object = &result.object;
    if object.backing.address_allocation_id != allocation.id
        || object.backing.range != allocation.range
        || allocation.purpose != ProcessAddressAllocationPurpose::ComEndpoint
        || allocation.owner != RUNTIME_COM_ADDRESS_OWNER
        || allocation.classification != ProcessAddressAllocationClassification::Com
        || allocation.lifetime != ProcessAddressAllocationLifetime::Object
        || allocation.release_policy != ProcessAddressAllocationReleasePolicy::Explicit
        || !allocation.executable
    {
        return Err(RuntimeStateError::conflict(
            "runtime-com",
            "com-backing-allocation-mismatch",
            "logical COM object backing does not match its planned process allocation",
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::com::{
        COM_S_OK, ComCallTrap, ComDispatchOutcome, ComFaultClass, ComReturnValue,
        install_audio_device_com_object_with_guest_writes, install_com_object,
    };
    use crate::d3d9::{
        D3D9_CREATE_HARDWARE_VERTEX_PROCESSING_MULTITHREADED, D3D9_D3DBLEND_INVSRCALPHA,
        D3D9_D3DBLEND_SRCALPHA, D3D9_D3DBLENDOP_ADD, D3D9_D3DCLEAR_TARGET, D3D9_D3DCULL_NONE,
        D3D9_D3DDEVTYPE_HAL, D3D9_D3DERR_INVALIDCALL, D3D9_D3DERR_NOTAVAILABLE, D3D9_D3DFMT_D16,
        D3D9_D3DFMT_UNKNOWN, D3D9_D3DFMT_X8R8G8B8, D3D9_D3DLOCK_NO_DIRTY_UPDATE,
        D3D9_D3DMULTISAMPLE_NONE, D3D9_D3DPOOL_DEFAULT, D3D9_D3DPOOL_MANAGED,
        D3D9_D3DPOOL_SYSTEMMEM, D3D9_D3DPT_TRIANGLESTRIP, D3D9_D3DRS_ALPHABLENDENABLE,
        D3D9_D3DRS_BLENDOP, D3D9_D3DRS_CULLMODE, D3D9_D3DRS_DESTBLEND, D3D9_D3DRS_LIGHTING,
        D3D9_D3DRS_SRCBLEND, D3D9_D3DRS_ZENABLE, D3D9_D3DSAMP_MAGFILTER, D3D9_D3DSAMP_MINFILTER,
        D3D9_D3DSWAPEFFECT_DISCARD, D3D9_D3DTA_DIFFUSE, D3D9_D3DTA_TEXTURE, D3D9_D3DTEXF_LINEAR,
        D3D9_D3DTOP_MODULATE, D3D9_D3DTOP_SELECTARG1, D3D9_D3DTSS_ALPHAARG1, D3D9_D3DTSS_ALPHAARG2,
        D3D9_D3DTSS_ALPHAOP, D3D9_D3DTSS_COLORARG1, D3D9_D3DTSS_COLORARG2, D3D9_D3DTSS_COLOROP,
        D3D9_D3DUSAGE_DYNAMIC, D3D9_D3DUSAGE_WRITEONLY, D3D9_IDIRECT3D9_IID,
        D3D9_IDIRECT3DDEVICE9_IID, D3D9_IDIRECT3DSURFACE9_IID, D3D9_IDIRECT3DTEXTURE9_IID,
        D3D9_IDIRECT3DVERTEXBUFFER9_IID, D3d9CompatibilityPolicy, direct3d9_object_spec,
        dispatch_d3d9_com_call,
    };
    use crate::dsound::{
        DSOUND_DSBPLAY_LOOPING, DSOUND_DSBUFFERDESC_BYTES, DSOUND_DSSCL_PRIORITY,
        DSOUND_E_INVALIDARG, DSOUND_IDIRECTSOUND8_IID, DSOUND_IDIRECTSOUNDBUFFER8_IID,
        DSOUND_PRIMARY_BUFFER_FLAGS, DSOUND_SECONDARY_BUFFER_FORWARD_FLAGS,
        DsoundCompatibilityPolicy, direct_sound8_object_spec, dispatch_dsound_com_call,
    };
    use crate::memory::{MemoryLimits, MemoryOwnerKind};
    use crate::runtime::RuntimeGraphicsPrimitiveTopology;
    use crate::runtime::{
        RUNTIME_AUDIO_RESOURCE_ARENA_BASE, RUNTIME_GDI_BLACK_BRUSH_INDEX,
        RUNTIME_GRAPHICS_SURFACE_ID_NAMESPACE, RUNTIME_WINDOW_CLASS_NAME_A_ENCODING,
        RUNTIME_WINDOW_NAME_A_ENCODING, RUNTIME_WINDOW_PROCEDURE_ARGUMENT_BYTES,
        RuntimeAudioBufferKind, RuntimeAudioBufferSegment, RuntimeAudioPlaybackState,
        RuntimeGraphicsBlendFactor, RuntimeGraphicsBlendOperation, RuntimeGraphicsColorFormat,
        RuntimeGraphicsCullMode, RuntimeGraphicsFlexibleVertexFormat, RuntimeGraphicsPresentMode,
        RuntimeGraphicsRenderStates, RuntimeGraphicsSamplerStates, RuntimeGraphicsTextureArgument,
        RuntimeGraphicsTextureDirtyRegion, RuntimeGraphicsTextureFilter,
        RuntimeGraphicsTextureOperation, RuntimeGraphicsTexturePixelSource,
        RuntimeGraphicsTexturePool, RuntimeGraphicsTextureStageState, RuntimeGraphicsTextureUsage,
        RuntimeGraphicsVertexBufferPool, RuntimeGraphicsVertexBufferUsage,
        RuntimeWindowClassModule, RuntimeWindowClassNameA, RuntimeWindowClassReference,
        RuntimeWindowGeometry, RuntimeWindowNameA, RuntimeWindowProcedureCallingConvention,
    };
    use crate::{ExecutionSession, RuntimeRect, Rva};

    const D3D9_FIXTURE_MEMORY_BASE: GuestAddress = GuestAddress(0x2000_0000);
    const D3D9_FIXTURE_STACK: GuestAddress = GuestAddress(D3D9_FIXTURE_MEMORY_BASE.0 + 0x100);
    const D3D9_FIXTURE_PRESENTATION: GuestAddress =
        GuestAddress(D3D9_FIXTURE_MEMORY_BASE.0 + 0x800);
    const D3D9_FIXTURE_OUTPUT: GuestAddress = GuestAddress(D3D9_FIXTURE_MEMORY_BASE.0 + 0x900);
    const D3D9_FIXTURE_TEXTURE_OUTPUT: GuestAddress =
        GuestAddress(D3D9_FIXTURE_MEMORY_BASE.0 + 0x904);
    const D3D9_FIXTURE_SURFACE_OUTPUT: GuestAddress =
        GuestAddress(D3D9_FIXTURE_MEMORY_BASE.0 + 0x908);
    const D3D9_FIXTURE_DC_OUTPUT: GuestAddress = GuestAddress(D3D9_FIXTURE_MEMORY_BASE.0 + 0x90c);
    const D3D9_FIXTURE_RASTER_STATUS: GuestAddress =
        GuestAddress(D3D9_FIXTURE_MEMORY_BASE.0 + 0x910);
    const D3D9_FIXTURE_CALL_SITE: GuestAddress = GuestAddress(0x1005_aca5);

    #[test]
    fn callback_scratch_arena_bounds_reached_deep_seh_nesting() {
        let deepest = callback_scratch_for_depth(64, 1)
            .unwrap()
            .expect("nonempty callback scratch");
        assert_eq!(deepest.slot, 63);
        assert_eq!(
            deepest.address,
            GuestAddress(CALLBACK_SCRATCH_ARENA_BASE.0 + 63 * CALLBACK_SCRATCH_SLOT_SIZE)
        );
        assert_eq!(
            callback_scratch_for_depth(65, 1).unwrap_err().code,
            "callback-scratch-depth-limit-exceeded"
        );
    }

    #[test]
    fn audio_notification_wait_payload_retains_captured_handles() {
        let handles = [
            RuntimeHandle::from_parts(RuntimeObjectKind::Event, 1, 1).unwrap(),
            RuntimeHandle::from_parts(RuntimeObjectKind::Event, 1, 2).unwrap(),
        ];
        let inactive = encode_inactive_audio_notification_wait_payload(&handles, 100);
        assert_eq!(
            decode_audio_notification_wait_handles(&inactive, 100).unwrap(),
            handles
        );

        let active_words = [
            1,
            25,
            0,
            7,
            0,
            9,
            0,
            100,
            200,
            0,
            1,
            100,
            handles.len() as u32,
        ];
        let mut active = Vec::new();
        for word in active_words
            .into_iter()
            .chain(handles.iter().map(|handle| handle.0))
        {
            active.extend_from_slice(&word.to_le_bytes());
        }
        assert_eq!(
            decode_audio_notification_wait_handles(&active, 100).unwrap(),
            handles
        );

        active[12 * 4..13 * 4].copy_from_slice(&3u32.to_le_bytes());
        assert_eq!(
            decode_audio_notification_wait_handles(&active, 100)
                .unwrap_err()
                .code,
            "audio-notification-wait-payload-invalid"
        );
    }

    #[test]
    fn failed_private_heap_registration_preserves_the_slot_allocator() {
        let mut runtime = RuntimeKernel::new(RuntimeBootstrap {
            process_id: RuntimeProcessId(0x1000),
            initial_thread_id: RuntimeThreadId(0x2000),
            initial_last_error: 0,
            address_space: ProcessAddressSpaceLayout::vine_v1(),
            clock: RuntimeClockState {
                wall_clock_file_time: 134_282_016_000_000_000,
                tick_count: 3_600_000,
                performance_counter: 12_345_678_901,
                performance_frequency: 10_000_000,
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
        let handle = 0x6801_0001;
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
    fn failed_runtime_system_module_object_registration_preserves_the_load_plan() {
        let mut runtime = RuntimeKernel::new(RuntimeBootstrap {
            process_id: RuntimeProcessId(0x1000),
            initial_thread_id: RuntimeThreadId(0x2000),
            initial_last_error: 0,
            address_space: ProcessAddressSpaceLayout::vine_v1(),
            clock: RuntimeClockState {
                wall_clock_file_time: 134_282_016_000_000_000,
                tick_count: 3_600_000,
                performance_counter: 12_345_678_901,
                performance_frequency: 10_000_000,
            },
            startup_info: RuntimeStartupInfo {
                profile_id: "runtime-system-module-object-atomicity-test".to_owned(),
                bytes: vec![0; X86_STARTUP_INFO_W_BYTE_LENGTH],
            },
            process_image_path_a: RuntimeProcessImagePathA {
                source_authority: "runtime-system-module-object-atomicity-test".to_owned(),
                bytes_with_nul: b"C:\\Vine\\fixture.exe\0".to_vec(),
                encoding: RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING.to_owned(),
            },
            initial_current_directory_a: RuntimeCurrentDirectoryA {
                source_authority: "runtime-system-module-object-atomicity-test".to_owned(),
                bytes_with_nul: b"C:\\Vine\0".to_vec(),
                encoding: RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING.to_owned(),
            },
            encoded_pointer_namespace: LOGICAL_ENCODED_POINTER_NAMESPACE.to_owned(),
            encoded_pointer_token_prefix: 0xec01_0000,
        })
        .unwrap();
        runtime.start_initial_thread().unwrap();
        for (handle, name, main) in [
            (0x4d01_0001, "kernel32.dll", false),
            (0x4d01_0002, "fixture.exe", true),
        ] {
            let module = RuntimeModule {
                owner: "runtime-system-module-object-atomicity-test".to_owned(),
                variant_id: format!("controlled-{name}"),
                namespace: LOGICAL_LOADED_MODULE_NAMESPACE.to_owned(),
                handle,
                requested_name: name.to_owned(),
                canonical_name: name.to_owned(),
                runtime_system_module: !main,
            };
            if main {
                runtime.register_main_module(module).unwrap();
            } else {
                runtime.load_module(module).unwrap();
            }
        }
        runtime
            .add_module_dependency(0x4d01_0002, 0x4d01_0001)
            .unwrap();
        runtime.mark_module_ready(0x4d01_0001).unwrap();
        runtime.mark_module_ready(0x4d01_0002).unwrap();
        runtime.attach_modules().unwrap();

        let collided_handle = 0x4d01_0003;
        runtime
            .objects
            .register_object_with_handle(
                RuntimeObjectKey::Module {
                    handle: collided_handle,
                },
                RuntimeHandle(collided_handle),
            )
            .unwrap();
        let before = runtime.clone();
        assert_eq!(
            runtime
                .load_current_thread_runtime_system_module(
                    "d3dx9_43.dll".to_owned(),
                    "d3dx9_43.dll".to_owned(),
                )
                .unwrap_err()
                .code,
            "runtime-handle-slot-live"
        );
        assert_eq!(runtime, before);
        assert_eq!(
            runtime.module_by_name("d3dx9_43.dll").unwrap_err().code,
            "module-name-unknown"
        );
    }

    #[test]
    fn host_request_serialization_preserves_the_runtime_compatibility_shape() {
        let mut runtime = RuntimeKernel::new(RuntimeBootstrap {
            process_id: RuntimeProcessId(0x1000),
            initial_thread_id: RuntimeThreadId(0x2000),
            initial_last_error: 0,
            address_space: ProcessAddressSpaceLayout::vine_v1(),
            clock: RuntimeClockState {
                wall_clock_file_time: 134_282_016_000_000_000,
                tick_count: 3_600_000,
                performance_counter: 12_345_678_901,
                performance_frequency: 10_000_000,
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

    #[test]
    fn d3d9_device_and_texture_commit_guest_com_and_graphics_state_as_one_lifetime() {
        let mut runtime = d3d9_fixture_runtime();
        let window = seed_created_d3d9_window(&mut runtime);
        let mut memory = d3d9_fixture_memory();
        let root = install_com_object(&mut memory, &mut runtime, direct3d9_object_spec())
            .expect("install D3D9 root COM object");
        let root_interface = root
            .interfaces
            .iter()
            .find(|interface| interface.iid == D3D9_IDIRECT3D9_IID)
            .expect("IDirect3D9 interface")
            .clone();
        acknowledge_d3d9_fixture_memory(&mut memory);

        let presentation_words = [
            0,
            0,
            D3D9_D3DFMT_UNKNOWN,
            1,
            D3D9_D3DMULTISAMPLE_NONE,
            0,
            D3D9_D3DSWAPEFFECT_DISCARD,
            window.handle,
            1,
            0,
            D3D9_D3DFMT_D16,
            0,
            0,
            0,
        ];
        write_d3d9_fixture_words(&mut memory, D3D9_FIXTURE_PRESENTATION, &presentation_words);
        write_d3d9_fixture_words(&mut memory, D3D9_FIXTURE_OUTPUT, &[0xcccc_cccc]);
        write_d3d9_fixture_words(
            &mut memory,
            D3D9_FIXTURE_STACK,
            &[
                D3D9_FIXTURE_CALL_SITE.0,
                root_interface.guest_pointer.0,
                0,
                D3D9_D3DDEVTYPE_HAL,
                window.handle,
                D3D9_CREATE_HARDWARE_VERTEX_PROCESSING_MULTITHREADED,
                D3D9_FIXTURE_PRESENTATION.0,
                D3D9_FIXTURE_OUTPUT.0,
            ],
        );
        acknowledge_d3d9_fixture_memory(&mut memory);

        let completion = {
            let mut session = ExecutionSession::new(&mut memory, &mut runtime);
            match dispatch_d3d9_com_call(
                &D3d9CompatibilityPolicy::controlled_v31(),
                ComCallTrap {
                    endpoint_address: root_interface.methods[&16].endpoint,
                    stack_pointer: D3D9_FIXTURE_STACK,
                    return_address: D3D9_FIXTURE_CALL_SITE,
                    call_site: D3D9_FIXTURE_CALL_SITE,
                },
                &mut session,
            ) {
                ComDispatchOutcome::Completed(completion) => completion,
                other => panic!("CreateDevice did not complete: {other:?}"),
            }
        };
        assert_eq!(completion.return_value, ComReturnValue::HResult(COM_S_OK));
        assert_eq!(
            completion.stack_pointer,
            GuestAddress(D3D9_FIXTURE_STACK.0 + 32)
        );
        assert_eq!(completion.guest_writes.len(), 2);
        assert_eq!(
            completion.guest_writes[0].address,
            D3D9_FIXTURE_PRESENTATION
        );
        assert_eq!(completion.guest_writes[1].address, D3D9_FIXTURE_OUTPUT);
        acknowledge_d3d9_fixture_memory(&mut memory);

        let presentation = memory
            .read(D3D9_FIXTURE_PRESENTATION, presentation_words.len() * 4)
            .expect("resolved presentation parameters");
        let resolved_words = presentation
            .chunks_exact(4)
            .map(|bytes| u32::from_le_bytes(bytes.try_into().unwrap()))
            .collect::<Vec<_>>();
        assert_eq!(&resolved_words[..3], &[800, 600, D3D9_D3DFMT_X8R8G8B8]);
        assert_eq!(&resolved_words[3..], &presentation_words[3..]);
        let device_pointer = u32::from_le_bytes(
            memory
                .read(D3D9_FIXTURE_OUTPUT, 4)
                .expect("CreateDevice output pointer")
                .try_into()
                .unwrap(),
        );
        let device_interface = runtime
            .com_interface_for_pointer(GuestAddress(device_pointer))
            .expect("IDirect3DDevice9 output")
            .clone();
        assert_eq!(device_interface.iid, D3D9_IDIRECT3DDEVICE9_IID);
        let device_object = runtime
            .com()
            .object(device_interface.object_id)
            .expect("device COM object")
            .clone();
        assert_eq!(device_object.reference_count, 1);
        assert_eq!(device_object.parent_interface_id, Some(root_interface.id));
        assert_eq!(
            runtime
                .com()
                .object(root.object.id)
                .unwrap()
                .reference_count,
            2
        );

        let graphics = runtime
            .graphics_devices()
            .values()
            .next()
            .expect("logical graphics device")
            .clone();
        assert_eq!(runtime.graphics_devices().len(), 1);
        assert_eq!(graphics.window_id, window.id);
        assert_eq!(graphics.window_handle, window.handle);
        assert_eq!(graphics.owner_thread_id, RuntimeThreadId(0x2000));
        assert_eq!(graphics.com_object_id, device_object.id);
        assert_eq!(graphics.presentation.width, 800);
        assert_eq!(graphics.presentation.height, 600);
        assert_eq!(
            graphics.presentation.color_format,
            RuntimeGraphicsColorFormat::B8g8r8x8Unorm
        );
        assert_eq!(graphics.presentation.back_buffer_count, 1);
        assert_eq!(graphics.presentation.sample_count, 1);
        assert_eq!(
            graphics.presentation.present_mode,
            RuntimeGraphicsPresentMode::Discard
        );
        assert!(graphics.presentation.windowed);
        assert!(graphics.renderer.is_none());
        assert_eq!(
            graphics.render_states,
            RuntimeGraphicsRenderStates::default()
        );
        assert_eq!(
            graphics.sampler_states,
            RuntimeGraphicsSamplerStates::default()
        );
        acknowledge_d3d9_fixture_memory(&mut memory);

        write_d3d9_fixture_words(
            &mut memory,
            D3D9_FIXTURE_STACK,
            &[0x1005_adcd, device_interface.guest_pointer.0],
        );
        acknowledge_d3d9_fixture_memory(&mut memory);
        let before_memory = memory.stats();
        let before_runtime = runtime.clone();
        let cooperative_level = {
            let mut session = ExecutionSession::new(&mut memory, &mut runtime);
            match dispatch_d3d9_com_call(
                &D3d9CompatibilityPolicy::controlled_v32(),
                ComCallTrap {
                    endpoint_address: device_interface.methods[&3].endpoint,
                    stack_pointer: D3D9_FIXTURE_STACK,
                    return_address: GuestAddress(0x1005_adcd),
                    call_site: GuestAddress(0x1005_adcd),
                },
                &mut session,
            ) {
                ComDispatchOutcome::Completed(completion) => completion,
                other => panic!("TestCooperativeLevel did not complete: {other:?}"),
            }
        };
        assert_eq!(
            cooperative_level.return_value,
            ComReturnValue::HResult(COM_S_OK)
        );
        assert_eq!(
            cooperative_level.stack_pointer,
            GuestAddress(D3D9_FIXTURE_STACK.0 + 8)
        );
        assert_eq!(cooperative_level.reference_count, 1);
        assert!(!cooperative_level.object_retired);
        assert!(cooperative_level.guest_writes.is_empty());
        assert_eq!(runtime.current_last_error().unwrap(), 0x1234_5678);
        assert_eq!(memory.stats(), before_memory);
        assert_eq!(runtime, before_runtime);

        let disabled_policy = D3d9CompatibilityPolicy::controlled_v31();
        let disabled_cooperative_level = {
            let mut session = ExecutionSession::new(&mut memory, &mut runtime);
            match dispatch_d3d9_com_call(
                &disabled_policy,
                ComCallTrap {
                    endpoint_address: device_interface.methods[&3].endpoint,
                    stack_pointer: D3D9_FIXTURE_STACK,
                    return_address: GuestAddress(0x1005_adcd),
                    call_site: GuestAddress(0x1005_adcd),
                },
                &mut session,
            ) {
                ComDispatchOutcome::Fault(fault) => fault,
                other => panic!("disabled TestCooperativeLevel did not fault: {other:?}"),
            }
        };
        assert_eq!(disabled_cooperative_level.class, ComFaultClass::Unsupported);
        assert_eq!(
            disabled_cooperative_level.code,
            "test-cooperative-level-policy-disabled"
        );
        assert_eq!(runtime.current_last_error().unwrap(), 0x1234_5678);
        assert_eq!(memory.stats(), before_memory);
        assert_eq!(runtime, before_runtime);

        write_d3d9_fixture_words(
            &mut memory,
            D3D9_FIXTURE_STACK,
            &[
                0x1005_ad13,
                device_interface.guest_pointer.0,
                D3D9_D3DRS_CULLMODE,
                3,
            ],
        );
        acknowledge_d3d9_fixture_memory(&mut memory);
        let before_memory = memory.stats();
        let before_runtime = runtime.clone();
        let unavailable = {
            let mut session = ExecutionSession::new(&mut memory, &mut runtime);
            match dispatch_d3d9_com_call(
                &D3d9CompatibilityPolicy::controlled_v31(),
                ComCallTrap {
                    endpoint_address: device_interface.methods[&57].endpoint,
                    stack_pointer: D3D9_FIXTURE_STACK,
                    return_address: GuestAddress(0x1005_ad13),
                    call_site: GuestAddress(0x1005_ad13),
                },
                &mut session,
            ) {
                ComDispatchOutcome::Completed(completion) => completion,
                other => panic!("unsupported SetRenderState did not complete: {other:?}"),
            }
        };
        assert_eq!(
            unavailable.return_value,
            ComReturnValue::HResult(D3D9_D3DERR_NOTAVAILABLE)
        );
        assert!(unavailable.guest_writes.is_empty());
        assert_eq!(memory.stats(), before_memory);
        assert_eq!(runtime, before_runtime);

        let mut disabled_render_states = D3d9CompatibilityPolicy::controlled_v31();
        disabled_render_states.initial_render_states = None;
        disabled_render_states.draw_primitive = None;
        disabled_render_states.present = None;
        write_d3d9_fixture_words(
            &mut memory,
            D3D9_FIXTURE_STACK,
            &[
                0x1005_ad13,
                device_interface.guest_pointer.0,
                D3D9_D3DRS_CULLMODE,
                D3D9_D3DCULL_NONE,
            ],
        );
        acknowledge_d3d9_fixture_memory(&mut memory);
        let before_memory = memory.stats();
        let before_runtime = runtime.clone();
        let disabled_fault = {
            let mut session = ExecutionSession::new(&mut memory, &mut runtime);
            match dispatch_d3d9_com_call(
                &disabled_render_states,
                ComCallTrap {
                    endpoint_address: device_interface.methods[&57].endpoint,
                    stack_pointer: D3D9_FIXTURE_STACK,
                    return_address: GuestAddress(0x1005_ad13),
                    call_site: GuestAddress(0x1005_ad13),
                },
                &mut session,
            ) {
                ComDispatchOutcome::Fault(fault) => fault,
                other => panic!("disabled SetRenderState did not fault: {other:?}"),
            }
        };
        assert_eq!(disabled_fault.class, ComFaultClass::Unsupported);
        assert_eq!(disabled_fault.code, "set-render-state-policy-disabled");
        assert_eq!(memory.stats(), before_memory);
        assert_eq!(runtime, before_runtime);

        for (call_site, state, value) in [
            (
                GuestAddress(0x1005_ad13),
                D3D9_D3DRS_CULLMODE,
                D3D9_D3DCULL_NONE,
            ),
            (GuestAddress(0x1005_ad26), D3D9_D3DRS_ZENABLE, 0),
            (GuestAddress(0x1005_ad3c), D3D9_D3DRS_LIGHTING, 0),
            (GuestAddress(0x1005_ad50), D3D9_D3DRS_ALPHABLENDENABLE, 1),
            (
                GuestAddress(0x1005_b1df),
                D3D9_D3DRS_SRCBLEND,
                D3D9_D3DBLEND_SRCALPHA,
            ),
            (
                GuestAddress(0x1005_b1cb),
                D3D9_D3DRS_DESTBLEND,
                D3D9_D3DBLEND_INVSRCALPHA,
            ),
            (
                GuestAddress(0x1005_b1b7),
                D3D9_D3DRS_BLENDOP,
                D3D9_D3DBLENDOP_ADD,
            ),
        ] {
            write_d3d9_fixture_words(
                &mut memory,
                D3D9_FIXTURE_STACK,
                &[call_site.0, device_interface.guest_pointer.0, state, value],
            );
            acknowledge_d3d9_fixture_memory(&mut memory);
            let before_memory = memory.stats();
            let completion = {
                let mut session = ExecutionSession::new(&mut memory, &mut runtime);
                match dispatch_d3d9_com_call(
                    &D3d9CompatibilityPolicy::controlled_v31(),
                    ComCallTrap {
                        endpoint_address: device_interface.methods[&57].endpoint,
                        stack_pointer: D3D9_FIXTURE_STACK,
                        return_address: call_site,
                        call_site,
                    },
                    &mut session,
                ) {
                    ComDispatchOutcome::Completed(completion) => completion,
                    other => panic!("SetRenderState did not complete: {other:?}"),
                }
            };
            assert_eq!(completion.return_value, ComReturnValue::HResult(COM_S_OK));
            assert_eq!(
                completion.stack_pointer,
                GuestAddress(D3D9_FIXTURE_STACK.0 + 16)
            );
            assert!(completion.guest_writes.is_empty());
            assert_eq!(memory.stats(), before_memory);
            assert_eq!(runtime.current_last_error().unwrap(), 0x1234_5678);
        }
        let render_state_device = runtime.graphics_device(graphics.id).unwrap();
        assert_eq!(
            render_state_device.render_states.cull_mode,
            Some(RuntimeGraphicsCullMode::None)
        );
        assert_eq!(
            render_state_device.render_states.depth_test_enabled,
            Some(false)
        );
        assert_eq!(
            render_state_device.render_states.lighting_enabled,
            Some(false)
        );
        assert_eq!(
            render_state_device.render_states.alpha_blend_enabled,
            Some(true)
        );
        assert_eq!(
            render_state_device.render_states.source_blend_factor,
            Some(RuntimeGraphicsBlendFactor::SourceAlpha)
        );
        assert_eq!(
            render_state_device.render_states.destination_blend_factor,
            Some(RuntimeGraphicsBlendFactor::InverseSourceAlpha)
        );
        assert_eq!(
            render_state_device.render_states.blend_operation,
            Some(RuntimeGraphicsBlendOperation::Add)
        );
        assert_eq!(render_state_device.generation, graphics.generation + 7);

        write_d3d9_fixture_words(
            &mut memory,
            D3D9_FIXTURE_STACK,
            &[
                0x1005_ad50,
                device_interface.guest_pointer.0,
                D3D9_D3DRS_ALPHABLENDENABLE,
                1,
            ],
        );
        acknowledge_d3d9_fixture_memory(&mut memory);
        let before_memory = memory.stats();
        let before_runtime = runtime.clone();
        let repeated = {
            let mut session = ExecutionSession::new(&mut memory, &mut runtime);
            match dispatch_d3d9_com_call(
                &D3d9CompatibilityPolicy::controlled_v31(),
                ComCallTrap {
                    endpoint_address: device_interface.methods[&57].endpoint,
                    stack_pointer: D3D9_FIXTURE_STACK,
                    return_address: GuestAddress(0x1005_ad50),
                    call_site: GuestAddress(0x1005_ad50),
                },
                &mut session,
            ) {
                ComDispatchOutcome::Completed(completion) => completion,
                other => panic!("repeated SetRenderState did not complete: {other:?}"),
            }
        };
        assert_eq!(repeated.return_value, ComReturnValue::HResult(COM_S_OK));
        assert!(repeated.guest_writes.is_empty());
        assert_eq!(memory.stats(), before_memory);
        assert_eq!(runtime, before_runtime);

        write_d3d9_fixture_words(
            &mut memory,
            D3D9_FIXTURE_STACK,
            &[
                0x1005_b1f5,
                device_interface.guest_pointer.0,
                1,
                D3D9_D3DTSS_COLOROP,
                D3D9_D3DTOP_MODULATE,
            ],
        );
        acknowledge_d3d9_fixture_memory(&mut memory);
        let before_memory = memory.stats();
        let before_runtime = runtime.clone();
        let unavailable = {
            let mut session = ExecutionSession::new(&mut memory, &mut runtime);
            match dispatch_d3d9_com_call(
                &D3d9CompatibilityPolicy::controlled_v31(),
                ComCallTrap {
                    endpoint_address: device_interface.methods[&67].endpoint,
                    stack_pointer: D3D9_FIXTURE_STACK,
                    return_address: GuestAddress(0x1005_b1f5),
                    call_site: GuestAddress(0x1005_b1f5),
                },
                &mut session,
            ) {
                ComDispatchOutcome::Completed(completion) => completion,
                other => panic!("unsupported SetTextureStageState did not complete: {other:?}"),
            }
        };
        assert_eq!(
            unavailable.return_value,
            ComReturnValue::HResult(D3D9_D3DERR_NOTAVAILABLE)
        );
        assert!(unavailable.guest_writes.is_empty());
        assert_eq!(memory.stats(), before_memory);
        assert_eq!(runtime, before_runtime);

        let mut disabled_texture_stage_states = D3d9CompatibilityPolicy::controlled_v31();
        disabled_texture_stage_states.initial_texture_stage_states = None;
        disabled_texture_stage_states.draw_primitive = None;
        disabled_texture_stage_states.present = None;
        write_d3d9_fixture_words(
            &mut memory,
            D3D9_FIXTURE_STACK,
            &[
                0x1005_b1f5,
                device_interface.guest_pointer.0,
                0,
                D3D9_D3DTSS_COLOROP,
                D3D9_D3DTOP_MODULATE,
            ],
        );
        acknowledge_d3d9_fixture_memory(&mut memory);
        let before_memory = memory.stats();
        let before_runtime = runtime.clone();
        let disabled_fault = {
            let mut session = ExecutionSession::new(&mut memory, &mut runtime);
            match dispatch_d3d9_com_call(
                &disabled_texture_stage_states,
                ComCallTrap {
                    endpoint_address: device_interface.methods[&67].endpoint,
                    stack_pointer: D3D9_FIXTURE_STACK,
                    return_address: GuestAddress(0x1005_b1f5),
                    call_site: GuestAddress(0x1005_b1f5),
                },
                &mut session,
            ) {
                ComDispatchOutcome::Fault(fault) => fault,
                other => panic!("disabled SetTextureStageState did not fault: {other:?}"),
            }
        };
        assert_eq!(disabled_fault.class, ComFaultClass::Unsupported);
        assert_eq!(
            disabled_fault.code,
            "set-texture-stage-state-policy-disabled"
        );
        assert_eq!(memory.stats(), before_memory);
        assert_eq!(runtime, before_runtime);

        let texture_stage_base_generation =
            runtime.graphics_device(graphics.id).unwrap().generation;
        for (call_site, state, value) in [
            (
                GuestAddress(0x1005_b1f5),
                D3D9_D3DTSS_COLOROP,
                D3D9_D3DTOP_MODULATE,
            ),
            (
                GuestAddress(0x1005_b20b),
                D3D9_D3DTSS_COLORARG1,
                D3D9_D3DTA_TEXTURE,
            ),
            (
                GuestAddress(0x1005_b221),
                D3D9_D3DTSS_COLORARG2,
                D3D9_D3DTA_DIFFUSE,
            ),
            (
                GuestAddress(0x1005_b237),
                D3D9_D3DTSS_ALPHAOP,
                D3D9_D3DTOP_SELECTARG1,
            ),
            (
                GuestAddress(0x1005_b24d),
                D3D9_D3DTSS_ALPHAARG1,
                D3D9_D3DTA_TEXTURE,
            ),
            (
                GuestAddress(0x1005_b263),
                D3D9_D3DTSS_ALPHAARG2,
                D3D9_D3DTA_DIFFUSE,
            ),
        ] {
            write_d3d9_fixture_words(
                &mut memory,
                D3D9_FIXTURE_STACK,
                &[
                    call_site.0,
                    device_interface.guest_pointer.0,
                    0,
                    state,
                    value,
                ],
            );
            acknowledge_d3d9_fixture_memory(&mut memory);
            let before_memory = memory.stats();
            let completion = {
                let mut session = ExecutionSession::new(&mut memory, &mut runtime);
                match dispatch_d3d9_com_call(
                    &D3d9CompatibilityPolicy::controlled_v31(),
                    ComCallTrap {
                        endpoint_address: device_interface.methods[&67].endpoint,
                        stack_pointer: D3D9_FIXTURE_STACK,
                        return_address: call_site,
                        call_site,
                    },
                    &mut session,
                ) {
                    ComDispatchOutcome::Completed(completion) => completion,
                    other => panic!("SetTextureStageState did not complete: {other:?}"),
                }
            };
            assert_eq!(completion.return_value, ComReturnValue::HResult(COM_S_OK));
            assert_eq!(
                completion.stack_pointer,
                GuestAddress(D3D9_FIXTURE_STACK.0 + 20)
            );
            assert!(completion.guest_writes.is_empty());
            assert_eq!(memory.stats(), before_memory);
            assert_eq!(runtime.current_last_error().unwrap(), 0x1234_5678);
        }
        let texture_stage_device = runtime.graphics_device(graphics.id).unwrap();
        assert_eq!(
            texture_stage_device.texture_stage_states.stage_zero,
            RuntimeGraphicsTextureStageState {
                color_operation: Some(RuntimeGraphicsTextureOperation::Modulate),
                color_argument_one: Some(RuntimeGraphicsTextureArgument::Texture),
                color_argument_two: Some(RuntimeGraphicsTextureArgument::Diffuse),
                alpha_operation: Some(RuntimeGraphicsTextureOperation::SelectArgumentOne),
                alpha_argument_one: Some(RuntimeGraphicsTextureArgument::Texture),
                alpha_argument_two: Some(RuntimeGraphicsTextureArgument::Diffuse),
            }
        );
        assert_eq!(
            texture_stage_device.generation,
            texture_stage_base_generation + 6
        );

        let before_memory = memory.stats();
        let before_runtime = runtime.clone();
        let repeated = {
            let mut session = ExecutionSession::new(&mut memory, &mut runtime);
            match dispatch_d3d9_com_call(
                &D3d9CompatibilityPolicy::controlled_v31(),
                ComCallTrap {
                    endpoint_address: device_interface.methods[&67].endpoint,
                    stack_pointer: D3D9_FIXTURE_STACK,
                    return_address: GuestAddress(0x1005_b263),
                    call_site: GuestAddress(0x1005_b263),
                },
                &mut session,
            ) {
                ComDispatchOutcome::Completed(completion) => completion,
                other => panic!("repeated SetTextureStageState did not complete: {other:?}"),
            }
        };
        assert_eq!(repeated.return_value, ComReturnValue::HResult(COM_S_OK));
        assert!(repeated.guest_writes.is_empty());
        assert_eq!(memory.stats(), before_memory);
        assert_eq!(runtime, before_runtime);

        write_d3d9_fixture_words(
            &mut memory,
            D3D9_FIXTURE_STACK,
            &[
                0x1005_b18a,
                device_interface.guest_pointer.0,
                1,
                D3D9_D3DSAMP_MINFILTER,
                D3D9_D3DTEXF_LINEAR,
            ],
        );
        acknowledge_d3d9_fixture_memory(&mut memory);
        let before_memory = memory.stats();
        let before_runtime = runtime.clone();
        let unavailable = {
            let mut session = ExecutionSession::new(&mut memory, &mut runtime);
            match dispatch_d3d9_com_call(
                &D3d9CompatibilityPolicy::controlled_v31(),
                ComCallTrap {
                    endpoint_address: device_interface.methods[&69].endpoint,
                    stack_pointer: D3D9_FIXTURE_STACK,
                    return_address: GuestAddress(0x1005_b18a),
                    call_site: GuestAddress(0x1005_b18a),
                },
                &mut session,
            ) {
                ComDispatchOutcome::Completed(completion) => completion,
                other => panic!("unsupported SetSamplerState did not complete: {other:?}"),
            }
        };
        assert_eq!(
            unavailable.return_value,
            ComReturnValue::HResult(D3D9_D3DERR_NOTAVAILABLE)
        );
        assert!(unavailable.guest_writes.is_empty());
        assert_eq!(memory.stats(), before_memory);
        assert_eq!(runtime, before_runtime);

        let mut disabled_sampler_states = D3d9CompatibilityPolicy::controlled_v31();
        disabled_sampler_states.initial_sampler_states = None;
        disabled_sampler_states.draw_primitive = None;
        disabled_sampler_states.present = None;
        write_d3d9_fixture_words(
            &mut memory,
            D3D9_FIXTURE_STACK,
            &[
                0x1005_b18a,
                device_interface.guest_pointer.0,
                0,
                D3D9_D3DSAMP_MINFILTER,
                D3D9_D3DTEXF_LINEAR,
            ],
        );
        acknowledge_d3d9_fixture_memory(&mut memory);
        let before_memory = memory.stats();
        let before_runtime = runtime.clone();
        let disabled_fault = {
            let mut session = ExecutionSession::new(&mut memory, &mut runtime);
            match dispatch_d3d9_com_call(
                &disabled_sampler_states,
                ComCallTrap {
                    endpoint_address: device_interface.methods[&69].endpoint,
                    stack_pointer: D3D9_FIXTURE_STACK,
                    return_address: GuestAddress(0x1005_b18a),
                    call_site: GuestAddress(0x1005_b18a),
                },
                &mut session,
            ) {
                ComDispatchOutcome::Fault(fault) => fault,
                other => panic!("disabled SetSamplerState did not fault: {other:?}"),
            }
        };
        assert_eq!(disabled_fault.class, ComFaultClass::Unsupported);
        assert_eq!(disabled_fault.code, "set-sampler-state-policy-disabled");
        assert_eq!(memory.stats(), before_memory);
        assert_eq!(runtime, before_runtime);

        let sampler_base_generation = runtime.graphics_device(graphics.id).unwrap().generation;
        for (call_site, state) in [
            (GuestAddress(0x1005_b18a), D3D9_D3DSAMP_MINFILTER),
            (GuestAddress(0x1005_b1a0), D3D9_D3DSAMP_MAGFILTER),
        ] {
            write_d3d9_fixture_words(
                &mut memory,
                D3D9_FIXTURE_STACK,
                &[
                    call_site.0,
                    device_interface.guest_pointer.0,
                    0,
                    state,
                    D3D9_D3DTEXF_LINEAR,
                ],
            );
            acknowledge_d3d9_fixture_memory(&mut memory);
            let before_memory = memory.stats();
            let completion = {
                let mut session = ExecutionSession::new(&mut memory, &mut runtime);
                match dispatch_d3d9_com_call(
                    &D3d9CompatibilityPolicy::controlled_v31(),
                    ComCallTrap {
                        endpoint_address: device_interface.methods[&69].endpoint,
                        stack_pointer: D3D9_FIXTURE_STACK,
                        return_address: call_site,
                        call_site,
                    },
                    &mut session,
                ) {
                    ComDispatchOutcome::Completed(completion) => completion,
                    other => panic!("SetSamplerState did not complete: {other:?}"),
                }
            };
            assert_eq!(completion.return_value, ComReturnValue::HResult(COM_S_OK));
            assert_eq!(
                completion.stack_pointer,
                GuestAddress(D3D9_FIXTURE_STACK.0 + 20)
            );
            assert!(completion.guest_writes.is_empty());
            assert_eq!(memory.stats(), before_memory);
            assert_eq!(runtime.current_last_error().unwrap(), 0x1234_5678);
        }
        let sampler_state_device = runtime.graphics_device(graphics.id).unwrap();
        assert_eq!(
            sampler_state_device.sampler_states.sampler_zero.min_filter,
            Some(RuntimeGraphicsTextureFilter::Linear)
        );
        assert_eq!(
            sampler_state_device.sampler_states.sampler_zero.mag_filter,
            Some(RuntimeGraphicsTextureFilter::Linear)
        );
        assert_eq!(sampler_state_device.generation, sampler_base_generation + 2);

        let before_memory = memory.stats();
        let before_runtime = runtime.clone();
        let repeated = {
            let mut session = ExecutionSession::new(&mut memory, &mut runtime);
            match dispatch_d3d9_com_call(
                &D3d9CompatibilityPolicy::controlled_v31(),
                ComCallTrap {
                    endpoint_address: device_interface.methods[&69].endpoint,
                    stack_pointer: D3D9_FIXTURE_STACK,
                    return_address: GuestAddress(0x1005_b1a0),
                    call_site: GuestAddress(0x1005_b1a0),
                },
                &mut session,
            ) {
                ComDispatchOutcome::Completed(completion) => completion,
                other => panic!("repeated SetSamplerState did not complete: {other:?}"),
            }
        };
        assert_eq!(repeated.return_value, ComReturnValue::HResult(COM_S_OK));
        assert!(repeated.guest_writes.is_empty());
        assert_eq!(memory.stats(), before_memory);
        assert_eq!(runtime, before_runtime);

        write_d3d9_fixture_words(
            &mut memory,
            D3D9_FIXTURE_RASTER_STATUS,
            &[0xcccc_cccc, 0xdddd_dddd],
        );
        for (swap_chain, output) in [(0, 0), (1, D3D9_FIXTURE_RASTER_STATUS.0)] {
            write_d3d9_fixture_words(
                &mut memory,
                D3D9_FIXTURE_STACK,
                &[
                    0x1005_aedd,
                    device_interface.guest_pointer.0,
                    swap_chain,
                    output,
                ],
            );
            acknowledge_d3d9_fixture_memory(&mut memory);
            let before_memory = memory.stats();
            let before_runtime = runtime.clone();
            let invalid = {
                let mut session = ExecutionSession::new(&mut memory, &mut runtime);
                match dispatch_d3d9_com_call(
                    &D3d9CompatibilityPolicy::controlled_v31(),
                    ComCallTrap {
                        endpoint_address: device_interface.methods[&19].endpoint,
                        stack_pointer: D3D9_FIXTURE_STACK,
                        return_address: GuestAddress(0x1005_aedd),
                        call_site: GuestAddress(0x1005_aedd),
                    },
                    &mut session,
                ) {
                    ComDispatchOutcome::Completed(completion) => completion,
                    other => panic!("invalid GetRasterStatus did not complete: {other:?}"),
                }
            };
            assert_eq!(
                invalid.return_value,
                ComReturnValue::HResult(D3D9_D3DERR_INVALIDCALL)
            );
            assert!(invalid.guest_writes.is_empty());
            assert_eq!(memory.stats(), before_memory);
            assert_eq!(runtime, before_runtime);
            assert_eq!(
                memory.read(D3D9_FIXTURE_RASTER_STATUS, 8).unwrap(),
                [0xcc, 0xcc, 0xcc, 0xcc, 0xdd, 0xdd, 0xdd, 0xdd]
            );
        }

        let mut disabled_raster_status = D3d9CompatibilityPolicy::controlled_v31();
        disabled_raster_status.raster_status = None;
        write_d3d9_fixture_words(
            &mut memory,
            D3D9_FIXTURE_STACK,
            &[
                0x1005_aedd,
                device_interface.guest_pointer.0,
                0,
                D3D9_FIXTURE_RASTER_STATUS.0,
            ],
        );
        acknowledge_d3d9_fixture_memory(&mut memory);
        let before_memory = memory.stats();
        let before_runtime = runtime.clone();
        let disabled_fault = {
            let mut session = ExecutionSession::new(&mut memory, &mut runtime);
            match dispatch_d3d9_com_call(
                &disabled_raster_status,
                ComCallTrap {
                    endpoint_address: device_interface.methods[&19].endpoint,
                    stack_pointer: D3D9_FIXTURE_STACK,
                    return_address: GuestAddress(0x1005_aedd),
                    call_site: GuestAddress(0x1005_aedd),
                },
                &mut session,
            ) {
                ComDispatchOutcome::Fault(fault) => fault,
                other => panic!("disabled GetRasterStatus did not fault: {other:?}"),
            }
        };
        assert_eq!(disabled_fault.class, ComFaultClass::Unsupported);
        assert_eq!(disabled_fault.code, "raster-status-policy-disabled");
        assert_eq!(memory.stats(), before_memory);
        assert_eq!(runtime, before_runtime);

        let before_runtime = runtime.clone();
        let raster_status = {
            let mut session = ExecutionSession::new(&mut memory, &mut runtime);
            match dispatch_d3d9_com_call(
                &D3d9CompatibilityPolicy::controlled_v31(),
                ComCallTrap {
                    endpoint_address: device_interface.methods[&19].endpoint,
                    stack_pointer: D3D9_FIXTURE_STACK,
                    return_address: GuestAddress(0x1005_aedd),
                    call_site: GuestAddress(0x1005_aedd),
                },
                &mut session,
            ) {
                ComDispatchOutcome::Completed(completion) => completion,
                other => panic!("GetRasterStatus did not complete: {other:?}"),
            }
        };
        assert_eq!(
            raster_status.return_value,
            ComReturnValue::HResult(COM_S_OK)
        );
        assert_eq!(
            raster_status.stack_pointer,
            GuestAddress(D3D9_FIXTURE_STACK.0 + 16)
        );
        assert_eq!(raster_status.guest_writes.len(), 1);
        assert_eq!(
            raster_status.guest_writes[0].address,
            D3D9_FIXTURE_RASTER_STATUS
        );
        assert_eq!(
            raster_status.guest_writes[0].bytes,
            [0, 0, 0, 0, 0x37, 0x04, 0, 0]
        );
        acknowledge_d3d9_fixture_memory(&mut memory);
        assert_eq!(
            memory.read(D3D9_FIXTURE_RASTER_STATUS, 8).unwrap(),
            [0, 0, 0, 0, 0x37, 0x04, 0, 0]
        );
        assert_eq!(runtime, before_runtime);
        assert_eq!(runtime.current_last_error().unwrap(), 0x1234_5678);

        for arguments in [
            [
                1,
                0,
                D3D9_D3DCLEAR_TARGET,
                0xff00_0000,
                1.0_f32.to_bits(),
                0,
            ],
            [
                0,
                D3D9_FIXTURE_RASTER_STATUS.0,
                D3D9_D3DCLEAR_TARGET,
                0xff00_0000,
                1.0_f32.to_bits(),
                0,
            ],
            [0, 0, 0, 0xff00_0000, 1.0_f32.to_bits(), 0],
            [0, 0, D3D9_D3DCLEAR_TARGET, 0, 1.0_f32.to_bits(), 0],
            [0, 0, D3D9_D3DCLEAR_TARGET, 0xff00_0000, 0, 0],
            [
                0,
                0,
                D3D9_D3DCLEAR_TARGET,
                0xff00_0000,
                1.0_f32.to_bits(),
                1,
            ],
        ] {
            write_d3d9_fixture_words(
                &mut memory,
                D3D9_FIXTURE_STACK,
                &[
                    0x1005_af9e,
                    device_interface.guest_pointer.0,
                    arguments[0],
                    arguments[1],
                    arguments[2],
                    arguments[3],
                    arguments[4],
                    arguments[5],
                ],
            );
            acknowledge_d3d9_fixture_memory(&mut memory);
            let before_memory = memory.stats();
            let before_runtime = runtime.clone();
            let invalid = {
                let mut session = ExecutionSession::new(&mut memory, &mut runtime);
                match dispatch_d3d9_com_call(
                    &D3d9CompatibilityPolicy::controlled_v31(),
                    ComCallTrap {
                        endpoint_address: device_interface.methods[&43].endpoint,
                        stack_pointer: D3D9_FIXTURE_STACK,
                        return_address: GuestAddress(0x1005_af9e),
                        call_site: GuestAddress(0x1005_af9e),
                    },
                    &mut session,
                ) {
                    ComDispatchOutcome::Completed(completion) => completion,
                    other => panic!("invalid Clear did not complete: {other:?}"),
                }
            };
            assert_eq!(
                invalid.return_value,
                ComReturnValue::HResult(D3D9_D3DERR_INVALIDCALL)
            );
            assert!(invalid.guest_writes.is_empty());
            assert_eq!(memory.stats(), before_memory);
            assert_eq!(runtime, before_runtime);
        }

        let valid_clear_words = [
            0x1005_af9e,
            device_interface.guest_pointer.0,
            0,
            0,
            D3D9_D3DCLEAR_TARGET,
            0xff00_0000,
            1.0_f32.to_bits(),
            0,
        ];
        write_d3d9_fixture_words(&mut memory, D3D9_FIXTURE_STACK, &valid_clear_words);
        acknowledge_d3d9_fixture_memory(&mut memory);
        let mut disabled_clear = D3d9CompatibilityPolicy::controlled_v31();
        disabled_clear.clear = None;
        disabled_clear.draw_primitive = None;
        disabled_clear.present = None;
        let before_memory = memory.stats();
        let before_runtime = runtime.clone();
        let disabled_fault = {
            let mut session = ExecutionSession::new(&mut memory, &mut runtime);
            match dispatch_d3d9_com_call(
                &disabled_clear,
                ComCallTrap {
                    endpoint_address: device_interface.methods[&43].endpoint,
                    stack_pointer: D3D9_FIXTURE_STACK,
                    return_address: GuestAddress(0x1005_af9e),
                    call_site: GuestAddress(0x1005_af9e),
                },
                &mut session,
            ) {
                ComDispatchOutcome::Fault(fault) => fault,
                other => panic!("disabled Clear did not fault: {other:?}"),
            }
        };
        assert_eq!(disabled_fault.class, ComFaultClass::Unsupported);
        assert_eq!(disabled_fault.code, "clear-policy-disabled");
        assert_eq!(memory.stats(), before_memory);
        assert_eq!(runtime, before_runtime);

        let before_memory = memory.stats();
        let before_device = runtime.graphics_device(graphics.id).unwrap().clone();
        let before_reference_count = runtime
            .com()
            .object(device_object.id)
            .unwrap()
            .reference_count;
        let clear = {
            let mut session = ExecutionSession::new(&mut memory, &mut runtime);
            match dispatch_d3d9_com_call(
                &D3d9CompatibilityPolicy::controlled_v31(),
                ComCallTrap {
                    endpoint_address: device_interface.methods[&43].endpoint,
                    stack_pointer: D3D9_FIXTURE_STACK,
                    return_address: GuestAddress(0x1005_af9e),
                    call_site: GuestAddress(0x1005_af9e),
                },
                &mut session,
            ) {
                ComDispatchOutcome::Completed(completion) => completion,
                other => panic!("Clear did not complete: {other:?}"),
            }
        };
        assert_eq!(clear.return_value, ComReturnValue::HResult(COM_S_OK));
        assert_eq!(clear.stack_pointer, GuestAddress(D3D9_FIXTURE_STACK.0 + 32));
        assert!(clear.guest_writes.is_empty());
        assert_eq!(memory.stats(), before_memory);
        assert_eq!(runtime.current_last_error().unwrap(), 0x1234_5678);
        assert_eq!(
            runtime
                .com()
                .object(device_object.id)
                .unwrap()
                .reference_count,
            before_reference_count
        );
        let cleared_device = runtime.graphics_device(graphics.id).unwrap();
        assert_eq!(cleared_device.back_buffer.content_generation, 1);
        let uniform_clear = cleared_device
            .back_buffer
            .uniform_clear
            .expect("uniform logical back-buffer clear");
        assert_eq!(uniform_clear.sequence, 1);
        assert_eq!(uniform_clear.color_argb, 0xff00_0000);
        assert_eq!(
            uniform_clear.rect,
            RuntimeRect {
                left: 0,
                top: 0,
                right: 800,
                bottom: 600,
            }
        );
        assert_eq!(cleared_device.generation, before_device.generation + 1);
        assert!(cleared_device.renderer.is_none());

        write_d3d9_fixture_words(&mut memory, D3D9_FIXTURE_TEXTURE_OUTPUT, &[0xcccc_cccc]);
        for (width, levels, usage, pool, output, shared_handle, expected) in [
            (
                0,
                1,
                0,
                D3D9_D3DPOOL_SYSTEMMEM,
                D3D9_FIXTURE_TEXTURE_OUTPUT.0,
                0,
                D3D9_D3DERR_INVALIDCALL,
            ),
            (
                1_024,
                1,
                0,
                D3D9_D3DPOOL_SYSTEMMEM,
                0,
                0,
                D3D9_D3DERR_INVALIDCALL,
            ),
            (
                1_024,
                1,
                0,
                D3D9_D3DPOOL_SYSTEMMEM,
                D3D9_FIXTURE_TEXTURE_OUTPUT.0,
                D3D9_FIXTURE_TEXTURE_OUTPUT.0,
                D3D9_D3DERR_INVALIDCALL,
            ),
            (
                512,
                1,
                0,
                D3D9_D3DPOOL_SYSTEMMEM,
                D3D9_FIXTURE_TEXTURE_OUTPUT.0,
                0,
                D3D9_D3DERR_NOTAVAILABLE,
            ),
            (
                1_024,
                2,
                0,
                D3D9_D3DPOOL_SYSTEMMEM,
                D3D9_FIXTURE_TEXTURE_OUTPUT.0,
                0,
                D3D9_D3DERR_NOTAVAILABLE,
            ),
            (
                1_024,
                1,
                D3D9_D3DUSAGE_DYNAMIC,
                D3D9_D3DPOOL_SYSTEMMEM,
                D3D9_FIXTURE_TEXTURE_OUTPUT.0,
                0,
                D3D9_D3DERR_NOTAVAILABLE,
            ),
            (
                1_024,
                1,
                1,
                D3D9_D3DPOOL_DEFAULT,
                D3D9_FIXTURE_TEXTURE_OUTPUT.0,
                0,
                D3D9_D3DERR_NOTAVAILABLE,
            ),
        ] {
            write_d3d9_fixture_words(
                &mut memory,
                D3D9_FIXTURE_STACK,
                &[
                    0x1005_a715,
                    device_interface.guest_pointer.0,
                    width,
                    1_024,
                    levels,
                    usage,
                    D3D9_D3DFMT_X8R8G8B8,
                    pool,
                    output,
                    shared_handle,
                ],
            );
            acknowledge_d3d9_fixture_memory(&mut memory);
            let before_memory = memory.stats();
            let before_runtime = runtime.clone();
            let completion = {
                let mut session = ExecutionSession::new(&mut memory, &mut runtime);
                match dispatch_d3d9_com_call(
                    &D3d9CompatibilityPolicy::controlled_v31(),
                    ComCallTrap {
                        endpoint_address: device_interface.methods[&23].endpoint,
                        stack_pointer: D3D9_FIXTURE_STACK,
                        return_address: GuestAddress(0x1005_a715),
                        call_site: GuestAddress(0x1005_a715),
                    },
                    &mut session,
                ) {
                    ComDispatchOutcome::Completed(completion) => completion,
                    other => panic!("invalid CreateTexture did not complete: {other:?}"),
                }
            };
            assert_eq!(completion.return_value, ComReturnValue::HResult(expected));
            assert!(completion.guest_writes.is_empty());
            assert_eq!(memory.stats(), before_memory);
            assert_eq!(runtime, before_runtime);
            assert_eq!(
                memory.read(D3D9_FIXTURE_TEXTURE_OUTPUT, 4).unwrap(),
                0xcccc_cccc_u32.to_le_bytes()
            );
        }

        write_d3d9_fixture_words(
            &mut memory,
            D3D9_FIXTURE_STACK,
            &[
                0x1005_a715,
                device_interface.guest_pointer.0,
                1_024,
                1_024,
                1,
                0,
                D3D9_D3DFMT_X8R8G8B8,
                D3D9_D3DPOOL_SYSTEMMEM,
                D3D9_FIXTURE_TEXTURE_OUTPUT.0,
                0,
            ],
        );
        acknowledge_d3d9_fixture_memory(&mut memory);
        let mut disabled = D3d9CompatibilityPolicy::controlled_v31();
        disabled.create_texture = None;
        disabled.get_surface_level = None;
        disabled.surface_dc = None;
        disabled.add_dirty_rect = None;
        disabled.texture_lock_rect = None;
        disabled.update_texture = None;
        disabled.texture_binding = None;
        disabled.draw_primitive = None;
        disabled.present = None;
        let before_disabled_memory = memory.stats();
        let before_disabled_runtime = runtime.clone();
        let disabled_fault = {
            let mut session = ExecutionSession::new(&mut memory, &mut runtime);
            match dispatch_d3d9_com_call(
                &disabled,
                ComCallTrap {
                    endpoint_address: device_interface.methods[&23].endpoint,
                    stack_pointer: D3D9_FIXTURE_STACK,
                    return_address: GuestAddress(0x1005_a715),
                    call_site: GuestAddress(0x1005_a715),
                },
                &mut session,
            ) {
                ComDispatchOutcome::Fault(fault) => fault,
                other => panic!("disabled CreateTexture did not fault: {other:?}"),
            }
        };
        assert_eq!(disabled_fault.class, ComFaultClass::Unsupported);
        assert_eq!(disabled_fault.code, "create-texture-policy-disabled");
        assert_eq!(memory.stats(), before_disabled_memory);
        assert_eq!(runtime, before_disabled_runtime);

        write_d3d9_fixture_words(
            &mut memory,
            D3D9_FIXTURE_STACK,
            &[
                0x1005_a715,
                device_interface.guest_pointer.0,
                1_024,
                1_024,
                1,
                0,
                D3D9_D3DFMT_X8R8G8B8,
                D3D9_D3DPOOL_SYSTEMMEM,
                D3D9_FIXTURE_TEXTURE_OUTPUT.0,
                0,
            ],
        );
        acknowledge_d3d9_fixture_memory(&mut memory);
        let texture_completion = {
            let mut session = ExecutionSession::new(&mut memory, &mut runtime);
            match dispatch_d3d9_com_call(
                &D3d9CompatibilityPolicy::controlled_v31(),
                ComCallTrap {
                    endpoint_address: device_interface.methods[&23].endpoint,
                    stack_pointer: D3D9_FIXTURE_STACK,
                    return_address: GuestAddress(0x1005_a715),
                    call_site: GuestAddress(0x1005_a715),
                },
                &mut session,
            ) {
                ComDispatchOutcome::Completed(completion) => completion,
                other => panic!("CreateTexture did not complete: {other:?}"),
            }
        };
        assert_eq!(
            texture_completion.return_value,
            ComReturnValue::HResult(COM_S_OK)
        );
        assert_eq!(
            texture_completion.stack_pointer,
            GuestAddress(D3D9_FIXTURE_STACK.0 + 40)
        );
        assert_eq!(texture_completion.guest_writes.len(), 1);
        assert_eq!(
            texture_completion.guest_writes[0].address,
            D3D9_FIXTURE_TEXTURE_OUTPUT
        );
        acknowledge_d3d9_fixture_memory(&mut memory);

        let texture_pointer = u32::from_le_bytes(
            memory
                .read(D3D9_FIXTURE_TEXTURE_OUTPUT, 4)
                .expect("CreateTexture output pointer")
                .try_into()
                .unwrap(),
        );
        let texture_interface = runtime
            .com_interface_for_pointer(GuestAddress(texture_pointer))
            .expect("IDirect3DTexture9 output")
            .clone();
        assert_eq!(texture_interface.iid, D3D9_IDIRECT3DTEXTURE9_IID);
        let texture_object = runtime
            .com()
            .object(texture_interface.object_id)
            .expect("texture COM object")
            .clone();
        assert_eq!(texture_object.reference_count, 1);
        assert_eq!(
            texture_object.parent_interface_id,
            Some(device_interface.id)
        );
        assert_eq!(
            runtime
                .com()
                .object(device_object.id)
                .unwrap()
                .reference_count,
            2
        );
        let texture = runtime
            .graphics_textures()
            .values()
            .next()
            .expect("logical graphics texture")
            .clone();
        assert_eq!(runtime.graphics_textures().len(), 1);
        assert_eq!(texture.device_id, graphics.id);
        assert_eq!(texture.com_object_id, texture_object.id);
        assert_eq!(texture.descriptor.width, 1_024);
        assert_eq!(texture.descriptor.height, 1_024);
        assert_eq!(texture.descriptor.levels, 1);
        assert_eq!(texture.descriptor.usage, RuntimeGraphicsTextureUsage::None);
        assert_eq!(
            texture.descriptor.color_format,
            RuntimeGraphicsColorFormat::B8g8r8x8Unorm
        );
        assert_eq!(
            texture.descriptor.pool,
            RuntimeGraphicsTexturePool::SystemMemory
        );

        let mut additional_texture_interfaces = Vec::new();
        for (call_site, width, usage, pool, expected_usage, expected_pool) in [
            (
                GuestAddress(0x1005_a7b7),
                1_024,
                0,
                D3D9_D3DPOOL_DEFAULT,
                RuntimeGraphicsTextureUsage::None,
                RuntimeGraphicsTexturePool::Default,
            ),
            (
                GuestAddress(0x1005_a7dc),
                1_024,
                D3D9_D3DUSAGE_DYNAMIC,
                D3D9_D3DPOOL_DEFAULT,
                RuntimeGraphicsTextureUsage::Dynamic,
                RuntimeGraphicsTexturePool::Default,
            ),
            (
                GuestAddress(0x1005_a715),
                2_048,
                0,
                D3D9_D3DPOOL_SYSTEMMEM,
                RuntimeGraphicsTextureUsage::None,
                RuntimeGraphicsTexturePool::SystemMemory,
            ),
            (
                GuestAddress(0x1005_a7b7),
                2_048,
                0,
                D3D9_D3DPOOL_DEFAULT,
                RuntimeGraphicsTextureUsage::None,
                RuntimeGraphicsTexturePool::Default,
            ),
            (
                GuestAddress(0x1005_a7dc),
                2_048,
                D3D9_D3DUSAGE_DYNAMIC,
                D3D9_D3DPOOL_DEFAULT,
                RuntimeGraphicsTextureUsage::Dynamic,
                RuntimeGraphicsTexturePool::Default,
            ),
        ] {
            write_d3d9_fixture_words(
                &mut memory,
                D3D9_FIXTURE_STACK,
                &[
                    call_site.0,
                    device_interface.guest_pointer.0,
                    width,
                    1_024,
                    1,
                    usage,
                    D3D9_D3DFMT_X8R8G8B8,
                    pool,
                    D3D9_FIXTURE_TEXTURE_OUTPUT.0,
                    0,
                ],
            );
            acknowledge_d3d9_fixture_memory(&mut memory);
            let completion = {
                let mut session = ExecutionSession::new(&mut memory, &mut runtime);
                match dispatch_d3d9_com_call(
                    &D3d9CompatibilityPolicy::controlled_v31(),
                    ComCallTrap {
                        endpoint_address: device_interface.methods[&23].endpoint,
                        stack_pointer: D3D9_FIXTURE_STACK,
                        return_address: call_site,
                        call_site,
                    },
                    &mut session,
                ) {
                    ComDispatchOutcome::Completed(completion) => completion,
                    other => panic!("controlled CreateTexture did not complete: {other:?}"),
                }
            };
            assert_eq!(completion.return_value, ComReturnValue::HResult(COM_S_OK));
            assert_eq!(completion.guest_writes.len(), 1);
            acknowledge_d3d9_fixture_memory(&mut memory);
            let pointer = u32::from_le_bytes(
                memory
                    .read(D3D9_FIXTURE_TEXTURE_OUTPUT, 4)
                    .unwrap()
                    .try_into()
                    .unwrap(),
            );
            let interface = runtime
                .com_interface_for_pointer(GuestAddress(pointer))
                .expect("additional IDirect3DTexture9 output")
                .clone();
            let logical_texture = runtime
                .graphics_textures()
                .values()
                .find(|texture| texture.com_object_id == interface.object_id)
                .expect("additional logical graphics texture");
            assert_eq!(logical_texture.descriptor.usage, expected_usage);
            assert_eq!(logical_texture.descriptor.pool, expected_pool);
            assert_eq!(logical_texture.descriptor.width, width);
            assert_eq!(logical_texture.descriptor.height, 1_024);
            assert_eq!(logical_texture.descriptor.levels, 1);
            additional_texture_interfaces.push(interface);
        }
        assert_eq!(runtime.graphics_textures().len(), 6);
        assert_eq!(
            runtime
                .com()
                .object(device_object.id)
                .unwrap()
                .reference_count,
            7
        );

        let recreated_system_texture_interface = additional_texture_interfaces[2].clone();
        let recreated_system_texture = runtime
            .graphics_textures()
            .values()
            .find(|texture| texture.com_object_id == recreated_system_texture_interface.object_id)
            .expect("recreated system-memory logical texture")
            .clone();
        assert_eq!(
            recreated_system_texture.dirty_region,
            RuntimeGraphicsTextureDirtyRegion::WholeTexture
        );
        let dirty_rect_address = GuestAddress(D3D9_FIXTURE_MEMORY_BASE.0 + 0xa00);
        let locked_rect_address = GuestAddress(D3D9_FIXTURE_MEMORY_BASE.0 + 0xa20);
        write_d3d9_fixture_words(&mut memory, dirty_rect_address, &[0, 0, 1_280, 720]);

        write_d3d9_fixture_words(
            &mut memory,
            locked_rect_address,
            &[0xcccc_cccc, 0xcccc_cccc],
        );
        write_d3d9_fixture_words(
            &mut memory,
            D3D9_FIXTURE_STACK,
            &[
                0x1004_154c,
                recreated_system_texture_interface.guest_pointer.0,
                0,
                locked_rect_address.0,
                dirty_rect_address.0,
                D3D9_D3DLOCK_NO_DIRTY_UPDATE,
            ],
        );
        acknowledge_d3d9_fixture_memory(&mut memory);
        let before_lock_memory = memory.stats();
        let lock = match dispatch_d3d9_fixture_frontend(
            &mut memory,
            &mut runtime,
            &D3d9CompatibilityPolicy::controlled_v31(),
            &recreated_system_texture_interface,
            19,
            GuestAddress(0x1004_154c),
        ) {
            ComDispatchOutcome::Completed(completion) => completion,
            other => panic!("IDirect3DTexture9::LockRect did not complete: {other:?}"),
        };
        assert_eq!(lock.return_value, ComReturnValue::HResult(COM_S_OK));
        assert_eq!(lock.stack_pointer, GuestAddress(D3D9_FIXTURE_STACK.0 + 24));
        assert_eq!(lock.guest_writes.len(), 1);
        assert_eq!(lock.guest_writes[0].address, locked_rect_address);
        acknowledge_d3d9_fixture_memory(&mut memory);
        let locked_rect = memory.read(locked_rect_address, 8).unwrap();
        let texture_pitch = u32::from_le_bytes(locked_rect[0..4].try_into().unwrap());
        let texture_bits = GuestAddress(u32::from_le_bytes(locked_rect[4..8].try_into().unwrap()));
        assert_eq!(texture_pitch, 2_048 * 4);
        let locked_texture = runtime
            .graphics_texture_for_com_object(recreated_system_texture_interface.object_id)
            .unwrap();
        let texture_backing = locked_texture.backing.expect("lazy texture backing");
        assert_eq!(texture_bits, texture_backing.range.start());
        assert_eq!(texture_backing.row_pitch, texture_pitch);
        assert_eq!(texture_backing.range.byte_length(), 2_048 * 1_024 * 4);
        assert_eq!(
            memory.stats().committed_pages,
            before_lock_memory.committed_pages + 2_048
        );
        assert_eq!(memory.read(texture_bits, 4).unwrap(), [0, 0, 0, 0]);

        let before_duplicate_memory = memory.stats();
        let before_duplicate_runtime = runtime.clone();
        let duplicate_lock = match dispatch_d3d9_fixture_frontend(
            &mut memory,
            &mut runtime,
            &D3d9CompatibilityPolicy::controlled_v31(),
            &recreated_system_texture_interface,
            19,
            GuestAddress(0x1004_154c),
        ) {
            ComDispatchOutcome::Completed(completion) => completion,
            other => panic!("duplicate IDirect3DTexture9::LockRect did not complete: {other:?}"),
        };
        assert_eq!(
            duplicate_lock.return_value,
            ComReturnValue::HResult(D3D9_D3DERR_INVALIDCALL)
        );
        assert!(duplicate_lock.guest_writes.is_empty());
        assert_eq!(memory.stats(), before_duplicate_memory);
        assert_eq!(runtime, before_duplicate_runtime);

        memory
            .write(texture_bits, &[0x11, 0x22, 0x33, 0])
            .expect("write locked texture pixel");
        acknowledge_d3d9_fixture_memory(&mut memory);
        write_d3d9_fixture_words(
            &mut memory,
            D3D9_FIXTURE_STACK,
            &[
                0x1004_156d,
                recreated_system_texture_interface.guest_pointer.0,
                0,
            ],
        );
        acknowledge_d3d9_fixture_memory(&mut memory);
        let unlock = match dispatch_d3d9_fixture_frontend(
            &mut memory,
            &mut runtime,
            &D3d9CompatibilityPolicy::controlled_v31(),
            &recreated_system_texture_interface,
            20,
            GuestAddress(0x1004_156d),
        ) {
            ComDispatchOutcome::Completed(completion) => completion,
            other => panic!("IDirect3DTexture9::UnlockRect did not complete: {other:?}"),
        };
        assert_eq!(unlock.return_value, ComReturnValue::HResult(COM_S_OK));
        assert_eq!(
            unlock.stack_pointer,
            GuestAddress(D3D9_FIXTURE_STACK.0 + 12)
        );
        assert!(unlock.guest_writes.is_empty());
        acknowledge_d3d9_fixture_memory(&mut memory);
        assert_eq!(memory.read(texture_bits, 4).unwrap(), [0x11, 0x22, 0x33, 0]);
        assert!(memory.write(texture_bits, &[0xff]).is_err());
        let unlocked_texture = runtime
            .graphics_texture_for_com_object(recreated_system_texture_interface.object_id)
            .unwrap();
        assert!(unlocked_texture.active_lock.is_none());
        assert_eq!(
            unlocked_texture.content.pixel_source,
            Some(RuntimeGraphicsTexturePixelSource {
                source_texture_id: unlocked_texture.id,
                source_content_generation: 1,
                range: texture_backing.range,
                row_pitch: texture_pitch,
            })
        );
        assert_eq!(
            unlocked_texture.dirty_region,
            RuntimeGraphicsTextureDirtyRegion::WholeTexture
        );

        let before_repeated_unlock_memory = memory.stats();
        let before_repeated_unlock_runtime = runtime.clone();
        let repeated_unlock = match dispatch_d3d9_fixture_frontend(
            &mut memory,
            &mut runtime,
            &D3d9CompatibilityPolicy::controlled_v31(),
            &recreated_system_texture_interface,
            20,
            GuestAddress(0x1004_156d),
        ) {
            ComDispatchOutcome::Completed(completion) => completion,
            other => panic!("repeated IDirect3DTexture9::UnlockRect did not complete: {other:?}"),
        };
        assert_eq!(
            repeated_unlock.return_value,
            ComReturnValue::HResult(COM_S_OK)
        );
        assert!(repeated_unlock.guest_writes.is_empty());
        assert_eq!(memory.stats(), before_repeated_unlock_memory);
        assert_eq!(runtime, before_repeated_unlock_runtime);

        for rect_pointer in [dirty_rect_address.0, 0] {
            write_d3d9_fixture_words(
                &mut memory,
                D3D9_FIXTURE_STACK,
                &[
                    0x1005_b01d,
                    recreated_system_texture_interface.guest_pointer.0,
                    rect_pointer,
                ],
            );
            acknowledge_d3d9_fixture_memory(&mut memory);
            let before_memory = memory.stats();
            let before_runtime = runtime.clone();
            let completion = {
                let mut session = ExecutionSession::new(&mut memory, &mut runtime);
                match dispatch_d3d9_com_call(
                    &D3d9CompatibilityPolicy::controlled_v31(),
                    ComCallTrap {
                        endpoint_address: recreated_system_texture_interface.methods[&21].endpoint,
                        stack_pointer: D3D9_FIXTURE_STACK,
                        return_address: GuestAddress(0x1005_b01d),
                        call_site: GuestAddress(0x1005_b01d),
                    },
                    &mut session,
                ) {
                    ComDispatchOutcome::Completed(completion) => completion,
                    other => panic!("AddDirtyRect did not complete: {other:?}"),
                }
            };
            assert_eq!(completion.return_value, ComReturnValue::HResult(COM_S_OK));
            assert_eq!(
                completion.stack_pointer,
                GuestAddress(D3D9_FIXTURE_STACK.0 + 12)
            );
            assert!(completion.guest_writes.is_empty());
            assert_eq!(memory.stats(), before_memory);
            assert_eq!(runtime, before_runtime);
            assert_eq!(runtime.current_last_error().unwrap(), 0x1234_5678);
        }

        for rect in [
            [4, 5, 4, 6],
            [8, 5, 7, 6],
            [u32::MAX, 0, 1, 1],
            [0, 0, 2_049, 1_024],
        ] {
            write_d3d9_fixture_words(&mut memory, dirty_rect_address, &rect);
            write_d3d9_fixture_words(
                &mut memory,
                D3D9_FIXTURE_STACK,
                &[
                    0x1005_b01d,
                    recreated_system_texture_interface.guest_pointer.0,
                    dirty_rect_address.0,
                ],
            );
            acknowledge_d3d9_fixture_memory(&mut memory);
            let before_memory = memory.stats();
            let before_runtime = runtime.clone();
            let completion = {
                let mut session = ExecutionSession::new(&mut memory, &mut runtime);
                match dispatch_d3d9_com_call(
                    &D3d9CompatibilityPolicy::controlled_v31(),
                    ComCallTrap {
                        endpoint_address: recreated_system_texture_interface.methods[&21].endpoint,
                        stack_pointer: D3D9_FIXTURE_STACK,
                        return_address: GuestAddress(0x1005_b01d),
                        call_site: GuestAddress(0x1005_b01d),
                    },
                    &mut session,
                ) {
                    ComDispatchOutcome::Completed(completion) => completion,
                    other => panic!("invalid AddDirtyRect did not complete: {other:?}"),
                }
            };
            assert_eq!(
                completion.return_value,
                ComReturnValue::HResult(D3D9_D3DERR_INVALIDCALL)
            );
            assert!(completion.guest_writes.is_empty());
            assert_eq!(memory.stats(), before_memory);
            assert_eq!(runtime, before_runtime);
        }

        let mut disabled_add_dirty_rect = D3d9CompatibilityPolicy::controlled_v31();
        disabled_add_dirty_rect.add_dirty_rect = None;
        write_d3d9_fixture_words(
            &mut memory,
            D3D9_FIXTURE_STACK,
            &[
                0x1005_b01d,
                recreated_system_texture_interface.guest_pointer.0,
                dirty_rect_address.0,
            ],
        );
        acknowledge_d3d9_fixture_memory(&mut memory);
        let before_disabled_memory = memory.stats();
        let before_disabled_runtime = runtime.clone();
        let disabled_fault = {
            let mut session = ExecutionSession::new(&mut memory, &mut runtime);
            match dispatch_d3d9_com_call(
                &disabled_add_dirty_rect,
                ComCallTrap {
                    endpoint_address: recreated_system_texture_interface.methods[&21].endpoint,
                    stack_pointer: D3D9_FIXTURE_STACK,
                    return_address: GuestAddress(0x1005_b01d),
                    call_site: GuestAddress(0x1005_b01d),
                },
                &mut session,
            ) {
                ComDispatchOutcome::Fault(fault) => fault,
                other => panic!("disabled AddDirtyRect did not fault: {other:?}"),
            }
        };
        assert_eq!(disabled_fault.class, ComFaultClass::Unsupported);
        assert_eq!(disabled_fault.code, "add-dirty-rect-policy-disabled");
        assert_eq!(memory.stats(), before_disabled_memory);
        assert_eq!(runtime, before_disabled_runtime);

        let recreated_default_texture_interface = additional_texture_interfaces[3].clone();
        write_d3d9_fixture_words(
            &mut memory,
            D3D9_FIXTURE_STACK,
            &[
                0x1005_b01d,
                recreated_default_texture_interface.guest_pointer.0,
                dirty_rect_address.0,
            ],
        );
        acknowledge_d3d9_fixture_memory(&mut memory);
        let before_variant_memory = memory.stats();
        let before_variant_runtime = runtime.clone();
        let variant_fault = {
            let mut session = ExecutionSession::new(&mut memory, &mut runtime);
            match dispatch_d3d9_com_call(
                &D3d9CompatibilityPolicy::controlled_v31(),
                ComCallTrap {
                    endpoint_address: recreated_default_texture_interface.methods[&21].endpoint,
                    stack_pointer: D3D9_FIXTURE_STACK,
                    return_address: GuestAddress(0x1005_b01d),
                    call_site: GuestAddress(0x1005_b01d),
                },
                &mut session,
            ) {
                ComDispatchOutcome::Fault(fault) => fault,
                other => panic!("unsupported AddDirtyRect variant did not fault: {other:?}"),
            }
        };
        assert_eq!(variant_fault.class, ComFaultClass::Unsupported);
        assert_eq!(
            variant_fault.code,
            "add-dirty-rect-texture-variant-unsupported"
        );
        assert_eq!(memory.stats(), before_variant_memory);
        assert_eq!(runtime, before_variant_runtime);

        write_d3d9_fixture_words(
            &mut memory,
            D3D9_FIXTURE_STACK,
            &[
                0x1005_b038,
                device_interface.guest_pointer.0,
                recreated_system_texture_interface.guest_pointer.0,
                recreated_default_texture_interface.guest_pointer.0,
            ],
        );
        acknowledge_d3d9_fixture_memory(&mut memory);
        let update_texture = {
            let mut session = ExecutionSession::new(&mut memory, &mut runtime);
            match dispatch_d3d9_com_call(
                &D3d9CompatibilityPolicy::controlled_v31(),
                ComCallTrap {
                    endpoint_address: device_interface.methods[&31].endpoint,
                    stack_pointer: D3D9_FIXTURE_STACK,
                    return_address: GuestAddress(0x1005_b038),
                    call_site: GuestAddress(0x1005_b038),
                },
                &mut session,
            ) {
                ComDispatchOutcome::Completed(completion) => completion,
                other => panic!("UpdateTexture did not complete: {other:?}"),
            }
        };
        assert_eq!(
            update_texture.return_value,
            ComReturnValue::HResult(COM_S_OK)
        );
        assert_eq!(
            update_texture.stack_pointer,
            GuestAddress(D3D9_FIXTURE_STACK.0 + 16)
        );
        assert!(update_texture.guest_writes.is_empty());
        assert_eq!(runtime.current_last_error().unwrap(), 0x1234_5678);
        let updated_source = runtime
            .graphics_texture_for_com_object(recreated_system_texture_interface.object_id)
            .unwrap();
        assert_eq!(
            updated_source.dirty_region,
            RuntimeGraphicsTextureDirtyRegion::Clean
        );
        let updated_destination = runtime
            .graphics_texture_for_com_object(recreated_default_texture_interface.object_id)
            .unwrap();
        assert_eq!(updated_destination.content.content_generation, 1);
        assert!(updated_destination.content.fills.is_empty());
        assert_eq!(
            updated_destination.content.pixel_source,
            updated_source.content.pixel_source
        );
        let update = updated_destination.content.last_update.unwrap();
        assert_eq!(update.sequence, 1);
        assert_eq!(update.source_texture_id, updated_source.id);
        assert_eq!(
            update.dirty_region,
            RuntimeGraphicsTextureDirtyRegion::WholeTexture
        );

        write_d3d9_fixture_words(&mut memory, dirty_rect_address, &[0, 0, 1_280, 720]);
        write_d3d9_fixture_words(
            &mut memory,
            D3D9_FIXTURE_STACK,
            &[
                0x1004_154c,
                recreated_system_texture_interface.guest_pointer.0,
                0,
                locked_rect_address.0,
                dirty_rect_address.0,
                D3D9_D3DLOCK_NO_DIRTY_UPDATE,
            ],
        );
        acknowledge_d3d9_fixture_memory(&mut memory);
        let relock = match dispatch_d3d9_fixture_frontend(
            &mut memory,
            &mut runtime,
            &D3d9CompatibilityPolicy::controlled_v31(),
            &recreated_system_texture_interface,
            19,
            GuestAddress(0x1004_154c),
        ) {
            ComDispatchOutcome::Completed(completion) => completion,
            other => panic!("second IDirect3DTexture9::LockRect did not complete: {other:?}"),
        };
        assert_eq!(relock.return_value, ComReturnValue::HResult(COM_S_OK));
        acknowledge_d3d9_fixture_memory(&mut memory);
        memory
            .write(texture_bits, &[0x44, 0x55, 0x66, 0])
            .expect("write second locked texture pixel");
        acknowledge_d3d9_fixture_memory(&mut memory);
        write_d3d9_fixture_words(
            &mut memory,
            D3D9_FIXTURE_STACK,
            &[
                0x1004_156d,
                recreated_system_texture_interface.guest_pointer.0,
                0,
            ],
        );
        acknowledge_d3d9_fixture_memory(&mut memory);
        let reunlock = match dispatch_d3d9_fixture_frontend(
            &mut memory,
            &mut runtime,
            &D3d9CompatibilityPolicy::controlled_v31(),
            &recreated_system_texture_interface,
            20,
            GuestAddress(0x1004_156d),
        ) {
            ComDispatchOutcome::Completed(completion) => completion,
            other => panic!("second IDirect3DTexture9::UnlockRect did not complete: {other:?}"),
        };
        assert_eq!(reunlock.return_value, ComReturnValue::HResult(COM_S_OK));
        acknowledge_d3d9_fixture_memory(&mut memory);
        assert_eq!(memory.read(texture_bits, 4).unwrap(), [0x44, 0x55, 0x66, 0]);

        write_d3d9_fixture_words(
            &mut memory,
            D3D9_FIXTURE_STACK,
            &[
                0x1005_b01d,
                recreated_system_texture_interface.guest_pointer.0,
                dirty_rect_address.0,
            ],
        );
        acknowledge_d3d9_fixture_memory(&mut memory);
        let partial_dirty = match dispatch_d3d9_fixture_frontend(
            &mut memory,
            &mut runtime,
            &D3d9CompatibilityPolicy::controlled_v31(),
            &recreated_system_texture_interface,
            21,
            GuestAddress(0x1005_b01d),
        ) {
            ComDispatchOutcome::Completed(completion) => completion,
            other => panic!("partial AddDirtyRect did not complete: {other:?}"),
        };
        assert_eq!(
            partial_dirty.return_value,
            ComReturnValue::HResult(COM_S_OK)
        );
        assert_eq!(
            runtime
                .graphics_texture_for_com_object(recreated_system_texture_interface.object_id)
                .unwrap()
                .dirty_region,
            RuntimeGraphicsTextureDirtyRegion::Rect(RuntimeRect {
                left: 0,
                top: 0,
                right: 1_280,
                bottom: 720,
            })
        );

        write_d3d9_fixture_words(
            &mut memory,
            D3D9_FIXTURE_STACK,
            &[
                0x1005_b038,
                device_interface.guest_pointer.0,
                recreated_system_texture_interface.guest_pointer.0,
                recreated_default_texture_interface.guest_pointer.0,
            ],
        );
        acknowledge_d3d9_fixture_memory(&mut memory);
        let partial_update = match dispatch_d3d9_fixture_frontend(
            &mut memory,
            &mut runtime,
            &D3d9CompatibilityPolicy::controlled_v31(),
            &device_interface,
            31,
            GuestAddress(0x1005_b038),
        ) {
            ComDispatchOutcome::Completed(completion) => completion,
            other => panic!("partial UpdateTexture did not complete: {other:?}"),
        };
        assert_eq!(
            partial_update.return_value,
            ComReturnValue::HResult(COM_S_OK)
        );
        let updated_source = runtime
            .graphics_texture_for_com_object(recreated_system_texture_interface.object_id)
            .unwrap();
        let updated_destination = runtime
            .graphics_texture_for_com_object(recreated_default_texture_interface.object_id)
            .unwrap();
        assert_eq!(
            updated_source.dirty_region,
            RuntimeGraphicsTextureDirtyRegion::Clean
        );
        assert_eq!(updated_destination.content.content_generation, 2);
        assert_eq!(
            updated_destination.content.pixel_source,
            updated_source.content.pixel_source
        );
        assert_eq!(
            updated_destination
                .content
                .last_update
                .unwrap()
                .dirty_region,
            RuntimeGraphicsTextureDirtyRegion::Rect(RuntimeRect {
                left: 0,
                top: 0,
                right: 1_280,
                bottom: 720,
            })
        );

        write_d3d9_fixture_words(&mut memory, dirty_rect_address, &[0, 0, 1_280, 720]);
        write_d3d9_fixture_words(
            &mut memory,
            D3D9_FIXTURE_STACK,
            &[
                0x1004_154c,
                recreated_system_texture_interface.guest_pointer.0,
                0,
                locked_rect_address.0,
                dirty_rect_address.0,
                D3D9_D3DLOCK_NO_DIRTY_UPDATE,
            ],
        );
        acknowledge_d3d9_fixture_memory(&mut memory);
        let third_lock = match dispatch_d3d9_fixture_frontend(
            &mut memory,
            &mut runtime,
            &D3d9CompatibilityPolicy::controlled_v31(),
            &recreated_system_texture_interface,
            19,
            GuestAddress(0x1004_154c),
        ) {
            ComDispatchOutcome::Completed(completion) => completion,
            other => panic!("third IDirect3DTexture9::LockRect did not complete: {other:?}"),
        };
        assert_eq!(third_lock.return_value, ComReturnValue::HResult(COM_S_OK));
        acknowledge_d3d9_fixture_memory(&mut memory);
        memory
            .write(texture_bits, &[0x77, 0x88, 0x99, 0])
            .expect("write third locked texture pixel");
        acknowledge_d3d9_fixture_memory(&mut memory);
        write_d3d9_fixture_words(
            &mut memory,
            D3D9_FIXTURE_STACK,
            &[
                0x1004_156d,
                recreated_system_texture_interface.guest_pointer.0,
                0,
            ],
        );
        acknowledge_d3d9_fixture_memory(&mut memory);
        let third_unlock = match dispatch_d3d9_fixture_frontend(
            &mut memory,
            &mut runtime,
            &D3d9CompatibilityPolicy::controlled_v31(),
            &recreated_system_texture_interface,
            20,
            GuestAddress(0x1004_156d),
        ) {
            ComDispatchOutcome::Completed(completion) => completion,
            other => panic!("third IDirect3DTexture9::UnlockRect did not complete: {other:?}"),
        };
        assert_eq!(third_unlock.return_value, ComReturnValue::HResult(COM_S_OK));
        acknowledge_d3d9_fixture_memory(&mut memory);

        let later_partial_rect = RuntimeRect {
            left: 232,
            top: 556,
            right: 346,
            bottom: 619,
        };
        write_d3d9_fixture_words(
            &mut memory,
            dirty_rect_address,
            &[
                later_partial_rect.left as u32,
                later_partial_rect.top as u32,
                later_partial_rect.right as u32,
                later_partial_rect.bottom as u32,
            ],
        );
        write_d3d9_fixture_words(
            &mut memory,
            D3D9_FIXTURE_STACK,
            &[
                0x1005_afe1,
                recreated_system_texture_interface.guest_pointer.0,
                dirty_rect_address.0,
            ],
        );
        acknowledge_d3d9_fixture_memory(&mut memory);
        let later_partial_dirty = match dispatch_d3d9_fixture_frontend(
            &mut memory,
            &mut runtime,
            &D3d9CompatibilityPolicy::controlled_v31(),
            &recreated_system_texture_interface,
            21,
            GuestAddress(0x1005_afe1),
        ) {
            ComDispatchOutcome::Completed(completion) => completion,
            other => panic!("later partial AddDirtyRect did not complete: {other:?}"),
        };
        assert_eq!(
            later_partial_dirty.return_value,
            ComReturnValue::HResult(COM_S_OK)
        );
        assert_eq!(
            runtime
                .graphics_texture_for_com_object(recreated_system_texture_interface.object_id)
                .unwrap()
                .dirty_region,
            RuntimeGraphicsTextureDirtyRegion::Rect(later_partial_rect)
        );

        write_d3d9_fixture_words(
            &mut memory,
            D3D9_FIXTURE_STACK,
            &[
                0x1005_b038,
                device_interface.guest_pointer.0,
                recreated_system_texture_interface.guest_pointer.0,
                recreated_default_texture_interface.guest_pointer.0,
            ],
        );
        acknowledge_d3d9_fixture_memory(&mut memory);
        let before_v32_memory = memory.stats();
        let before_v32_runtime = runtime.clone();
        let v32_fault = match dispatch_d3d9_fixture_frontend(
            &mut memory,
            &mut runtime,
            &D3d9CompatibilityPolicy::controlled_v32(),
            &device_interface,
            31,
            GuestAddress(0x1005_b038),
        ) {
            ComDispatchOutcome::Fault(fault) => fault,
            other => panic!("v32 later partial UpdateTexture did not fault: {other:?}"),
        };
        assert_eq!(v32_fault.class, ComFaultClass::Unsupported);
        assert_eq!(
            v32_fault.code,
            "update-texture-partial-dirty-region-variant-unsupported"
        );
        assert_eq!(memory.stats(), before_v32_memory);
        assert_eq!(runtime, before_v32_runtime);

        let later_partial_update = match dispatch_d3d9_fixture_frontend(
            &mut memory,
            &mut runtime,
            &D3d9CompatibilityPolicy::controlled_v33(),
            &device_interface,
            31,
            GuestAddress(0x1005_b038),
        ) {
            ComDispatchOutcome::Completed(completion) => completion,
            other => panic!("v33 later partial UpdateTexture did not complete: {other:?}"),
        };
        assert_eq!(
            later_partial_update.return_value,
            ComReturnValue::HResult(COM_S_OK)
        );
        let updated_source = runtime
            .graphics_texture_for_com_object(recreated_system_texture_interface.object_id)
            .unwrap();
        let updated_destination = runtime
            .graphics_texture_for_com_object(recreated_default_texture_interface.object_id)
            .unwrap();
        assert_eq!(updated_source.content.content_generation, 3);
        assert_eq!(
            updated_source.dirty_region,
            RuntimeGraphicsTextureDirtyRegion::Clean
        );
        assert_eq!(updated_destination.content.content_generation, 3);
        assert_eq!(
            updated_destination.content.pixel_source,
            updated_source.content.pixel_source
        );
        let later_update = updated_destination.content.last_update.unwrap();
        assert_eq!(later_update.sequence, 3);
        assert_eq!(later_update.source_content_generation, 3);
        assert_eq!(
            later_update.dirty_region,
            RuntimeGraphicsTextureDirtyRegion::Rect(later_partial_rect)
        );

        let destination_texture_id = updated_destination.id;
        let texture_reference_count = runtime
            .com()
            .object(recreated_default_texture_interface.object_id)
            .unwrap()
            .reference_count;
        write_d3d9_fixture_words(
            &mut memory,
            D3D9_FIXTURE_STACK,
            &[
                0x1005_b27c,
                device_interface.guest_pointer.0,
                0,
                recreated_default_texture_interface.guest_pointer.0,
            ],
        );
        acknowledge_d3d9_fixture_memory(&mut memory);
        let set_texture = match dispatch_d3d9_fixture_frontend(
            &mut memory,
            &mut runtime,
            &D3d9CompatibilityPolicy::controlled_v31(),
            &device_interface,
            65,
            GuestAddress(0x1005_b27c),
        ) {
            ComDispatchOutcome::Completed(completion) => completion,
            other => panic!("SetTexture did not complete: {other:?}"),
        };
        assert_eq!(set_texture.return_value, ComReturnValue::HResult(COM_S_OK));
        assert_eq!(
            set_texture.stack_pointer,
            GuestAddress(D3D9_FIXTURE_STACK.0 + 16)
        );
        assert!(set_texture.guest_writes.is_empty());
        assert_eq!(
            runtime
                .graphics_device(graphics.id)
                .unwrap()
                .texture_bindings
                .stage_zero,
            Some(destination_texture_id)
        );
        assert_eq!(
            runtime
                .com()
                .object(recreated_default_texture_interface.object_id)
                .unwrap()
                .reference_count,
            texture_reference_count,
            "SetTexture must not mutate the texture COM reference count"
        );
        assert_eq!(runtime.current_last_error().unwrap(), 0x1234_5678);

        let before_repeat_binding_memory = memory.stats();
        let before_repeat_binding_runtime = runtime.clone();
        let repeat_binding = match dispatch_d3d9_fixture_frontend(
            &mut memory,
            &mut runtime,
            &D3d9CompatibilityPolicy::controlled_v31(),
            &device_interface,
            65,
            GuestAddress(0x1005_b27c),
        ) {
            ComDispatchOutcome::Completed(completion) => completion,
            other => panic!("repeated SetTexture did not complete: {other:?}"),
        };
        assert_eq!(
            repeat_binding.return_value,
            ComReturnValue::HResult(COM_S_OK)
        );
        assert_eq!(memory.stats(), before_repeat_binding_memory);
        assert_eq!(runtime, before_repeat_binding_runtime);

        write_d3d9_fixture_words(
            &mut memory,
            D3D9_FIXTURE_STACK,
            &[0x1005_b291, device_interface.guest_pointer.0, 0x144],
        );
        acknowledge_d3d9_fixture_memory(&mut memory);
        let set_fvf = match dispatch_d3d9_fixture_frontend(
            &mut memory,
            &mut runtime,
            &D3d9CompatibilityPolicy::controlled_v31(),
            &device_interface,
            89,
            GuestAddress(0x1005_b291),
        ) {
            ComDispatchOutcome::Completed(completion) => completion,
            other => panic!("SetFVF did not complete: {other:?}"),
        };
        assert_eq!(set_fvf.return_value, ComReturnValue::HResult(COM_S_OK));
        assert_eq!(
            set_fvf.stack_pointer,
            GuestAddress(D3D9_FIXTURE_STACK.0 + 12)
        );
        assert!(set_fvf.guest_writes.is_empty());
        assert_eq!(
            runtime
                .graphics_device(graphics.id)
                .unwrap()
                .flexible_vertex_format,
            Some(RuntimeGraphicsFlexibleVertexFormat::XyzRhwDiffuseTexture1)
        );
        assert_eq!(runtime.current_last_error().unwrap(), 0x1234_5678);

        let before_repeat_fvf_memory = memory.stats();
        let before_repeat_fvf_runtime = runtime.clone();
        let repeat_fvf = match dispatch_d3d9_fixture_frontend(
            &mut memory,
            &mut runtime,
            &D3d9CompatibilityPolicy::controlled_v31(),
            &device_interface,
            89,
            GuestAddress(0x1005_b291),
        ) {
            ComDispatchOutcome::Completed(completion) => completion,
            other => panic!("repeated SetFVF did not complete: {other:?}"),
        };
        assert_eq!(repeat_fvf.return_value, ComReturnValue::HResult(COM_S_OK));
        assert_eq!(memory.stats(), before_repeat_fvf_memory);
        assert_eq!(runtime, before_repeat_fvf_runtime);

        for unsupported_fvf in [0, 2, u32::MAX] {
            write_d3d9_fixture_words(
                &mut memory,
                D3D9_FIXTURE_STACK,
                &[
                    0x1005_b291,
                    device_interface.guest_pointer.0,
                    unsupported_fvf,
                ],
            );
            acknowledge_d3d9_fixture_memory(&mut memory);
            let before_memory = memory.stats();
            let before_runtime = runtime.clone();
            let rejected = match dispatch_d3d9_fixture_frontend(
                &mut memory,
                &mut runtime,
                &D3d9CompatibilityPolicy::controlled_v31(),
                &device_interface,
                89,
                GuestAddress(0x1005_b291),
            ) {
                ComDispatchOutcome::Completed(completion) => completion,
                other => panic!("unsupported SetFVF did not complete: {other:?}"),
            };
            assert_eq!(
                rejected.return_value,
                ComReturnValue::HResult(D3D9_D3DERR_NOTAVAILABLE)
            );
            assert!(rejected.guest_writes.is_empty());
            assert_eq!(memory.stats(), before_memory);
            assert_eq!(runtime, before_runtime);
        }

        write_d3d9_fixture_words(
            &mut memory,
            D3D9_FIXTURE_STACK,
            &[0x1005_b291, device_interface.guest_pointer.0, 0x144],
        );
        acknowledge_d3d9_fixture_memory(&mut memory);
        let mut disabled_fvf = D3d9CompatibilityPolicy::controlled_v31();
        disabled_fvf.flexible_vertex_format = None;
        disabled_fvf.stream_source = None;
        disabled_fvf.draw_primitive = None;
        disabled_fvf.present = None;
        let before_disabled_fvf_memory = memory.stats();
        let before_disabled_fvf_runtime = runtime.clone();
        let disabled_fvf_fault = match dispatch_d3d9_fixture_frontend(
            &mut memory,
            &mut runtime,
            &disabled_fvf,
            &device_interface,
            89,
            GuestAddress(0x1005_b291),
        ) {
            ComDispatchOutcome::Fault(fault) => fault,
            other => panic!("disabled SetFVF did not fault: {other:?}"),
        };
        assert_eq!(disabled_fvf_fault.class, ComFaultClass::Unsupported);
        assert_eq!(disabled_fvf_fault.code, "set-fvf-policy-disabled");
        assert_eq!(memory.stats(), before_disabled_fvf_memory);
        assert_eq!(runtime, before_disabled_fvf_runtime);

        for (stage, texture_pointer, expected_result) in [
            (
                1,
                recreated_default_texture_interface.guest_pointer.0,
                D3D9_D3DERR_NOTAVAILABLE,
            ),
            (
                0,
                recreated_system_texture_interface.guest_pointer.0,
                D3D9_D3DERR_NOTAVAILABLE,
            ),
            (0, root_interface.guest_pointer.0, D3D9_D3DERR_INVALIDCALL),
            (0, 0x0102_0304, D3D9_D3DERR_INVALIDCALL),
        ] {
            write_d3d9_fixture_words(
                &mut memory,
                D3D9_FIXTURE_STACK,
                &[
                    0x1005_b27c,
                    device_interface.guest_pointer.0,
                    stage,
                    texture_pointer,
                ],
            );
            acknowledge_d3d9_fixture_memory(&mut memory);
            let before_memory = memory.stats();
            let before_runtime = runtime.clone();
            let rejected = match dispatch_d3d9_fixture_frontend(
                &mut memory,
                &mut runtime,
                &D3d9CompatibilityPolicy::controlled_v31(),
                &device_interface,
                65,
                GuestAddress(0x1005_b27c),
            ) {
                ComDispatchOutcome::Completed(completion) => completion,
                other => panic!("rejected SetTexture did not complete: {other:?}"),
            };
            assert_eq!(
                rejected.return_value,
                ComReturnValue::HResult(expected_result)
            );
            assert!(rejected.guest_writes.is_empty());
            assert_eq!(memory.stats(), before_memory);
            assert_eq!(runtime, before_runtime);
        }

        write_d3d9_fixture_words(
            &mut memory,
            D3D9_FIXTURE_STACK,
            &[
                0x1005_b27c,
                device_interface.guest_pointer.0,
                0,
                recreated_default_texture_interface.guest_pointer.0,
            ],
        );
        acknowledge_d3d9_fixture_memory(&mut memory);
        let mut disabled_texture_binding = D3d9CompatibilityPolicy::controlled_v31();
        disabled_texture_binding.texture_binding = None;
        disabled_texture_binding.draw_primitive = None;
        disabled_texture_binding.present = None;
        let before_disabled_binding_memory = memory.stats();
        let before_disabled_binding_runtime = runtime.clone();
        let disabled_binding = match dispatch_d3d9_fixture_frontend(
            &mut memory,
            &mut runtime,
            &disabled_texture_binding,
            &device_interface,
            65,
            GuestAddress(0x1005_b27c),
        ) {
            ComDispatchOutcome::Fault(fault) => fault,
            other => panic!("disabled SetTexture did not fault: {other:?}"),
        };
        assert_eq!(disabled_binding.class, ComFaultClass::Unsupported);
        assert_eq!(disabled_binding.code, "set-texture-policy-disabled");
        assert_eq!(memory.stats(), before_disabled_binding_memory);
        assert_eq!(runtime, before_disabled_binding_runtime);

        write_d3d9_fixture_words(
            &mut memory,
            D3D9_FIXTURE_STACK,
            &[0x1005_b27c, device_interface.guest_pointer.0, 0, 0],
        );
        acknowledge_d3d9_fixture_memory(&mut memory);
        let unbind = match dispatch_d3d9_fixture_frontend(
            &mut memory,
            &mut runtime,
            &D3d9CompatibilityPolicy::controlled_v31(),
            &device_interface,
            65,
            GuestAddress(0x1005_b27c),
        ) {
            ComDispatchOutcome::Completed(completion) => completion,
            other => panic!("NULL SetTexture did not complete: {other:?}"),
        };
        assert_eq!(unbind.return_value, ComReturnValue::HResult(COM_S_OK));
        assert_eq!(
            runtime
                .graphics_device(graphics.id)
                .unwrap()
                .texture_bindings
                .stage_zero,
            None
        );
        assert_eq!(
            runtime
                .com()
                .object(recreated_default_texture_interface.object_id)
                .unwrap()
                .reference_count,
            texture_reference_count
        );

        write_d3d9_fixture_words(
            &mut memory,
            D3D9_FIXTURE_STACK,
            &[
                0x1005_b038,
                device_interface.guest_pointer.0,
                recreated_system_texture_interface.guest_pointer.0,
                recreated_default_texture_interface.guest_pointer.0,
            ],
        );
        acknowledge_d3d9_fixture_memory(&mut memory);

        let before_repeat_memory = memory.stats();
        let before_repeat_runtime = runtime.clone();
        let repeat_update = {
            let mut session = ExecutionSession::new(&mut memory, &mut runtime);
            match dispatch_d3d9_com_call(
                &D3d9CompatibilityPolicy::controlled_v31(),
                ComCallTrap {
                    endpoint_address: device_interface.methods[&31].endpoint,
                    stack_pointer: D3D9_FIXTURE_STACK,
                    return_address: GuestAddress(0x1005_b038),
                    call_site: GuestAddress(0x1005_b038),
                },
                &mut session,
            ) {
                ComDispatchOutcome::Completed(completion) => completion,
                other => panic!("repeated UpdateTexture did not complete: {other:?}"),
            }
        };
        assert_eq!(
            repeat_update.return_value,
            ComReturnValue::HResult(COM_S_OK)
        );
        assert_eq!(memory.stats(), before_repeat_memory);
        assert_eq!(runtime, before_repeat_runtime);

        write_d3d9_fixture_words(&mut memory, dirty_rect_address, &[1, 0, 1_280, 720]);
        write_d3d9_fixture_words(
            &mut memory,
            D3D9_FIXTURE_STACK,
            &[
                0x1005_b01d,
                recreated_system_texture_interface.guest_pointer.0,
                dirty_rect_address.0,
            ],
        );
        acknowledge_d3d9_fixture_memory(&mut memory);
        let dirty_variant = match dispatch_d3d9_fixture_frontend(
            &mut memory,
            &mut runtime,
            &D3d9CompatibilityPolicy::controlled_v31(),
            &recreated_system_texture_interface,
            21,
            GuestAddress(0x1005_b01d),
        ) {
            ComDispatchOutcome::Completed(completion) => completion,
            other => panic!("partial dirty-region variant did not complete: {other:?}"),
        };
        assert_eq!(
            dirty_variant.return_value,
            ComReturnValue::HResult(COM_S_OK)
        );
        write_d3d9_fixture_words(
            &mut memory,
            D3D9_FIXTURE_STACK,
            &[
                0x1005_b038,
                device_interface.guest_pointer.0,
                recreated_system_texture_interface.guest_pointer.0,
                recreated_default_texture_interface.guest_pointer.0,
            ],
        );
        acknowledge_d3d9_fixture_memory(&mut memory);
        let before_partial_variant_memory = memory.stats();
        let before_partial_variant_runtime = runtime.clone();
        let partial_variant_fault = match dispatch_d3d9_fixture_frontend(
            &mut memory,
            &mut runtime,
            &D3d9CompatibilityPolicy::controlled_v31(),
            &device_interface,
            31,
            GuestAddress(0x1005_b038),
        ) {
            ComDispatchOutcome::Fault(fault) => fault,
            other => panic!("unsupported partial UpdateTexture did not fault: {other:?}"),
        };
        assert_eq!(partial_variant_fault.class, ComFaultClass::Unsupported);
        assert_eq!(
            partial_variant_fault.code,
            "update-texture-partial-dirty-region-variant-unsupported"
        );
        assert_eq!(memory.stats(), before_partial_variant_memory);
        assert_eq!(runtime, before_partial_variant_runtime);

        for (source_pointer, destination_pointer) in [
            (
                recreated_system_texture_interface.guest_pointer.0,
                recreated_system_texture_interface.guest_pointer.0,
            ),
            (
                recreated_default_texture_interface.guest_pointer.0,
                recreated_system_texture_interface.guest_pointer.0,
            ),
            (0, recreated_default_texture_interface.guest_pointer.0),
        ] {
            write_d3d9_fixture_words(
                &mut memory,
                D3D9_FIXTURE_STACK,
                &[
                    0x1005_b038,
                    device_interface.guest_pointer.0,
                    source_pointer,
                    destination_pointer,
                ],
            );
            acknowledge_d3d9_fixture_memory(&mut memory);
            let before_memory = memory.stats();
            let before_runtime = runtime.clone();
            let completion = {
                let mut session = ExecutionSession::new(&mut memory, &mut runtime);
                match dispatch_d3d9_com_call(
                    &D3d9CompatibilityPolicy::controlled_v31(),
                    ComCallTrap {
                        endpoint_address: device_interface.methods[&31].endpoint,
                        stack_pointer: D3D9_FIXTURE_STACK,
                        return_address: GuestAddress(0x1005_b038),
                        call_site: GuestAddress(0x1005_b038),
                    },
                    &mut session,
                ) {
                    ComDispatchOutcome::Completed(completion) => completion,
                    other => panic!("invalid UpdateTexture did not complete: {other:?}"),
                }
            };
            assert_eq!(
                completion.return_value,
                ComReturnValue::HResult(D3D9_D3DERR_INVALIDCALL)
            );
            assert_eq!(memory.stats(), before_memory);
            assert_eq!(runtime, before_runtime);
        }

        write_d3d9_fixture_words(
            &mut memory,
            D3D9_FIXTURE_STACK,
            &[
                0x1005_b038,
                device_interface.guest_pointer.0,
                recreated_system_texture_interface.guest_pointer.0,
                additional_texture_interfaces[4].guest_pointer.0,
            ],
        );
        acknowledge_d3d9_fixture_memory(&mut memory);
        let before_dynamic_memory = memory.stats();
        let before_dynamic_runtime = runtime.clone();
        let dynamic_fault = {
            let mut session = ExecutionSession::new(&mut memory, &mut runtime);
            match dispatch_d3d9_com_call(
                &D3d9CompatibilityPolicy::controlled_v31(),
                ComCallTrap {
                    endpoint_address: device_interface.methods[&31].endpoint,
                    stack_pointer: D3D9_FIXTURE_STACK,
                    return_address: GuestAddress(0x1005_b038),
                    call_site: GuestAddress(0x1005_b038),
                },
                &mut session,
            ) {
                ComDispatchOutcome::Fault(fault) => fault,
                other => panic!("unsupported dynamic UpdateTexture did not fault: {other:?}"),
            }
        };
        assert_eq!(dynamic_fault.class, ComFaultClass::Unsupported);
        assert_eq!(
            dynamic_fault.code,
            "update-texture-destination-variant-unsupported"
        );
        assert_eq!(memory.stats(), before_dynamic_memory);
        assert_eq!(runtime, before_dynamic_runtime);

        let mut disabled_update_texture = D3d9CompatibilityPolicy::controlled_v31();
        disabled_update_texture.update_texture = None;
        disabled_update_texture.texture_binding = None;
        disabled_update_texture.draw_primitive = None;
        disabled_update_texture.present = None;
        let before_disabled_update_memory = memory.stats();
        let before_disabled_update_runtime = runtime.clone();
        let disabled_update_fault = {
            let mut session = ExecutionSession::new(&mut memory, &mut runtime);
            match dispatch_d3d9_com_call(
                &disabled_update_texture,
                ComCallTrap {
                    endpoint_address: device_interface.methods[&31].endpoint,
                    stack_pointer: D3D9_FIXTURE_STACK,
                    return_address: GuestAddress(0x1005_b038),
                    call_site: GuestAddress(0x1005_b038),
                },
                &mut session,
            ) {
                ComDispatchOutcome::Fault(fault) => fault,
                other => panic!("disabled UpdateTexture did not fault: {other:?}"),
            }
        };
        assert_eq!(disabled_update_fault.class, ComFaultClass::Unsupported);
        assert_eq!(disabled_update_fault.code, "update-texture-policy-disabled");
        assert_eq!(memory.stats(), before_disabled_update_memory);
        assert_eq!(runtime, before_disabled_update_runtime);

        for (byte_length, output_pointer, expected_result) in [
            (0, D3D9_FIXTURE_SURFACE_OUTPUT.0, D3D9_D3DERR_INVALIDCALL),
            (116, D3D9_FIXTURE_SURFACE_OUTPUT.0, D3D9_D3DERR_NOTAVAILABLE),
            (112, 0, D3D9_D3DERR_INVALIDCALL),
        ] {
            write_d3d9_fixture_words(&mut memory, D3D9_FIXTURE_SURFACE_OUTPUT, &[0xcccc_cccc]);
            write_d3d9_fixture_words(
                &mut memory,
                D3D9_FIXTURE_STACK,
                &[
                    0x1005_a890,
                    device_interface.guest_pointer.0,
                    byte_length,
                    D3D9_D3DUSAGE_WRITEONLY,
                    0x144,
                    D3D9_D3DPOOL_MANAGED,
                    output_pointer,
                    0,
                ],
            );
            acknowledge_d3d9_fixture_memory(&mut memory);
            let before_memory = memory.stats();
            let before_runtime = runtime.clone();
            let completion = {
                let mut session = ExecutionSession::new(&mut memory, &mut runtime);
                match dispatch_d3d9_com_call(
                    &D3d9CompatibilityPolicy::controlled_v31(),
                    ComCallTrap {
                        endpoint_address: device_interface.methods[&26].endpoint,
                        stack_pointer: D3D9_FIXTURE_STACK,
                        return_address: GuestAddress(0x1005_a890),
                        call_site: GuestAddress(0x1005_a890),
                    },
                    &mut session,
                ) {
                    ComDispatchOutcome::Completed(completion) => completion,
                    other => panic!("negative CreateVertexBuffer did not complete: {other:?}"),
                }
            };
            assert_eq!(
                completion.return_value,
                ComReturnValue::HResult(expected_result)
            );
            assert!(completion.guest_writes.is_empty());
            assert_eq!(memory.stats(), before_memory);
            assert_eq!(runtime, before_runtime);
            assert_eq!(
                memory.read(D3D9_FIXTURE_SURFACE_OUTPUT, 4).unwrap(),
                0xcccc_cccc_u32.to_le_bytes()
            );
        }

        write_d3d9_fixture_words(&mut memory, D3D9_FIXTURE_SURFACE_OUTPUT, &[0xcccc_cccc]);
        write_d3d9_fixture_words(
            &mut memory,
            D3D9_FIXTURE_STACK,
            &[
                0x1005_a890,
                device_interface.guest_pointer.0,
                112,
                D3D9_D3DUSAGE_WRITEONLY,
                0x144,
                D3D9_D3DPOOL_MANAGED,
                D3D9_FIXTURE_SURFACE_OUTPUT.0,
                0,
            ],
        );
        acknowledge_d3d9_fixture_memory(&mut memory);
        let create_vertex_buffer = {
            let mut session = ExecutionSession::new(&mut memory, &mut runtime);
            match dispatch_d3d9_com_call(
                &D3d9CompatibilityPolicy::controlled_v31(),
                ComCallTrap {
                    endpoint_address: device_interface.methods[&26].endpoint,
                    stack_pointer: D3D9_FIXTURE_STACK,
                    return_address: GuestAddress(0x1005_a890),
                    call_site: GuestAddress(0x1005_a890),
                },
                &mut session,
            ) {
                ComDispatchOutcome::Completed(completion) => completion,
                other => panic!("CreateVertexBuffer did not complete: {other:?}"),
            }
        };
        assert_eq!(
            create_vertex_buffer.return_value,
            ComReturnValue::HResult(COM_S_OK)
        );
        assert_eq!(create_vertex_buffer.guest_writes.len(), 1);
        assert_eq!(
            create_vertex_buffer.stack_pointer,
            GuestAddress(D3D9_FIXTURE_STACK.0 + 32)
        );
        acknowledge_d3d9_fixture_memory(&mut memory);
        let vertex_buffer_pointer = u32::from_le_bytes(
            memory
                .read(D3D9_FIXTURE_SURFACE_OUTPUT, 4)
                .unwrap()
                .try_into()
                .unwrap(),
        );
        let vertex_buffer_interface = runtime
            .com_interface_for_pointer(GuestAddress(vertex_buffer_pointer))
            .expect("IDirect3DVertexBuffer9 output")
            .clone();
        assert_eq!(vertex_buffer_interface.iid, D3D9_IDIRECT3DVERTEXBUFFER9_IID);
        assert_eq!(vertex_buffer_interface.methods.len(), 14);
        assert_eq!(vertex_buffer_interface.methods[&11].name, "Lock");
        assert_eq!(vertex_buffer_interface.methods[&12].name, "Unlock");
        let vertex_buffer = runtime
            .graphics_vertex_buffers()
            .values()
            .next()
            .expect("logical graphics vertex buffer")
            .clone();
        assert_eq!(vertex_buffer.device_id, graphics.id);
        assert_eq!(vertex_buffer.descriptor.byte_length, 112);
        assert_eq!(
            vertex_buffer.descriptor.usage,
            RuntimeGraphicsVertexBufferUsage::WriteOnly
        );
        assert_eq!(vertex_buffer.descriptor.flexible_vertex_format, 0x144);
        assert_eq!(
            vertex_buffer.descriptor.pool,
            RuntimeGraphicsVertexBufferPool::Managed
        );
        assert_eq!(vertex_buffer.content_generation, 0);
        assert!(vertex_buffer.active_lock.is_none());
        assert_eq!(
            memory
                .read(vertex_buffer.backing.range.start(), 112)
                .unwrap(),
            vec![0; 112]
        );
        assert_eq!(
            runtime
                .com()
                .object(device_object.id)
                .unwrap()
                .reference_count,
            8
        );

        write_d3d9_fixture_words(
            &mut memory,
            D3D9_FIXTURE_STACK,
            &[
                0x1005_aa96,
                vertex_buffer_interface.guest_pointer.0,
                4,
                0,
                D3D9_FIXTURE_DC_OUTPUT.0,
                0,
            ],
        );
        acknowledge_d3d9_fixture_memory(&mut memory);
        let before_memory = memory.stats();
        let before_runtime = runtime.clone();
        let invalid_lock = {
            let mut session = ExecutionSession::new(&mut memory, &mut runtime);
            match dispatch_d3d9_com_call(
                &D3d9CompatibilityPolicy::controlled_v31(),
                ComCallTrap {
                    endpoint_address: vertex_buffer_interface.methods[&11].endpoint,
                    stack_pointer: D3D9_FIXTURE_STACK,
                    return_address: GuestAddress(0x1005_aa96),
                    call_site: GuestAddress(0x1005_aa96),
                },
                &mut session,
            ) {
                ComDispatchOutcome::Completed(completion) => completion,
                other => panic!("invalid IDirect3DVertexBuffer9::Lock did not complete: {other:?}"),
            }
        };
        assert_eq!(
            invalid_lock.return_value,
            ComReturnValue::HResult(D3D9_D3DERR_INVALIDCALL)
        );
        assert!(invalid_lock.guest_writes.is_empty());
        assert_eq!(memory.stats(), before_memory);
        assert_eq!(runtime, before_runtime);

        write_d3d9_fixture_words(&mut memory, D3D9_FIXTURE_DC_OUTPUT, &[0xcccc_cccc]);
        write_d3d9_fixture_words(
            &mut memory,
            D3D9_FIXTURE_STACK,
            &[
                0x1005_aa96,
                vertex_buffer_interface.guest_pointer.0,
                0,
                0,
                D3D9_FIXTURE_DC_OUTPUT.0,
                0,
            ],
        );
        acknowledge_d3d9_fixture_memory(&mut memory);
        let lock = {
            let mut session = ExecutionSession::new(&mut memory, &mut runtime);
            match dispatch_d3d9_com_call(
                &D3d9CompatibilityPolicy::controlled_v31(),
                ComCallTrap {
                    endpoint_address: vertex_buffer_interface.methods[&11].endpoint,
                    stack_pointer: D3D9_FIXTURE_STACK,
                    return_address: GuestAddress(0x1005_aa96),
                    call_site: GuestAddress(0x1005_aa96),
                },
                &mut session,
            ) {
                ComDispatchOutcome::Completed(completion) => completion,
                other => panic!("IDirect3DVertexBuffer9::Lock did not complete: {other:?}"),
            }
        };
        assert_eq!(lock.return_value, ComReturnValue::HResult(COM_S_OK));
        assert_eq!(lock.guest_writes.len(), 1);
        assert_eq!(lock.stack_pointer, GuestAddress(D3D9_FIXTURE_STACK.0 + 24));
        acknowledge_d3d9_fixture_memory(&mut memory);
        let vertex_data_pointer = u32::from_le_bytes(
            memory
                .read(D3D9_FIXTURE_DC_OUTPUT, 4)
                .unwrap()
                .try_into()
                .unwrap(),
        );
        assert_eq!(
            GuestAddress(vertex_data_pointer),
            vertex_buffer.backing.range.start()
        );

        let before_memory = memory.stats();
        let before_runtime = runtime.clone();
        let duplicate_lock = {
            let mut session = ExecutionSession::new(&mut memory, &mut runtime);
            match dispatch_d3d9_com_call(
                &D3d9CompatibilityPolicy::controlled_v31(),
                ComCallTrap {
                    endpoint_address: vertex_buffer_interface.methods[&11].endpoint,
                    stack_pointer: D3D9_FIXTURE_STACK,
                    return_address: GuestAddress(0x1005_aa96),
                    call_site: GuestAddress(0x1005_aa96),
                },
                &mut session,
            ) {
                ComDispatchOutcome::Completed(completion) => completion,
                other => {
                    panic!("duplicate IDirect3DVertexBuffer9::Lock did not complete: {other:?}")
                }
            }
        };
        assert_eq!(
            duplicate_lock.return_value,
            ComReturnValue::HResult(D3D9_D3DERR_INVALIDCALL)
        );
        assert!(duplicate_lock.guest_writes.is_empty());
        assert_eq!(memory.stats(), before_memory);
        assert_eq!(runtime, before_runtime);

        let vertex_bytes = (0_u8..112).collect::<Vec<_>>();
        memory
            .write(GuestAddress(vertex_data_pointer), &vertex_bytes)
            .expect("write locked vertex-buffer bytes");
        acknowledge_d3d9_fixture_memory(&mut memory);

        write_d3d9_fixture_words(
            &mut memory,
            D3D9_FIXTURE_STACK,
            &[0x1005_aab2, vertex_buffer_interface.guest_pointer.0],
        );
        acknowledge_d3d9_fixture_memory(&mut memory);
        let unlock = {
            let mut session = ExecutionSession::new(&mut memory, &mut runtime);
            match dispatch_d3d9_com_call(
                &D3d9CompatibilityPolicy::controlled_v31(),
                ComCallTrap {
                    endpoint_address: vertex_buffer_interface.methods[&12].endpoint,
                    stack_pointer: D3D9_FIXTURE_STACK,
                    return_address: GuestAddress(0x1005_aab2),
                    call_site: GuestAddress(0x1005_aab2),
                },
                &mut session,
            ) {
                ComDispatchOutcome::Completed(completion) => completion,
                other => panic!("IDirect3DVertexBuffer9::Unlock did not complete: {other:?}"),
            }
        };
        assert_eq!(unlock.return_value, ComReturnValue::HResult(COM_S_OK));
        assert!(unlock.guest_writes.is_empty());
        acknowledge_d3d9_fixture_memory(&mut memory);
        let committed_vertex_buffer = runtime
            .graphics_vertex_buffer(vertex_buffer.id)
            .expect("committed logical graphics vertex buffer");
        assert!(committed_vertex_buffer.active_lock.is_none());
        assert_eq!(committed_vertex_buffer.content_generation, 1);
        assert_eq!(
            memory.read(GuestAddress(vertex_data_pointer), 112).unwrap(),
            vertex_bytes
        );
        assert!(
            memory
                .write(GuestAddress(vertex_data_pointer), &[0xff])
                .is_err()
        );

        let before_memory = memory.stats();
        let before_runtime = runtime.clone();
        let repeated_unlock = {
            let mut session = ExecutionSession::new(&mut memory, &mut runtime);
            match dispatch_d3d9_com_call(
                &D3d9CompatibilityPolicy::controlled_v31(),
                ComCallTrap {
                    endpoint_address: vertex_buffer_interface.methods[&12].endpoint,
                    stack_pointer: D3D9_FIXTURE_STACK,
                    return_address: GuestAddress(0x1005_aab2),
                    call_site: GuestAddress(0x1005_aab2),
                },
                &mut session,
            ) {
                ComDispatchOutcome::Completed(completion) => completion,
                other => {
                    panic!("repeated IDirect3DVertexBuffer9::Unlock did not complete: {other:?}")
                }
            }
        };
        assert_eq!(
            repeated_unlock.return_value,
            ComReturnValue::HResult(D3D9_D3DERR_INVALIDCALL)
        );
        assert!(repeated_unlock.guest_writes.is_empty());
        assert_eq!(memory.stats(), before_memory);
        assert_eq!(runtime, before_runtime);

        let vertex_buffer_reference_count = runtime
            .com()
            .object(vertex_buffer_interface.object_id)
            .unwrap()
            .reference_count;
        write_d3d9_fixture_words(
            &mut memory,
            D3D9_FIXTURE_STACK,
            &[
                0x1005_b3ec,
                device_interface.guest_pointer.0,
                0,
                vertex_buffer_interface.guest_pointer.0,
                0,
                28,
            ],
        );
        acknowledge_d3d9_fixture_memory(&mut memory);
        let set_stream_source = match dispatch_d3d9_fixture_frontend(
            &mut memory,
            &mut runtime,
            &D3d9CompatibilityPolicy::controlled_v31(),
            &device_interface,
            100,
            GuestAddress(0x1005_b3ec),
        ) {
            ComDispatchOutcome::Completed(completion) => completion,
            other => panic!("SetStreamSource did not complete: {other:?}"),
        };
        assert_eq!(
            set_stream_source.return_value,
            ComReturnValue::HResult(COM_S_OK)
        );
        assert_eq!(
            set_stream_source.stack_pointer,
            GuestAddress(D3D9_FIXTURE_STACK.0 + 24)
        );
        assert!(set_stream_source.guest_writes.is_empty());
        let stream_source = runtime
            .graphics_device(graphics.id)
            .unwrap()
            .stream_sources
            .stream_zero
            .expect("logical stream-zero source binding");
        assert_eq!(stream_source.vertex_buffer_id, vertex_buffer.id);
        assert_eq!(stream_source.offset_in_bytes, 0);
        assert_eq!(stream_source.stride, 28);
        assert_eq!(
            runtime
                .com()
                .object(vertex_buffer_interface.object_id)
                .unwrap()
                .reference_count,
            vertex_buffer_reference_count,
            "SetStreamSource must not mutate the vertex-buffer COM reference count"
        );
        assert_eq!(runtime.current_last_error().unwrap(), 0x1234_5678);

        let before_repeat_stream_memory = memory.stats();
        let before_repeat_stream_runtime = runtime.clone();
        let repeat_stream_source = match dispatch_d3d9_fixture_frontend(
            &mut memory,
            &mut runtime,
            &D3d9CompatibilityPolicy::controlled_v31(),
            &device_interface,
            100,
            GuestAddress(0x1005_b3ec),
        ) {
            ComDispatchOutcome::Completed(completion) => completion,
            other => panic!("repeated SetStreamSource did not complete: {other:?}"),
        };
        assert_eq!(
            repeat_stream_source.return_value,
            ComReturnValue::HResult(COM_S_OK)
        );
        assert_eq!(memory.stats(), before_repeat_stream_memory);
        assert_eq!(runtime, before_repeat_stream_runtime);

        for (stream, vertex_buffer_pointer, offset, stride, expected_result) in [
            (
                1,
                vertex_buffer_interface.guest_pointer.0,
                0,
                28,
                D3D9_D3DERR_NOTAVAILABLE,
            ),
            (0, 0, 0, 28, D3D9_D3DERR_NOTAVAILABLE),
            (
                0,
                vertex_buffer_interface.guest_pointer.0,
                4,
                28,
                D3D9_D3DERR_NOTAVAILABLE,
            ),
            (
                0,
                vertex_buffer_interface.guest_pointer.0,
                0,
                0,
                D3D9_D3DERR_NOTAVAILABLE,
            ),
            (
                0,
                vertex_buffer_interface.guest_pointer.0,
                0,
                32,
                D3D9_D3DERR_NOTAVAILABLE,
            ),
            (
                0,
                root_interface.guest_pointer.0,
                0,
                28,
                D3D9_D3DERR_INVALIDCALL,
            ),
            (0, 0x0102_0304, 0, 28, D3D9_D3DERR_INVALIDCALL),
        ] {
            write_d3d9_fixture_words(
                &mut memory,
                D3D9_FIXTURE_STACK,
                &[
                    0x1005_b3ec,
                    device_interface.guest_pointer.0,
                    stream,
                    vertex_buffer_pointer,
                    offset,
                    stride,
                ],
            );
            acknowledge_d3d9_fixture_memory(&mut memory);
            let before_memory = memory.stats();
            let before_runtime = runtime.clone();
            let rejected = match dispatch_d3d9_fixture_frontend(
                &mut memory,
                &mut runtime,
                &D3d9CompatibilityPolicy::controlled_v31(),
                &device_interface,
                100,
                GuestAddress(0x1005_b3ec),
            ) {
                ComDispatchOutcome::Completed(completion) => completion,
                other => panic!("rejected SetStreamSource did not complete: {other:?}"),
            };
            assert_eq!(
                rejected.return_value,
                ComReturnValue::HResult(expected_result)
            );
            assert!(rejected.guest_writes.is_empty());
            assert_eq!(memory.stats(), before_memory);
            assert_eq!(runtime, before_runtime);
        }

        write_d3d9_fixture_words(
            &mut memory,
            D3D9_FIXTURE_STACK,
            &[
                0x1005_b27c,
                device_interface.guest_pointer.0,
                0,
                recreated_default_texture_interface.guest_pointer.0,
            ],
        );
        acknowledge_d3d9_fixture_memory(&mut memory);
        let restore_texture_binding = match dispatch_d3d9_fixture_frontend(
            &mut memory,
            &mut runtime,
            &D3d9CompatibilityPolicy::controlled_v31(),
            &device_interface,
            65,
            GuestAddress(0x1005_b27c),
        ) {
            ComDispatchOutcome::Completed(completion) => completion,
            other => panic!("restored SetTexture did not complete: {other:?}"),
        };
        assert_eq!(
            restore_texture_binding.return_value,
            ComReturnValue::HResult(COM_S_OK)
        );

        write_d3d9_fixture_words(
            &mut memory,
            D3D9_FIXTURE_STACK,
            &[0x1005_b048, device_interface.guest_pointer.0],
        );
        acknowledge_d3d9_fixture_memory(&mut memory);
        let begin_scene = match dispatch_d3d9_fixture_frontend(
            &mut memory,
            &mut runtime,
            &D3d9CompatibilityPolicy::controlled_v31(),
            &device_interface,
            41,
            GuestAddress(0x1005_b048),
        ) {
            ComDispatchOutcome::Completed(completion) => completion,
            other => panic!("BeginScene did not complete: {other:?}"),
        };
        assert_eq!(begin_scene.return_value, ComReturnValue::HResult(COM_S_OK));

        let bound_texture = runtime
            .graphics_texture(destination_texture_id)
            .unwrap()
            .clone();
        let draw_texture_reference_count = runtime
            .com()
            .object(recreated_default_texture_interface.object_id)
            .unwrap()
            .reference_count;
        let device_reference_count = runtime
            .com()
            .object(device_object.id)
            .unwrap()
            .reference_count;
        write_d3d9_fixture_words(
            &mut memory,
            D3D9_FIXTURE_STACK,
            &[
                0x1005_b402,
                device_interface.guest_pointer.0,
                D3D9_D3DPT_TRIANGLESTRIP,
                0,
                2,
            ],
        );
        acknowledge_d3d9_fixture_memory(&mut memory);
        runtime
            .plan_graphics_draw_capture(RuntimeGraphicsDrawCaptureRequest {
                device_com_object_id: device_interface.object_id,
                primitive_topology: RuntimeGraphicsPrimitiveTopology::TriangleStrip,
                start_vertex: 0,
                primitive_count: 2,
            })
            .expect("prepared D3D9 fixture draw state");
        let before_draw_memory = memory.stats();
        let draw = match dispatch_d3d9_fixture_frontend(
            &mut memory,
            &mut runtime,
            &D3d9CompatibilityPolicy::controlled_v31(),
            &device_interface,
            81,
            GuestAddress(0x1005_b402),
        ) {
            ComDispatchOutcome::Completed(completion) => completion,
            other => panic!("DrawPrimitive did not complete: {other:?}"),
        };
        assert_eq!(draw.return_value, ComReturnValue::HResult(COM_S_OK));
        assert_eq!(draw.stack_pointer, GuestAddress(D3D9_FIXTURE_STACK.0 + 20));
        assert!(draw.guest_writes.is_empty());
        assert_eq!(memory.stats(), before_draw_memory);
        let drawn_device = runtime.graphics_device(graphics.id).unwrap();
        assert_eq!(drawn_device.back_buffer.content_generation, 2);
        assert_eq!(drawn_device.back_buffer.draw_submissions.len(), 1);
        let submission = &drawn_device.back_buffer.draw_submissions[0];
        assert_eq!(submission.sequence, 1);
        assert_eq!(submission.scene_sequence, 1);
        assert_eq!(
            submission.primitive_topology,
            RuntimeGraphicsPrimitiveTopology::TriangleStrip
        );
        assert_eq!(submission.start_vertex, 0);
        assert_eq!(submission.primitive_count, 2);
        assert_eq!(submission.vertex_count, 4);
        assert_eq!(submission.vertex_data, vertex_bytes);
        assert_eq!(submission.stream_source, stream_source);
        assert_eq!(submission.vertex_buffer_content_generation, 1);
        assert_eq!(submission.texture.texture_id, bound_texture.id);
        assert_eq!(submission.texture.content, bound_texture.content);
        assert_eq!(submission.texture.generation, bound_texture.generation);
        assert_eq!(runtime.current_last_error().unwrap(), 0x1234_5678);
        assert_eq!(
            runtime
                .com()
                .object(vertex_buffer_interface.object_id)
                .unwrap()
                .reference_count,
            vertex_buffer_reference_count
        );
        assert_eq!(
            runtime
                .com()
                .object(recreated_default_texture_interface.object_id)
                .unwrap()
                .reference_count,
            draw_texture_reference_count
        );
        assert_eq!(
            runtime
                .com()
                .object(device_object.id)
                .unwrap()
                .reference_count,
            device_reference_count
        );

        for (primitive_type, start_vertex, primitive_count) in [
            (4, 0, 2),
            (D3D9_D3DPT_TRIANGLESTRIP, 1, 2),
            (D3D9_D3DPT_TRIANGLESTRIP, 0, 1),
        ] {
            write_d3d9_fixture_words(
                &mut memory,
                D3D9_FIXTURE_STACK,
                &[
                    0x1005_b402,
                    device_interface.guest_pointer.0,
                    primitive_type,
                    start_vertex,
                    primitive_count,
                ],
            );
            acknowledge_d3d9_fixture_memory(&mut memory);
            let before_memory = memory.stats();
            let before_runtime = runtime.clone();
            let rejected = match dispatch_d3d9_fixture_frontend(
                &mut memory,
                &mut runtime,
                &D3d9CompatibilityPolicy::controlled_v31(),
                &device_interface,
                81,
                GuestAddress(0x1005_b402),
            ) {
                ComDispatchOutcome::Completed(completion) => completion,
                other => panic!("rejected DrawPrimitive did not complete: {other:?}"),
            };
            assert_eq!(
                rejected.return_value,
                ComReturnValue::HResult(D3D9_D3DERR_NOTAVAILABLE)
            );
            assert!(rejected.guest_writes.is_empty());
            assert_eq!(memory.stats(), before_memory);
            assert_eq!(runtime, before_runtime);
        }

        write_d3d9_fixture_words(
            &mut memory,
            D3D9_FIXTURE_STACK,
            &[
                0x1005_b402,
                device_interface.guest_pointer.0,
                D3D9_D3DPT_TRIANGLESTRIP,
                0,
                2,
            ],
        );
        acknowledge_d3d9_fixture_memory(&mut memory);
        let mut disabled_draw = D3d9CompatibilityPolicy::controlled_v31();
        disabled_draw.draw_primitive = None;
        disabled_draw.present = None;
        let before_disabled_draw_memory = memory.stats();
        let before_disabled_draw_runtime = runtime.clone();
        let disabled_draw_fault = match dispatch_d3d9_fixture_frontend(
            &mut memory,
            &mut runtime,
            &disabled_draw,
            &device_interface,
            81,
            GuestAddress(0x1005_b402),
        ) {
            ComDispatchOutcome::Fault(fault) => fault,
            other => panic!("disabled DrawPrimitive did not fault: {other:?}"),
        };
        assert_eq!(disabled_draw_fault.class, ComFaultClass::Unsupported);
        assert_eq!(disabled_draw_fault.code, "draw-primitive-policy-disabled");
        assert_eq!(memory.stats(), before_disabled_draw_memory);
        assert_eq!(runtime, before_disabled_draw_runtime);

        write_d3d9_fixture_words(
            &mut memory,
            D3D9_FIXTURE_STACK,
            &[0x1005_b43b, device_interface.guest_pointer.0],
        );
        acknowledge_d3d9_fixture_memory(&mut memory);
        let mut disabled_end_scene = D3d9CompatibilityPolicy::controlled_v31();
        disabled_end_scene.end_scene = None;
        disabled_end_scene.present = None;
        let before_disabled_end_memory = memory.stats();
        let before_disabled_end_runtime = runtime.clone();
        let disabled_end_fault = match dispatch_d3d9_fixture_frontend(
            &mut memory,
            &mut runtime,
            &disabled_end_scene,
            &device_interface,
            42,
            GuestAddress(0x1005_b43b),
        ) {
            ComDispatchOutcome::Fault(fault) => fault,
            other => panic!("disabled EndScene did not fault: {other:?}"),
        };
        assert_eq!(disabled_end_fault.class, ComFaultClass::Unsupported);
        assert_eq!(disabled_end_fault.code, "end-scene-policy-disabled");
        assert_eq!(memory.stats(), before_disabled_end_memory);
        assert_eq!(runtime, before_disabled_end_runtime);

        let before_end_back_buffer = runtime
            .graphics_device(graphics.id)
            .unwrap()
            .back_buffer
            .clone();
        let before_end_memory = memory.stats();
        let end_scene = match dispatch_d3d9_fixture_frontend(
            &mut memory,
            &mut runtime,
            &D3d9CompatibilityPolicy::controlled_v31(),
            &device_interface,
            42,
            GuestAddress(0x1005_b43b),
        ) {
            ComDispatchOutcome::Completed(completion) => completion,
            other => panic!("EndScene did not complete: {other:?}"),
        };
        assert_eq!(end_scene.return_value, ComReturnValue::HResult(COM_S_OK));
        assert_eq!(
            end_scene.stack_pointer,
            GuestAddress(D3D9_FIXTURE_STACK.0 + 8)
        );
        assert!(end_scene.guest_writes.is_empty());
        assert_eq!(memory.stats(), before_end_memory);
        let ended_device = runtime.graphics_device(graphics.id).unwrap();
        assert_eq!(ended_device.active_scene, None);
        assert_eq!(ended_device.back_buffer, before_end_back_buffer);
        assert_eq!(runtime.current_last_error().unwrap(), 0x1234_5678);
        assert_eq!(
            runtime
                .com()
                .object(device_object.id)
                .unwrap()
                .reference_count,
            device_reference_count
        );

        write_d3d9_fixture_words(
            &mut memory,
            D3D9_FIXTURE_STACK,
            &[0x1005_b43b, device_interface.guest_pointer.0],
        );
        acknowledge_d3d9_fixture_memory(&mut memory);
        let before_repeated_end_memory = memory.stats();
        let before_repeated_end_runtime = runtime.clone();
        let repeated_end = match dispatch_d3d9_fixture_frontend(
            &mut memory,
            &mut runtime,
            &D3d9CompatibilityPolicy::controlled_v31(),
            &device_interface,
            42,
            GuestAddress(0x1005_b43b),
        ) {
            ComDispatchOutcome::Completed(completion) => completion,
            other => panic!("repeated EndScene did not complete: {other:?}"),
        };
        assert_eq!(
            repeated_end.return_value,
            ComReturnValue::HResult(D3D9_D3DERR_INVALIDCALL)
        );
        assert!(repeated_end.guest_writes.is_empty());
        assert_eq!(memory.stats(), before_repeated_end_memory);
        assert_eq!(runtime, before_repeated_end_runtime);

        for arguments in [[1, 0, 0, 0], [0, 1, 0, 0], [0, 0, 1, 0], [0, 0, 0, 1]] {
            write_d3d9_fixture_words(
                &mut memory,
                D3D9_FIXTURE_STACK,
                &[
                    0x1005_b7b1,
                    device_interface.guest_pointer.0,
                    arguments[0],
                    arguments[1],
                    arguments[2],
                    arguments[3],
                ],
            );
            acknowledge_d3d9_fixture_memory(&mut memory);
            let before_memory = memory.stats();
            let before_runtime = runtime.clone();
            let rejected = match dispatch_d3d9_fixture_frontend(
                &mut memory,
                &mut runtime,
                &D3d9CompatibilityPolicy::controlled_v31(),
                &device_interface,
                17,
                GuestAddress(0x1005_b7b1),
            ) {
                ComDispatchOutcome::Completed(completion) => completion,
                other => panic!("rejected Present did not complete: {other:?}"),
            };
            assert_eq!(
                rejected.return_value,
                ComReturnValue::HResult(D3D9_D3DERR_INVALIDCALL)
            );
            assert!(rejected.guest_writes.is_empty());
            assert_eq!(memory.stats(), before_memory);
            assert_eq!(runtime, before_runtime);
        }

        write_d3d9_fixture_words(
            &mut memory,
            D3D9_FIXTURE_STACK,
            &[0x1005_b7b1, device_interface.guest_pointer.0, 0, 0, 0, 0],
        );
        acknowledge_d3d9_fixture_memory(&mut memory);
        let mut disabled_present = D3d9CompatibilityPolicy::controlled_v31();
        disabled_present.present = None;
        let before_disabled_present_memory = memory.stats();
        let before_disabled_present_runtime = runtime.clone();
        let disabled_present_fault = match dispatch_d3d9_fixture_frontend(
            &mut memory,
            &mut runtime,
            &disabled_present,
            &device_interface,
            17,
            GuestAddress(0x1005_b7b1),
        ) {
            ComDispatchOutcome::Fault(fault) => fault,
            other => panic!("disabled Present did not fault: {other:?}"),
        };
        assert_eq!(disabled_present_fault.class, ComFaultClass::Unsupported);
        assert_eq!(disabled_present_fault.code, "present-policy-disabled");
        assert_eq!(memory.stats(), before_disabled_present_memory);
        assert_eq!(runtime, before_disabled_present_runtime);

        let before_present_device = runtime.graphics_device(graphics.id).unwrap().clone();
        let before_present_memory = memory.stats();
        let present = match dispatch_d3d9_fixture_frontend(
            &mut memory,
            &mut runtime,
            &D3d9CompatibilityPolicy::controlled_v31(),
            &device_interface,
            17,
            GuestAddress(0x1005_b7b1),
        ) {
            ComDispatchOutcome::Completed(completion) => completion,
            other => panic!("Present did not complete: {other:?}"),
        };
        assert_eq!(present.return_value, ComReturnValue::HResult(COM_S_OK));
        assert_eq!(
            present.stack_pointer,
            GuestAddress(D3D9_FIXTURE_STACK.0 + 24)
        );
        assert!(present.guest_writes.is_empty());
        assert_eq!(memory.stats(), before_present_memory);
        let presented_device = runtime.graphics_device(graphics.id).unwrap();
        assert_eq!(presented_device.active_scene, None);
        assert_eq!(
            presented_device.generation,
            before_present_device.generation + 1
        );
        assert_eq!(
            presented_device.presented_frame.as_ref().unwrap().sequence,
            1
        );
        assert_eq!(
            presented_device
                .presented_frame
                .as_ref()
                .unwrap()
                .back_buffer,
            before_present_device.back_buffer
        );
        assert_eq!(
            presented_device.back_buffer.content_generation,
            before_present_device.back_buffer.content_generation + 1
        );
        assert!(presented_device.back_buffer.uniform_clear.is_none());
        assert!(presented_device.back_buffer.draw_submissions.is_empty());
        assert_eq!(runtime.current_last_error().unwrap(), 0x1234_5678);
        assert_eq!(
            runtime
                .com()
                .object(device_object.id)
                .unwrap()
                .reference_count,
            device_reference_count
        );

        acknowledge_d3d9_fixture_memory(&mut memory);
        let before_repeated_present_memory = memory.stats();
        let before_repeated_present_runtime = runtime.clone();
        let repeated_present = match dispatch_d3d9_fixture_frontend(
            &mut memory,
            &mut runtime,
            &D3d9CompatibilityPolicy::controlled_v31(),
            &device_interface,
            17,
            GuestAddress(0x1005_b7b1),
        ) {
            ComDispatchOutcome::Completed(completion) => completion,
            other => panic!("repeated Present did not complete: {other:?}"),
        };
        assert_eq!(
            repeated_present.return_value,
            ComReturnValue::HResult(D3D9_D3DERR_INVALIDCALL)
        );
        assert!(repeated_present.guest_writes.is_empty());
        assert_eq!(memory.stats(), before_repeated_present_memory);
        assert_eq!(runtime, before_repeated_present_runtime);

        write_d3d9_fixture_words(
            &mut memory,
            D3D9_FIXTURE_STACK,
            &[
                0x1005_b3ec,
                device_interface.guest_pointer.0,
                0,
                vertex_buffer_interface.guest_pointer.0,
                0,
                28,
            ],
        );
        acknowledge_d3d9_fixture_memory(&mut memory);
        let mut disabled_stream_source = D3d9CompatibilityPolicy::controlled_v31();
        disabled_stream_source.stream_source = None;
        disabled_stream_source.draw_primitive = None;
        disabled_stream_source.present = None;
        let before_disabled_stream_memory = memory.stats();
        let before_disabled_stream_runtime = runtime.clone();
        let disabled_stream_fault = match dispatch_d3d9_fixture_frontend(
            &mut memory,
            &mut runtime,
            &disabled_stream_source,
            &device_interface,
            100,
            GuestAddress(0x1005_b3ec),
        ) {
            ComDispatchOutcome::Fault(fault) => fault,
            other => panic!("disabled SetStreamSource did not fault: {other:?}"),
        };
        assert_eq!(disabled_stream_fault.class, ComFaultClass::Unsupported);
        assert_eq!(
            disabled_stream_fault.code,
            "set-stream-source-policy-disabled"
        );
        assert_eq!(memory.stats(), before_disabled_stream_memory);
        assert_eq!(runtime, before_disabled_stream_runtime);

        let vertex_buffer_address_allocation = vertex_buffer.backing.address_allocation_id;
        write_d3d9_fixture_words(
            &mut memory,
            D3D9_FIXTURE_STACK,
            &[0x1005_aaea, vertex_buffer_interface.guest_pointer.0],
        );
        acknowledge_d3d9_fixture_memory(&mut memory);
        let release_vertex_buffer = {
            let mut session = ExecutionSession::new(&mut memory, &mut runtime);
            session.dispatch_com_call(ComCallTrap {
                endpoint_address: vertex_buffer_interface.methods[&2].endpoint,
                stack_pointer: D3D9_FIXTURE_STACK,
                return_address: GuestAddress(0x1005_aaea),
                call_site: GuestAddress(0x1005_aaea),
            })
        };
        let ComDispatchOutcome::Completed(release_vertex_buffer) = release_vertex_buffer else {
            panic!("IDirect3DVertexBuffer9::Release did not complete")
        };
        assert_eq!(release_vertex_buffer.return_value, ComReturnValue::U32(0));
        assert!(release_vertex_buffer.object_retired);
        assert!(runtime.graphics_vertex_buffers().is_empty());
        assert_eq!(
            runtime
                .graphics_device(graphics.id)
                .unwrap()
                .stream_sources
                .stream_zero,
            None,
            "retiring a non-owned vertex buffer must clear the dangling stream binding"
        );
        assert!(
            runtime
                .process_address_space_allocator()
                .allocation(vertex_buffer_address_allocation)
                .is_err()
        );
        assert!(memory.read(GuestAddress(vertex_data_pointer), 1).is_err());
        assert_eq!(
            runtime
                .com()
                .object(device_object.id)
                .unwrap()
                .reference_count,
            7
        );
        acknowledge_d3d9_fixture_memory(&mut memory);

        write_d3d9_fixture_words(&mut memory, D3D9_FIXTURE_SURFACE_OUTPUT, &[0xcccc_cccc]);
        write_d3d9_fixture_words(
            &mut memory,
            D3D9_FIXTURE_STACK,
            &[
                0x1005_a7f0,
                additional_texture_interfaces[0].guest_pointer.0,
                0,
                D3D9_FIXTURE_SURFACE_OUTPUT.0,
            ],
        );
        acknowledge_d3d9_fixture_memory(&mut memory);
        let before_default_surface_memory = memory.stats();
        let before_default_surface_runtime = runtime.clone();
        let default_surface_fault = {
            let mut session = ExecutionSession::new(&mut memory, &mut runtime);
            match dispatch_d3d9_com_call(
                &D3d9CompatibilityPolicy::controlled_v31(),
                ComCallTrap {
                    endpoint_address: additional_texture_interfaces[0].methods[&18].endpoint,
                    stack_pointer: D3D9_FIXTURE_STACK,
                    return_address: GuestAddress(0x1005_a7f0),
                    call_site: GuestAddress(0x1005_a7f0),
                },
                &mut session,
            ) {
                ComDispatchOutcome::Fault(fault) => fault,
                other => panic!("default-pool GetSurfaceLevel did not fault: {other:?}"),
            }
        };
        assert_eq!(default_surface_fault.class, ComFaultClass::Unsupported);
        assert_eq!(
            default_surface_fault.code,
            "get-surface-level-texture-variant-unsupported"
        );
        assert_eq!(memory.stats(), before_default_surface_memory);
        assert_eq!(runtime, before_default_surface_runtime);
        assert_eq!(
            memory.read(D3D9_FIXTURE_SURFACE_OUTPUT, 4).unwrap(),
            0xcccc_cccc_u32.to_le_bytes()
        );

        for (level, output, expected) in [
            (1, D3D9_FIXTURE_SURFACE_OUTPUT.0, D3D9_D3DERR_INVALIDCALL),
            (0, 0, D3D9_D3DERR_INVALIDCALL),
        ] {
            write_d3d9_fixture_words(&mut memory, D3D9_FIXTURE_SURFACE_OUTPUT, &[0xcccc_cccc]);
            write_d3d9_fixture_words(
                &mut memory,
                D3D9_FIXTURE_STACK,
                &[
                    0x1005_a731,
                    texture_interface.guest_pointer.0,
                    level,
                    output,
                ],
            );
            acknowledge_d3d9_fixture_memory(&mut memory);
            let before_memory = memory.stats();
            let before_runtime = runtime.clone();
            let completion = {
                let mut session = ExecutionSession::new(&mut memory, &mut runtime);
                match dispatch_d3d9_com_call(
                    &D3d9CompatibilityPolicy::controlled_v31(),
                    ComCallTrap {
                        endpoint_address: texture_interface.methods[&18].endpoint,
                        stack_pointer: D3D9_FIXTURE_STACK,
                        return_address: GuestAddress(0x1005_a731),
                        call_site: GuestAddress(0x1005_a731),
                    },
                    &mut session,
                ) {
                    ComDispatchOutcome::Completed(completion) => completion,
                    other => panic!("invalid GetSurfaceLevel did not complete: {other:?}"),
                }
            };
            assert_eq!(completion.return_value, ComReturnValue::HResult(expected));
            assert!(completion.guest_writes.is_empty());
            assert_eq!(memory.stats(), before_memory);
            assert_eq!(runtime, before_runtime);
            assert_eq!(
                memory.read(D3D9_FIXTURE_SURFACE_OUTPUT, 4).unwrap(),
                0xcccc_cccc_u32.to_le_bytes()
            );
        }

        write_d3d9_fixture_words(
            &mut memory,
            D3D9_FIXTURE_STACK,
            &[
                0x1005_a731,
                texture_interface.guest_pointer.0,
                0,
                D3D9_FIXTURE_SURFACE_OUTPUT.0,
            ],
        );
        acknowledge_d3d9_fixture_memory(&mut memory);
        let mut disabled_surface = D3d9CompatibilityPolicy::controlled_v31();
        disabled_surface.get_surface_level = None;
        disabled_surface.surface_dc = None;
        let before_disabled_memory = memory.stats();
        let before_disabled_runtime = runtime.clone();
        let disabled_fault = {
            let mut session = ExecutionSession::new(&mut memory, &mut runtime);
            match dispatch_d3d9_com_call(
                &disabled_surface,
                ComCallTrap {
                    endpoint_address: texture_interface.methods[&18].endpoint,
                    stack_pointer: D3D9_FIXTURE_STACK,
                    return_address: GuestAddress(0x1005_a731),
                    call_site: GuestAddress(0x1005_a731),
                },
                &mut session,
            ) {
                ComDispatchOutcome::Fault(fault) => fault,
                other => panic!("disabled GetSurfaceLevel did not fault: {other:?}"),
            }
        };
        assert_eq!(disabled_fault.class, ComFaultClass::Unsupported);
        assert_eq!(disabled_fault.code, "get-surface-level-policy-disabled");
        assert_eq!(memory.stats(), before_disabled_memory);
        assert_eq!(runtime, before_disabled_runtime);

        write_d3d9_fixture_words(&mut memory, D3D9_FIXTURE_SURFACE_OUTPUT, &[0xcccc_cccc]);
        write_d3d9_fixture_words(
            &mut memory,
            D3D9_FIXTURE_STACK,
            &[
                0x1005_a731,
                texture_interface.guest_pointer.0,
                0,
                D3D9_FIXTURE_SURFACE_OUTPUT.0,
            ],
        );
        acknowledge_d3d9_fixture_memory(&mut memory);
        let surface_completion = {
            let mut session = ExecutionSession::new(&mut memory, &mut runtime);
            match dispatch_d3d9_com_call(
                &D3d9CompatibilityPolicy::controlled_v31(),
                ComCallTrap {
                    endpoint_address: texture_interface.methods[&18].endpoint,
                    stack_pointer: D3D9_FIXTURE_STACK,
                    return_address: GuestAddress(0x1005_a731),
                    call_site: GuestAddress(0x1005_a731),
                },
                &mut session,
            ) {
                ComDispatchOutcome::Completed(completion) => completion,
                other => panic!("GetSurfaceLevel did not complete: {other:?}"),
            }
        };
        assert_eq!(
            surface_completion.return_value,
            ComReturnValue::HResult(COM_S_OK)
        );
        assert_eq!(
            surface_completion.stack_pointer,
            GuestAddress(D3D9_FIXTURE_STACK.0 + 16)
        );
        assert_eq!(surface_completion.guest_writes.len(), 1);
        assert_eq!(
            surface_completion.guest_writes[0].address,
            D3D9_FIXTURE_SURFACE_OUTPUT
        );
        acknowledge_d3d9_fixture_memory(&mut memory);

        let surface_pointer = u32::from_le_bytes(
            memory
                .read(D3D9_FIXTURE_SURFACE_OUTPUT, 4)
                .expect("GetSurfaceLevel output pointer")
                .try_into()
                .unwrap(),
        );
        let surface_interface = runtime
            .com_interface_for_pointer(GuestAddress(surface_pointer))
            .expect("IDirect3DSurface9 output")
            .clone();
        assert_eq!(surface_interface.iid, D3D9_IDIRECT3DSURFACE9_IID);
        assert_eq!(surface_interface.methods.len(), 17);
        assert_eq!(surface_interface.methods[&15].name, "GetDC");
        let surface_object = runtime
            .com()
            .object(surface_interface.object_id)
            .expect("surface COM object")
            .clone();
        assert_eq!(surface_object.reference_count, 1);
        assert_eq!(
            surface_object.parent_interface_id,
            Some(texture_interface.id)
        );
        assert_eq!(
            runtime
                .com()
                .object(texture_object.id)
                .unwrap()
                .reference_count,
            2
        );
        let surface = runtime
            .graphics_surfaces()
            .values()
            .next()
            .expect("logical graphics surface")
            .clone();
        assert_eq!(runtime.graphics_surfaces().len(), 1);
        assert_eq!(surface.namespace, RUNTIME_GRAPHICS_SURFACE_ID_NAMESPACE);
        assert_eq!(surface.texture_id, texture.id);
        assert_eq!(surface.com_object_id, surface_object.id);
        assert_eq!(surface.descriptor.level, 0);
        assert_eq!(surface.descriptor.width, 1_024);
        assert_eq!(surface.descriptor.height, 1_024);
        assert_eq!(
            surface.descriptor.color_format,
            RuntimeGraphicsColorFormat::B8g8r8x8Unorm
        );

        write_d3d9_fixture_words(&mut memory, D3D9_FIXTURE_DC_OUTPUT, &[0xcccc_cccc]);
        write_d3d9_fixture_words(
            &mut memory,
            D3D9_FIXTURE_STACK,
            &[0x1005_a746, surface_pointer, 0],
        );
        acknowledge_d3d9_fixture_memory(&mut memory);
        let before_null_memory = memory.stats();
        let before_null_runtime = runtime.clone();
        let null_output = match dispatch_d3d9_fixture_frontend(
            &mut memory,
            &mut runtime,
            &D3d9CompatibilityPolicy::controlled_v31(),
            &surface_interface,
            15,
            GuestAddress(0x1005_a746),
        ) {
            ComDispatchOutcome::Completed(completion) => completion,
            other => panic!("NULL surface GetDC did not complete: {other:?}"),
        };
        assert_eq!(
            null_output.return_value,
            ComReturnValue::HResult(D3D9_D3DERR_INVALIDCALL)
        );
        assert!(null_output.guest_writes.is_empty());
        assert_eq!(memory.stats(), before_null_memory);
        assert_eq!(runtime, before_null_runtime);

        let mut disabled_surface_dc = D3d9CompatibilityPolicy::controlled_v31();
        disabled_surface_dc.surface_dc = None;
        write_d3d9_fixture_words(
            &mut memory,
            D3D9_FIXTURE_STACK,
            &[0x1005_a746, surface_pointer, D3D9_FIXTURE_DC_OUTPUT.0],
        );
        acknowledge_d3d9_fixture_memory(&mut memory);
        let before_disabled_memory = memory.stats();
        let before_disabled_runtime = runtime.clone();
        let disabled_get_dc = match dispatch_d3d9_fixture_frontend(
            &mut memory,
            &mut runtime,
            &disabled_surface_dc,
            &surface_interface,
            15,
            GuestAddress(0x1005_a746),
        ) {
            ComDispatchOutcome::Fault(fault) => fault,
            other => panic!("disabled surface GetDC did not fault: {other:?}"),
        };
        assert_eq!(disabled_get_dc.class, ComFaultClass::Unsupported);
        assert_eq!(disabled_get_dc.code, "surface-dc-policy-disabled");
        assert_eq!(memory.stats(), before_disabled_memory);
        assert_eq!(runtime, before_disabled_runtime);

        write_d3d9_fixture_words(
            &mut memory,
            D3D9_FIXTURE_STACK,
            &[0x1005_a746, surface_pointer, surface_pointer],
        );
        acknowledge_d3d9_fixture_memory(&mut memory);
        let before_unwritable_memory = memory.stats();
        let before_unwritable_runtime = runtime.clone();
        let unwritable_get_dc = match dispatch_d3d9_fixture_frontend(
            &mut memory,
            &mut runtime,
            &D3d9CompatibilityPolicy::controlled_v31(),
            &surface_interface,
            15,
            GuestAddress(0x1005_a746),
        ) {
            ComDispatchOutcome::Fault(fault) => fault,
            other => panic!("unwritable surface GetDC did not fault: {other:?}"),
        };
        assert_eq!(unwritable_get_dc.class, ComFaultClass::InvalidCall);
        assert_eq!(memory.stats(), before_unwritable_memory);
        assert_eq!(runtime, before_unwritable_runtime);

        write_d3d9_fixture_words(&mut memory, D3D9_FIXTURE_DC_OUTPUT, &[0xcccc_cccc]);
        write_d3d9_fixture_words(
            &mut memory,
            D3D9_FIXTURE_STACK,
            &[0x1005_a746, surface_pointer, D3D9_FIXTURE_DC_OUTPUT.0],
        );
        acknowledge_d3d9_fixture_memory(&mut memory);
        let surface_reference_count = runtime
            .com()
            .object(surface_object.id)
            .unwrap()
            .reference_count;
        let get_dc = match dispatch_d3d9_fixture_frontend(
            &mut memory,
            &mut runtime,
            &D3d9CompatibilityPolicy::controlled_v31(),
            &surface_interface,
            15,
            GuestAddress(0x1005_a746),
        ) {
            ComDispatchOutcome::Completed(completion) => completion,
            other => panic!("surface GetDC did not complete: {other:?}"),
        };
        assert_eq!(get_dc.return_value, ComReturnValue::HResult(COM_S_OK));
        assert_eq!(
            get_dc.stack_pointer,
            GuestAddress(D3D9_FIXTURE_STACK.0 + 12)
        );
        assert_eq!(get_dc.guest_writes.len(), 1);
        assert_eq!(get_dc.guest_writes[0].address, D3D9_FIXTURE_DC_OUTPUT);
        acknowledge_d3d9_fixture_memory(&mut memory);
        let dc_handle = u32::from_le_bytes(
            memory
                .read(D3D9_FIXTURE_DC_OUTPUT, 4)
                .unwrap()
                .try_into()
                .unwrap(),
        );
        assert_ne!(dc_handle, 0);
        let device_context = runtime.gdi_device_context(dc_handle).unwrap().clone();
        assert_eq!(device_context.owner_thread_id, RuntimeThreadId(0x2000));
        assert!(matches!(
            device_context.target,
            RuntimeDeviceContextTarget::GraphicsSurface {
                surface_id,
                surface_com_object_id,
                width: 1_024,
                height: 1_024,
                color_format: RuntimeGraphicsColorFormat::B8g8r8x8Unorm,
            } if surface_id == surface.id && surface_com_object_id == surface_object.id
        ));
        assert_eq!(runtime.gdi_device_contexts().len(), 1);
        assert_eq!(
            runtime
                .com()
                .object(surface_object.id)
                .unwrap()
                .reference_count,
            surface_reference_count
        );

        write_d3d9_fixture_words(&mut memory, D3D9_FIXTURE_DC_OUTPUT, &[0xcccc_cccc]);
        write_d3d9_fixture_words(
            &mut memory,
            D3D9_FIXTURE_STACK,
            &[0x1005_a746, surface_pointer, D3D9_FIXTURE_DC_OUTPUT.0],
        );
        acknowledge_d3d9_fixture_memory(&mut memory);
        let before_repeat_memory = memory.stats();
        let before_repeat_runtime = runtime.clone();
        let repeated_get_dc = match dispatch_d3d9_fixture_frontend(
            &mut memory,
            &mut runtime,
            &D3d9CompatibilityPolicy::controlled_v31(),
            &surface_interface,
            15,
            GuestAddress(0x1005_a746),
        ) {
            ComDispatchOutcome::Completed(completion) => completion,
            other => panic!("repeated surface GetDC did not complete: {other:?}"),
        };
        assert_eq!(
            repeated_get_dc.return_value,
            ComReturnValue::HResult(D3D9_D3DERR_INVALIDCALL)
        );
        assert!(repeated_get_dc.guest_writes.is_empty());
        assert_eq!(memory.stats(), before_repeat_memory);
        assert_eq!(runtime, before_repeat_runtime);
        assert_eq!(
            memory.read(D3D9_FIXTURE_DC_OUTPUT, 4).unwrap(),
            0xcccc_cccc_u32.to_le_bytes()
        );

        let brush = runtime
            .get_gdi_stock_object(RUNTIME_GDI_BLACK_BRUSH_INDEX)
            .unwrap();
        let fill_rect = RuntimeRect {
            left: 0,
            top: 0,
            right: 800,
            bottom: 600,
        };
        runtime
            .fill_gdi_rect(dc_handle, fill_rect, brush.handle)
            .unwrap();
        let filled_texture = runtime.graphics_texture(surface.texture_id).unwrap();
        assert_eq!(filled_texture.content.fills.len(), 1);
        assert_eq!(filled_texture.content.fills[0].rect, fill_rect);
        assert_eq!(filled_texture.content.fills[0].color_ref, 0);
        assert_eq!(filled_texture.content.content_generation, 1);
        let before_invalid_fill = runtime.clone();
        assert_eq!(
            runtime
                .fill_gdi_rect(
                    dc_handle,
                    RuntimeRect {
                        left: 0,
                        top: 0,
                        right: 1_025,
                        bottom: 1,
                    },
                    brush.handle,
                )
                .unwrap_err()
                .code,
            "fill-rect-outside-supported-surface"
        );
        assert_eq!(runtime, before_invalid_fill);

        write_d3d9_fixture_words(
            &mut memory,
            D3D9_FIXTURE_STACK,
            &[0x1005_a79a, surface_pointer],
        );
        acknowledge_d3d9_fixture_memory(&mut memory);
        let before_guard_memory = memory.stats();
        let before_guard_runtime = runtime.clone();
        let guarded_release = {
            let mut session = ExecutionSession::new(&mut memory, &mut runtime);
            match session.dispatch_com_call(ComCallTrap {
                endpoint_address: surface_interface.methods[&2].endpoint,
                stack_pointer: D3D9_FIXTURE_STACK,
                return_address: GuestAddress(0x1005_a79a),
                call_site: GuestAddress(0x1005_a79a),
            }) {
                ComDispatchOutcome::Fault(fault) => fault,
                other => panic!("surface Release with a live HDC did not fault: {other:?}"),
            }
        };
        assert_eq!(guarded_release.class, ComFaultClass::InvalidCall);
        assert_eq!(guarded_release.code, "graphics-surface-dc-still-acquired");
        assert_eq!(memory.stats(), before_guard_memory);
        assert_eq!(runtime, before_guard_runtime);

        write_d3d9_fixture_words(
            &mut memory,
            D3D9_FIXTURE_STACK,
            &[0x1005_a78e, surface_pointer, dc_handle + 1],
        );
        acknowledge_d3d9_fixture_memory(&mut memory);
        let before_wrong_release = runtime.clone();
        let wrong_release = match dispatch_d3d9_fixture_frontend(
            &mut memory,
            &mut runtime,
            &D3d9CompatibilityPolicy::controlled_v31(),
            &surface_interface,
            16,
            GuestAddress(0x1005_a78e),
        ) {
            ComDispatchOutcome::Completed(completion) => completion,
            other => panic!("wrong surface ReleaseDC did not complete: {other:?}"),
        };
        assert_eq!(
            wrong_release.return_value,
            ComReturnValue::HResult(D3D9_D3DERR_INVALIDCALL)
        );
        assert!(wrong_release.guest_writes.is_empty());
        assert_eq!(runtime, before_wrong_release);

        write_d3d9_fixture_words(
            &mut memory,
            D3D9_FIXTURE_STACK,
            &[0x1005_a78e, surface_pointer, dc_handle],
        );
        acknowledge_d3d9_fixture_memory(&mut memory);
        let release_dc = match dispatch_d3d9_fixture_frontend(
            &mut memory,
            &mut runtime,
            &D3d9CompatibilityPolicy::controlled_v31(),
            &surface_interface,
            16,
            GuestAddress(0x1005_a78e),
        ) {
            ComDispatchOutcome::Completed(completion) => completion,
            other => panic!("surface ReleaseDC did not complete: {other:?}"),
        };
        assert_eq!(release_dc.return_value, ComReturnValue::HResult(COM_S_OK));
        assert!(release_dc.guest_writes.is_empty());
        assert!(runtime.gdi_device_contexts().is_empty());
        assert_eq!(
            runtime
                .graphics_texture(surface.texture_id)
                .unwrap()
                .content
                .fills
                .len(),
            1
        );

        let before_repeated_release = runtime.clone();
        let repeated_release_dc = match dispatch_d3d9_fixture_frontend(
            &mut memory,
            &mut runtime,
            &D3d9CompatibilityPolicy::controlled_v31(),
            &surface_interface,
            16,
            GuestAddress(0x1005_a78e),
        ) {
            ComDispatchOutcome::Completed(completion) => completion,
            other => panic!("repeated surface ReleaseDC did not complete: {other:?}"),
        };
        assert_eq!(
            repeated_release_dc.return_value,
            ComReturnValue::HResult(D3D9_D3DERR_INVALIDCALL)
        );
        assert_eq!(runtime, before_repeated_release);

        write_d3d9_fixture_words(&mut memory, D3D9_FIXTURE_DC_OUTPUT, &[0xcccc_cccc]);
        write_d3d9_fixture_words(
            &mut memory,
            D3D9_FIXTURE_STACK,
            &[0x1005_a746, surface_pointer, D3D9_FIXTURE_DC_OUTPUT.0],
        );
        acknowledge_d3d9_fixture_memory(&mut memory);
        let reacquired = match dispatch_d3d9_fixture_frontend(
            &mut memory,
            &mut runtime,
            &D3d9CompatibilityPolicy::controlled_v31(),
            &surface_interface,
            15,
            GuestAddress(0x1005_a746),
        ) {
            ComDispatchOutcome::Completed(completion) => completion,
            other => panic!("reacquired surface GetDC did not complete: {other:?}"),
        };
        assert_eq!(reacquired.return_value, ComReturnValue::HResult(COM_S_OK));
        acknowledge_d3d9_fixture_memory(&mut memory);
        let reacquired_handle = u32::from_le_bytes(
            memory
                .read(D3D9_FIXTURE_DC_OUTPUT, 4)
                .unwrap()
                .try_into()
                .unwrap(),
        );
        assert_ne!(reacquired_handle, dc_handle);
        write_d3d9_fixture_words(
            &mut memory,
            D3D9_FIXTURE_STACK,
            &[0x1005_a78e, surface_pointer, reacquired_handle],
        );
        acknowledge_d3d9_fixture_memory(&mut memory);
        let reacquired_release = match dispatch_d3d9_fixture_frontend(
            &mut memory,
            &mut runtime,
            &D3d9CompatibilityPolicy::controlled_v31(),
            &surface_interface,
            16,
            GuestAddress(0x1005_a78e),
        ) {
            ComDispatchOutcome::Completed(completion) => completion,
            other => panic!("reacquired surface ReleaseDC did not complete: {other:?}"),
        };
        assert_eq!(
            reacquired_release.return_value,
            ComReturnValue::HResult(COM_S_OK)
        );
        assert!(runtime.gdi_device_contexts().is_empty());
        assert_eq!(runtime.current_last_error().unwrap(), 0x1234_5678);

        write_d3d9_fixture_words(&mut memory, D3D9_FIXTURE_SURFACE_OUTPUT, &[0xcccc_cccc]);
        write_d3d9_fixture_words(
            &mut memory,
            D3D9_FIXTURE_STACK,
            &[
                0x1005_a731,
                texture_interface.guest_pointer.0,
                0,
                D3D9_FIXTURE_SURFACE_OUTPUT.0,
            ],
        );
        acknowledge_d3d9_fixture_memory(&mut memory);
        let repeated_surface = {
            let mut session = ExecutionSession::new(&mut memory, &mut runtime);
            match dispatch_d3d9_com_call(
                &D3d9CompatibilityPolicy::controlled_v31(),
                ComCallTrap {
                    endpoint_address: texture_interface.methods[&18].endpoint,
                    stack_pointer: D3D9_FIXTURE_STACK,
                    return_address: GuestAddress(0x1005_a731),
                    call_site: GuestAddress(0x1005_a731),
                },
                &mut session,
            ) {
                ComDispatchOutcome::Completed(completion) => completion,
                other => panic!("repeated GetSurfaceLevel did not complete: {other:?}"),
            }
        };
        assert_eq!(
            repeated_surface.return_value,
            ComReturnValue::HResult(COM_S_OK)
        );
        acknowledge_d3d9_fixture_memory(&mut memory);
        assert_eq!(
            memory.read(D3D9_FIXTURE_SURFACE_OUTPUT, 4).unwrap(),
            surface_pointer.to_le_bytes()
        );
        assert_eq!(runtime.graphics_surfaces().len(), 1);
        assert_eq!(
            runtime
                .com()
                .object(surface_object.id)
                .unwrap()
                .reference_count,
            2
        );
        assert_eq!(
            runtime
                .com()
                .object(texture_object.id)
                .unwrap()
                .reference_count,
            2
        );
        let repeated_release = dispatch_d3d9_fixture_release(
            &mut memory,
            &mut runtime,
            &surface_interface,
            surface_interface.guest_pointer,
        );
        assert_eq!(repeated_release.return_value, ComReturnValue::U32(1));
        assert!(!repeated_release.object_retired);

        for interface in additional_texture_interfaces.iter().rev() {
            let release = dispatch_d3d9_fixture_release(
                &mut memory,
                &mut runtime,
                interface,
                interface.guest_pointer,
            );
            assert_eq!(release.return_value, ComReturnValue::U32(0));
            assert!(release.object_retired);
            assert!(memory.read(interface.guest_pointer, 4).is_err());
        }
        assert_eq!(runtime.graphics_textures().len(), 1);
        assert_eq!(
            runtime
                .com()
                .object(device_object.id)
                .unwrap()
                .reference_count,
            2
        );

        let root_release = dispatch_d3d9_fixture_release(
            &mut memory,
            &mut runtime,
            &root_interface,
            root_interface.guest_pointer,
        );
        assert_eq!(root_release.return_value, ComReturnValue::U32(1));
        assert!(!root_release.object_retired);
        assert_eq!(
            runtime
                .com()
                .object(root.object.id)
                .unwrap()
                .reference_count,
            1
        );
        assert_eq!(runtime.graphics_devices().len(), 1);

        let device_release = dispatch_d3d9_fixture_release(
            &mut memory,
            &mut runtime,
            &device_interface,
            device_interface.guest_pointer,
        );
        assert_eq!(device_release.return_value, ComReturnValue::U32(1));
        assert!(!device_release.object_retired);
        assert_eq!(runtime.graphics_devices().len(), 1);
        assert_eq!(runtime.graphics_textures().len(), 1);

        let texture_release = dispatch_d3d9_fixture_release(
            &mut memory,
            &mut runtime,
            &texture_interface,
            texture_interface.guest_pointer,
        );
        assert_eq!(texture_release.return_value, ComReturnValue::U32(1));
        assert!(!texture_release.object_retired);
        assert_eq!(runtime.graphics_surfaces().len(), 1);

        let surface_release = dispatch_d3d9_fixture_release(
            &mut memory,
            &mut runtime,
            &surface_interface,
            surface_interface.guest_pointer,
        );
        assert_eq!(surface_release.return_value, ComReturnValue::U32(0));
        assert!(surface_release.object_retired);
        assert!(runtime.com().is_empty());
        assert!(runtime.graphics_devices().is_empty());
        assert!(runtime.graphics_textures().is_empty());
        assert!(runtime.graphics_surfaces().is_empty());
        assert!(memory.read(root_interface.guest_pointer, 4).is_err());
        assert!(memory.read(device_interface.guest_pointer, 4).is_err());
        assert!(memory.read(texture_interface.guest_pointer, 4).is_err());
        assert!(memory.read(surface_interface.guest_pointer, 4).is_err());
        assert_eq!(runtime.current_last_error().unwrap(), 0x1234_5678);
    }

    #[test]
    fn direct_sound_secondary_lock_unlock_play_commits_memory_audio_and_com_lifetime() {
        const SECONDARY_BYTES: u32 = 16_760;

        let mut runtime = d3d9_fixture_runtime();
        let window = seed_created_d3d9_window(&mut runtime);
        let mut memory = d3d9_fixture_memory();
        let (installed, device, guest_writes) = install_audio_device_com_object_with_guest_writes(
            &mut memory,
            &mut runtime,
            direct_sound8_object_spec(),
            RuntimeAudioDeviceCreateRequest {
                frontend: super::super::RuntimeAudioFrontend::DirectSound8,
                selector: super::super::RuntimeAudioDeviceSelector::DefaultPlayback,
            },
            |_| Ok(Vec::new()),
        )
        .expect("install DirectSound root and logical audio device");
        assert!(guest_writes.is_empty());
        assert_eq!(device.com_object_id, installed.object.id);
        let interface = installed
            .interfaces
            .iter()
            .find(|interface| interface.iid == DSOUND_IDIRECTSOUND8_IID)
            .expect("IDirectSound8 interface")
            .clone();
        acknowledge_d3d9_fixture_memory(&mut memory);
        write_d3d9_fixture_words(
            &mut memory,
            D3D9_FIXTURE_STACK,
            &[
                D3D9_FIXTURE_CALL_SITE.0,
                interface.guest_pointer.0,
                window.handle,
                DSOUND_DSSCL_PRIORITY,
            ],
        );
        acknowledge_d3d9_fixture_memory(&mut memory);

        let completion = {
            let mut session = ExecutionSession::new(&mut memory, &mut runtime);
            match dispatch_dsound_com_call(
                &DsoundCompatibilityPolicy::forward_v2(),
                ComCallTrap {
                    endpoint_address: interface.methods[&6].endpoint,
                    stack_pointer: D3D9_FIXTURE_STACK,
                    return_address: D3D9_FIXTURE_CALL_SITE,
                    call_site: D3D9_FIXTURE_CALL_SITE,
                },
                &mut session,
            ) {
                ComDispatchOutcome::Completed(completion) => completion,
                other => panic!("SetCooperativeLevel did not complete: {other:?}"),
            }
        };
        assert_eq!(completion.return_value, ComReturnValue::HResult(COM_S_OK));
        assert_eq!(
            completion.stack_pointer,
            GuestAddress(D3D9_FIXTURE_STACK.0 + 16)
        );
        assert!(completion.guest_writes.is_empty());
        let device = runtime
            .audio_device(device.id)
            .expect("logical audio device");
        let cooperative = device
            .cooperative
            .expect("committed DirectSound cooperative state");
        assert_eq!(cooperative.window_id, window.id);
        assert_eq!(cooperative.window_handle, window.handle);
        assert_eq!(cooperative.owner_thread_id, RuntimeThreadId(0x2000));
        assert_eq!(
            cooperative.level,
            super::super::RuntimeAudioCooperativeLevel::Priority
        );

        let policy = DsoundCompatibilityPolicy::forward_v2();
        let pcm_words = [0x0002_0001, 44_100, 176_400, 0x0010_0004];
        let format_address = GuestAddress(D3D9_FIXTURE_PRESENTATION.0 + 0x40);
        let iid_address = GuestAddress(D3D9_FIXTURE_PRESENTATION.0 + 0x60);
        write_d3d9_fixture_words(&mut memory, format_address, &pcm_words);
        write_d3d9_fixture_words(
            &mut memory,
            D3D9_FIXTURE_PRESENTATION,
            &[
                DSOUND_DSBUFFERDESC_BYTES,
                DSOUND_PRIMARY_BUFFER_FLAGS,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
            ],
        );
        write_d3d9_fixture_words(&mut memory, D3D9_FIXTURE_OUTPUT, &[0xcccc_cccc]);
        write_d3d9_fixture_words(
            &mut memory,
            D3D9_FIXTURE_STACK,
            &[
                D3D9_FIXTURE_CALL_SITE.0,
                interface.guest_pointer.0,
                D3D9_FIXTURE_PRESENTATION.0,
                D3D9_FIXTURE_OUTPUT.0,
                0,
            ],
        );
        acknowledge_d3d9_fixture_memory(&mut memory);
        let create_primary = {
            let mut session = ExecutionSession::new(&mut memory, &mut runtime);
            match dispatch_dsound_com_call(
                &policy,
                ComCallTrap {
                    endpoint_address: interface.methods[&3].endpoint,
                    stack_pointer: D3D9_FIXTURE_STACK,
                    return_address: D3D9_FIXTURE_CALL_SITE,
                    call_site: D3D9_FIXTURE_CALL_SITE,
                },
                &mut session,
            ) {
                ComDispatchOutcome::Completed(completion) => completion,
                other => panic!("primary CreateSoundBuffer did not complete: {other:?}"),
            }
        };
        assert_eq!(
            create_primary.return_value,
            ComReturnValue::HResult(COM_S_OK)
        );
        assert_eq!(create_primary.guest_writes.len(), 1);
        acknowledge_d3d9_fixture_memory(&mut memory);
        let primary_pointer = u32::from_le_bytes(
            memory
                .read(D3D9_FIXTURE_OUTPUT, 4)
                .unwrap()
                .try_into()
                .unwrap(),
        );
        let primary_interface = runtime
            .com_interface_for_pointer(GuestAddress(primary_pointer))
            .expect("primary IDirectSoundBuffer8 output")
            .clone();
        assert_eq!(primary_interface.iid, DSOUND_IDIRECTSOUNDBUFFER8_IID);
        let primary = runtime
            .audio_buffer_for_com_object(primary_interface.object_id)
            .expect("primary logical audio buffer");
        assert_eq!(primary.descriptor.kind, RuntimeAudioBufferKind::Primary);
        assert_eq!(primary.active_format, None);

        write_d3d9_fixture_words(
            &mut memory,
            D3D9_FIXTURE_STACK,
            &[
                D3D9_FIXTURE_CALL_SITE.0,
                primary_interface.guest_pointer.0,
                format_address.0,
            ],
        );
        acknowledge_d3d9_fixture_memory(&mut memory);
        let set_format = {
            let mut session = ExecutionSession::new(&mut memory, &mut runtime);
            match dispatch_dsound_com_call(
                &policy,
                ComCallTrap {
                    endpoint_address: primary_interface.methods[&14].endpoint,
                    stack_pointer: D3D9_FIXTURE_STACK,
                    return_address: D3D9_FIXTURE_CALL_SITE,
                    call_site: D3D9_FIXTURE_CALL_SITE,
                },
                &mut session,
            ) {
                ComDispatchOutcome::Completed(completion) => completion,
                other => panic!("primary SetFormat did not complete: {other:?}"),
            }
        };
        assert_eq!(set_format.return_value, ComReturnValue::HResult(COM_S_OK));

        write_d3d9_fixture_words(
            &mut memory,
            D3D9_FIXTURE_STACK,
            &[
                D3D9_FIXTURE_CALL_SITE.0,
                primary_interface.guest_pointer.0,
                0,
                0,
                DSOUND_DSBPLAY_LOOPING,
            ],
        );
        acknowledge_d3d9_fixture_memory(&mut memory);
        let play = {
            let mut session = ExecutionSession::new(&mut memory, &mut runtime);
            match dispatch_dsound_com_call(
                &policy,
                ComCallTrap {
                    endpoint_address: primary_interface.methods[&12].endpoint,
                    stack_pointer: D3D9_FIXTURE_STACK,
                    return_address: D3D9_FIXTURE_CALL_SITE,
                    call_site: D3D9_FIXTURE_CALL_SITE,
                },
                &mut session,
            ) {
                ComDispatchOutcome::Completed(completion) => completion,
                other => panic!("primary Play did not complete: {other:?}"),
            }
        };
        assert_eq!(play.return_value, ComReturnValue::HResult(COM_S_OK));
        let primary = runtime
            .audio_buffer_for_com_object(primary_interface.object_id)
            .expect("playing primary logical audio buffer");
        assert_eq!(
            primary.active_format,
            policy.initial_buffers.as_ref().map(|p| p.pcm_format)
        );
        assert_eq!(primary.playback, RuntimeAudioPlaybackState::PlayingLooping);

        write_d3d9_fixture_words(
            &mut memory,
            D3D9_FIXTURE_PRESENTATION,
            &[
                DSOUND_DSBUFFERDESC_BYTES,
                DSOUND_SECONDARY_BUFFER_FORWARD_FLAGS,
                SECONDARY_BYTES,
                0,
                format_address.0,
                0,
                0,
                0,
                0,
            ],
        );
        write_d3d9_fixture_words(&mut memory, D3D9_FIXTURE_TEXTURE_OUTPUT, &[0xcccc_cccc]);
        write_d3d9_fixture_words(
            &mut memory,
            D3D9_FIXTURE_STACK,
            &[
                D3D9_FIXTURE_CALL_SITE.0,
                interface.guest_pointer.0,
                D3D9_FIXTURE_PRESENTATION.0,
                D3D9_FIXTURE_TEXTURE_OUTPUT.0,
                0,
            ],
        );
        acknowledge_d3d9_fixture_memory(&mut memory);
        let create_secondary = {
            let mut session = ExecutionSession::new(&mut memory, &mut runtime);
            match dispatch_dsound_com_call(
                &policy,
                ComCallTrap {
                    endpoint_address: interface.methods[&3].endpoint,
                    stack_pointer: D3D9_FIXTURE_STACK,
                    return_address: D3D9_FIXTURE_CALL_SITE,
                    call_site: D3D9_FIXTURE_CALL_SITE,
                },
                &mut session,
            ) {
                ComDispatchOutcome::Completed(completion) => completion,
                other => panic!("secondary CreateSoundBuffer did not complete: {other:?}"),
            }
        };
        assert_eq!(
            create_secondary.return_value,
            ComReturnValue::HResult(COM_S_OK)
        );
        acknowledge_d3d9_fixture_memory(&mut memory);
        let secondary_pointer = u32::from_le_bytes(
            memory
                .read(D3D9_FIXTURE_TEXTURE_OUTPUT, 4)
                .unwrap()
                .try_into()
                .unwrap(),
        );
        let secondary_interface = runtime
            .com_interface_for_pointer(GuestAddress(secondary_pointer))
            .expect("secondary IDirectSoundBuffer8 output")
            .clone();
        assert_eq!(runtime.audio_buffers().len(), 2);
        assert_eq!(
            runtime
                .audio_buffer_for_com_object(secondary_interface.object_id)
                .unwrap()
                .descriptor
                .kind,
            RuntimeAudioBufferKind::Secondary
        );

        let iid_words = DSOUND_IDIRECTSOUNDBUFFER8_IID
            .windows_bytes()
            .chunks_exact(4)
            .map(|bytes| u32::from_le_bytes(bytes.try_into().unwrap()))
            .collect::<Vec<_>>();
        write_d3d9_fixture_words(&mut memory, iid_address, &iid_words);
        write_d3d9_fixture_words(&mut memory, D3D9_FIXTURE_SURFACE_OUTPUT, &[0xcccc_cccc]);
        write_d3d9_fixture_words(
            &mut memory,
            D3D9_FIXTURE_STACK,
            &[
                D3D9_FIXTURE_CALL_SITE.0,
                secondary_interface.guest_pointer.0,
                iid_address.0,
                D3D9_FIXTURE_SURFACE_OUTPUT.0,
            ],
        );
        acknowledge_d3d9_fixture_memory(&mut memory);
        let query = {
            let mut session = ExecutionSession::new(&mut memory, &mut runtime);
            dispatch_dsound_com_call(
                &policy,
                ComCallTrap {
                    endpoint_address: secondary_interface.methods[&0].endpoint,
                    stack_pointer: D3D9_FIXTURE_STACK,
                    return_address: D3D9_FIXTURE_CALL_SITE,
                    call_site: D3D9_FIXTURE_CALL_SITE,
                },
                &mut session,
            )
        };
        let ComDispatchOutcome::Completed(query) = query else {
            panic!("IDirectSoundBuffer8 QueryInterface did not complete: {query:?}");
        };
        assert_eq!(query.return_value, ComReturnValue::HResult(COM_S_OK));
        acknowledge_d3d9_fixture_memory(&mut memory);
        assert_eq!(
            memory.read(D3D9_FIXTURE_SURFACE_OUTPUT, 4).unwrap(),
            secondary_pointer.to_le_bytes()
        );
        let release = dispatch_d3d9_fixture_release(
            &mut memory,
            &mut runtime,
            &secondary_interface,
            secondary_interface.guest_pointer,
        );
        assert_eq!(release.return_value, ComReturnValue::U32(1));

        write_d3d9_fixture_words(
            &mut memory,
            D3D9_FIXTURE_STACK,
            &[
                D3D9_FIXTURE_CALL_SITE.0,
                secondary_interface.guest_pointer.0,
                0,
                SECONDARY_BYTES,
                D3D9_FIXTURE_OUTPUT.0,
                D3D9_FIXTURE_OUTPUT.0 + 4,
                D3D9_FIXTURE_OUTPUT.0 + 8,
                D3D9_FIXTURE_OUTPUT.0 + 12,
                0,
            ],
        );
        acknowledge_d3d9_fixture_memory(&mut memory);
        let lock = {
            let mut session = ExecutionSession::new(&mut memory, &mut runtime);
            dispatch_dsound_com_call(
                &policy,
                ComCallTrap {
                    endpoint_address: secondary_interface.methods[&11].endpoint,
                    stack_pointer: D3D9_FIXTURE_STACK,
                    return_address: D3D9_FIXTURE_CALL_SITE,
                    call_site: D3D9_FIXTURE_CALL_SITE,
                },
                &mut session,
            )
        };
        let ComDispatchOutcome::Completed(lock) = lock else {
            panic!("Lock did not complete: {lock:?}");
        };
        assert_eq!(lock.return_value, ComReturnValue::HResult(COM_S_OK));
        assert_eq!(lock.guest_writes.len(), 4);
        acknowledge_d3d9_fixture_memory(&mut memory);
        let data1 = u32::from_le_bytes(
            memory
                .read(D3D9_FIXTURE_OUTPUT, 4)
                .unwrap()
                .try_into()
                .unwrap(),
        );
        let bytes1 = u32::from_le_bytes(
            memory
                .read(GuestAddress(D3D9_FIXTURE_OUTPUT.0 + 4), 4)
                .unwrap()
                .try_into()
                .unwrap(),
        );
        let data2 = u32::from_le_bytes(
            memory
                .read(GuestAddress(D3D9_FIXTURE_OUTPUT.0 + 8), 4)
                .unwrap()
                .try_into()
                .unwrap(),
        );
        let bytes2 = u32::from_le_bytes(
            memory
                .read(GuestAddress(D3D9_FIXTURE_OUTPUT.0 + 12), 4)
                .unwrap()
                .try_into()
                .unwrap(),
        );
        assert_eq!(data1, RUNTIME_AUDIO_RESOURCE_ARENA_BASE.0);
        assert_eq!(bytes1, SECONDARY_BYTES);
        assert_eq!((data2, bytes2), (0, 0));
        let locked_secondary = runtime
            .audio_buffer_for_com_object(secondary_interface.object_id)
            .expect("locked secondary logical audio buffer");
        let backing = locked_secondary.backing.expect("secondary sample backing");
        assert_eq!(backing.range.start(), GuestAddress(data1));
        assert_eq!(
            locked_secondary.active_lock.expect("active secondary Lock"),
            RuntimeAudioBufferLock {
                offset: 0,
                byte_length: SECONDARY_BYTES,
                flags: 0,
                first_segment: RuntimeAudioBufferSegment {
                    guest_address: GuestAddress(data1),
                    byte_length: SECONDARY_BYTES,
                },
                second_segment: None,
            }
        );
        assert_eq!(locked_secondary.content_generation, 0);
        assert!(
            runtime
                .process_address_space_allocator()
                .allocation(backing.address_allocation_id)
                .is_ok()
        );

        write_d3d9_fixture_words(
            &mut memory,
            D3D9_FIXTURE_OUTPUT,
            &[0xcccc_cccc, 0xcccc_cccc, 0xcccc_cccc, 0xcccc_cccc],
        );
        write_d3d9_fixture_words(
            &mut memory,
            D3D9_FIXTURE_STACK,
            &[
                D3D9_FIXTURE_CALL_SITE.0,
                secondary_interface.guest_pointer.0,
                0,
                SECONDARY_BYTES,
                D3D9_FIXTURE_OUTPUT.0,
                D3D9_FIXTURE_OUTPUT.0 + 4,
                D3D9_FIXTURE_OUTPUT.0 + 8,
                D3D9_FIXTURE_OUTPUT.0 + 12,
                0,
            ],
        );
        acknowledge_d3d9_fixture_memory(&mut memory);
        let before_double_lock = runtime.clone();
        let double_lock = {
            let mut session = ExecutionSession::new(&mut memory, &mut runtime);
            dispatch_dsound_com_call(
                &policy,
                ComCallTrap {
                    endpoint_address: secondary_interface.methods[&11].endpoint,
                    stack_pointer: D3D9_FIXTURE_STACK,
                    return_address: D3D9_FIXTURE_CALL_SITE,
                    call_site: D3D9_FIXTURE_CALL_SITE,
                },
                &mut session,
            )
        };
        let ComDispatchOutcome::Completed(double_lock) = double_lock else {
            panic!("double Lock did not return HRESULT: {double_lock:?}");
        };
        assert_eq!(
            double_lock.return_value,
            ComReturnValue::HResult(DSOUND_E_INVALIDARG)
        );
        assert!(double_lock.guest_writes.is_empty());
        assert_eq!(runtime, before_double_lock);
        assert_eq!(
            memory.read(D3D9_FIXTURE_OUTPUT, 16).unwrap(),
            vec![0xcc; 16]
        );

        write_d3d9_fixture_words(
            &mut memory,
            D3D9_FIXTURE_STACK,
            &[
                D3D9_FIXTURE_CALL_SITE.0,
                secondary_interface.guest_pointer.0,
                data1 + 4,
                bytes1,
                data2,
                bytes2,
            ],
        );
        acknowledge_d3d9_fixture_memory(&mut memory);
        let before_mismatched_unlock = runtime.clone();
        let mismatched_unlock = {
            let mut session = ExecutionSession::new(&mut memory, &mut runtime);
            dispatch_dsound_com_call(
                &policy,
                ComCallTrap {
                    endpoint_address: secondary_interface.methods[&19].endpoint,
                    stack_pointer: D3D9_FIXTURE_STACK,
                    return_address: D3D9_FIXTURE_CALL_SITE,
                    call_site: D3D9_FIXTURE_CALL_SITE,
                },
                &mut session,
            )
        };
        let ComDispatchOutcome::Fault(mismatched_unlock) = mismatched_unlock else {
            panic!("mismatched Unlock unexpectedly completed: {mismatched_unlock:?}");
        };
        assert_eq!(mismatched_unlock.class, ComFaultClass::Unsupported);
        assert_eq!(
            mismatched_unlock.code,
            "dsound-buffer-unlock-segments-unsupported"
        );
        assert_eq!(runtime, before_mismatched_unlock);

        memory
            .write(GuestAddress(data1), &vec![0; SECONDARY_BYTES as usize])
            .expect("zero writable secondary sample backing");
        acknowledge_d3d9_fixture_memory(&mut memory);
        write_d3d9_fixture_words(
            &mut memory,
            D3D9_FIXTURE_STACK,
            &[
                D3D9_FIXTURE_CALL_SITE.0,
                secondary_interface.guest_pointer.0,
                data1,
                bytes1,
                data2,
                bytes2,
            ],
        );
        acknowledge_d3d9_fixture_memory(&mut memory);
        let unlock = {
            let mut session = ExecutionSession::new(&mut memory, &mut runtime);
            dispatch_dsound_com_call(
                &policy,
                ComCallTrap {
                    endpoint_address: secondary_interface.methods[&19].endpoint,
                    stack_pointer: D3D9_FIXTURE_STACK,
                    return_address: D3D9_FIXTURE_CALL_SITE,
                    call_site: D3D9_FIXTURE_CALL_SITE,
                },
                &mut session,
            )
        };
        let ComDispatchOutcome::Completed(unlock) = unlock else {
            panic!("Unlock did not complete: {unlock:?}");
        };
        assert_eq!(unlock.return_value, ComReturnValue::HResult(COM_S_OK));
        assert!(unlock.guest_writes.is_empty());
        acknowledge_d3d9_fixture_memory(&mut memory);
        assert!(memory.write(GuestAddress(data1), &[1]).is_err());
        let unlocked_secondary = runtime
            .audio_buffer_for_com_object(secondary_interface.object_id)
            .expect("unlocked secondary logical audio buffer");
        assert_eq!(unlocked_secondary.active_lock, None);
        assert_eq!(unlocked_secondary.content_generation, 1);

        for (slot, value) in [
            (15_u16, (-1_000_i32) as u32),
            (16_u16, (-500_i32) as u32),
            (17_u16, 22_050),
            (13_u16, 4),
        ] {
            write_d3d9_fixture_words(
                &mut memory,
                D3D9_FIXTURE_STACK,
                &[
                    D3D9_FIXTURE_CALL_SITE.0,
                    secondary_interface.guest_pointer.0,
                    value,
                ],
            );
            acknowledge_d3d9_fixture_memory(&mut memory);
            let control = {
                let mut session = ExecutionSession::new(&mut memory, &mut runtime);
                dispatch_dsound_com_call(
                    &policy,
                    ComCallTrap {
                        endpoint_address: secondary_interface.methods[&slot].endpoint,
                        stack_pointer: D3D9_FIXTURE_STACK,
                        return_address: D3D9_FIXTURE_CALL_SITE,
                        call_site: D3D9_FIXTURE_CALL_SITE,
                    },
                    &mut session,
                )
            };
            let ComDispatchOutcome::Completed(control) = control else {
                panic!("secondary control slot {slot} did not complete: {control:?}");
            };
            assert_eq!(control.return_value, ComReturnValue::HResult(COM_S_OK));
        }
        let controls = runtime
            .audio_buffer_for_com_object(secondary_interface.object_id)
            .expect("controlled secondary logical audio buffer")
            .controls;
        assert_eq!(controls.volume_hundredths_db, -1_000);
        assert_eq!(controls.pan_hundredths_db, -500);
        assert_eq!(controls.frequency_hz, Some(22_050));
        assert_eq!(controls.current_position_bytes, 4);

        write_d3d9_fixture_words(
            &mut memory,
            D3D9_FIXTURE_STACK,
            &[
                D3D9_FIXTURE_CALL_SITE.0,
                secondary_interface.guest_pointer.0,
                1,
            ],
        );
        acknowledge_d3d9_fixture_memory(&mut memory);
        let before_invalid_volume = runtime.clone();
        let invalid_volume = {
            let mut session = ExecutionSession::new(&mut memory, &mut runtime);
            dispatch_dsound_com_call(
                &policy,
                ComCallTrap {
                    endpoint_address: secondary_interface.methods[&15].endpoint,
                    stack_pointer: D3D9_FIXTURE_STACK,
                    return_address: D3D9_FIXTURE_CALL_SITE,
                    call_site: D3D9_FIXTURE_CALL_SITE,
                },
                &mut session,
            )
        };
        let ComDispatchOutcome::Completed(invalid_volume) = invalid_volume else {
            panic!("invalid secondary volume did not return HRESULT: {invalid_volume:?}");
        };
        assert_eq!(
            invalid_volume.return_value,
            ComReturnValue::HResult(DSOUND_E_INVALIDARG)
        );
        assert_eq!(runtime, before_invalid_volume);

        write_d3d9_fixture_words(
            &mut memory,
            D3D9_FIXTURE_STACK,
            &[
                D3D9_FIXTURE_CALL_SITE.0,
                secondary_interface.guest_pointer.0,
                0,
                0,
                0,
            ],
        );
        acknowledge_d3d9_fixture_memory(&mut memory);
        let secondary_play = {
            let mut session = ExecutionSession::new(&mut memory, &mut runtime);
            dispatch_dsound_com_call(
                &policy,
                ComCallTrap {
                    endpoint_address: secondary_interface.methods[&12].endpoint,
                    stack_pointer: D3D9_FIXTURE_STACK,
                    return_address: D3D9_FIXTURE_CALL_SITE,
                    call_site: D3D9_FIXTURE_CALL_SITE,
                },
                &mut session,
            )
        };
        let ComDispatchOutcome::Completed(secondary_play) = secondary_play else {
            panic!("secondary looping Play did not complete: {secondary_play:?}");
        };
        assert_eq!(
            secondary_play.return_value,
            ComReturnValue::HResult(COM_S_OK)
        );
        assert_eq!(
            runtime
                .audio_buffer_for_com_object(secondary_interface.object_id)
                .unwrap()
                .playback,
            RuntimeAudioPlaybackState::PlayingOnce
        );

        let release = dispatch_d3d9_fixture_release(
            &mut memory,
            &mut runtime,
            &secondary_interface,
            secondary_interface.guest_pointer,
        );
        assert_eq!(release.return_value, ComReturnValue::U32(0));
        assert!(release.object_retired);
        assert!(memory.read(backing.range.start(), 4).is_err());
        assert!(
            runtime
                .process_address_space_allocator()
                .allocation(backing.address_allocation_id)
                .is_err()
        );
        assert!(
            runtime
                .audio_buffer_for_com_object(secondary_interface.object_id)
                .is_err()
        );
        assert_eq!(runtime.audio_buffers().len(), 1);
        assert_eq!(runtime.current_last_error().unwrap(), 0x1234_5678);
    }

    #[test]
    fn d3d9_vertex_buffer_resource_owner_exhaustion_rolls_back_all_authorities() {
        let mut runtime = d3d9_fixture_runtime();
        let window = seed_created_d3d9_window(&mut runtime);
        let mut memory = d3d9_fixture_memory_with_limits(MemoryLimits {
            max_owners: 4,
            ..MemoryLimits::default()
        });
        let root = install_com_object(&mut memory, &mut runtime, direct3d9_object_spec())
            .expect("install D3D9 root COM object");
        let root_interface = root
            .interfaces
            .iter()
            .find(|interface| interface.iid == D3D9_IDIRECT3D9_IID)
            .expect("IDirect3D9 interface")
            .clone();
        acknowledge_d3d9_fixture_memory(&mut memory);

        write_d3d9_fixture_words(
            &mut memory,
            D3D9_FIXTURE_PRESENTATION,
            &[
                0,
                0,
                D3D9_D3DFMT_UNKNOWN,
                1,
                D3D9_D3DMULTISAMPLE_NONE,
                0,
                D3D9_D3DSWAPEFFECT_DISCARD,
                window.handle,
                1,
                0,
                D3D9_D3DFMT_D16,
                0,
                0,
                0,
            ],
        );
        write_d3d9_fixture_words(&mut memory, D3D9_FIXTURE_OUTPUT, &[0xcccc_cccc]);
        write_d3d9_fixture_words(
            &mut memory,
            D3D9_FIXTURE_STACK,
            &[
                D3D9_FIXTURE_CALL_SITE.0,
                root_interface.guest_pointer.0,
                0,
                D3D9_D3DDEVTYPE_HAL,
                window.handle,
                D3D9_CREATE_HARDWARE_VERTEX_PROCESSING_MULTITHREADED,
                D3D9_FIXTURE_PRESENTATION.0,
                D3D9_FIXTURE_OUTPUT.0,
            ],
        );
        acknowledge_d3d9_fixture_memory(&mut memory);
        let create_device = match dispatch_d3d9_fixture_frontend(
            &mut memory,
            &mut runtime,
            &D3d9CompatibilityPolicy::controlled_v31(),
            &root_interface,
            16,
            D3D9_FIXTURE_CALL_SITE,
        ) {
            ComDispatchOutcome::Completed(completion) => completion,
            other => panic!("CreateDevice did not complete: {other:?}"),
        };
        assert_eq!(
            create_device.return_value,
            ComReturnValue::HResult(COM_S_OK)
        );
        acknowledge_d3d9_fixture_memory(&mut memory);
        let device_pointer = u32::from_le_bytes(
            memory
                .read(D3D9_FIXTURE_OUTPUT, 4)
                .unwrap()
                .try_into()
                .unwrap(),
        );
        let device_interface = runtime
            .com_interface_for_pointer(GuestAddress(device_pointer))
            .expect("IDirect3DDevice9 output")
            .clone();

        write_d3d9_fixture_words(&mut memory, D3D9_FIXTURE_SURFACE_OUTPUT, &[0xcccc_cccc]);
        write_d3d9_fixture_words(
            &mut memory,
            D3D9_FIXTURE_STACK,
            &[
                0x1005_a890,
                device_interface.guest_pointer.0,
                112,
                D3D9_D3DUSAGE_WRITEONLY,
                0x144,
                D3D9_D3DPOOL_MANAGED,
                D3D9_FIXTURE_SURFACE_OUTPUT.0,
                0,
            ],
        );
        acknowledge_d3d9_fixture_memory(&mut memory);
        let before_memory = memory.stats();
        let before_runtime = runtime.clone();
        let fault = match dispatch_d3d9_fixture_frontend(
            &mut memory,
            &mut runtime,
            &D3d9CompatibilityPolicy::controlled_v31(),
            &device_interface,
            26,
            GuestAddress(0x1005_a890),
        ) {
            ComDispatchOutcome::Fault(fault) => fault,
            other => panic!("resource exhaustion unexpectedly completed: {other:?}"),
        };
        assert_eq!(fault.class, ComFaultClass::ResourceLimit);
        assert_eq!(fault.code, "memory-owner-limit-exceeded");
        assert_eq!(memory.stats(), before_memory);
        assert_eq!(runtime, before_runtime);
        assert_eq!(
            memory.read(D3D9_FIXTURE_SURFACE_OUTPUT, 4).unwrap(),
            0xcccc_cccc_u32.to_le_bytes()
        );
    }

    fn d3d9_fixture_runtime() -> RuntimeKernel {
        let mut runtime = RuntimeKernel::new(RuntimeBootstrap {
            process_id: RuntimeProcessId(0x1000),
            initial_thread_id: RuntimeThreadId(0x2000),
            initial_last_error: 0x1234_5678,
            address_space: ProcessAddressSpaceLayout::vine_v1(),
            clock: RuntimeClockState {
                wall_clock_file_time: 134_282_016_000_000_000,
                tick_count: 3_600_000,
                performance_counter: 12_345_678_901,
                performance_frequency: 10_000_000,
            },
            startup_info: RuntimeStartupInfo {
                profile_id: "d3d9-create-device-transaction-test".to_owned(),
                bytes: vec![0; X86_STARTUP_INFO_W_BYTE_LENGTH],
            },
            process_image_path_a: RuntimeProcessImagePathA {
                source_authority: "d3d9-create-device-transaction-fixture".to_owned(),
                bytes_with_nul: b"C:\\Vine\\fixture.exe\0".to_vec(),
                encoding: RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING.to_owned(),
            },
            initial_current_directory_a: RuntimeCurrentDirectoryA {
                source_authority: "d3d9-create-device-transaction-fixture".to_owned(),
                bytes_with_nul: b"C:\\Vine\0".to_vec(),
                encoding: RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING.to_owned(),
            },
            encoded_pointer_namespace: LOGICAL_ENCODED_POINTER_NAMESPACE.to_owned(),
            encoded_pointer_token_prefix: 0xec01_0000,
        })
        .expect("D3D9 transaction runtime");
        runtime
            .start_initial_thread()
            .expect("start fixture thread");
        runtime
            .configure_display_profile(RuntimeDisplayProfile {
                id: "single-monitor-1920x1080-work-1920x1032-v2".to_owned(),
                primary_monitor_rect: RuntimeRect {
                    left: 0,
                    top: 0,
                    right: 1920,
                    bottom: 1080,
                },
                primary_work_rect: RuntimeRect {
                    left: 0,
                    top: 0,
                    right: 1920,
                    bottom: 1032,
                },
            })
            .expect("configure fixture display");
        runtime
    }

    fn seed_created_d3d9_window(runtime: &mut RuntimeKernel) -> RuntimeWindow {
        let module = RuntimeWindowClassModule {
            module_id: RuntimeModuleId(1),
            load_base: GuestAddress(0x1000_0000),
            source_sha256: "0".repeat(64),
        };
        let class_plan = runtime
            .user32
            .plan_window_class_registration(RuntimeWindowClassRegistration {
                module: module.clone(),
                style: 0x28,
                window_procedure: RuntimeWindowProcedure {
                    module_id: module.module_id,
                    address: GuestAddress(0x1009_2550),
                    rva: Rva(0x0009_2550),
                    calling_convention: RuntimeWindowProcedureCallingConvention::Stdcall,
                    argument_bytes: RUNTIME_WINDOW_PROCEDURE_ARGUMENT_BYTES,
                },
                class_extra_bytes: 0,
                window_extra_bytes: 0,
                icon: None,
                cursor: None,
                background_brush: 0,
                menu_name: None,
                class_name: RuntimeWindowClassNameA {
                    value: "BGI - Main window".to_owned(),
                    encoding: RUNTIME_WINDOW_CLASS_NAME_A_ENCODING.to_owned(),
                },
                small_icon: None,
            })
            .expect("plan fixture window class");
        let class = runtime
            .user32
            .commit_window_class_registration(class_plan)
            .expect("commit fixture window class");
        let creation_plan = runtime
            .user32
            .plan_window_creation(RuntimeWindowCreationRequest {
                owner_thread_id: RuntimeThreadId(0x2000),
                class: RuntimeWindowClassReference {
                    class_id: class.class_id,
                    atom: class.atom,
                    module_id: module.module_id,
                },
                instance: module,
                name: RuntimeWindowNameA {
                    value: "Ethornell - BURIKO General Interpreter".to_owned(),
                    encoding: RUNTIME_WINDOW_NAME_A_ENCODING.to_owned(),
                },
                extended_style: 0x0004_0000,
                effective_extended_style: 0x0004_0100,
                style: 0x80ca_0000,
                effective_style: 0x84ca_0000,
                geometry: RuntimeWindowGeometry {
                    x: 557,
                    y: 214,
                    width: 806,
                    height: 629,
                    client_x: 560,
                    client_y: 240,
                    client_width: 800,
                    client_height: 600,
                },
                parent: 0,
                menu: 0,
                create_parameter: 0,
                entry_last_error: 0x1234_5678,
            })
            .expect("plan fixture window");
        let created = runtime
            .user32
            .commit_window_creation(creation_plan)
            .expect("commit fixture window");
        for (message, result) in [
            (RuntimeWindowCreationMessage::NonClientCreate, 1),
            (RuntimeWindowCreationMessage::NonClientCalculate, 0),
            (RuntimeWindowCreationMessage::Create, 0),
            (RuntimeWindowCreationMessage::Size, 0),
            (RuntimeWindowCreationMessage::Move, 0),
        ] {
            let plan = runtime
                .user32
                .plan_window_creation_callback(created.window_id, message, result)
                .expect("plan fixture window callback");
            runtime
                .user32
                .commit_window_creation_callback(plan)
                .expect("commit fixture window callback");
        }
        runtime
            .window(created.handle)
            .expect("created fixture window")
            .clone()
    }

    fn d3d9_fixture_memory() -> GuestMemory {
        d3d9_fixture_memory_with_limits(MemoryLimits::default())
    }

    fn d3d9_fixture_memory_with_limits(limits: MemoryLimits) -> GuestMemory {
        let mut memory = GuestMemory::new(limits).expect("fixture guest memory");
        let owner = memory
            .create_owner(MemoryOwnerKind::Runtime)
            .expect("fixture memory owner");
        let range = memory
            .reserve_fixed(
                owner,
                MemoryRegionKind::Runtime,
                D3D9_FIXTURE_MEMORY_BASE,
                0x3000,
            )
            .expect("fixture memory reservation");
        memory
            .commit(
                owner,
                range,
                PageProtection {
                    read: true,
                    write: true,
                    execute: false,
                },
            )
            .expect("fixture memory commit");
        memory.seal_owner(owner).expect("seal fixture memory owner");
        let reflection = memory
            .pending_reflection()
            .expect("fixture initial memory reflection");
        memory
            .acknowledge_delta(reflection.invalidation.generation)
            .expect("acknowledge fixture initial memory");
        memory
    }

    fn write_d3d9_fixture_words(memory: &mut GuestMemory, address: GuestAddress, words: &[u32]) {
        let bytes = words
            .iter()
            .flat_map(|word| word.to_le_bytes())
            .collect::<Vec<_>>();
        memory.write(address, &bytes).expect("write fixture words");
    }

    fn acknowledge_d3d9_fixture_memory(memory: &mut GuestMemory) {
        if memory.stats().pending_invalidation.is_some() {
            let delta = memory.pending_delta().expect("fixture pending delta");
            memory
                .acknowledge_delta(delta.generation)
                .expect("acknowledge fixture delta");
        }
    }

    fn dispatch_d3d9_fixture_release(
        memory: &mut GuestMemory,
        runtime: &mut RuntimeKernel,
        interface: &RuntimeComInterfaceRecord,
        pointer: GuestAddress,
    ) -> crate::com::ComCallCompletion {
        write_d3d9_fixture_words(
            memory,
            D3D9_FIXTURE_STACK,
            &[D3D9_FIXTURE_CALL_SITE.0, pointer.0],
        );
        acknowledge_d3d9_fixture_memory(memory);
        let mut session = ExecutionSession::new(memory, runtime);
        match session.dispatch_com_call(ComCallTrap {
            endpoint_address: interface.methods[&2].endpoint,
            stack_pointer: D3D9_FIXTURE_STACK,
            return_address: D3D9_FIXTURE_CALL_SITE,
            call_site: D3D9_FIXTURE_CALL_SITE,
        }) {
            ComDispatchOutcome::Completed(completion) => completion,
            other => panic!("COM Release did not complete: {other:?}"),
        }
    }

    fn dispatch_d3d9_fixture_frontend(
        memory: &mut GuestMemory,
        runtime: &mut RuntimeKernel,
        policy: &D3d9CompatibilityPolicy,
        interface: &RuntimeComInterfaceRecord,
        slot: u16,
        call_site: GuestAddress,
    ) -> ComDispatchOutcome {
        let mut session = ExecutionSession::new(memory, runtime);
        dispatch_d3d9_com_call(
            policy,
            ComCallTrap {
                endpoint_address: interface.methods[&slot].endpoint,
                stack_pointer: D3D9_FIXTURE_STACK,
                return_address: call_site,
                call_site,
            },
            &mut session,
        )
    }
}

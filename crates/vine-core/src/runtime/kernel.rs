//! Runtime aggregate root and cross-subsystem transaction coordinator.

#[path = "dynamic_library.rs"]
mod dynamic_library;
#[path = "image_tls.rs"]
mod image_tls;
use super::RuntimeRect;
use image_tls::RuntimeImageTls;

#[cfg(test)]
use super::audio::RuntimeAudioDevice;
use super::audio::RuntimeAudioDeviceId;

#[cfg(test)]
use super::audio::RuntimeAudioBufferLock;
use crate::api::ApiGuestCallbackCall;
use crate::com::{ComObjectSpec, InstalledComObject};

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::bootstrap::{INITIAL_THREAD_STACK_TOP, InitialThreadStack};
use crate::callbacks::{
    CALLBACK_SCRATCH_ARENA_BASE, CALLBACK_SCRATCH_ARENA_SIZE, CALLBACK_SCRATCH_SLOT_SIZE,
};
use crate::dynamic_exports::{
    DYNAMIC_EXPORT_ARENA_BASE, DYNAMIC_EXPORT_ARENA_SIZE, DynamicExportThunk,
};
use crate::environment::InitialProcessEnvironment;
use crate::imports::RuntimeExportCatalog;
use crate::materialize::ImageMaterialization;
use crate::memory::{
    GUEST_PAGE_SIZE, GuestAddress, GuestMemory, GuestMemoryError, GuestMemoryErrorClass,
    GuestRange, MemoryAccess, MemoryOwnerId, MemoryOwnerKind, MemoryRegionInfo, MemoryRegionKind,
    PageProtection,
};
use crate::module_graph::{RuntimeModuleClass, RuntimeModuleGraph, RuntimeModuleId};
use crate::module_images::{
    RuntimeModuleImage, RuntimeModuleImageError, RuntimeModuleImageRegistry, RuntimeResolvedExport,
};
use crate::pe32::{ExportLookup, Pe32LoadPlan, ResourceIdentifier};

use super::address_space::{
    ProcessAddressAllocationClassification, ProcessAddressAllocationId,
    ProcessAddressAllocationPurpose, ProcessAddressAllocationRecord,
    ProcessAddressAllocationReleasePolicy, ProcessAddressAllocationRequest,
    ProcessAddressSpaceAllocator, RuntimeAddressRange,
};
use super::audio::{
    AudioProgress, BrowserAudioAction, BrowserAudioBufferState, BrowserAudioPreparation,
    PreparedBrowserAudio, RUNTIME_AUDIO_NOTIFICATION_LIMIT,
    RUNTIME_AUDIO_NOTIFICATION_PACING_SERVICE, RuntimeAudio, RuntimeAudioBuffer,
    RuntimeAudioBufferBacking, RuntimeAudioBufferId, RuntimeAudioComRetirement,
    RuntimeAudioCooperativeState, RuntimeAudioNotification, RuntimeAudioPlaybackState,
};
use super::clock::RuntimeClockState;
use super::com::{
    RuntimeCom, RuntimeComBacking, RuntimeComClass, RuntimeComIid, RuntimeComInterfaceId,
    RuntimeComInterfaceRecord, RuntimeComMethodTarget, RuntimeComObjectId,
    RuntimeComObjectRegistration, RuntimeComRelease,
};
use super::displays::{RuntimeDisplayProfile, RuntimeDisplays, RuntimeLogicalMonitor};
use super::files::{
    RUNTIME_OPFS_READ_SERVICE, RUNTIME_SOURCE_READ_SERVICE, RuntimeFileSearch, RuntimeFileSearchId,
    RuntimeFiles, RuntimeSourceFile, RuntimeSourceFileId, RuntimeSourceFileOpen,
    RuntimeSourceReadCompletionPlan, RuntimeSourceReadHostResponse, RuntimeSourceSnapshotManifest,
    SourceOpenOutcome, SourceSearchBatchEntry, SourceSearchSelection,
};
use super::gdi::{
    RuntimeDeviceContext, RuntimeDeviceContextTarget, RuntimeGdi, RuntimeGdiDibSectionDescriptor,
    RuntimeGdiGraphicsSurfaceTarget, RuntimeGdiScreenTarget, RuntimeGdiTextRasterTarget,
    RuntimeGdiWindowTarget, RuntimeGetDcResult, RuntimeLogFontA,
};
use super::gdiplus::RuntimeGdiplus;
use super::graphics::{
    RuntimeGraphics, RuntimeGraphicsComRetirement, RuntimeGraphicsDevice,
    RuntimeGraphicsDeviceCreationRequest, RuntimeGraphicsDeviceId,
    RuntimeGraphicsFlexibleVertexFormat, RuntimeGraphicsRenderStateUpdate,
    RuntimeGraphicsSamplerStateUpdate, RuntimeGraphicsSurface, RuntimeGraphicsSurfaceDescriptor,
    RuntimeGraphicsSurfaceId, RuntimeGraphicsTexture, RuntimeGraphicsTextureBacking,
    RuntimeGraphicsTextureDescriptor, RuntimeGraphicsTextureId,
    RuntimeGraphicsTextureStageStateUpdate, RuntimeGraphicsWindowTarget, texture_byte_layout,
};
#[cfg(test)]
use super::heaps::RuntimeHeapBacking;
use super::heaps::{RuntimeHeap, RuntimeHeapAllocation, RuntimeHeapInitialization, RuntimeHeaps};
use super::imm32::{RuntimeImeContext, RuntimeImm32};
use super::input::{
    RuntimeCursorPosition, RuntimeInput, RuntimeInputProfile, RuntimeKeyboardSourceEvent,
    RuntimeKeyboardSourceEventKind, RuntimeKeyboardState, RuntimePointerButtonEventTarget,
    RuntimePointerButtonSourceEvent, RuntimePointerPositionSourceEvent,
};
#[cfg(test)]
use super::messages::RuntimeMessageReceiveDisposition;
use super::messages::{RuntimeMessages, RuntimeThreadMessage};
use super::modules::{
    RuntimeDllLifecycleAction, RuntimeDllLifecyclePhase, RuntimeDllLifecycleRequest,
    RuntimeGuestImportBinding, RuntimeModule, RuntimeModules, runtime_module_image_error,
    runtime_module_image_state_error,
};
use super::objects::{
    RuntimeEvent, RuntimeHandle, RuntimeObjectId, RuntimeObjectKey, RuntimeObjectKind,
    RuntimeObjectRegistrationPlan, RuntimeObjectTable, RuntimeWaitState,
};
use super::process::RuntimeProcessDirectories;
use super::process::{
    RUNTIME_PROCESS_IMAGE_PATH_A_MAX_BYTES, RuntimeCommandLine, RuntimeCurrentDirectoryA,
    RuntimeEnvironmentStrings, RuntimeProcessEnvironment, RuntimeProcessId,
    RuntimeProcessImagePathA, RuntimeProcessState, RuntimeStartupInfo,
    X86_STARTUP_INFO_W_BYTE_LENGTH,
};
use super::registries::{RuntimeDynamicEndpoint, RuntimeRegistries};
use super::scheduler::{
    RuntimeApiBoundaryDisposition, RuntimeHostCompletion, RuntimeHostRequest, RuntimeScheduler,
    RuntimeSchedulerEvent, SchedulerThreadState,
};
#[cfg(test)]
use super::synchronization::RuntimeMutex;
use super::synchronization::RuntimeSynchronization;
use super::synchronization::{RuntimeCriticalSection, RuntimeMutexId, RuntimeMutexName};
use super::thread::{
    RUNTIME_GUEST_THREAD_ENVIRONMENT_ARENA_BASE, RUNTIME_GUEST_THREAD_ENVIRONMENT_ARENA_SIZE,
    RUNTIME_GUEST_THREAD_STACK_ARENA_BASE, RUNTIME_GUEST_THREAD_STACK_ARENA_SIZE,
    RuntimeCallbackFrame, RuntimeCallbackFrameId, RuntimeCallbackScratch, RuntimeComApartmentModel,
    RuntimeCreateThreadRequest, RuntimeGuestThreadBootstrap, RuntimeThreadEnvironment,
    RuntimeThreadId, RuntimeThreadLifecycleState, RuntimeThreadPriority, RuntimeThreadStack,
    RuntimeThreadState, RuntimeThreads,
};
use super::timers::{
    RuntimeTimers, RuntimeWaitableTimer, RuntimeWaitableTimerId,
    RuntimeWindowTimerRegistrationRequest,
};
use super::user32::{
    RUNTIME_MESSAGE_BOX_SERVICE, RUNTIME_WINDOW_CLASS_STYLE_DOUBLE_CLICKS, RuntimeCursor,
    RuntimeCursorSource, RuntimeIcon, RuntimeIconResource, RuntimeImeStatusWindowRequest,
    RuntimeMessageBox, RuntimeSetWindowPosCallbackAction, RuntimeSetWindowPosCallbackMessage,
    RuntimeSetWindowPosDefaultCallbackAction, RuntimeSetWindowPosDefaultCallbackMessage,
    RuntimeSetWindowPosDefaultOperation, RuntimeSetWindowPosOperation, RuntimeSetWindowPosRequest,
    RuntimeSetWindowPosVariant, RuntimeUpdateWindowOperation, RuntimeUser32, RuntimeWindow,
    RuntimeWindowClass, RuntimeWindowClassRegistration, RuntimeWindowCreationCallbackAction,
    RuntimeWindowCreationMessage, RuntimeWindowCreationRequest, RuntimeWindowGeometry,
    RuntimeWindowId, RuntimeWindowLifecycle, RuntimeWindowNameA, RuntimeWindowProcedure,
};
use super::waits::{
    RUNTIME_OBJECT_WAIT_SERVICE, RUNTIME_WAITABLE_TIMER_PACING_SERVICE, RuntimeObjectWaitPacing,
    RuntimeWaitDisposition, RuntimeWaitForSingleObjectExBegin, RuntimeWaitForSingleObjectExOutcome,
    RuntimeWaitRegistration, RuntimeWaitableTimerPacing,
};

const WM_MOUSEMOVE: u32 = 0x0200;

#[cfg(test)]
use super::gdi::{
    RuntimeDeviceContextId, RuntimeGdiObject, RuntimeGdiObjectId, RuntimeGdiSurface,
    RuntimeGdiSurfaceId,
};

#[cfg(test)]
use super::imm32::{RuntimeImeContextAssociation, RuntimeImeContextId};
use super::objects::RuntimeObjectRecord;
use super::thread::RuntimeComApartmentState;
#[cfg(test)]
use super::timers::{RuntimeWindowTimer, RuntimeWindowTimerId};
#[cfg(test)]
use super::user32::RuntimeImeWindow;

#[cfg(test)]
use super::user32::{
    RuntimeCursorId, RuntimeIconId, RuntimeImeWindowId, RuntimePaintSession, RuntimeWindowClassId,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeBootstrap {
    pub process_id: RuntimeProcessId,
    pub initial_thread_id: RuntimeThreadId,
    pub initial_last_error: u32,
    pub clock: RuntimeClockState,
    pub startup_info: RuntimeStartupInfo,
    pub process_image_path_a: RuntimeProcessImagePathA,
    pub initial_current_directory_a: RuntimeCurrentDirectoryA,
    pub encoded_pointer_cookie: u32,
}

pub(crate) struct RuntimeSourceReadAggregateCompletionPlan {
    scheduler: RuntimeHostRequest,
    source_read: RuntimeSourceReadCompletionPlan,
}

pub(crate) struct RuntimeWaitableTimerPacingCompletionPlan {
    scheduler: RuntimeHostRequest,
    registration: RuntimeWaitRegistration,
    clock: (u64, RuntimeClockState),
    expiration: Option<(RuntimeWaitableTimerId, u64)>,
    wait_updates: Option<Vec<(RuntimeObjectId, RuntimeWaitState)>>,
    outcome: RuntimeWaitForSingleObjectExOutcome,
    repace: Option<RuntimeWaitableTimerPacing>,
}

pub(crate) struct RuntimeObjectWaitCompletionPlan {
    scheduler: RuntimeHostRequest,
    registration: RuntimeWaitRegistration,
    outcome: RuntimeWaitForSingleObjectExOutcome,
    retry: Option<RuntimeObjectWaitPacing>,
}

impl RuntimeObjectWaitCompletionPlan {
    pub(crate) const fn registration(&self) -> RuntimeWaitRegistration {
        self.registration
    }
    pub(crate) const fn outcome(&self) -> RuntimeWaitForSingleObjectExOutcome {
        self.outcome
    }
    pub(crate) const fn retry(&self) -> Option<RuntimeObjectWaitPacing> {
        self.retry
    }
}

pub(crate) struct RuntimeAudioNotificationWaitCompletionPlan {
    scheduler: RuntimeHostRequest,
    audio: Option<AudioProgress>,
    event_updates: Vec<(RuntimeObjectId, RuntimeWaitState)>,
    result_index: Option<u32>,
    retry: Option<Vec<u8>>,
}

impl RuntimeAudioNotificationWaitCompletionPlan {
    pub(crate) const fn result_index(&self) -> Option<u32> {
        self.result_index
    }

    pub(crate) fn retry_payload(&self) -> Option<&[u8]> {
        self.retry.as_deref()
    }
}

fn encode_inactive_audio_notification_wait_payload(
    handles: &[RuntimeHandle],
    timeout_milliseconds: u32,
    host_time_100ns: u64,
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
    payload.extend_from_slice(&host_time_100ns.to_le_bytes());
    payload
}

fn decode_audio_notification_wait_handles(
    payload: &[u8],
    timeout_milliseconds: u32,
) -> Result<Vec<RuntimeHandle>, RuntimeStateError> {
    if !payload.len().is_multiple_of(4) {
        return Err(RuntimeStateError::new(
            "audio notification wait payload is not DWORD aligned",
        ));
    }
    if payload.len() > (8 + RUNTIME_AUDIO_NOTIFICATION_LIMIT) * 4 {
        return Err(RuntimeStateError::new(
            "audio notification wait payload exceeds its bounded handle layout",
        ));
    }
    let word = |index: usize| {
        payload
            .get(index * 4..index * 4 + 4)
            .map(|bytes| u32::from_le_bytes(bytes.try_into().unwrap()))
    };
    let (payload_timeout, handle_count, handles_start) = match word(0) {
        Some(1) => (word(4), word(5), 6),
        Some(2) => (word(2), word(3), 4),
        _ => {
            return Err(RuntimeStateError::new(
                "audio notification wait payload has an unknown active/inactive mode",
            ));
        }
    };
    let handle_count = handle_count.unwrap_or(0) as usize;
    if payload_timeout != Some(timeout_milliseconds)
        || handle_count == 0
        || handle_count > RUNTIME_AUDIO_NOTIFICATION_LIMIT
        || payload.len() / 4 != handles_start + handle_count + 2
    {
        return Err(RuntimeStateError::new(
            "audio notification wait payload has inconsistent timeout or handle bounds",
        ));
    }
    Ok(payload[handles_start * 4..payload.len() - 8]
        .chunks_exact(4)
        .map(|bytes| RuntimeHandle(u32::from_le_bytes(bytes.try_into().unwrap())))
        .collect())
}

impl RuntimeWaitableTimerPacingCompletionPlan {
    pub(crate) const fn outcome(&self) -> RuntimeWaitForSingleObjectExOutcome {
        self.outcome
    }

    pub(crate) const fn registration(&self) -> RuntimeWaitRegistration {
        self.registration
    }

    pub(crate) const fn repace(&self) -> Option<RuntimeWaitableTimerPacing> {
        self.repace
    }
}

impl RuntimeSourceReadAggregateCompletionPlan {
    pub(crate) const fn buffer(&self) -> GuestAddress {
        self.source_read.buffer()
    }

    pub(crate) const fn bytes_read(&self) -> GuestAddress {
        self.source_read.bytes_read()
    }
}

struct RuntimeComRetirement {
    com: RuntimeComRelease,
    address_space: Option<Vec<ProcessAddressAllocationId>>,
    audio: Option<RuntimeAudioComRetirement>,
    graphics: Option<RuntimeGraphicsComRetirement>,
    memory_retirements: Vec<(MemoryOwnerId, GuestRange)>,
}

#[derive(Debug)]
struct PendingMessageBox {
    thread_id: RuntimeThreadId,
    owner: Option<(u32, RuntimeWindowId)>,
    dialog: RuntimeMessageBox,
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
        return Err(RuntimeStateError::new(
            "guest thread reserve and commit must be page-aligned, nonzero x86 sizes with commit no larger than reserve",
        ));
    }
    if reserve_bytes > commit_bytes && reserve_bytes - commit_bytes < GUEST_PAGE_SIZE {
        return Err(RuntimeStateError::new(
            "a growable guest thread stack requires one page between its commit and reserve limits",
        ));
    }
    Ok(())
}

fn pointer_input_coordinates(
    normalized_x: u16,
    normalized_y: u16,
    geometry: RuntimeWindowGeometry,
) -> Result<(i32, i32, i32, i32, u32), RuntimeStateError> {
    if geometry.client_width == 0 || geometry.client_height == 0 {
        return Err(RuntimeStateError::new(
            "pointer input requires bounded nonempty client geometry",
        ));
    }
    let denominator = 65_536_u64;
    let client_x = i32::try_from(
        u64::from(normalized_x)
            .checked_mul(u64::from(geometry.client_width))
            .ok_or_else(|| {
                RuntimeStateError::new(
                    "normalized pointer x position overflowed logical client accounting",
                )
            })?
            / denominator,
    )
    .map_err(|_| {
        RuntimeStateError::new("logical pointer x position does not fit x86 client coordinates")
    })?;
    let client_y = i32::try_from(
        u64::from(normalized_y)
            .checked_mul(u64::from(geometry.client_height))
            .ok_or_else(|| {
                RuntimeStateError::new(
                    "normalized pointer y position overflowed logical client accounting",
                )
            })?
            / denominator,
    )
    .map_err(|_| {
        RuntimeStateError::new("logical pointer y position does not fit x86 client coordinates")
    })?;
    let client_x_i16 = i16::try_from(client_x).map_err(|_| {
        RuntimeStateError::new("logical pointer x position is outside the signed x86 LPARAM range")
    })?;
    let client_y_i16 = i16::try_from(client_y).map_err(|_| {
        RuntimeStateError::new("logical pointer y position is outside the signed x86 LPARAM range")
    })?;
    let screen_x = geometry.client_x.checked_add(client_x).ok_or_else(|| {
        RuntimeStateError::new("logical pointer screen x position overflowed i32 accounting")
    })?;
    let screen_y = geometry.client_y.checked_add(client_y).ok_or_else(|| {
        RuntimeStateError::new("logical pointer screen y position overflowed i32 accounting")
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
        return Err(RuntimeStateError::new(format!(
            "callback scratch requires {byte_length} bytes; slot capacity is {CALLBACK_SCRATCH_SLOT_SIZE}"
        )));
    }
    let slot = depth.checked_sub(1).ok_or_else(|| {
        RuntimeStateError::new("live callback scratch cannot be assigned to depth zero")
    })?;
    let slot_count = usize::try_from(CALLBACK_SCRATCH_ARENA_SIZE)
        .ok()
        .and_then(|bytes| bytes.checked_div(CALLBACK_SCRATCH_SLOT_SIZE as usize))
        .ok_or_else(|| {
            RuntimeStateError::new("callback scratch arena does not contain a complete slot")
        })?;
    if slot >= slot_count {
        return Err(RuntimeStateError::new(format!(
            "callback scratch depth {depth} exceeds {slot_count} arena slots"
        )));
    }
    let offset = u32::try_from(slot)
        .ok()
        .and_then(|slot| slot.checked_mul(CALLBACK_SCRATCH_SLOT_SIZE))
        .ok_or_else(|| {
            RuntimeStateError::new(
                "validated callback scratch slot offset overflowed x86 accounting",
            )
        })?;
    let address = CALLBACK_SCRATCH_ARENA_BASE
        .0
        .checked_add(offset)
        .map(GuestAddress)
        .ok_or_else(|| {
            RuntimeStateError::new(
                "validated callback scratch slot address overflowed x86 accounting",
            )
        })?;
    Ok(Some(RuntimeCallbackScratch {
        slot: u16::try_from(slot).map_err(|_| {
            RuntimeStateError::new("validated callback scratch slot does not fit its identity type")
        })?,
        address,
        byte_length,
        capacity: CALLBACK_SCRATCH_SLOT_SIZE,
    }))
}

#[derive(Debug)]
struct PendingOpfsWrite {
    truncate: bool,
    file_id: super::RuntimeSourceFileId,
    cursor: u64,
    byte_length: u64,
    transferred: u32,
    count: GuestAddress,
    relative_path: String,
}

#[derive(Debug)]
struct PendingOpfsOpen {
    prepared: RuntimeSourceFileOpen,
    source_fallback: Option<RuntimeSourceFileOpen>,
    object: RuntimeObjectRegistrationPlan,
    relative_path: String,
    disposition: u32,
    source_missing: u8,
    source_parents: u8,
    inheritable: bool,
}

#[derive(Debug)]
pub(crate) enum RuntimeFindFirstFileOutcome {
    Found(RuntimeHandle),
    NotFound(u32),
}

#[derive(Debug)]
pub struct Runtime {
    pub(crate) memory: GuestMemory,
    process: RuntimeProcessState,
    pub(crate) address_space_allocator: ProcessAddressSpaceAllocator,
    threads: RuntimeThreads,
    image_tls: RuntimeImageTls,
    pub(crate) pending_api_callbacks: BTreeMap<RuntimeCallbackFrameId, ApiGuestCallbackCall>,
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
    pub(crate) direct_show: crate::directshow::RuntimeDirectShow,
    pub(crate) winmm: crate::winmm::RuntimeWinMm,
    pub(crate) message_waits: BTreeMap<u64, crate::user32_wait::MessageWait>,
    heaps: RuntimeHeaps,
    files: RuntimeFiles,
    pending_opfs_opens: BTreeMap<u64, PendingOpfsOpen>,
    pending_opfs_writes: BTreeMap<u64, PendingOpfsWrite>,
    pending_current_directories: BTreeMap<u64, Vec<u8>>,
    pending_message_boxes: BTreeMap<u64, PendingMessageBox>,
    pending_shell_folders: BTreeMap<u64, (GuestAddress, Vec<MemoryRegionInfo>)>,
    guid_seed: Option<([u8; 32], u64)>,
    synchronization: RuntimeSynchronization,
    pub(crate) modules: RuntimeModules,
    registries: RuntimeRegistries,
    pub(crate) registry: super::registry::RuntimeRegistry,
    pub(crate) global_atoms: super::RuntimeGlobalAtoms,
    pub(crate) text_layouts: BTreeMap<u64, crate::text_layout::PendingTextLayout>,
}

impl Runtime {
    pub(crate) fn is_gdi_stock_brush(&self, handle: u32) -> Result<bool, RuntimeStateError> {
        Ok(self.gdi.object_option(handle)?.is_some_and(|object| {
            matches!(
                object.object,
                super::gdi::RuntimeGdiObjectKind::StockBrush { .. }
            )
        }))
    }
    pub(crate) fn process_exit_in_progress(&self) -> bool {
        self.process.exit_owner.is_some()
    }
    pub(crate) fn begin_process_exit(&mut self) {
        self.process.exit_owner = Some(self.current_thread_id());
    }

    pub(crate) fn top_level_exception_filter(&self) -> Option<GuestAddress> {
        self.process.unhandled_exception_filter()
    }
    pub(crate) fn system_power_setting(&mut self, index: usize, value: Option<bool>) -> bool {
        let previous = self.user32.power_settings[index];
        if let Some(value) = value {
            self.user32.power_settings[index] = value;
        }
        previous
    }
    /// The browser supplies process entropy; GUID bytes and sequencing stay in Rust.
    pub fn configure_guid_seed(&mut self, seed: [u8; 32]) -> Result<(), RuntimeStateError> {
        if self.guid_seed.is_some() {
            return Err(RuntimeStateError::new(
                "process GUID entropy is already configured",
            ));
        }
        self.guid_seed = Some((seed, 0));
        Ok(())
    }

    pub(crate) fn create_guid(&mut self) -> Result<[u8; 16], RuntimeStateError> {
        use sha2::{Digest, Sha256};
        let (seed, counter) = self.guid_seed.as_mut().ok_or_else(|| {
            RuntimeStateError::new("GUID creation requires host-supplied process entropy")
        })?;
        let next = counter
            .checked_add(1)
            .ok_or_else(|| RuntimeStateError::new("GUID counter exhausted"))?;
        let mut hash = Sha256::new();
        hash.update(b"Vine/CoCreateGuid/v1\0");
        hash.update(seed);
        hash.update(counter.to_le_bytes());
        let mut bytes: [u8; 16] = hash.finalize()[..16]
            .try_into()
            .expect("SHA-256 has 16 bytes");
        // GUID stores Data3 little-endian; RFC 4122 version/variant bits.
        bytes[7] = (bytes[7] & 0x0f) | 0x40;
        bytes[8] = (bytes[8] & 0x3f) | 0x80;
        *counter = next;
        Ok(bytes)
    }
    pub fn new(
        bootstrap: RuntimeBootstrap,
        memory: GuestMemory,
    ) -> Result<Self, RuntimeStateError> {
        if bootstrap.process_id.0 == 0 || bootstrap.initial_thread_id.0 == 0 {
            return Err(RuntimeStateError::new(
                "runtime process and initial thread IDs must be nonzero",
            ));
        }
        if bootstrap.clock.performance_frequency == 0 {
            return Err(RuntimeStateError::new(
                "runtime performance-counter frequency must be nonzero",
            ));
        }
        if bootstrap.clock.performance_frequency > i64::MAX as u64 {
            return Err(RuntimeStateError::new(
                "runtime performance-counter frequency must fit a nonnegative LARGE_INTEGER",
            ));
        }
        if bootstrap.clock.performance_counter > i64::MAX as u64 {
            return Err(RuntimeStateError::new(
                "runtime performance counter must fit a nonnegative LARGE_INTEGER",
            ));
        }
        if bootstrap.startup_info.bytes.len() != X86_STARTUP_INFO_W_BYTE_LENGTH {
            return Err(RuntimeStateError::new(
                "runtime startup requires one x86 STARTUPINFOW image",
            ));
        }
        if bootstrap.process_image_path_a.bytes_with_nul.len() < 2
            || bootstrap.process_image_path_a.bytes_with_nul.len()
                > RUNTIME_PROCESS_IMAGE_PATH_A_MAX_BYTES
            || bootstrap.process_image_path_a.bytes_with_nul.last() != Some(&0)
            || bootstrap.process_image_path_a.bytes_with_nul
                [..bootstrap.process_image_path_a.bytes_with_nul.len() - 1]
                .contains(&0)
        {
            return Err(RuntimeStateError::new(
                "runtime process image path requires bounded Windows-932 bytes with exactly one trailing NUL",
            ));
        }
        let address_space_allocator = ProcessAddressSpaceAllocator::for_memory(&memory)?;
        let process_directories = RuntimeProcessDirectories::new(
            bootstrap.initial_current_directory_a.clone(),
            &bootstrap.process_image_path_a,
        )?;
        let registries = RuntimeRegistries::new(bootstrap.encoded_pointer_cookie)?;

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
            memory,
            process: RuntimeProcessState::new(
                bootstrap.process_id,
                bootstrap.clock,
                bootstrap.startup_info,
                bootstrap.process_image_path_a,
                process_directories,
            ),
            address_space_allocator,
            threads,
            pending_api_callbacks: BTreeMap::new(),
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
            direct_show: crate::directshow::RuntimeDirectShow::default(),
            winmm: crate::winmm::RuntimeWinMm::default(),
            message_waits: BTreeMap::new(),
            heaps: RuntimeHeaps::default(),
            files: RuntimeFiles::default(),
            pending_opfs_opens: BTreeMap::new(),
            pending_opfs_writes: BTreeMap::new(),
            pending_current_directories: BTreeMap::new(),
            pending_message_boxes: BTreeMap::new(),
            pending_shell_folders: BTreeMap::new(),
            guid_seed: None,
            image_tls: RuntimeImageTls::default(),
            synchronization: RuntimeSynchronization::default(),
            modules: RuntimeModules::default(),
            registries,
            registry: super::registry::RuntimeRegistry::default(),
            global_atoms: super::RuntimeGlobalAtoms::default(),
            text_layouts: BTreeMap::new(),
        })
    }

    pub fn memory(&self) -> &GuestMemory {
        &self.memory
    }

    pub fn memory_mut(&mut self) -> &mut GuestMemory {
        &mut self.memory
    }

    pub(crate) fn add_ref_interface_with_guest_pointer_write(
        &mut self,
        interface_id: RuntimeComInterfaceId,
        output_address: GuestAddress,
    ) -> Result<u32, RuntimeStateError> {
        self.memory
            .validate_access(output_address, 4, MemoryAccess::Write)
            .map_err(RuntimeStateError::from_memory)?;
        let pointer = self.com.interface(interface_id)?.guest_pointer;
        let bytes = pointer.0.to_le_bytes().to_vec();
        let mut transaction = self.memory.begin_transaction();
        transaction
            .write(output_address, &bytes)
            .map_err(RuntimeStateError::from_memory)?;
        let reference_count = self.com.add_ref(interface_id)?;
        transaction.commit_transaction();
        Ok(reference_count)
    }

    pub(crate) fn acquire_graphics_surface_dc_with_guest_handle_write(
        &mut self,
        surface_com_object_id: RuntimeComObjectId,
        output_address: GuestAddress,
    ) -> Result<Option<()>, RuntimeStateError> {
        self.memory
            .validate_access(output_address, 4, MemoryAccess::Write)
            .map_err(RuntimeStateError::from_memory)?;
        let Some(preview) = self.preview_graphics_surface_dc_acquisition(surface_com_object_id)?
        else {
            return Ok(None);
        };
        let target = self.resolve_gdi_graphics_surface_target(surface_com_object_id)?;
        let bytes = preview.to_le_bytes().to_vec();
        let mut transaction = self.memory.begin_transaction();
        transaction
            .write(output_address, &bytes)
            .map_err(RuntimeStateError::from_memory)?;
        // There is no host/guest handoff here. If an owner check fails, the
        // guest write transaction rolls back before the HDC is published.
        let result = self.gdi.get_graphics_surface_dc(target)?.ok_or_else(|| {
            RuntimeStateError::new(
                "validated surface DC became occupied during synchronous guest output write",
            )
        })?;
        debug_assert_eq!(result, preview);
        transaction.commit_transaction();
        Ok(Some(()))
    }

    pub(crate) fn complete_api_immediate(
        &mut self,
        argument_words: u8,
        frame: &crate::api::ApiCallFrame,
        immediate: crate::api::ApiImmediate,
    ) -> Result<crate::api::ApiImmediateCompletion, crate::api::ApiFault> {
        // Select and validate the completion thread before borrowing its LastError
        // together with the owned memory. None preserves that live value.
        let last_error = self
            .threads
            .last_error_mut(self.scheduler.current_thread_id())
            .map_err(|error| crate::api::ApiFault::from_runtime(error, &frame.endpoint))?;
        let completion = crate::api::complete_immediate(
            &mut self.memory,
            last_error,
            argument_words,
            frame,
            immediate,
        )?;
        if completion.execution_boundary == RuntimeApiBoundaryDisposition::TerminateProcess {
            self.process
                .terminate(completion.eax.expect("process exit code"));
        }
        Ok(completion)
    }

    pub(crate) fn process_id(&self) -> RuntimeProcessId {
        self.process.id()
    }

    pub fn configure_display_profile(
        &mut self,
        profile: RuntimeDisplayProfile,
    ) -> Result<(), RuntimeStateError> {
        self.displays.configure(profile)
    }

    pub(crate) fn primary_monitor(&self) -> Option<&RuntimeLogicalMonitor> {
        self.displays.primary_monitor()
    }

    pub(crate) fn monitor(&self, handle: u32) -> Option<&RuntimeLogicalMonitor> {
        self.displays.monitor(handle)
    }

    pub fn configure_input_profile(
        &mut self,
        profile: RuntimeInputProfile,
    ) -> Result<(), RuntimeStateError> {
        self.input.configure(profile)
    }

    pub(crate) fn swap_mouse_buttons(&mut self, swapped: bool) -> Result<bool, RuntimeStateError> {
        self.input.swap_mouse_buttons(swapped)
    }

    pub(crate) fn mouse_buttons_swapped(&self) -> Result<bool, RuntimeStateError> {
        self.input.mouse_buttons_swapped()
    }

    pub(crate) fn keyboard_state(&self) -> Option<&RuntimeKeyboardState> {
        self.input.keyboard_state()
    }
    pub(crate) fn direct_input_keys(&self) -> Option<[u8; 256]> {
        self.input.direct_input_keys()
    }
    pub(crate) fn direct_input_device(
        &self,
        object: RuntimeComObjectId,
    ) -> Result<&crate::dinput8::KeyboardDevice, RuntimeStateError> {
        self.input
            .direct_input
            .get(&object)
            .ok_or_else(|| RuntimeStateError::new("DirectInput device is not live"))
    }
    pub(crate) fn direct_input_device_mut(
        &mut self,
        object: RuntimeComObjectId,
    ) -> Result<&mut crate::dinput8::KeyboardDevice, RuntimeStateError> {
        self.input
            .direct_input
            .get_mut(&object)
            .ok_or_else(|| RuntimeStateError::new("DirectInput device is not live"))
    }
    pub(crate) fn insert_direct_input_device(&mut self, object: RuntimeComObjectId) {
        self.input
            .direct_input
            .insert(object, crate::dinput8::KeyboardDevice::default());
    }

    pub(crate) fn cursor_position(&self) -> Option<&RuntimeCursorPosition> {
        self.input.cursor_position()
    }

    pub(crate) fn set_cursor_position(
        &mut self,
        screen_x: i32,
        screen_y: i32,
    ) -> Result<(), RuntimeStateError> {
        self.input.set_cursor_position(screen_x, screen_y)
    }

    pub fn ingest_keyboard_event(
        &mut self,
        event: RuntimeKeyboardSourceEvent,
    ) -> Result<(), RuntimeStateError> {
        if !self.process.is_running() {
            return Err(RuntimeStateError::new(
                "keyboard ingress requires a running process",
            ));
        }
        let message = self.input.keyboard_message(event)?;
        let Some((message, w_param, l_param)) = message else {
            self.input.publish_keyboard_event(event);
            if event.kind == RuntimeKeyboardSourceEventKind::FocusLost {
                self.messages.release_keyboard_source();
            }
            return Ok(());
        };
        // Asynchronous key state is meaningful before a guest window exists.
        // Window messages are routed only after Windows has a focused target.
        if self.user32.foreground_window_handle()?.is_none() {
            self.input.publish_keyboard_event(event);
            return Ok(());
        }
        let (window, target_thread_id, target_scheduler_state, _) =
            self.window_input_target(false)?;
        let cursor = self.input.cursor_position().ok_or_else(|| {
            RuntimeStateError::new("keyboard messages require a configured logical cursor position")
        })?;
        let posted = self.messages.checked_post(
            target_thread_id,
            window.handle,
            message,
            w_param,
            l_param,
            self.process.clock().tick_count,
            cursor.screen_x,
            cursor.screen_y,
            target_scheduler_state == SchedulerThreadState::WaitingMessage,
        )?;
        let resumed_state = (target_scheduler_state == SchedulerThreadState::WaitingMessage)
            .then(|| self.resumed_execution_state_for_thread(target_thread_id))
            .transpose()?;
        self.input.publish_keyboard_event(event);
        self.messages.push_input(posted);
        if let Some(resumed_state) = resumed_state {
            self.observe_thread_message_state(target_thread_id);
            self.scheduler
                .set_validated_state(target_thread_id, resumed_state);
            self.scheduler
                .commit_event(RuntimeSchedulerEvent::MessageReady(target_thread_id));
        }
        Ok(())
    }

    pub fn skip_unroutable_pointer_event(
        &mut self,
        sequence: u64,
    ) -> Result<(), RuntimeStateError> {
        if !self.process.is_running() || self.user32.foreground_window_handle()?.is_some() {
            return Err(RuntimeStateError::new(
                "only an event with no foreground guest window can be ignored",
            ));
        }
        self.input.skip_unroutable_pointer_event(sequence)
    }

    pub fn ingest_pointer_button_event(
        &mut self,
        event: RuntimePointerButtonSourceEvent,
    ) -> Result<RuntimeThreadMessage, RuntimeStateError> {
        let (window, target_thread_id, target_scheduler_state, geometry) =
            self.window_input_target(true)?;
        let window_handle = window.handle;
        let (_, _, screen_x, screen_y, l_param) =
            pointer_input_coordinates(event.normalized_x, event.normalized_y, geometry)?;
        let window_class = self.user32.window_class(window.creation.class.atom)?;
        if window_class.id != window.creation.class.class_id {
            return Err(RuntimeStateError::new(
                "logical pointer target does not match its registered window class identity",
            ));
        }
        let input_update = self.input.preview_pointer_button_event(
            event,
            screen_x,
            screen_y,
            RuntimePointerButtonEventTarget {
                window_id: window.id.0,
                window_handle,
                accepts_double_clicks: window_class.registration.style
                    & RUNTIME_WINDOW_CLASS_STYLE_DOUBLE_CLICKS
                    != 0,
                tick_count: self.process.clock().tick_count,
            },
        )?;
        let (message, w_param) = input_update.message_and_key_state();
        let posted = self.messages.checked_post(
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
        let resumed_state = (target_scheduler_state == SchedulerThreadState::WaitingMessage)
            .then(|| self.resumed_execution_state_for_thread(target_thread_id))
            .transpose()?;

        self.input.apply_pointer_button_event(input_update);
        self.input.publish_pointer_enter(posted.hwnd);
        self.messages.push_input(posted);
        if let Some(resumed_state) = resumed_state {
            self.observe_thread_message_state(target_thread_id);
            self.scheduler
                .set_validated_state(target_thread_id, resumed_state);
            self.scheduler
                .commit_event(RuntimeSchedulerEvent::MessageReady(target_thread_id));
        }
        Ok(posted)
    }

    pub(crate) fn track_mouse_leave(
        &mut self,
        handle: u32,
        cancel: bool,
    ) -> Result<bool, RuntimeStateError> {
        let Some(window) = self.window_option(handle)? else {
            return Ok(false);
        };
        if window.lifecycle != RuntimeWindowLifecycle::Created {
            return Ok(false);
        }
        let id = window.id;
        let thread = window.creation.owner_thread_id;
        if thread != self.current_thread_id() {
            return Err(RuntimeStateError::new(
                "mouse tracking requires the window owner thread",
            ));
        }
        let inside = self.input.pointer_window() == Some(handle);
        if !cancel && !inside {
            self.post_thread_message(thread, handle, 0x02a3, 0, 0)?;
        }
        self.user32.set_mouse_leave_tracking(id, !cancel && inside);
        Ok(true)
    }

    pub fn ingest_pointer_leave_event(
        &mut self,
        sequence: u64,
    ) -> Result<Option<RuntimeThreadMessage>, RuntimeStateError> {
        if !self.process.is_running() {
            return Err(RuntimeStateError::new(
                "pointer leave requires a running process",
            ));
        }
        self.input.pointer_position_key_state(sequence)?;
        let target = self
            .input
            .pointer_window()
            .map(|handle| self.window_option(handle))
            .transpose()?
            .flatten()
            .filter(|window| {
                window.mouse_leave_tracking && window.lifecycle == RuntimeWindowLifecycle::Created
            })
            .map(|window| (window.id, window.handle, window.creation.owner_thread_id));
        let posted = if let Some((id, handle, thread)) = target {
            let posted = self.enqueue_thread_message(thread, handle, 0x02a3, 0, 0)?;
            self.user32.set_mouse_leave_tracking(id, false);
            Some(posted)
        } else {
            None
        };
        self.input.publish_pointer_leave(sequence);
        Ok(posted)
    }

    pub fn ingest_pointer_position_event(
        &mut self,
        event: RuntimePointerPositionSourceEvent,
    ) -> Result<RuntimeThreadMessage, RuntimeStateError> {
        let (window, target_thread_id, target_scheduler_state, geometry) =
            self.window_input_target(true)?;
        let (_, _, screen_x, screen_y, l_param) =
            pointer_input_coordinates(event.normalized_x, event.normalized_y, geometry)?;
        let key_state = self.input.pointer_position_key_state(event.sequence)?;
        let posted = self.messages.checked_post(
            target_thread_id,
            window.handle,
            WM_MOUSEMOVE,
            key_state,
            l_param,
            self.process.clock().tick_count,
            screen_x,
            screen_y,
            target_scheduler_state == SchedulerThreadState::WaitingMessage,
        )?;
        let resumed_state = (target_scheduler_state == SchedulerThreadState::WaitingMessage)
            .then(|| self.resumed_execution_state_for_thread(target_thread_id))
            .transpose()?;

        self.input
            .publish_pointer_position(event.sequence, screen_x, screen_y);
        self.input.publish_pointer_enter(posted.hwnd);
        self.messages.push_input(posted);
        if let Some(resumed_state) = resumed_state {
            self.observe_thread_message_state(target_thread_id);
            self.scheduler
                .set_validated_state(target_thread_id, resumed_state);
            self.scheduler
                .commit_event(RuntimeSchedulerEvent::MessageReady(target_thread_id));
        }
        Ok(posted)
    }

    fn window_input_target(
        &self,
        pointer: bool,
    ) -> Result<
        (
            &RuntimeWindow,
            RuntimeThreadId,
            SchedulerThreadState,
            RuntimeWindowGeometry,
        ),
        RuntimeStateError,
    > {
        if !self.process.is_running() {
            return Err(RuntimeStateError::new(
                "window input requires a running process",
            ));
        }
        let window_handle = self.user32.foreground_window_handle()?.ok_or_else(|| {
            RuntimeStateError::new("window input requires a live logical foreground window")
        })?;
        if pointer {
            self.user32.validate_pointer_capture_target(window_handle)?;
        }
        let window = self.user32.window(window_handle)?;
        let target_thread_id = window.creation.owner_thread_id;
        if self
            .pending_message_boxes
            .values()
            .any(|pending| pending.owner.is_some_and(|(_, id)| id == window.id))
        {
            return Err(RuntimeStateError::new(
                "modal MessageBox blocks input to this window",
            ));
        }
        if window.lifecycle != RuntimeWindowLifecycle::Created
            || !window.visible
            || self.user32.active_window_handle(target_thread_id)? != Some(window_handle)
            || self.user32.focused_window_handle(target_thread_id)? != Some(window_handle)
        {
            return Err(RuntimeStateError::new(
                "window input requires one visible active focused foreground logical window",
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
                    | SchedulerThreadState::Suspended { .. }
                    | SchedulerThreadState::WaitingHost { .. }
                    | SchedulerThreadState::WaitingObject { .. }
                    | SchedulerThreadState::WaitingCriticalSection { .. }
                    | SchedulerThreadState::WaitingMessage
                    | SchedulerThreadState::InCallback { .. }
            )
        {
            return Err(RuntimeStateError::new(
                "window input target requires matching live thread and scheduler states",
            ));
        }
        let geometry = window.creation.geometry;
        Ok((window, target_thread_id, target_scheduler_state, geometry))
    }

    pub(crate) fn query_key_state(&self, key: u8) -> Result<i16, RuntimeStateError> {
        self.messages.key_state(self.current_thread_id(), key)
    }

    pub(crate) fn queue_keyboard_state(&self) -> Result<[u8; 256], RuntimeStateError> {
        self.messages.keyboard_state(self.current_thread_id())
    }

    pub(crate) fn set_queue_keyboard_state(
        &mut self,
        state: [u8; 256],
    ) -> Result<(), RuntimeStateError> {
        self.messages
            .set_keyboard_state(self.current_thread_id(), state)
    }

    pub(crate) fn query_async_key_state(
        &mut self,
        virtual_key: u32,
    ) -> Result<Option<i16>, RuntimeStateError> {
        self.input.query_async_key_state(virtual_key)
    }

    pub(crate) fn translate_keyboard_message(
        &mut self,
        message: RuntimeThreadMessage,
    ) -> Result<(), RuntimeStateError> {
        self.resolve_window_procedure(message.hwnd)?;
        if message.target_thread_id != self.current_thread_id() {
            return Err(RuntimeStateError::new(
                "keyboard translation must use the calling thread queue",
            ));
        }
        if !matches!(message.message, 0x0100 | 0x0104) {
            return Ok(());
        }
        let Some(character) = self
            .messages
            .keyboard_character(self.current_thread_id(), message.w_param)?
        else {
            return Ok(());
        };
        let posted = self.messages.checked_post(
            self.current_thread_id(),
            message.hwnd,
            if message.message == 0x0104 {
                0x0106
            } else {
                0x0102
            },
            u32::from(character),
            message.l_param,
            message.time,
            message.point_x,
            message.point_y,
            false,
        )?;
        self.messages.push_character(posted);
        Ok(())
    }

    pub(crate) fn begin_timer_period(
        &mut self,
        period_milliseconds: u32,
    ) -> Result<u32, RuntimeStateError> {
        self.require_timer_operation_state()?;
        Ok(self.timers.begin_period(period_milliseconds))
    }

    pub(crate) fn end_timer_period(
        &mut self,
        period_milliseconds: u32,
    ) -> Result<u32, RuntimeStateError> {
        self.require_timer_operation_state()?;
        Ok(self.timers.end_period(period_milliseconds))
    }

    pub(crate) fn timer_time_100ns(&self) -> u64 {
        self.process.timer_time_100ns()
    }

    pub fn synchronized_instruction_count(&self) -> u64 {
        self.process.synchronized_instruction_count()
    }

    fn require_timer_operation_state(&self) -> Result<(), RuntimeStateError> {
        if !self.process.is_running()
            || self.current_thread()?.lifecycle != RuntimeThreadLifecycleState::Running
            || !matches!(
                self.scheduler_state()?,
                SchedulerThreadState::Runnable | SchedulerThreadState::InCallback { .. }
            )
        {
            return Err(RuntimeStateError::new(
                "timer resolution requests require a running process and active current thread",
            ));
        }
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn process(&self) -> &RuntimeProcessState {
        &self.process
    }

    pub(crate) fn process_image_path_a(&self) -> &RuntimeProcessImagePathA {
        self.process.image_path_a()
    }

    pub(crate) fn current_directory_a(&self) -> &RuntimeCurrentDirectoryA {
        self.process.current_directory_a()
    }

    pub(crate) fn is_process_image_drive_root_a(&self, requested: Option<&[u8]>) -> bool {
        self.process.is_process_image_drive_root_a(requested)
    }

    pub(crate) fn set_current_directory_a(
        &mut self,
        requested: &[u8],
    ) -> Result<(), RuntimeStateError> {
        self.process.set_current_directory_a(requested)
    }

    pub(crate) fn set_resolved_current_directory_a(
        &mut self,
        resolved_bytes_with_nul: &[u8],
    ) -> Result<(), RuntimeStateError> {
        self.process
            .set_resolved_current_directory_a(resolved_bytes_with_nul)
    }

    pub(crate) fn record_pending_current_directory(&mut self, request_id: u64, target: Vec<u8>) {
        self.pending_current_directories.insert(request_id, target);
    }

    pub fn commit_pending_current_directory(
        &mut self,
        request_id: u64,
        succeeded: bool,
    ) -> Result<(), RuntimeStateError> {
        if let Some(target) = self.pending_current_directories.remove(&request_id)
            && succeeded
        {
            self.process.set_resolved_current_directory_a(&target)?;
        }
        Ok(())
    }

    pub fn register_source_snapshot(
        &mut self,
        manifest: RuntimeSourceSnapshotManifest,
    ) -> Result<(), RuntimeStateError> {
        self.files.register_snapshot(manifest)
    }

    pub(crate) fn source_root_path_a(&self) -> Result<Vec<u8>, RuntimeStateError> {
        self.files.source_root_path()
    }

    pub(crate) fn is_process_image_path_a(&self, path: &[u8]) -> Result<bool, RuntimeStateError> {
        self.files.same_path(
            path,
            &self.process_image_path_a().bytes_with_nul,
            &self.process.current_directory_a().bytes_with_nul,
        )
    }

    pub(crate) fn full_path_a(&self, path: &[u8]) -> Result<Vec<u8>, RuntimeStateError> {
        self.files
            .full_path(path, &self.process.current_directory_a().bytes_with_nul)
    }

    #[cfg(test)]
    pub(crate) fn source_snapshot(&self) -> Option<&super::files::RuntimeSourceSnapshot> {
        self.files.snapshot()
    }

    pub(crate) fn next_source_file_object_id(&self) -> Result<u32, RuntimeStateError> {
        let snapshot = self
            .files
            .snapshot()
            .ok_or_else(|| RuntimeStateError::new("source mount is missing"))?;
        snapshot
            .files
            .keys()
            .next_back()
            .copied()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or_else(|| RuntimeStateError::new("source file identities exhausted"))
    }

    pub fn register_source_directory(
        &mut self,
        relative_path_a: &[u8],
    ) -> Result<(), RuntimeStateError> {
        self.require_source_file_operation_state()?;
        self.files.register_source_directory(relative_path_a)
    }

    pub fn register_source_file(
        &mut self,
        file: super::RuntimeSourceFileManifest,
    ) -> Result<(), RuntimeStateError> {
        self.require_source_file_operation_state()?;
        self.files.register_source_file(file)
    }

    pub(crate) fn source_library_relative_path(
        &self,
        requested: &[u8],
    ) -> Result<Vec<u8>, RuntimeStateError> {
        self.require_source_file_operation_state()?;
        self.files.source_library_relative_path(
            requested,
            &self.process.current_directory_a().bytes_with_nul,
        )
    }

    pub(crate) fn source_lookup_relative_path(
        &self,
        requested_path: &[u8],
    ) -> Result<Option<Vec<u8>>, RuntimeStateError> {
        self.require_source_file_operation_state()?;
        self.files.source_lookup_relative_path(
            requested_path,
            &self.process.current_directory_a().bytes_with_nul,
        )
    }

    pub(crate) fn opfs_relative_path(&self, requested_path: &[u8]) -> Result<String, u32> {
        // No host operation may outlive a non-running process.
        if !self.process.is_running() {
            return Err(5);
        }
        self.files.opfs_relative_path(
            requested_path,
            &self.process.current_directory_a().bytes_with_nul,
        )
    }

    pub(crate) fn opfs_create_directory_path(
        &self,
        requested_path: &[u8],
    ) -> Result<(String, u8), u32> {
        if !self.process.is_running() {
            return Err(5);
        }
        self.files.opfs_create_directory_path(
            requested_path,
            &self.process.current_directory_a().bytes_with_nul,
        )
    }

    pub(crate) fn opfs_delete_path(&self, requested_path: &[u8]) -> Result<(String, u8), u32> {
        if !self.process.is_running() {
            return Err(5);
        }
        let (path, missing) = self.files.opfs_delete_path(
            requested_path,
            &self.process.current_directory_a().bytes_with_nul,
        )?;
        if self
            .pending_opfs_opens
            .values()
            .any(|pending| pending.relative_path.eq_ignore_ascii_case(&path))
        {
            return Err(32); // An in-flight creation cannot race file deletion.
        }
        Ok((path, missing))
    }

    pub(crate) fn source_search_directory(
        &self,
        requested: &[u8],
    ) -> Result<Option<Vec<u8>>, RuntimeStateError> {
        self.require_source_file_operation_state()?;
        self.files.source_search_directory(
            requested,
            &self.process.current_directory_a().bytes_with_nul,
        )
    }

    pub fn source_search_matches(
        &self,
        requested: &[u8],
        actual_segments: &[String],
        names: &[(String, bool)],
    ) -> Result<SourceSearchSelection, RuntimeStateError> {
        self.require_source_file_operation_state()?;
        self.files.source_search_matches(
            requested,
            &self.process.current_directory_a().bytes_with_nul,
            actual_segments,
            names,
        )
    }

    // Validate the complete browser listing before publishing any registrations.
    pub fn register_source_search_batch(
        &mut self,
        parent: &[u8],
        entries: Vec<SourceSearchBatchEntry>,
    ) -> Result<Vec<(usize, u32)>, RuntimeStateError> {
        self.require_source_file_operation_state()?;
        self.files.register_source_search_batch(parent, entries)
    }

    pub fn register_missing_source_path(
        &mut self,
        relative_path_a: &[u8],
    ) -> Result<(), RuntimeStateError> {
        self.require_source_file_operation_state()?;
        self.files.register_missing_source_path(relative_path_a)
    }

    pub fn register_source_directory_listing(
        &mut self,
        actual_segments: &[String],
        names: &[(String, bool)],
    ) -> Result<(), RuntimeStateError> {
        self.require_source_file_operation_state()?;
        self.files
            .register_source_directory_listing(actual_segments, names)
    }

    pub fn register_opfs_snapshot(
        &mut self,
        files: &[String],
        directories: &[String],
    ) -> Result<(), RuntimeStateError> {
        self.require_source_file_operation_state()?;
        self.files.register_opfs_snapshot(files, directories)
    }

    pub fn record_opfs_file_deleted(&mut self, relative_path: &str) {
        self.files.record_opfs_file_deleted(relative_path);
    }

    pub fn record_opfs_directory_created(&mut self, relative_path: &str) {
        self.files.record_opfs_directory_created(relative_path);
    }

    pub(crate) fn try_resolve_unshadowed_opfs_stat(&self, relative_path: &str) -> Option<u32> {
        if self
            .pending_opfs_opens
            .values()
            .any(|pending| pending.relative_path.eq_ignore_ascii_case(relative_path))
            || self
                .pending_opfs_writes
                .values()
                .any(|pending| pending.relative_path.eq_ignore_ascii_case(relative_path))
        {
            return None;
        }
        self.files.opfs_unshadowed_observation(relative_path)
    }

    pub(crate) fn try_resolve_unshadowed_opfs_read_open(
        &mut self,
        requested_path: &[u8],
        desired_access: u32,
        share_mode: u32,
        disposition: u32,
        flags_and_attributes: u32,
        inheritable: bool,
    ) -> Result<Option<Result<RuntimeHandle, u32>>, RuntimeStateError> {
        if desired_access != 0x8000_0000 || disposition != 3 {
            return Ok(None);
        }
        self.require_source_file_operation_state()?;
        let (_, relative_path, source_missing, _) = match self.files.prepare_opfs_open(
            requested_path,
            &self.process.current_directory_a().bytes_with_nul,
            desired_access,
            share_mode,
            flags_and_attributes,
        ) {
            Ok(value) => value,
            Err(error) => return Ok(Some(Err(error))),
        };
        if self
            .pending_opfs_opens
            .values()
            .any(|pending| pending.relative_path.eq_ignore_ascii_case(&relative_path))
            || self
                .pending_opfs_writes
                .values()
                .any(|pending| pending.relative_path.eq_ignore_ascii_case(&relative_path))
        {
            return Ok(Some(Err(32)));
        }
        if self.files.opfs_is_known_directory(&relative_path) {
            return Ok(Some(Err(5)));
        }
        let Some(stat_obs) = self.files.opfs_unshadowed_observation(&relative_path) else {
            return Ok(None);
        };
        if source_missing == 0 {
            return match self.open_file_a(
                requested_path,
                desired_access,
                share_mode,
                3,
                flags_and_attributes,
            )? {
                Ok(Some(handle)) => {
                    self.set_new_file_handle_inheritable(handle, inheritable);
                    Ok(Some(Ok(handle)))
                }
                Ok(None) => Err(RuntimeStateError::new(
                    "source fallback no longer matches the checked file open",
                )),
                Err(error) => Ok(Some(Err(error))),
            };
        }
        let error = if stat_obs == 3 {
            2
        } else {
            u32::from(source_missing)
        };
        Ok(Some(Err(error)))
    }

    pub(crate) fn resolve_file_system_path_a(
        &self,
        requested_path: &[u8],
    ) -> Result<super::RuntimePathResolution, RuntimeStateError> {
        self.require_source_file_operation_state()?;
        self.files.resolve_path_kind(
            requested_path,
            &self.process.current_directory_a().bytes_with_nul,
        )
    }

    pub(crate) fn open_file_a(
        &mut self,
        requested_path: &[u8],
        desired_access: u32,
        share_mode: u32,
        creation_disposition: u32,
        flags_and_attributes: u32,
    ) -> Result<Result<Option<RuntimeHandle>, u32>, RuntimeStateError> {
        self.require_source_file_operation_state()?;
        let prepared = match self.files.prepare_open_with_disposition(
            requested_path,
            &self.process.current_directory_a().bytes_with_nul,
            desired_access,
            share_mode,
            creation_disposition,
            flags_and_attributes,
        )? {
            SourceOpenOutcome::Opened(prepared) => prepared,
            SourceOpenOutcome::Missing => return Ok(Ok(None)),
            SourceOpenOutcome::SharingViolation => return Ok(Err(32)), // ERROR_SHARING_VIOLATION
        };
        let key = RuntimeObjectKey::File {
            file_id: prepared.file_id().0,
        };
        let (_, handle) = self.objects.register_object_and_open_handle(key)?;
        self.files.open_validated(prepared);
        Ok(Ok(Some(handle)))
    }

    pub(crate) fn set_new_file_handle_inheritable(
        &mut self,
        handle: RuntimeHandle,
        inheritable: bool,
    ) {
        self.objects.set_new_handle_inheritable(handle, inheritable);
    }

    pub(crate) fn begin_opfs_file_open(
        &mut self,
        requested_path: &[u8],
        desired_access: u32,
        share_mode: u32,
        disposition: u32,
        flags_and_attributes: u32,
        inheritable: bool,
    ) -> Result<Result<crate::api::HostRequest, u32>, RuntimeStateError> {
        self.require_source_file_operation_state()?;
        let (prepared, relative_path, source_missing, source_parents) =
            match self.files.prepare_opfs_open(
                requested_path,
                &self.process.current_directory_a().bytes_with_nul,
                desired_access,
                share_mode,
                flags_and_attributes,
            ) {
                Ok(value) => value,
                Err(error) => return Ok(Err(error)),
            };
        if self
            .pending_opfs_opens
            .values()
            .any(|pending| pending.relative_path.eq_ignore_ascii_case(&relative_path))
            || self
                .pending_opfs_writes
                .values()
                .any(|pending| pending.relative_path.eq_ignore_ascii_case(&relative_path))
        {
            return Ok(Err(32)); // No competing open may race durable OPFS work.
        }
        let object = self
            .objects
            .plan_object_and_handle_registration(RuntimeObjectKey::File {
                file_id: prepared.file_id().0,
            })?;
        let source_fallback =
            if source_missing == 0 && desired_access == 0x8000_0000 && disposition == 3 {
                match self.files.prepare_open_with_disposition(
                    requested_path,
                    &self.process.current_directory_a().bytes_with_nul,
                    desired_access,
                    share_mode,
                    3,
                    flags_and_attributes,
                )? {
                    SourceOpenOutcome::Opened(open) => Some(open),
                    _ => {
                        return Err(RuntimeStateError::new(
                            "source fallback no longer matches the checked file open",
                        ));
                    }
                }
            } else {
                None
            };
        let scheduler = self
            .scheduler
            .checked_host_request("browser.opfs-open".to_owned())?;
        self.objects.preflight_object_registration_commit(&object)?;
        let payload = [disposition as u8, source_missing, source_parents]
            .into_iter()
            .chain(relative_path.bytes())
            .collect();
        let request = self.scheduler.publish_host_request(scheduler);
        let previous = self.pending_opfs_opens.insert(
            request.request_id,
            PendingOpfsOpen {
                prepared,
                source_fallback,
                object,
                relative_path,
                disposition,
                source_missing,
                source_parents,
                inheritable,
            },
        );
        debug_assert!(previous.is_none());
        Ok(Ok(crate::api::HostRequest {
            request_id: request.request_id,
            service: request.service,
            payload,
        }))
    }

    pub(crate) fn preflight_opfs_file_open_completion(
        &self,
        request_id: u64,
        payload: &[u8],
    ) -> Result<(RuntimeHandle, u32, u8, bool), RuntimeStateError> {
        let pending = self
            .pending_opfs_opens
            .get(&request_id)
            .ok_or_else(|| RuntimeStateError::new("OPFS open has no pending file identity"))?;
        if payload.get(..3)
            != Some(
                &[
                    pending.disposition as u8,
                    pending.source_missing,
                    pending.source_parents,
                ][..],
            )
            || payload.get(3..) != Some(pending.relative_path.as_bytes())
        {
            return Err(RuntimeStateError::new(
                "OPFS open payload differs from Rust file state",
            ));
        }
        let scheduler = self.scheduler.validate_host_completion(request_id)?;
        if scheduler.service != "browser.opfs-open" {
            return Err(RuntimeStateError::new(
                "pending host service is not OPFS open",
            ));
        }
        self.files.preflight_opfs_open(
            &pending.prepared,
            &self.process.current_directory_a().bytes_with_nul,
        )?;
        self.objects
            .preflight_object_registration_commit(&pending.object)?;
        Ok((
            pending.object.handle(),
            pending.disposition,
            pending.source_missing,
            pending.source_fallback.is_some(),
        ))
    }

    // Only after the API completion has validated its suspended frame. All
    // fallible scheduler, handle and VFS checks have already run above.
    pub(crate) fn commit_opfs_file_open(
        &mut self,
        request_id: u64,
        opened: Option<u64>,
        source_fallback: bool,
    ) {
        let pending = self
            .pending_opfs_opens
            .remove(&request_id)
            .expect("preflighted OPFS open");
        let scheduler = self
            .scheduler
            .validate_host_completion(request_id)
            .expect("preflighted OPFS host completion");
        self.scheduler.commit_host_completion(scheduler);
        if let Some(byte_length) = opened {
            let (_, handle) = self.objects.commit_object_registration(pending.object);
            self.objects
                .set_new_handle_inheritable(handle, pending.inheritable);
            let prepared = if source_fallback {
                pending.source_fallback.expect("validated source fallback")
            } else {
                self.files.record_opfs_file_created(&pending.relative_path);
                pending.prepared.confirmed_opfs(byte_length)
            };
            self.files.open_validated(prepared);
        }
    }

    pub(crate) fn find_first_file(
        &mut self,
        requested_pattern: &[u8],
        output: GuestAddress,
        unicode: bool,
    ) -> Result<RuntimeFindFirstFileOutcome, RuntimeStateError> {
        self.require_source_file_operation_state()?;
        let search = match self.files.new_file_search(
            requested_pattern,
            &self.process.current_directory_a().bytes_with_nul,
        )? {
            Ok(Some(search)) => search,
            Ok(None) => return Ok(RuntimeFindFirstFileOutcome::NotFound(2)),
            Err(windows_error) => return Ok(RuntimeFindFirstFileOutcome::NotFound(windows_error)),
        };
        let bytes = search
            .entries
            .first()
            .expect("nonempty search results")
            .encode_find_data(unicode)?;
        self.memory
            .validate_access(output, bytes.len(), MemoryAccess::Write)
            .map_err(RuntimeStateError::from_memory)?;
        let key = RuntimeObjectKey::FileSearch {
            search_id: search.id.0,
        };
        let mut transaction = self.memory.begin_transaction();
        transaction
            .write(output, &bytes)
            .map_err(RuntimeStateError::from_memory)?;
        let (_, handle) = self.objects.register_object_and_open_handle(key)?;
        self.files.insert_file_search(search);
        transaction.commit_transaction();
        Ok(RuntimeFindFirstFileOutcome::Found(handle))
    }

    pub(crate) fn find_next_file(
        &mut self,
        handle: RuntimeHandle,
        output: GuestAddress,
        unicode: bool,
    ) -> Result<bool, RuntimeStateError> {
        self.require_source_file_operation_state()?;
        let search_id = self.file_search_for_handle(handle)?.id;
        let Some(entry) = self.files.next_file_search_entry(search_id)? else {
            return Ok(false);
        };
        let bytes = entry.encode_find_data(unicode)?;
        self.memory
            .validate_access(output, bytes.len(), MemoryAccess::Write)
            .map_err(RuntimeStateError::from_memory)?;
        let mut transaction = self.memory.begin_transaction();
        transaction
            .write(output, &bytes)
            .map_err(RuntimeStateError::from_memory)?;
        self.files.advance_file_search(search_id);
        transaction.commit_transaction();
        Ok(true)
    }

    pub(crate) fn file_search_for_handle(
        &self,
        handle: RuntimeHandle,
    ) -> Result<&RuntimeFileSearch, RuntimeStateError> {
        let record = self
            .objects
            .object_for_handle(handle, RuntimeObjectKind::FileSearch)?;
        let RuntimeObjectKey::FileSearch { search_id } = record.key else {
            return Err(RuntimeStateError::new(
                "file search handle resolved to a different object identity",
            ));
        };
        self.files
            .file_search(RuntimeFileSearchId(search_id))
            .map_err(|_| RuntimeStateError::new("live file search handle has no search state"))
    }

    pub(crate) fn find_close(&mut self, handle: RuntimeHandle) -> Result<(), RuntimeStateError> {
        self.require_source_file_operation_state()?;
        let record = self
            .objects
            .object_for_handle(handle, RuntimeObjectKind::FileSearch)?;
        let RuntimeObjectKey::FileSearch { search_id } = &record.key else {
            return Err(RuntimeStateError::new(
                "file search handle resolved to a different object identity",
            ));
        };
        let search_id = RuntimeFileSearchId(*search_id);
        self.file_search_for_handle(handle)?;
        self.objects.find_close(handle)?;
        self.files.retire_file_search(search_id);
        Ok(())
    }

    pub(crate) fn source_file_for_handle(
        &self,
        handle: RuntimeHandle,
    ) -> Result<&RuntimeSourceFile, RuntimeStateError> {
        self.live_source_file_for_handle(handle)?
            .ok_or_else(|| RuntimeStateError::new("file handle is invalid or no longer live"))
    }

    fn live_source_file_for_handle(
        &self,
        handle: RuntimeHandle,
    ) -> Result<Option<&RuntimeSourceFile>, RuntimeStateError> {
        let Some(record) = self
            .objects
            .live_object_for_handle(handle, RuntimeObjectKind::File)?
        else {
            return Ok(None);
        };
        let RuntimeObjectKey::File { file_id } = record.key else {
            return Err(RuntimeStateError::new(
                "source file handle resolved to a different object identity",
            ));
        };
        self.files
            .open_file(super::RuntimeSourceFileId(file_id))
            .map(Some)
            .map_err(|_| RuntimeStateError::new("live file handle has no open file state"))
    }

    pub(crate) fn source_file_byte_length(
        &self,
        handle: RuntimeHandle,
    ) -> Result<u64, RuntimeStateError> {
        let file_id = self.source_file_for_handle(handle)?.id;
        self.files.file_byte_length(file_id)
    }

    pub(crate) fn begin_source_file_read(
        &mut self,
        handle: RuntimeHandle,
        buffer: GuestAddress,
        bytes_read: GuestAddress,
        requested_bytes: u32,
    ) -> Result<crate::api::HostRequest, RuntimeStateError> {
        self.require_source_file_operation_state()?;
        let file_id = self.source_file_for_handle(handle)?.id;
        if self.opfs_write_pending(handle)? {
            return Err(RuntimeStateError::new(
                "file cannot read while an OPFS write is pending",
            ));
        }
        let service = self.files.file_read_service(file_id)?;
        let scheduler_plan = self.scheduler.checked_host_request(service.to_owned())?;
        let source_request = self.files.begin_read(
            file_id,
            scheduler_plan.request_id,
            buffer,
            bytes_read,
            requested_bytes,
        )?;
        let runtime_request = self.scheduler.publish_host_request(scheduler_plan);
        Ok(crate::api::HostRequest {
            request_id: runtime_request.request_id,
            service: runtime_request.service,
            payload: source_request.encode_host_payload().to_vec(),
        })
    }

    pub(crate) fn begin_opfs_write(
        &mut self,
        handle: RuntimeHandle,
        count: GuestAddress,
        transferred: u32,
    ) -> Result<Result<RuntimeHostRequest, u32>, RuntimeStateError> {
        self.begin_opfs_mutation(handle, count, transferred, false)
    }

    pub(crate) fn begin_opfs_truncate(
        &mut self,
        handle: RuntimeHandle,
    ) -> Result<Result<RuntimeHostRequest, u32>, RuntimeStateError> {
        self.begin_opfs_mutation(handle, GuestAddress(0), 0, true)
    }

    fn begin_opfs_mutation(
        &mut self,
        handle: RuntimeHandle,
        count: GuestAddress,
        transferred: u32,
        truncate: bool,
    ) -> Result<Result<RuntimeHostRequest, u32>, RuntimeStateError> {
        self.require_source_file_operation_state()?;
        let file = self.source_file_for_handle(handle)?;
        let super::RuntimeFileBacking::Opfs { byte_length } = file.backing else {
            return Ok(Err(5));
        };
        if file.desired_access & 0x4000_0000 == 0 {
            return Ok(Err(5));
        }
        if self.opfs_write_pending(handle)? {
            return Ok(Err(32));
        }
        if file
            .cursor
            .checked_add(u64::from(transferred))
            .is_none_or(|end| end > u64::from(u32::MAX))
        {
            return Ok(Err(112));
        }
        let relative_path = self
            .files
            .opfs_relative_path(
                &file.normalized_path_a,
                &self.process.current_directory_a().bytes_with_nul,
            )
            .map_err(|_| RuntimeStateError::new("open OPFS file path left the selected mount"))?;
        let pending = PendingOpfsWrite {
            truncate,
            file_id: file.id,
            cursor: file.cursor,
            byte_length,
            transferred,
            count,
            relative_path,
        };
        let scheduler = self.scheduler.checked_host_request(
            if truncate {
                "browser.opfs-truncate"
            } else {
                "browser.opfs-write"
            }
            .to_owned(),
        )?;
        let request = self.scheduler.publish_host_request(scheduler);
        let previous = self.pending_opfs_writes.insert(request.request_id, pending);
        debug_assert!(previous.is_none());
        Ok(Ok(request))
    }

    pub(crate) fn opfs_write_pending(
        &self,
        handle: RuntimeHandle,
    ) -> Result<bool, RuntimeStateError> {
        let file = self.source_file_for_handle(handle)?;
        for pending in self.pending_opfs_writes.values() {
            let other = self.files.open_file(pending.file_id)?;
            if self.files.same_path(
                &file.normalized_path_a,
                &other.normalized_path_a,
                &self.process.current_directory_a().bytes_with_nul,
            )? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub fn opfs_write_host_info(
        &self,
        request_id: u64,
    ) -> Result<(&str, u64, u32), RuntimeStateError> {
        let pending = self
            .pending_opfs_writes
            .get(&request_id)
            .ok_or_else(|| RuntimeStateError::new("OPFS write request is not pending"))?;
        Ok((
            pending.relative_path.as_str(),
            pending.cursor,
            pending.transferred,
        ))
    }

    pub(crate) fn preflight_opfs_write_completion(
        &self,
        request_id: u64,
        payload_bytes: usize,
    ) -> Result<(GuestAddress, u32), RuntimeStateError> {
        let pending = self
            .pending_opfs_writes
            .get(&request_id)
            .ok_or_else(|| RuntimeStateError::new("OPFS write request is not pending"))?;
        let scheduler = self.scheduler.validate_host_completion(request_id)?;
        let file = self.files.open_file(pending.file_id)?;
        if scheduler.service
            != if pending.truncate {
                "browser.opfs-truncate"
            } else {
                "browser.opfs-write"
            }
            || payload_bytes != pending.transferred as usize
            || file.cursor != pending.cursor
            || file.backing
                != (super::RuntimeFileBacking::Opfs {
                    byte_length: pending.byte_length,
                })
        {
            return Err(RuntimeStateError::new(
                "OPFS write no longer matches its open file",
            ));
        }
        Ok((pending.count, pending.transferred))
    }

    pub(crate) fn commit_opfs_write(&mut self, request_id: u64, succeeded: bool) {
        let pending = self
            .pending_opfs_writes
            .remove(&request_id)
            .expect("preflighted OPFS write");
        let scheduler = self
            .scheduler
            .validate_host_completion(request_id)
            .expect("preflighted OPFS completion");
        self.scheduler.commit_host_completion(scheduler);
        if succeeded {
            if pending.truncate {
                self.files
                    .finish_opfs_truncate(pending.file_id, pending.cursor);
            } else {
                self.files
                    .finish_opfs_write(pending.file_id, pending.cursor, pending.transferred);
            }
        }
    }

    pub fn opfs_read_host_path(&self, request_id: u64) -> Result<String, RuntimeStateError> {
        self.files.opfs_read_path(
            request_id,
            &self.process.current_directory_a().bytes_with_nul,
        )
    }

    pub(crate) fn plan_source_file_read_completion(
        &self,
        response: &RuntimeSourceReadHostResponse,
    ) -> Result<RuntimeSourceReadAggregateCompletionPlan, RuntimeStateError> {
        let scheduler = self
            .scheduler
            .validate_host_completion(response.request_id)?;
        if !matches!(
            scheduler.service.as_str(),
            RUNTIME_SOURCE_READ_SERVICE | RUNTIME_OPFS_READ_SERVICE
        ) {
            return Err(RuntimeStateError::new(
                "source read completion belongs to another host service",
            ));
        }
        let source_read = self.files.plan_read_completion(response)?;
        if scheduler.service != source_read.service() {
            return Err(RuntimeStateError::new(
                "file read completion service does not match its backing authority",
            ));
        }
        Ok(RuntimeSourceReadAggregateCompletionPlan {
            scheduler,
            source_read,
        })
    }

    pub(crate) fn commit_source_file_read_completion(
        &mut self,
        plan: RuntimeSourceReadAggregateCompletionPlan,
    ) {
        self.scheduler.commit_host_completion(plan.scheduler);
        self.files.commit_read_completion(plan.source_read)
    }

    pub(crate) fn seek_source_file(
        &mut self,
        handle: RuntimeHandle,
        distance: i64,
        origin: u32,
        distance_high: Option<GuestAddress>,
    ) -> Result<Result<u64, u32>, RuntimeStateError> {
        self.require_source_file_operation_state()?;
        let file_id = self.source_file_for_handle(handle)?.id;
        if self.opfs_write_pending(handle)? {
            return Err(RuntimeStateError::new(
                "file cannot seek while an OPFS write is pending",
            ));
        }
        let max_cursor = if distance_high.is_some() {
            i64::MAX as u64
        } else {
            u64::from(u32::MAX - 1)
        };
        let memory = &mut self.memory;
        self.files
            .seek_with(file_id, distance, origin, max_cursor, |cursor| {
                if let Some(address) = distance_high {
                    let mut transaction = memory.begin_transaction();
                    transaction
                        .write(address, &((cursor >> 32) as u32).to_le_bytes())
                        .map_err(RuntimeStateError::from_memory)?;
                    transaction.commit_transaction();
                }
                Ok(())
            })
    }

    fn require_source_file_operation_state(&self) -> Result<(), RuntimeStateError> {
        if !self.process.is_running()
            || self.current_thread()?.lifecycle != RuntimeThreadLifecycleState::Running
            || !matches!(
                self.scheduler_state()?,
                SchedulerThreadState::Runnable | SchedulerThreadState::InCallback { .. }
            )
        {
            return Err(RuntimeStateError::new(
                "source file operation requires a running process and active current thread",
            ));
        }
        Ok(())
    }

    pub(crate) fn set_unhandled_exception_filter(
        &mut self,
        filter: Option<GuestAddress>,
    ) -> Result<Option<GuestAddress>, RuntimeStateError> {
        self.process.set_unhandled_exception_filter(filter)
    }

    pub fn process_address_space_allocator(&self) -> &ProcessAddressSpaceAllocator {
        &self.address_space_allocator
    }

    pub(crate) fn reserve_virtual_memory(
        &mut self,
        byte_length: u32,
    ) -> Result<Result<GuestAddress, u32>, RuntimeStateError> {
        self.allocate_virtual_memory(
            GuestAddress(0),
            byte_length,
            true,
            false,
            PageProtection {
                read: true,
                write: true,
                execute: false,
            },
        )
    }

    pub(crate) fn allocate_virtual_memory(
        &mut self,
        preferred: GuestAddress,
        byte_length: u32,
        top_down: bool,
        commit: bool,
        protection: PageProtection,
    ) -> Result<Result<GuestAddress, u32>, RuntimeStateError> {
        self.require_virtual_memory_operation_state()?;
        if byte_length == 0 {
            return Ok(Err(87)); // ERROR_INVALID_PARAMETER
        }
        let (base, length) = if preferred.0 == 0 {
            (
                GuestAddress(0x0001_0000),
                u64::from(byte_length).div_ceil(GUEST_PAGE_SIZE) * GUEST_PAGE_SIZE,
            )
        } else {
            let base = GuestAddress(preferred.0 & !0xffff);
            let end = (u64::from(preferred.0) + u64::from(byte_length)).div_ceil(GUEST_PAGE_SIZE)
                * GUEST_PAGE_SIZE;
            if base.0 < 0x10000 || end > 0x7fff0000 {
                return Ok(Err(487));
            }
            (base, end - u64::from(base.0))
        };
        if preferred.0 != 0
            && self.address_space_allocator.records().any(|record| {
                u64::from(record.range.base.0) < u64::from(base.0) + length
                    && u64::from(base.0) < u64::from(record.range.base.0) + record.range.byte_length
            })
        {
            return Ok(Err(487));
        }
        let address_request = ProcessAddressAllocationRequest {
            purpose: ProcessAddressAllocationPurpose::VirtualMemory,
            preferred_address: (preferred.0 != 0).then_some(base),
            search_range: RuntimeAddressRange::new(
                base,
                if preferred.0 == 0 {
                    0x7ffe_0000
                } else {
                    length
                },
            )?,
            reserve_size: length,
            alignment: 65_536,
            guard_pages: 0,
            classification: ProcessAddressAllocationClassification::VirtualMemory,
            release_policy: ProcessAddressAllocationReleasePolicy::Explicit,
            executable: commit && protection.execute,
        };
        let memory = &mut self.memory;
        let mut install = |allocation: &ProcessAddressAllocationRecord| {
            let mut transaction = memory.begin_transaction();
            let memory_owner = match transaction.create_owner(MemoryOwnerKind::Anonymous) {
                Ok(owner) => owner,
                Err(error) if error.class == GuestMemoryErrorClass::ResourceLimit => {
                    return Ok(None); // ERROR_NOT_ENOUGH_MEMORY
                }
                Err(error) => return Err(RuntimeStateError::from_memory(error)),
            };
            if let Err(error) = transaction.reserve_fixed_for_address_allocation(
                allocation.id.0,
                memory_owner,
                MemoryRegionKind::Anonymous,
                allocation.range.base,
                allocation.range.byte_length,
            ) {
                return match error.class {
                    GuestMemoryErrorClass::ResourceLimit => Ok(None),
                    _ => Err(RuntimeStateError::from_memory(error)),
                };
            }
            if commit {
                let range = GuestRange::new(allocation.range.base, allocation.range.byte_length)
                    .map_err(RuntimeStateError::from_memory)?;
                if let Err(error) = transaction.commit(memory_owner, range, protection) {
                    return match error.class {
                        GuestMemoryErrorClass::ResourceLimit => Ok(None),
                        _ => Err(RuntimeStateError::from_memory(error)),
                    };
                }
            }
            let address = allocation.range.base;
            transaction
                .seal_owner(memory_owner)
                .map_err(RuntimeStateError::from_memory)?;
            transaction.commit_transaction();
            Ok(Some(address))
        };
        let reserved = if top_down {
            self.address_space_allocator
                .try_reserve_top_down_with(address_request, install)?
        } else {
            self.address_space_allocator
                .try_reserve_batch_with(vec![(None, address_request)], |allocations| {
                    install(&allocations[0])
                })?
        };
        Ok(reserved.ok_or(8)) // ERROR_NOT_ENOUGH_MEMORY
    }

    pub(crate) fn commit_virtual_memory(
        &mut self,
        address: GuestAddress,
        byte_length: u32,
        protection: PageProtection,
    ) -> Result<Result<GuestAddress, u32>, RuntimeStateError> {
        self.require_virtual_memory_operation_state()?;
        let Some(committed_range) = Self::virtual_memory_page_range(address, byte_length) else {
            return Ok(Err(487)); // ERROR_INVALID_ADDRESS
        };
        let mut matching_allocation = None;
        for allocation in self.address_space_allocator.records() {
            if allocation.purpose != ProcessAddressAllocationPurpose::VirtualMemory
                || !allocation.range.contains(committed_range)?
            {
                continue;
            }
            if matching_allocation.replace(allocation).is_some() {
                return Err(RuntimeStateError::new(
                    "virtual memory commit range belongs to multiple process allocations",
                ));
            }
        }
        let Some(allocation) = matching_allocation else {
            return Ok(Err(487)); // ERROR_INVALID_ADDRESS
        };
        if allocation.classification != ProcessAddressAllocationClassification::VirtualMemory
            || allocation.release_policy != ProcessAddressAllocationReleasePolicy::Explicit
        {
            return Err(RuntimeStateError::new(
                "virtual memory commit resolved to an incompatible process allocation",
            ));
        }
        let mut matching_region = self
            .memory
            .regions()
            .filter(|region| region.address_allocation_id == Some(allocation.id.0));
        let region = matching_region.next().ok_or_else(|| {
            RuntimeStateError::new(
                "virtual memory process allocation has no authoritative guest region",
            )
        })?;
        if matching_region.next().is_some()
            || region.kind != MemoryRegionKind::Anonymous
            || region.range.start() != allocation.range.base
            || region.range.byte_length() != allocation.range.byte_length
        {
            return Err(RuntimeStateError::new(
                "virtual memory guest region does not match its process allocation identity",
            ));
        }
        drop(matching_region);
        let guest_range = GuestRange::new(committed_range.base, committed_range.byte_length)
            .map_err(RuntimeStateError::from_memory)?;
        let mut missing = Vec::<GuestRange>::new();
        for offset in (0..committed_range.byte_length).step_by(GUEST_PAGE_SIZE as usize) {
            let page_address = GuestAddress((u64::from(committed_range.base.0) + offset) as u32);
            if let Some((owner, _)) = self
                .memory
                .committed_page(page_address)
                .map_err(RuntimeStateError::from_memory)?
            {
                if owner != region.owner {
                    return Err(RuntimeStateError::new(
                        "virtual memory page belongs to a different reservation owner",
                    ));
                }
            } else if let Some(last) = missing
                .last_mut()
                .filter(|range| range.end() == u64::from(page_address.0))
            {
                *last = GuestRange::new(last.start(), last.byte_length() + GUEST_PAGE_SIZE)
                    .map_err(RuntimeStateError::from_memory)?;
            } else {
                missing.push(
                    GuestRange::new(page_address, GUEST_PAGE_SIZE)
                        .map_err(RuntimeStateError::from_memory)?,
                );
            }
        }
        let allocation_id = allocation.id;
        let executable = allocation.executable || (!missing.is_empty() && protection.execute);
        let mut transaction = self.memory.begin_transaction();
        for range in missing {
            if let Err(error) = transaction.commit(region.owner, range, protection) {
                return match error.class {
                    GuestMemoryErrorClass::ResourceLimit => Ok(Err(8)),
                    _ => Err(RuntimeStateError::from_memory(error)),
                };
            }
        }
        self.address_space_allocator
            .set_validated_virtual_executable(allocation_id, executable);
        transaction.commit_transaction();
        Ok(Ok(guest_range.start()))
    }

    pub(crate) fn protect_virtual_memory(
        &mut self,
        address: GuestAddress,
        byte_length: u32,
        protection: PageProtection,
        old_output: GuestAddress,
    ) -> Result<Result<(), u32>, RuntimeStateError> {
        self.require_virtual_memory_operation_state()?;
        self.memory
            .validate_access(old_output, 4, MemoryAccess::Write)
            .map_err(RuntimeStateError::from_memory)?;
        if byte_length == 0 {
            return Ok(Err(87));
        }
        let base = GuestAddress(address.0 & !(GUEST_PAGE_SIZE as u32 - 1));
        let end = (u64::from(address.0) + u64::from(byte_length)).div_ceil(GUEST_PAGE_SIZE)
            * GUEST_PAGE_SIZE;
        if end > 1_u64 << 32 {
            return Ok(Err(487));
        }
        let range = GuestRange::new(base, end - u64::from(base.0))
            .map_err(RuntimeStateError::from_memory)?;
        let Some(allocation) = self
            .address_space_allocator
            .records()
            .find(|allocation| {
                allocation.purpose == ProcessAddressAllocationPurpose::VirtualMemory
                    && allocation.range.base.0 <= base.0
                    && u64::from(allocation.range.base.0) + allocation.range.byte_length >= end
            })
            .cloned()
        else {
            return Ok(Err(487));
        };
        let mut regions = self
            .memory
            .regions()
            .filter(|region| region.address_allocation_id == Some(allocation.id.0));
        let region = regions
            .next()
            .ok_or_else(|| RuntimeStateError::new("virtual protection has no owned region"))?;
        if regions.next().is_some()
            || region.kind != MemoryRegionKind::Anonymous
            || region.range.start() != allocation.range.base
            || region.range.byte_length() != allocation.range.byte_length
        {
            return Err(RuntimeStateError::new(
                "virtual protection region differs from its process allocation",
            ));
        }
        drop(regions);
        let Some((owner, old)) = self
            .memory
            .committed_page(base)
            .map_err(RuntimeStateError::from_memory)?
        else {
            return Ok(Err(487));
        };
        if owner != region.owner {
            return Err(RuntimeStateError::new(
                "virtual protection page has a different owner",
            ));
        }
        for offset in (0..range.byte_length()).step_by(GUEST_PAGE_SIZE as usize) {
            match self
                .memory
                .committed_page(GuestAddress((u64::from(base.0) + offset) as u32))
                .map_err(RuntimeStateError::from_memory)?
            {
                Some((owner, _)) if owner == region.owner => {}
                None => return Ok(Err(487)),
                _ => {
                    return Err(RuntimeStateError::new(
                        "virtual protection crosses page ownership",
                    ));
                }
            }
        }
        let mut executable = protection.execute;
        if !executable {
            for offset in (0..allocation.range.byte_length).step_by(GUEST_PAGE_SIZE as usize) {
                let page = u64::from(allocation.range.base.0) + offset;
                if page >= u64::from(base.0) && page < end {
                    continue;
                }
                if self
                    .memory
                    .committed_page(GuestAddress(page as u32))
                    .map_err(RuntimeStateError::from_memory)?
                    .is_some_and(|(_, protection)| protection.execute)
                {
                    executable = true;
                    break;
                }
            }
        }
        let old_word: u32 = match (old.read, old.write, old.execute) {
            (_, true, true) => 0x40,
            (true, false, true) => 0x20,
            (false, false, true) => 0x10,
            (_, true, false) => 4,
            (true, false, false) => 2,
            (false, false, false) => 1,
        };
        let mut transaction = self.memory.begin_transaction();
        transaction
            .write(old_output, &old_word.to_le_bytes())
            .map_err(RuntimeStateError::from_memory)?;
        transaction
            .protect(region.owner, range, protection)
            .map_err(RuntimeStateError::from_memory)?;
        self.address_space_allocator
            .set_validated_virtual_executable(allocation.id, executable);
        transaction.commit_transaction();
        Ok(Ok(()))
    }

    pub(crate) fn release_virtual_memory(
        &mut self,
        address: GuestAddress,
    ) -> Result<Result<(), u32>, RuntimeStateError> {
        self.require_virtual_memory_operation_state()?;
        let Some(allocation) = self
            .address_space_allocator
            .records()
            .find(|allocation| {
                allocation.purpose == ProcessAddressAllocationPurpose::VirtualMemory
                    && allocation.range.base == address
            })
            .cloned()
        else {
            return Ok(Err(487));
        };
        self.address_space_allocator
            .validate_releases(&[allocation.id])?;
        let mut regions = self
            .memory
            .regions()
            .filter(|region| region.address_allocation_id == Some(allocation.id.0));
        let region = regions
            .next()
            .ok_or_else(|| RuntimeStateError::new("virtual release has no authoritative region"))?;
        if regions.next().is_some()
            || region.kind != MemoryRegionKind::Anonymous
            || region.range.start() != allocation.range.base
            || region.range.byte_length() != allocation.range.byte_length
        {
            return Err(RuntimeStateError::new(
                "virtual release does not own its complete region",
            ));
        }
        drop(regions);
        let mut transaction = self.memory.begin_transaction();
        transaction
            .release_region(region.owner, region.range.start())
            .map_err(RuntimeStateError::from_memory)?;
        transaction
            .destroy_owner(region.owner)
            .map_err(RuntimeStateError::from_memory)?;
        self.address_space_allocator
            .remove_validated_allocations(vec![allocation.id]);
        transaction.commit_transaction();
        Ok(Ok(()))
    }

    pub(crate) fn decommit_virtual_memory(
        &mut self,
        address: GuestAddress,
        byte_length: u32,
    ) -> Result<Result<(), u32>, RuntimeStateError> {
        self.require_virtual_memory_operation_state()?;
        let starting_page = Self::virtual_memory_page_range(address, 1)
            .expect("one guest byte fits within the x86 address space");
        let mut matching_allocation = None;
        for allocation in self.address_space_allocator.records() {
            if allocation.purpose != ProcessAddressAllocationPurpose::VirtualMemory
                || !allocation.range.contains(starting_page)?
            {
                continue;
            }
            if matching_allocation.replace(allocation).is_some() {
                return Err(RuntimeStateError::new(
                    "virtual memory decommit address belongs to multiple process allocations",
                ));
            }
        }
        let Some(allocation) = matching_allocation else {
            return Ok(Err(487)); // ERROR_INVALID_ADDRESS
        };
        let decommitted_range = if byte_length == 0 {
            if starting_page.base != allocation.range.base {
                return Ok(Err(487)); // ERROR_INVALID_ADDRESS
            }
            allocation.range
        } else {
            let Some(range) = Self::virtual_memory_page_range(address, byte_length) else {
                return Ok(Err(87)); // ERROR_INVALID_PARAMETER
            };
            if !allocation.range.contains(range)? {
                return Ok(Err(87)); // ERROR_INVALID_PARAMETER
            }
            range
        };
        if allocation.classification != ProcessAddressAllocationClassification::VirtualMemory
            || allocation.release_policy != ProcessAddressAllocationReleasePolicy::Explicit
        {
            return Err(RuntimeStateError::new(
                "virtual memory decommit resolved to an incompatible process allocation",
            ));
        }
        let mut matching_region = self
            .memory
            .regions()
            .filter(|region| region.address_allocation_id == Some(allocation.id.0));
        let region = matching_region.next().ok_or_else(|| {
            RuntimeStateError::new(
                "virtual memory process allocation has no authoritative guest region",
            )
        })?;
        if matching_region.next().is_some()
            || region.kind != MemoryRegionKind::Anonymous
            || region.range.start() != allocation.range.base
            || region.range.byte_length() != allocation.range.byte_length
        {
            return Err(RuntimeStateError::new(
                "virtual memory guest region does not match its process allocation identity",
            ));
        }
        drop(matching_region);
        let mut committed = Vec::<GuestRange>::new();
        for offset in (0..decommitted_range.byte_length).step_by(GUEST_PAGE_SIZE as usize) {
            let page = GuestAddress((u64::from(decommitted_range.base.0) + offset) as u32);
            match self
                .memory
                .committed_page(page)
                .map_err(RuntimeStateError::from_memory)?
            {
                Some((owner, _)) if owner != region.owner => {
                    return Err(RuntimeStateError::new(
                        "virtual memory page belongs to a different reservation owner",
                    ));
                }
                None => {}
                Some(_) => {
                    if let Some(last) = committed
                        .last_mut()
                        .filter(|range| range.end() == u64::from(page.0))
                    {
                        *last = GuestRange::new(last.start(), last.byte_length() + GUEST_PAGE_SIZE)
                            .map_err(RuntimeStateError::from_memory)?;
                    } else {
                        committed.push(
                            GuestRange::new(page, GUEST_PAGE_SIZE)
                                .map_err(RuntimeStateError::from_memory)?,
                        );
                    }
                }
            }
        }
        let mut executable = false;
        for offset in (0..allocation.range.byte_length).step_by(GUEST_PAGE_SIZE as usize) {
            let page = u64::from(allocation.range.base.0) + offset;
            if page >= u64::from(decommitted_range.base.0)
                && page < u64::from(decommitted_range.base.0) + decommitted_range.byte_length
            {
                continue;
            }
            if self
                .memory
                .committed_page(GuestAddress(page as u32))
                .map_err(RuntimeStateError::from_memory)?
                .is_some_and(|(_, protection)| protection.execute)
            {
                executable = true;
                break;
            }
        }
        let allocation_id = allocation.id;
        let mut transaction = self.memory.begin_transaction();
        for range in committed {
            transaction
                .decommit(region.owner, range)
                .map_err(RuntimeStateError::from_memory)?;
        }
        self.address_space_allocator
            .set_validated_virtual_executable(allocation_id, executable);
        transaction.commit_transaction();
        Ok(Ok(()))
    }

    fn virtual_memory_page_range(
        address: GuestAddress,
        byte_length: u32,
    ) -> Option<RuntimeAddressRange> {
        if byte_length == 0 {
            return None;
        }
        let page_mask = GUEST_PAGE_SIZE - 1;
        let base = GuestAddress(address.0 & !(GUEST_PAGE_SIZE as u32 - 1));
        let rounded_end = (u64::from(address.0) + u64::from(byte_length))
            .checked_add(page_mask)
            .map(|value| value & !page_mask)?;
        if rounded_end > (1u64 << 32) {
            return None;
        }
        Some(RuntimeAddressRange {
            base,
            byte_length: rounded_end - u64::from(base.0),
        })
    }

    fn require_virtual_memory_operation_state(&self) -> Result<(), RuntimeStateError> {
        if !self.process.is_running()
            || self.current_thread()?.lifecycle != RuntimeThreadLifecycleState::Running
            || !matches!(
                self.scheduler_state()?,
                SchedulerThreadState::Runnable | SchedulerThreadState::InCallback { .. }
            )
        {
            return Err(RuntimeStateError::new(
                "virtual memory reservation requires a running process and active current thread",
            ));
        }
        Ok(())
    }

    pub(crate) fn com(&self) -> &RuntimeCom {
        &self.com
    }

    pub(crate) fn resolve_com_method(
        &self,
        endpoint: GuestAddress,
    ) -> Result<Option<RuntimeComMethodTarget>, RuntimeStateError> {
        self.com.resolve_method(endpoint)
    }

    pub fn is_com_endpoint(&self, endpoint: GuestAddress) -> bool {
        self.com.has_endpoint(endpoint)
    }

    pub(crate) fn com_interface_for_pointer(
        &self,
        pointer: GuestAddress,
    ) -> Result<Option<&RuntimeComInterfaceRecord>, RuntimeStateError> {
        self.com.interface_for_pointer(pointer)
    }

    pub(crate) fn com_interface_for_object_iid(
        &self,
        object_id: RuntimeComObjectId,
        iid: RuntimeComIid,
    ) -> Result<&RuntimeComInterfaceRecord, RuntimeStateError> {
        self.com.interface_for_object_iid(object_id, iid)
    }

    pub(crate) fn install_graphics_vertex_buffer_object(
        &mut self,
        parent_interface_id: RuntimeComInterfaceId,
        device_com_object_id: RuntimeComObjectId,
        descriptor: super::RuntimeGraphicsVertexBufferDescriptor,
        output_address: GuestAddress,
    ) -> Result<(), RuntimeStateError> {
        let d3d8 = self.com.object(device_com_object_id)?.class == RuntimeComClass::D3d8Device;
        let spec = if d3d8 {
            crate::d3d8::vertex_buffer_spec()
        } else {
            crate::d3d9::direct3dvertexbuffer9_object_spec()
        };
        let iid = if d3d8 {
            crate::d3d8::VERTEX_BUFFER8_IID
        } else {
            crate::d3d9::D3D9_IDIRECT3DVERTEXBUFFER9_IID
        };
        crate::com::validate_object_spec(&spec)?;
        self.memory
            .validate_access(output_address, 4, MemoryAccess::Write)
            .map_err(RuntimeStateError::from_memory)?;
        self.require_graphics_operation_state()?;
        let arena = self
            .address_space_allocator
            .allocation_for_purpose(&ProcessAddressAllocationPurpose::GraphicsResourceArena)?;
        if arena.range.base != super::RUNTIME_GRAPHICS_RESOURCE_ARENA_BASE
            || arena.range.byte_length != super::RUNTIME_GRAPHICS_RESOURCE_ARENA_SIZE
            || arena.classification != ProcessAddressAllocationClassification::Graphics
            || arena.release_policy != ProcessAddressAllocationReleasePolicy::Permanent
            || arena.executable
        {
            return Err(RuntimeStateError::new(
                "graphics resource arena does not match the controlled process layout",
            ));
        }
        let memory = &mut self.memory;
        let com = &mut self.com;
        let graphics = &mut self.graphics;
        self.address_space_allocator.reserve_batch_with(
            vec![
                (None, crate::com::com_address_allocation_request()),
                (
                    Some(arena.id),
                    ProcessAddressAllocationRequest {
                        purpose: ProcessAddressAllocationPurpose::GraphicsResource,

                        preferred_address: None,
                        search_range: arena.range,
                        reserve_size: u64::from(descriptor.byte_length),
                        alignment: GUEST_PAGE_SIZE as u32,
                        guard_pages: 0,
                        classification: ProcessAddressAllocationClassification::Graphics,

                        release_policy: ProcessAddressAllocationReleasePolicy::Explicit,
                        executable: false,
                    },
                ),
            ],
            |allocations| {
                let com_allocation = &allocations[0];
                let resource_allocation = &allocations[1];
                let resource_range = GuestRange::new(
                    resource_allocation.range.base,
                    resource_allocation.range.byte_length,
                )
                .map_err(RuntimeStateError::from_memory)?;

                let creation = graphics.validate_vertex_buffer_creation(
                    device_com_object_id,
                    descriptor,
                    resource_allocation.id,
                    resource_range,
                )?;
                let resource_allocation_id = resource_allocation.id;
                let mut transaction = memory.begin_transaction();
                let (com_memory_owner, interfaces) =
                    crate::com::initialize_com_backing(&mut transaction, com_allocation, &spec)?;
                let writable = PageProtection {
                    read: true,
                    write: true,
                    execute: false,
                };

                let resource_memory_owner = transaction
                    .create_owner(MemoryOwnerKind::Runtime)
                    .map_err(RuntimeStateError::from_memory)?;
                transaction
                    .reserve_fixed_for_address_allocation(
                        resource_allocation.id.0,
                        resource_memory_owner,
                        MemoryRegionKind::GraphicsResource,
                        resource_allocation.range.base,
                        resource_allocation.range.byte_length,
                    )
                    .map_err(RuntimeStateError::from_memory)?;
                transaction
                    .commit(resource_memory_owner, resource_range, writable)
                    .map_err(RuntimeStateError::from_memory)?;
                transaction
                    .protect(
                        resource_memory_owner,
                        resource_range,
                        PageProtection {
                            read: true,
                            write: false,
                            execute: false,
                        },
                    )
                    .map_err(RuntimeStateError::from_memory)?;

                let com_object_id = com.register_with(
                    RuntimeComObjectRegistration {
                        class: spec.class,
                        initial_reference_count: 1,
                        parent_interface_id: Some(parent_interface_id),
                        backing: RuntimeComBacking {
                            address_allocation_id: com_allocation.id,
                            memory_owner: com_memory_owner,
                            range: com_allocation.range,
                        },
                        interfaces,
                    },
                    |object, interfaces| {
                        let output_interface = interfaces
            .iter()
            .find(|interface| interface.iid == iid)
            .ok_or_else(|| {
                RuntimeStateError::new(
                    "installed graphics vertex-buffer object omitted its requested interface",
                )
            })?;
                        let bytes = output_interface.guest_pointer.0.to_le_bytes();
                        transaction
                            .write(output_address, &bytes)
                            .map_err(RuntimeStateError::from_memory)?;
                        Ok(object.id)
                    },
                )?;
                graphics.insert_vertex_buffer(
                    creation,
                    com_object_id,
                    resource_memory_owner,
                    resource_allocation_id,
                    resource_range,
                );
                transaction.commit_transaction();
                Ok(())
            },
        )
    }

    pub(crate) fn install_com_object_with_guest_writes<F>(
        &mut self,
        parent_interface_id: Option<RuntimeComInterfaceId>,
        spec: ComObjectSpec,
        build_guest_writes: F,
    ) -> Result<InstalledComObject, RuntimeStateError>
    where
        F: FnOnce(&InstalledComObject) -> Result<Vec<(GuestAddress, Vec<u8>)>, RuntimeStateError>,
    {
        crate::com::validate_object_spec(&spec)?;
        if spec.class == RuntimeComClass::DirectSound8 {
            self.require_audio_operation_state()?;
        }
        let memory = &mut self.memory;
        let com = &mut self.com;
        let audio = &mut self.audio;
        let direct_show = &mut self.direct_show;
        self.address_space_allocator.reserve_with(
            crate::com::com_address_allocation_request(),
            |allocation| {
                let mut transaction = memory.begin_transaction();
                let (memory_owner, interfaces) =
                    crate::com::initialize_com_backing(&mut transaction, allocation, &spec)?;

                let installed = com.register_with(
                    RuntimeComObjectRegistration {
                        class: spec.class,
                        initial_reference_count: 1,
                        parent_interface_id,
                        backing: RuntimeComBacking {
                            address_allocation_id: allocation.id,
                            memory_owner,
                            range: allocation.range,
                        },
                        interfaces,
                    },
                    |object, interfaces| {
                        let installed = InstalledComObject {
                            object_id: object.id,
                            interfaces: interfaces
                                .iter()
                                .map(|interface| (interface.iid, interface.guest_pointer))
                                .collect(),
                        };
                        let additional_writes = build_guest_writes(&installed)?;
                        for (address, bytes) in additional_writes {
                            transaction
                                .write(address, &bytes)
                                .map_err(RuntimeStateError::from_memory)?;
                        }
                        if object.class == RuntimeComClass::DirectSound8 {
                            audio.create_device(object.id)?;
                        }
                        if object.class == RuntimeComClass::DirectShowGraph {
                            direct_show.graphs.insert(object.id, Default::default());
                        }
                        Ok(installed)
                    },
                )?;
                transaction.commit_transaction();
                Ok(installed)
            },
        )
    }

    pub(crate) fn add_ref_com_interface(
        &mut self,
        interface_id: RuntimeComInterfaceId,
    ) -> Result<u32, RuntimeStateError> {
        self.com.add_ref(interface_id)
    }

    pub(crate) fn release_com_interface(
        &mut self,
        interface_id: RuntimeComInterfaceId,
    ) -> Result<u32, RuntimeStateError> {
        if let Some(count) = self.com.release_shared(interface_id)? {
            return Ok(count);
        }
        let retirement = self.validate_com_release(interface_id)?;
        debug_assert_eq!(retirement.com.reference_count(), 0);
        let mut transaction = self.memory.begin_transaction();
        for (owner, range) in &retirement.memory_retirements {
            transaction
                .release_region(*owner, range.start())
                .map_err(RuntimeStateError::from_memory)?;
            transaction
                .destroy_owner(*owner)
                .map_err(RuntimeStateError::from_memory)?;
        }
        transaction.commit_transaction();
        self.retire_com_objects(retirement);
        Ok(0)
    }

    fn validate_com_release(
        &self,
        interface_id: RuntimeComInterfaceId,
    ) -> Result<RuntimeComRetirement, RuntimeStateError> {
        let com = self.com.validate_release(interface_id)?;
        let retired_object_ids = com
            .retired_objects()
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let com_backings = com
            .retired_objects()
            .iter()
            .map(|id| self.com.object(*id).map(|object| &object.backing))
            .collect::<Result<Vec<_>, _>>()?;
        let audio = self
            .audio
            .validate_com_object_retirements(&retired_object_ids)?;
        let audio_buffer_backings = audio
            .as_ref()
            .map(|retirement| self.audio.buffer_backings_for_retirement(retirement))
            .unwrap_or_default();
        let graphics = self
            .graphics
            .validate_com_object_retirements(&retired_object_ids)?;
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
        let media_backings = self.direct_show.retirement_backings(&retired_object_ids)?;
        for backing in &media_backings {
            self.validate_media_backing(*backing)?;
        }
        // A released sample may return its bytes to a live allocator rather
        // than destroy them. Validate that ownership before publishing it free.
        for id in &retired_object_ids {
            if let Some(sample) = self.direct_show.samples.get(id) {
                self.validate_media_backing(sample.backing)?;
            }
        }
        let mut allocation_ids = com_backings
            .iter()
            .map(|backing| backing.address_allocation_id)
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
        allocation_ids.extend(media_backings.iter().map(|backing| backing.allocation_id));
        let address_space = if allocation_ids.is_empty() {
            None
        } else {
            let plan = self
                .address_space_allocator
                .validate_releases(&allocation_ids)?;
            let (com_allocations, resource_allocations) =
                plan.split_at(com.retired_objects().len());
            let (audio_allocations, graphics_allocations) =
                resource_allocations.split_at(audio_buffer_backings.len());
            let (texture_allocations, remaining_allocations) =
                graphics_allocations.split_at(texture_backings.len());
            let (vertex_buffer_allocations, media_allocations) =
                remaining_allocations.split_at(vertex_buffer_backings.len());
            for (backing, released) in com_backings.iter().zip(com_allocations) {
                if released.range != backing.range
                    || released.purpose != ProcessAddressAllocationPurpose::ComEndpoint
                    || released.classification != ProcessAddressAllocationClassification::Com
                    || released.release_policy != ProcessAddressAllocationReleasePolicy::Explicit
                    || !released.executable
                {
                    return Err(RuntimeStateError::new(
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
                    || released.classification != ProcessAddressAllocationClassification::Audio
                    || released.release_policy != ProcessAddressAllocationReleasePolicy::Explicit
                    || released.executable
                {
                    return Err(RuntimeStateError::new(
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
                    || released.classification != ProcessAddressAllocationClassification::Graphics
                    || released.release_policy != ProcessAddressAllocationReleasePolicy::Explicit
                    || released.executable
                {
                    return Err(RuntimeStateError::new(
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
                    || released.classification != ProcessAddressAllocationClassification::Graphics
                    || released.release_policy != ProcessAddressAllocationReleasePolicy::Explicit
                    || released.executable
                {
                    return Err(RuntimeStateError::new(
                        "retiring graphics vertex buffer does not own its planned guest backing",
                    ));
                }
            }
            for (backing, released) in media_backings.iter().zip(media_allocations) {
                if released.range.base != backing.range.start()
                    || released.range.byte_length != backing.range.byte_length()
                    || released.purpose != ProcessAddressAllocationPurpose::MediaResource
                    || released.classification != ProcessAddressAllocationClassification::Com
                    || released.release_policy != ProcessAddressAllocationReleasePolicy::Explicit
                    || released.executable
                {
                    return Err(RuntimeStateError::new(
                        "retiring DirectShow sample does not own its media byte allocation",
                    ));
                }
            }
            Some(allocation_ids)
        };
        let mut memory_retirements = com_backings
            .iter()
            .map(|backing| {
                GuestRange::new(backing.range.base, backing.range.byte_length)
                    .map(|range| (backing.memory_owner, range))
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
        memory_retirements.extend(
            media_backings
                .iter()
                .map(|backing| (backing.owner, backing.range)),
        );
        self.gdi
            .validate_com_object_retirements(&retired_object_ids)?;
        Ok(RuntimeComRetirement {
            com,
            address_space,
            audio,
            graphics,
            memory_retirements,
        })
    }

    fn retire_com_objects(&mut self, plan: RuntimeComRetirement) {
        let retired_object_ids = plan.com.retired_objects().to_vec();
        for object in &retired_object_ids {
            self.input.direct_input.remove(object);
        }
        self.direct_show.retire_objects(&retired_object_ids);
        self.com.remove_validated_objects(plan.com);
        if let Some(address_space) = plan.address_space {
            self.address_space_allocator
                .remove_validated_allocations(address_space);
        }
        if let Some(graphics) = plan.graphics {
            self.graphics.remove_validated_com_objects(graphics);
        }
        if let Some(audio) = plan.audio {
            self.audio.remove_validated_com_objects(audio);
        }
    }

    pub(crate) fn process_command_line(&self) -> Result<&RuntimeCommandLine, RuntimeStateError> {
        self.process.command_line()
    }

    pub(crate) fn process_environment_strings(
        &self,
    ) -> Result<&RuntimeEnvironmentStrings, RuntimeStateError> {
        if let Some((heap, block)) = self.process.environment_heap_authority
            && self.private_heap_allocation_option(heap, block.address)? != Some(block)
        {
            return Err(RuntimeStateError::new(
                "process environment allocation is no longer live",
            ));
        }
        self.process.environment_strings()
    }

    pub(crate) fn allocate_environment_copy(
        &mut self,
        bytes: &[u8],
        wide: bool,
    ) -> Result<Option<GuestAddress>, RuntimeStateError> {
        let heap = self.process_heap()?;
        let Some(block) = self.allocate_private_heap_block(
            heap,
            bytes.len() as u32,
            RuntimeHeapInitialization::Uninitialized,
        )?
        else {
            return Ok(None);
        };
        if let Err(error) = self.memory.write(block.address, bytes) {
            self.free_private_heap_block(heap, block.address)?;
            return Err(RuntimeStateError::from_memory(error));
        }
        self.process
            .environment_copies
            .insert(block.address, (wide, block));
        Ok(Some(block.address))
    }

    pub(crate) fn free_environment_copy(
        &mut self,
        pointer: GuestAddress,
        wide: bool,
    ) -> Result<bool, RuntimeStateError> {
        let Some((encoding, block)) = self.process.environment_copies.get(&pointer).copied() else {
            return Ok(false);
        };
        if encoding != wide {
            return Ok(false);
        }
        let heap = self.process_heap()?;
        if self.private_heap_allocation_option(heap, pointer)? != Some(block) {
            return Ok(false);
        }
        self.free_private_heap_block(heap, pointer)?;
        self.process.environment_copies.remove(&pointer);
        Ok(true)
    }

    pub(crate) fn replace_environment_strings(
        &mut self,
        bytes: &[u8],
    ) -> Result<bool, RuntimeStateError> {
        let environment = self
            .process
            .environment()
            .ok_or_else(|| RuntimeStateError::new("process environment is absent"))?;
        let parameters = self
            .memory
            .read(GuestAddress(environment.peb_address.0 + 0x10), 4)
            .map_err(RuntimeStateError::from_memory)?;
        let pointer = GuestAddress(
            u32::from_le_bytes(parameters.try_into().unwrap())
                .checked_add(crate::environment::PROCESS_PARAMETERS_ENVIRONMENT_POINTER_OFFSET)
                .ok_or_else(|| RuntimeStateError::new("process environment pointer overflowed"))?,
        );
        self.memory
            .validate_access(pointer, 4, MemoryAccess::Write)
            .map_err(RuntimeStateError::from_memory)?;
        self.process_environment_strings()?;
        let heap = self.process_heap()?;
        let Some(block) = self.allocate_private_heap_block(
            heap,
            bytes.len() as u32,
            RuntimeHeapInitialization::Uninitialized,
        )?
        else {
            return Ok(false);
        };
        if let Err(error) = self.memory.write(block.address, bytes) {
            self.free_private_heap_block(heap, block.address)?;
            return Err(RuntimeStateError::from_memory(error));
        }
        self.memory
            .write(pointer, &block.address.0.to_le_bytes())
            .map_err(RuntimeStateError::from_memory)?;
        let authority = self.process.environment_strings_mut()?;
        authority.authority_buffer_address = block.address;
        authority.byte_length_with_double_nul = bytes.len() as u32;
        if let Some(old) = self
            .process
            .environment_heap_authority
            .replace((heap, block))
        {
            self.free_private_heap_block(old.0, old.1.address)?;
        }
        Ok(true)
    }

    pub fn current_thread_id(&self) -> RuntimeThreadId {
        self.scheduler.current_thread_id()
    }

    /// Reads a known location without requiring the stopped CPU's pending delta to be acknowledged.
    #[cold]
    pub fn diagnostic_location(&self, address: GuestAddress) -> (Option<String>, Option<String>) {
        let page_address = GuestAddress(address.0 & !(GUEST_PAGE_SIZE as u32 - 1));
        let Ok(Some((owner, _))) = self.memory.committed_page(page_address) else {
            return (None, None);
        };
        if !self
            .memory
            .owner_is_sealed(owner)
            .is_ok_and(|sealed| sealed)
        {
            return (None, None);
        }
        if let Ok(Some(target)) = self.com.resolve_method(address) {
            let Ok(object) = self.com.object(target.object_id) else {
                return (None, None);
            };
            let backing = &object.backing;
            let live = backing.memory_owner == owner
                && self
                    .address_space_allocator
                    .allocation(backing.address_allocation_id)
                    .is_ok_and(|allocation| {
                        allocation.range == backing.range
                            && allocation.purpose == ProcessAddressAllocationPurpose::ComEndpoint
                            && allocation.classification
                                == ProcessAddressAllocationClassification::Com
                    })
                && self.memory.regions().any(|region| {
                    region.owner == owner
                        && region.kind == MemoryRegionKind::Runtime
                        && region.address_allocation_id == Some(backing.address_allocation_id.0)
                        && region.range.start() == backing.range.base
                        && region.range.byte_length() == backing.range.byte_length
                });
            return if live {
                (
                    None,
                    Some(format!("{}::{}", target.interface_name, target.method.name)),
                )
            } else {
                (None, None)
            };
        }
        let Some(image) = self.modules.images().images().values().find(|image| {
            image.memory_owner == owner
                && address.0 >= image.image_range.start().0
                && u64::from(address.0) < image.image_range.end()
        }) else {
            return (None, None);
        };
        if !self.module_option(image.handle).is_ok_and(|module| {
            module.is_some_and(|module| module.canonical_name == image.canonical_name)
        }) || !self
            .modules
            .graph()
            .module(image.module_id)
            .is_ok_and(|module| {
                module.handle == image.handle && module.canonical_name == image.canonical_name
            })
            || !self.memory.regions().any(|region| {
                region.owner == owner
                    && region.kind == MemoryRegionKind::Image
                    && region.range == image.image_range
            })
        {
            return (None, None);
        }
        let symbol = image.exports.as_ref().and_then(|exports| {
            exports
                .symbols
                .iter()
                .find_map(|symbol| match symbol.target {
                    crate::pe32::ExportTarget::Address { rva }
                        if image.image_range.start().0.checked_add(rva.0) == Some(address.0) =>
                    {
                        Some(
                            symbol
                                .names
                                .first()
                                .cloned()
                                .unwrap_or_else(|| format!("#{}", symbol.ordinal)),
                        )
                    }
                    _ => None,
                })
        });
        (Some(image.canonical_name.clone()), symbol)
    }

    pub(crate) fn current_thread_state(&self) -> Result<&RuntimeThreadState, RuntimeStateError> {
        self.current_thread()
    }

    pub(crate) fn current_thread_teb_address(&self) -> Result<GuestAddress, RuntimeStateError> {
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
                    RuntimeStateError::new("initial runtime thread has no registered process TEB")
                });
        }
        Err(RuntimeStateError::new(
            "registered guest thread has no thread environment",
        ))
    }

    pub(crate) fn thread_stack(
        &self,
        thread_id: RuntimeThreadId,
    ) -> Result<&RuntimeThreadStack, RuntimeStateError> {
        self.thread_state(thread_id)?.stack.as_ref().ok_or_else(|| {
            RuntimeStateError::new(format!(
                "runtime thread ID {} has no registered stack",
                thread_id.0
            ))
        })
    }

    pub fn install_initial_thread_stack(
        &mut self,
        entry_return: &crate::callbacks::ProcessEntryReturnSentinel,
        reserve_size: u64,
        commit_size: u64,
    ) -> Result<InitialThreadStack, RuntimeStateError> {
        let thread_id = self.current_thread_id();
        let thread = self.thread_state(thread_id)?;
        if !matches!(
            thread.lifecycle,
            RuntimeThreadLifecycleState::Initialized | RuntimeThreadLifecycleState::Running
        ) || thread.stack.is_some()
        {
            return Err(RuntimeStateError::new(
                "initial thread stack requires a live thread without a stack",
            ));
        }
        if reserve_size == 0
            || commit_size == 0
            || commit_size > reserve_size
            || !reserve_size.is_multiple_of(GUEST_PAGE_SIZE)
            || !commit_size.is_multiple_of(GUEST_PAGE_SIZE)
        {
            return Err(RuntimeStateError::new(
                "initial stack sizes must be page aligned and nonzero with commit no larger than reserve",
            ));
        }
        let guard_pages = u32::from(commit_size < reserve_size);
        let preferred_address = u64::from(INITIAL_THREAD_STACK_TOP.0)
            .checked_sub(reserve_size)
            .filter(|base| *base >= 0x1_0000)
            .map(|base| GuestAddress(base as u32));
        let memory = &mut self.memory;
        let threads = &mut self.threads;
        self.address_space_allocator.reserve_with(
            ProcessAddressAllocationRequest {
                purpose: ProcessAddressAllocationPurpose::ThreadStack,
                preferred_address,
                // Keep the stack below the control/TEB band and inside the Fast arena.
                search_range: RuntimeAddressRange::new(GuestAddress(0x1_0000), 0x7eff_0000)?,
                reserve_size: reserve_size - u64::from(guard_pages) * GUEST_PAGE_SIZE,
                alignment: GUEST_PAGE_SIZE as u32,
                guard_pages,
                classification: ProcessAddressAllocationClassification::Stack,
                release_policy: ProcessAddressAllocationReleasePolicy::Explicit,
                executable: false,
            },
            |allocation| {
                let stack_top = GuestAddress(
                    u32::try_from(allocation.range.end_exclusive()?).map_err(|_| {
                        RuntimeStateError::new("initial stack top exceeds the x86 address space")
                    })?,
                );
                let stack = crate::bootstrap::install_initial_thread_stack_at(
                    memory,
                    entry_return,
                    reserve_size,
                    commit_size,
                    stack_top,
                )
                .map_err(RuntimeStateError::from_memory)?;
                threads
                    .register_stack(
                        thread_id,
                        registered_initial_thread_stack(thread_id, &stack, allocation.id),
                    )
                    .expect("thread availability was validated before stack installation");
                Ok(stack)
            },
        )
    }

    pub fn register_initial_thread_stack(
        &mut self,
        stack: &InitialThreadStack,
    ) -> Result<(), RuntimeStateError> {
        self.register_thread_stack(self.current_thread_id(), stack)
    }

    pub(crate) fn register_thread_stack(
        &mut self,
        thread_id: RuntimeThreadId,
        stack: &InitialThreadStack,
    ) -> Result<(), RuntimeStateError> {
        let thread = self.thread_state(thread_id)?;
        if !matches!(
            thread.lifecycle,
            RuntimeThreadLifecycleState::Initialized | RuntimeThreadLifecycleState::Running
        ) {
            return Err(RuntimeStateError::new(
                "runtime thread stack registration requires an initialized or running thread",
            ));
        }
        if thread.stack.is_some() {
            return Err(RuntimeStateError::new(
                "runtime thread stack may be registered only once",
            ));
        }
        validate_initial_thread_stack(stack)?;
        self.memory
            .validate_stack_owner(stack.owner, stack.stack_range, stack.committed_range)
            .map_err(RuntimeStateError::from_memory)?;

        let guard_pages = u32::from(stack.guard_page.is_some());
        let usable_byte_length = stack
            .stack_range
            .byte_length()
            .checked_sub(u64::from(guard_pages) * GUEST_PAGE_SIZE)
            .ok_or_else(|| {
                RuntimeStateError::new("thread stack guard exceeds its reserved range")
            })?;
        let reserved_range =
            RuntimeAddressRange::new(stack.stack_range.start(), stack.stack_range.byte_length())?;
        let allocation = self
            .address_space_allocator
            .reserve(ProcessAddressAllocationRequest {
                purpose: ProcessAddressAllocationPurpose::ThreadStack,

                preferred_address: Some(reserved_range.base),
                search_range: reserved_range,
                reserve_size: usable_byte_length,
                alignment: GUEST_PAGE_SIZE as u32,
                guard_pages,
                classification: ProcessAddressAllocationClassification::Stack,

                release_policy: ProcessAddressAllocationReleasePolicy::Explicit,
                executable: false,
            })?;
        // The search range equals the page-aligned reservation size, so a successful
        // reservation has exactly one possible range.
        assert_eq!(allocation.range, reserved_range);
        let registered = registered_initial_thread_stack(thread_id, stack, allocation.id);
        self.threads.register_stack(thread_id, registered).expect(
            "thread availability and constructed stack identity were validated before reservation",
        );
        Ok(())
    }

    fn grow_current_thread_stack(
        &mut self,
        fault_address: GuestAddress,
        page_fault_error_code: u32,
    ) -> Result<(), RuntimeStateError> {
        if !self.process.is_running()
            || self.current_thread()?.lifecycle != RuntimeThreadLifecycleState::Running
        {
            return Err(RuntimeStateError::new(
                "thread stack growth requires a running process and current thread",
            ));
        }
        let thread_id = self.current_thread_id();
        let stack = self.threads.stack_mut(thread_id)?;
        let guard_page = stack.guard_page.ok_or_else(|| {
            RuntimeStateError::new("current thread stack has committed its entire reserved range")
        })?;
        let fault_page = GuestAddress(fault_address.0 & !(GUEST_PAGE_SIZE as u32 - 1));
        if fault_page != guard_page.start() {
            return Err(RuntimeStateError::new(
                "page fault does not fall within the current thread stack guard page",
            ));
        }
        if page_fault_error_code & 1 != 0 {
            return Err(RuntimeStateError::new(
                "present-page protection faults cannot grow the current thread stack",
            ));
        }
        let stats = self.memory.stats();
        if stats.generation != stats.acknowledged_generation || stats.pending_invalidation.is_some()
        {
            return Err(RuntimeStateError::new(
                "thread stack growth requires fully acknowledged guest memory",
            ));
        }
        validate_runtime_thread_stack_allocation(&self.address_space_allocator, stack)?;
        let stack_region_matches = self.memory.regions().any(|region| {
            region.owner == stack.memory_owner
                && region.kind == MemoryRegionKind::Stack
                && region.range == stack.reserved_range
        });
        if !stack_region_matches {
            return Err(RuntimeStateError::new(
                "runtime stack metadata does not match the Rust guest-memory reservation",
            ));
        }
        self.memory
            .preflight_growth(0, 0, 1)
            .map_err(RuntimeStateError::from_memory)?;

        let next_committed_byte_length = stack
            .committed_range
            .byte_length()
            .checked_add(GUEST_PAGE_SIZE)
            .ok_or_else(|| {
                RuntimeStateError::new("current thread stack committed size overflowed")
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
                        RuntimeStateError::new(
                            "current thread stack guard underflowed its reserved range",
                        )
                    })
            })
            .transpose()?
            .map(|base| {
                GuestRange::new(base, GUEST_PAGE_SIZE).map_err(RuntimeStateError::from_memory)
            })
            .transpose()?;
        let mut transaction = self.memory.begin_transaction();
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
        stack.committed_range = committed_range;
        stack.guard_page = next_guard_page;
        transaction.commit_transaction();

        Ok(())
    }

    pub fn service_page_fault(
        &mut self,
        fault_address: GuestAddress,
        page_fault_error_code: u32,
    ) -> Result<(), RuntimeStateError> {
        let fault_page = GuestAddress(fault_address.0 & !(GUEST_PAGE_SIZE as u32 - 1));
        let stack_guard_matches = self
            .current_thread()?
            .stack
            .as_ref()
            .and_then(|stack| stack.guard_page)
            .is_some_and(|guard| guard.start() == fault_page);
        if stack_guard_matches {
            return self.grow_current_thread_stack(fault_address, page_fault_error_code);
        }
        if self.heaps.large_allocation_contains(fault_address) {
            return self
                .materialize_private_heap_page(fault_address, page_fault_error_code)
                .map(|_| ());
        }
        Err(RuntimeStateError::new(
            "page fault does not match a current stack guard or live lazy heap allocation",
        ))
    }

    pub fn thread_state(
        &self,
        thread_id: RuntimeThreadId,
    ) -> Result<&RuntimeThreadState, RuntimeStateError> {
        self.threads.state(thread_id)
    }

    fn validate_cpu_snapshot_thread(
        &self,
        thread_id: RuntimeThreadId,
    ) -> Result<(), RuntimeStateError> {
        if !self.process.is_running()
            || self.thread_state(thread_id)?.lifecycle != RuntimeThreadLifecycleState::Running
            || !matches!(
                self.scheduler_state_for_thread(thread_id)?,
                SchedulerThreadState::Suspended { .. }
                    | SchedulerThreadState::Runnable
                    | SchedulerThreadState::WaitingHost { .. }
                    | SchedulerThreadState::WaitingObject { .. }
                    | SchedulerThreadState::WaitingCriticalSection { .. }
                    | SchedulerThreadState::WaitingMessage
                    | SchedulerThreadState::InCallback { .. }
            )
            || !self.objects.contains_identity(&RuntimeObjectKey::Thread {
                thread_id: thread_id.0,
            })
        {
            return Err(RuntimeStateError::new(
                "CPU snapshot requires matching live process, thread, object and scheduler identity",
            ));
        }
        Ok(())
    }

    /// Park backend-owned bytes on the existing Windows thread identity. A
    /// revision belongs to this CPU handoff, not the mutable GuestMemory epoch.
    pub fn put_thread_cpu_snapshot(
        &mut self,
        thread_id: RuntimeThreadId,
        expected_revision: u64,
        backend_tag: u32,
        abi_version: u32,
        bytes: &[u8],
    ) -> Result<u64, RuntimeStateError> {
        self.validate_cpu_snapshot_thread(thread_id)?;
        self.threads.put_cpu_snapshot(
            thread_id,
            expected_revision,
            backend_tag,
            abi_version,
            bytes,
        )
    }

    /// Non-destructive read: a failed backend restore must not consume the only
    /// parked copy. Thread retirement discards it through its existing owner.
    pub fn thread_cpu_snapshot(
        &self,
        thread_id: RuntimeThreadId,
    ) -> Result<(u32, u32, u64, &[u8]), RuntimeStateError> {
        self.validate_cpu_snapshot_thread(thread_id)?;
        self.threads.cpu_snapshot(thread_id)
    }

    pub fn scheduler(&self) -> &RuntimeScheduler {
        &self.scheduler
    }

    pub(crate) fn module_graph(&self) -> &RuntimeModuleGraph {
        self.modules.graph()
    }

    pub(crate) fn module_images(&self) -> &RuntimeModuleImageRegistry {
        self.modules.images()
    }

    pub(crate) fn resolve_guest_export(
        &self,
        module: &str,
        symbol: ExportLookup,
    ) -> Result<Option<RuntimeResolvedExport>, RuntimeModuleImageError> {
        self.modules.resolve(module, symbol)
    }

    pub(crate) fn module_image_by_load_base(
        &self,
        load_base: GuestAddress,
    ) -> Result<Option<&RuntimeModuleImage>, RuntimeModuleImageError> {
        self.modules.images().image_by_load_base(load_base)
    }

    pub fn register_module_image(
        &mut self,
        handle: u32,
        loader: &Pe32LoadPlan,
        materialization: &ImageMaterialization,
    ) -> Result<&RuntimeModuleImage, RuntimeModuleImageError> {
        self.module(handle)
            .map_err(runtime_module_image_state_error)?;
        if loader.tls.is_none() {
            return self
                .modules
                .register_image(&self.memory, handle, loader, materialization);
        }
        let tebs = self
            .image_tls_tebs()
            .map_err(runtime_module_image_state_error)?;
        let mut modules = self.modules.clone();
        let mut tls = self.image_tls.clone();
        let mut allocator = self.address_space_allocator.clone();
        let mut memory = self.memory.begin_transaction();
        modules.register_image(&memory, handle, loader, materialization)?;
        tls.register_module(
            &mut memory,
            &mut allocator,
            handle,
            loader,
            materialization,
            &tebs,
        )
        .map_err(runtime_module_image_state_error)?;
        memory.commit_transaction();
        self.modules = modules;
        self.image_tls = tls;
        self.address_space_allocator = allocator;
        let id = self
            .modules
            .module_id_by_handle(handle)
            .map_err(runtime_module_image_state_error)?;
        self.modules.images().image(id)
    }

    pub fn bind_guest_module_imports(
        &mut self,
        handle: u32,
        loader: &Pe32LoadPlan,
        materialization: &ImageMaterialization,
        runtime_catalog: &RuntimeExportCatalog,
    ) -> Result<RuntimeGuestImportBinding, RuntimeStateError> {
        crate::dll::bind_guest_module_imports(
            self,
            handle,
            loader,
            materialization,
            runtime_catalog,
        )
    }

    pub fn install_process_environment(
        &mut self,
        command_line: &str,
    ) -> Result<InitialProcessEnvironment, RuntimeStateError> {
        if self.process.environment().is_some() {
            return Err(RuntimeStateError::new(
                "runtime process environment may be installed only once",
            ));
        }
        let command_line =
            crate::environment::preflight_process_environment(&self.memory, command_line)
                .map_err(RuntimeStateError::from_memory)?;
        let mut requests = vec![(
            None,
            crate::environment::controlled_process_environment_allocation_request(),
        )];
        requests.extend(self.image_tls.thread_requests()?);
        let thread_id = self.current_thread_id();
        let image_tls = &mut self.image_tls;
        let memory = &mut self.memory;
        let process = &mut self.process;
        self.address_space_allocator
            .reserve_batch_with(requests, |allocations| {
                let allocation = &allocations[0];
                let mut transaction = memory.begin_transaction();
                let environment =
                    crate::environment::install_controlled_process_environment_transaction(
                        &mut transaction,
                        allocation,
                        &command_line,
                    )
                    .map_err(RuntimeStateError::from_memory)?;
                let parameters = &environment.process_parameters;
                let process_environment = RuntimeProcessEnvironment {
                    initial_teb_address: environment.teb_address,
                    peb_address: environment.peb_address,
                    command_line: RuntimeCommandLine {
                        ansi_buffer_address: parameters.ansi.buffer_address,
                        ansi_byte_length_with_nul: parameters.ansi.byte_length_with_nul,
                        wide_buffer_address: parameters.command_line_buffer_address,
                        wide_byte_length_with_nul: u32::from(
                            parameters.command_line_maximum_length_bytes,
                        ),
                    },
                    environment_strings: RuntimeEnvironmentStrings {
                        authority_buffer_address: parameters
                            .environment_strings
                            .authority_buffer_address,
                        copy_buffer_address: parameters.environment_strings.copy_buffer_address,
                        byte_length_with_double_nul: parameters
                            .environment_strings
                            .byte_length_with_double_nul,
                        copy_active: false,
                    },
                };
                let tls = image_tls.initialize_thread(
                    &mut transaction,
                    &allocations[1..],
                    environment.teb_address,
                )?;
                process.install_environment(process_environment);
                image_tls.publish_thread(thread_id, tls);
                transaction.commit_transaction();
                Ok(environment)
            })
    }

    #[cfg(test)]
    pub(crate) fn module_import_bindings(
        &self,
    ) -> &BTreeMap<RuntimeModuleId, RuntimeGuestImportBinding> {
        self.modules.bindings()
    }

    #[cfg(test)]
    pub(crate) fn runtime_system_exports(
        &self,
    ) -> Option<&super::modules::RuntimeSystemExportTable> {
        self.modules.runtime_system_exports()
    }

    #[cfg(test)]
    pub(crate) fn process_handle(&self) -> Result<RuntimeHandle, RuntimeStateError> {
        self.objects.handle_for_object(&RuntimeObjectKey::Process {
            process_id: self.process.id().0,
        })
    }

    pub(crate) fn open_process_query_handle(
        &mut self,
        process_id: u32,
    ) -> Result<Option<RuntimeHandle>, RuntimeStateError> {
        if process_id != self.process.id().0 {
            return Ok(None);
        }
        self.objects
            .open_handle(&RuntimeObjectKey::Process { process_id })
            .map(Some)
    }

    pub(crate) fn is_current_process_query_handle(
        &self,
        handle: RuntimeHandle,
    ) -> Result<bool, RuntimeStateError> {
        if handle.0 == u32::MAX {
            return Ok(true);
        }
        let Some(record) = self
            .objects
            .live_object_for_handle(handle, RuntimeObjectKind::Process)?
        else {
            return Ok(false);
        };
        // OpenProcess currently publishes only handles with process query access.
        Ok(record.key
            == RuntimeObjectKey::Process {
                process_id: self.process.id().0,
            })
    }

    #[cfg(test)]
    pub(crate) fn current_thread_handle(&self) -> Result<RuntimeHandle, RuntimeStateError> {
        self.thread_handle(self.current_thread_id())
    }

    pub(crate) fn current_thread_pseudo_handle(&self) -> Result<u32, RuntimeStateError> {
        self.thread_object_for_handle(RuntimeHandle(RUNTIME_CURRENT_THREAD_PSEUDO_HANDLE))?;
        Ok(RUNTIME_CURRENT_THREAD_PSEUDO_HANDLE)
    }

    #[cfg(test)]
    pub(crate) fn thread_handle(
        &self,
        thread_id: RuntimeThreadId,
    ) -> Result<RuntimeHandle, RuntimeStateError> {
        self.thread_state(thread_id)?;
        self.objects.handle_for_object(&RuntimeObjectKey::Thread {
            thread_id: thread_id.0,
        })
    }

    #[cfg(test)]
    pub(crate) fn open_current_thread_handle(
        &mut self,
    ) -> Result<RuntimeHandle, RuntimeStateError> {
        self.open_thread_handle(self.current_thread_id())
    }

    #[cfg(test)]
    pub(crate) fn open_thread_handle(
        &mut self,
        thread_id: RuntimeThreadId,
    ) -> Result<RuntimeHandle, RuntimeStateError> {
        self.thread_state(thread_id)?;
        self.objects.open_handle(&RuntimeObjectKey::Thread {
            thread_id: thread_id.0,
        })
    }

    fn thread_object_for_handle(
        &self,
        handle: RuntimeHandle,
    ) -> Result<Option<&RuntimeObjectRecord>, RuntimeStateError> {
        let record = if handle.0 == RUNTIME_CURRENT_THREAD_PSEUDO_HANDLE {
            self.objects
                .object_for_identity(&RuntimeObjectKey::Thread {
                    thread_id: self.current_thread_id().0,
                })?
        } else {
            let Some(record) = self
                .objects
                .live_object_for_handle(handle, RuntimeObjectKind::Thread)?
            else {
                return Ok(None);
            };
            record
        };
        let RuntimeObjectKey::Thread { thread_id } = record.key else {
            return Err(RuntimeStateError::new(
                "thread handle resolved to a different object identity",
            ));
        };
        self.threads.state(RuntimeThreadId(thread_id))?;
        Ok(Some(record))
    }

    pub(crate) fn thread_exit_code(
        &self,
        handle: RuntimeHandle,
    ) -> Result<Option<u32>, RuntimeStateError> {
        const STILL_ACTIVE: u32 = 259;
        let Some(record) = self.thread_object_for_handle(handle)? else {
            return Ok(None);
        };
        Ok(Some(match record.wait_state {
            Some(RuntimeWaitState::Nonsignaled) => STILL_ACTIVE,
            Some(RuntimeWaitState::Signaled { exit_code }) => exit_code,
            None => {
                return Err(RuntimeStateError::new("thread object has no wait state"));
            }
        }))
    }

    pub(crate) fn is_live_object_handle(
        &self,
        handle: RuntimeHandle,
        kind: RuntimeObjectKind,
    ) -> Result<bool, RuntimeStateError> {
        self.objects.is_live_handle(handle, kind)
    }

    #[cfg(test)]
    pub(crate) fn object_for_handle(
        &self,
        handle: RuntimeHandle,
        expected_kind: RuntimeObjectKind,
    ) -> Result<&RuntimeObjectRecord, RuntimeStateError> {
        self.objects.object_for_handle(handle, expected_kind)
    }

    pub(crate) fn close_handle(&mut self, handle: RuntimeHandle) -> Result<(), RuntimeStateError> {
        match handle.kind()? {
            RuntimeObjectKind::Mutex => {
                let record = self
                    .objects
                    .object_for_handle(handle, RuntimeObjectKind::Mutex)?;
                let RuntimeObjectKey::Mutex { mutex_id } = record.key else {
                    return Err(RuntimeStateError::new(
                        "mutex handle resolved to a non-mutex object identity",
                    ));
                };
                let retire = record.open_handles == 1;
                if retire {
                    self.synchronization
                        .validate_mutex_retirement(RuntimeMutexId(mutex_id))?;
                }
                self.objects.close_payload_handle(handle)?;
                if retire {
                    self.synchronization.retire_mutex(RuntimeMutexId(mutex_id));
                }
                Ok(())
            }
            RuntimeObjectKind::WaitableTimer => {
                let record = self
                    .objects
                    .object_for_handle(handle, RuntimeObjectKind::WaitableTimer)?;
                if self
                    .scheduler
                    .object_waiters()
                    .get(&record.id)
                    .is_some_and(|waiters| !waiters.is_empty())
                {
                    return Err(RuntimeStateError::new(
                        "bounded waitable timer pacing does not close a timer with a pending wait",
                    ));
                }
                let RuntimeObjectKey::WaitableTimer { timer_id } = record.key else {
                    return Err(RuntimeStateError::new(
                        "waitable timer handle resolved to a different object identity",
                    ));
                };
                let retire = record.open_handles == 1;
                if retire {
                    self.timers
                        .waitable_timer(RuntimeWaitableTimerId(timer_id))?;
                }
                self.objects.close_payload_handle(handle)?;
                if retire {
                    self.timers
                        .retire_waitable_timer(RuntimeWaitableTimerId(timer_id));
                }
                Ok(())
            }
            RuntimeObjectKind::Event => {
                let record = self
                    .objects
                    .object_for_handle(handle, RuntimeObjectKind::Event)?;
                if self
                    .scheduler
                    .object_waiters()
                    .get(&record.id)
                    .is_some_and(|waiters| !waiters.is_empty())
                {
                    return Err(RuntimeStateError::new(
                        "bounded event handling does not close an event with a pending wait",
                    ));
                }
                self.objects.close_payload_handle(handle)
            }
            RuntimeObjectKind::File => {
                let record = self
                    .objects
                    .object_for_handle(handle, RuntimeObjectKind::File)?;
                let RuntimeObjectKey::File { file_id } = record.key else {
                    return Err(RuntimeStateError::new(
                        "source file handle resolved to a different object identity",
                    ));
                };
                if self
                    .pending_opfs_writes
                    .values()
                    .any(|pending| pending.file_id.0 == file_id)
                {
                    return Err(RuntimeStateError::new(
                        "file cannot close while an OPFS write is pending",
                    ));
                }
                let retire = record.open_handles == 1;
                if retire {
                    self.files
                        .validate_retirement(RuntimeSourceFileId(file_id))?;
                }
                self.objects.close_payload_handle(handle)?;
                if retire {
                    self.files.retire(RuntimeSourceFileId(file_id));
                }
                Ok(())
            }
            RuntimeObjectKind::Thread => {
                self.objects
                    .object_for_handle(handle, RuntimeObjectKind::Thread)?;
                if self
                    .scheduler
                    .wait_registrations()
                    .values()
                    .any(|registration| {
                        registration.handle == handle && registration.object_wait_pacing.is_some()
                    })
                {
                    return Err(RuntimeStateError::new(
                        "finite thread wait retains its live handle until completion",
                    ));
                }
                self.objects.close_handle(handle)
            }
            RuntimeObjectKind::Process
            | RuntimeObjectKind::FileSearch
            | RuntimeObjectKind::PrivateHeap
            | RuntimeObjectKind::Module => self.objects.close_handle(handle),
        }
    }

    pub(crate) fn object_wait_state(
        &self,
        handle: RuntimeHandle,
        expected_kind: RuntimeObjectKind,
    ) -> Result<RuntimeWaitState, RuntimeStateError> {
        self.objects.wait_state(handle, expected_kind)
    }

    #[cfg(test)]
    pub(crate) fn allocated_tls_indices(&self) -> &BTreeSet<u32> {
        self.threads.allocated_tls_indices()
    }

    pub(crate) fn fls_index_allocated(&self, index: u32) -> bool {
        self.threads.fls_slot(index).is_ok()
    }

    pub(crate) fn clock(&self) -> &RuntimeClockState {
        self.process.clock()
    }

    #[cfg(test)]
    pub(crate) fn advance_scheduled_clock_to(
        &mut self,
        target_time_100ns: u64,
    ) -> Result<u64, RuntimeStateError> {
        self.require_timer_operation_state()?;
        self.process.advance_clock(target_time_100ns)
    }

    pub fn synchronize_scheduled_clock_for_host_execution(
        &mut self,
        retired_instructions: u64,
        host_elapsed_time_100ns: u64,
    ) -> Result<u64, RuntimeStateError> {
        // Host time is process-wide, including when all live threads are parked
        // or the selected thread just exited and made another owner's wait ready.
        // This observation neither executes nor revives the selected thread.
        if !self.process.is_running()
            || (self.current_thread()?.lifecycle != RuntimeThreadLifecycleState::Running
                && !self.threads.has_running_thread())
        {
            return Err(RuntimeStateError::new(
                "host clock observation requires a running process with a live thread",
            ));
        }
        self.process
            .advance_scheduled_host_execution_clock(retired_instructions, host_elapsed_time_100ns)
    }

    /// Observe the real process-wide host clock and read the fixed-profile TSC.
    /// The retirement cursor validates ordering only; it never advances time.
    /// A CPU must stop before RDTSC and reconcile its preceding guest writes
    /// before this boundary, then commit its architectural result separately.
    pub fn read_timestamp_counter(
        &mut self,
        retired_instructions: u64,
        host_elapsed_time_100ns: u64,
    ) -> Result<u64, RuntimeStateError> {
        if !self.process.is_running()
            || self.current_thread()?.lifecycle != RuntimeThreadLifecycleState::Running
            || !matches!(
                self.scheduler_state()?,
                SchedulerThreadState::Runnable | SchedulerThreadState::InCallback { .. }
            )
        {
            return Err(RuntimeStateError::new(
                "timestamp counter requires a running process and active current thread",
            ));
        }
        self.process.advance_scheduled_host_execution_clock(
            retired_instructions,
            host_elapsed_time_100ns,
        )?;
        Ok(self.process.timestamp_counter())
    }

    pub(crate) fn sample_tick_count(&self) -> Result<u32, RuntimeStateError> {
        self.process.sample_tick_count()
    }

    pub(crate) fn sample_performance_counter(&self) -> Result<u64, RuntimeStateError> {
        self.process.sample_performance_counter()
    }

    pub(crate) fn startup_info(&self) -> &RuntimeStartupInfo {
        self.process.startup_info()
    }

    pub fn current_last_error(&self) -> Result<u32, RuntimeStateError> {
        self.threads.last_error(self.current_thread_id())
    }

    pub fn set_current_last_error(&mut self, value: u32) -> Result<(), RuntimeStateError> {
        self.threads.set_last_error(self.current_thread_id(), value)
    }

    pub(crate) fn current_com_apartment(
        &self,
    ) -> Result<Option<&RuntimeComApartmentState>, RuntimeStateError> {
        self.threads.com_apartment(self.current_thread_id())
    }

    pub(crate) fn initialize_current_com_apartment(
        &mut self,
        model: RuntimeComApartmentModel,
    ) -> Result<crate::com::ComHResult, RuntimeStateError> {
        self.threads
            .initialize_com_apartment(self.current_thread_id(), model)
    }

    pub(crate) fn uninitialize_current_com_apartment(&mut self) -> Result<(), RuntimeStateError> {
        self.threads
            .uninitialize_com_apartment(self.current_thread_id())
    }

    #[cfg(test)]
    pub(crate) fn thread_priority(
        &self,
        thread_id: RuntimeThreadId,
    ) -> Result<RuntimeThreadPriority, RuntimeStateError> {
        self.threads.effective_priority(thread_id)
    }

    pub(crate) fn set_thread_affinity_mask(
        &self,
        handle: RuntimeHandle,
        requested_mask: u32,
    ) -> Result<Result<u32, u32>, RuntimeStateError> {
        const ERROR_INVALID_HANDLE: u32 = 6;
        const ERROR_INVALID_PARAMETER: u32 = 87;
        const PROCESS_MASK: u32 = 1;
        let Some(object) = self.thread_object_for_handle(handle)? else {
            return Ok(Err(ERROR_INVALID_HANDLE));
        };
        let RuntimeObjectKey::Thread { thread_id } = object.key else {
            return Err(RuntimeStateError::new(
                "thread affinity resolved to a different object identity",
            ));
        };
        if self.threads.state(RuntimeThreadId(thread_id))?.lifecycle
            != RuntimeThreadLifecycleState::Running
        {
            return Ok(Err(ERROR_INVALID_HANDLE));
        }
        if requested_mask != PROCESS_MASK {
            return Ok(Err(ERROR_INVALID_PARAMETER));
        }
        Ok(Ok(PROCESS_MASK))
    }

    pub(crate) fn thread_priority_for_handle(
        &self,
        handle: RuntimeHandle,
    ) -> Result<Option<RuntimeThreadPriority>, RuntimeStateError> {
        let Some(object) = self.thread_object_for_handle(handle)? else {
            return Ok(None);
        };
        let RuntimeObjectKey::Thread { thread_id } = object.key else {
            return Err(RuntimeStateError::new(
                "priority query resolved to a different object identity",
            ));
        };
        self.threads
            .effective_priority(RuntimeThreadId(thread_id))
            .map(Some)
    }

    pub(crate) fn set_thread_priority(
        &mut self,
        handle: RuntimeHandle,
        requested_priority: RuntimeThreadPriority,
    ) -> Result<bool, RuntimeStateError> {
        let Some(object) = self.thread_object_for_handle(handle)? else {
            return Ok(false);
        };
        let RuntimeObjectKey::Thread { thread_id } = object.key else {
            return Err(RuntimeStateError::new(
                "thread handle resolved to a different object identity",
            ));
        };
        self.threads
            .set_priority(RuntimeThreadId(thread_id), requested_priority)
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

    fn resumed_execution_state_for_thread(
        &self,
        thread_id: RuntimeThreadId,
    ) -> Result<SchedulerThreadState, RuntimeStateError> {
        let callback_depth = self.threads.callback_depth(thread_id)?;
        Ok(if callback_depth == 0 {
            SchedulerThreadState::Runnable
        } else {
            SchedulerThreadState::InCallback {
                depth: callback_depth,
            }
        })
    }

    pub fn start_initial_thread(&mut self) -> Result<(), RuntimeStateError> {
        let thread_id = self.current_thread_id();
        self.threads.validate_start(thread_id)?;
        if self.scheduler_state()? != SchedulerThreadState::Initialized {
            return Err(RuntimeStateError::new(
                "initial thread start requires matching process, thread, and scheduler bootstrap states",
            ));
        }
        self.process.start()?;
        // No callback or host handoff occurs between validation and these owner updates.
        self.threads
            .start(thread_id)
            .expect("validated initial thread");
        self.scheduler.start_initial_thread(thread_id);
        Ok(())
    }

    pub fn register_thread(
        &mut self,
        thread_id: RuntimeThreadId,
        initial_last_error: u32,
    ) -> Result<RuntimeHandle, RuntimeStateError> {
        if !self.process.is_running() {
            return Err(RuntimeStateError::new(
                "thread registration requires a running process",
            ));
        }
        let thread_registered = self.threads.contains(thread_id);
        let scheduler_registered = self.scheduler.contains_thread(thread_id);
        if thread_registered != scheduler_registered {
            return Err(RuntimeStateError::new(
                "thread and scheduler registries disagree about a thread identity",
            ));
        }
        self.threads.validate_registration(thread_id)?;
        self.scheduler.validate_registration(thread_id)?;
        self.messages.validate_registration(thread_id)?;
        let thread_key = RuntimeObjectKey::Thread {
            thread_id: thread_id.0,
        };
        if self.objects.contains_identity(&thread_key) {
            return Err(RuntimeStateError::new(
                "thread object identity exists without a thread registry record",
            ));
        }
        // The object handle is the only fallible allocation. No owner changes until it succeeds.
        let (_, handle) = self.objects.register_object_and_open_handle(thread_key)?;
        self.threads.register(thread_id, initial_last_error);
        self.scheduler
            .register(thread_id, SchedulerThreadState::Runnable);
        self.messages.register(thread_id);
        Ok(handle)
    }

    pub(crate) fn create_guest_thread(
        &mut self,
        request: RuntimeCreateThreadRequest,
    ) -> Result<Option<(RuntimeThreadId, RuntimeHandle)>, RuntimeStateError> {
        self.memory
            .validate_access(request.entry_point, 1, MemoryAccess::Execute)
            .map_err(RuntimeStateError::from_memory)?;
        if let Some(output) = request.thread_id_output {
            self.memory
                .validate_access(output, 4, MemoryAccess::Write)
                .map_err(RuntimeStateError::from_memory)?;
        }
        self.create_guest_thread_inner(request)
    }

    /// A freshly installed Rust COM endpoint has not crossed the CPU delta
    /// handoff yet. Only that live internal stream identity may use host access.
    pub(crate) fn create_media_guest_thread(
        &mut self,
        request: RuntimeCreateThreadRequest,
        object_id: RuntimeComObjectId,
    ) -> Result<Option<(RuntimeThreadId, RuntimeHandle)>, RuntimeStateError> {
        let target = self
            .resolve_com_method(request.entry_point)?
            .ok_or_else(|| {
                RuntimeStateError::new("media thread entry is not a live COM endpoint")
            })?;
        if target.object_id != object_id
            || target.class != RuntimeComClass::DirectShowStream
            || target.method.slot != 3
            || target.guest_pointer.0 != request.parameter
            || request.thread_id_output.is_some()
        {
            return Err(RuntimeStateError::new(
                "media thread entry lost its owned stream identity",
            ));
        }
        self.create_guest_thread_inner(request)
    }

    pub(crate) fn create_multimedia_timer_thread(
        &mut self,
        request: RuntimeCreateThreadRequest,
        owner: MemoryOwnerId,
        allocation: ProcessAddressAllocationId,
    ) -> Result<Option<(RuntimeThreadId, RuntimeHandle)>, RuntimeStateError> {
        let record = self.address_space_allocator.allocation(allocation)?;
        if record.purpose != ProcessAddressAllocationPurpose::MultimediaTimer
            || record.range.base != request.entry_point
            || request.parameter != request.entry_point.0 + 4096
            || request.thread_id_output.is_some()
        {
            return Err(RuntimeStateError::new(
                "timer thread does not match its owned code and cancellation word",
            ));
        }
        self.memory
            .validate_owned_host_execute(owner, request.entry_point, 1)
            .map_err(RuntimeStateError::from_memory)?;
        self.create_guest_thread_inner(request)
    }

    fn create_guest_thread_inner(
        &mut self,
        request: RuntimeCreateThreadRequest,
    ) -> Result<Option<(RuntimeThreadId, RuntimeHandle)>, RuntimeStateError> {
        if !self.process.is_running()
            || self.current_thread()?.lifecycle != RuntimeThreadLifecycleState::Running
            || !self.scheduler.current_state()?.is_executable()
        {
            return Err(RuntimeStateError::new(
                "guest thread creation requires a running process and executable caller thread",
            ));
        }
        validate_guest_thread_stack_sizes(request.stack_reserve_bytes, request.stack_commit_bytes)?;
        self.memory
            .validate_host_execute(request.entry_point, 1)
            .map_err(RuntimeStateError::from_memory)?;
        if let Some(output) = request.thread_id_output {
            self.memory
                .validate_host_write(output, 4)
                .map_err(RuntimeStateError::from_memory)?;
        }

        let process_environment = self.process.environment().ok_or_else(|| {
            RuntimeStateError::new(
                "guest thread creation requires the registered process PEB authority",
            )
        })?;
        let return_sentinel = self
            .current_thread()?
            .stack
            .as_ref()
            .map(|stack| stack.return_sentinel)
            .ok_or_else(|| {
                RuntimeStateError::new(
                    "guest thread creation requires the process entry return sentinel",
                )
            })?;
        self.memory
            .validate_host_execute(return_sentinel, 1)
            .map_err(RuntimeStateError::from_memory)?;

        let Some(thread_id) = self.threads.next_guest_thread_id()? else {
            return Ok(None);
        };
        let guard_pages = u32::from(request.stack_reserve_bytes > request.stack_commit_bytes);
        let stack_usable_bytes = request
            .stack_reserve_bytes
            .checked_sub(u64::from(guard_pages) * GUEST_PAGE_SIZE)
            .ok_or_else(|| {
                RuntimeStateError::new("guest thread stack reserve cannot contain its growth guard")
            })?;
        let memory = &mut self.memory;
        let threads = &mut self.threads;
        let scheduler = &mut self.scheduler;
        let messages = &mut self.messages;
        let objects = &mut self.objects;
        let process_id = self.process.id();
        let image_tls = &mut self.image_tls;
        let mut requests = vec![
            (
                None,
                ProcessAddressAllocationRequest {
                    purpose: ProcessAddressAllocationPurpose::ThreadStack,

                    preferred_address: None,
                    search_range: RuntimeAddressRange::new(
                        RUNTIME_GUEST_THREAD_STACK_ARENA_BASE,
                        RUNTIME_GUEST_THREAD_STACK_ARENA_SIZE,
                    )?,
                    reserve_size: stack_usable_bytes,
                    alignment: GUEST_PAGE_SIZE as u32,
                    guard_pages,
                    classification: ProcessAddressAllocationClassification::Stack,

                    release_policy: ProcessAddressAllocationReleasePolicy::Explicit,
                    executable: false,
                },
            ),
            (
                None,
                ProcessAddressAllocationRequest {
                    purpose: ProcessAddressAllocationPurpose::ThreadEnvironment,

                    preferred_address: None,
                    search_range: RuntimeAddressRange::new(
                        RUNTIME_GUEST_THREAD_ENVIRONMENT_ARENA_BASE,
                        RUNTIME_GUEST_THREAD_ENVIRONMENT_ARENA_SIZE,
                    )?,
                    reserve_size: GUEST_PAGE_SIZE,
                    alignment: GUEST_PAGE_SIZE as u32,
                    guard_pages: 0,
                    classification: ProcessAddressAllocationClassification::ProcessEnvironment,

                    release_policy: ProcessAddressAllocationReleasePolicy::Explicit,
                    executable: false,
                },
            ),
        ];
        requests.extend(image_tls.thread_requests()?);
        self.address_space_allocator
            .try_reserve_batch_with(requests, |allocations| {
                let [
                    stack_allocation,
                    environment_allocation,
                    tls_allocations @ ..,
                ] = allocations
                else {
                    return Err(RuntimeStateError::new(
                        "guest thread address plan did not contain stack and TEB allocations",
                    ));
                };
                if stack_allocation.range.byte_length != request.stack_reserve_bytes
                    || environment_allocation.range.byte_length != GUEST_PAGE_SIZE
                {
                    return Err(RuntimeStateError::new(
                        "guest thread address allocator changed the requested stack or TEB size",
                    ));
                }

                let stack_top = GuestAddress(
                    u32::try_from(stack_allocation.range.end_exclusive()?).map_err(|_| {
                        RuntimeStateError::new(
                            "planned guest thread stack top does not fit an x86 address",
                        )
                    })?,
                );
                let commit_bytes_u32 = u32::try_from(request.stack_commit_bytes).map_err(|_| {
                    RuntimeStateError::new(
                        "guest thread stack commit does not fit x86 address accounting",
                    )
                })?;
                let committed_base =
                    GuestAddress(stack_top.0.checked_sub(commit_bytes_u32).ok_or_else(|| {
                        RuntimeStateError::new(
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
                let initial_stack_pointer =
                    GuestAddress(stack_top.0.checked_sub(8).ok_or_else(|| {
                        RuntimeStateError::new("guest thread stack cannot contain its entry frame")
                    })?);
                let guard_page = (guard_pages != 0)
                    .then(|| {
                        committed_base
                            .0
                            .checked_sub(GUEST_PAGE_SIZE as u32)
                            .map(GuestAddress)
                            .ok_or_else(|| {
                                RuntimeStateError::new(
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
                        RuntimeStateError::new(
                            "guest thread committed stack pages do not fit host accounting",
                        )
                    })?;

                if memory
                    .preflight_growth(
                        2,
                        2,
                        committed_pages.checked_add(1).ok_or_else(|| {
                            RuntimeStateError::new(
                                "guest thread physical page count overflowed host accounting",
                            )
                        })?,
                    )
                    .is_err()
                {
                    return Ok(None);
                }
                memory
                    .preflight_fixed_reservation(
                        reserved_range.start(),
                        reserved_range.byte_length(),
                    )
                    .map_err(RuntimeStateError::from_memory)?;
                memory
                    .preflight_fixed_reservation(
                        environment_range.start(),
                        environment_range.byte_length(),
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
                    .copy_from_slice(&process_id.0.to_le_bytes());
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

                let tls =
                    image_tls.initialize_thread(&mut transaction, tls_allocations, teb_address)?;
                let stack = RuntimeThreadStack {
                    thread_id,
                    address_allocation_id: stack_allocation.id,
                    memory_owner: stack_owner,
                    reserved_range,
                    committed_range,
                    initial_commit_bytes: request.stack_commit_bytes,
                    guard_page,
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
                threads.validate_guest_registration(thread_id, &stack, &environment, &bootstrap)?;
                let scheduler_state = if request.initial_suspend_count == 0 {
                    SchedulerThreadState::Runnable
                } else {
                    SchedulerThreadState::Suspended {
                        count: request.initial_suspend_count,
                    }
                };
                scheduler.validate_registration(thread_id)?;
                messages.validate_registration(thread_id)?;
                if let Some(output) = request.thread_id_output {
                    transaction
                        .write(output, &thread_id.0.to_le_bytes())
                        .map_err(RuntimeStateError::from_memory)?;
                }

                let Some((_, handle)) =
                    objects.try_register_object_and_open_handle(RuntimeObjectKey::Thread {
                        thread_id: thread_id.0,
                    })?
                else {
                    return Ok(None);
                };

                image_tls.publish_thread(thread_id, tls);
                threads.register_guest(thread_id, 0, stack, environment, bootstrap);
                scheduler.register(thread_id, scheduler_state);
                messages.register(thread_id);
                transaction.commit_transaction();

                Ok(Some((thread_id, handle)))
            })
    }

    pub(crate) fn resume_thread(
        &mut self,
        handle: RuntimeHandle,
    ) -> Result<u32, RuntimeStateError> {
        let Some(object) = self.thread_object_for_handle(handle)? else {
            return Ok(u32::MAX);
        };
        let RuntimeObjectKey::Thread { thread_id } = object.key else {
            return Err(RuntimeStateError::new(
                "thread handle resolved to a different object identity",
            ));
        };
        let thread_id = RuntimeThreadId(thread_id);
        let thread = self.threads.state(thread_id)?;
        if thread.lifecycle != RuntimeThreadLifecycleState::Running {
            return Ok(u32::MAX);
        }
        self.scheduler.resume_thread(thread_id)
    }

    pub(crate) fn suspend_thread(
        &mut self,
        handle: RuntimeHandle,
    ) -> Result<Result<u32, u32>, RuntimeStateError> {
        let Some(object) = self.thread_object_for_handle(handle)? else {
            return Ok(Err(6));
        };
        let RuntimeObjectKey::Thread { thread_id } = object.key else {
            return Err(RuntimeStateError::new(
                "thread handle resolved to a different object identity",
            ));
        };
        let thread_id = RuntimeThreadId(thread_id);
        if self.threads.state(thread_id)?.lifecycle != RuntimeThreadLifecycleState::Running {
            return Ok(Err(5));
        }
        self.scheduler.suspend_thread(thread_id)
    }

    pub fn select_thread(&mut self, thread_id: RuntimeThreadId) -> Result<(), RuntimeStateError> {
        if !self.process.is_running() {
            return Err(RuntimeStateError::new(
                "thread selection requires a running process",
            ));
        }
        let thread = self.thread_state(thread_id)?;
        let scheduler_state = self.scheduler_state_for_thread(thread_id)?;
        if thread.lifecycle != RuntimeThreadLifecycleState::Running
            || !matches!(
                scheduler_state,
                SchedulerThreadState::Runnable
                    | SchedulerThreadState::Suspended { .. }
                    | SchedulerThreadState::WaitingHost { .. }
                    | SchedulerThreadState::WaitingObject { .. }
                    | SchedulerThreadState::WaitingCriticalSection { .. }
                    | SchedulerThreadState::WaitingMessage
                    | SchedulerThreadState::InCallback { .. }
            )
        {
            return Err(RuntimeStateError::new(
                "thread selection requires matching live thread and scheduler states",
            ));
        }
        let thread_key = RuntimeObjectKey::Thread {
            thread_id: thread_id.0,
        };
        if !self.objects.contains_identity(&thread_key) {
            return Err(RuntimeStateError::new(
                "registered thread has no runtime object identity",
            ));
        }
        self.scheduler.select_thread(thread_id)
    }

    pub fn select_next_executable_thread(
        &mut self,
    ) -> Result<Option<RuntimeThreadId>, RuntimeStateError> {
        if !self.process.is_running() {
            return Err(RuntimeStateError::new(
                "thread selection requires a running process",
            ));
        }
        let mut wake = Vec::new();
        for (&thread_id, &state) in self.scheduler.thread_states() {
            if state == SchedulerThreadState::WaitingMessage
                && self.has_new_thread_message(thread_id)?
            {
                wake.push((
                    thread_id,
                    self.resumed_execution_state_for_thread(thread_id)?,
                ));
            }
        }
        for (thread_id, state) in wake {
            self.observe_thread_message_state(thread_id);
            self.scheduler.set_validated_state(thread_id, state);
            self.scheduler
                .commit_event(RuntimeSchedulerEvent::MessageReady(thread_id));
        }
        let Some((thread_id, _)) = self.preferred_executable_thread(true)? else {
            return Ok(None);
        };
        let state = self.scheduler_state_for_thread(thread_id)?;
        if !state.is_executable()
            || self.thread_state(thread_id)?.lifecycle != RuntimeThreadLifecycleState::Running
        {
            return Err(RuntimeStateError::new(
                "scheduler selected a thread without matching executable runtime state",
            ));
        }
        let thread_key = RuntimeObjectKey::Thread {
            thread_id: thread_id.0,
        };
        if !self.objects.contains_identity(&thread_key) {
            return Err(RuntimeStateError::new(
                "selected executable thread has no runtime object identity",
            ));
        }
        self.scheduler.select_thread(thread_id)?;
        Ok(Some(thread_id))
    }

    #[cfg(test)]
    pub(crate) fn complete_api_execution_boundary(
        &mut self,
    ) -> Result<Option<RuntimeThreadId>, RuntimeStateError> {
        self.complete_api_execution_boundary_with_disposition(
            RuntimeApiBoundaryDisposition::ContinueQuantum,
        )
    }

    pub fn complete_api_execution_boundary_with_disposition(
        &mut self,
        disposition: RuntimeApiBoundaryDisposition,
    ) -> Result<Option<RuntimeThreadId>, RuntimeStateError> {
        if matches!(
            disposition,
            RuntimeApiBoundaryDisposition::TerminateCurrentThread
                | RuntimeApiBoundaryDisposition::TerminateProcess
        ) {
            return Err(RuntimeStateError::new(
                "a terminating API boundary must commit through the thread lifecycle owner",
            ));
        }
        if !self.process.is_running()
            || self.current_thread()?.lifecycle != RuntimeThreadLifecycleState::Running
            || !self.scheduler_state()?.is_executable()
        {
            return Err(RuntimeStateError::new(
                "an API execution boundary requires a running process and executable current thread",
            ));
        }
        let current_key = RuntimeObjectKey::Thread {
            thread_id: self.current_thread_id().0,
        };
        if !self.objects.contains_identity(&current_key) {
            return Err(RuntimeStateError::new(
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
            (
                RuntimeApiBoundaryDisposition::TerminateCurrentThread
                | RuntimeApiBoundaryDisposition::TerminateProcess,
                _,
            ) => unreachable!(),
        };
        if let Some(thread_id) = competitor {
            let state = self.scheduler_state_for_thread(thread_id)?;
            if !state.is_executable()
                || self.thread_state(thread_id)?.lifecycle != RuntimeThreadLifecycleState::Running
            {
                return Err(RuntimeStateError::new(
                    "API boundary selected a thread without matching executable runtime state",
                ));
            }
            let thread_key = RuntimeObjectKey::Thread {
                thread_id: thread_id.0,
            };
            if !self.objects.contains_identity(&thread_key) {
                return Err(RuntimeStateError::new(
                    "API boundary selected a thread without runtime object identity",
                ));
            }
        }
        self.scheduler
            .complete_api_boundary(disposition, competitor, force_competitor)
    }

    fn preferred_executable_thread(
        &self,
        include_current: bool,
    ) -> Result<Option<(RuntimeThreadId, RuntimeThreadPriority)>, RuntimeStateError> {
        if let Some(owner) = self.process.exit_owner {
            return Ok(
                if include_current && self.scheduler_state_for_thread(owner)?.is_executable() {
                    Some((owner, self.threads.effective_priority(owner)?))
                } else {
                    None
                },
            );
        }
        let thread_states = self.scheduler.thread_states();
        let current_thread_id = self.current_thread_id();
        if !thread_states.contains_key(&current_thread_id) {
            return Err(RuntimeStateError::new(
                "current runtime thread has no scheduler lane",
            ));
        }
        let mut preferred = None;
        let after_current = thread_states.range((
            std::ops::Bound::Excluded(current_thread_id),
            std::ops::Bound::Unbounded,
        ));
        for (&thread_id, state) in after_current.chain(thread_states.range(..=current_thread_id)) {
            if (!include_current && thread_id == current_thread_id) || !state.is_executable() {
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
            || !self.scheduler_state()?.is_executable()
        {
            return Err(RuntimeStateError::new(
                "wait registration requires a running process and executable current thread",
            ));
        }
        let thread_id = self.current_thread_id();
        if self
            .scheduler
            .wait_registration_for_thread(thread_id)?
            .is_some()
        {
            return Err(RuntimeStateError::new(
                "runnable thread already owns a wait registration",
            ));
        }
        let kind = handle.kind()?;
        let object = self.objects.object_for_handle(handle, kind)?;
        let object_id = object.id;
        if kind == RuntimeObjectKind::Mutex {
            let RuntimeObjectKey::Mutex { mutex_id } = &object.key else {
                return Err(RuntimeStateError::new(
                    "mutex wait resolved to a non-mutex object identity",
                ));
            };
            let mutex_id = RuntimeMutexId(*mutex_id);
            let (recursion_count, abandoned) =
                self.synchronization.acquire_mutex(mutex_id, thread_id)?;
            return Ok(RuntimeWaitDisposition::MutexAcquired {
                object_id,
                mutex_id,
                recursion_count,
                abandoned,
            });
        }
        let wait_state = object.wait_state.ok_or_else(|| {
            RuntimeStateError::new("runtime object type does not expose a wait state")
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
                self.objects
                    .set_wait_state(handle, kind, RuntimeWaitState::Nonsignaled)?;
            }
            return Ok(RuntimeWaitDisposition::AlreadySignaled {
                object_id,
                wait_state,
            });
        }
        let registration = self.scheduler.register_wait(thread_id, object_id, handle)?;
        self.scheduler.set_validated_state(
            thread_id,
            SchedulerThreadState::WaitingObject {
                registration_id: registration.id,
                object_id,
            },
        );
        Ok(RuntimeWaitDisposition::Pending { registration })
    }

    pub(crate) fn begin_finite_object_wait(
        &mut self,
        handle: RuntimeHandle,
        timeout_milliseconds: u32,
    ) -> Result<RuntimeWaitForSingleObjectExBegin, RuntimeStateError> {
        if !self.process.is_running()
            || self.current_thread()?.lifecycle != RuntimeThreadLifecycleState::Running
            || !self.scheduler_state()?.is_executable()
            || timeout_milliseconds == 0
            || timeout_milliseconds == u32::MAX
        {
            return Err(RuntimeStateError::new(
                "finite object wait requires an executable owner and finite positive timeout",
            ));
        }
        let kind = handle.kind()?;
        if !matches!(kind, RuntimeObjectKind::Event | RuntimeObjectKind::Thread) {
            return Err(RuntimeStateError::new(
                "finite object wait requires an event or thread",
            ));
        }
        let object = self.objects.object_for_handle(handle, kind)?;
        if matches!(object.wait_state, Some(RuntimeWaitState::Signaled { .. })) {
            self.wait_current_thread_on(handle)?;
            return Ok(RuntimeWaitForSingleObjectExBegin::Immediate {
                outcome: RuntimeWaitForSingleObjectExOutcome::ObjectSignaled,
            });
        }
        if object.wait_state != Some(RuntimeWaitState::Nonsignaled) {
            return Err(RuntimeStateError::new(
                "finite wait object has no valid wait state",
            ));
        }
        let started = self.process.timer_time_100ns();
        let deadline = started
            .checked_add(u64::from(timeout_milliseconds) * 10_000)
            .ok_or_else(|| {
                RuntimeStateError::new("finite object wait deadline exceeds the timer clock")
            })?;
        let request = self
            .scheduler
            .checked_host_request(RUNTIME_OBJECT_WAIT_SERVICE.to_owned())?;
        let pacing = RuntimeObjectWaitPacing {
            request_id: request.request_id,
            delay_milliseconds: timeout_milliseconds.min(i32::MAX as u32),
            started_at_time_100ns: started,
            signaled: false,
        };
        let registration = self
            .scheduler
            .checked_object_wait_registration(object.id, handle, deadline, pacing)?;
        self.scheduler.publish_wait_registration(registration);
        self.scheduler.publish_host_request(request);
        Ok(RuntimeWaitForSingleObjectExBegin::Deferred { registration })
    }

    /// Read-only readiness for the browser's outstanding object-wait resources.
    /// The latch was committed by SetEvent/thread exit while the wait was live.
    pub fn ready_object_wait_request_id(&self) -> u64 {
        self.scheduler
            .wait_registrations()
            .values()
            .find_map(|registration| {
                registration
                    .object_wait_pacing
                    .filter(|pacing| pacing.signaled)
                    .map(|pacing| pacing.request_id)
            })
            .unwrap_or(0)
    }

    pub(crate) fn plan_object_wait_completion(
        &self,
        request_id: u64,
        acknowledgement: u32,
    ) -> Result<RuntimeObjectWaitCompletionPlan, RuntimeStateError> {
        if acknowledgement > 1 {
            return Err(RuntimeStateError::new(
                "object wait acknowledgement must be signal or timer",
            ));
        }
        let scheduler = self.scheduler.validate_host_completion(request_id)?;
        if scheduler.service != RUNTIME_OBJECT_WAIT_SERVICE
            || scheduler.thread_id != self.current_thread_id()
        {
            return Err(RuntimeStateError::new(
                "object wait completion does not own the selected host request",
            ));
        }
        let registration = self
            .scheduler
            .wait_registration_for_thread(scheduler.thread_id)?
            .ok_or_else(|| {
                RuntimeStateError::new("object wait host request lost its registration")
            })?;
        let pacing = registration.object_wait_pacing.ok_or_else(|| {
            RuntimeStateError::new("object wait registration lost its pacing identity")
        })?;
        if pacing.request_id != request_id
            || registration.waitable_timer_pacing.is_some()
            || pacing.delay_milliseconds == 0
            || pacing.delay_milliseconds > i32::MAX as u32
        {
            return Err(RuntimeStateError::new(
                "object wait request/pacing identity is inconsistent",
            ));
        }
        let kind = registration.handle.kind()?;
        if !matches!(kind, RuntimeObjectKind::Event | RuntimeObjectKind::Thread) {
            return Err(RuntimeStateError::new(
                "object wait completion has an invalid object kind",
            ));
        }
        let object = self.objects.object_for_handle(registration.handle, kind)?;
        if object.id != registration.object_id || object.wait_state.is_none() {
            return Err(RuntimeStateError::new(
                "object wait completion lost its live object identity",
            ));
        }
        let now = self.process.timer_time_100ns();
        let deadline = registration
            .timeout_deadline_time_100ns
            .ok_or_else(|| RuntimeStateError::new("finite object wait has no deadline"))?;
        if now < pacing.started_at_time_100ns || deadline <= pacing.started_at_time_100ns {
            return Err(RuntimeStateError::new(
                "object wait clock or deadline is inconsistent",
            ));
        }
        if acknowledgement == 0 && !pacing.signaled {
            return Err(RuntimeStateError::new(
                "object wait signal acknowledgement has no Rust signal latch",
            ));
        }
        let retry = if !pacing.signaled && now < deadline {
            Some(RuntimeObjectWaitPacing {
                request_id,
                delay_milliseconds: ((deadline - now).div_ceil(10_000)).min(i32::MAX as u64) as u32,
                started_at_time_100ns: now,
                signaled: false,
            })
        } else {
            None
        };
        Ok(RuntimeObjectWaitCompletionPlan {
            scheduler,
            registration,
            retry,
            outcome: if pacing.signaled {
                RuntimeWaitForSingleObjectExOutcome::ObjectSignaled
            } else {
                RuntimeWaitForSingleObjectExOutcome::TimedOut
            },
        })
    }

    pub(crate) fn commit_object_wait_completion(&mut self, plan: RuntimeObjectWaitCompletionPlan) {
        if let Some(retry) = plan.retry {
            self.scheduler
                .replace_object_wait_pacing(plan.registration, retry);
            return;
        }
        self.scheduler
            .commit_remove_wait_registration(plan.registration);
        self.scheduler.commit_host_completion(plan.scheduler);
    }

    pub(crate) fn post_thread_message(
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
            return Err(RuntimeStateError::new(
                "message post requires a running process and executing source thread",
            ));
        }
        self.enqueue_thread_message(target_thread_id, hwnd, message, w_param, l_param)
    }

    // System input can arrive while the selected guest thread is waiting. Only
    // guest PostMessage requires an executing caller; both paths validate the
    // destination, queue capacity and wakeup before publishing the message.
    fn enqueue_thread_message(
        &mut self,
        target_thread_id: RuntimeThreadId,
        hwnd: u32,
        message: u32,
        w_param: u32,
        l_param: u32,
    ) -> Result<RuntimeThreadMessage, RuntimeStateError> {
        let cursor = self.input.cursor_position().ok_or_else(|| {
            RuntimeStateError::new(
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
                    | SchedulerThreadState::Suspended { .. }
                    | SchedulerThreadState::WaitingHost { .. }
                    | SchedulerThreadState::WaitingObject { .. }
                    | SchedulerThreadState::WaitingCriticalSection { .. }
                    | SchedulerThreadState::WaitingMessage
                    | SchedulerThreadState::InCallback { .. }
            )
        {
            return Err(RuntimeStateError::new(
                "message target requires matching live thread and scheduler states",
            ));
        }
        let posted = self.messages.checked_post(
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
        let resumed_state = (target_scheduler_state == SchedulerThreadState::WaitingMessage)
            .then(|| self.resumed_execution_state_for_thread(target_thread_id))
            .transpose()?;
        self.messages.push(posted);
        if let Some(resumed_state) = resumed_state {
            self.observe_thread_message_state(target_thread_id);
            self.scheduler
                .set_validated_state(target_thread_id, resumed_state);
            self.scheduler
                .commit_event(RuntimeSchedulerEvent::MessageReady(target_thread_id));
        }
        Ok(posted)
    }

    pub(crate) fn get_current_thread_queue_status(
        &mut self,
        flags: u32,
    ) -> Result<u32, RuntimeStateError> {
        self.require_user32_operation_state()?;
        self.message_wait_queue_status(flags, true)
    }

    pub(crate) fn message_wait_queue_status(
        &mut self,
        flags: u32,
        observe: bool,
    ) -> Result<u32, RuntimeStateError> {
        let thread_id = self.current_thread_id();
        let mut status = if observe {
            self.messages.get_queue_status(thread_id, flags)?
        } else {
            self.messages.peek_queue_status(thread_id, flags)?
        };
        if flags & 0x20 != 0 {
            if self.user32.pending_paint_window(thread_id, false).is_some() {
                status |= 0x0020_0000;
            }
            if self.user32.pending_paint_window(thread_id, true).is_some() {
                status |= 0x20;
            }
            if observe {
                self.user32.observe_paint(thread_id);
            }
        }
        if flags & 0x10 != 0 {
            let clock = self.process.timer_time_100ns();
            for timer in self.timers.window_timers().values() {
                if timer.owner_thread_id == thread_id && timer.next_deadline_time_100ns <= clock {
                    status |= 0x0010_0000;
                    if !timer.message_observed {
                        status |= 0x10;
                    }
                }
            }
            if observe {
                self.timers.observe_window_timers(thread_id, clock);
            }
        }
        Ok(status)
    }

    fn has_new_thread_message(
        &self,
        thread_id: RuntimeThreadId,
    ) -> Result<bool, RuntimeStateError> {
        Ok(self.messages.has_new(thread_id)?
            || self.user32.pending_paint_window(thread_id, true).is_some()
            || self.timers.window_timers().values().any(|timer| {
                timer.owner_thread_id == thread_id
                    && !timer.message_observed
                    && timer.next_deadline_time_100ns <= self.process.timer_time_100ns()
            }))
    }

    fn observe_thread_message_state(&mut self, thread_id: RuntimeThreadId) {
        self.messages.observe_unfiltered(thread_id);
        self.user32.observe_paint(thread_id);
        self.timers
            .observe_window_timers(thread_id, self.process.timer_time_100ns());
    }

    fn observe_thread_message_filter(
        &mut self,
        thread_id: RuntimeThreadId,
        filter: super::RuntimeMessageFilter,
    ) {
        self.messages.observe_filtered(thread_id, filter);
        if filter.includes_message(0x000f) {
            self.user32.observe_paint(thread_id);
        }
        if filter.includes_message(0x0113) {
            self.timers
                .observe_window_timers(thread_id, self.process.timer_time_100ns());
        }
    }

    /// WaitMessage observes new arrivals without removing previously inspected messages.
    pub(crate) fn wait_for_current_thread_message(&mut self) -> Result<bool, RuntimeStateError> {
        self.require_user32_operation_state()?;
        let thread_id = self.current_thread_id();
        let ready = self.has_new_thread_message(thread_id)?;
        self.observe_thread_message_state(thread_id);
        if !ready {
            self.scheduler
                .set_validated_state(thread_id, SchedulerThreadState::WaitingMessage);
        }
        Ok(ready)
    }

    /// Browser pacing for idle message waiters; all timer and thread state stays in Rust.
    pub fn message_wait_status(&self) -> (bool, Option<u32>) {
        let mut pending = false;
        let mut deadline = None;
        for (&thread_id, &state) in self.scheduler.thread_states() {
            if state != SchedulerThreadState::WaitingMessage {
                continue;
            }
            pending = true;
            for timer in self.timers.window_timers().values() {
                if timer.owner_thread_id == thread_id && !timer.message_observed {
                    deadline = Some(deadline.map_or(timer.next_deadline_time_100ns, |old: u64| {
                        old.min(timer.next_deadline_time_100ns)
                    }));
                }
            }
        }
        (
            pending,
            deadline.map(|deadline| {
                deadline
                    .saturating_sub(self.process.timer_time_100ns())
                    .div_ceil(10_000)
                    .min(0x7fff_ffff) as u32
            }),
        )
    }

    #[cfg(test)]
    pub(crate) fn receive_current_thread_message(
        &mut self,
    ) -> Result<RuntimeMessageReceiveDisposition, RuntimeStateError> {
        if !self.process.is_running()
            || self.current_thread()?.lifecycle != RuntimeThreadLifecycleState::Running
            || self.scheduler_state()? != SchedulerThreadState::Runnable
        {
            return Err(RuntimeStateError::new(
                "message receive requires a running process and runnable owner thread",
            ));
        }
        let thread_id = self.current_thread_id();
        if let Some(message) = self.messages.front(thread_id)? {
            self.messages.observe_unfiltered(thread_id);
            self.messages.pop(thread_id);
            return Ok(RuntimeMessageReceiveDisposition::Message { message });
        }
        self.messages.observe_unfiltered(thread_id);
        self.scheduler
            .set_validated_state(thread_id, SchedulerThreadState::WaitingMessage);
        Ok(RuntimeMessageReceiveDisposition::Blocked)
    }

    pub(crate) fn peek_current_thread_message(
        &mut self,
        output: GuestAddress,
        remove: bool,
        filter: super::RuntimeMessageFilter,
    ) -> Result<bool, RuntimeStateError> {
        let scheduler_state = self.scheduler_state()?;
        if !self.process.is_running()
            || self.current_thread()?.lifecycle != RuntimeThreadLifecycleState::Running
            || !scheduler_state.is_executable()
        {
            return Err(RuntimeStateError::new(format!(
                "message peek requires a running process and executable owner thread; current thread {} is {scheduler_state:?}",
                self.current_thread_id().0
            )));
        }
        let thread_id = self.current_thread_id();
        let timer_clock = self.process.timer_time_100ns();
        let queued = self.messages.find(thread_id, filter)?;
        if queued.is_none()
            && let Some(window) = self
                .user32
                .pending_paint_window_filtered(thread_id, false, filter)
        {
            let cursor = self.input.cursor_position().ok_or_else(|| {
                RuntimeStateError::new(
                    "paint message delivery requires a configured logical cursor",
                )
            })?;
            let message = RuntimeThreadMessage {
                target_thread_id: thread_id,
                hwnd: window.handle,
                message: 0x000f,
                w_param: 0,
                l_param: 0,
                time: self.process.clock().tick_count,
                point_x: cursor.screen_x,
                point_y: cursor.screen_y,
            };
            let bytes = message.to_x86_msg_bytes();
            self.memory
                .validate_access(output, bytes.len(), MemoryAccess::Write)
                .map_err(RuntimeStateError::from_memory)?;
            let mut transaction = self.memory.begin_transaction();
            transaction
                .write(output, &bytes)
                .map_err(RuntimeStateError::from_memory)?;
            transaction.commit_transaction();
            // PM_REMOVE does not validate the update region; BeginPaint does.
            self.observe_thread_message_filter(thread_id, filter);
            return Ok(true);
        }
        if queued.is_none()
            && let Some(timer) = self.timers.preview_due_window_timer_delivery_filtered(
                thread_id,
                timer_clock,
                filter,
            )?
        {
            let window = self.user32.window(timer.window_handle)?;
            if window.id != timer.window_id
                || window.lifecycle != RuntimeWindowLifecycle::Created
                || window.creation.owner_thread_id != timer.owner_thread_id
            {
                return Err(RuntimeStateError::new(
                    "logical window timer target changed before message delivery",
                ));
            }
            let cursor = self.input.cursor_position().ok_or_else(|| {
                RuntimeStateError::new(
                    "timer message delivery requires a configured logical cursor position",
                )
            })?;
            let message = self.messages.checked_post(
                thread_id,
                timer.window_handle,
                0x0113, // WM_TIMER
                timer.timer_id,
                timer.callback_address.0,
                self.process.clock().tick_count,
                cursor.screen_x,
                cursor.screen_y,
                false,
            )?;
            let bytes = message.to_x86_msg_bytes();
            self.memory
                .validate_access(output, bytes.len(), MemoryAccess::Write)
                .map_err(RuntimeStateError::from_memory)?;
            let mut transaction = self.memory.begin_transaction();
            transaction
                .write(output, &bytes)
                .map_err(RuntimeStateError::from_memory)?;
            self.timers.publish_window_timer_delivery(timer);
            if !remove {
                self.messages.push_window_timer(message);
            }
            transaction.commit_transaction();
            self.observe_thread_message_filter(thread_id, filter);
            return Ok(true);
        }
        let Some((index, message)) = queued else {
            self.observe_thread_message_filter(thread_id, filter);
            return Ok(false);
        };
        let bytes = message.to_x86_msg_bytes();
        self.memory
            .validate_access(output, bytes.len(), MemoryAccess::Write)
            .map_err(RuntimeStateError::from_memory)?;
        let mut transaction = self.memory.begin_transaction();
        transaction
            .write(output, &bytes)
            .map_err(RuntimeStateError::from_memory)?;
        if remove {
            self.messages.remove_at(thread_id, index);
        }
        transaction.commit_transaction();
        self.observe_thread_message_filter(thread_id, filter);
        Ok(true)
    }

    #[cfg(test)]
    pub(crate) fn queued_thread_messages(
        &self,
        thread_id: RuntimeThreadId,
    ) -> Result<Vec<RuntimeThreadMessage>, RuntimeStateError> {
        self.thread_state(thread_id)?;
        self.messages.snapshot(thread_id)
    }

    pub(crate) fn set_heap_termination_on_corruption(&mut self, enabled: bool) {
        self.process.set_heap_termination_on_corruption(enabled);
    }

    pub(crate) fn ensure_standard_handle(
        &mut self,
        selector: u32,
    ) -> Result<u32, RuntimeStateError> {
        if self.current_thread()?.lifecycle != RuntimeThreadLifecycleState::Running {
            return Err(RuntimeStateError::new(
                "standard handle lookup requires a running thread",
            ));
        }
        self.process.ensure_standard_handle(selector)
    }

    pub(crate) fn set_standard_handle(
        &mut self,
        selector: u32,
        handle: u32,
    ) -> Result<bool, RuntimeStateError> {
        self.require_source_file_operation_state()?;
        Ok(self.process.set_standard_handle(selector, handle))
    }

    pub(crate) fn file_type(&self, handle: u32) -> Result<Option<u32>, RuntimeStateError> {
        if let Some(file_type) = self.process.standard_file_type(handle) {
            return Ok(Some(file_type));
        }
        Ok(self
            .live_source_file_for_handle(RuntimeHandle(handle))?
            .map(|_| 1))
    }

    pub(crate) fn process_heap(&mut self) -> Result<u32, RuntimeStateError> {
        self.heaps.process_heap(&mut self.objects)
    }

    pub(crate) fn allocate_private_heap(
        &mut self,
        initial_size: u32,
        maximum_size: u32,
    ) -> Result<u32, RuntimeStateError> {
        self.heaps
            .create_private(&mut self.objects, initial_size, maximum_size)
    }

    pub(crate) fn destroy_private_heap(&mut self, handle: u32) -> Result<bool, RuntimeStateError> {
        self.require_private_heap_operation_state()?;
        if !self
            .objects
            .is_live_handle(RuntimeHandle(handle), RuntimeObjectKind::PrivateHeap)?
        {
            return Ok(false);
        }
        self.heaps.destroy_private(
            &mut self.memory,
            &mut self.address_space_allocator,
            &mut self.objects,
            handle,
        )
    }

    pub(crate) fn configure_heap_serialization(
        &mut self,
        handle: u32,
        serialized: bool,
    ) -> Result<(), RuntimeStateError> {
        self.heap(handle)?;
        self.heaps.configure_serialization(handle, serialized)
    }

    pub(crate) fn enable_low_fragmentation_heap(
        &mut self,
        handle: u32,
    ) -> Result<bool, RuntimeStateError> {
        self.require_private_heap_operation_state()?;
        self.heap(handle)?;
        self.heaps.enable_low_fragmentation(handle)
    }

    pub(crate) fn largest_committed_heap_free_block(
        &self,
        handle: u32,
    ) -> Result<u32, RuntimeStateError> {
        self.heap(handle)?;
        self.heaps.largest_committed_free_block(handle)
    }

    pub(crate) fn allocate_com_task_memory(
        &mut self,
        byte_length: u32,
    ) -> Result<Option<RuntimeHeapAllocation>, RuntimeStateError> {
        self.require_private_heap_operation_state()?;
        self.heaps.allocate_task(
            &mut self.memory,
            &mut self.address_space_allocator,
            byte_length,
        )
    }

    pub(crate) fn free_com_task_memory(
        &mut self,
        address: GuestAddress,
    ) -> Result<(), RuntimeStateError> {
        self.require_private_heap_operation_state()?;
        self.heaps
            .free_task(&mut self.memory, &mut self.address_space_allocator, address)
    }

    pub(crate) fn heap(&self, handle: u32) -> Result<&RuntimeHeap, RuntimeStateError> {
        self.objects
            .object_for_handle(RuntimeHandle(handle), RuntimeObjectKind::PrivateHeap)?;
        self.heaps.heap(handle)
    }

    #[cfg(test)]
    pub(crate) fn heap_backing(
        &self,
        handle: u32,
    ) -> Result<Option<&RuntimeHeapBacking>, RuntimeStateError> {
        self.heap(handle)?;
        Ok(self.heaps.backing(handle))
    }

    #[cfg(test)]
    pub(crate) fn private_heap_allocation(
        &self,
        handle: u32,
        address: GuestAddress,
    ) -> Result<RuntimeHeapAllocation, RuntimeStateError> {
        self.require_private_heap_operation_state()?;
        self.heap(handle)?;
        self.heaps.allocation(handle, address)
    }

    pub(crate) fn private_heap_allocation_option(
        &self,
        handle: u32,
        address: GuestAddress,
    ) -> Result<Option<RuntimeHeapAllocation>, RuntimeStateError> {
        self.require_private_heap_operation_state()?;
        self.heap(handle)?;
        self.heaps.allocation_option(handle, address)
    }

    pub(crate) fn allocate_private_heap_block(
        &mut self,
        handle: u32,
        byte_length: u32,
        initialization: RuntimeHeapInitialization,
    ) -> Result<Option<RuntimeHeapAllocation>, RuntimeStateError> {
        self.require_private_heap_operation_state()?;
        self.heap(handle)?;
        self.heaps.allocate(
            &mut self.memory,
            &mut self.address_space_allocator,
            handle,
            byte_length,
            initialization,
            None,
        )
    }

    pub(crate) fn free_private_heap_block(
        &mut self,
        handle: u32,
        address: GuestAddress,
    ) -> Result<(), RuntimeStateError> {
        self.require_private_heap_operation_state()?;
        self.heap(handle)?;
        self.heaps.free(
            &mut self.memory,
            &mut self.address_space_allocator,
            handle,
            address,
        )
    }

    pub(crate) fn reallocate_private_heap_block(
        &mut self,
        handle: u32,
        address: GuestAddress,
        byte_length: u32,
        initialization: RuntimeHeapInitialization,
    ) -> Result<Option<RuntimeHeapAllocation>, RuntimeStateError> {
        self.require_private_heap_operation_state()?;
        self.heap(handle)?;
        self.heaps.allocate(
            &mut self.memory,
            &mut self.address_space_allocator,
            handle,
            byte_length,
            initialization,
            Some(address),
        )
    }

    pub(crate) fn materialize_private_heap_page(
        &mut self,
        fault_address: GuestAddress,
        page_fault_error_code: u32,
    ) -> Result<(), RuntimeStateError> {
        self.require_private_heap_operation_state()?;
        if page_fault_error_code & 1 != 0 {
            return Err(RuntimeStateError::new(
                "present-page protection faults cannot materialize a lazy heap page",
            ));
        }
        let stats = self.memory.stats();
        if stats.generation != stats.acknowledged_generation || stats.pending_invalidation.is_some()
        {
            return Err(RuntimeStateError::new(
                "large heap page materialization requires fully acknowledged guest memory",
            ));
        }
        let (backing, committed_page) = self
            .heaps
            .preview_page_materialization(&self.address_space_allocator, fault_address)?;
        let base = backing.allocation.address.0;
        let memory_owner = backing.memory_owner;
        let region_matches = self.memory.regions().any(|region| {
            region.owner == backing.memory_owner
                && region.kind == MemoryRegionKind::Heap
                && region.range == backing.range
        });
        if !region_matches {
            return Err(RuntimeStateError::new(
                "large heap metadata does not match the Rust guest-memory reservation",
            ));
        }
        self.memory
            .preflight_growth(0, 0, 1)
            .map_err(RuntimeStateError::from_memory)?;

        let mut transaction = self.memory.begin_transaction();
        transaction
            .commit(
                memory_owner,
                committed_page,
                PageProtection {
                    read: true,
                    write: true,
                    execute: false,
                },
            )
            .map_err(RuntimeStateError::from_memory)?;
        self.heaps.record_materialized_page(base, committed_page);
        transaction.commit_transaction();
        Ok(())
    }

    fn require_private_heap_operation_state(&self) -> Result<(), RuntimeStateError> {
        if !self.process.is_running()
            || self.current_thread()?.lifecycle != RuntimeThreadLifecycleState::Running
        {
            return Err(RuntimeStateError::new(
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
            return Err(RuntimeStateError::new(
                "main executable cannot be registered as a runtime system module",
            ));
        }
        self.register_module_record(module, RuntimeModuleClass::MainExecutable)
    }

    pub(crate) fn load_current_thread_runtime_system_module(
        &mut self,
        requested_name: String,
        canonical_name: String,
    ) -> Result<u32, RuntimeStateError> {
        if !self.process.is_running()
            || self.current_thread()?.lifecycle != RuntimeThreadLifecycleState::Running
        {
            return Err(RuntimeStateError::new(
                "dynamic runtime-system loading requires a running process and current thread",
            ));
        }
        let thread_id = self.current_thread_id().0;
        self.modules.load_runtime_system_module(
            &mut self.objects,
            requested_name,
            canonical_name,
            thread_id,
        )
    }

    fn register_module_record(
        &mut self,
        module: RuntimeModule,
        class: RuntimeModuleClass,
    ) -> Result<&RuntimeModule, RuntimeStateError> {
        let handle = module.handle;
        self.modules.register(&mut self.objects, module, class)?;
        Ok(self
            .modules
            .module(handle)
            .expect("committed runtime module disappeared"))
    }

    pub(crate) fn module(&self, handle: u32) -> Result<&RuntimeModule, RuntimeStateError> {
        self.module_option(handle)?.ok_or_else(|| {
            RuntimeStateError::new(format!(
                "logical module handle {handle:#010x} is not loaded"
            ))
        })
    }

    pub(crate) fn module_option(
        &self,
        handle: u32,
    ) -> Result<Option<&RuntimeModule>, RuntimeStateError> {
        if !self
            .objects
            .is_live_handle(RuntimeHandle(handle), RuntimeObjectKind::Module)?
        {
            return Ok(None);
        }
        let object = self
            .objects
            .object_for_handle(RuntimeHandle(handle), RuntimeObjectKind::Module)?;
        let module =
            self.modules.modules().get(&handle).ok_or_else(|| {
                RuntimeStateError::new("live module handle has no module payload")
            })?;
        if module.handle != handle || object.key != (RuntimeObjectKey::Module { handle }) {
            return Err(RuntimeStateError::new(
                "live module handle, object identity, and payload disagree",
            ));
        }
        Ok(Some(module))
    }

    pub fn module_by_name(&self, name: &str) -> Result<Option<&RuntimeModule>, RuntimeStateError> {
        let Some(handle) = self.modules.module_by_name_handle(name)? else {
            return Ok(None);
        };
        let module = self.module_option(handle)?.ok_or_else(|| {
            RuntimeStateError::new(
                "registered module name does not resolve to a live module handle",
            )
        })?;
        Ok(Some(module))
    }

    pub(crate) fn main_module(&self) -> Result<&RuntimeModule, RuntimeStateError> {
        let handle = self.modules.main_module_handle()?;
        self.module(handle)
    }

    #[cfg(test)]
    pub(crate) fn add_module_dependency(
        &mut self,
        requester_handle: u32,
        dependency_handle: u32,
    ) -> Result<(), RuntimeStateError> {
        self.module(requester_handle)?;
        self.module(dependency_handle)?;
        self.modules
            .add_dependency(requester_handle, dependency_handle)
    }

    pub(crate) fn add_current_thread_module_reference(
        &mut self,
        handle: u32,
    ) -> Result<u32, RuntimeStateError> {
        self.module(handle)?;
        self.modules
            .add_dynamic_reference(handle, self.current_thread_id().0)
    }

    pub fn mark_module_ready(&mut self, handle: u32) -> Result<(), RuntimeStateError> {
        self.module(handle)?;
        self.modules.mark_ready(handle)
    }

    #[cfg(test)]
    pub(crate) fn attach_modules(&mut self) -> Result<Vec<RuntimeModuleId>, RuntimeStateError> {
        self.modules.attach_all()
    }

    pub(crate) fn dll_lifecycle_phase(&self) -> RuntimeDllLifecyclePhase {
        self.modules.dll_lifecycle_phase()
    }

    pub fn begin_dll_process_attach(
        &mut self,
        return_sentinel: GuestAddress,
    ) -> Result<RuntimeDllLifecycleAction, RuntimeStateError> {
        self.modules.require_process_attach_start()?;
        self.require_dll_callback_thread_state()?;
        self.advance_dll_lifecycle(RuntimeDllLifecycleRequest::Attach(return_sentinel), None)
    }

    #[cfg(test)]
    pub(crate) fn begin_dll_process_detach(
        &mut self,
        return_sentinel: GuestAddress,
    ) -> Result<RuntimeDllLifecycleAction, RuntimeStateError> {
        self.modules.require_process_detach_start()?;
        self.require_dll_process_detach_thread_state()?;
        self.advance_dll_lifecycle(RuntimeDllLifecycleRequest::Detach(return_sentinel), None)
    }

    pub(crate) fn complete_dll_callback(
        &mut self,
        returned_sentinel: GuestAddress,
        result: u32,
    ) -> Result<RuntimeDllLifecycleAction, RuntimeStateError> {
        let frame = self
            .current_thread()?
            .callback_frames
            .last()
            .copied()
            .ok_or_else(|| {
                RuntimeStateError::new("DLL callback return has no active Rust thread frame")
            })?;
        if frame.return_sentinel != returned_sentinel {
            return Err(RuntimeStateError::new(
                "DLL callback return does not identify the active return sentinel",
            ));
        }
        self.memory
            .validate_access(returned_sentinel, 1, MemoryAccess::Execute)
            .map_err(RuntimeStateError::from_memory)?;
        self.advance_dll_lifecycle(
            RuntimeDllLifecycleRequest::Complete {
                frame_id: frame.id,
                result,
            },
            Some(frame.id),
        )
    }

    fn require_dll_callback_thread_state(&self) -> Result<(), RuntimeStateError> {
        if !self.process.is_running()
            || self.current_thread()?.lifecycle != RuntimeThreadLifecycleState::Running
            || self.scheduler_state()? != SchedulerThreadState::Runnable
        {
            return Err(RuntimeStateError::new(
                "DLL process callbacks require a running process and runnable selected thread",
            ));
        }
        Ok(())
    }

    #[cfg(test)]
    fn require_dll_process_detach_thread_state(&self) -> Result<(), RuntimeStateError> {
        if !self.process.is_running()
            || self.current_thread()?.lifecycle != RuntimeThreadLifecycleState::Running
        {
            return Err(RuntimeStateError::new(
                "DLL process detach requires a running process and an active selected thread when guest callbacks are pending",
            ));
        }
        Ok(())
    }

    fn advance_dll_lifecycle(
        &mut self,
        request: RuntimeDllLifecycleRequest,
        completed_frame: Option<RuntimeCallbackFrameId>,
    ) -> Result<RuntimeDllLifecycleAction, RuntimeStateError> {
        if let Some(expected) = completed_frame {
            self.validate_callback_frame_return(expected)?;
        }
        let memory = &mut self.memory;
        let threads = &mut self.threads;
        let scheduler = &mut self.scheduler;
        let pending_api_callbacks = &mut self.pending_api_callbacks;
        let address_space = &self.address_space_allocator;
        self.modules.advance_lifecycle(request, |spec| {
            let Some(spec) = spec else {
                if let Some(expected) = completed_frame {
                    let thread_id = scheduler.current_thread_id();
                    let frame = threads.pop_callback_frame(thread_id, expected)?;
                    let depth = threads
                        .callback_depth(thread_id)
                        .expect("validated callback thread");
                    scheduler.set_validated_state(
                        thread_id,
                        if depth == 0 {
                            SchedulerThreadState::Runnable
                        } else {
                            SchedulerThreadState::InCallback { depth }
                        },
                    );
                    pending_api_callbacks.remove(&frame.id);
                }
                return Ok(None);
            };
            let frame = Self::prepare_callback_frame_state(
                threads,
                scheduler,
                completed_frame,
                spec.entry_point,
                spec.return_sentinel,
                0,
            )?;
            memory
                .validate_owned_host_execute(spec.memory_owner, spec.entry_point, 1)
                .map_err(RuntimeStateError::from_memory)?;
            memory
                .validate_host_execute(spec.return_sentinel, 1)
                .map_err(RuntimeStateError::from_memory)?;
            let stack = threads
                .current_state(frame.thread_id)?
                .stack
                .as_ref()
                .ok_or_else(|| {
                    RuntimeStateError::new("DLL callback thread has no registered stack")
                })?;
            let allocation = address_space.allocation(stack.address_allocation_id)?;
            if allocation.purpose != ProcessAddressAllocationPurpose::ThreadStack
                || allocation.classification != ProcessAddressAllocationClassification::Stack
                || allocation.range.base != stack.reserved_range.start()
                || allocation.range.byte_length != stack.reserved_range.byte_length()
            {
                return Err(RuntimeStateError::new(
                    "DLL callback stack no longer matches its live address allocation",
                ));
            }
            memory
                .validate_stack_owner(
                    stack.memory_owner,
                    stack.reserved_range,
                    stack.committed_range,
                )
                .map_err(RuntimeStateError::from_memory)?;
            let pointer = GuestAddress(stack.initial_stack_pointer.0.checked_sub(16).ok_or_else(
                || {
                    RuntimeStateError::new(
                        "DLL callback argument frame underflows its thread stack",
                    )
                },
            )?);
            let range = GuestRange::new(pointer, 16).map_err(RuntimeStateError::from_memory)?;
            if range.start().0 < stack.committed_range.start().0
                || range.end() > stack.committed_range.end()
            {
                return Err(RuntimeStateError::new(
                    "DLL callback arguments are outside the committed thread stack",
                ));
            }
            let words = [
                spec.return_sentinel.0,
                spec.module_base.0,
                spec.reason.value(),
                spec.reserved,
            ];
            let mut bytes = [0_u8; 16];
            for (slot, word) in words.into_iter().enumerate() {
                bytes[slot * 4..slot * 4 + 4].copy_from_slice(&word.to_le_bytes());
            }
            let mut transaction = memory.begin_transaction();
            transaction
                .write(pointer, &bytes)
                .map_err(RuntimeStateError::from_memory)?;
            transaction.commit_transaction();
            let exited = threads.publish_callback_frame(frame, completed_frame.is_some());
            let depth = threads
                .callback_depth(frame.thread_id)
                .expect("validated callback thread");
            scheduler
                .set_validated_state(frame.thread_id, SchedulerThreadState::InCallback { depth });
            if let Some(exited) = exited {
                pending_api_callbacks.remove(&exited.id);
            }
            Ok(Some((frame, pointer)))
        })
    }

    pub(crate) fn register_dynamic_endpoint(
        &mut self,
        endpoint: RuntimeDynamicEndpoint,
    ) -> Result<&RuntimeDynamicEndpoint, RuntimeStateError> {
        self.registries.register_dynamic_endpoint(
            RuntimeAddressRange::new(DYNAMIC_EXPORT_ARENA_BASE, DYNAMIC_EXPORT_ARENA_SIZE)?,
            endpoint,
        )
    }

    pub fn register_dynamic_export_thunk(
        &mut self,
        thunk: &DynamicExportThunk,
    ) -> Result<&RuntimeDynamicEndpoint, RuntimeStateError> {
        self.register_dynamic_endpoint(RuntimeDynamicEndpoint::from(thunk))
    }

    pub(crate) fn dynamic_endpoint_by_name(
        &self,
        module: &str,
        name: &str,
    ) -> Option<&RuntimeDynamicEndpoint> {
        self.registries.dynamic_endpoint_by_name(module, name)
    }

    pub fn runtime_system_export_endpoints(
        &self,
    ) -> impl Iterator<Item = &super::modules::RuntimeSystemExportEndpoint> {
        self.modules
            .runtime_system_exports()
            .into_iter()
            .flat_map(|table| table.endpoints.iter())
    }

    pub(crate) fn has_runtime_system_exports_for_module(&self, name: &str) -> bool {
        self.runtime_system_export_endpoints().any(|endpoint| {
            let (crate::RuntimeExport::Name { module, .. }
            | crate::RuntimeExport::Ordinal { module, .. }) = &endpoint.export;
            module.eq_ignore_ascii_case(name)
        })
    }

    pub(crate) fn dynamic_endpoint_by_ordinal(
        &self,
        module: &str,
        ordinal: u32,
    ) -> Option<&RuntimeDynamicEndpoint> {
        self.registries.dynamic_endpoint_by_ordinal(module, ordinal)
    }

    pub(crate) fn allocate_tls_index(&mut self) -> Result<u32, RuntimeStateError> {
        self.threads.allocate_tls_index()
    }

    pub(crate) fn free_tls_index(&mut self, index: u32) -> bool {
        self.threads.free_tls_index(index)
    }

    pub(crate) fn tls_index_available(&self, index: u32) -> bool {
        self.threads.tls_index_available(index)
    }

    pub(crate) fn allocate_fls_slot(
        &mut self,
        callback: Option<GuestAddress>,
    ) -> Result<u32, RuntimeStateError> {
        if !self.process.is_running()
            || self.current_thread()?.lifecycle != RuntimeThreadLifecycleState::Running
            || !matches!(
                self.scheduler_state()?,
                SchedulerThreadState::Runnable | SchedulerThreadState::InCallback { .. }
            )
        {
            return Err(RuntimeStateError::new(
                "FLS allocation requires a running process and active current thread",
            ));
        }
        self.threads
            .allocate_fls_slot(self.current_thread_id(), callback)
    }

    pub(crate) fn free_fls_slot(&mut self, index: u32) -> Result<bool, RuntimeStateError> {
        if !self.process.is_running()
            || self.current_thread()?.lifecycle != RuntimeThreadLifecycleState::Running
            || !self.scheduler_state()?.is_executable()
        {
            return Err(RuntimeStateError::new(
                "FLS release requires a running process and active current thread",
            ));
        }
        self.threads.free_fls_slot(index)
    }

    pub(crate) fn set_fls_value(
        &mut self,
        index: u32,
        value: u32,
    ) -> Result<(), RuntimeStateError> {
        if !self.process.is_running()
            || self.current_thread()?.lifecycle != RuntimeThreadLifecycleState::Running
            || !matches!(
                self.scheduler_state()?,
                SchedulerThreadState::Runnable | SchedulerThreadState::InCallback { .. }
            )
        {
            return Err(RuntimeStateError::new(
                "FLS value storage requires a running process and active current thread",
            ));
        }
        self.threads.fls_slot(index)?;
        self.threads
            .set_fls_value(self.current_thread_id(), index, value)
    }

    #[cfg(test)]
    pub(crate) fn fls_value(&self, index: u32) -> Result<u32, RuntimeStateError> {
        self.fls_value_or_unset(index)?.ok_or_else(|| {
            RuntimeStateError::new(format!(
                "logical FLS index {index} has no value for the current thread"
            ))
        })
    }

    pub(crate) fn fls_value_or_unset(&self, index: u32) -> Result<Option<u32>, RuntimeStateError> {
        self.threads.fls_slot(index)?;
        self.threads
            .fls_value_or_unset(self.current_thread_id(), index)
    }

    pub(crate) fn set_tls_value(
        &mut self,
        index: u32,
        value: u32,
    ) -> Result<(), RuntimeStateError> {
        self.threads.require_tls_index(index)?;
        self.threads
            .set_tls_value(self.current_thread_id(), index, value)
    }

    #[cfg(test)]
    pub(crate) fn tls_value(&self, index: u32) -> Result<u32, RuntimeStateError> {
        self.tls_value_or_unset(index)?.ok_or_else(|| {
            RuntimeStateError::new(format!(
                "current thread has no value for logical TLS index {index}"
            ))
        })
    }

    pub(crate) fn tls_value_or_unset(&self, index: u32) -> Result<Option<u32>, RuntimeStateError> {
        self.threads.require_tls_index(index)?;
        self.threads
            .tls_value_or_unset(self.current_thread_id(), index)
    }

    pub(crate) fn create_mutex(
        &mut self,
        name: Option<RuntimeMutexName>,
        initial_owner: bool,
    ) -> Result<(RuntimeHandle, bool), RuntimeStateError> {
        self.require_mutex_operation_state()?;
        let current_thread_id = self.current_thread_id();
        let (mutex_id, already_exists) = self
            .synchronization
            .validate_mutex_creation(name.as_ref())?;
        let key = RuntimeObjectKey::Mutex {
            mutex_id: mutex_id.0,
        };
        let handle = if already_exists {
            self.objects.open_handle(&key)?
        } else {
            self.objects.register_object_and_open_handle(key)?.1
        };
        if !already_exists {
            self.synchronization
                .insert_mutex(mutex_id, name, initial_owner, current_thread_id);
        }
        Ok((handle, already_exists))
    }

    pub(crate) fn create_event(
        &mut self,
        manual_reset: bool,
        initial_state: bool,
    ) -> Result<RuntimeHandle, RuntimeStateError> {
        self.require_event_operation_state()?;
        self.objects.register_event(manual_reset, initial_state)
    }

    pub(crate) fn event_for_handle(
        &self,
        handle: RuntimeHandle,
    ) -> Result<RuntimeEvent, RuntimeStateError> {
        self.live_event_for_handle(handle)?
            .ok_or_else(|| RuntimeStateError::new("event handle is not live"))
    }

    fn live_event_for_handle(
        &self,
        handle: RuntimeHandle,
    ) -> Result<Option<RuntimeEvent>, RuntimeStateError> {
        let Some(record) = self
            .objects
            .live_object_for_handle(handle, RuntimeObjectKind::Event)?
        else {
            return Ok(None);
        };
        let RuntimeObjectKey::Event {
            event_id,
            manual_reset,
        } = &record.key
        else {
            return Err(RuntimeStateError::new(
                "event handle resolved to a different object identity",
            ));
        };
        let wait_state = record
            .wait_state
            .ok_or_else(|| RuntimeStateError::new("event object has no wait state"))?;
        Ok(Some(RuntimeEvent {
            event_id: *event_id,
            manual_reset: *manual_reset,
            wait_state,
        }))
    }

    pub(crate) fn set_event(&mut self, handle: RuntimeHandle) -> Result<bool, RuntimeStateError> {
        self.require_event_operation_state()?;
        self.signal_event(handle)
    }

    fn signal_event(&mut self, handle: RuntimeHandle) -> Result<bool, RuntimeStateError> {
        let Some(record) = self
            .objects
            .live_object_for_handle(handle, RuntimeObjectKind::Event)?
        else {
            return Ok(false);
        };
        let RuntimeObjectKey::Event {
            event_id,
            manual_reset,
        } = record.key
        else {
            return Err(RuntimeStateError::new(
                "event handle resolved to a different object identity",
            ));
        };
        let (object_id, wait_state) =
            self.objects
                .wait_state_mut_for_identity(&RuntimeObjectKey::Event {
                    event_id,
                    manual_reset,
                })?;
        let wakes = self.scheduler.wake_object_waiters(
            &self.threads,
            object_id,
            RuntimeWaitState::Signaled { exit_code: 0 },
            &[],
            if manual_reset { usize::MAX } else { 1 },
            self.process.timer_time_100ns(),
        )?;
        *wait_state = if !manual_reset && wakes != 0 {
            RuntimeWaitState::Nonsignaled
        } else {
            RuntimeWaitState::Signaled { exit_code: 0 }
        };
        Ok(true)
    }

    pub(crate) fn reset_event(&mut self, handle: RuntimeHandle) -> Result<bool, RuntimeStateError> {
        self.require_event_operation_state()?;
        if self
            .objects
            .live_object_for_handle(handle, RuntimeObjectKind::Event)?
            .is_none()
        {
            return Ok(false);
        }
        self.objects.set_wait_state(
            handle,
            RuntimeObjectKind::Event,
            RuntimeWaitState::Nonsignaled,
        )?;
        Ok(true)
    }

    pub(crate) fn try_wait_event(
        &mut self,
        handle: RuntimeHandle,
    ) -> Result<bool, RuntimeStateError> {
        self.require_event_operation_state()?;
        self.consume_event_signal(handle)
    }

    // Used after the message-wait owner has checked its suspended host request.
    pub(crate) fn consume_event_signal(
        &mut self,
        handle: RuntimeHandle,
    ) -> Result<bool, RuntimeStateError> {
        let event = self.event_for_handle(handle)?;
        if !matches!(event.wait_state, RuntimeWaitState::Signaled { .. }) {
            return Ok(false);
        }
        if !event.manual_reset {
            self.objects.set_wait_state(
                handle,
                RuntimeObjectKind::Event,
                RuntimeWaitState::Nonsignaled,
            )?;
        }
        Ok(true)
    }

    pub(crate) fn create_waitable_timer_a(
        &mut self,
        manual_reset: bool,
    ) -> Result<RuntimeHandle, RuntimeStateError> {
        self.require_waitable_timer_operation_state()?;
        let timer = self.timers.new_waitable_timer(manual_reset)?;
        let timer_id = timer.id;
        let key = RuntimeObjectKey::WaitableTimer {
            timer_id: timer_id.0,
        };
        let (_, handle) = self.objects.register_object_and_open_handle(key)?;
        self.timers.insert_waitable_timer(timer);
        Ok(handle)
    }

    pub(crate) fn waitable_timer_for_handle(
        &self,
        handle: RuntimeHandle,
    ) -> Result<&RuntimeWaitableTimer, RuntimeStateError> {
        let record = self
            .objects
            .object_for_handle(handle, RuntimeObjectKind::WaitableTimer)?;
        let RuntimeObjectKey::WaitableTimer { timer_id } = &record.key else {
            return Err(RuntimeStateError::new(
                "waitable timer handle resolved to a different object identity",
            ));
        };
        self.timers
            .waitable_timer(RuntimeWaitableTimerId(*timer_id))
    }

    pub(crate) fn set_waitable_timer(
        &mut self,
        handle: RuntimeHandle,
        relative_due_time_100ns: u64,
    ) -> Result<(), RuntimeStateError> {
        self.require_waitable_timer_operation_state()?;
        let record = self
            .objects
            .object_for_handle(handle, RuntimeObjectKind::WaitableTimer)?;
        let RuntimeObjectKey::WaitableTimer { timer_id } = &record.key else {
            return Err(RuntimeStateError::new(
                "waitable timer handle resolved to a different object identity",
            ));
        };
        let timer_id = RuntimeWaitableTimerId(*timer_id);
        let object_id = record.id;
        let timer_clock = self.process.timer_time_100ns();
        let elapsed_arm = self
            .timers
            .waitable_timer(timer_id)?
            .arm
            .filter(|arm| arm.deadline_time_100ns <= timer_clock);
        let armed_timer =
            self.timers
                .armed_waitable_timer(timer_id, relative_due_time_100ns, timer_clock)?;
        self.objects.set_wait_state(
            handle,
            RuntimeObjectKind::WaitableTimer,
            RuntimeWaitState::Nonsignaled,
        )?;
        if let Some(arm) = elapsed_arm {
            // Rearming cancels future expiration, but cannot erase a signal
            // already delivered to eligible blocked waits on the shared clock.
            self.scheduler.signal_timer_waiters(
                object_id,
                arm.deadline_time_100ns,
                armed_timer.manual_reset,
            );
        }
        self.timers.replace_waitable_timer(armed_timer);
        Ok(())
    }

    pub(crate) fn begin_wait_for_single_object_ex(
        &mut self,
        handle: RuntimeHandle,
        timeout_milliseconds: u32,
        alertable: bool,
    ) -> Result<RuntimeWaitForSingleObjectExBegin, RuntimeStateError> {
        self.require_waitable_timer_operation_state()?;
        let started_at_time_100ns = self.process.timer_time_100ns();
        let object = self
            .objects
            .object_for_handle(handle, RuntimeObjectKind::WaitableTimer)?;
        let RuntimeObjectKey::WaitableTimer { timer_id } = object.key else {
            return Err(RuntimeStateError::new(
                "waitable timer handle resolved to a different object identity",
            ));
        };
        let timer_id = RuntimeWaitableTimerId(timer_id);
        let timer = *self.timers.waitable_timer(timer_id)?;
        let object_wait_state = object
            .wait_state
            .ok_or_else(|| RuntimeStateError::new("waitable timer object has no wait state"))?;
        if matches!(object_wait_state, RuntimeWaitState::Signaled { .. }) {
            return self
                .poll_waitable_timer(handle)
                .map(|outcome| RuntimeWaitForSingleObjectExBegin::Immediate { outcome });
        }

        let timeout_100ns = u64::from(timeout_milliseconds)
            .checked_mul(10_000)
            .ok_or_else(|| {
                RuntimeStateError::new("wait timeout exceeds the logical timer clock unit range")
            })?;
        let timeout_deadline_time_100ns = started_at_time_100ns
            .checked_add(timeout_100ns)
            .ok_or_else(|| {
                RuntimeStateError::new("wait timeout deadline exceeds the logical timer clock")
            })?;
        let expiration_arm = timer
            .arm
            .filter(|arm| arm.deadline_time_100ns <= timeout_deadline_time_100ns);
        let target_time_100ns = expiration_arm.map_or(timeout_deadline_time_100ns, |arm| {
            arm.deadline_time_100ns.max(started_at_time_100ns)
        });
        if target_time_100ns == started_at_time_100ns {
            return self
                .poll_waitable_timer(handle)
                .map(|outcome| RuntimeWaitForSingleObjectExBegin::Immediate { outcome });
        }

        let delay_100ns = target_time_100ns - started_at_time_100ns;
        let delay_milliseconds = u32::try_from(delay_100ns.div_ceil(10_000)).map_err(|_| {
            RuntimeStateError::new(
                "waitable timer pacing delay exceeds the bounded browser transport",
            )
        })?;
        let host_plan: RuntimeHostRequest = self
            .scheduler
            .checked_host_request(RUNTIME_WAITABLE_TIMER_PACING_SERVICE.to_owned())?;
        let pacing = RuntimeWaitableTimerPacing {
            request_id: host_plan.request_id,
            delay_milliseconds,
            target_time_100ns,
            started_at_time_100ns,
            timer_before: timer,
            outcome: if expiration_arm.is_some() {
                RuntimeWaitForSingleObjectExOutcome::ObjectSignaled
            } else {
                RuntimeWaitForSingleObjectExOutcome::TimedOut
            },
            signaled: false,
        };
        let registration = self.scheduler.checked_host_wait_registration(
            self.current_thread_id(),
            object.id,
            handle,
            timeout_deadline_time_100ns,
            alertable,
            pacing,
        )?;

        self.scheduler.publish_wait_registration(registration);
        self.scheduler.publish_host_request(host_plan);
        Ok(RuntimeWaitForSingleObjectExBegin::Deferred { registration })
    }

    fn poll_waitable_timer(
        &mut self,
        handle: RuntimeHandle,
    ) -> Result<RuntimeWaitForSingleObjectExOutcome, RuntimeStateError> {
        let current_time_100ns = self.process.timer_time_100ns();
        let object = self
            .objects
            .object_for_handle(handle, RuntimeObjectKind::WaitableTimer)?;
        let RuntimeObjectKey::WaitableTimer { timer_id } = &object.key else {
            return Err(RuntimeStateError::new(
                "waitable timer handle resolved to a different object identity",
            ));
        };
        let timer_id = RuntimeWaitableTimerId(*timer_id);
        let timer = *self.timers.waitable_timer(timer_id)?;
        let object_wait_state = object
            .wait_state
            .ok_or_else(|| RuntimeStateError::new("waitable timer object has no wait state"))?;
        if matches!(object_wait_state, RuntimeWaitState::Signaled { .. }) {
            if timer.arm.is_some() {
                return Err(RuntimeStateError::new(
                    "signaled waitable timer retains a live one-shot arm",
                ));
            }
            if !timer.manual_reset {
                self.objects.set_wait_state(
                    handle,
                    RuntimeObjectKind::WaitableTimer,
                    RuntimeWaitState::Nonsignaled,
                )?;
            }
            return Ok(RuntimeWaitForSingleObjectExOutcome::ObjectSignaled);
        }

        let Some(arm) = timer
            .arm
            .filter(|arm| arm.deadline_time_100ns <= current_time_100ns)
        else {
            return Ok(RuntimeWaitForSingleObjectExOutcome::TimedOut);
        };
        let next_wait_state = if timer.manual_reset {
            RuntimeWaitState::Signaled { exit_code: 0 }
        } else {
            RuntimeWaitState::Nonsignaled
        };
        let object_id = object.id;
        let (_, wait_state) =
            self.objects
                .wait_state_mut_for_identity(&RuntimeObjectKey::WaitableTimer {
                    timer_id: timer_id.0,
                })?;
        self.timers
            .expire_waitable_timer(timer_id, arm.generation)?;
        *wait_state = next_wait_state;
        if timer.manual_reset {
            self.scheduler
                .signal_timer_waiters(object_id, arm.deadline_time_100ns, true);
        }
        Ok(RuntimeWaitForSingleObjectExOutcome::ObjectSignaled)
    }

    pub(crate) fn plan_waitable_timer_pacing_completion(
        &self,
        request_id: u64,
    ) -> Result<RuntimeWaitableTimerPacingCompletionPlan, RuntimeStateError> {
        let scheduler = self.scheduler.validate_host_completion(request_id)?;
        if scheduler.service != RUNTIME_WAITABLE_TIMER_PACING_SERVICE {
            return Err(RuntimeStateError::new(
                "waitable timer pacing completion belongs to another host service",
            ));
        }
        let registration = self
            .scheduler
            .wait_registration_for_thread(scheduler.thread_id)?
            .ok_or_else(|| {
                RuntimeStateError::new("timer pacing host request has no wait registration")
            })?;
        let pacing = registration.waitable_timer_pacing.ok_or_else(|| {
            RuntimeStateError::new("timer pacing wait registration has no pacing contract")
        })?;
        if pacing.request_id != request_id {
            return Err(RuntimeStateError::new(
                "timer pacing wait registration belongs to another host request",
            ));
        }
        if pacing.target_time_100ns <= pacing.started_at_time_100ns
            || pacing.delay_milliseconds == 0
        {
            return Err(RuntimeStateError::new(
                "pending timer pacing wait must advance logical time by a positive delay",
            ));
        }
        let object = self
            .objects
            .object_for_handle(registration.handle, RuntimeObjectKind::WaitableTimer)?;
        if object.id != registration.object_id {
            return Err(RuntimeStateError::new(
                "timer pacing object identity changed before completion",
            ));
        }
        let object_signaled =
            object.wait_state == Some(RuntimeWaitState::Signaled { exit_code: 0 });
        if !object_signaled && object.wait_state != Some(RuntimeWaitState::Nonsignaled) {
            return Err(RuntimeStateError::new(
                "timer pacing object identity or wait state changed before completion",
            ));
        }
        let RuntimeObjectKey::WaitableTimer { timer_id } = object.key else {
            return Err(RuntimeStateError::new(
                "waitable timer pacing handle resolved to another object identity",
            ));
        };
        let timer_id = RuntimeWaitableTimerId(timer_id);
        let current_timer = *self.timers.waitable_timer(timer_id)?;
        if timer_id != pacing.timer_before.id
            || current_timer.id != timer_id
            || current_timer.manual_reset != pacing.timer_before.manual_reset
            || (object_signaled
                && (current_timer.arm.is_some() || current_timer.last_expiration.is_none()))
        {
            return Err(RuntimeStateError::new(
                "timer pacing lost its live timer identity or signal state",
            ));
        }
        let completion_time_100ns = self.process.timer_time_100ns();
        if completion_time_100ns < pacing.started_at_time_100ns {
            return Err(RuntimeStateError::new(
                "process-wide logical clock regressed while timer pacing was pending",
            ));
        }

        let timeout_deadline_time_100ns =
            registration.timeout_deadline_time_100ns.ok_or_else(|| {
                RuntimeStateError::new("timer pacing wait registration has no timeout deadline")
            })?;
        // Validate the browser delay that actually completed. Rearming may
        // change the live arm, but cannot change this request's elapsed delay.
        match pacing.outcome {
            RuntimeWaitForSingleObjectExOutcome::ObjectSignaled => {
                let arm = pacing.timer_before.arm.ok_or_else(|| {
                    RuntimeStateError::new(
                        "signaled timer pacing completion has no matching live arm",
                    )
                })?;
                if arm.deadline_time_100ns != pacing.target_time_100ns
                    || arm.deadline_time_100ns > timeout_deadline_time_100ns
                {
                    return Err(RuntimeStateError::new(
                        "timer arm deadline does not match the pending pacing target",
                    ));
                }
            }
            RuntimeWaitForSingleObjectExOutcome::TimedOut => {
                if pacing.target_time_100ns != timeout_deadline_time_100ns
                    || pacing
                        .timer_before
                        .arm
                        .is_some_and(|arm| arm.deadline_time_100ns <= timeout_deadline_time_100ns)
                {
                    return Err(RuntimeStateError::new(
                        "timer pacing timeout no longer precedes the live arm deadline",
                    ));
                }
            }
        }
        // The old browser acknowledgement establishes only its old target.
        // A later rearm needs another real delay, with the original timeout
        // and the existing parked thread/request identities preserved.
        let effective_time_100ns = completion_time_100ns.max(pacing.target_time_100ns);
        let clock = self.process.project_clock_advance(effective_time_100ns)?;
        let expiration_arm = current_timer.arm.filter(|arm| {
            !pacing.signaled
                && arm.deadline_time_100ns <= effective_time_100ns
                && arm.deadline_time_100ns <= timeout_deadline_time_100ns
        });
        let eligible_object_signal = object_signaled
            && current_timer.last_expiration.is_some_and(|expiration| {
                expiration.deadline_time_100ns <= timeout_deadline_time_100ns
            });
        let signaled = pacing.signaled || eligible_object_signal || expiration_arm.is_some();
        let outcome = if signaled {
            RuntimeWaitForSingleObjectExOutcome::ObjectSignaled
        } else {
            RuntimeWaitForSingleObjectExOutcome::TimedOut
        };
        let repace = if !signaled && effective_time_100ns < timeout_deadline_time_100ns {
            let next_arm = current_timer
                .arm
                .filter(|arm| arm.deadline_time_100ns <= timeout_deadline_time_100ns);
            let target_time_100ns =
                next_arm.map_or(timeout_deadline_time_100ns, |arm| arm.deadline_time_100ns);
            let delay_milliseconds =
                u32::try_from((target_time_100ns - effective_time_100ns).div_ceil(10_000))
                    .ok()
                    .filter(|delay| *delay > 0 && *delay <= i32::MAX as u32)
                    .ok_or_else(|| {
                        RuntimeStateError::new("rearmed timer wait exceeds the browser delay bound")
                    })?;
            Some(RuntimeWaitableTimerPacing {
                request_id,
                delay_milliseconds,
                target_time_100ns,
                started_at_time_100ns: effective_time_100ns,
                timer_before: current_timer,
                outcome: if next_arm.is_some() {
                    RuntimeWaitForSingleObjectExOutcome::ObjectSignaled
                } else {
                    RuntimeWaitForSingleObjectExOutcome::TimedOut
                },
                signaled: false,
            })
        } else {
            None
        };
        let expiration = expiration_arm.map(|arm| (timer_id, arm.generation));
        let wait_updates = if expiration_arm.is_some()
            || (eligible_object_signal && !current_timer.manual_reset)
        {
            Some(self.objects.plan_wait_state_updates(&[(
                RuntimeObjectKey::WaitableTimer {
                    timer_id: timer_id.0,
                },
                if current_timer.manual_reset {
                    RuntimeWaitState::Signaled { exit_code: 0 }
                } else {
                    RuntimeWaitState::Nonsignaled
                },
            )])?)
        } else {
            None
        };
        if let Some((timer_id, generation)) = expiration {
            self.timers
                .validate_waitable_timer_expiration(timer_id, generation)?;
        }

        Ok(RuntimeWaitableTimerPacingCompletionPlan {
            scheduler,
            registration,
            clock,
            expiration,
            wait_updates,
            outcome,
            repace,
        })
    }

    pub(crate) fn commit_waitable_timer_pacing_completion(
        &mut self,
        plan: RuntimeWaitableTimerPacingCompletionPlan,
    ) {
        debug_assert!(matches!(
            self.scheduler.wait_registration_for_thread(plan.registration.thread_id),
            Ok(Some(registration)) if registration == plan.registration
        ));
        self.process.apply_clock_advance(plan.clock);
        if let Some(pacing) = plan.repace {
            self.scheduler
                .replace_timer_wait_pacing(plan.registration, pacing);
            return;
        }
        self.scheduler
            .commit_remove_wait_registration(plan.registration);
        if let Some((timer_id, generation)) = plan.expiration {
            let timer = self
                .timers
                .waitable_timer(timer_id)
                .expect("validated live timer");
            if timer.manual_reset {
                self.scheduler.signal_timer_waiters(
                    plan.registration.object_id,
                    timer.arm.expect("validated arm").deadline_time_100ns,
                    true,
                );
            }
            self.timers
                .expire_waitable_timer(timer_id, generation)
                .expect("validated live timer expiration");
        }
        if let Some(wait_updates) = plan.wait_updates {
            self.objects.commit_wait_state_updates(wait_updates);
        }
        let completion = self.scheduler.commit_host_completion(plan.scheduler);
        debug_assert_eq!(
            completion.request_id,
            plan.registration.waitable_timer_pacing.unwrap().request_id
        );
    }

    pub(crate) fn load_shared_icon(
        &mut self,
        resource: RuntimeIconResource,
    ) -> Result<u32, RuntimeStateError> {
        self.require_user32_operation_state()?;
        let image = self
            .modules
            .images()
            .image(resource.module_id)
            .map_err(runtime_module_image_error)?;
        if image.image_range.start() != resource.module_load_base {
            return Err(RuntimeStateError::new(
                "shared icon resource does not match its registered module image",
            ));
        }
        let resources = image.resources.as_ref().ok_or_else(|| {
            RuntimeStateError::new("registered module image has no PE resource catalog")
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
            return Err(RuntimeStateError::new(
                "shared icon resource does not match unique group-icon and icon catalog entries",
            ));
        }
        self.user32.load_shared_icon(resource)
    }

    pub(crate) fn icon_option(
        &self,
        handle: u32,
    ) -> Result<Option<&RuntimeIcon>, RuntimeStateError> {
        self.user32.icon_option(handle)
    }

    #[cfg(test)]
    pub(crate) fn icons(&self) -> &BTreeMap<RuntimeIconId, RuntimeIcon> {
        self.user32.icons()
    }

    pub(crate) fn load_shared_cursor(
        &mut self,
        source: RuntimeCursorSource,
    ) -> Result<u32, RuntimeStateError> {
        self.require_user32_operation_state()?;
        if let RuntimeCursorSource::ModuleResource { resource } = &source {
            let image = self
                .modules
                .images()
                .image(resource.module_id)
                .map_err(runtime_module_image_error)?;
            if image.image_range.start() != resource.module_load_base {
                return Err(RuntimeStateError::new(
                    "shared cursor resource does not match its registered module image",
                ));
            }
            let resources = image.resources.as_ref().ok_or_else(|| {
                RuntimeStateError::new("registered module image has no PE resource catalog")
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
                return Err(RuntimeStateError::new(
                    "shared cursor resource does not match unique group-cursor and cursor catalog entries",
                ));
            }
        }
        self.user32.load_shared_cursor(source)
    }

    pub(crate) fn cursor_option(
        &self,
        handle: u32,
    ) -> Result<Option<&RuntimeCursor>, RuntimeStateError> {
        self.user32.cursor_option(handle)
    }

    pub(crate) fn current_cursor(&self) -> Result<Option<&RuntimeCursor>, RuntimeStateError> {
        self.user32.current_cursor()
    }

    pub(crate) fn set_current_cursor(
        &mut self,
        handle: Option<u32>,
    ) -> Result<u32, RuntimeStateError> {
        self.require_user32_operation_state()?;
        self.user32.set_current_cursor(handle)
    }

    #[cfg(test)]
    pub(crate) fn cursors(&self) -> &BTreeMap<RuntimeCursorId, RuntimeCursor> {
        self.user32.cursors()
    }

    pub(crate) fn show_cursor(&mut self, show: bool) -> Result<i32, RuntimeStateError> {
        self.require_user32_operation_state()?;
        self.threads
            .change_cursor_display_count(self.current_thread_id(), show)
    }

    pub(crate) fn current_thread_cursor_display_count(&self) -> Result<i32, RuntimeStateError> {
        self.threads.cursor_display_count(self.current_thread_id())
    }

    pub(crate) fn register_window_class(
        &mut self,
        registration: RuntimeWindowClassRegistration,
    ) -> Result<u16, RuntimeStateError> {
        self.require_user32_operation_state()?;
        let image = self
            .modules
            .images()
            .image(registration.module.module_id)
            .map_err(runtime_module_image_error)?;
        if image.image_range.start() != registration.module.load_base {
            return Err(RuntimeStateError::new(
                "window class module identity does not match its registered image",
            ));
        }
        let expected = self.describe_window_procedure(
            registration.window_procedure.address,
            registration.module.load_base,
        )?;
        if expected != registration.window_procedure {
            return Err(RuntimeStateError::new(
                "window class WndProc does not belong to its registered module image",
            ));
        }
        self.user32.register_window_class(registration)
    }

    pub(crate) fn describe_window_procedure(
        &self,
        address: GuestAddress,
        instance: GuestAddress,
    ) -> Result<RuntimeWindowProcedure, RuntimeStateError> {
        let is_default = ["DefWindowProcA", "DefWindowProcW"].iter().any(|name| {
            self.dynamic_endpoint_by_name("user32.dll", name).is_some_and(|e| e.address == address)
                || self.modules.bindings().values().flat_map(|binding| &binding.imports).any(|import| {
                    matches!(&import.resolution, super::modules::RuntimeGuestImportResolution::RuntimeExport { endpoint }
                        if endpoint.address == address && matches!(&endpoint.export, crate::imports::RuntimeExport::Name { module, name: export_name }
                            if module.eq_ignore_ascii_case("user32.dll") && export_name == name))
                })
        });
        if is_default {
            return Ok(RuntimeWindowProcedure {
                module_id: RuntimeModuleId(0),
                address,
                rva: crate::pe32::Rva(0),
                runtime_default: true,
            });
        }
        let image = self
            .module_image_by_load_base(instance)
            .map_err(runtime_module_image_error)?
            .ok_or_else(|| RuntimeStateError::new("WndProc instance has no live mapped image"))?;
        if address.0 <= image.image_range.start().0
            || u64::from(address.0) >= image.image_range.end()
        {
            return Err(RuntimeStateError::new(
                "WndProc is neither a registered default procedure nor in its instance image",
            ));
        }
        self.memory
            .validate_access(address, 1, MemoryAccess::Execute)
            .map_err(RuntimeStateError::from_memory)?;
        Ok(RuntimeWindowProcedure {
            module_id: image.module_id,
            address,
            rva: crate::pe32::Rva(address.0 - image.image_range.start().0),
            runtime_default: false,
        })
    }

    pub(crate) fn set_window_long_data(
        &mut self,
        handle: u32,
        index: i32,
        value: u32,
    ) -> Result<u32, RuntimeStateError> {
        self.require_user32_operation_state()?;
        let window = self.user32.window(handle)?;
        let procedure = if index == -4 {
            Some(self.describe_window_procedure(
                GuestAddress(value),
                window.creation.instance.load_base,
            )?)
        } else {
            None
        };
        self.user32.set_window_long_data(
            handle,
            self.current_thread_id(),
            procedure,
            (index == -21).then_some(value),
        )
    }

    pub(crate) fn window_class(&self, atom: u16) -> Result<&RuntimeWindowClass, RuntimeStateError> {
        self.user32.window_class(atom)
    }

    pub(crate) fn top_level_window_before(&self, before: u64) -> Option<&RuntimeWindow> {
        self.user32.top_level_window_before(before)
    }

    #[cfg(test)]
    pub(crate) fn window_class_by_name(
        &self,
        module_id: RuntimeModuleId,
        name: &str,
    ) -> Result<&RuntimeWindowClass, RuntimeStateError> {
        self.user32.window_class_by_name(module_id, name)
    }

    pub(crate) fn window_class_by_name_option(
        &self,
        module_id: RuntimeModuleId,
        name: &str,
    ) -> Result<Option<&RuntimeWindowClass>, RuntimeStateError> {
        self.user32.window_class_by_name_option(module_id, name)
    }

    #[cfg(test)]
    pub(crate) fn window_classes(&self) -> &BTreeMap<RuntimeWindowClassId, RuntimeWindowClass> {
        self.user32.window_classes()
    }

    pub(crate) fn checked_window_creation(
        &self,
        request: RuntimeWindowCreationRequest,
    ) -> Result<RuntimeWindow, RuntimeStateError> {
        self.require_user32_operation_state()?;
        self.validate_window_creation_authority(&request)?;
        self.user32.checked_window_creation(request)
    }

    pub(crate) fn publish_window_creation(&mut self, window: RuntimeWindow) {
        self.user32.publish_window_creation(window);
    }

    pub(crate) fn preview_window_creation_callback(
        &self,
        window_id: RuntimeWindowId,
        message: RuntimeWindowCreationMessage,
        result: u32,
    ) -> Result<RuntimeWindowCreationCallbackAction, RuntimeStateError> {
        self.require_user32_operation_state()?;
        self.require_window_owner_thread(window_id)?;
        self.user32
            .preview_window_creation_callback(window_id, message, result)
    }

    pub(crate) fn complete_window_creation_callback(
        &mut self,
        window_id: RuntimeWindowId,
        message: RuntimeWindowCreationMessage,
        result: u32,
    ) -> Result<RuntimeWindowCreationCallbackAction, RuntimeStateError> {
        self.require_user32_operation_state()?;
        self.require_window_owner_thread(window_id)?;
        self.user32
            .complete_window_creation_callback(window_id, message, result)
    }

    pub(crate) fn preview_creation_show(
        &self,
        window: &RuntimeWindow,
    ) -> Result<RuntimeSetWindowPosOperation, RuntimeStateError> {
        self.user32.preview_creation_show(window)
    }

    pub(crate) fn window(&self, handle: u32) -> Result<&RuntimeWindow, RuntimeStateError> {
        self.user32.window(handle)
    }

    pub(crate) fn begin_window_destruction(
        &mut self,
        id: RuntimeWindowId,
    ) -> Result<(), RuntimeStateError> {
        self.require_user32_operation_state()?;
        self.require_window_owner_thread(id)?;
        self.user32.begin_window_destruction(id)
    }

    pub(crate) fn publish_move_window(
        &mut self,
        id: RuntimeWindowId,
        previous: RuntimeWindowGeometry,
        next: RuntimeWindowGeometry,
        repaint: bool,
    ) -> Result<(), RuntimeStateError> {
        self.require_user32_operation_state()?;
        self.require_window_owner_thread(id)?;
        self.user32.validate_move_window_publication(id, previous)?;
        if (previous.client_width, previous.client_height)
            != (next.client_width, next.client_height)
        {
            self.gdi.resize_window_surface(
                id,
                previous.client_width,
                previous.client_height,
                next.client_width,
                next.client_height,
            )?;
        }
        self.user32.publish_move_window(id, previous, next, repaint)
    }

    pub(crate) fn publish_hide_window(
        &mut self,
        id: RuntimeWindowId,
        expected: RuntimeWindowGeometry,
        deactivate: bool,
    ) -> Result<(), RuntimeStateError> {
        self.require_user32_operation_state()?;
        self.require_window_owner_thread(id)?;
        self.user32.publish_hide_window(id, expected, deactivate)
    }

    pub(crate) fn destroy_window_notifications(
        &self,
        id: RuntimeWindowId,
    ) -> Result<u32, RuntimeStateError> {
        self.user32.destroy_window_notifications(id)
    }

    pub(crate) fn finish_window_destruction(
        &mut self,
        id: RuntimeWindowId,
    ) -> Result<(), RuntimeStateError> {
        self.require_user32_operation_state()?;
        self.require_window_owner_thread(id)?;
        let window = self.user32.finish_window_destruction(id)?;
        self.gdi.retire_window(id);
        self.timers.retire_window(id);
        for (atom, (_, owned)) in window.properties {
            if owned {
                let _ = self.global_atoms.delete(atom);
            }
        }
        Ok(())
    }

    pub(crate) fn window_option(
        &self,
        handle: u32,
    ) -> Result<Option<&RuntimeWindow>, RuntimeStateError> {
        self.user32.window_option(handle)
    }

    pub(crate) fn window_is_iconic(&self, handle: u32) -> Result<Option<bool>, RuntimeStateError> {
        self.user32.window_is_iconic(handle)
    }

    pub(crate) fn window_by_id(
        &self,
        id: RuntimeWindowId,
    ) -> Result<&RuntimeWindow, RuntimeStateError> {
        self.user32.window_by_id(id)
    }

    #[cfg(test)]
    pub(crate) fn windows(&self) -> &BTreeMap<RuntimeWindowId, RuntimeWindow> {
        self.user32.windows()
    }

    #[cfg(test)]
    pub(crate) fn paint_sessions(&self) -> &BTreeMap<RuntimeWindowId, RuntimePaintSession> {
        self.user32.paint_sessions()
    }

    pub(crate) fn get_system_menu(
        &mut self,
        window_handle: u32,
        close_item_command: u32,
    ) -> Result<Option<u32>, RuntimeStateError> {
        self.require_user32_operation_state()?;
        self.user32
            .get_system_menu(window_handle, close_item_command)
    }

    pub(crate) fn enable_system_menu_close_item(
        &mut self,
        menu_handle: u32,
        close_item_command: u32,
    ) -> Result<Option<u32>, RuntimeStateError> {
        self.require_user32_operation_state()?;
        self.user32
            .enable_system_menu_close_item(menu_handle, close_item_command)
    }

    pub(crate) fn reset_system_menu(&mut self, handle: u32) -> Result<bool, RuntimeStateError> {
        self.require_user32_operation_state()?;
        self.user32.reset_system_menu(handle)
    }

    pub(crate) fn get_default_ime_window(
        &mut self,
        window_handle: u32,
    ) -> Result<Option<u32>, RuntimeStateError> {
        self.require_user32_operation_state()?;
        self.require_imm32_operation_state()?;
        // The default IME window belongs to the HWND's thread, not the caller.
        // A query from another thread must not use synchronous-message ownership rules.
        let Some(window) = self.user32.window_option(window_handle)? else {
            return Ok(None);
        };
        let owner_thread_id = window.creation.owner_thread_id;
        let target = self
            .user32
            .validate_default_ime_window(window_handle, owner_thread_id)?;
        // IMM32 validates before publishing; no User32 state changes during that
        // call. Both owners therefore publish without a later fallible operation.
        self.imm32.ensure_window(target)?;
        Ok(Some(self.user32.ensure_default_ime_window(owner_thread_id)))
    }

    pub(crate) fn associate_ime_context(
        &mut self,
        window_handle: u32,
        context_handle: u32,
    ) -> Result<u32, RuntimeStateError> {
        self.require_user32_operation_state()?;
        self.require_imm32_operation_state()?;
        // Detaching a context is allowed across threads in this process. Attaching
        // a non-null context still requires the caller to own the target window.
        let owner_thread_id = if context_handle == 0 {
            self.user32.window(window_handle)?.creation.owner_thread_id
        } else {
            self.current_thread_id()
        };
        let target = self
            .user32
            .resolve_ime_app_window_target(window_handle, owner_thread_id)?;
        self.imm32.associate_context(target, context_handle)
    }

    pub(crate) fn get_ime_context(
        &mut self,
        window_handle: u32,
    ) -> Result<Option<u32>, RuntimeStateError> {
        self.require_user32_operation_state()?;
        self.require_imm32_operation_state()?;
        let Some(window) = self.user32.window_option(window_handle)? else {
            return Ok(None);
        };
        if window.creation.owner_thread_id != self.current_thread_id() {
            return Err(RuntimeStateError::new(
                "IME context access requires the window owner thread",
            ));
        }
        let target = self
            .user32
            .resolve_ime_app_window_target(window_handle, self.current_thread_id())?;
        self.imm32.get_context(target).map(Some)
    }

    pub(crate) fn control_ime_status_window(
        &mut self,
        window_handle: u32,
        request: RuntimeImeStatusWindowRequest,
    ) -> Result<(), RuntimeStateError> {
        self.require_user32_operation_state()?;
        self.user32
            .control_ime_status_window(window_handle, request)
    }

    #[cfg(test)]
    pub(crate) fn ime_window(&self, handle: u32) -> Result<&RuntimeImeWindow, RuntimeStateError> {
        self.user32.ime_window(handle)
    }

    #[cfg(test)]
    pub(crate) fn ime_windows(&self) -> &BTreeMap<RuntimeImeWindowId, RuntimeImeWindow> {
        self.user32.ime_windows()
    }

    pub(crate) fn ime_context(&self, handle: u32) -> Result<&RuntimeImeContext, RuntimeStateError> {
        self.imm32.context(handle)
    }

    #[cfg(test)]
    pub(crate) fn ime_contexts(&self) -> &BTreeMap<RuntimeImeContextId, RuntimeImeContext> {
        self.imm32.contexts()
    }

    #[cfg(test)]
    pub(crate) fn ime_context_associations(
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
    ) -> Result<u32, RuntimeStateError> {
        self.require_user32_operation_state()?;
        let (window_id, owner_thread_id) = self.resolve_window_timer_target(window_handle)?;
        let timer_clock = self.process.timer_time_100ns();
        self.timers.register_window_timer(
            RuntimeWindowTimerRegistrationRequest {
                window_id,
                window_handle,
                timer_id,
                owner_thread_id,
                interval_milliseconds,
                callback_address,
            },
            timer_clock,
        )
    }

    pub(crate) fn kill_window_timer(
        &mut self,
        window_handle: u32,
        timer_id: u32,
    ) -> Result<bool, RuntimeStateError> {
        self.require_user32_operation_state()?;
        let (window, owner) = self.resolve_window_timer_target(window_handle)?;
        self.timers.kill_window_timer(window, timer_id, owner)
    }

    #[cfg(test)]
    pub(crate) fn window_timers(&self) -> &BTreeMap<RuntimeWindowTimerId, RuntimeWindowTimer> {
        self.timers.window_timers()
    }

    pub(crate) fn resolve_window_timer_callback(
        &self,
        window_handle: u32,
        timer_id: u32,
        callback_address: GuestAddress,
    ) -> Result<GuestAddress, RuntimeStateError> {
        self.require_user32_operation_state()?;
        let window = self.user32.window(window_handle)?;
        let current_thread = self.current_thread_id();
        if window.lifecycle != RuntimeWindowLifecycle::Created
            || window.creation.owner_thread_id != current_thread
        {
            return Err(RuntimeStateError::new(
                "timer message no longer matches its live window, owner, registration, or callback",
            ));
        }
        if let Some(timer) = self
            .timers
            .window_timers()
            .values()
            .find(|timer| timer.window_handle == window_handle && timer.timer_id == timer_id)
            && timer.owner_thread_id == current_thread
            && timer.callback_address == callback_address
            && timer.delivery_count != 0
            && timer.last_delivery.is_some()
            && window.id == timer.window_id
        {
            return Ok(timer.callback_address);
        }
        if self.timers.has_delivered_timer_callback(
            window.id,
            window_handle,
            timer_id,
            current_thread,
            callback_address,
        ) {
            return Ok(callback_address);
        }
        Err(RuntimeStateError::new(
            "timer message no longer matches its live window, owner, registration, or callback",
        ))
    }

    fn resolve_window_timer_target(
        &self,
        window_handle: u32,
    ) -> Result<(RuntimeWindowId, RuntimeThreadId), RuntimeStateError> {
        let window = self.user32.window(window_handle)?;
        if window.lifecycle != RuntimeWindowLifecycle::Created {
            return Err(RuntimeStateError::new(
                "logical window timer registration requires a created window",
            ));
        }
        if window.creation.owner_thread_id != self.current_thread_id() {
            return Err(RuntimeStateError::new(
                "logical window timer registration requires the selected window owner thread",
            ));
        }
        Ok((window.id, window.creation.owner_thread_id))
    }

    pub(crate) fn set_window_style(
        &mut self,
        window_handle: u32,
        requested_style: u32,
    ) -> Result<u32, RuntimeStateError> {
        self.require_user32_operation_state()?;
        self.user32
            .set_window_style(window_handle, self.current_thread_id(), requested_style)
    }

    pub(crate) fn set_window_extended_style(
        &mut self,
        handle: u32,
        style: u32,
    ) -> Result<u32, RuntimeStateError> {
        self.require_user32_operation_state()?;
        self.user32
            .set_window_extended_style(handle, self.current_thread_id(), style)
    }

    pub(crate) fn set_window_file_drop_acceptance(
        &mut self,
        window_handle: u32,
        accepts_files: bool,
    ) -> Result<(), RuntimeStateError> {
        self.require_user32_operation_state()?;
        self.user32
            .set_window_file_drop_acceptance(window_handle, accepts_files)
    }

    pub(crate) fn acquire_window_dc(
        &mut self,
        window_handle: u32,
    ) -> Result<RuntimeGetDcResult, RuntimeStateError> {
        self.require_gdi_operation_state()?;
        let target = self.resolve_gdi_window_target(window_handle)?;
        self.gdi.get_window_dc(target)
    }

    pub(crate) fn default_window_paint(&mut self, handle: u32) -> Result<(), RuntimeStateError> {
        self.require_user32_operation_state()?;
        self.user32.default_paint(self.current_thread_id(), handle)
    }

    pub(crate) fn preview_default_paint_dc(
        &self,
        handle: u32,
    ) -> Result<RuntimeGetDcResult, RuntimeStateError> {
        self.require_gdi_operation_state()?;
        self.gdi
            .preview_window_dc(self.resolve_gdi_window_target(handle)?)
    }

    pub(crate) fn begin_default_window_paint(
        &mut self,
        handle: u32,
        dc: RuntimeGetDcResult,
    ) -> Result<(), RuntimeStateError> {
        self.require_user32_operation_state()?;
        self.require_gdi_operation_state()?;
        let target = self.resolve_gdi_window_target(handle)?;
        if self.gdi.preview_window_dc(target)? != dc {
            return Err(RuntimeStateError::new(
                "default paint HDC identity changed before publication",
            ));
        }
        self.user32
            .default_paint(self.current_thread_id(), handle)?;
        self.gdi.publish_window_dc(target, dc);
        Ok(())
    }

    pub(crate) fn finish_default_window_paint(
        &mut self,
        window_id: RuntimeWindowId,
        handle: u32,
        dc: RuntimeGetDcResult,
    ) -> Result<(), RuntimeStateError> {
        if self.window(handle)?.id != window_id
            || self.gdi.device_context_by_handle(dc.handle)?.id != dc.device_context_id
        {
            return Err(RuntimeStateError::new(
                "default paint callback lost its window or HDC identity",
            ));
        }
        self.release_window_dc(handle, dc.handle)
    }

    pub(crate) fn begin_window_paint(
        &mut self,
        window_handle: u32,
        paint_struct_address: GuestAddress,
    ) -> Result<u32, RuntimeStateError> {
        self.require_user32_operation_state()?;
        self.require_gdi_operation_state()?;
        self.memory
            .validate_access(
                paint_struct_address,
                super::user32::RUNTIME_X86_PAINT_STRUCT_BYTE_LENGTH,
                MemoryAccess::Write,
            )
            .map_err(RuntimeStateError::from_memory)?;
        let target = self.resolve_gdi_window_target(window_handle)?;
        let gdi_result = self.gdi.preview_window_dc(target)?;
        let planned_session = self.user32.validate_begin_paint(
            self.current_thread_id(),
            window_handle,
            paint_struct_address,
            gdi_result.device_context_id,
            gdi_result.handle,
        )?;
        if planned_session.window_id != target.window_id
            || planned_session.window_handle != target.window_handle
            || planned_session.owner_thread_id != target.owner_thread_id
            || planned_session.device_context_id != gdi_result.device_context_id
            || planned_session.device_context_handle != gdi_result.handle
        {
            return Err(RuntimeStateError::new(
                "BeginPaint User32 session and GDI HDC do not identify the same logical window",
            ));
        }
        let mut transaction = self.memory.begin_transaction();
        transaction
            .write(paint_struct_address, &planned_session.paint_struct_bytes)
            .map_err(RuntimeStateError::from_memory)?;
        self.gdi.publish_window_dc(target, gdi_result);
        self.user32.finish_begin_paint(planned_session);
        transaction.commit_transaction();
        Ok(gdi_result.handle)
    }

    pub(crate) fn end_window_paint(
        &mut self,
        window_handle: u32,
        paint_struct_address: GuestAddress,
        paint_struct_bytes: &[u8],
    ) -> Result<(), RuntimeStateError> {
        self.require_user32_operation_state()?;
        self.require_gdi_operation_state()?;
        let session = self.user32.validate_end_paint(
            self.current_thread_id(),
            window_handle,
            paint_struct_address,
            paint_struct_bytes,
        )?;
        let (window_id, paint_generation, device_context_id, device_context_handle) = (
            session.window_id,
            session.paint_generation,
            session.device_context_id,
            session.device_context_handle,
        );
        let target = self.resolve_gdi_window_target(window_handle)?;
        let device_context = self.gdi.device_context_by_handle(device_context_handle)?;
        validate_device_context_target(device_context, target)?;
        if device_context.id != device_context_id {
            return Err(RuntimeStateError::new(
                "EndPaint session and logical HDC do not identify the same GDI object",
            ));
        }
        self.gdi.release_window_dc(
            window_handle,
            device_context_handle,
            self.current_thread_id(),
        )?;
        self.user32.finish_end_paint(window_id, paint_generation);
        Ok(())
    }

    pub(crate) fn acquire_screen_dc(&mut self) -> Result<u32, RuntimeStateError> {
        self.require_gdi_operation_state()?;
        let target = self.resolve_gdi_screen_target()?;
        self.gdi.get_screen_dc(target)
    }

    pub(crate) fn create_compatible_memory_dc(&mut self) -> Result<Option<u32>, RuntimeStateError> {
        self.require_gdi_operation_state()?;
        let target = self.resolve_gdi_screen_target()?;
        self.gdi.create_compatible_dc(target)
    }

    pub(crate) fn create_gdi_compatible_bitmap(
        &mut self,
        dc: u32,
        width: i32,
        height: i32,
    ) -> Result<Option<u32>, RuntimeStateError> {
        self.require_gdi_operation_state()?;
        self.gdi
            .create_compatible_bitmap(dc, self.current_thread_id(), width, height)
    }

    pub(crate) fn gdi_device_bitmap_pixel(
        &mut self,
        dc: u32,
        x: i32,
        y: i32,
        color: Option<u32>,
    ) -> Result<u32, RuntimeStateError> {
        self.require_gdi_operation_state()?;
        self.gdi
            .device_bitmap_pixel(dc, self.current_thread_id(), x, y, color)
    }

    pub(crate) fn delete_compatible_memory_dc(
        &mut self,
        device_context_handle: u32,
    ) -> Result<(), RuntimeStateError> {
        self.require_gdi_operation_state()?;
        self.gdi
            .delete_compatible_dc(device_context_handle, self.current_thread_id())
    }

    pub(crate) fn validate_screen_dc_operation(
        &self,
        device_context_handle: u32,
    ) -> Result<(), RuntimeStateError> {
        self.require_gdi_operation_state()?;
        let target = self.resolve_gdi_screen_target()?;
        self.gdi
            .validate_screen_dc_access(target, device_context_handle, self.current_thread_id())
    }

    pub(crate) fn preview_graphics_surface_dc_acquisition(
        &self,
        surface_com_object_id: RuntimeComObjectId,
    ) -> Result<Option<u32>, RuntimeStateError> {
        self.require_gdi_operation_state()?;
        self.require_graphics_operation_state()?;
        let target = self.resolve_gdi_graphics_surface_target(surface_com_object_id)?;
        self.gdi.preview_graphics_surface_dc(target)
    }

    pub(crate) fn release_graphics_surface_dc(
        &mut self,
        surface_com_object_id: RuntimeComObjectId,
        device_context_handle: u32,
    ) -> Result<bool, RuntimeStateError> {
        self.require_gdi_operation_state()?;
        self.require_graphics_operation_state()?;
        let target = self.resolve_gdi_graphics_surface_target(surface_com_object_id)?;
        self.gdi.release_graphics_surface_dc(
            target,
            device_context_handle,
            self.current_thread_id(),
        )
    }

    pub(crate) fn get_gdi_stock_object(
        &mut self,
        index: i32,
    ) -> Result<Option<u32>, RuntimeStateError> {
        self.require_gdi_operation_state()?;
        self.gdi.get_stock_object(index)
    }

    pub(crate) fn create_gdi_font_a(
        &mut self,
        log_font: RuntimeLogFontA,
    ) -> Result<Option<u32>, RuntimeStateError> {
        self.require_gdi_operation_state()?;
        self.gdi.create_font_a(log_font)
    }

    pub(crate) fn add_gdi_memory_font(
        &mut self,
        bytes: Vec<u8>,
        count_output: GuestAddress,
    ) -> Result<Option<u32>, RuntimeStateError> {
        self.require_gdi_operation_state()?;
        self.gdi
            .add_memory_font(&mut self.memory, bytes, count_output)
    }

    pub(crate) fn remove_gdi_memory_font(
        &mut self,
        handle: u32,
    ) -> Result<bool, RuntimeStateError> {
        self.require_gdi_operation_state()?;
        Ok(self.gdi.remove_memory_font(handle))
    }

    pub(crate) fn create_gdi_dib_section(
        &mut self,
        device_context_handle: u32,
        descriptor: RuntimeGdiDibSectionDescriptor,
        bits_output: GuestAddress,
    ) -> Result<Option<u32>, RuntimeStateError> {
        self.require_gdi_operation_state()?;
        self.memory
            .validate_access(bits_output, 4, MemoryAccess::Write)
            .map_err(RuntimeStateError::from_memory)?;
        let arena = self
            .address_space_allocator
            .allocation_for_purpose(&ProcessAddressAllocationPurpose::GraphicsResourceArena)?;
        if arena.range.base != super::RUNTIME_GRAPHICS_RESOURCE_ARENA_BASE
            || arena.range.byte_length != super::RUNTIME_GRAPHICS_RESOURCE_ARENA_SIZE
            || arena.classification != ProcessAddressAllocationClassification::Graphics
            || arena.release_policy != ProcessAddressAllocationReleasePolicy::Permanent
            || arena.executable
        {
            return Err(RuntimeStateError::new(
                "GDI resource arena does not match the controlled process layout",
            ));
        }
        let thread_id = self.current_thread_id();
        let memory = &mut self.memory;
        let gdi = &mut self.gdi;
        self.address_space_allocator.try_reserve_batch_with(
            vec![(
                Some(arena.id),
                ProcessAddressAllocationRequest {
                    purpose: ProcessAddressAllocationPurpose::GdiResource,
                    preferred_address: None,
                    search_range: arena.range,
                    reserve_size: u64::from(descriptor.pixel_byte_length),
                    alignment: GUEST_PAGE_SIZE as u32,
                    guard_pages: 0,
                    classification: ProcessAddressAllocationClassification::Graphics,
                    release_policy: ProcessAddressAllocationReleasePolicy::Explicit,
                    executable: false,
                },
            )],
            |allocations| {
                let allocation = &allocations[0];
                let backing_range =
                    GuestRange::new(allocation.range.base, allocation.range.byte_length)
                        .map_err(RuntimeStateError::from_memory)?;
                gdi.create_dib_section(
                    device_context_handle,
                    thread_id,
                    descriptor,
                    allocation.id,
                    backing_range,
                    || {
                        if memory
                            .preflight_growth(1, 1, backing_range.page_count())
                            .is_err()
                        {
                            return Ok(None);
                        }
                        memory
                            .preflight_fixed_reservation(
                                backing_range.start(),
                                backing_range.byte_length(),
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
                        transaction
                            .write(bits_output, &backing_range.start().0.to_le_bytes())
                            .map_err(RuntimeStateError::from_memory)?;
                        transaction.commit_transaction();
                        Ok(Some(memory_owner))
                    },
                )
            },
        )
    }

    pub(crate) fn get_gdi_object_bytes(
        &self,
        object_handle: u32,
        requested_byte_length: u32,
    ) -> Result<Option<Vec<u8>>, RuntimeStateError> {
        self.require_gdi_operation_state()?;
        self.gdi.object_bytes(object_handle, requested_byte_length)
    }

    pub(crate) fn get_gdi_object_bytes_w(
        &self,
        object_handle: u32,
        requested_byte_length: u32,
    ) -> Result<Option<Vec<u8>>, RuntimeStateError> {
        self.require_gdi_operation_state()?;
        self.gdi
            .object_bytes_w(object_handle, requested_byte_length)
    }

    pub(crate) fn get_gdi_selected_font_a(
        &self,
        device_context_handle: u32,
    ) -> Result<&RuntimeLogFontA, RuntimeStateError> {
        self.require_gdi_operation_state()?;
        self.gdi
            .selected_font_a(device_context_handle, self.current_thread_id())
    }

    pub(crate) fn get_gdi_text_raster_target(
        &self,
        device_context_handle: u32,
    ) -> Result<Option<RuntimeGdiTextRasterTarget>, RuntimeStateError> {
        self.require_gdi_operation_state()?;
        self.gdi
            .text_raster_target(device_context_handle, self.current_thread_id())
    }

    pub(crate) fn get_gdi_text_font(
        &self,
        dc: u32,
    ) -> Result<super::RuntimeLogFontA, RuntimeStateError> {
        self.require_gdi_operation_state()?;
        self.gdi.text_font(dc, self.current_thread_id())
    }

    pub(crate) fn set_gdi_text_attribute(
        &mut self,
        dc: u32,
        attribute: u32,
        value: u32,
    ) -> Result<Option<u32>, RuntimeStateError> {
        self.require_gdi_operation_state()?;
        self.gdi
            .set_text_attribute(dc, self.current_thread_id(), attribute, value)
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
            return Err(RuntimeStateError::new(
                "browser text raster completion requires a running process and thread",
            ));
        }
        let scheduler = self.scheduler.validate_host_completion(request_id)?;
        if scheduler.service != expected_service || scheduler.thread_id != self.current_thread_id()
        {
            return Err(RuntimeStateError::new(
                "browser text raster completion does not own the current waiting thread",
            ));
        }
        self.gdi
            .text_raster_target(device_context_handle, self.current_thread_id())?
            .ok_or_else(|| {
                RuntimeStateError::new(
                    "pending text raster device context is no longer live or owned by its thread",
                )
            })
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn commit_pending_gdi_text_raster(
        &mut self,
        request_id: u64,
        expected_service: &str,
        target: RuntimeGdiTextRasterTarget,
        byte_offset: u32,
        expected_bytes: &[u8],
        replacement_bytes: &[u8],
    ) -> Result<RuntimeHostCompletion, RuntimeStateError> {
        let scheduler = self.scheduler.validate_host_completion(request_id)?;
        if scheduler.service != expected_service || scheduler.thread_id != self.current_thread_id()
        {
            return Err(RuntimeStateError::new(
                "browser text raster completion does not own the current waiting thread",
            ));
        }
        if expected_bytes.is_empty() || expected_bytes.len() != replacement_bytes.len() {
            return Err(RuntimeStateError::new(
                "text rasterization requires equally sized nonempty before and after bytes",
            ));
        }
        let byte_length = u64::try_from(expected_bytes.len()).map_err(|_| {
            RuntimeStateError::new("text raster byte length does not fit guest range accounting")
        })?;
        let pixel_end = u64::from(byte_offset)
            .checked_add(byte_length)
            .ok_or_else(|| {
                RuntimeStateError::new("text raster byte range overflowed x86 accounting")
            })?;
        if pixel_end > u64::from(target.descriptor.pixel_byte_length) {
            return Err(RuntimeStateError::new(
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
                RuntimeStateError::new("text raster guest address overflowed x86 accounting")
            })?;
        self.memory
            .validate_access(address, expected_bytes.len(), MemoryAccess::Read)
            .map_err(RuntimeStateError::from_memory)?;
        self.memory
            .validate_access(address, replacement_bytes.len(), MemoryAccess::Write)
            .map_err(RuntimeStateError::from_memory)?;
        if self
            .memory
            .read(address, expected_bytes.len())
            .map_err(RuntimeStateError::from_memory)?
            != expected_bytes
        {
            return Err(RuntimeStateError::new(
                "selected DIB bytes changed while browser rasterization was pending",
            ));
        }
        self.gdi.validate_text_raster_completion(&target)?;
        let mut transaction = self.memory.begin_transaction();
        transaction
            .write(address, replacement_bytes)
            .map_err(RuntimeStateError::from_memory)?;
        self.gdi.publish_text_raster_completion(&target);
        transaction.commit_transaction();
        Ok(self.scheduler.commit_host_completion(scheduler))
    }

    pub(crate) fn start_gdiplus_session(
        &mut self,
        token_output: GuestAddress,
    ) -> Result<u32, RuntimeStateError> {
        self.require_gdiplus_operation_state()?;
        self.gdiplus.startup(&mut self.memory, token_output)
    }

    pub(crate) fn shutdown_gdiplus_session(&mut self, token: u32) -> Result<(), RuntimeStateError> {
        self.require_gdiplus_operation_state()?;
        self.gdiplus.shutdown(token)
    }

    pub(crate) fn select_gdi_object(
        &mut self,
        device_context_handle: u32,
        object_handle: u32,
    ) -> Result<Option<u32>, RuntimeStateError> {
        self.require_gdi_operation_state()?;
        let thread_id = self.current_thread_id();
        self.gdi
            .select_object(device_context_handle, object_handle, thread_id)
    }

    pub(crate) fn delete_gdi_object(
        &mut self,
        object_handle: u32,
    ) -> Result<bool, RuntimeStateError> {
        self.require_gdi_operation_state()?;
        let memory = &mut self.memory;
        let address_space = &mut self.address_space_allocator;
        self.gdi
            .delete_object(object_handle, |object| {
                let super::gdi::RuntimeGdiObjectKind::DibSection {
                    descriptor,
                    backing,
                } = &object.object
                else {
                    return Ok(());
                };
                let allocation = address_space.allocation(backing.address_allocation_id)?;
                if allocation.range.base != backing.range.start()
                    || allocation.range.byte_length != backing.range.byte_length()
                    || allocation.usable_byte_length != u64::from(descriptor.pixel_byte_length)
                    || allocation.purpose != ProcessAddressAllocationPurpose::GdiResource
                    || allocation.classification != ProcessAddressAllocationClassification::Graphics
                    || allocation.release_policy != ProcessAddressAllocationReleasePolicy::Explicit
                    || allocation.executable
                {
                    return Err(RuntimeStateError::new(
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
                address_space.release(backing.address_allocation_id)?;
                transaction.commit_transaction();
                Ok(())
            })
            .map(|object| object.is_some())
    }

    pub(crate) fn fill_gdi_rect(
        &mut self,
        device_context_handle: u32,
        rect: super::displays::RuntimeRect,
        brush_handle: u32,
    ) -> Result<bool, RuntimeStateError> {
        self.require_gdi_operation_state()?;
        let Some(device_context) = self
            .gdi
            .device_context_option(device_context_handle)?
            .cloned()
        else {
            return Ok(false);
        };
        if self.gdi.object_option(brush_handle)?.is_none() {
            return Ok(false);
        }
        match device_context.target {
            RuntimeDeviceContextTarget::Window { window_handle, .. } => {
                if self.user32.window_option(window_handle)?.is_none() {
                    return Ok(false);
                }
                let target = self.resolve_gdi_window_target(window_handle)?;
                validate_device_context_target(&device_context, target)?;
                self.gdi.fill_rect(
                    device_context_handle,
                    rect,
                    brush_handle,
                    self.current_thread_id(),
                )?;
                Ok(true)
            }
            RuntimeDeviceContextTarget::GraphicsSurface {
                surface_com_object_id,
                ..
            } => {
                self.require_graphics_operation_state()?;
                let target = self.resolve_gdi_graphics_surface_target(surface_com_object_id)?;
                validate_graphics_device_context_target(&device_context, target)?;
                let color_ref = self.gdi.graphics_surface_fill_color(
                    device_context_handle,
                    rect,
                    brush_handle,
                    self.current_thread_id(),
                )?;
                self.graphics
                    .fill_surface(surface_com_object_id, rect, color_ref)?;
                Ok(true)
            }
            RuntimeDeviceContextTarget::Screen { .. } => Err(RuntimeStateError::new(
                "drawing through a logical screen HDC is outside the reached GDI support band",
            )),
            RuntimeDeviceContextTarget::Memory { .. } => Err(RuntimeStateError::new(
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
        self.gdi.release_window_dc(
            window_handle,
            device_context_handle,
            self.current_thread_id(),
        )
    }

    pub(crate) fn release_screen_dc(
        &mut self,
        device_context_handle: u32,
    ) -> Result<(), RuntimeStateError> {
        self.require_gdi_operation_state()?;
        let target = self.resolve_gdi_screen_target()?;
        self.gdi
            .release_screen_dc(target, device_context_handle, self.current_thread_id())
    }

    #[cfg(test)]
    pub(crate) fn gdi_device_context(
        &self,
        handle: u32,
    ) -> Result<&RuntimeDeviceContext, RuntimeStateError> {
        self.gdi.device_context_by_handle(handle)
    }

    pub(crate) fn gdi_device_context_option(
        &self,
        handle: u32,
    ) -> Result<Option<&RuntimeDeviceContext>, RuntimeStateError> {
        self.gdi.device_context_option(handle)
    }

    #[cfg(test)]
    pub(crate) fn gdi_device_contexts(
        &self,
    ) -> &BTreeMap<RuntimeDeviceContextId, RuntimeDeviceContext> {
        self.gdi.device_contexts()
    }

    #[cfg(test)]
    pub(crate) fn gdi_objects(&self) -> &BTreeMap<RuntimeGdiObjectId, RuntimeGdiObject> {
        self.gdi.objects()
    }

    #[cfg(test)]
    pub(crate) fn gdi_surfaces(&self) -> &BTreeMap<RuntimeGdiSurfaceId, RuntimeGdiSurface> {
        self.gdi.surfaces()
    }

    pub(crate) fn validate_audio_buffer_creation(
        &self,
        device_com_object_id: RuntimeComObjectId,
        descriptor: super::RuntimeAudioBufferDescriptor,
    ) -> Result<
        (
            RuntimeAudioBufferId,
            RuntimeAudioDeviceId,
            super::RuntimeAudioBufferDescriptor,
        ),
        RuntimeStateError,
    > {
        self.require_audio_operation_state()?;
        self.audio
            .validate_buffer_creation(device_com_object_id, descriptor)
    }

    pub(crate) fn insert_audio_buffer(
        &mut self,
        creation: (
            RuntimeAudioBufferId,
            RuntimeAudioDeviceId,
            super::RuntimeAudioBufferDescriptor,
        ),
        com_object_id: RuntimeComObjectId,
    ) {
        self.audio.insert_buffer(creation, com_object_id);
    }

    pub(crate) fn share_audio_buffer_samples(
        &mut self,
        source: RuntimeComObjectId,
        destination: RuntimeComObjectId,
    ) {
        self.audio.share_buffer_samples(source, destination);
    }

    pub(crate) fn set_audio_buffer_format(
        &mut self,
        buffer_com_object_id: RuntimeComObjectId,
        format: super::audio::RuntimeAudioPcmFormat,
    ) -> Result<(), RuntimeStateError> {
        self.require_audio_operation_state()?;
        self.audio.set_buffer_format(buffer_com_object_id, format)
    }

    pub(crate) fn lock_audio_buffer(
        &mut self,
        buffer_com_object_id: RuntimeComObjectId,
        offset: u32,
        byte_length: u32,
        flags: u32,
        output_addresses: [GuestAddress; 4],
    ) -> Result<bool, RuntimeStateError> {
        for address in output_addresses {
            self.memory
                .validate_access(address, 4, MemoryAccess::Write)
                .map_err(RuntimeStateError::from_memory)?;
        }
        self.require_audio_operation_state()?;
        let buffer = self.audio.buffer_by_com_object(buffer_com_object_id)?;
        let buffer_id = buffer.id;
        let existing_backing = buffer.backing;
        let request = if existing_backing.is_none() {
            let arena = self
                .address_space_allocator
                .allocation_for_purpose(&ProcessAddressAllocationPurpose::AudioResourceArena)?;
            if arena.range.base != super::RUNTIME_AUDIO_RESOURCE_ARENA_BASE
                || arena.range.byte_length != super::RUNTIME_AUDIO_RESOURCE_ARENA_SIZE
                || arena.classification != ProcessAddressAllocationClassification::Audio
                || arena.release_policy != ProcessAddressAllocationReleasePolicy::Permanent
                || arena.executable
            {
                return Err(RuntimeStateError::new(
                    "audio resource arena does not match the controlled process layout",
                ));
            }
            Some((
                Some(arena.id),
                ProcessAddressAllocationRequest {
                    purpose: ProcessAddressAllocationPurpose::AudioResource,
                    preferred_address: None,
                    search_range: arena.range,
                    reserve_size: u64::from(buffer.descriptor.byte_length),
                    alignment: GUEST_PAGE_SIZE as u32,
                    guard_pages: 0,
                    classification: ProcessAddressAllocationClassification::Audio,
                    release_policy: ProcessAddressAllocationReleasePolicy::Explicit,
                    executable: false,
                },
            ))
        } else {
            None
        };
        let existing_lock = if let Some(backing) = existing_backing {
            let Some(lock) = self.audio.preview_buffer_lock(
                buffer_com_object_id,
                offset,
                byte_length,
                flags,
                None,
            )?
            else {
                return Ok(false);
            };
            let allocation = self
                .address_space_allocator
                .allocation(backing.address_allocation_id)?;
            if allocation.range.base != backing.range.start()
                || allocation.range.byte_length != backing.range.byte_length()
                || allocation.purpose != ProcessAddressAllocationPurpose::AudioResource
                || allocation.classification != ProcessAddressAllocationClassification::Audio
                || allocation.release_policy != ProcessAddressAllocationReleasePolicy::Explicit
                || allocation.executable
            {
                return Err(RuntimeStateError::new(
                    "live audio backing disagrees with its process allocation",
                ));
            }
            Some(lock)
        } else {
            None
        };
        let audio = &mut self.audio;
        let memory = &mut self.memory;
        let mut lock_backing = |allocation: Option<&ProcessAddressAllocationRecord>| {
            let backing_range = allocation
                .map(|allocation| {
                    GuestRange::new(allocation.range.base, allocation.range.byte_length)
                })
                .transpose()
                .map_err(RuntimeStateError::from_memory)?;
            let lock = if let Some(lock) = existing_lock {
                lock
            } else {
                let Some(lock) = audio.preview_buffer_lock(
                    buffer_com_object_id,
                    offset,
                    byte_length,
                    flags,
                    backing_range,
                )?
                else {
                    return Ok(None);
                };
                lock
            };
            let mut transaction = memory.begin_transaction();
            let backing_installation = if let Some(allocation) = allocation {
                let owner = transaction
                    .create_owner(MemoryOwnerKind::Runtime)
                    .map_err(RuntimeStateError::from_memory)?;
                transaction
                    .reserve_fixed_for_address_allocation(
                        allocation.id.0,
                        owner,
                        MemoryRegionKind::AudioResource,
                        allocation.range.base,
                        allocation.range.byte_length,
                    )
                    .map_err(RuntimeStateError::from_memory)?;
                let range = backing_range.expect("new audio allocation has a range");
                transaction
                    .commit(
                        owner,
                        range,
                        PageProtection {
                            read: true,
                            write: true,
                            execute: false,
                        },
                    )
                    .map_err(RuntimeStateError::from_memory)?;
                Some(RuntimeAudioBufferBacking {
                    address_allocation_id: allocation.id,
                    memory_owner: owner,
                    range,
                })
            } else {
                let backing = existing_backing.expect("existing audio backing was validated");
                transaction
                    .protect(
                        backing.memory_owner,
                        backing.range,
                        PageProtection {
                            read: true,
                            write: true,
                            execute: false,
                        },
                    )
                    .map_err(RuntimeStateError::from_memory)?;
                None
            };
            let output_values = [
                lock.first_segment.guest_address.0,
                lock.first_segment.byte_length,
                lock.second_segment
                    .map_or(0, |segment| segment.guest_address.0),
                lock.second_segment.map_or(0, |segment| segment.byte_length),
            ];
            for (address, value) in output_addresses.into_iter().zip(output_values) {
                transaction
                    .write(address, &value.to_le_bytes())
                    .map_err(RuntimeStateError::from_memory)?;
            }
            audio.publish_buffer_lock(buffer_id, lock, backing_installation);
            transaction.commit_transaction();
            Ok(Some(()))
        };
        if let Some(request) = request {
            let mut range_available = false;
            let result = self.address_space_allocator.try_reserve_batch_with(
                vec![request],
                |allocations| {
                    range_available = true;
                    lock_backing(allocations.first())
                },
            )?;
            if !range_available {
                return Err(RuntimeStateError::new(
                    "process address reservation has no fitting range or allocation ID",
                ));
            }
            Ok(result.is_some())
        } else {
            Ok(lock_backing(None)?.is_some())
        }
    }

    pub(crate) fn unlock_audio_buffer(
        &mut self,
        buffer_com_object_id: RuntimeComObjectId,
        first_segment: super::audio::RuntimeAudioBufferSegment,
        second_segment: Option<super::audio::RuntimeAudioBufferSegment>,
    ) -> Result<bool, RuntimeStateError> {
        self.require_audio_operation_state()?;
        let Some((buffer_id, lock_index, releases_last_lock)) =
            self.audio
                .select_buffer_unlock(buffer_com_object_id, first_segment, second_segment)?
        else {
            return Ok(false);
        };
        if releases_last_lock {
            let backing = self
                .audio
                .buffer(buffer_id)?
                .backing
                .expect("selected lock has backing");
            let mut transaction = self.memory.begin_transaction();
            transaction
                .protect(
                    backing.memory_owner,
                    backing.range,
                    PageProtection {
                        read: true,
                        write: false,
                        execute: false,
                    },
                )
                .map_err(RuntimeStateError::from_memory)?;
            self.audio.finish_buffer_unlock(buffer_id, lock_index);
            transaction.commit_transaction();
        } else {
            self.audio.finish_buffer_unlock(buffer_id, lock_index);
        }
        Ok(true)
    }

    pub(crate) fn set_audio_buffer_notifications(
        &mut self,
        buffer_com_object_id: RuntimeComObjectId,
        notifications: Vec<RuntimeAudioNotification>,
    ) -> Result<bool, RuntimeStateError> {
        self.require_audio_operation_state()?;
        for notification in &notifications {
            let Some(event) = self.live_event_for_handle(notification.event_handle)? else {
                return Ok(false);
            };
            if event.manual_reset {
                return Ok(false);
            }
        }
        self.audio
            .set_buffer_notifications(buffer_com_object_id, notifications)
    }

    pub(crate) fn begin_audio_notification_wait(
        &mut self,
        handles: &[RuntimeHandle],
        timeout_milliseconds: u32,
    ) -> Result<Option<(RuntimeHostRequest, Vec<u8>)>, RuntimeStateError> {
        self.require_audio_operation_state()?;
        if timeout_milliseconds == 0 || timeout_milliseconds > 100 {
            return Err(RuntimeStateError::new(
                "audio notification pacing supports finite waits from 1 through 100 milliseconds",
            ));
        }
        self.validate_audio_notification_wait_events(handles)?;
        self.process
            .host_elapsed_time_100ns()
            .checked_add(u64::from(timeout_milliseconds) * 10_000)
            .ok_or_else(|| RuntimeStateError::new("audio wait deadline overflowed"))?;
        if !handles.is_empty()
            && handles.len() <= RUNTIME_AUDIO_NOTIFICATION_LIMIT
            && !self.audio.matches_notification_wait_handles(handles)
        {
            return Ok(None);
        }
        let audio = self.audio.plan_notification_wait(
            handles,
            timeout_milliseconds,
            self.process.host_elapsed_time_100ns(),
        )?;
        let scheduler = self
            .scheduler
            .checked_host_request(RUNTIME_AUDIO_NOTIFICATION_PACING_SERVICE.to_owned())?;
        let payload = audio.as_ref().map_or_else(
            || {
                encode_inactive_audio_notification_wait_payload(
                    handles,
                    timeout_milliseconds,
                    self.process.host_elapsed_time_100ns(),
                )
            },
            |audio| audio.encode_host_payload(handles),
        );
        let request = self.scheduler.publish_host_request(scheduler);
        Ok(Some((request, payload)))
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
        let scheduler = self.scheduler.validate_host_completion(request_id)?;
        if scheduler.service != RUNTIME_AUDIO_NOTIFICATION_PACING_SERVICE {
            return Err(RuntimeStateError::new(
                "audio notification completion does not match its pacing service",
            ));
        }
        let delay = u32::from_le_bytes(payload[4..8].try_into().unwrap());
        if !(1..=timeout_milliseconds).contains(&delay) {
            return Err(RuntimeStateError::new(
                "audio wait delay exceeds its captured timeout",
            ));
        }
        let started_at = u64::from_le_bytes(payload[payload.len() - 8..].try_into().unwrap());
        let now = self.process.host_elapsed_time_100ns();
        let deadline = started_at
            .checked_add(u64::from(timeout_milliseconds) * 10_000)
            .ok_or_else(|| RuntimeStateError::new("audio wait deadline overflowed"))?;
        if now < started_at {
            return Err(RuntimeStateError::new("audio wait host clock regressed"));
        }
        if payload[..4] == 1u32.to_le_bytes() {
            match self
                .audio
                .plan_notification_wait(&handles, timeout_milliseconds, now)?
            {
                Some(audio) if audio.matches_host_payload(&handles, payload) => (),
                Some(_) => {
                    return Err(RuntimeStateError::new(
                        "audio wait differs from its captured buffer identity",
                    ));
                }
                None => self.audio.validate_stopped_notification_wait(
                    &handles,
                    timeout_milliseconds,
                    payload,
                )?,
            }
        }
        let audio = self.audio.progress_for_handles(&handles, now)?;
        let eligible_crossed = self
            .audio
            .notifications_before(&handles, now.min(deadline))?;
        let mut plan = RuntimeAudioNotificationWaitCompletionPlan {
            scheduler,
            audio,
            event_updates: Vec::new(),
            result_index: None,
            retry: None,
        };
        // A notification is an event, not a property of being stopped. In particular,
        // Stop may occur before a wait begins or while an inactive wait is pending.
        // Consume the same Rust-owned auto-reset signal as WaitForMultipleObjects.
        self.validate_audio_notification_wait_events(&handles)?;
        let crossed = plan
            .audio
            .as_ref()
            .map_or(&[][..], |audio| audio.notifications.as_slice());
        let mut updates = Vec::new();
        for &handle in crossed {
            let record = self
                .objects
                .object_for_handle(handle, RuntimeObjectKind::Event)?;
            updates.push((
                record.key.clone(),
                RuntimeWaitState::Signaled { exit_code: 0 },
            ));
        }
        for (index, &handle) in handles.iter().enumerate() {
            let event = self.event_for_handle(handle)?;
            if eligible_crossed.contains(&handle)
                || matches!(event.wait_state, RuntimeWaitState::Signaled { .. })
            {
                let record = self
                    .objects
                    .object_for_handle(handle, RuntimeObjectKind::Event)?;
                updates.retain(|(key, _)| key != &record.key);
                updates.push((record.key.clone(), RuntimeWaitState::Nonsignaled));
                plan.result_index = Some(index as u32);
                break;
            }
        }
        plan.event_updates = self.objects.plan_wait_state_updates(&updates)?;
        if plan.result_index.is_none() && now < deadline {
            let remaining = (deadline - now).div_ceil(10_000) as u32;
            let mut retry = self
                .audio
                .plan_notification_wait(&handles, timeout_milliseconds, now)?
                .map_or_else(
                    || {
                        encode_inactive_audio_notification_wait_payload(
                            &handles,
                            timeout_milliseconds,
                            started_at,
                        )
                    },
                    |audio| audio.encode_host_payload(&handles),
                );
            let next_delay = u32::from_le_bytes(retry[4..8].try_into().unwrap())
                .min(remaining)
                .max(1);
            retry[4..8].copy_from_slice(&next_delay.to_le_bytes());
            let length = retry.len();
            retry[length - 8..].copy_from_slice(&started_at.to_le_bytes());
            plan.retry = Some(retry);
        }
        Ok(plan)
    }

    pub(crate) fn commit_audio_notification_wait_completion(
        &mut self,
        plan: RuntimeAudioNotificationWaitCompletionPlan,
    ) {
        if let Some(audio) = plan.audio {
            self.audio.commit_progress(audio);
        }
        self.objects.commit_wait_state_updates(plan.event_updates);
        if plan.retry.is_none() {
            self.scheduler.commit_host_completion(plan.scheduler);
        }
    }

    pub(crate) fn refresh_audio_notifications_for_handles(
        &mut self,
        handles: &[RuntimeHandle],
    ) -> Result<(), RuntimeStateError> {
        let Some(progress) = self
            .audio
            .progress_for_handles(handles, self.process.host_elapsed_time_100ns())?
        else {
            return Ok(());
        };
        self.commit_audio_progress(progress)
    }

    fn commit_audio_progress(&mut self, progress: AudioProgress) -> Result<(), RuntimeStateError> {
        let mut updates = Vec::new();
        for &handle in &progress.notifications {
            let event = self.event_for_handle(handle)?;
            if event.manual_reset {
                return Err(RuntimeStateError::new(
                    "audio notification requires an auto-reset event",
                ));
            }
            let record = self
                .objects
                .object_for_handle(handle, RuntimeObjectKind::Event)?;
            updates.push((
                record.key.clone(),
                RuntimeWaitState::Signaled { exit_code: 0 },
            ));
        }
        let updates = self.objects.plan_wait_state_updates(&updates)?;
        self.audio.commit_progress(progress);
        self.objects.commit_wait_state_updates(updates);
        Ok(())
    }

    pub(crate) fn audio_buffer_position(
        &self,
        object_id: RuntimeComObjectId,
    ) -> Result<u32, RuntimeStateError> {
        self.audio
            .position_bytes(object_id, self.process.host_elapsed_time_100ns())
    }

    fn validate_audio_notification_wait_events(
        &self,
        handles: &[RuntimeHandle],
    ) -> Result<(), RuntimeStateError> {
        for &handle in handles {
            let event = self.event_for_handle(handle)?;
            if event.manual_reset {
                return Err(RuntimeStateError::new(
                    "reached DirectSound wait requires auto-reset event handles",
                ));
            }
        }
        Ok(())
    }

    pub(crate) fn validate_audio_backing(
        &self,
        object_id: RuntimeComObjectId,
    ) -> Result<(), RuntimeStateError> {
        let buffer = self.audio.buffer_by_com_object(object_id)?;
        if let Some(backing) = buffer.backing {
            let allocation = self
                .address_space_allocator
                .allocation(backing.address_allocation_id)?;
            let mut regions = self.memory.regions().filter(|region| {
                region.address_allocation_id == Some(backing.address_allocation_id.0)
            });
            let region = regions.next().ok_or_else(|| {
                RuntimeStateError::new("audio playback backing has no authoritative guest region")
            })?;
            if regions.next().is_some()
                || region.owner != backing.memory_owner
                || region.range != backing.range
                || region.kind != MemoryRegionKind::AudioResource
                || allocation.range.base != backing.range.start()
                || allocation.range.byte_length != backing.range.byte_length()
                || allocation.purpose != ProcessAddressAllocationPurpose::AudioResource
                || allocation.classification != ProcessAddressAllocationClassification::Audio
                || allocation.executable
            {
                return Err(RuntimeStateError::new(
                    "audio playback backing disagrees with its live memory owner and process allocation",
                ));
            }
            self.memory
                .validate_access(
                    backing.range.start(),
                    buffer.descriptor.byte_length as usize,
                    MemoryAccess::Read,
                )
                .map_err(RuntimeStateError::from_memory)?;
        }
        Ok(())
    }

    fn browser_audio_command(
        &self,
        object_id: RuntimeComObjectId,
        operation: u32,
        resource_id: u64,
        controls: super::audio::RuntimeAudioBufferControls,
        playback: RuntimeAudioPlaybackState,
        include_pcm: bool,
    ) -> Result<Vec<u8>, RuntimeStateError> {
        self.validate_audio_backing(object_id)?;
        let buffer = self.audio.buffer_by_com_object(object_id)?;
        let format = buffer
            .active_format
            .ok_or_else(|| RuntimeStateError::new("browser audio buffer has no PCM format"))?;
        let pcm = if include_pcm {
            let backing = buffer.backing.ok_or_else(|| {
                RuntimeStateError::new("browser audio buffer has no owned PCM backing")
            })?;
            self.memory
                .read(
                    backing.range.start(),
                    buffer.descriptor.byte_length as usize,
                )
                .map_err(RuntimeStateError::from_memory)?
        } else {
            Vec::new()
        };
        let words = [
            operation,
            resource_id as u32,
            (resource_id >> 32) as u32,
            u32::from(format.channels),
            format.samples_per_second,
            u32::from(format.bits_per_sample),
            controls.volume_hundredths_db as u32,
            controls.pan_hundredths_db as u32,
            controls.frequency_hz.unwrap_or(format.samples_per_second),
            if operation == 4 {
                controls.current_position_bytes
            } else {
                self.audio_buffer_position(object_id)?
            },
            u32::from(playback == RuntimeAudioPlaybackState::PlayingLooping),
            pcm.len() as u32,
        ];
        let mut command = Vec::with_capacity(48 + pcm.len());
        for word in words {
            command.extend_from_slice(&word.to_le_bytes());
        }
        command.extend_from_slice(&pcm);
        Ok(command)
    }

    pub(crate) fn prepare_browser_audio_play(
        &mut self,
        object_id: RuntimeComObjectId,
        playback: RuntimeAudioPlaybackState,
    ) -> Result<BrowserAudioPreparation, RuntimeStateError> {
        self.require_audio_operation_state()?;
        let Some(stop_event) = self.audio.validate_buffer_play(object_id, playback)? else {
            return Ok(BrowserAudioPreparation::Complete(false));
        };
        if let Some(handle) = stop_event
            && self.live_event_for_handle(handle)?.is_none()
        {
            return Err(RuntimeStateError::new(
                "audio stop notification event is not live",
            ));
        }
        let buffer = self.audio.buffer_by_com_object(object_id)?;
        if buffer.descriptor.kind == super::audio::RuntimeAudioBufferKind::Primary
            || (buffer.playback == playback && buffer.browser_resource_id.is_some())
            || (playback == RuntimeAudioPlaybackState::Stopped
                && buffer.browser_resource_id.is_none())
        {
            return self
                .set_audio_buffer_play(object_id, playback)
                .map(BrowserAudioPreparation::Complete);
        }
        let state = BrowserAudioBufferState::capture(buffer);
        let old_resource_id = buffer.browser_resource_id;
        let controls = buffer.controls;
        let operation = if playback == RuntimeAudioPlaybackState::Stopped {
            5
        } else {
            1
        };
        let mut command = self.browser_audio_command(
            object_id,
            operation,
            old_resource_id.unwrap_or(0),
            controls,
            playback,
            operation == 1,
        )?;
        let request_id = self.audio.next_browser_operation_id()?;
        let resource_id = if operation == 1 {
            Some(old_resource_id.unwrap_or(request_id))
        } else {
            None
        };
        if let Some(id) = resource_id {
            command[4..8].copy_from_slice(&(id as u32).to_le_bytes());
            command[8..12].copy_from_slice(&((id >> 32) as u32).to_le_bytes());
        }
        Ok(BrowserAudioPreparation::Pending(PreparedBrowserAudio {
            request_id,
            thread_id: self.current_thread_id(),
            command,
            action: BrowserAudioAction::Playback {
                object_id,
                playback,
            },
            buffers: vec![state],
            resource_id,
        }))
    }

    pub(crate) fn prepare_browser_audio_unlock(
        &mut self,
        object_id: RuntimeComObjectId,
        first: super::audio::RuntimeAudioBufferSegment,
        second: Option<super::audio::RuntimeAudioBufferSegment>,
    ) -> Result<BrowserAudioPreparation, RuntimeStateError> {
        self.require_audio_operation_state()?;
        let Some((_, _, last)) = self.audio.select_buffer_unlock(object_id, first, second)? else {
            return Ok(BrowserAudioPreparation::Complete(false));
        };
        let buffer = self.audio.buffer_by_com_object(object_id)?;
        let Some(resource_id) = buffer.browser_resource_id.filter(|_| last) else {
            return self
                .unlock_audio_buffer(object_id, first, second)
                .map(BrowserAudioPreparation::Complete);
        };
        let state = BrowserAudioBufferState::capture(buffer);
        let command = self.browser_audio_command(
            object_id,
            2,
            resource_id,
            buffer.controls,
            buffer.playback,
            true,
        )?;
        let request_id = self.audio.next_browser_operation_id()?;
        Ok(BrowserAudioPreparation::Pending(PreparedBrowserAudio {
            request_id,
            thread_id: self.current_thread_id(),
            command,
            action: BrowserAudioAction::Unlock {
                object_id,
                first,
                second,
            },
            buffers: vec![state],
            resource_id: Some(resource_id),
        }))
    }

    pub(crate) fn prepare_browser_audio_control(
        &mut self,
        object_id: RuntimeComObjectId,
        update: super::audio::RuntimeAudioBufferControlUpdate,
    ) -> Result<BrowserAudioPreparation, RuntimeStateError> {
        self.require_audio_operation_state()?;
        let Some(next) = self.audio.preview_buffer_control(object_id, update)? else {
            return Ok(BrowserAudioPreparation::Complete(false));
        };
        let buffer = self.audio.buffer_by_com_object(object_id)?;
        let Some(resource_id) = buffer.browser_resource_id.filter(|_| {
            next != buffer.controls
                || matches!(
                    update,
                    super::audio::RuntimeAudioBufferControlUpdate::CurrentPosition(_)
                )
        }) else {
            return self
                .set_audio_buffer_control(object_id, update)
                .map(BrowserAudioPreparation::Complete);
        };
        let state = BrowserAudioBufferState::capture(buffer);
        let operation = if matches!(
            update,
            super::audio::RuntimeAudioBufferControlUpdate::CurrentPosition(_)
        ) {
            4
        } else {
            3
        };
        let mut command = self.browser_audio_command(
            object_id,
            operation,
            resource_id,
            next,
            buffer.playback,
            false,
        )?;
        let request_id = self.audio.next_browser_operation_id()?;
        let next_resource_id = if matches!(
            update,
            super::audio::RuntimeAudioBufferControlUpdate::CurrentPosition(_)
        ) {
            command[4..12].copy_from_slice(&request_id.to_le_bytes());
            command[44..48].copy_from_slice(&8_u32.to_le_bytes());
            command.extend_from_slice(&resource_id.to_le_bytes());
            request_id
        } else {
            resource_id
        };
        Ok(BrowserAudioPreparation::Pending(PreparedBrowserAudio {
            request_id,
            thread_id: self.current_thread_id(),
            command,
            action: BrowserAudioAction::Control { object_id, update },
            buffers: vec![state],
            resource_id: Some(next_resource_id),
        }))
    }

    pub(crate) fn prepare_browser_audio_release(
        &mut self,
        interface_id: RuntimeComInterfaceId,
    ) -> Result<Option<PreparedBrowserAudio>, RuntimeStateError> {
        let release = self.com.validate_release(interface_id)?;
        if release.reference_count() != 0 {
            return Ok(None);
        }
        let objects = release
            .retired_objects()
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let buffers = self.audio.browser_buffers_for_objects(&objects);
        if buffers.is_empty() {
            return Ok(None);
        }
        self.require_audio_operation_state()?;
        self.validate_com_release(interface_id)?;
        let states = buffers
            .iter()
            .map(|buffer| BrowserAudioBufferState::capture(buffer))
            .collect();
        let mut command = vec![0; 48];
        command[..4].copy_from_slice(&6u32.to_le_bytes());
        command[44..48].copy_from_slice(&((buffers.len() * 8) as u32).to_le_bytes());
        for buffer in buffers {
            command.extend_from_slice(
                &buffer
                    .browser_resource_id
                    .expect("selected browser audio resource")
                    .to_le_bytes(),
            );
        }
        let request_id = self.audio.next_browser_operation_id()?;
        Ok(Some(PreparedBrowserAudio {
            request_id,
            thread_id: self.current_thread_id(),
            command,
            action: BrowserAudioAction::Release { interface_id },
            buffers: states,
            resource_id: None,
        }))
    }

    pub(crate) fn complete_browser_audio(
        &mut self,
        pending: &PreparedBrowserAudio,
        accepted: bool,
    ) -> Result<u32, RuntimeStateError> {
        self.require_audio_operation_state()?;
        if self.current_thread_id() != pending.thread_id {
            return Err(RuntimeStateError::new(
                "browser audio completion belongs to another guest thread",
            ));
        }
        for state in &pending.buffers {
            self.validate_audio_backing(state.object_id)?;
            if !state.matches(self.audio.buffer_by_com_object(state.object_id)?) {
                return Err(RuntimeStateError::new(
                    "browser audio completion no longer owns its live buffer state",
                ));
            }
        }
        if matches!(
            pending.action,
            BrowserAudioAction::Playback {
                playback: RuntimeAudioPlaybackState::PlayingOnce
                    | RuntimeAudioPlaybackState::PlayingLooping,
                ..
            } | BrowserAudioAction::Unlock { .. }
        ) {
            let state = pending
                .buffers
                .first()
                .expect("PCM operation has a selected buffer");
            let buffer = self.audio.buffer_by_com_object(state.object_id)?;
            let backing = buffer.backing.expect("validated PCM backing");
            let bytes = self
                .memory
                .read(
                    backing.range.start(),
                    buffer.descriptor.byte_length as usize,
                )
                .map_err(RuntimeStateError::from_memory)?;
            if pending.command.get(48..) != Some(bytes.as_slice()) {
                return Err(RuntimeStateError::new(
                    "authoritative PCM bytes changed during browser handoff",
                ));
            }
        }
        if !accepted {
            if matches!(pending.action, BrowserAudioAction::Release { .. }) {
                return Err(RuntimeStateError::new(
                    "browser could not release its audio resources",
                ));
            }
            return Ok(0x8000_4005);
        }
        let result = match pending.action {
            BrowserAudioAction::Playback {
                object_id,
                playback,
            } => {
                let result = self.set_audio_buffer_play(object_id, playback)?;
                if result {
                    self.audio
                        .set_browser_resource(object_id, pending.resource_id);
                }
                result
            }
            BrowserAudioAction::Unlock {
                object_id,
                first,
                second,
            } => self.unlock_audio_buffer(object_id, first, second)?,
            BrowserAudioAction::Control { object_id, update } => {
                let success = self.set_audio_buffer_control(object_id, update)?;
                if success {
                    self.audio
                        .set_browser_resource(object_id, pending.resource_id);
                }
                success
            }
            BrowserAudioAction::Release { interface_id } => {
                return self.release_com_interface(interface_id);
            }
        };
        if !result {
            return Err(RuntimeStateError::new(
                "validated browser audio operation became invalid",
            ));
        }
        Ok(0)
    }

    pub fn has_browser_audio_end_pending(&self) -> bool {
        self.process.is_running() && self.audio.has_browser_end_pending()
    }

    pub fn observe_browser_audio_end(
        &mut self,
        resource_id: u64,
    ) -> Result<bool, RuntimeStateError> {
        if !self.process.is_running() {
            return Ok(false);
        }
        let Some(buffer) = self.audio.buffer_for_browser_resource(resource_id) else {
            return Ok(false);
        };
        if buffer.playback != RuntimeAudioPlaybackState::PlayingOnce {
            return Ok(false);
        }
        let object_id = buffer.com_object_id;
        self.validate_audio_backing(object_id)?;
        let stop_event = self
            .audio
            .validate_buffer_play(object_id, RuntimeAudioPlaybackState::Stopped)?
            .ok_or_else(|| RuntimeStateError::new("live playing audio cannot stop"))?;
        if let Some(handle) = stop_event
            && self.live_event_for_handle(handle)?.is_none()
        {
            return Err(RuntimeStateError::new(
                "audio end notification event is not live",
            ));
        }
        let progress = self
            .audio
            .progress(object_id, self.process.host_elapsed_time_100ns())?;
        self.commit_audio_progress(progress)?;
        if let Some(handle) = stop_event
            && !self.signal_event(handle)?
        {
            return Err(RuntimeStateError::new(
                "audio end notification event is not live",
            ));
        }
        self.audio.finish_browser_end(object_id);
        self.audio.set_browser_resource(object_id, None);
        Ok(true)
    }

    pub(crate) fn set_audio_buffer_play(
        &mut self,
        buffer_com_object_id: RuntimeComObjectId,
        playback: RuntimeAudioPlaybackState,
    ) -> Result<bool, RuntimeStateError> {
        self.require_audio_operation_state()?;
        let Some(stop_event) = self
            .audio
            .validate_buffer_play(buffer_com_object_id, playback)?
        else {
            return Ok(false);
        };
        if playback != RuntimeAudioPlaybackState::Stopped {
            self.validate_audio_backing(buffer_com_object_id)?;
        }
        if let Some(handle) = stop_event
            && self.live_event_for_handle(handle)?.is_none()
        {
            return Err(RuntimeStateError::new(
                "audio stop notification event is not live",
            ));
        }
        let progress = self
            .audio
            .progress(buffer_com_object_id, self.process.host_elapsed_time_100ns())?;
        self.commit_audio_progress(progress)?;
        if let Some(handle) = stop_event {
            // An invalid/retired event cannot partially change audio playback.
            if !self.set_event(handle)? {
                return Err(RuntimeStateError::new(
                    "audio stop notification event is not live",
                ));
            }
        }
        self.audio
            .finish_buffer_play(buffer_com_object_id, playback);
        if playback == RuntimeAudioPlaybackState::Stopped {
            self.audio.set_browser_resource(buffer_com_object_id, None);
        }
        Ok(true)
    }

    pub(crate) fn set_audio_buffer_control(
        &mut self,
        buffer_com_object_id: RuntimeComObjectId,
        update: super::audio::RuntimeAudioBufferControlUpdate,
    ) -> Result<bool, RuntimeStateError> {
        self.require_audio_operation_state()?;
        if self
            .audio
            .preview_buffer_control(buffer_com_object_id, update)?
            .is_none()
        {
            return Ok(false);
        }
        let progress = self
            .audio
            .progress(buffer_com_object_id, self.process.host_elapsed_time_100ns())?;
        self.commit_audio_progress(progress)?;
        self.audio.set_buffer_control(buffer_com_object_id, update)
    }

    pub(crate) fn set_audio_priority_cooperative_level(
        &mut self,
        device_com_object_id: RuntimeComObjectId,
        window_handle: u32,
    ) -> Result<bool, RuntimeStateError> {
        self.require_audio_operation_state()?;
        let Some(target) = self.resolve_audio_cooperative_state(window_handle)? else {
            return Ok(false);
        };
        self.audio
            .set_priority_cooperative_level(device_com_object_id, target)
            .map(|()| true)
    }

    #[cfg(test)]
    pub(crate) fn audio_device(
        &self,
        id: RuntimeAudioDeviceId,
    ) -> Result<&RuntimeAudioDevice, RuntimeStateError> {
        self.audio.device(id)
    }

    #[cfg(test)]
    pub(crate) fn audio_devices(&self) -> &BTreeMap<RuntimeAudioDeviceId, RuntimeAudioDevice> {
        self.audio.devices()
    }

    pub(crate) fn audio_buffer_for_com_object(
        &self,
        com_object_id: RuntimeComObjectId,
    ) -> Result<&RuntimeAudioBuffer, RuntimeStateError> {
        self.audio.buffer_by_com_object(com_object_id)
    }

    #[cfg(test)]
    pub(crate) fn audio_buffers(&self) -> &BTreeMap<RuntimeAudioBufferId, RuntimeAudioBuffer> {
        self.audio.buffers()
    }

    pub(crate) fn validate_graphics_device_creation(
        &self,
        window_handle: u32,
        back_buffer_width: u32,
        back_buffer_height: u32,
        present_mode: super::RuntimeGraphicsPresentMode,
    ) -> Result<Option<RuntimeGraphicsDeviceCreationRequest>, RuntimeStateError> {
        self.require_graphics_operation_state()?;
        let Some(target) = self.resolve_graphics_window_target(window_handle)? else {
            return Ok(None);
        };
        self.graphics
            .validate_device_creation(RuntimeGraphicsDeviceCreationRequest {
                target,
                back_buffer_width: if back_buffer_width == 0 {
                    target.client_width
                } else {
                    back_buffer_width
                },
                back_buffer_height: if back_buffer_height == 0 {
                    target.client_height
                } else {
                    back_buffer_height
                },
                color_format: super::RuntimeGraphicsColorFormat::B8g8r8x8Unorm,
                back_buffer_count: 1,
                sample_count: 1,
                present_mode,
                windowed: true,
            })
    }

    pub(crate) fn insert_graphics_device(
        &mut self,
        request: RuntimeGraphicsDeviceCreationRequest,
        com_object_id: RuntimeComObjectId,
    ) {
        self.graphics.insert_device(request, com_object_id);
    }

    pub(crate) fn configure_d3d8(
        &mut self,
        object: RuntimeComObjectId,
        state: crate::d3d8::DeviceState,
    ) {
        self.graphics.configure_d3d8(object, state);
    }
    pub(crate) fn copy_d3d8_surface(
        &mut self,
        device: RuntimeComObjectId,
        source: RuntimeComObjectId,
        rectangles: &[(RuntimeRect, i32, i32)],
    ) -> Result<bool, RuntimeStateError> {
        self.require_graphics_operation_state()?;
        self.graphics
            .copy_d3d8_surface(device, source, rectangles, &self.memory)
    }
    pub(crate) fn d3d8_state_mut(
        &mut self,
        object: RuntimeComObjectId,
    ) -> Result<&mut crate::d3d8::DeviceState, RuntimeStateError> {
        self.require_graphics_operation_state()?;
        self.graphics.d3d8_state_mut(object)
    }
    pub(crate) fn clear_d3d8(
        &mut self,
        object: RuntimeComObjectId,
        flags: u32,
        color: u32,
        depth: u32,
    ) -> Result<bool, RuntimeStateError> {
        self.require_graphics_operation_state()?;
        self.graphics.clear_d3d8(object, flags, color, depth)
    }

    pub(crate) fn available_graphics_texture_memory(
        &self,
        device_com_object_id: RuntimeComObjectId,
    ) -> Result<u32, RuntimeStateError> {
        self.require_graphics_operation_state()?;
        self.graphics.available_texture_memory(device_com_object_id)
    }

    pub fn presented_graphics_frame(
        &self,
        id: RuntimeGraphicsDeviceId,
    ) -> Result<(u32, u32, &super::graphics::RuntimeGraphicsBackBuffer), RuntimeStateError> {
        let device = self.graphics.device(id)?;
        let frame = device.presented_frame.as_ref().ok_or_else(|| {
            RuntimeStateError::new("presented graphics device has no captured frame")
        })?;
        let (width, height) = device.presented_extent.ok_or_else(|| {
            RuntimeStateError::new("presented graphics frame has no captured extent")
        })?;
        Ok((width, height, frame))
    }

    pub(crate) fn graphics_device_by_com_object(
        &self,
        com_object_id: RuntimeComObjectId,
    ) -> Result<&RuntimeGraphicsDevice, RuntimeStateError> {
        self.require_graphics_operation_state()?;
        self.graphics.device_by_com_object(com_object_id)
    }

    pub(crate) fn update_graphics_render_state(
        &mut self,
        device_com_object_id: RuntimeComObjectId,
        update: RuntimeGraphicsRenderStateUpdate,
    ) -> Result<(), RuntimeStateError> {
        self.require_graphics_operation_state()?;
        self.graphics
            .update_render_state(device_com_object_id, update)
    }

    pub(crate) fn update_graphics_sampler_state(
        &mut self,
        device_com_object_id: RuntimeComObjectId,
        update: RuntimeGraphicsSamplerStateUpdate,
    ) -> Result<(), RuntimeStateError> {
        self.require_graphics_operation_state()?;
        self.graphics
            .update_sampler_state(device_com_object_id, update)
    }

    pub(crate) fn update_graphics_texture_stage_state(
        &mut self,
        device_com_object_id: RuntimeComObjectId,
        update: RuntimeGraphicsTextureStageStateUpdate,
    ) -> Result<(), RuntimeStateError> {
        self.require_graphics_operation_state()?;
        self.graphics
            .update_texture_stage_state(device_com_object_id, update)
    }

    pub(crate) fn bind_graphics_texture(
        &mut self,
        device_com_object_id: RuntimeComObjectId,
        stage: u32,
        texture_com_object_id: Option<RuntimeComObjectId>,
    ) -> Result<bool, RuntimeStateError> {
        self.require_graphics_operation_state()?;
        self.graphics
            .bind_texture(device_com_object_id, stage, texture_com_object_id)
    }

    pub(crate) fn update_graphics_flexible_vertex_format(
        &mut self,
        device_com_object_id: RuntimeComObjectId,
        format: RuntimeGraphicsFlexibleVertexFormat,
    ) -> Result<(), RuntimeStateError> {
        self.require_graphics_operation_state()?;
        self.graphics
            .update_flexible_vertex_format(device_com_object_id, format)
    }

    pub(crate) fn bind_graphics_stream_source(
        &mut self,
        device_com_object_id: RuntimeComObjectId,
        stream: u32,
        vertex_buffer_com_object_id: RuntimeComObjectId,
        offset_in_bytes: u32,
        stride: u32,
    ) -> Result<bool, RuntimeStateError> {
        self.require_graphics_operation_state()?;
        self.graphics.bind_stream_source(
            device_com_object_id,
            stream,
            vertex_buffer_com_object_id,
            offset_in_bytes,
            stride,
        )
    }

    pub(crate) fn set_graphics_viewport(
        &mut self,
        object: RuntimeComObjectId,
        viewport: [u32; 6],
    ) -> Result<(), RuntimeStateError> {
        self.require_graphics_operation_state()?;
        self.graphics.set_viewport(object, viewport)
    }

    pub(crate) fn submit_graphics_draw(
        &mut self,
        device_com_object_id: RuntimeComObjectId,
        start_vertex: u32,
        primitive_count: u32,
    ) -> Result<bool, RuntimeStateError> {
        self.require_graphics_operation_state()?;
        self.graphics.submit_draw(
            device_com_object_id,
            start_vertex,
            primitive_count,
            &self.memory,
        )
    }

    pub(crate) fn submit_graphics_draw_up(
        &mut self,
        device_com_object_id: RuntimeComObjectId,
        primitive_count: u32,
        vertices: GuestAddress,
        stride: u32,
    ) -> Result<bool, RuntimeStateError> {
        self.require_graphics_operation_state()?;
        self.graphics.submit_draw_up(
            device_com_object_id,
            primitive_count,
            vertices,
            stride,
            &self.memory,
        )
    }

    pub(crate) fn clear_graphics_back_buffer(
        &mut self,
        device_com_object_id: RuntimeComObjectId,
        color_argb: u32,
    ) -> Result<(), RuntimeStateError> {
        self.require_graphics_operation_state()?;
        self.graphics
            .clear_back_buffer(device_com_object_id, color_argb)
    }

    pub(crate) fn begin_graphics_scene(
        &mut self,
        device_com_object_id: RuntimeComObjectId,
    ) -> Result<bool, RuntimeStateError> {
        self.require_graphics_operation_state()?;
        self.graphics.begin_scene(device_com_object_id)
    }

    pub(crate) fn end_graphics_scene(
        &mut self,
        device_com_object_id: RuntimeComObjectId,
    ) -> Result<bool, RuntimeStateError> {
        self.require_graphics_operation_state()?;
        self.graphics.end_scene(device_com_object_id)
    }

    pub(crate) fn present_graphics(
        &mut self,
        device_com_object_id: RuntimeComObjectId,
        source: Option<RuntimeRect>,
        destination: Option<RuntimeRect>,
        window_override: u32,
    ) -> Result<bool, RuntimeStateError> {
        self.require_graphics_operation_state()?;
        let device = self.graphics.device_by_com_object(device_com_object_id)?;
        let target = device.target;
        let Some(live) = self.resolve_graphics_window_target(target.window_handle)? else {
            return Ok(false);
        };
        if live.window_id != target.window_id || live.owner_thread_id != target.owner_thread_id {
            return Ok(false);
        }
        if window_override != 0 && window_override != target.window_handle {
            return Err(RuntimeStateError::new(
                "presentation to another window is not implemented",
            ));
        }
        if (source.is_some() || destination.is_some())
            && device.presentation.present_mode != super::RuntimeGraphicsPresentMode::Copy
        {
            return Ok(false);
        }
        let source = source.unwrap_or(RuntimeRect {
            left: 0,
            top: 0,
            right: device.presentation.width as i32,
            bottom: device.presentation.height as i32,
        });
        let destination = destination.unwrap_or(RuntimeRect {
            left: 0,
            top: 0,
            right: live.client_width as i32,
            bottom: live.client_height as i32,
        });
        for rect in [source, destination] {
            if rect.right <= rect.left || rect.bottom <= rect.top {
                return Ok(false);
            }
        }
        // The browser surface represents the complete client area. Support an
        // unscaled upper-left crop while keeping unsupported stretch/offset explicit.
        if source != destination
            || source.left != 0
            || source.top != 0
            || source.right as u32 != live.client_width
            || source.bottom as u32 != live.client_height
            || source.right as u32 > device.presentation.width
            || source.bottom as u32 > device.presentation.height
        {
            return Err(RuntimeStateError::new(
                "scaled or offset presentation is not implemented",
            ));
        }
        self.graphics.present(
            device_com_object_id,
            Some((live.client_width, live.client_height)),
        )
    }

    pub(crate) fn validate_graphics_texture_creation(
        &self,
        device_com_object_id: RuntimeComObjectId,
        descriptor: RuntimeGraphicsTextureDescriptor,
    ) -> Result<
        (
            RuntimeGraphicsTextureId,
            RuntimeGraphicsDeviceId,
            RuntimeGraphicsTextureDescriptor,
        ),
        RuntimeStateError,
    > {
        self.require_graphics_operation_state()?;
        self.graphics
            .validate_texture_creation(device_com_object_id, descriptor)
    }

    pub(crate) fn insert_graphics_texture(
        &mut self,
        creation: (
            RuntimeGraphicsTextureId,
            RuntimeGraphicsDeviceId,
            RuntimeGraphicsTextureDescriptor,
        ),
        com_object_id: RuntimeComObjectId,
    ) {
        self.graphics.insert_texture(creation, com_object_id);
    }

    pub(crate) fn lock_graphics_texture(
        &mut self,
        texture_com_object_id: RuntimeComObjectId,
        level: u32,
        rect: super::RuntimeRect,
        flags: u32,
        output_address: GuestAddress,
    ) -> Result<bool, RuntimeStateError> {
        self.memory
            .validate_access(output_address, 8, MemoryAccess::Write)
            .map_err(RuntimeStateError::from_memory)?;
        self.require_graphics_operation_state()?;
        let texture = self
            .graphics
            .texture_for_com_object(texture_com_object_id)?;
        let texture_id = texture.id;
        let existing_backing = texture.backing;
        let (row_pitch, byte_length) = texture_byte_layout(texture.descriptor)?;
        if let Some(backing) = existing_backing {
            self.validate_graphics_texture_backing(backing, byte_length)?;
        }
        let request = if existing_backing.is_none() {
            let arena = self
                .address_space_allocator
                .allocation_for_purpose(&ProcessAddressAllocationPurpose::GraphicsResourceArena)?;
            if arena.range.base != super::RUNTIME_GRAPHICS_RESOURCE_ARENA_BASE
                || arena.range.byte_length != super::RUNTIME_GRAPHICS_RESOURCE_ARENA_SIZE
                || arena.classification != ProcessAddressAllocationClassification::Graphics
                || arena.release_policy != ProcessAddressAllocationReleasePolicy::Permanent
                || arena.executable
            {
                return Err(RuntimeStateError::new(
                    "graphics resource arena does not match the controlled process layout",
                ));
            }
            Some((
                Some(arena.id),
                ProcessAddressAllocationRequest {
                    purpose: ProcessAddressAllocationPurpose::GraphicsResource,
                    preferred_address: None,
                    search_range: arena.range,
                    reserve_size: u64::from(byte_length),
                    alignment: GUEST_PAGE_SIZE as u32,
                    guard_pages: 0,
                    classification: ProcessAddressAllocationClassification::Graphics,
                    release_policy: ProcessAddressAllocationReleasePolicy::Explicit,
                    executable: false,
                },
            ))
        } else {
            None
        };
        let graphics = &mut self.graphics;
        let memory = &mut self.memory;
        let mut lock_backing = |allocation: Option<&ProcessAddressAllocationRecord>| {
            let range = allocation
                .map(|allocation| {
                    GuestRange::new(allocation.range.base, allocation.range.byte_length)
                })
                .transpose()
                .map_err(RuntimeStateError::from_memory)?;
            let Some(lock) =
                graphics.preview_texture_lock(texture_com_object_id, level, rect, flags, range)?
            else {
                return Ok(None);
            };
            let mut transaction = memory.begin_transaction();
            let backing = if let Some(allocation) = allocation {
                let memory_owner = transaction
                    .create_owner(MemoryOwnerKind::Runtime)
                    .map_err(RuntimeStateError::from_memory)?;
                transaction
                    .reserve_fixed_for_address_allocation(
                        allocation.id.0,
                        memory_owner,
                        MemoryRegionKind::GraphicsResource,
                        allocation.range.base,
                        allocation.range.byte_length,
                    )
                    .map_err(RuntimeStateError::from_memory)?;
                let range = range.expect("new texture allocation has a range");
                transaction
                    .commit(
                        memory_owner,
                        range,
                        PageProtection {
                            read: true,
                            write: flags & super::graphics::RUNTIME_GRAPHICS_TEXTURE_LOCK_READONLY
                                == 0,
                            execute: false,
                        },
                    )
                    .map_err(RuntimeStateError::from_memory)?;
                Some(RuntimeGraphicsTextureBacking {
                    address_allocation_id: allocation.id,
                    memory_owner,
                    range,
                    row_pitch,
                })
            } else {
                let backing = existing_backing.expect("existing texture backing was validated");
                transaction
                    .protect(
                        backing.memory_owner,
                        backing.range,
                        PageProtection {
                            read: true,
                            write: flags & super::graphics::RUNTIME_GRAPHICS_TEXTURE_LOCK_READONLY
                                == 0,
                            execute: false,
                        },
                    )
                    .map_err(RuntimeStateError::from_memory)?;
                None
            };
            let mut bytes = [0_u8; 8];
            bytes[..4].copy_from_slice(&lock.pitch.to_le_bytes());
            bytes[4..].copy_from_slice(&lock.guest_address.0.to_le_bytes());
            transaction
                .write(output_address, &bytes)
                .map_err(RuntimeStateError::from_memory)?;
            graphics.publish_texture_lock(texture_id, lock, backing);
            transaction.commit_transaction();
            Ok(Some(()))
        };
        if let Some(request) = request {
            let mut range_available = false;
            let result = self.address_space_allocator.try_reserve_batch_with(
                vec![request],
                |allocations| {
                    range_available = true;
                    lock_backing(allocations.first())
                },
            )?;
            if !range_available {
                return Err(RuntimeStateError::new(
                    "process address reservation has no fitting range or allocation ID",
                ));
            }
            Ok(result.is_some())
        } else {
            Ok(lock_backing(None)?.is_some())
        }
    }

    pub(crate) fn unlock_graphics_texture(
        &mut self,
        com_object_id: RuntimeComObjectId,
    ) -> Result<bool, RuntimeStateError> {
        self.require_graphics_operation_state()?;
        let texture = self.graphics.texture_for_com_object(com_object_id)?;
        if texture.active_lock.is_none() {
            return Ok(false);
        }
        let backing = texture.backing.ok_or_else(|| {
            RuntimeStateError::new(
                "logical graphics texture has an active lock without texel backing",
            )
        })?;
        let (_, byte_length) = texture_byte_layout(texture.descriptor)?;
        self.validate_graphics_texture_backing(backing, byte_length)?;
        let mut transaction = self.memory.begin_transaction();
        transaction
            .protect(
                backing.memory_owner,
                backing.range,
                PageProtection {
                    read: true,
                    write: false,
                    execute: false,
                },
            )
            .map_err(RuntimeStateError::from_memory)?;
        self.graphics.unlock_texture(com_object_id)?;
        transaction.commit_transaction();
        Ok(true)
    }

    fn validate_graphics_texture_backing(
        &self,
        backing: RuntimeGraphicsTextureBacking,
        byte_length: u32,
    ) -> Result<(), RuntimeStateError> {
        let allocation = self
            .address_space_allocator
            .allocation(backing.address_allocation_id)?;
        let arena = self
            .address_space_allocator
            .allocation_for_purpose(&ProcessAddressAllocationPurpose::GraphicsResourceArena)?;
        let mut regions = self
            .memory
            .regions()
            .filter(|region| region.address_allocation_id == Some(backing.address_allocation_id.0));
        let region = regions.next().ok_or_else(|| {
            RuntimeStateError::new("texture backing has no authoritative guest region")
        })?;
        if regions.next().is_some()
            || region.owner != backing.memory_owner
            || region.range != backing.range
            || region.kind != MemoryRegionKind::GraphicsResource
            || allocation.range.base != backing.range.start()
            || allocation.range.byte_length != backing.range.byte_length()
            || allocation.parent_id != Some(arena.id)
            || allocation.purpose != ProcessAddressAllocationPurpose::GraphicsResource
            || allocation.classification != ProcessAddressAllocationClassification::Graphics
            || allocation.release_policy != ProcessAddressAllocationReleasePolicy::Explicit
            || allocation.executable
        {
            return Err(RuntimeStateError::new(
                "texture backing disagrees with its live memory owner and process allocation",
            ));
        }
        self.memory
            .validate_access(
                backing.range.start(),
                byte_length as usize,
                MemoryAccess::Read,
            )
            .map_err(RuntimeStateError::from_memory)
    }

    pub(crate) fn mark_graphics_texture_dirty_region(
        &mut self,
        texture_com_object_id: RuntimeComObjectId,
        rect: Option<super::RuntimeRect>,
    ) -> Result<bool, RuntimeStateError> {
        self.require_graphics_operation_state()?;
        self.graphics
            .mark_texture_dirty_region(texture_com_object_id, rect)
    }

    pub(crate) fn update_graphics_texture(
        &mut self,
        device_com_object_id: RuntimeComObjectId,
        source_texture_com_object_id: RuntimeComObjectId,
        destination_texture_com_object_id: RuntimeComObjectId,
    ) -> Result<bool, RuntimeStateError> {
        self.require_graphics_operation_state()?;
        self.graphics.update_texture_with_memory(
            device_com_object_id,
            source_texture_com_object_id,
            destination_texture_com_object_id,
            Some(&self.memory),
        )
    }

    pub(crate) fn graphics_texture_for_com_object(
        &self,
        com_object_id: RuntimeComObjectId,
    ) -> Result<&RuntimeGraphicsTexture, RuntimeStateError> {
        self.require_graphics_operation_state()?;
        self.graphics.texture_for_com_object(com_object_id)
    }

    pub(crate) fn lock_graphics_vertex_buffer(
        &mut self,
        vertex_buffer_com_object_id: RuntimeComObjectId,
        offset: u32,
        byte_length: u32,
        flags: u32,
        output_address: GuestAddress,
    ) -> Result<bool, RuntimeStateError> {
        self.memory
            .validate_access(output_address, 4, MemoryAccess::Write)
            .map_err(RuntimeStateError::from_memory)?;
        self.require_graphics_operation_state()?;
        let Some(guest_address) = self.graphics.preview_vertex_buffer_lock(
            vertex_buffer_com_object_id,
            offset,
            byte_length,
            flags,
        )?
        else {
            return Ok(false);
        };
        let vertex_buffer = self
            .graphics
            .vertex_buffer_for_com_object(vertex_buffer_com_object_id)?;
        let id = vertex_buffer.id;
        let backing = vertex_buffer.backing;
        let mut transaction = self.memory.begin_transaction();
        transaction
            .protect(
                backing.memory_owner,
                backing.range,
                PageProtection {
                    read: true,
                    write: true,
                    execute: false,
                },
            )
            .map_err(RuntimeStateError::from_memory)?;
        transaction
            .write(output_address, &guest_address.0.to_le_bytes())
            .map_err(RuntimeStateError::from_memory)?;
        self.graphics.publish_vertex_buffer_lock(id);
        transaction.commit_transaction();
        Ok(true)
    }

    pub(crate) fn unlock_graphics_vertex_buffer(
        &mut self,
        com_object_id: RuntimeComObjectId,
    ) -> Result<bool, RuntimeStateError> {
        self.require_graphics_operation_state()?;
        let vertex_buffer = self.graphics.vertex_buffer_for_com_object(com_object_id)?;
        if !vertex_buffer.locked {
            return Ok(false);
        }
        let backing = vertex_buffer.backing;
        let mut transaction = self.memory.begin_transaction();
        transaction
            .protect(
                backing.memory_owner,
                backing.range,
                PageProtection {
                    read: true,
                    write: false,
                    execute: false,
                },
            )
            .map_err(RuntimeStateError::from_memory)?;
        let unlocked = self.graphics.unlock_vertex_buffer(com_object_id)?;
        transaction.commit_transaction();
        Ok(unlocked)
    }

    pub(crate) fn validate_graphics_surface_creation(
        &self,
        texture_com_object_id: RuntimeComObjectId,
        level: u32,
    ) -> Result<
        Option<(
            RuntimeGraphicsSurfaceId,
            RuntimeGraphicsTextureId,
            RuntimeGraphicsSurfaceDescriptor,
        )>,
        RuntimeStateError,
    > {
        self.require_graphics_operation_state()?;
        self.graphics
            .validate_surface_creation(texture_com_object_id, level)
    }

    pub(crate) fn insert_graphics_surface(
        &mut self,
        creation: (
            RuntimeGraphicsSurfaceId,
            RuntimeGraphicsTextureId,
            RuntimeGraphicsSurfaceDescriptor,
        ),
        com_object_id: RuntimeComObjectId,
    ) {
        self.graphics.insert_surface(creation, com_object_id);
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

    fn resolve_audio_cooperative_state(
        &self,
        window_handle: u32,
    ) -> Result<Option<RuntimeAudioCooperativeState>, RuntimeStateError> {
        let Some(window) = self.user32.window_option(window_handle)? else {
            return Ok(None);
        };
        if window.lifecycle != RuntimeWindowLifecycle::Created
            || window.creation.parent != 0
            || window.creation.owner_thread_id != self.current_thread_id()
        {
            return Ok(None);
        }
        Ok(Some(RuntimeAudioCooperativeState {
            window_id: window.id,
            window_handle: window.handle,
            owner_thread_id: window.creation.owner_thread_id,
        }))
    }

    fn resolve_graphics_window_target(
        &self,
        window_handle: u32,
    ) -> Result<Option<RuntimeGraphicsWindowTarget>, RuntimeStateError> {
        let Some(window) = self.user32.window_option(window_handle)? else {
            return Ok(None);
        };
        if window.lifecycle != RuntimeWindowLifecycle::Created {
            return Ok(None);
        }
        if window.creation.owner_thread_id != self.current_thread_id() {
            return Ok(None);
        }
        Ok(Some(RuntimeGraphicsWindowTarget {
            window_id: window.id,
            window_handle: window.handle,
            owner_thread_id: window.creation.owner_thread_id,
            client_width: window.creation.geometry.client_width,
            client_height: window.creation.geometry.client_height,
        }))
    }

    fn resolve_gdi_window_target(
        &self,
        window_handle: u32,
    ) -> Result<RuntimeGdiWindowTarget, RuntimeStateError> {
        let window = self.user32.window(window_handle)?;
        if window.lifecycle != RuntimeWindowLifecycle::Created {
            return Err(RuntimeStateError::new(
                "logical window DC access requires a created window",
            ));
        }
        if window.creation.owner_thread_id != self.current_thread_id() {
            return Err(RuntimeStateError::new(
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
            RuntimeStateError::new("screen DC access requires a configured logical primary monitor")
        })?;
        if !monitor.primary {
            return Err(RuntimeStateError::new(
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
    ) -> Result<RuntimeWindowProcedure, RuntimeStateError> {
        self.require_user32_operation_state()?;
        self.user32.resolve_small_icon_window_procedure(
            window_handle,
            self.current_thread_id(),
            icon_handle,
        )
    }

    pub(crate) fn set_window_small_icon(
        &mut self,
        window_handle: u32,
        icon_handle: u32,
    ) -> Result<u32, RuntimeStateError> {
        self.require_user32_operation_state()?;
        self.user32
            .set_window_small_icon(window_handle, self.current_thread_id(), icon_handle)
    }

    pub(crate) fn set_window_title(
        &mut self,
        window_handle: u32,
        title: RuntimeWindowNameA,
    ) -> Result<(), RuntimeStateError> {
        self.require_user32_operation_state()?;
        self.user32
            .set_window_title(window_handle, self.current_thread_id(), title)
    }

    pub(crate) fn window_property(
        &self,
        handle: u32,
        name: &super::RuntimeAtomName,
    ) -> Result<Option<u32>, RuntimeStateError> {
        self.user32
            .window_property(handle, self.global_atoms.find(name))
    }

    pub(crate) fn set_window_property(
        &mut self,
        handle: u32,
        name: super::RuntimeAtomName,
        data: u32,
    ) -> Result<Result<(), u32>, RuntimeStateError> {
        self.user32
            .set_window_property(handle, name, data, &mut self.global_atoms)
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
        let action = self
            .user32
            .preview_set_window_pos_callback(operation, message, result)?;
        if operation.variant == RuntimeSetWindowPosVariant::Resize
            && message == RuntimeSetWindowPosCallbackMessage::NonClientCalculate
        {
            self.gdi.validate_window_surface_resize(
                operation.window_id,
                operation.previous_geometry.client_width,
                operation.previous_geometry.client_height,
                operation.next_geometry.client_width,
                operation.next_geometry.client_height,
            )?;
        }
        Ok(action)
    }

    pub(crate) fn complete_set_window_pos_callback(
        &mut self,
        operation: RuntimeSetWindowPosOperation,
        message: RuntimeSetWindowPosCallbackMessage,
        result: u32,
    ) -> Result<RuntimeSetWindowPosCallbackAction, RuntimeStateError> {
        self.require_user32_operation_state()?;
        self.require_window_owner_thread(operation.window_id)?;
        let disposition = self
            .user32
            .checked_set_window_pos_callback(operation, message, result)?;
        if operation.variant != RuntimeSetWindowPosVariant::Resize
            || message != RuntimeSetWindowPosCallbackMessage::NonClientCalculate
        {
            return Ok(self.user32.publish_set_window_pos_callback(disposition));
        }

        self.gdi.resize_window_surface(
            operation.window_id,
            operation.previous_geometry.client_width,
            operation.previous_geometry.client_height,
            operation.next_geometry.client_width,
            operation.next_geometry.client_height,
        )?;
        Ok(self.user32.publish_set_window_pos_callback(disposition))
    }

    pub(crate) fn create_menu(&mut self, popup: bool) -> Result<u32, RuntimeStateError> {
        self.require_user32_operation_state()?;
        self.user32.create_menu(popup)
    }

    pub(crate) fn set_menu(
        &mut self,
        window_handle: u32,
        menu_handle: u32,
    ) -> Result<(), RuntimeStateError> {
        self.require_user32_operation_state()?;
        self.user32
            .set_menu(self.current_thread_id(), window_handle, menu_handle)
    }

    pub(crate) fn menu_count(&self, handle: u32) -> Option<usize> {
        self.user32.menus.count(handle)
    }

    pub(crate) fn menu_item(
        &self,
        handle: u32,
        item: u32,
        by_position: bool,
    ) -> Option<&super::RuntimeMenuItem> {
        self.user32.menus.get(handle, item, by_position)
    }

    pub(crate) fn put_menu_item(
        &mut self,
        handle: u32,
        item: u32,
        by_position: bool,
        mask: u32,
        incoming: super::RuntimeMenuItem,
        insert: bool,
    ) -> Result<bool, RuntimeStateError> {
        self.require_user32_operation_state()?;
        self.user32
            .menus
            .put(handle, item, by_position, mask, incoming, insert)
    }

    pub(crate) fn remove_menu_item(
        &mut self,
        handle: u32,
        item: u32,
        by_position: bool,
    ) -> Result<bool, RuntimeStateError> {
        self.require_user32_operation_state()?;
        Ok(self.user32.menus.remove(handle, item, by_position))
    }

    pub(crate) fn create_accelerators(
        &mut self,
        entries: Vec<super::RuntimeAccelerator>,
    ) -> Result<u32, RuntimeStateError> {
        self.require_user32_operation_state()?;
        self.user32.create_accelerators(entries)
    }

    pub(crate) fn destroy_accelerators(&mut self, handle: u32) -> Result<bool, RuntimeStateError> {
        self.require_user32_operation_state()?;
        Ok(self.user32.destroy_accelerators(handle))
    }

    pub(crate) fn accelerators(&self, handle: u32) -> Option<&[super::RuntimeAccelerator]> {
        self.user32.accelerators(handle)
    }

    pub(crate) fn invalidate_full_client(
        &mut self,
        window_handle: u32,
        erase: bool,
    ) -> Result<(), RuntimeStateError> {
        self.require_user32_operation_state()?;
        self.user32
            .invalidate_full_client(self.current_thread_id(), window_handle, erase)
    }

    pub(crate) fn preview_update_window(
        &self,
        window_handle: u32,
        entry_last_error: u32,
        invalidate_full: bool,
    ) -> Result<RuntimeUpdateWindowOperation, RuntimeStateError> {
        self.require_user32_operation_state()?;
        self.user32.preview_update_window(
            self.current_thread_id(),
            window_handle,
            entry_last_error,
            invalidate_full,
        )
    }

    pub(crate) fn validate_update_window_callback(
        &self,
        operation: RuntimeUpdateWindowOperation,
    ) -> Result<(), RuntimeStateError> {
        self.require_user32_operation_state()?;
        self.require_window_owner_thread(operation.window_id)?;
        self.user32.validate_update_window_callback(operation)
    }

    pub(crate) fn preview_set_window_pos_focus(
        &self,
        window_handle: u32,
    ) -> Result<RuntimeSetWindowPosDefaultOperation, RuntimeStateError> {
        self.require_user32_operation_state()?;
        self.user32
            .preview_set_window_pos_focus(window_handle, self.current_thread_id())
    }

    pub(crate) fn focus_set_window_pos_window(
        &mut self,
        window_handle: u32,
    ) -> Result<RuntimeSetWindowPosDefaultOperation, RuntimeStateError> {
        self.require_user32_operation_state()?;
        self.user32
            .focus_set_window_pos_window(window_handle, self.current_thread_id())
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

    pub(crate) fn active_window_handle(&self) -> Result<Option<u32>, RuntimeStateError> {
        self.user32.active_window_handle(self.current_thread_id())
    }

    pub(crate) fn focused_window_handle(&self) -> Result<Option<u32>, RuntimeStateError> {
        self.user32.focused_window_handle(self.current_thread_id())
    }

    pub(crate) fn create_caret(
        &mut self,
        handle: u32,
        width: u32,
        height: u32,
        gray: bool,
    ) -> Result<(), RuntimeStateError> {
        self.user32
            .create_caret(self.current_thread_id(), handle, width, height, gray)
    }

    pub(crate) fn window_from_point(
        &self,
        x: i32,
        y: i32,
    ) -> Result<Option<u32>, RuntimeStateError> {
        self.user32.window_from_point(x, y)
    }

    pub(crate) fn capture_window_handle(&self) -> Result<Option<u32>, RuntimeStateError> {
        self.user32.capture_window_handle(self.current_thread_id())
    }

    pub(crate) fn set_capture_window(&mut self, handle: u32) -> Result<(), RuntimeStateError> {
        self.require_user32_operation_state()?;
        self.user32
            .set_capture_window(self.current_thread_id(), handle)
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
            return Err(RuntimeStateError::new(
                "logical window owner must be the selected creating guest thread",
            ));
        }
        let image = self
            .modules
            .images()
            .image(request.instance.module_id)
            .map_err(runtime_module_image_error)?;
        if image.image_range.start() != request.instance.load_base {
            return Err(RuntimeStateError::new(
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
            return Err(RuntimeStateError::new(
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
            return Err(RuntimeStateError::new(
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
            return Err(RuntimeStateError::new(
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
            return Err(RuntimeStateError::new(
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
            return Err(RuntimeStateError::new(
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
            return Err(RuntimeStateError::new(
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
            return Err(RuntimeStateError::new(
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
            return Err(RuntimeStateError::new(
                "waitable timer operation requires a running process and active current thread",
            ));
        }
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn mutex_for_handle(
        &self,
        handle: RuntimeHandle,
    ) -> Result<&RuntimeMutex, RuntimeStateError> {
        let record = self
            .objects
            .object_for_handle(handle, RuntimeObjectKind::Mutex)?;
        let RuntimeObjectKey::Mutex { mutex_id } = &record.key else {
            return Err(RuntimeStateError::new(
                "mutex handle resolved to a non-mutex object identity",
            ));
        };
        self.synchronization.mutex(RuntimeMutexId(*mutex_id))
    }

    pub(crate) fn try_release_mutex(
        &mut self,
        handle: RuntimeHandle,
    ) -> Result<Result<(), u32>, RuntimeStateError> {
        const ERROR_INVALID_HANDLE: u32 = 6;
        const ERROR_NOT_OWNER: u32 = 288;
        self.require_mutex_operation_state()?;
        let Some(record) = self
            .objects
            .live_object_for_handle(handle, RuntimeObjectKind::Mutex)?
        else {
            return Ok(Err(ERROR_INVALID_HANDLE));
        };
        let RuntimeObjectKey::Mutex { mutex_id } = record.key else {
            return Err(RuntimeStateError::new(
                "mutex handle resolved to a non-mutex object identity",
            ));
        };
        let id = RuntimeMutexId(mutex_id);
        let current_thread_id = self.current_thread_id();
        let mutex = self.synchronization.mutex(id)?;
        if mutex.owning_thread_id != Some(current_thread_id) || mutex.recursion_count == 0 {
            return Ok(Err(ERROR_NOT_OWNER));
        }
        self.synchronization.release_mutex(id, current_thread_id)?;
        Ok(Ok(()))
    }

    #[cfg(test)]
    pub(crate) fn release_mutex(
        &mut self,
        handle: RuntimeHandle,
    ) -> Result<&RuntimeMutex, RuntimeStateError> {
        self.require_mutex_operation_state()?;
        let id = self.mutex_for_handle(handle)?.id;
        self.synchronization
            .release_mutex(id, self.current_thread_id())
    }

    fn require_mutex_operation_state(&self) -> Result<(), RuntimeStateError> {
        if !self.process.is_running()
            || self.current_thread()?.lifecycle != RuntimeThreadLifecycleState::Running
            || !matches!(
                self.scheduler_state()?,
                SchedulerThreadState::Runnable | SchedulerThreadState::InCallback { .. }
            )
        {
            return Err(RuntimeStateError::new(
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
            return Err(RuntimeStateError::new(
                "event operation requires a running process and active current thread",
            ));
        }
        Ok(())
    }

    pub(crate) fn initialize_critical_section(
        &mut self,
        address: GuestAddress,
        spin_count: u32,
    ) -> Result<&RuntimeCriticalSection, RuntimeStateError> {
        self.require_critical_section_operation_state()?;
        if spin_count > 0x00ff_ffff {
            return Err(RuntimeStateError::new(
                "critical section spin count exceeds the Win32 24-bit range",
            ));
        }
        self.synchronization.initialize(address, spin_count)
    }

    pub(crate) fn initialize_critical_section_with_spin_state(
        &mut self,
        address: GuestAddress,
        spin_count_field: u32,
    ) -> Result<&RuntimeCriticalSection, RuntimeStateError> {
        self.require_critical_section_operation_state()?;
        self.synchronization.initialize(address, spin_count_field)
    }

    pub(crate) fn enter_critical_section(
        &mut self,
        address: GuestAddress,
    ) -> Result<&RuntimeCriticalSection, RuntimeStateError> {
        self.require_critical_section_operation_state()?;
        let current_thread_id = self.current_thread_id();
        self.synchronization.enter(address, current_thread_id)
    }

    pub(crate) fn wait_current_thread_for_critical_section(
        &mut self,
        address: GuestAddress,
    ) -> Result<(), RuntimeStateError> {
        self.require_critical_section_operation_state()?;
        let current_thread_id = self.current_thread_id();
        let executable_competitor = self.preferred_executable_thread(false)?.is_some();
        let owner_thread_id = self
            .synchronization
            .critical_section(address)?
            .owning_thread_id
            .ok_or_else(|| RuntimeStateError::new("critical section wait requires a live owner"))?;
        let owner_host_wait = match self.scheduler.state(owner_thread_id)? {
            SchedulerThreadState::WaitingHost { request_id } => self
                .scheduler
                .host_requests()
                .get(&request_id)
                .is_some_and(|request| request.thread_id == owner_thread_id),
            _ => false,
        };
        if !executable_competitor && !owner_host_wait {
            return Err(RuntimeStateError::new(
                "contended critical section has neither an executable lane nor a host-waiting owner",
            ));
        }
        self.synchronization
            .validate_wait(address, current_thread_id)?;
        self.scheduler.state(current_thread_id)?;
        // Both owners have been checked; no control leaves Rust before publication.
        self.synchronization
            .enqueue_waiter(address, current_thread_id);
        self.scheduler.set_validated_state(
            current_thread_id,
            SchedulerThreadState::WaitingCriticalSection { address: address.0 },
        );
        Ok(())
    }

    pub(crate) fn leave_critical_section(
        &mut self,
        address: GuestAddress,
    ) -> Result<&RuntimeCriticalSection, RuntimeStateError> {
        self.require_critical_section_operation_state()?;
        let current_thread_id = self.current_thread_id();
        let awakened_thread_id = self
            .synchronization
            .validate_leave(address, current_thread_id)?;
        let resumed_lane = awakened_thread_id
            .map(|thread_id| {
                if self.thread_state(thread_id)?.lifecycle != RuntimeThreadLifecycleState::Running
                    || self.scheduler_state_for_thread(thread_id)?
                        != (SchedulerThreadState::WaitingCriticalSection { address: address.0 })
                {
                    return Err(RuntimeStateError::new(
                        "critical section waiter does not have a matching live scheduler lane",
                    ));
                }
                let resumed_state = self.resumed_execution_state_for_thread(thread_id)?;
                Ok((thread_id, resumed_state))
            })
            .transpose()?;
        self.synchronization.leave(address, current_thread_id)?;
        if let Some((thread_id, resumed_state)) = resumed_lane {
            self.scheduler.set_validated_state(thread_id, resumed_state);
        }
        self.synchronization.critical_section(address)
    }

    pub(crate) fn delete_critical_section(
        &mut self,
        address: GuestAddress,
    ) -> Result<RuntimeCriticalSection, RuntimeStateError> {
        self.require_critical_section_operation_state()?;
        self.synchronization.delete(address)
    }

    pub(crate) fn critical_section(
        &self,
        address: GuestAddress,
    ) -> Result<&RuntimeCriticalSection, RuntimeStateError> {
        self.synchronization.critical_section(address)
    }

    #[cfg(test)]
    pub(crate) fn critical_section_waiters(&self, address: GuestAddress) -> Vec<RuntimeThreadId> {
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
            return Err(RuntimeStateError::new(
                "critical section operations require a running process and active current thread",
            ));
        }
        Ok(())
    }

    pub(crate) fn encode_pointer(&self, source: u32) -> u32 {
        self.registries.encode_pointer(source)
    }

    pub(crate) fn decode_pointer(&self, token: u32) -> u32 {
        self.registries.decode_pointer(token)
    }

    #[cfg(test)]
    pub(crate) fn push_callback_frame(
        &mut self,
        target: GuestAddress,
        return_sentinel: GuestAddress,
    ) -> Result<RuntimeCallbackFrame, RuntimeStateError> {
        let frame = self.prepare_callback_frame(None, target, return_sentinel, 0)?;
        self.publish_callback_frame(frame, false);
        Ok(frame)
    }

    pub(crate) fn callback_scratch_allocation(
        &self,
    ) -> Result<&ProcessAddressAllocationRecord, RuntimeStateError> {
        self.address_space_allocator
            .allocation_for_purpose(&ProcessAddressAllocationPurpose::CallbackScratch)
    }

    pub(crate) fn prepare_callback_frame(
        &self,
        replacing: Option<RuntimeCallbackFrameId>,
        target: GuestAddress,
        return_sentinel: GuestAddress,
        scratch_bytes: u32,
    ) -> Result<RuntimeCallbackFrame, RuntimeStateError> {
        Self::prepare_callback_frame_state(
            &self.threads,
            &self.scheduler,
            replacing,
            target,
            return_sentinel,
            scratch_bytes,
        )
    }

    fn prepare_callback_frame_state(
        threads: &RuntimeThreads,
        scheduler: &RuntimeScheduler,
        replacing: Option<RuntimeCallbackFrameId>,
        target: GuestAddress,
        return_sentinel: GuestAddress,
        scratch_bytes: u32,
    ) -> Result<RuntimeCallbackFrame, RuntimeStateError> {
        let thread_id = scheduler.current_thread_id();
        let scheduler_state = scheduler.current_state()?;
        Self::prepare_callback_frame_in_state(
            threads,
            thread_id,
            scheduler_state,
            replacing,
            target,
            return_sentinel,
            scratch_bytes,
        )
    }

    pub(crate) fn prepare_media_callback_frame(
        &self,
        request_id: u64,
        target: GuestAddress,
        scratch_bytes: u32,
        callback_stack: GuestRange,
    ) -> Result<RuntimeCallbackFrame, RuntimeStateError> {
        let request = self.scheduler.validate_host_completion(request_id)?;
        if request.service != "browser.media" || request.thread_id != self.current_thread_id() {
            return Err(RuntimeStateError::new(
                "media read callback does not own its actual host wait",
            ));
        }
        self.memory
            .validate_access(
                crate::callbacks::CALLBACK_RETURN_ARENA_BASE,
                1,
                MemoryAccess::Execute,
            )
            .map_err(RuntimeStateError::from_memory)?;
        let frame = Self::prepare_callback_frame_in_state(
            &self.threads,
            request.thread_id,
            request.resume_state,
            None,
            target,
            crate::callbacks::CALLBACK_RETURN_ARENA_BASE,
            scratch_bytes,
        )?;
        let stack =
            self.current_thread_state()?.stack.as_ref().ok_or_else(|| {
                RuntimeStateError::new("media callback has no owned caller stack")
            })?;
        if callback_stack.start().0 < stack.committed_range.start().0
            || callback_stack.end() > stack.committed_range.end()
        {
            return Err(RuntimeStateError::new(
                "media callback extends outside its committed caller stack",
            ));
        }
        self.memory
            .validate_access(
                callback_stack.start(),
                callback_stack.byte_length() as usize,
                MemoryAccess::Write,
            )
            .map_err(RuntimeStateError::from_memory)?;
        let allocation = self.callback_scratch_allocation()?;
        let region = self
            .memory
            .regions()
            .find(|region| region.address_allocation_id == Some(allocation.id.0));
        if let Some(region) = region
            && (region.kind != MemoryRegionKind::Runtime
                || region.range.start() != allocation.range.base
                || region.range.byte_length() != allocation.range.byte_length)
        {
            return Err(RuntimeStateError::new(
                "media callback scratch mapping lost its allocation owner",
            ));
        }
        if let Some(slot) = frame.scratch {
            let page = self
                .memory
                .committed_page(slot.address)
                .map_err(RuntimeStateError::from_memory)?;
            match page {
                Some((owner, protection))
                    if region.is_some_and(|region| {
                        region.owner == owner
                            && protection
                                == PageProtection {
                                    read: true,
                                    write: true,
                                    execute: false,
                                }
                    }) => {}
                Some(_) => {
                    return Err(RuntimeStateError::new(
                        "media callback scratch page has wrong owner or protection",
                    ));
                }
                None => self
                    .memory
                    .preflight_growth(
                        usize::from(region.is_none()),
                        usize::from(region.is_none()),
                        slot.capacity as usize / GUEST_PAGE_SIZE as usize,
                    )
                    .map_err(RuntimeStateError::from_memory)?,
            }
        }
        Ok(frame)
    }

    fn prepare_callback_frame_in_state(
        threads: &RuntimeThreads,
        thread_id: RuntimeThreadId,
        scheduler_state: SchedulerThreadState,
        replacing: Option<RuntimeCallbackFrameId>,
        target: GuestAddress,
        return_sentinel: GuestAddress,
        scratch_bytes: u32,
    ) -> Result<RuntimeCallbackFrame, RuntimeStateError> {
        let current_depth = threads.callback_depth(thread_id)?;
        match scheduler_state {
            SchedulerThreadState::Runnable if current_depth == 0 && replacing.is_none() => {}
            SchedulerThreadState::InCallback { depth } if depth == current_depth => {}
            _ => {
                return Err(RuntimeStateError::new(format!(
                    "callback entry on thread {} requires a runnable thread or the matching nested callback state; scheduler is {scheduler_state:?} and callback depth is {current_depth}",
                    thread_id.0,
                )));
            }
        }
        let id = threads.next_callback_frame_id()?;
        let depth = threads.validate_callback_entry(thread_id, replacing)?;
        Ok(RuntimeCallbackFrame {
            id,
            thread_id,
            target,
            return_sentinel,
            entry_last_error: threads.last_error(thread_id)?,
            scratch: callback_scratch_for_depth(depth, scratch_bytes)?,
        })
    }

    pub(crate) fn pop_callback_frame(
        &mut self,
        expected: RuntimeCallbackFrameId,
    ) -> Result<RuntimeCallbackFrame, RuntimeStateError> {
        let thread_id = self.current_thread_id();
        self.validate_callback_return_state()?;
        let frame = self.threads.pop_callback_frame(thread_id, expected)?;
        let depth = self
            .threads
            .callback_depth(thread_id)
            .expect("validated callback thread");
        self.scheduler.set_validated_state(
            thread_id,
            if depth == 0 {
                SchedulerThreadState::Runnable
            } else {
                SchedulerThreadState::InCallback { depth }
            },
        );
        self.pending_api_callbacks.remove(&frame.id);
        Ok(frame)
    }

    pub(crate) fn validate_callback_frame_return(
        &self,
        expected: RuntimeCallbackFrameId,
    ) -> Result<(), RuntimeStateError> {
        self.validate_callback_return_state()?;
        self.threads
            .callback_frame_for_return(self.current_thread_id(), expected)?;
        Ok(())
    }

    fn validate_callback_return_state(&self) -> Result<(), RuntimeStateError> {
        let thread_id = self.current_thread_id();
        let current_depth = self.threads.callback_depth(thread_id)?;
        if self.scheduler_state()?
            != (SchedulerThreadState::InCallback {
                depth: current_depth,
            })
        {
            return Err(RuntimeStateError::new(
                "callback return requires the scheduler to identify the current callback depth",
            ));
        }
        Ok(())
    }

    pub(crate) fn publish_callback_frame(&mut self, frame: RuntimeCallbackFrame, replacing: bool) {
        let exited = self.threads.publish_callback_frame(frame, replacing);
        let depth = self
            .threads
            .callback_depth(frame.thread_id)
            .expect("validated callback thread");
        self.scheduler
            .set_validated_state(frame.thread_id, SchedulerThreadState::InCallback { depth });
        if let Some(exited) = exited {
            self.pending_api_callbacks.remove(&exited.id);
        }
    }

    pub(crate) fn begin_message_box(
        &mut self,
        owner_handle: u32,
        title: String,
        text: String,
        flags: u32,
    ) -> Result<crate::api::HostRequest, RuntimeStateError> {
        if !self.process.is_running() || self.pending_message_boxes.len() >= 32 {
            return Err(RuntimeStateError::new(
                "MessageBox requires a running process and available dialog capacity",
            ));
        }
        let dialog = RuntimeMessageBox::new(title, text, flags)?;
        let owner = if owner_handle == 0 {
            None
        } else {
            let window = self.user32.window(owner_handle)?;
            if window.lifecycle != RuntimeWindowLifecycle::Created {
                return Err(RuntimeStateError::new(
                    "MessageBox owner must be a created window",
                ));
            }
            let thread = self.threads.state(window.creation.owner_thread_id)?;
            if thread.lifecycle != RuntimeThreadLifecycleState::Running {
                return Err(RuntimeStateError::new(
                    "MessageBox owner thread is not live",
                ));
            }
            Some((owner_handle, window.id))
        };
        let request = self
            .scheduler
            .checked_host_request(RUNTIME_MESSAGE_BOX_SERVICE.to_owned())?;
        self.pending_message_boxes.insert(
            request.request_id,
            PendingMessageBox {
                thread_id: request.thread_id,
                owner,
                dialog,
            },
        );
        let request = self.scheduler.publish_host_request(request);
        Ok(crate::api::HostRequest {
            request_id: request.request_id,
            service: request.service,
            payload: Vec::new(),
        })
    }

    pub fn message_box(&self, request_id: u64) -> Result<&RuntimeMessageBox, RuntimeStateError> {
        let pending = self
            .pending_message_boxes
            .get(&request_id)
            .ok_or_else(|| RuntimeStateError::new("MessageBox request is not pending"))?;
        Ok(&pending.dialog)
    }

    pub(crate) fn validate_message_box_result(
        &self,
        request_id: u64,
        button_id: u32,
    ) -> Result<(), RuntimeStateError> {
        let pending = self
            .pending_message_boxes
            .get(&request_id)
            .ok_or_else(|| RuntimeStateError::new("MessageBox request is not pending"))?;
        let host = self.scheduler.validate_host_completion(request_id)?;
        if host.service != RUNTIME_MESSAGE_BOX_SERVICE
            || host.thread_id != pending.thread_id
            || self.current_thread_id() != pending.thread_id
            || !self.process.is_running()
            || self.threads.state(pending.thread_id)?.lifecycle
                != RuntimeThreadLifecycleState::Running
            || !pending
                .dialog
                .buttons
                .iter()
                .any(|button| button.id == button_id)
        {
            return Err(RuntimeStateError::new(
                "MessageBox completion has the wrong owner or button",
            ));
        }
        if let Some((handle, id)) = pending.owner {
            let window = self.user32.window(handle)?;
            if window.id != id
                || window.lifecycle != RuntimeWindowLifecycle::Created
                || self
                    .threads
                    .state(window.creation.owner_thread_id)?
                    .lifecycle
                    != RuntimeThreadLifecycleState::Running
            {
                return Err(RuntimeStateError::new(
                    "MessageBox owner lifetime has ended",
                ));
            }
        }
        Ok(())
    }

    pub(crate) fn commit_message_box_result(&mut self, request_id: u64) {
        let host = self
            .scheduler
            .validate_host_completion(request_id)
            .expect("MessageBox host identity was validated before completion");
        self.pending_message_boxes
            .remove(&request_id)
            .expect("MessageBox was validated before completion");
        self.scheduler.commit_host_completion(host);
    }

    fn shell_folder_output_regions(
        &self,
        output: GuestAddress,
    ) -> Result<Vec<MemoryRegionInfo>, RuntimeStateError> {
        self.memory
            .validate_access(output, 520, MemoryAccess::Write)
            .map_err(RuntimeStateError::from_memory)?;
        let end = u64::from(output.0) + 520;
        Ok(self
            .memory
            .regions()
            .filter(|region| {
                u64::from(region.range.start().0) < end && region.range.end() > u64::from(output.0)
            })
            .collect())
    }

    pub(crate) fn begin_shell_folder(
        &mut self,
        output: GuestAddress,
    ) -> Result<RuntimeHostRequest, RuntimeStateError> {
        let regions = self.shell_folder_output_regions(output)?;
        let plan = self
            .scheduler
            .checked_host_request("browser.opfs-ensure-directory".to_owned())?;
        let request = self.scheduler.publish_host_request(plan);
        self.pending_shell_folders
            .insert(request.request_id, (output, regions));
        Ok(request)
    }

    pub(crate) fn validate_shell_folder_output(
        &self,
        request_id: u64,
        output: GuestAddress,
    ) -> Result<(), RuntimeStateError> {
        let pending = self
            .pending_shell_folders
            .get(&request_id)
            .ok_or_else(|| RuntimeStateError::new("special folder has no pending output"))?;
        if pending.0 != output || pending.1 != self.shell_folder_output_regions(output)? {
            return Err(RuntimeStateError::new(
                "special folder output mapping changed while provisioning",
            ));
        }
        Ok(())
    }

    pub fn begin_host_request(
        &mut self,
        service: impl Into<String>,
    ) -> Result<RuntimeHostRequest, RuntimeStateError> {
        let service = service.into();
        if matches!(
            service.as_str(),
            RUNTIME_WAITABLE_TIMER_PACING_SERVICE
                | RUNTIME_MESSAGE_BOX_SERVICE
                | "browser.opfs-ensure-directory"
        ) {
            return Err(RuntimeStateError::new(
                "timer pacing requests must be created with their authoritative wait registration",
            ));
        }
        let plan = self.scheduler.checked_host_request(service)?;
        Ok(self.scheduler.publish_host_request(plan))
    }

    pub fn complete_host_request(
        &mut self,
        request_id: u64,
    ) -> Result<RuntimeHostCompletion, RuntimeStateError> {
        let plan = self.scheduler.validate_host_completion(request_id)?;
        if plan.service == "browser.opfs-ensure-directory" {
            let output = self
                .pending_shell_folders
                .get(&request_id)
                .ok_or_else(|| RuntimeStateError::new("special folder has no pending output"))?
                .0;
            self.validate_shell_folder_output(request_id, output)?;
            self.pending_shell_folders.remove(&request_id);
        }
        if matches!(
            plan.service.as_str(),
            RUNTIME_WAITABLE_TIMER_PACING_SERVICE
                | RUNTIME_AUDIO_NOTIFICATION_PACING_SERVICE
                | RUNTIME_MESSAGE_BOX_SERVICE
        ) {
            return Err(RuntimeStateError::new(
                "pacing completion must atomically commit its subsystem and scheduler state",
            ));
        }
        Ok(self.scheduler.commit_host_completion(plan))
    }

    pub(crate) fn validate_owned_host_request(
        &self,
        request_id: u64,
        service: &str,
    ) -> Result<(), RuntimeStateError> {
        let completion = self.scheduler.validate_host_completion(request_id)?;
        if completion.thread_id != self.current_thread_id() || completion.service != service {
            return Err(RuntimeStateError::new(
                "host completion does not own the selected thread and service",
            ));
        }
        Ok(())
    }

    pub(crate) fn complete_media_host_with<T>(
        &mut self,
        request_id: u64,
        publish_sample: impl FnOnce(&mut Self) -> Result<T, RuntimeStateError>,
    ) -> Result<T, RuntimeStateError> {
        let request = self.scheduler.validate_host_completion(request_id)?;
        if request.service != "browser.media" || request.thread_id != self.current_thread_id() {
            return Err(RuntimeStateError::new(
                "media completion does not own the selected host wait",
            ));
        }
        // The synchronous closure owns its memory/COM transaction. A failed
        // sample publication leaves both that transaction and this wait intact.
        let result = publish_sample(self)?;
        self.scheduler.commit_consumed_host_completion(request);
        Ok(result)
    }

    pub fn pop_scheduler_event(&mut self) -> Option<RuntimeSchedulerEvent> {
        self.scheduler.pop_event()
    }

    pub fn request_current_thread_exit(&mut self, exit_code: u32) -> Result<(), RuntimeStateError> {
        if !self.process.is_running() {
            return Err(RuntimeStateError::new(
                "thread exit request requires a running process",
            ));
        }
        let thread_id = self.current_thread_id();
        self.threads.validate_exit_request(thread_id)?;
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
            return Err(RuntimeStateError::new(
                "thread exit request requires a running thread and a live scheduler state",
            ));
        }
        let cancelled_wait = self
            .scheduler
            .wait_registration_for_cancellation(thread_id)?;
        let host_request_ids = self.scheduler.host_request_ids_for_thread(thread_id);
        let source_read_cancellation = self.files.pending_read_ids(&host_request_ids);
        self.messages.is_empty(thread_id)?;

        self.messages.clear(thread_id);
        self.user32.release_thread_input(thread_id);
        self.files.cancel_reads(&source_read_cancellation);
        self.pending_opfs_opens
            .retain(|request_id, _| !host_request_ids.contains(request_id));
        self.pending_opfs_writes
            .retain(|request_id, _| !host_request_ids.contains(request_id));
        self.pending_message_boxes
            .retain(|request_id, _| !host_request_ids.contains(request_id));
        self.pending_shell_folders
            .retain(|request_id, _| !host_request_ids.contains(request_id));
        self.scheduler
            .cancel_host_requests(thread_id, &host_request_ids);
        if let Some(registration) = cancelled_wait {
            self.scheduler.commit_remove_wait_registration(registration);
        }
        self.scheduler.commit_remove_events_for_thread(thread_id);
        for frame in &self
            .threads
            .state(thread_id)
            .expect("validated exiting thread")
            .callback_frames
        {
            self.pending_api_callbacks.remove(&frame.id);
        }
        self.threads
            .request_exit(thread_id, exit_code)
            .expect("validated thread exit request became invalid during synchronous cancellation");
        self.scheduler
            .set_validated_state(thread_id, SchedulerThreadState::Terminating { exit_code });
        Ok(())
    }

    pub fn complete_current_thread_exit(&mut self) -> Result<(), RuntimeStateError> {
        if !self.process.is_running() {
            return Err(RuntimeStateError::new(
                "thread exit completion requires a running process",
            ));
        }
        let thread_id = self.current_thread_id();
        let exit_code = self.validate_thread_exit_completion(thread_id)?;
        let tls_addresses = self.image_tls.thread_allocation_ids(thread_id);
        self.address_space_allocator
            .validate_releases(&tls_addresses)?;
        let teb = if tls_addresses.is_empty() {
            None
        } else {
            Some(self.current_thread_teb_address()?)
        };
        let mut transaction = self.memory.begin_transaction();
        self.image_tls
            .release_thread_memory(&mut transaction, thread_id)?;
        if let Some(teb) = teb {
            transaction
                .write(GuestAddress(teb.0 + 0x2c), &0_u32.to_le_bytes())
                .map_err(RuntimeStateError::from_memory)?;
        }
        let (object_id, wait_state) =
            self.objects
                .wait_state_mut_for_identity(&RuntimeObjectKey::Thread {
                    thread_id: thread_id.0,
                })?;
        self.scheduler.wake_object_waiters(
            &self.threads,
            object_id,
            RuntimeWaitState::Signaled { exit_code },
            &[thread_id],
            usize::MAX,
            self.process.timer_time_100ns(),
        )?;
        transaction.commit_transaction();
        *wait_state = RuntimeWaitState::Signaled { exit_code };
        self.image_tls.retire_thread(thread_id);
        self.address_space_allocator
            .remove_validated_allocations(tls_addresses);
        self.publish_thread_exit_completion(thread_id, exit_code);
        Ok(())
    }

    pub fn complete_current_guest_thread_exit(&mut self) -> Result<(), RuntimeStateError> {
        let thread_id = self.current_thread_id();
        let thread = self.threads.state(thread_id)?;
        if thread.guest_bootstrap.is_none() {
            return self.complete_current_thread_exit();
        }
        let stack = thread.stack.as_ref().ok_or_else(|| {
            RuntimeStateError::new(
                "dynamically created guest thread lost its stack authority before exit",
            )
        })?;
        let environment = thread.environment.as_ref().ok_or_else(|| {
            RuntimeStateError::new(
                "dynamically created guest thread lost its TEB authority before exit",
            )
        })?;
        let exit_code = self.validate_thread_exit_completion(thread_id)?;
        let (object_id, wait_state) =
            self.objects
                .wait_state_mut_for_identity(&RuntimeObjectKey::Thread {
                    thread_id: thread_id.0,
                })?;
        let mut address_releases = vec![
            stack.address_allocation_id,
            environment.address_allocation_id,
        ];
        address_releases.extend(self.image_tls.thread_allocation_ids(thread_id));
        self.address_space_allocator
            .validate_releases(&address_releases)?;

        let mut transaction = self.memory.begin_transaction();
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
        self.image_tls
            .release_thread_memory(&mut transaction, thread_id)?;
        self.scheduler.wake_object_waiters(
            &self.threads,
            object_id,
            RuntimeWaitState::Signaled { exit_code },
            &[thread_id],
            usize::MAX,
            self.process.timer_time_100ns(),
        )?;
        transaction.commit_transaction();
        *wait_state = RuntimeWaitState::Signaled { exit_code };
        self.publish_thread_exit_completion(thread_id, exit_code);
        self.address_space_allocator
            .remove_validated_allocations(address_releases);
        self.image_tls.retire_thread(thread_id);
        crate::winmm::retire_timer_thread(self, thread_id)?;
        Ok(())
    }

    fn validate_thread_exit_completion(
        &self,
        thread_id: RuntimeThreadId,
    ) -> Result<u32, RuntimeStateError> {
        let exit_code = self.threads.validate_exit_completion(thread_id)?;
        if self.scheduler_state_for_thread(thread_id)?
            != (SchedulerThreadState::Terminating { exit_code })
            || self.scheduler.has_host_request_for_thread(thread_id)
            || !self.threads.state(thread_id)?.callback_frames.is_empty()
            || !self.messages.is_empty(thread_id)?
            || self.scheduler.has_wait_registration_for_thread(thread_id)
            || self.scheduler.has_event_for_thread(thread_id)
        {
            return Err(RuntimeStateError::new(
                "thread exit completion requires terminating scheduler state and no retained work",
            ));
        }
        Ok(exit_code)
    }

    fn publish_thread_exit_completion(&mut self, thread_id: RuntimeThreadId, exit_code: u32) {
        self.threads
            .complete_exit(thread_id)
            .expect("validated thread exit completion became invalid during synchronous cleanup");
        self.scheduler
            .set_validated_state(thread_id, SchedulerThreadState::Terminated { exit_code });
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
        return Err(RuntimeStateError::new(
            "logical HDC does not reference the current window identity and owner thread",
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
        return Err(RuntimeStateError::new(
            "logical HDC does not reference the current graphics surface and owner thread",
        ));
    }
    Ok(())
}

fn registered_initial_thread_stack(
    thread_id: RuntimeThreadId,
    stack: &InitialThreadStack,
    address_allocation_id: ProcessAddressAllocationId,
) -> RuntimeThreadStack {
    RuntimeThreadStack {
        thread_id,
        address_allocation_id,
        memory_owner: stack.owner,
        reserved_range: stack.stack_range,
        committed_range: stack.committed_range,
        initial_commit_bytes: stack.committed_range.byte_length(),
        guard_page: stack.guard_page,
        stack_top: GuestAddress(
            stack
                .initial_esp
                .0
                .checked_add(4)
                .expect("validated initial stack top"),
        ),
        initial_stack_pointer: stack.initial_esp,
        return_sentinel: stack.entry_return_address,
    }
}

fn validate_initial_thread_stack(stack: &InitialThreadStack) -> Result<(), RuntimeStateError> {
    let page_size = GUEST_PAGE_SIZE as u32;
    if !stack.stack_range.start().0.is_multiple_of(page_size)
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
        || stack.stack_range.start().0 > stack.committed_range.start().0
        || stack.committed_range.end() != stack.stack_range.end()
        || stack.initial_esp.0.checked_add(4).map(u64::from) != Some(stack.stack_range.end())
    {
        return Err(RuntimeStateError::new(
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
                            RuntimeStateError::new(
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
        return Err(RuntimeStateError::new(
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
        || allocation.release_policy != ProcessAddressAllocationReleasePolicy::Explicit
        || allocation.executable
        || allocation.guard_pages != 1
    {
        return Err(RuntimeStateError::new(
            "initial thread stack is not backed by its thread-lifetime process allocation record",
        ));
    }
    Ok(())
}

#[derive(Debug)]
#[cfg_attr(test, derive(Clone, PartialEq, Eq))]
pub struct RuntimeStateError {
    pub message: String,
}

impl RuntimeStateError {
    pub(crate) fn from_memory(error: GuestMemoryError) -> Self {
        Self::new(error.message)
    }

    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for RuntimeStateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for RuntimeStateError {}

#[cfg(test)]
#[path = "../runtime_guest_thread_tests.rs"]
mod guest_thread_tests;

#[cfg(test)]
#[path = "../runtime_task_memory_tests.rs"]
mod task_memory_tests;

#[cfg(test)]
mod tests {
    use super::*;
    include!("../d3d9_texture_discard_tests.rs");
    include!("../d3d9_device_creation_tests.rs");
    include!("../dinput_keyboard_tests.rs");
    include!("../dsound_legacy_tests.rs");
    include!("../d3d9_resource_capacity_tests.rs");
    use crate::com::{COM_S_OK, ComCallTrap, ComDispatchOutcome};
    use crate::d3d9::{
        D3D9_CREATE_HARDWARE_VERTEX_PROCESSING_MULTITHREADED, D3D9_D3DCLEAR_TARGET,
        D3D9_D3DDEVTYPE_HAL, D3D9_D3DERR_INVALIDCALL, D3D9_D3DERR_NOTAVAILABLE,
        D3D9_D3DFMT_UNKNOWN, D3D9_D3DFMT_X8R8G8B8, D3D9_D3DMULTISAMPLE_NONE, D3D9_D3DPOOL_MANAGED,
        D3D9_D3DPOOL_SYSTEMMEM, D3D9_D3DSWAPEFFECT_DISCARD, D3D9_D3DUSAGE_WRITEONLY,
        D3D9_IDIRECT3D9_IID, direct3d9_object_spec,
    };
    use crate::dsound::{
        DSOUND_DSBPLAY_LOOPING, DSOUND_DSBUFFERDESC_BYTES, DSOUND_DSSCL_PRIORITY,
        DSOUND_E_INVALIDARG, DSOUND_IDIRECTSOUND8_IID, DSOUND_IDIRECTSOUNDBUFFER8_IID,
        DSOUND_PRIMARY_BUFFER_FLAGS, DSOUND_SECONDARY_BUFFER_FORWARD_FLAGS,
        direct_sound8_object_spec,
    };
    use crate::memory::{MemoryLimits, MemoryOwnerKind};
    use crate::runtime::{
        RUNTIME_AUDIO_RESOURCE_ARENA_BASE, RuntimeAudioBufferKind, RuntimeAudioBufferSegment,
        RuntimeAudioPlaybackState, RuntimeWindowClassModule, RuntimeWindowClassReference,
        RuntimeWindowGeometry, RuntimeWindowNameA,
    };
    use crate::{RuntimeRect, Rva};

    const D3D9_FIXTURE_MEMORY_BASE: GuestAddress = GuestAddress(0x2000_0000);
    const D3D9_FIXTURE_STACK: GuestAddress = GuestAddress(D3D9_FIXTURE_MEMORY_BASE.0 + 0x100);
    const D3D9_FIXTURE_PRESENTATION: GuestAddress =
        GuestAddress(D3D9_FIXTURE_MEMORY_BASE.0 + 0x800);
    const D3D9_FIXTURE_OUTPUT: GuestAddress = GuestAddress(D3D9_FIXTURE_MEMORY_BASE.0 + 0x900);
    const D3D9_FIXTURE_TEXTURE_OUTPUT: GuestAddress =
        GuestAddress(D3D9_FIXTURE_MEMORY_BASE.0 + 0x904);
    const D3D9_FIXTURE_SURFACE_OUTPUT: GuestAddress =
        GuestAddress(D3D9_FIXTURE_MEMORY_BASE.0 + 0x908);
    const D3D9_FIXTURE_CALL_SITE: GuestAddress = GuestAddress(0x1005_aca5);

    #[test]
    fn parked_cpu_snapshots_follow_live_threads_without_changing_scheduler_or_guest_memory() {
        let mut runtime = d3d9_fixture_runtime(d3d9_fixture_memory());
        let initial = runtime.current_thread_id();
        let second = RuntimeThreadId(initial.0 + 1);
        runtime.register_thread(second, 0x22).unwrap();
        let first_bytes = vec![0xa5; 512];
        let second_bytes = vec![0x5a; 512];
        let memory_before = runtime.memory.stats();
        let clock_before = runtime.clock().clone();
        assert!(runtime.thread_cpu_snapshot(initial).is_err());
        assert_eq!(
            runtime.put_thread_cpu_snapshot(initial, 0, 2, 2, &first_bytes),
            Ok(1)
        );
        assert_eq!(
            runtime.put_thread_cpu_snapshot(second, 0, 2, 2, &second_bytes),
            Ok(1)
        );
        let waiting = runtime.begin_host_request("fixture.parked-cpu").unwrap();
        runtime.select_thread(second).unwrap();
        // A deferred completion may patch another live owner's parked context;
        // the CPU handoff revision does not select that owner as the guest thread.
        let scheduler_before = runtime.scheduler.clone();
        assert_eq!(runtime.thread_cpu_snapshot(initial).unwrap().3, first_bytes);
        assert_eq!(
            runtime.put_thread_cpu_snapshot(initial, 1, 2, 2, &second_bytes),
            Ok(2)
        );
        assert_eq!(runtime.current_thread_id(), second);
        assert_eq!(runtime.scheduler, scheduler_before);
        assert_eq!(runtime.current_last_error().unwrap(), 0x22);
        assert_eq!(runtime.memory.stats(), memory_before);
        assert_eq!(*runtime.clock(), clock_before);
        assert!(
            runtime
                .put_thread_cpu_snapshot(initial, 1, 2, 2, &first_bytes)
                .is_err()
        );
        assert_eq!(
            runtime.thread_cpu_snapshot(initial).unwrap().3,
            second_bytes
        );
        assert!(runtime.thread_cpu_snapshot(RuntimeThreadId(0)).is_err());
        assert!(
            runtime
                .put_thread_cpu_snapshot(RuntimeThreadId(0), 0, 2, 2, &first_bytes)
                .is_err()
        );
        runtime.request_current_thread_exit(0).unwrap();
        assert!(runtime.thread_cpu_snapshot(second).is_err());
        assert!(
            runtime
                .put_thread_cpu_snapshot(second, 1, 2, 2, &first_bytes)
                .is_err()
        );
        runtime.complete_current_thread_exit().unwrap();
        assert!(runtime.thread_cpu_snapshot(second).is_err());
        runtime.select_thread(initial).unwrap();
        runtime.complete_host_request(waiting.request_id).unwrap();
        assert_eq!(runtime.thread_cpu_snapshot(initial).unwrap().2, 2);
        assert_eq!(runtime.current_last_error().unwrap(), 0x1234_5678);
    }

    #[test]
    fn timestamp_counter_is_process_wide_and_rejects_inactive_thread_reads() {
        let mut runtime = d3d9_fixture_runtime(d3d9_fixture_memory());
        let initial = runtime.current_thread_id();
        let second = RuntimeThreadId(initial.0 + 1);
        runtime.register_thread(second, 0x22).unwrap();
        let memory_before = runtime.memory.stats();
        assert_eq!(runtime.read_timestamp_counter(0, 10).unwrap(), 1000);
        runtime.select_thread(second).unwrap();
        assert_eq!(runtime.read_timestamp_counter(1, 20).unwrap(), 2000);
        assert_eq!(runtime.current_last_error().unwrap(), 0x22);
        runtime.select_thread(initial).unwrap();
        assert_eq!(runtime.read_timestamp_counter(2, 20).unwrap(), 2000);
        assert_eq!(runtime.current_last_error().unwrap(), 0x1234_5678);
        let before = runtime.clock().clone();
        assert!(runtime.read_timestamp_counter(3, 19).is_err());
        assert_eq!(*runtime.clock(), before);
        assert_eq!(runtime.synchronized_instruction_count(), 2);
        let waiting = runtime
            .begin_host_request("fixture.timestamp-wait")
            .unwrap();
        assert!(runtime.read_timestamp_counter(3, 30).is_err());
        assert_eq!(*runtime.clock(), before);
        assert_eq!(runtime.synchronized_instruction_count(), 2);
        runtime.complete_host_request(waiting.request_id).unwrap();
        assert_eq!(runtime.read_timestamp_counter(3, 30).unwrap(), 3000);
        runtime.select_thread(second).unwrap();
        runtime.request_current_thread_exit(0).unwrap();
        runtime.complete_current_thread_exit().unwrap();
        assert!(runtime.read_timestamp_counter(4, 40).is_err());
        runtime.select_thread(initial).unwrap();
        assert_eq!(runtime.read_timestamp_counter(4, 40).unwrap(), 4000);
        assert_eq!(runtime.memory.stats(), memory_before);
    }

    #[test]
    fn diagnostic_location_reads_live_com_without_acknowledging_or_retaining_it() {
        let mut runtime = d3d9_fixture_runtime(d3d9_fixture_memory());
        let installed = runtime
            .install_com_object_with_guest_writes(None, direct3d9_object_spec(), |_| Ok(Vec::new()))
            .unwrap();
        let interface = runtime
            .com_interface_for_object_iid(installed.object_id, D3D9_IDIRECT3D9_IID)
            .unwrap()
            .clone();
        let endpoint = interface.methods[&2].endpoint;
        let before = runtime.memory.stats();
        assert!(runtime.memory.has_pending_delta());
        assert_eq!(
            runtime.diagnostic_location(endpoint),
            (None, Some("IDirect3D9::Release".to_owned()))
        );
        assert_eq!(
            runtime.diagnostic_location(GuestAddress(endpoint.0 + 1)),
            (None, None)
        );
        assert_eq!(runtime.memory.stats(), before);
        assert_eq!(
            runtime
                .com
                .object(installed.object_id)
                .unwrap()
                .reference_count,
            1
        );
        assert_eq!(runtime.current_last_error().unwrap(), 0x1234_5678);
        assert_eq!(
            runtime.scheduler_state().unwrap(),
            SchedulerThreadState::Runnable
        );
        acknowledge_d3d9_fixture_memory(&mut runtime.memory);
        assert_eq!(
            dispatch_d3d9_fixture_release(&mut runtime, &interface, interface.guest_pointer).eax,
            0
        );
        assert_eq!(runtime.diagnostic_location(endpoint), (None, None));

        let installed = runtime
            .install_com_object_with_guest_writes(None, direct3d9_object_spec(), |_| Ok(Vec::new()))
            .unwrap();
        let interface = runtime
            .com_interface_for_object_iid(installed.object_id, D3D9_IDIRECT3D9_IID)
            .unwrap();
        let endpoint = interface.methods[&2].endpoint;
        let backing = runtime
            .com
            .object(installed.object_id)
            .unwrap()
            .backing
            .clone();
        runtime
            .memory
            .release_region(backing.memory_owner, backing.range.base)
            .unwrap();
        let before = runtime.memory.stats();
        assert_eq!(runtime.diagnostic_location(endpoint), (None, None));
        assert_eq!(runtime.memory.stats(), before);
    }

    #[test]
    fn diagnostic_location_names_only_live_image_and_exact_export() {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../.work/toolchain-spike/vine_fixture.dll");
        let limits = crate::Pe32Limits::default();
        let snapshot =
            crate::ModuleSnapshot::from_bytes(std::fs::read(path).unwrap(), &limits).unwrap();
        let loader = crate::build_pe32_load_plan(&snapshot, limits).unwrap();
        let base = GuestAddress(loader.image.preferred_image_base);
        let mut runtime = d3d9_fixture_runtime(GuestMemory::new(MemoryLimits::default()).unwrap());
        let image =
            crate::materialize_pe32_image(&mut runtime.memory, &snapshot, &loader, base).unwrap();
        runtime.memory.seal_owner(image.layout.owner).unwrap();
        let handle = 0x4d01_0001;
        runtime
            .load_module(RuntimeModule {
                handle,
                requested_name: "VINE_FIXTURE.DLL".to_owned(),
                canonical_name: "vine_fixture.dll".to_owned(),
                runtime_system_module: false,
            })
            .unwrap();
        runtime
            .register_module_image(handle, &loader, &image)
            .unwrap();
        let export = GuestAddress(base.0 + 0x1000);
        let before = runtime.memory.stats();
        assert!(runtime.memory.has_pending_delta());
        assert_eq!(
            runtime.diagnostic_location(export),
            (
                Some("vine_fixture.dll".to_owned()),
                Some("fixture_add".to_owned())
            )
        );
        assert_eq!(
            runtime.diagnostic_location(GuestAddress(export.0 + 1)),
            (Some("vine_fixture.dll".to_owned()), None)
        );
        assert_eq!(runtime.diagnostic_location(GuestAddress(0)), (None, None));
        assert_eq!(runtime.memory.stats(), before);
        assert_eq!(runtime.current_last_error().unwrap(), 0x1234_5678);
        runtime.objects = RuntimeObjectTable::default();
        assert_eq!(runtime.diagnostic_location(export), (None, None));
        runtime
            .memory
            .release_region(image.layout.owner, image.layout.image_range.start())
            .unwrap();
        let before = runtime.memory.stats();
        assert_eq!(runtime.diagnostic_location(export), (None, None));
        assert_eq!(runtime.memory.stats(), before);
    }

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
        callback_scratch_for_depth(65, 1).unwrap_err();
    }

    #[test]
    fn audio_notification_wait_payload_retains_captured_handles() {
        let handles = [
            RuntimeHandle::from_parts(RuntimeObjectKind::Event, 1, 1).unwrap(),
            RuntimeHandle::from_parts(RuntimeObjectKind::Event, 1, 2).unwrap(),
        ];
        let inactive = encode_inactive_audio_notification_wait_payload(&handles, 100, 0);
        assert_eq!(
            decode_audio_notification_wait_handles(&inactive, 100).unwrap(),
            handles
        );

        let active_words = [1, 25, 7, 0, 100, handles.len() as u32];
        let mut active = Vec::new();
        for word in active_words
            .into_iter()
            .chain(handles.iter().map(|handle| handle.0))
        {
            active.extend_from_slice(&word.to_le_bytes());
        }
        active.extend_from_slice(&0u64.to_le_bytes());
        assert_eq!(
            decode_audio_notification_wait_handles(&active, 100).unwrap(),
            handles
        );

        for payload in [&active, &inactive] {
            for length in 0..payload.len() {
                assert!(decode_audio_notification_wait_handles(&payload[..length], 100).is_err());
            }
            assert!(decode_audio_notification_wait_handles(payload, 99).is_err());
        }
        let max_handles = vec![handles[0]; RUNTIME_AUDIO_NOTIFICATION_LIMIT];
        let max_payload = encode_inactive_audio_notification_wait_payload(&max_handles, 100, 0);
        assert_eq!(
            decode_audio_notification_wait_handles(&max_payload, 100).unwrap(),
            max_handles
        );
        let oversized = vec![0; (7 + RUNTIME_AUDIO_NOTIFICATION_LIMIT) * 4];
        assert!(decode_audio_notification_wait_handles(&oversized, 100).is_err());
        active[5 * 4..6 * 4].copy_from_slice(&3u32.to_le_bytes());
        decode_audio_notification_wait_handles(&active, 100).unwrap_err();
    }

    #[test]
    fn direct_sound_secondary_lock_unlock_play_commits_memory_audio_and_com_lifetime() {
        let reject_unready_play =
            |runtime: &mut Runtime, interface: &RuntimeComInterfaceRecord, flags: u32| {
                write_d3d9_fixture_words(
                    &mut runtime.memory,
                    D3D9_FIXTURE_STACK,
                    &[
                        D3D9_FIXTURE_CALL_SITE.0,
                        interface.guest_pointer.0,
                        0,
                        0,
                        flags,
                    ],
                );
                acknowledge_d3d9_fixture_memory(&mut runtime.memory);
                let audio_before = runtime.audio.clone();
                let memory_before = runtime.memory.stats();
                let allocations_before = runtime.address_space_allocator.clone();
                let result = runtime.dispatch_com_call(ComCallTrap {
                    endpoint_address: interface.methods[&12].endpoint,
                    stack_pointer: D3D9_FIXTURE_STACK,
                    return_address: D3D9_FIXTURE_CALL_SITE,
                    call_site: D3D9_FIXTURE_CALL_SITE,
                });
                let ComDispatchOutcome::Completed(completion) = result else {
                    panic!("unready Play did not return HRESULT: {result:?}");
                };
                assert_eq!(completion.eax, 0x8878_0032);
                assert_eq!(runtime.audio, audio_before);
                assert_eq!(runtime.memory.stats(), memory_before);
                assert_eq!(runtime.address_space_allocator, allocations_before);
                assert_eq!(runtime.current_last_error().unwrap(), 0x1234_5678);
            };
        for wait_before_play in [false, true] {
            const SECONDARY_BYTES: u32 = 16_760;

            let memory = d3d9_fixture_memory();
            let mut runtime = d3d9_fixture_runtime(memory);
            let window = seed_created_d3d9_window(&mut runtime);

            let installed = runtime
                .install_com_object_with_guest_writes(None, direct_sound8_object_spec(), |_| {
                    Ok(Vec::new())
                })
                .expect("install DirectSound root and logical audio device");
            let device_id = runtime
                .audio_devices()
                .values()
                .find(|device| device.com_object_id == installed.object_id)
                .expect("live DirectSound audio device")
                .id;
            let interface = runtime
                .com()
                .interface_for_object_iid(installed.object_id, DSOUND_IDIRECTSOUND8_IID)
                .expect("IDirectSound8 interface")
                .clone();
            acknowledge_d3d9_fixture_memory(&mut runtime.memory);
            write_d3d9_fixture_words(
                &mut runtime.memory,
                D3D9_FIXTURE_STACK,
                &[
                    D3D9_FIXTURE_CALL_SITE.0,
                    interface.guest_pointer.0,
                    window.handle,
                    DSOUND_DSSCL_PRIORITY,
                ],
            );
            acknowledge_d3d9_fixture_memory(&mut runtime.memory);

            let completion = {
                match runtime.dispatch_com_call(ComCallTrap {
                    endpoint_address: interface.methods[&6].endpoint,
                    stack_pointer: D3D9_FIXTURE_STACK,
                    return_address: D3D9_FIXTURE_CALL_SITE,
                    call_site: D3D9_FIXTURE_CALL_SITE,
                }) {
                    ComDispatchOutcome::Completed(completion) => completion,
                    other => panic!("SetCooperativeLevel did not complete: {other:?}"),
                }
            };
            assert_eq!(completion.eax, COM_S_OK.bits());
            assert_eq!(
                completion.stack_pointer,
                GuestAddress(D3D9_FIXTURE_STACK.0 + 16)
            );
            let device = runtime
                .audio_device(device_id)
                .expect("logical audio device");
            let cooperative = device
                .cooperative
                .expect("committed DirectSound cooperative state");
            assert_eq!(cooperative.window_id, window.id);
            assert_eq!(cooperative.window_handle, window.handle);
            assert_eq!(cooperative.owner_thread_id, RuntimeThreadId(0x2000));

            let pcm_words = [0x0002_0001, 44_100, 176_400, 0x0010_0004];
            let format_address = GuestAddress(D3D9_FIXTURE_PRESENTATION.0 + 0x40);
            let iid_address = GuestAddress(D3D9_FIXTURE_PRESENTATION.0 + 0x60);
            write_d3d9_fixture_words(&mut runtime.memory, format_address, &pcm_words);
            write_d3d9_fixture_words(
                &mut runtime.memory,
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
            write_d3d9_fixture_words(&mut runtime.memory, D3D9_FIXTURE_OUTPUT, &[0xcccc_cccc]);
            write_d3d9_fixture_words(
                &mut runtime.memory,
                D3D9_FIXTURE_STACK,
                &[
                    D3D9_FIXTURE_CALL_SITE.0,
                    interface.guest_pointer.0,
                    D3D9_FIXTURE_PRESENTATION.0,
                    D3D9_FIXTURE_OUTPUT.0,
                    0,
                ],
            );
            acknowledge_d3d9_fixture_memory(&mut runtime.memory);
            let create_primary = {
                match runtime.dispatch_com_call(ComCallTrap {
                    endpoint_address: interface.methods[&3].endpoint,
                    stack_pointer: D3D9_FIXTURE_STACK,
                    return_address: D3D9_FIXTURE_CALL_SITE,
                    call_site: D3D9_FIXTURE_CALL_SITE,
                }) {
                    ComDispatchOutcome::Completed(completion) => completion,
                    other => panic!("primary CreateSoundBuffer did not complete: {other:?}"),
                }
            };
            assert_eq!(create_primary.eax, COM_S_OK.bits());

            acknowledge_d3d9_fixture_memory(&mut runtime.memory);
            let primary_pointer = u32::from_le_bytes(
                runtime
                    .memory
                    .read(D3D9_FIXTURE_OUTPUT, 4)
                    .unwrap()
                    .try_into()
                    .unwrap(),
            );
            let primary_interface = runtime
                .com_interface_for_pointer(GuestAddress(primary_pointer))
                .unwrap()
                .expect("primary IDirectSoundBuffer8 output")
                .clone();
            assert_eq!(primary_interface.iid, DSOUND_IDIRECTSOUNDBUFFER8_IID);
            let primary = runtime
                .audio_buffer_for_com_object(primary_interface.object_id)
                .expect("primary logical audio buffer");
            assert_eq!(primary.descriptor.kind, RuntimeAudioBufferKind::Primary);
            assert_eq!(primary.active_format, None);
            reject_unready_play(&mut runtime, &primary_interface, DSOUND_DSBPLAY_LOOPING);

            write_d3d9_fixture_words(
                &mut runtime.memory,
                D3D9_FIXTURE_STACK,
                &[
                    D3D9_FIXTURE_CALL_SITE.0,
                    primary_interface.guest_pointer.0,
                    format_address.0,
                ],
            );
            acknowledge_d3d9_fixture_memory(&mut runtime.memory);
            let set_format = {
                match runtime.dispatch_com_call(ComCallTrap {
                    endpoint_address: primary_interface.methods[&14].endpoint,
                    stack_pointer: D3D9_FIXTURE_STACK,
                    return_address: D3D9_FIXTURE_CALL_SITE,
                    call_site: D3D9_FIXTURE_CALL_SITE,
                }) {
                    ComDispatchOutcome::Completed(completion) => completion,
                    other => panic!("primary SetFormat did not complete: {other:?}"),
                }
            };
            assert_eq!(set_format.eax, COM_S_OK.bits());

            write_d3d9_fixture_words(
                &mut runtime.memory,
                D3D9_FIXTURE_STACK,
                &[
                    D3D9_FIXTURE_CALL_SITE.0,
                    primary_interface.guest_pointer.0,
                    0,
                    0,
                    DSOUND_DSBPLAY_LOOPING,
                ],
            );
            acknowledge_d3d9_fixture_memory(&mut runtime.memory);
            let play = {
                match runtime.dispatch_com_call(ComCallTrap {
                    endpoint_address: primary_interface.methods[&12].endpoint,
                    stack_pointer: D3D9_FIXTURE_STACK,
                    return_address: D3D9_FIXTURE_CALL_SITE,
                    call_site: D3D9_FIXTURE_CALL_SITE,
                }) {
                    ComDispatchOutcome::Completed(completion) => completion,
                    other => panic!("primary Play did not complete: {other:?}"),
                }
            };
            assert_eq!(play.eax, COM_S_OK.bits());
            let primary = runtime
                .audio_buffer_for_com_object(primary_interface.object_id)
                .expect("playing primary logical audio buffer");
            assert_eq!(
                primary.active_format,
                Some(super::super::RuntimeAudioPcmFormat {
                    channels: 2,
                    samples_per_second: 44_100,
                    average_bytes_per_second: 176_400,
                    block_align: 4,
                    bits_per_sample: 16,
                })
            );
            assert_eq!(primary.playback, RuntimeAudioPlaybackState::PlayingLooping);

            write_d3d9_fixture_words(
                &mut runtime.memory,
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
            write_d3d9_fixture_words(
                &mut runtime.memory,
                D3D9_FIXTURE_TEXTURE_OUTPUT,
                &[0xcccc_cccc],
            );
            write_d3d9_fixture_words(
                &mut runtime.memory,
                D3D9_FIXTURE_STACK,
                &[
                    D3D9_FIXTURE_CALL_SITE.0,
                    interface.guest_pointer.0,
                    D3D9_FIXTURE_PRESENTATION.0,
                    D3D9_FIXTURE_TEXTURE_OUTPUT.0,
                    0,
                ],
            );
            acknowledge_d3d9_fixture_memory(&mut runtime.memory);
            let create_secondary = {
                match runtime.dispatch_com_call(ComCallTrap {
                    endpoint_address: interface.methods[&3].endpoint,
                    stack_pointer: D3D9_FIXTURE_STACK,
                    return_address: D3D9_FIXTURE_CALL_SITE,
                    call_site: D3D9_FIXTURE_CALL_SITE,
                }) {
                    ComDispatchOutcome::Completed(completion) => completion,
                    other => panic!("secondary CreateSoundBuffer did not complete: {other:?}"),
                }
            };
            assert_eq!(create_secondary.eax, COM_S_OK.bits());
            acknowledge_d3d9_fixture_memory(&mut runtime.memory);
            let secondary_pointer = u32::from_le_bytes(
                runtime
                    .memory
                    .read(D3D9_FIXTURE_TEXTURE_OUTPUT, 4)
                    .unwrap()
                    .try_into()
                    .unwrap(),
            );
            let secondary_interface = runtime
                .com_interface_for_pointer(GuestAddress(secondary_pointer))
                .unwrap()
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
            write_d3d9_fixture_words(&mut runtime.memory, iid_address, &iid_words);
            write_d3d9_fixture_words(
                &mut runtime.memory,
                D3D9_FIXTURE_SURFACE_OUTPUT,
                &[0xcccc_cccc],
            );
            write_d3d9_fixture_words(
                &mut runtime.memory,
                D3D9_FIXTURE_STACK,
                &[
                    D3D9_FIXTURE_CALL_SITE.0,
                    secondary_interface.guest_pointer.0,
                    iid_address.0,
                    D3D9_FIXTURE_SURFACE_OUTPUT.0,
                ],
            );
            acknowledge_d3d9_fixture_memory(&mut runtime.memory);
            let query = {
                runtime.dispatch_com_call(ComCallTrap {
                    endpoint_address: secondary_interface.methods[&0].endpoint,
                    stack_pointer: D3D9_FIXTURE_STACK,
                    return_address: D3D9_FIXTURE_CALL_SITE,
                    call_site: D3D9_FIXTURE_CALL_SITE,
                })
            };
            let ComDispatchOutcome::Completed(query) = query else {
                panic!("IDirectSoundBuffer8 QueryInterface did not complete: {query:?}");
            };
            assert_eq!(query.eax, COM_S_OK.bits());
            acknowledge_d3d9_fixture_memory(&mut runtime.memory);
            assert_eq!(
                runtime.memory.read(D3D9_FIXTURE_SURFACE_OUTPUT, 4).unwrap(),
                secondary_pointer.to_le_bytes()
            );
            let release = dispatch_d3d9_fixture_release(
                &mut runtime,
                &secondary_interface,
                secondary_interface.guest_pointer,
            );
            assert_eq!(release.eax, 1);

            reject_unready_play(&mut runtime, &secondary_interface, 0);

            // Rejected guest output must not publish backing, allocation, or audio state.
            let buffer_before = runtime
                .audio_buffer_for_com_object(secondary_interface.object_id)
                .unwrap()
                .clone();
            let allocations_before = runtime.address_space_allocator.clone();
            let memory_before = runtime.memory.stats();
            assert!(
                runtime
                    .lock_audio_buffer(
                        secondary_interface.object_id,
                        0,
                        SECONDARY_BYTES,
                        0,
                        [GuestAddress(0); 4],
                    )
                    .is_err()
            );
            assert_eq!(
                runtime
                    .audio_buffer_for_com_object(secondary_interface.object_id)
                    .unwrap(),
                &buffer_before
            );
            assert_eq!(runtime.address_space_allocator, allocations_before);
            assert_eq!(
                runtime.com.objects()[&secondary_interface.object_id].reference_count,
                1
            );
            assert_eq!(runtime.current_last_error().unwrap(), 0x1234_5678);
            assert_eq!(runtime.memory.stats(), memory_before);

            for (offset, length, flags, expected_hresult) in [
                (SECONDARY_BYTES, 4, 0, Some(DSOUND_E_INVALIDARG.bits())),
                (0, 0, 0, Some(DSOUND_E_INVALIDARG.bits())),
                (0, SECONDARY_BYTES + 4, 0, Some(DSOUND_E_INVALIDARG.bits())),
                (1, 4, 0, Some(DSOUND_E_INVALIDARG.bits())),
                (0, 3, 0, Some(DSOUND_E_INVALIDARG.bits())),
                (0, 4, 4, Some(DSOUND_E_INVALIDARG.bits())),
                (0, 4, 1, None),
                (0, 4, 2, None),
            ] {
                write_d3d9_fixture_words(
                    &mut runtime.memory,
                    D3D9_FIXTURE_OUTPUT,
                    &[0xcccc_cccc; 4],
                );
                write_d3d9_fixture_words(
                    &mut runtime.memory,
                    D3D9_FIXTURE_STACK,
                    &[
                        D3D9_FIXTURE_CALL_SITE.0,
                        secondary_interface.guest_pointer.0,
                        offset,
                        length,
                        D3D9_FIXTURE_OUTPUT.0,
                        D3D9_FIXTURE_OUTPUT.0 + 4,
                        D3D9_FIXTURE_OUTPUT.0 + 8,
                        D3D9_FIXTURE_OUTPUT.0 + 12,
                        flags,
                    ],
                );
                acknowledge_d3d9_fixture_memory(&mut runtime.memory);
                let memory_before = runtime.memory.stats();
                let rejected = runtime.dispatch_com_call(ComCallTrap {
                    endpoint_address: secondary_interface.methods[&11].endpoint,
                    stack_pointer: D3D9_FIXTURE_STACK,
                    return_address: D3D9_FIXTURE_CALL_SITE,
                    call_site: D3D9_FIXTURE_CALL_SITE,
                });
                if let Some(hresult) = expected_hresult {
                    let ComDispatchOutcome::Completed(completion) = rejected else {
                        panic!("invalid Lock stopped instead of returning HRESULT: {rejected:?}");
                    };
                    assert_eq!(completion.eax, hresult);
                    assert_eq!(completion.instruction_pointer, D3D9_FIXTURE_CALL_SITE);
                    assert_eq!(
                        completion.stack_pointer,
                        GuestAddress(D3D9_FIXTURE_STACK.0 + 36)
                    );
                } else {
                    assert!(matches!(rejected, ComDispatchOutcome::Fault(_)));
                }
                assert_eq!(
                    runtime.memory.read(D3D9_FIXTURE_OUTPUT, 16).unwrap(),
                    vec![0xcc; 16]
                );
                assert_eq!(runtime.memory.stats(), memory_before);
                assert_eq!(
                    runtime
                        .audio_buffer_for_com_object(secondary_interface.object_id)
                        .unwrap(),
                    &buffer_before
                );
                assert_eq!(runtime.address_space_allocator, allocations_before);
                assert_eq!(
                    runtime.com.objects()[&secondary_interface.object_id].reference_count,
                    1
                );
                assert_eq!(runtime.current_last_error().unwrap(), 0x1234_5678);
            }
            write_d3d9_fixture_words(
                &mut runtime.memory,
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
            acknowledge_d3d9_fixture_memory(&mut runtime.memory);
            let lock = {
                runtime.dispatch_com_call(ComCallTrap {
                    endpoint_address: secondary_interface.methods[&11].endpoint,
                    stack_pointer: D3D9_FIXTURE_STACK,
                    return_address: D3D9_FIXTURE_CALL_SITE,
                    call_site: D3D9_FIXTURE_CALL_SITE,
                })
            };
            let ComDispatchOutcome::Completed(lock) = lock else {
                panic!("Lock did not complete: {lock:?}");
            };
            assert_eq!(lock.eax, COM_S_OK.bits());

            acknowledge_d3d9_fixture_memory(&mut runtime.memory);
            let data1 = u32::from_le_bytes(
                runtime
                    .memory
                    .read(D3D9_FIXTURE_OUTPUT, 4)
                    .unwrap()
                    .try_into()
                    .unwrap(),
            );
            let bytes1 = u32::from_le_bytes(
                runtime
                    .memory
                    .read(GuestAddress(D3D9_FIXTURE_OUTPUT.0 + 4), 4)
                    .unwrap()
                    .try_into()
                    .unwrap(),
            );
            let data2 = u32::from_le_bytes(
                runtime
                    .memory
                    .read(GuestAddress(D3D9_FIXTURE_OUTPUT.0 + 8), 4)
                    .unwrap()
                    .try_into()
                    .unwrap(),
            );
            let bytes2 = u32::from_le_bytes(
                runtime
                    .memory
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
                locked_secondary.active_locks,
                vec![RuntimeAudioBufferLock {
                    first_segment: RuntimeAudioBufferSegment {
                        guest_address: GuestAddress(data1),
                        byte_length: SECONDARY_BYTES,
                    },
                    second_segment: None,
                }]
            );
            assert!(!locked_secondary.initialized);
            reject_unready_play(&mut runtime, &secondary_interface, 0);
            assert!(
                runtime
                    .process_address_space_allocator()
                    .allocation(backing.address_allocation_id)
                    .is_ok()
            );

            write_d3d9_fixture_words(
                &mut runtime.memory,
                D3D9_FIXTURE_OUTPUT,
                &[0xcccc_cccc, 0xcccc_cccc, 0xcccc_cccc, 0xcccc_cccc],
            );
            write_d3d9_fixture_words(
                &mut runtime.memory,
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
            acknowledge_d3d9_fixture_memory(&mut runtime.memory);
            let buffer_before_double_lock = runtime
                .audio_buffer_for_com_object(secondary_interface.object_id)
                .unwrap()
                .clone();
            let memory_before_double_lock = runtime.memory.stats();
            let double_lock = {
                runtime.dispatch_com_call(ComCallTrap {
                    endpoint_address: secondary_interface.methods[&11].endpoint,
                    stack_pointer: D3D9_FIXTURE_STACK,
                    return_address: D3D9_FIXTURE_CALL_SITE,
                    call_site: D3D9_FIXTURE_CALL_SITE,
                })
            };
            let ComDispatchOutcome::Completed(double_lock) = double_lock else {
                panic!("double Lock did not return HRESULT: {double_lock:?}");
            };
            assert_eq!(double_lock.eax, DSOUND_E_INVALIDARG.bits());

            assert_eq!(
                runtime
                    .audio_buffer_for_com_object(secondary_interface.object_id)
                    .unwrap(),
                &buffer_before_double_lock
            );
            assert_eq!(runtime.memory.stats(), memory_before_double_lock);
            assert_eq!(
                runtime.com.objects()[&secondary_interface.object_id].reference_count,
                1
            );
            assert_eq!(runtime.current_last_error().unwrap(), 0x1234_5678);
            assert_eq!(
                runtime.memory.read(D3D9_FIXTURE_OUTPUT, 16).unwrap(),
                vec![0xcc; 16]
            );

            write_d3d9_fixture_words(
                &mut runtime.memory,
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
            acknowledge_d3d9_fixture_memory(&mut runtime.memory);
            let buffer_before_mismatched_unlock = runtime
                .audio_buffer_for_com_object(secondary_interface.object_id)
                .unwrap()
                .clone();
            let memory_before_mismatched_unlock = runtime.memory.stats();
            let mismatched_unlock = {
                runtime.dispatch_com_call(ComCallTrap {
                    endpoint_address: secondary_interface.methods[&19].endpoint,
                    stack_pointer: D3D9_FIXTURE_STACK,
                    return_address: D3D9_FIXTURE_CALL_SITE,
                    call_site: D3D9_FIXTURE_CALL_SITE,
                })
            };
            let ComDispatchOutcome::Completed(mismatched_unlock) = mismatched_unlock else {
                panic!("mismatched Unlock did not return HRESULT: {mismatched_unlock:?}");
            };
            assert_eq!(mismatched_unlock.eax, DSOUND_E_INVALIDARG.bits());
            assert_eq!(
                mismatched_unlock.instruction_pointer,
                D3D9_FIXTURE_CALL_SITE
            );
            assert_eq!(
                mismatched_unlock.stack_pointer,
                GuestAddress(D3D9_FIXTURE_STACK.0 + 24)
            );
            assert_eq!(
                runtime
                    .audio_buffer_for_com_object(secondary_interface.object_id)
                    .unwrap(),
                &buffer_before_mismatched_unlock
            );
            assert_eq!(runtime.memory.stats(), memory_before_mismatched_unlock);
            assert_eq!(
                runtime.com.objects()[&secondary_interface.object_id].reference_count,
                1
            );
            assert_eq!(runtime.current_last_error().unwrap(), 0x1234_5678);

            runtime
                .memory
                .write(GuestAddress(data1), &vec![0; SECONDARY_BYTES as usize])
                .expect("zero writable secondary sample backing");
            acknowledge_d3d9_fixture_memory(&mut runtime.memory);
            write_d3d9_fixture_words(
                &mut runtime.memory,
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
            acknowledge_d3d9_fixture_memory(&mut runtime.memory);
            let unlock = {
                runtime.dispatch_com_call(ComCallTrap {
                    endpoint_address: secondary_interface.methods[&19].endpoint,
                    stack_pointer: D3D9_FIXTURE_STACK,
                    return_address: D3D9_FIXTURE_CALL_SITE,
                    call_site: D3D9_FIXTURE_CALL_SITE,
                })
            };
            let ComDispatchOutcome::Completed(unlock) = unlock else {
                panic!("Unlock did not complete: {unlock:?}");
            };
            assert_eq!(unlock.eax, COM_S_OK.bits());

            acknowledge_d3d9_fixture_memory(&mut runtime.memory);
            assert!(runtime.memory.write(GuestAddress(data1), &[1]).is_err());
            let unlocked_secondary = runtime
                .audio_buffer_for_com_object(secondary_interface.object_id)
                .expect("unlocked secondary logical audio buffer");
            assert!(unlocked_secondary.active_locks.is_empty());
            assert!(unlocked_secondary.initialized);

            runtime
                .memory
                .protect(
                    backing.memory_owner,
                    backing.range,
                    PageProtection {
                        read: false,
                        write: false,
                        execute: false,
                    },
                )
                .unwrap();
            acknowledge_d3d9_fixture_memory(&mut runtime.memory);
            let audio_before_unreadable_play = runtime.audio.clone();
            let memory_before_unreadable_play = runtime.memory.stats();
            let allocations_before_unreadable_play = runtime.address_space_allocator.clone();
            assert!(
                runtime
                    .set_audio_buffer_play(
                        secondary_interface.object_id,
                        RuntimeAudioPlaybackState::PlayingOnce
                    )
                    .is_err()
            );
            assert_eq!(runtime.audio, audio_before_unreadable_play);
            assert_eq!(runtime.memory.stats(), memory_before_unreadable_play);
            assert_eq!(
                runtime.address_space_allocator,
                allocations_before_unreadable_play
            );
            assert_eq!(runtime.current_last_error().unwrap(), 0x1234_5678);
            runtime
                .memory
                .protect(
                    backing.memory_owner,
                    backing.range,
                    PageProtection {
                        read: true,
                        write: false,
                        execute: false,
                    },
                )
                .unwrap();
            acknowledge_d3d9_fixture_memory(&mut runtime.memory);

            for (slot, value) in [
                (15_u16, (-1_000_i32) as u32),
                (16_u16, (-500_i32) as u32),
                (17_u16, 22_050),
                (13_u16, 4),
            ] {
                write_d3d9_fixture_words(
                    &mut runtime.memory,
                    D3D9_FIXTURE_STACK,
                    &[
                        D3D9_FIXTURE_CALL_SITE.0,
                        secondary_interface.guest_pointer.0,
                        value,
                    ],
                );
                acknowledge_d3d9_fixture_memory(&mut runtime.memory);
                let control = {
                    runtime.dispatch_com_call(ComCallTrap {
                        endpoint_address: secondary_interface.methods[&slot].endpoint,
                        stack_pointer: D3D9_FIXTURE_STACK,
                        return_address: D3D9_FIXTURE_CALL_SITE,
                        call_site: D3D9_FIXTURE_CALL_SITE,
                    })
                };
                let ComDispatchOutcome::Completed(control) = control else {
                    panic!("secondary control slot {slot} did not complete: {control:?}");
                };
                assert_eq!(control.eax, COM_S_OK.bits());
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
                &mut runtime.memory,
                D3D9_FIXTURE_OUTPUT,
                &[0xcccc_cccc, 0xdddd_dddd],
            );
            write_d3d9_fixture_words(
                &mut runtime.memory,
                D3D9_FIXTURE_STACK,
                &[
                    D3D9_FIXTURE_CALL_SITE.0,
                    secondary_interface.guest_pointer.0,
                    D3D9_FIXTURE_OUTPUT.0,
                    D3D9_FIXTURE_OUTPUT.0 + 4,
                ],
            );
            acknowledge_d3d9_fixture_memory(&mut runtime.memory);
            let buffer_before_position_query = runtime
                .audio_buffer_for_com_object(secondary_interface.object_id)
                .unwrap()
                .clone();
            let clock_before_position_query = runtime.clock().clone();
            // A bad second output must not overwrite the valid first output.
            write_d3d9_fixture_words(
                &mut runtime.memory,
                D3D9_FIXTURE_STACK,
                &[
                    D3D9_FIXTURE_CALL_SITE.0,
                    secondary_interface.guest_pointer.0,
                    D3D9_FIXTURE_OUTPUT.0,
                    0xffff_fffe,
                ],
            );
            acknowledge_d3d9_fixture_memory(&mut runtime.memory);
            let before_rejected_query = runtime.memory.stats();
            let rejected = runtime.dispatch_com_call(ComCallTrap {
                endpoint_address: secondary_interface.methods[&4].endpoint,
                stack_pointer: D3D9_FIXTURE_STACK,
                return_address: D3D9_FIXTURE_CALL_SITE,
                call_site: D3D9_FIXTURE_CALL_SITE,
            });
            assert!(matches!(rejected, ComDispatchOutcome::Fault(_)));
            assert_eq!(
                runtime.memory.read(D3D9_FIXTURE_OUTPUT, 8).unwrap(),
                [0xcccc_cccc_u32.to_le_bytes(), 0xdddd_dddd_u32.to_le_bytes()].concat()
            );
            assert_eq!(runtime.memory.stats(), before_rejected_query);
            assert_eq!(
                runtime
                    .audio_buffer_for_com_object(secondary_interface.object_id)
                    .unwrap(),
                &buffer_before_position_query
            );
            assert_eq!(runtime.clock(), &clock_before_position_query);
            assert_eq!(runtime.current_last_error().unwrap(), 0x1234_5678);
            write_d3d9_fixture_words(
                &mut runtime.memory,
                D3D9_FIXTURE_STACK,
                &[
                    D3D9_FIXTURE_CALL_SITE.0,
                    secondary_interface.guest_pointer.0,
                    D3D9_FIXTURE_OUTPUT.0,
                    D3D9_FIXTURE_OUTPUT.0 + 4,
                ],
            );
            acknowledge_d3d9_fixture_memory(&mut runtime.memory);
            let position_query = {
                runtime.dispatch_com_call(ComCallTrap {
                    endpoint_address: secondary_interface.methods[&4].endpoint,
                    stack_pointer: D3D9_FIXTURE_STACK,
                    return_address: D3D9_FIXTURE_CALL_SITE,
                    call_site: D3D9_FIXTURE_CALL_SITE,
                })
            };
            let ComDispatchOutcome::Completed(position_query) = position_query else {
                panic!("stopped secondary position query did not complete: {position_query:?}");
            };
            assert_eq!(position_query.eax, COM_S_OK.bits());

            acknowledge_d3d9_fixture_memory(&mut runtime.memory);
            assert_eq!(
                runtime.memory.read(D3D9_FIXTURE_OUTPUT, 8).unwrap(),
                [4_u32.to_le_bytes(), 4_u32.to_le_bytes()].concat()
            );
            assert_eq!(
                runtime
                    .audio_buffer_for_com_object(secondary_interface.object_id)
                    .unwrap(),
                &buffer_before_position_query
            );
            assert_eq!(runtime.clock(), &clock_before_position_query);
            assert_eq!(runtime.current_last_error().unwrap(), 0x1234_5678);

            write_d3d9_fixture_words(
                &mut runtime.memory,
                D3D9_FIXTURE_STACK,
                &[
                    D3D9_FIXTURE_CALL_SITE.0,
                    secondary_interface.guest_pointer.0,
                    0,
                    0,
                ],
            );
            acknowledge_d3d9_fixture_memory(&mut runtime.memory);
            let buffer_before_empty_position_query = runtime
                .audio_buffer_for_com_object(secondary_interface.object_id)
                .unwrap()
                .clone();
            let memory_before_empty_position_query = runtime.memory.stats();
            let empty_position_query = {
                runtime.dispatch_com_call(ComCallTrap {
                    endpoint_address: secondary_interface.methods[&4].endpoint,
                    stack_pointer: D3D9_FIXTURE_STACK,
                    return_address: D3D9_FIXTURE_CALL_SITE,
                    call_site: D3D9_FIXTURE_CALL_SITE,
                })
            };
            let ComDispatchOutcome::Completed(empty_position_query) = empty_position_query else {
                panic!(
                    "empty secondary position query did not return HRESULT: {empty_position_query:?}"
                );
            };
            assert_eq!(empty_position_query.eax, DSOUND_E_INVALIDARG.bits());

            assert_eq!(
                runtime
                    .audio_buffer_for_com_object(secondary_interface.object_id)
                    .unwrap(),
                &buffer_before_empty_position_query
            );
            assert_eq!(runtime.memory.stats(), memory_before_empty_position_query);
            assert_eq!(runtime.current_last_error().unwrap(), 0x1234_5678);

            for (slot, value) in [(15_u16, 1), (13_u16, 1), (13_u16, SECONDARY_BYTES)] {
                write_d3d9_fixture_words(
                    &mut runtime.memory,
                    D3D9_FIXTURE_STACK,
                    &[
                        D3D9_FIXTURE_CALL_SITE.0,
                        secondary_interface.guest_pointer.0,
                        value,
                    ],
                );
                acknowledge_d3d9_fixture_memory(&mut runtime.memory);
                let buffer_before = runtime
                    .audio_buffer_for_com_object(secondary_interface.object_id)
                    .unwrap()
                    .clone();
                let memory_before = runtime.memory.stats();
                let result = runtime.dispatch_com_call(ComCallTrap {
                    endpoint_address: secondary_interface.methods[&slot].endpoint,
                    stack_pointer: D3D9_FIXTURE_STACK,
                    return_address: D3D9_FIXTURE_CALL_SITE,
                    call_site: D3D9_FIXTURE_CALL_SITE,
                });
                let ComDispatchOutcome::Completed(completion) = result else {
                    panic!("invalid secondary control did not return HRESULT: {result:?}");
                };
                assert_eq!(completion.eax, DSOUND_E_INVALIDARG.bits());
                assert_eq!(
                    runtime
                        .audio_buffer_for_com_object(secondary_interface.object_id)
                        .unwrap(),
                    &buffer_before
                );
                assert_eq!(runtime.memory.stats(), memory_before);
                assert_eq!(runtime.current_last_error().unwrap(), 0x1234_5678);
            }

            write_d3d9_fixture_words(
                &mut runtime.memory,
                D3D9_FIXTURE_STACK,
                &[
                    D3D9_FIXTURE_CALL_SITE.0,
                    secondary_interface.guest_pointer.0,
                    0,
                    0,
                    0,
                ],
            );
            acknowledge_d3d9_fixture_memory(&mut runtime.memory);
            let trap = ComCallTrap {
                endpoint_address: secondary_interface.methods[&12].endpoint,
                stack_pointer: D3D9_FIXTURE_STACK,
                return_address: D3D9_FIXTURE_CALL_SITE,
                call_site: D3D9_FIXTURE_CALL_SITE,
            };
            let ComDispatchOutcome::AudioPending(pending) = runtime.dispatch_com_call(trap) else {
                panic!("secondary Play must await actual Chrome preparation");
            };
            let buffer_before = runtime
                .audio_buffer_for_com_object(secondary_interface.object_id)
                .unwrap()
                .clone();
            let memory_before = runtime.memory.stats();
            assert_eq!(
                &pending.command_bytes()[48..],
                runtime
                    .memory
                    .read(
                        backing.range.start(),
                        buffer_before.descriptor.byte_length as usize
                    )
                    .unwrap()
            );
            let ComDispatchOutcome::Completed(rejected) =
                runtime.complete_com_audio(pending, false)
            else {
                panic!("Chrome media failure did not return HRESULT");
            };
            assert_eq!(rejected.eax, 0x8000_4005);
            assert_eq!(
                runtime
                    .audio_buffer_for_com_object(secondary_interface.object_id)
                    .unwrap(),
                &buffer_before
            );
            assert_eq!(runtime.memory.stats(), memory_before);
            assert_eq!(runtime.current_last_error().unwrap(), 0x1234_5678);
            let ComDispatchOutcome::AudioPending(pending) = runtime.dispatch_com_call(trap) else {
                panic!("retry Play must await Chrome preparation");
            };
            runtime
                .add_ref_com_interface(secondary_interface.id)
                .unwrap();
            assert!(matches!(
                runtime.complete_com_audio(pending, true),
                ComDispatchOutcome::Fault(_)
            ));
            assert_eq!(
                runtime
                    .audio_buffer_for_com_object(secondary_interface.object_id)
                    .unwrap(),
                &buffer_before
            );
            assert_eq!(
                runtime
                    .release_com_interface(secondary_interface.id)
                    .unwrap(),
                1
            );
            let ComDispatchOutcome::AudioPending(pending) = runtime.dispatch_com_call(trap) else {
                panic!("retry live Play");
            };
            let ComDispatchOutcome::Completed(secondary_play) =
                runtime.complete_com_audio(pending, true)
            else {
                panic!("secondary one-shot Play did not complete after Chrome acknowledgement");
            };
            assert_eq!(secondary_play.eax, COM_S_OK.bits());
            assert_eq!(
                runtime
                    .audio_buffer_for_com_object(secondary_interface.object_id)
                    .unwrap()
                    .playback,
                RuntimeAudioPlaybackState::PlayingOnce
            );

            // Rejection keeps the selected lock, backing protection and PCM bytes;
            // retry only unlocks after the replacement Chrome samples are installed.
            let object = secondary_interface.object_id;
            assert!(
                runtime
                    .lock_audio_buffer(
                        object,
                        0,
                        buffer_before.descriptor.byte_length,
                        0,
                        [0, 4, 8, 12]
                            .map(|offset| GuestAddress(D3D9_FIXTURE_PRESENTATION.0 + offset))
                    )
                    .unwrap()
            );
            let lock = runtime
                .audio_buffer_for_com_object(object)
                .unwrap()
                .active_locks[0];
            acknowledge_d3d9_fixture_memory(&mut runtime.memory);
            let pcm_before = runtime
                .memory
                .read(
                    backing.range.start(),
                    buffer_before.descriptor.byte_length as usize,
                )
                .unwrap();
            let BrowserAudioPreparation::Pending(update) = runtime
                .prepare_browser_audio_unlock(object, lock.first_segment, lock.second_segment)
                .unwrap()
            else {
                panic!("playing Unlock must await actual Chrome PCM installation");
            };
            let locked = runtime.audio_buffer_for_com_object(object).unwrap().clone();
            let memory = runtime.memory.stats();
            assert_eq!(
                runtime.complete_browser_audio(&update, false).unwrap(),
                0x8000_4005
            );
            assert_eq!(
                runtime.audio_buffer_for_com_object(object).unwrap(),
                &locked
            );
            assert_eq!(runtime.memory.stats(), memory);
            assert_eq!(
                runtime
                    .memory
                    .read(backing.range.start(), pcm_before.len())
                    .unwrap(),
                pcm_before
            );
            let BrowserAudioPreparation::Pending(update) = runtime
                .prepare_browser_audio_unlock(object, lock.first_segment, lock.second_segment)
                .unwrap()
            else {
                panic!("retry update");
            };
            assert_eq!(runtime.complete_browser_audio(&update, true).unwrap(), 0);
            acknowledge_d3d9_fixture_memory(&mut runtime.memory);
            let old_resource = runtime
                .audio_buffer_for_com_object(object)
                .unwrap()
                .browser_resource_id
                .unwrap();
            let seek = if runtime
                .audio_buffer_for_com_object(object)
                .unwrap()
                .controls
                .current_position_bytes
                == 0
            {
                u32::from(buffer_before.active_format.unwrap().block_align)
            } else {
                0
            };
            let BrowserAudioPreparation::Pending(update) = runtime
                .prepare_browser_audio_control(
                    object,
                    super::super::audio::RuntimeAudioBufferControlUpdate::CurrentPosition(seek),
                )
                .unwrap()
            else {
                panic!("seek must await Chrome");
            };
            let before = runtime.audio_buffer_for_com_object(object).unwrap().clone();
            assert_eq!(
                runtime.complete_browser_audio(&update, false).unwrap(),
                0x8000_4005
            );
            assert_eq!(
                runtime.audio_buffer_for_com_object(object).unwrap(),
                &before
            );
            let BrowserAudioPreparation::Pending(update) = runtime
                .prepare_browser_audio_control(
                    object,
                    super::super::audio::RuntimeAudioBufferControlUpdate::CurrentPosition(seek),
                )
                .unwrap()
            else {
                panic!("retry seek");
            };
            assert_eq!(runtime.complete_browser_audio(&update, true).unwrap(), 0);
            assert_ne!(
                runtime
                    .audio_buffer_for_com_object(object)
                    .unwrap()
                    .browser_resource_id,
                Some(old_resource)
            );
            assert!(!runtime.observe_browser_audio_end(old_resource).unwrap());
            let BrowserAudioPreparation::Pending(stop) = runtime
                .prepare_browser_audio_play(object, RuntimeAudioPlaybackState::Stopped)
                .unwrap()
            else {
                panic!("Stop must await Chrome");
            };
            let before = runtime.audio_buffer_for_com_object(object).unwrap().clone();
            assert_eq!(
                runtime.complete_browser_audio(&stop, false).unwrap(),
                0x8000_4005
            );
            assert_eq!(
                runtime.audio_buffer_for_com_object(object).unwrap(),
                &before
            );
            let BrowserAudioPreparation::Pending(stop) = runtime
                .prepare_browser_audio_play(object, RuntimeAudioPlaybackState::Stopped)
                .unwrap()
            else {
                panic!("retry Stop");
            };
            assert_eq!(runtime.complete_browser_audio(&stop, true).unwrap(), 0);
            assert_eq!(
                runtime
                    .audio_buffer_for_com_object(object)
                    .unwrap()
                    .browser_resource_id,
                None
            );
            let BrowserAudioPreparation::Pending(play) = runtime
                .prepare_browser_audio_play(object, RuntimeAudioPlaybackState::PlayingOnce)
                .unwrap()
            else {
                panic!("restart must await Chrome");
            };
            runtime.complete_browser_audio(&play, true).unwrap();
            let resource = runtime
                .audio_buffer_for_com_object(object)
                .unwrap()
                .browser_resource_id
                .unwrap();
            let stopped = runtime.create_event(false, false).unwrap();
            runtime
                .set_audio_buffer_notifications(
                    object,
                    vec![RuntimeAudioNotification {
                        offset_bytes: u32::MAX,
                        event_handle: stopped,
                    }],
                )
                .unwrap();
            let host = runtime
                .begin_host_request("fixture.browser-pending")
                .unwrap();
            assert!(runtime.observe_browser_audio_end(resource).unwrap());
            assert!(!runtime.observe_browser_audio_end(resource).unwrap());
            assert_eq!(
                runtime
                    .audio_buffer_for_com_object(object)
                    .unwrap()
                    .browser_resource_id,
                None
            );
            assert!(matches!(
                runtime.event_for_handle(stopped).unwrap().wait_state,
                RuntimeWaitState::Signaled { .. }
            ));
            runtime.complete_host_request(host.request_id).unwrap();
            assert!(runtime.try_wait_event(stopped).unwrap());
            let BrowserAudioPreparation::Pending(play) = runtime
                .prepare_browser_audio_play(object, RuntimeAudioPlaybackState::PlayingOnce)
                .unwrap()
            else {
                panic!("real object waiter restart");
            };
            runtime.complete_browser_audio(&play, true).unwrap();
            let resource = runtime
                .audio_buffer_for_com_object(object)
                .unwrap()
                .browser_resource_id
                .unwrap();
            assert!(matches!(
                runtime.wait_current_thread_on(stopped).unwrap(),
                RuntimeWaitDisposition::Pending { .. }
            ));
            assert!(matches!(
                runtime.scheduler_state().unwrap(),
                SchedulerThreadState::WaitingObject { .. }
            ));
            assert!(runtime.observe_browser_audio_end(resource).unwrap());
            assert_eq!(
                runtime.scheduler_state().unwrap(),
                SchedulerThreadState::Runnable
            );
            assert_eq!(
                runtime.event_for_handle(stopped).unwrap().wait_state,
                RuntimeWaitState::Nonsignaled
            );
            assert!(!runtime.observe_browser_audio_end(resource).unwrap());
            runtime
                .set_audio_buffer_notifications(object, Vec::new())
                .unwrap();
            acknowledge_d3d9_fixture_memory(&mut runtime.memory);

            // Exercise Stop on both sides of the wait/Play race without a second event authority.
            {
                let mut notification_runtime = runtime;
                let object = secondary_interface.object_id;
                let playback = |runtime: &mut Runtime, state| {
                    runtime.set_audio_buffer_play(object, state).unwrap();
                };
                playback(
                    &mut notification_runtime,
                    RuntimeAudioPlaybackState::Stopped,
                );
                let position = notification_runtime.create_event(false, false).unwrap();
                let stopped = notification_runtime.create_event(false, false).unwrap();
                let manual_reset = notification_runtime.create_event(true, false).unwrap();
                let closed = notification_runtime.create_event(false, false).unwrap();
                notification_runtime.close_handle(closed).unwrap();
                let wrong_kind = notification_runtime.open_current_thread_handle().unwrap();
                let notify_interface = notification_runtime
                    .com()
                    .interfaces()
                    .values()
                    .find(|interface| {
                        interface.object_id == object && interface.name == "IDirectSoundNotify"
                    })
                    .unwrap()
                    .clone();
                for event in [
                    RuntimeHandle(0),
                    RuntimeHandle(0xdead_beef),
                    closed,
                    wrong_kind,
                    manual_reset,
                ] {
                    write_d3d9_fixture_words(
                        &mut notification_runtime.memory,
                        D3D9_FIXTURE_PRESENTATION,
                        &[0, event.0],
                    );
                    write_d3d9_fixture_words(
                        &mut notification_runtime.memory,
                        D3D9_FIXTURE_STACK,
                        &[
                            D3D9_FIXTURE_CALL_SITE.0,
                            notify_interface.guest_pointer.0,
                            1,
                            D3D9_FIXTURE_PRESENTATION.0,
                        ],
                    );
                    acknowledge_d3d9_fixture_memory(&mut notification_runtime.memory);
                    let audio = notification_runtime.audio.clone();
                    let objects = notification_runtime.objects.clone();
                    let memory = notification_runtime.memory.stats();
                    let outcome = notification_runtime.dispatch_com_call(ComCallTrap {
                        endpoint_address: notify_interface.methods[&3].endpoint,
                        stack_pointer: D3D9_FIXTURE_STACK,
                        return_address: D3D9_FIXTURE_CALL_SITE,
                        call_site: D3D9_FIXTURE_CALL_SITE,
                    });
                    let ComDispatchOutcome::Completed(completion) = outcome else {
                        panic!("invalid notification handle did not return HRESULT: {outcome:?}");
                    };
                    assert_eq!(completion.eax, DSOUND_E_INVALIDARG.bits());
                    assert_eq!(notification_runtime.audio, audio);
                    assert_eq!(notification_runtime.objects, objects);
                    assert_eq!(notification_runtime.memory.stats(), memory);
                    assert_eq!(
                        notification_runtime.current_last_error().unwrap(),
                        0x1234_5678
                    );
                }
                write_d3d9_fixture_words(
                    &mut notification_runtime.memory,
                    D3D9_FIXTURE_PRESENTATION,
                    &[0, position.0],
                );
                write_d3d9_fixture_words(
                    &mut notification_runtime.memory,
                    D3D9_FIXTURE_STACK,
                    &[
                        D3D9_FIXTURE_CALL_SITE.0,
                        notify_interface.guest_pointer.0,
                        1,
                        D3D9_FIXTURE_PRESENTATION.0,
                    ],
                );
                acknowledge_d3d9_fixture_memory(&mut notification_runtime.memory);
                let pending = notification_runtime
                    .begin_host_request("fixture.pending")
                    .unwrap();
                let audio = notification_runtime.audio.clone();
                let objects = notification_runtime.objects.clone();
                let memory = notification_runtime.memory.stats();
                let scheduler = notification_runtime.scheduler_state().unwrap();
                let blocked = notification_runtime.dispatch_com_call(ComCallTrap {
                    endpoint_address: notify_interface.methods[&3].endpoint,
                    stack_pointer: D3D9_FIXTURE_STACK,
                    return_address: D3D9_FIXTURE_CALL_SITE,
                    call_site: D3D9_FIXTURE_CALL_SITE,
                });
                assert!(matches!(blocked, ComDispatchOutcome::Fault(_)));
                assert_eq!(notification_runtime.audio, audio);
                assert_eq!(notification_runtime.objects, objects);
                assert_eq!(notification_runtime.memory.stats(), memory);
                assert_eq!(notification_runtime.scheduler_state().unwrap(), scheduler);
                assert_eq!(
                    notification_runtime.current_last_error().unwrap(),
                    0x1234_5678
                );
                notification_runtime
                    .complete_host_request(pending.request_id)
                    .unwrap();
                let retried = notification_runtime.dispatch_com_call(ComCallTrap {
                    endpoint_address: notify_interface.methods[&3].endpoint,
                    stack_pointer: D3D9_FIXTURE_STACK,
                    return_address: D3D9_FIXTURE_CALL_SITE,
                    call_site: D3D9_FIXTURE_CALL_SITE,
                });
                let ComDispatchOutcome::Completed(completion) = retried else {
                    panic!("notification retry did not complete: {retried:?}");
                };
                assert_eq!(completion.eax, COM_S_OK.bits());
                let buffer_before_invalid = notification_runtime
                    .audio_buffer_for_com_object(object)
                    .unwrap()
                    .clone();
                let events_before_invalid = [position, stopped, manual_reset]
                    .map(|handle| notification_runtime.event_for_handle(handle).unwrap());
                assert!(
                    !notification_runtime
                        .set_audio_buffer_notifications(
                            object,
                            vec![
                                RuntimeAudioNotification {
                                    offset_bytes: 0,
                                    event_handle: position,
                                },
                                RuntimeAudioNotification {
                                    offset_bytes: u32::MAX,
                                    event_handle: manual_reset,
                                },
                            ]
                        )
                        .unwrap()
                );
                assert_eq!(
                    notification_runtime
                        .audio_buffer_for_com_object(object)
                        .unwrap(),
                    &buffer_before_invalid
                );
                assert_eq!(
                    [position, stopped, manual_reset]
                        .map(|handle| notification_runtime.event_for_handle(handle).unwrap()),
                    events_before_invalid
                );
                assert!(notification_runtime.scheduler.host_requests().is_empty());
                let handles = [position, stopped];
                notification_runtime
                    .set_audio_buffer_notifications(
                        object,
                        vec![
                            RuntimeAudioNotification {
                                offset_bytes: 0,
                                event_handle: position,
                            },
                            RuntimeAudioNotification {
                                offset_bytes: u32::MAX,
                                event_handle: stopped,
                            },
                        ],
                    )
                    .unwrap();
                let waiter = notification_runtime.current_thread_id();
                let controller = RuntimeThreadId(0x3000);
                notification_runtime.register_thread(controller, 0).unwrap();
                if !wait_before_play {
                    playback(
                        &mut notification_runtime,
                        RuntimeAudioPlaybackState::PlayingLooping,
                    );
                }
                let buffer_before_unmatched = notification_runtime
                    .audio_buffer_for_com_object(object)
                    .unwrap()
                    .clone();
                let scheduler_before_unmatched = notification_runtime.scheduler_state().unwrap();
                let events_before_unmatched =
                    handles.map(|handle| notification_runtime.event_for_handle(handle).unwrap());
                assert!(
                    notification_runtime
                        .begin_audio_notification_wait(&[stopped, position], 100)
                        .unwrap()
                        .is_none()
                );
                assert_eq!(
                    notification_runtime
                        .audio_buffer_for_com_object(object)
                        .unwrap(),
                    &buffer_before_unmatched
                );
                assert_eq!(
                    handles.map(|handle| notification_runtime.event_for_handle(handle).unwrap()),
                    events_before_unmatched
                );
                assert_eq!(
                    notification_runtime.scheduler_state().unwrap(),
                    scheduler_before_unmatched
                );
                assert!(notification_runtime.scheduler.host_requests().is_empty());
                let (request, payload) = notification_runtime
                    .begin_audio_notification_wait(&handles, 100)
                    .unwrap()
                    .expect("notification handles belong to the buffer");
                notification_runtime.select_thread(controller).unwrap();
                if wait_before_play {
                    playback(
                        &mut notification_runtime,
                        RuntimeAudioPlaybackState::PlayingLooping,
                    );
                }
                playback(
                    &mut notification_runtime,
                    RuntimeAudioPlaybackState::Stopped,
                );
                assert!(matches!(
                    notification_runtime
                        .event_for_handle(stopped)
                        .unwrap()
                        .wait_state,
                    RuntimeWaitState::Signaled { .. }
                ));
                let completion = notification_runtime
                    .plan_audio_notification_wait_completion(request.request_id, 100, &payload)
                    .unwrap();
                assert_eq!(completion.result_index(), Some(1));
                notification_runtime.commit_audio_notification_wait_completion(completion);
                assert!(!notification_runtime.try_wait_event(stopped).unwrap());

                // A late completion preserves actual progress and the late signal,
                // but does not report a notification that occurred after the timeout.
                notification_runtime.select_thread(waiter).unwrap();
                notification_runtime
                    .set_audio_buffer_control(
                        object,
                        super::super::audio::RuntimeAudioBufferControlUpdate::CurrentPosition(0),
                    )
                    .unwrap();
                notification_runtime
                    .set_audio_buffer_control(
                        object,
                        super::super::audio::RuntimeAudioBufferControlUpdate::Frequency(8_000),
                    )
                    .unwrap();
                notification_runtime
                    .set_audio_buffer_notifications(
                        object,
                        vec![
                            RuntimeAudioNotification {
                                offset_bytes: 3_840,
                                event_handle: position,
                            },
                            RuntimeAudioNotification {
                                offset_bytes: u32::MAX,
                                event_handle: stopped,
                            },
                        ],
                    )
                    .unwrap();
                playback(
                    &mut notification_runtime,
                    RuntimeAudioPlaybackState::PlayingLooping,
                );
                let start = notification_runtime.process.host_elapsed_time_100ns();
                let (request, payload) = notification_runtime
                    .begin_audio_notification_wait(&handles, 100)
                    .unwrap()
                    .unwrap();
                notification_runtime
                    .synchronize_scheduled_clock_for_host_execution(0, start + 1_300_000)
                    .unwrap();
                let completion = notification_runtime
                    .plan_audio_notification_wait_completion(request.request_id, 100, &payload)
                    .unwrap();
                assert_eq!(completion.result_index(), None);
                assert!(completion.retry_payload().is_none());
                notification_runtime.commit_audio_notification_wait_completion(completion);
                assert_eq!(
                    notification_runtime.audio_buffer_position(object).unwrap(),
                    4_160
                );
                assert!(notification_runtime.try_wait_event(position).unwrap());
                playback(
                    &mut notification_runtime,
                    RuntimeAudioPlaybackState::Stopped,
                );
                assert!(notification_runtime.try_wait_event(stopped).unwrap());

                // A slower frequency while waiting repaces the same request to
                // its original deadline, without completing the API early.
                notification_runtime
                    .set_audio_buffer_control(
                        object,
                        super::super::audio::RuntimeAudioBufferControlUpdate::CurrentPosition(0),
                    )
                    .unwrap();
                notification_runtime
                    .set_audio_buffer_control(
                        object,
                        super::super::audio::RuntimeAudioBufferControlUpdate::Frequency(16_000),
                    )
                    .unwrap();
                playback(
                    &mut notification_runtime,
                    RuntimeAudioPlaybackState::PlayingLooping,
                );
                let start = notification_runtime.process.host_elapsed_time_100ns();
                let (request, payload) = notification_runtime
                    .begin_audio_notification_wait(&handles, 100)
                    .unwrap()
                    .unwrap();
                notification_runtime.select_thread(controller).unwrap();
                notification_runtime
                    .synchronize_scheduled_clock_for_host_execution(0, start + 200_000)
                    .unwrap();
                notification_runtime
                    .set_audio_buffer_control(
                        object,
                        super::super::audio::RuntimeAudioBufferControlUpdate::Frequency(8_000),
                    )
                    .unwrap();
                notification_runtime
                    .synchronize_scheduled_clock_for_host_execution(0, start + 600_000)
                    .unwrap();
                let completion = notification_runtime
                    .plan_audio_notification_wait_completion(request.request_id, 100, &payload)
                    .unwrap();
                let retry = completion.retry_payload().unwrap().to_vec();
                assert_eq!(u32::from_le_bytes(retry[4..8].try_into().unwrap()), 40);
                notification_runtime.commit_audio_notification_wait_completion(completion);
                notification_runtime
                    .synchronize_scheduled_clock_for_host_execution(0, start + 1_000_000)
                    .unwrap();
                let completion = notification_runtime
                    .plan_audio_notification_wait_completion(request.request_id, 100, &retry)
                    .unwrap();
                assert_eq!(completion.result_index(), Some(0));
                assert!(completion.retry_payload().is_none());
                notification_runtime.commit_audio_notification_wait_completion(completion);
                notification_runtime.select_thread(waiter).unwrap();

                // Concurrent waits consume a crossed auto-reset notification once.
                notification_runtime
                    .set_audio_buffer_control(
                        object,
                        super::super::audio::RuntimeAudioBufferControlUpdate::CurrentPosition(0),
                    )
                    .unwrap();
                notification_runtime
                    .set_audio_buffer_control(
                        object,
                        super::super::audio::RuntimeAudioBufferControlUpdate::Frequency(16_000),
                    )
                    .unwrap();
                let start = notification_runtime.process.host_elapsed_time_100ns();
                let (first, first_payload) = notification_runtime
                    .begin_audio_notification_wait(&handles, 100)
                    .unwrap()
                    .unwrap();
                notification_runtime.select_thread(controller).unwrap();
                let (second, second_payload) = notification_runtime
                    .begin_audio_notification_wait(&handles, 100)
                    .unwrap()
                    .unwrap();
                notification_runtime
                    .synchronize_scheduled_clock_for_host_execution(0, start + 800_000)
                    .unwrap();
                let first = notification_runtime
                    .plan_audio_notification_wait_completion(first.request_id, 100, &first_payload)
                    .unwrap();
                assert_eq!(first.result_index(), Some(0));
                notification_runtime.commit_audio_notification_wait_completion(first);
                let second_completion = notification_runtime
                    .plan_audio_notification_wait_completion(
                        second.request_id,
                        100,
                        &second_payload,
                    )
                    .unwrap();
                assert_eq!(second_completion.result_index(), None);
                let retry = second_completion.retry_payload().unwrap().to_vec();
                notification_runtime.commit_audio_notification_wait_completion(second_completion);
                notification_runtime
                    .synchronize_scheduled_clock_for_host_execution(0, start + 1_000_000)
                    .unwrap();
                let second = notification_runtime
                    .plan_audio_notification_wait_completion(second.request_id, 100, &retry)
                    .unwrap();
                assert_eq!(second.result_index(), None);
                assert!(second.retry_payload().is_none());
                notification_runtime.commit_audio_notification_wait_completion(second);
                notification_runtime.select_thread(waiter).unwrap();

                // Stop freezes the real phase. Restart uses that byte position,
                // and an ACK rejection cannot publish a seek or alter the phase.
                playback(
                    &mut notification_runtime,
                    RuntimeAudioPlaybackState::Stopped,
                );
                assert!(notification_runtime.try_wait_event(stopped).unwrap());
                let frozen = notification_runtime.audio_buffer_position(object).unwrap();
                notification_runtime
                    .synchronize_scheduled_clock_for_host_execution(0, start + 2_000_000)
                    .unwrap();
                assert_eq!(
                    notification_runtime.audio_buffer_position(object).unwrap(),
                    frozen
                );
                let BrowserAudioPreparation::Pending(play) = notification_runtime
                    .prepare_browser_audio_play(object, RuntimeAudioPlaybackState::PlayingLooping)
                    .unwrap()
                else {
                    panic!("restart needs Chrome ACK");
                };
                assert_eq!(
                    u32::from_le_bytes(play.command[36..40].try_into().unwrap()),
                    frozen
                );
                notification_runtime
                    .complete_browser_audio(&play, true)
                    .unwrap();
                let BrowserAudioPreparation::Pending(seek) = notification_runtime
                    .prepare_browser_audio_control(
                        object,
                        super::super::audio::RuntimeAudioBufferControlUpdate::CurrentPosition(0),
                    )
                    .unwrap()
                else {
                    panic!("seek needs Chrome ACK");
                };
                let before = notification_runtime
                    .audio_buffer_for_com_object(object)
                    .unwrap()
                    .clone();
                notification_runtime
                    .synchronize_scheduled_clock_for_host_execution(0, start + 2_100_000)
                    .unwrap();
                assert_eq!(
                    notification_runtime
                        .complete_browser_audio(&seek, false)
                        .unwrap(),
                    0x8000_4005
                );
                assert_eq!(
                    notification_runtime
                        .audio_buffer_for_com_object(object)
                        .unwrap(),
                    &before
                );
                let BrowserAudioPreparation::Pending(seek) = notification_runtime
                    .prepare_browser_audio_control(
                        object,
                        super::super::audio::RuntimeAudioBufferControlUpdate::CurrentPosition(0),
                    )
                    .unwrap()
                else {
                    panic!("retry seek needs Chrome ACK");
                };
                notification_runtime
                    .complete_browser_audio(&seek, true)
                    .unwrap();
                assert_eq!(
                    notification_runtime.audio_buffer_position(object).unwrap(),
                    0
                );
                playback(
                    &mut notification_runtime,
                    RuntimeAudioPlaybackState::Stopped,
                );
                assert!(notification_runtime.try_wait_event(stopped).unwrap());

                // No synthetic stop is replayed once the auto-reset event has been consumed.
                notification_runtime.select_thread(waiter).unwrap();
                let (request, payload) = notification_runtime
                    .begin_audio_notification_wait(&handles, 100)
                    .unwrap()
                    .expect("notification handles belong to the buffer");
                let before_rejection = notification_runtime
                    .audio_buffer_for_com_object(object)
                    .unwrap()
                    .clone();
                let mut wrong_delay = payload.clone();
                wrong_delay[4..8].copy_from_slice(&0u32.to_le_bytes());
                assert!(
                    notification_runtime
                        .plan_audio_notification_wait_completion(
                            request.request_id,
                            100,
                            &wrong_delay,
                        )
                        .is_err()
                );
                assert_eq!(
                    notification_runtime
                        .scheduler
                        .validate_host_completion(request.request_id)
                        .unwrap()
                        .request_id,
                    request.request_id
                );
                assert_eq!(
                    notification_runtime
                        .audio_buffer_for_com_object(object)
                        .unwrap(),
                    &before_rejection
                );
                assert_eq!(
                    notification_runtime
                        .event_for_handle(stopped)
                        .unwrap()
                        .wait_state,
                    RuntimeWaitState::Nonsignaled
                );
                notification_runtime
                    .synchronize_scheduled_clock_for_host_execution(
                        0,
                        notification_runtime.process.host_elapsed_time_100ns() + 1_000_000,
                    )
                    .unwrap();
                let completion = notification_runtime
                    .plan_audio_notification_wait_completion(request.request_id, 100, &payload)
                    .unwrap();
                assert_eq!(completion.result_index(), None);
                notification_runtime.commit_audio_notification_wait_completion(completion);

                // A late waiter still sees Stop exactly once; repeated Stop creates no new transition.
                playback(
                    &mut notification_runtime,
                    RuntimeAudioPlaybackState::PlayingLooping,
                );
                playback(
                    &mut notification_runtime,
                    RuntimeAudioPlaybackState::Stopped,
                );
                assert!(notification_runtime.try_wait_event(stopped).unwrap());
                playback(
                    &mut notification_runtime,
                    RuntimeAudioPlaybackState::Stopped,
                );
                assert!(!notification_runtime.try_wait_event(stopped).unwrap());

                playback(
                    &mut notification_runtime,
                    RuntimeAudioPlaybackState::PlayingLooping,
                );
                notification_runtime.close_handle(stopped).unwrap();
                let buffer_before_failed_stop = notification_runtime
                    .audio_buffer_for_com_object(object)
                    .unwrap()
                    .clone();
                let position_event_before_failed_stop =
                    notification_runtime.event_for_handle(position).unwrap();
                assert!(
                    notification_runtime
                        .set_audio_buffer_play(object, RuntimeAudioPlaybackState::Stopped)
                        .is_err()
                );
                assert_eq!(
                    notification_runtime
                        .audio_buffer_for_com_object(object)
                        .unwrap(),
                    &buffer_before_failed_stop,
                    "failed event signal partially stopped audio"
                );
                assert_eq!(
                    notification_runtime.event_for_handle(position).unwrap(),
                    position_event_before_failed_stop
                );
                assert_eq!(notification_runtime.current_thread_id(), waiter);
                assert_eq!(
                    notification_runtime.scheduler_state().unwrap(),
                    SchedulerThreadState::Runnable
                );
                assert_eq!(
                    notification_runtime.current_last_error().unwrap(),
                    0x1234_5678
                );
                runtime = notification_runtime;
            }

            let BrowserAudioPreparation::Pending(play) = runtime
                .prepare_browser_audio_play(
                    secondary_interface.object_id,
                    RuntimeAudioPlaybackState::PlayingLooping,
                )
                .unwrap()
            else {
                panic!("install final browser playback");
            };
            runtime.complete_browser_audio(&play, true).unwrap();
            let release = runtime
                .prepare_browser_audio_release(secondary_interface.id)
                .unwrap()
                .expect("final live audio release must await Chrome");
            let audio = runtime.audio.clone();
            let com = runtime
                .com
                .object(secondary_interface.object_id)
                .unwrap()
                .clone();
            let memory = runtime.memory.stats();
            let allocations = runtime.address_space_allocator.clone();
            assert!(runtime.complete_browser_audio(&release, false).is_err());
            assert_eq!(runtime.audio, audio);
            assert_eq!(
                runtime.com.object(secondary_interface.object_id).unwrap(),
                &com
            );
            assert_eq!(runtime.memory.stats(), memory);
            assert_eq!(runtime.address_space_allocator, allocations);

            let release = dispatch_d3d9_fixture_release(
                &mut runtime,
                &secondary_interface,
                secondary_interface.guest_pointer,
            );
            assert_eq!(release.eax, 0);
            assert!(runtime.com().object(secondary_interface.object_id).is_err());
            assert!(runtime.memory.read(backing.range.start(), 4).is_err());
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
    }

    #[test]
    fn d3d9_vertex_buffer_resource_owner_exhaustion_rolls_back_all_authorities() {
        let memory = d3d9_fixture_memory_with_limits(MemoryLimits {
            max_owners: 4,
            ..MemoryLimits::default()
        });
        let mut runtime = d3d9_fixture_runtime(memory);
        let window = seed_created_d3d9_window(&mut runtime);

        let root = runtime
            .install_com_object(direct3d9_object_spec())
            .expect("install D3D9 root COM object");
        let root_interface = runtime
            .com()
            .interface_for_object_iid(root.object_id, D3D9_IDIRECT3D9_IID)
            .unwrap()
            .clone();
        acknowledge_d3d9_fixture_memory(&mut runtime.memory);

        write_d3d9_fixture_words(
            &mut runtime.memory,
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
                80,
                0,
                0,
                0,
            ],
        );
        write_d3d9_fixture_words(&mut runtime.memory, D3D9_FIXTURE_OUTPUT, &[0xcccc_cccc]);
        write_d3d9_fixture_words(
            &mut runtime.memory,
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
        acknowledge_d3d9_fixture_memory(&mut runtime.memory);
        let create_device = match dispatch_d3d9_fixture_basic(
            &mut runtime,
            &root_interface,
            16,
            D3D9_FIXTURE_CALL_SITE,
        ) {
            ComDispatchOutcome::Completed(completion) => completion,
            other => panic!("CreateDevice did not complete: {other:?}"),
        };
        assert_eq!(create_device.eax, COM_S_OK.bits());
        acknowledge_d3d9_fixture_memory(&mut runtime.memory);
        let device_pointer = u32::from_le_bytes(
            runtime
                .memory
                .read(D3D9_FIXTURE_OUTPUT, 4)
                .unwrap()
                .try_into()
                .unwrap(),
        );
        let device_interface = runtime
            .com_interface_for_pointer(GuestAddress(device_pointer))
            .unwrap()
            .expect("IDirect3DDevice9 output")
            .clone();

        write_d3d9_fixture_words(
            &mut runtime.memory,
            D3D9_FIXTURE_SURFACE_OUTPUT,
            &[0xcccc_cccc],
        );
        write_d3d9_fixture_words(
            &mut runtime.memory,
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
        acknowledge_d3d9_fixture_memory(&mut runtime.memory);
        let before_memory = runtime.memory.stats();
        let allocations_before = runtime.address_space_allocator.clone();
        let object_count_before = runtime.com.objects().len();
        let root_refs_before = runtime.com.objects()[&root_interface.object_id].reference_count;
        let device_refs_before = runtime.com.objects()[&device_interface.object_id].reference_count;
        match dispatch_d3d9_fixture_basic(
            &mut runtime,
            &device_interface,
            26,
            GuestAddress(0x1005_a890),
        ) {
            ComDispatchOutcome::Fault(fault) => fault,
            other => panic!("resource exhaustion unexpectedly completed: {other:?}"),
        };
        assert_eq!(runtime.memory.stats(), before_memory);
        assert_eq!(runtime.address_space_allocator, allocations_before);
        assert_eq!(runtime.com.objects().len(), object_count_before);
        assert_eq!(
            runtime.com.objects()[&root_interface.object_id].reference_count,
            root_refs_before
        );
        assert_eq!(
            runtime.com.objects()[&device_interface.object_id].reference_count,
            device_refs_before
        );
        assert!(runtime.graphics.vertex_buffers().is_empty());
        assert_eq!(runtime.current_last_error().unwrap(), 0x1234_5678);
        assert_eq!(
            runtime.memory.read(D3D9_FIXTURE_SURFACE_OUTPUT, 4).unwrap(),
            0xcccc_cccc_u32.to_le_bytes()
        );
    }

    #[test]
    fn generic_d3d9_device_texture_surface_and_dc_have_real_lifetimes() {
        let memory = d3d9_fixture_memory();
        let mut runtime = d3d9_fixture_runtime(memory);
        let window = seed_created_d3d9_window(&mut runtime);

        let root = runtime.install_com_object(direct3d9_object_spec()).unwrap();
        let root_interface = runtime
            .com()
            .interface_for_object_iid(root.object_id, D3D9_IDIRECT3D9_IID)
            .unwrap()
            .clone();
        acknowledge_d3d9_fixture_memory(&mut runtime.memory);
        write_d3d9_fixture_words(
            &mut runtime.memory,
            D3D9_FIXTURE_PRESENTATION,
            &[
                0,
                0,
                D3D9_D3DFMT_UNKNOWN,
                1,
                0,
                0,
                D3D9_D3DSWAPEFFECT_DISCARD,
                window.handle,
                1,
                0,
                80,
                0,
                0,
                0,
            ],
        );
        let invoke = |runtime: &mut Runtime,
                      interface: &RuntimeComInterfaceRecord,
                      slot: u16,
                      args: &[u32]| {
            let mut words = vec![D3D9_FIXTURE_CALL_SITE.0, interface.guest_pointer.0];
            words.extend_from_slice(args);
            write_d3d9_fixture_words(&mut runtime.memory, D3D9_FIXTURE_STACK, &words);
            acknowledge_d3d9_fixture_memory(&mut runtime.memory);
            let outcome = runtime.dispatch_com_call(ComCallTrap {
                endpoint_address: interface.methods[&slot].endpoint,
                stack_pointer: D3D9_FIXTURE_STACK,
                return_address: D3D9_FIXTURE_CALL_SITE,
                call_site: D3D9_FIXTURE_CALL_SITE,
            });
            let ComDispatchOutcome::Completed(completion) = outcome else {
                panic!("generic graphics call stopped: {outcome:?}");
            };
            acknowledge_d3d9_fixture_memory(&mut runtime.memory);
            completion
        };
        let device = invoke(
            &mut runtime,
            &root_interface,
            16,
            &[
                0,
                D3D9_D3DDEVTYPE_HAL,
                window.handle,
                D3D9_CREATE_HARDWARE_VERTEX_PROCESSING_MULTITHREADED,
                D3D9_FIXTURE_PRESENTATION.0,
                D3D9_FIXTURE_OUTPUT.0,
            ],
        );
        assert_eq!(device.eax, COM_S_OK.bits());
        let read_pointer = |memory: &GuestMemory, address: GuestAddress| {
            GuestAddress(u32::from_le_bytes(
                memory.read(address, 4).unwrap().try_into().unwrap(),
            ))
        };
        let device_interface = runtime
            .com_interface_for_pointer(read_pointer(&runtime.memory, D3D9_FIXTURE_OUTPUT))
            .unwrap()
            .unwrap()
            .clone();
        let prefix = runtime.memory.read(D3D9_FIXTURE_PRESENTATION, 12).unwrap();
        assert_eq!(
            prefix,
            [800_u32, 600, D3D9_D3DFMT_X8R8G8B8]
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect::<Vec<_>>()
        );
        let before_inactive = runtime.graphics.clone();
        let inactive = invoke(&mut runtime, &device_interface, 42, &[]);
        assert_eq!(inactive.eax, D3D9_D3DERR_INVALIDCALL.bits());
        assert_eq!(runtime.graphics, before_inactive);
        assert_eq!(runtime.current_last_error().unwrap(), 0x1234_5678);
        let begun = invoke(&mut runtime, &device_interface, 41, &[]);
        assert_eq!(begun.eax, COM_S_OK.bits());
        let before_nested = runtime.graphics.clone();
        let nested = invoke(&mut runtime, &device_interface, 41, &[]);
        assert_eq!(nested.eax, D3D9_D3DERR_INVALIDCALL.bits());
        assert_eq!(runtime.graphics, before_nested);
        assert_eq!(runtime.current_last_error().unwrap(), 0x1234_5678);
        let ended = invoke(&mut runtime, &device_interface, 42, &[]);
        assert_eq!(ended.eax, COM_S_OK.bits());
        for slot in [41, 42] {
            write_d3d9_fixture_words(
                &mut runtime.memory,
                D3D9_FIXTURE_STACK,
                &[D3D9_FIXTURE_CALL_SITE.0, device_interface.guest_pointer.0],
            );
            acknowledge_d3d9_fixture_memory(&mut runtime.memory);
            let pending = runtime.begin_host_request("fixture.pending").unwrap();
            let graphics = runtime.graphics.clone();
            let memory = runtime.memory.stats();
            let scheduler = runtime.scheduler_state().unwrap();
            let blocked = runtime.dispatch_com_call(ComCallTrap {
                endpoint_address: device_interface.methods[&slot].endpoint,
                stack_pointer: D3D9_FIXTURE_STACK,
                return_address: D3D9_FIXTURE_CALL_SITE,
                call_site: D3D9_FIXTURE_CALL_SITE,
            });
            assert!(matches!(blocked, ComDispatchOutcome::Fault(_)));
            assert_eq!(runtime.graphics, graphics);
            assert_eq!(runtime.memory.stats(), memory);
            assert_eq!(runtime.scheduler_state().unwrap(), scheduler);
            assert_eq!(runtime.current_last_error().unwrap(), 0x1234_5678);
            runtime.complete_host_request(pending.request_id).unwrap();
        }
        let empty = invoke(&mut runtime, &device_interface, 17, &[0, 0, 0, 0]);
        assert_eq!(empty.eax, D3D9_D3DERR_INVALIDCALL.bits());
        assert_eq!(
            invoke(
                &mut runtime,
                &device_interface,
                43,
                &[0, 0, D3D9_D3DCLEAR_TARGET, 0xff113355, 0, 0]
            )
            .eax,
            COM_S_OK.bits()
        );
        assert_eq!(
            invoke(&mut runtime, &device_interface, 41, &[]).eax,
            COM_S_OK.bits()
        );
        let before_active_present = runtime.graphics.clone();
        assert_eq!(
            invoke(&mut runtime, &device_interface, 17, &[0, 0, 0, 0]).eax,
            D3D9_D3DERR_INVALIDCALL.bits()
        );
        assert_eq!(runtime.graphics, before_active_present);
        assert_eq!(
            invoke(&mut runtime, &device_interface, 42, &[]).eax,
            COM_S_OK.bits()
        );
        assert_eq!(
            invoke(&mut runtime, &device_interface, 17, &[0, 0, 0, 0]).eax,
            COM_S_OK.bits()
        );
        let presented = runtime.graphics.devices().values().next().unwrap();
        assert_eq!(
            presented
                .presented_frame
                .as_ref()
                .unwrap()
                .clear_color
                .unwrap(),
            0xff113355
        );
        assert!(presented.back_buffer.clear_color.is_none());
        assert!(presented.back_buffer.draw_submissions.is_empty());
        assert_eq!(runtime.current_last_error().unwrap(), 0x1234_5678);
        write_d3d9_fixture_words(
            &mut runtime.memory,
            D3D9_FIXTURE_STACK,
            &[
                D3D9_FIXTURE_CALL_SITE.0,
                device_interface.guest_pointer.0,
                1,
                0,
                0,
                0,
            ],
        );
        acknowledge_d3d9_fixture_memory(&mut runtime.memory);
        let before_partial_present = runtime.graphics.clone();
        let unsupported = runtime.dispatch_com_call(ComCallTrap {
            endpoint_address: device_interface.methods[&17].endpoint,
            stack_pointer: D3D9_FIXTURE_STACK,
            return_address: D3D9_FIXTURE_CALL_SITE,
            call_site: D3D9_FIXTURE_CALL_SITE,
        });
        assert!(matches!(unsupported, ComDispatchOutcome::Fault(_)));
        assert_eq!(runtime.graphics, before_partial_present);
        assert_eq!(runtime.current_last_error().unwrap(), 0x1234_5678);
        assert!(!runtime.memory.has_pending_delta());
        // Non-title dimensions must be accepted; unsupported mipmaps must not mutate.
        let invalid = invoke(
            &mut runtime,
            &device_interface,
            23,
            &[
                37,
                19,
                2,
                0,
                D3D9_D3DFMT_X8R8G8B8,
                D3D9_D3DPOOL_SYSTEMMEM,
                D3D9_FIXTURE_OUTPUT.0,
                0,
            ],
        );
        assert_eq!(invalid.eax, D3D9_D3DERR_NOTAVAILABLE.bits());
        assert!(runtime.graphics.textures().is_empty());
        let texture = invoke(
            &mut runtime,
            &device_interface,
            23,
            &[
                37,
                19,
                1,
                0,
                D3D9_D3DFMT_X8R8G8B8,
                D3D9_D3DPOOL_SYSTEMMEM,
                D3D9_FIXTURE_OUTPUT.0,
                0,
            ],
        );
        assert_eq!(texture.eax, COM_S_OK.bits());
        let texture_interface = runtime
            .com_interface_for_pointer(read_pointer(&runtime.memory, D3D9_FIXTURE_OUTPUT))
            .unwrap()
            .unwrap()
            .clone();
        write_d3d9_fixture_words(
            &mut runtime.memory,
            D3D9_FIXTURE_PRESENTATION,
            &[0, 0, 38, 19],
        );
        write_d3d9_fixture_words(
            &mut runtime.memory,
            D3D9_FIXTURE_OUTPUT,
            &[0xcccc_cccc, 0xdddd_dddd],
        );
        write_d3d9_fixture_words(
            &mut runtime.memory,
            D3D9_FIXTURE_STACK,
            &[
                D3D9_FIXTURE_CALL_SITE.0,
                texture_interface.guest_pointer.0,
                0,
                D3D9_FIXTURE_OUTPUT.0,
                D3D9_FIXTURE_PRESENTATION.0,
                crate::d3d9::D3D9_D3DLOCK_NO_DIRTY_UPDATE,
            ],
        );
        acknowledge_d3d9_fixture_memory(&mut runtime.memory);
        let graphics_before_invalid_lock = runtime.graphics.clone();
        let memory_before_invalid_lock = runtime.memory.stats();
        let allocations_before_invalid_lock = runtime.address_space_allocator.clone();
        let invalid_lock = runtime.dispatch_com_call(ComCallTrap {
            endpoint_address: texture_interface.methods[&19].endpoint,
            stack_pointer: D3D9_FIXTURE_STACK,
            return_address: D3D9_FIXTURE_CALL_SITE,
            call_site: D3D9_FIXTURE_CALL_SITE,
        });
        let ComDispatchOutcome::Completed(invalid_lock) = invalid_lock else {
            panic!("invalid texture rectangle did not return HRESULT: {invalid_lock:?}");
        };
        assert_eq!(invalid_lock.eax, D3D9_D3DERR_INVALIDCALL.bits());
        assert_eq!(runtime.graphics, graphics_before_invalid_lock);
        assert_eq!(runtime.memory.stats(), memory_before_invalid_lock);
        assert_eq!(
            runtime.address_space_allocator,
            allocations_before_invalid_lock
        );
        assert_eq!(
            runtime.memory.read(D3D9_FIXTURE_OUTPUT, 8).unwrap(),
            [0xcccc_cccc_u32, 0xdddd_dddd]
                .into_iter()
                .flat_map(u32::to_le_bytes)
                .collect::<Vec<_>>()
        );
        assert_eq!(runtime.current_last_error().unwrap(), 0x1234_5678);

        for _ in 0..2 {
            let surface = invoke(
                &mut runtime,
                &texture_interface,
                18,
                &[0, D3D9_FIXTURE_SURFACE_OUTPUT.0],
            );
            assert_eq!(surface.eax, COM_S_OK.bits());
        }
        let surface_interface = runtime
            .com_interface_for_pointer(read_pointer(&runtime.memory, D3D9_FIXTURE_SURFACE_OUTPUT))
            .unwrap()
            .unwrap()
            .clone();
        let dc = invoke(
            &mut runtime,
            &surface_interface,
            15,
            &[D3D9_FIXTURE_OUTPUT.0],
        );
        assert_eq!(dc.eax, COM_S_OK.bits());
        let hdc = read_pointer(&runtime.memory, D3D9_FIXTURE_OUTPUT).0;
        assert!(runtime.gdi_device_context(hdc).is_ok());
        let repeated = invoke(
            &mut runtime,
            &surface_interface,
            15,
            &[D3D9_FIXTURE_OUTPUT.0],
        );
        assert_eq!(repeated.eax, D3D9_D3DERR_INVALIDCALL.bits());
        let released = invoke(&mut runtime, &surface_interface, 16, &[hdc]);
        assert_eq!(released.eax, COM_S_OK.bits());
        assert!(runtime.gdi_device_context(hdc).is_err());
        let vertex = invoke(
            &mut runtime,
            &device_interface,
            26,
            &[
                112,
                D3D9_D3DUSAGE_WRITEONLY,
                0x144,
                D3D9_D3DPOOL_MANAGED,
                D3D9_FIXTURE_OUTPUT.0,
                0,
            ],
        );
        assert_eq!(vertex.eax, COM_S_OK.bits());
        let vertex_interface = runtime
            .com_interface_for_pointer(read_pointer(&runtime.memory, D3D9_FIXTURE_OUTPUT))
            .unwrap()
            .unwrap()
            .clone();
        let locked = invoke(
            &mut runtime,
            &vertex_interface,
            11,
            &[0, 0, D3D9_FIXTURE_OUTPUT.0, 0],
        );
        assert_eq!(locked.eax, COM_S_OK.bits());
        let data = read_pointer(&runtime.memory, D3D9_FIXTURE_OUTPUT);
        runtime.memory.write(data, &[1, 2, 3, 4]).unwrap();
        acknowledge_d3d9_fixture_memory(&mut runtime.memory);
        let unlocked = invoke(&mut runtime, &vertex_interface, 12, &[]);
        assert_eq!(unlocked.eax, COM_S_OK.bits());
        let render = invoke(&mut runtime, &device_interface, 57, &[7, 0]);
        assert_eq!(render.eax, COM_S_OK.bits());
        let raster = invoke(
            &mut runtime,
            &device_interface,
            19,
            &[0, D3D9_FIXTURE_OUTPUT.0],
        );
        assert_eq!(raster.eax, COM_S_OK.bits());
        let line = u32::from_le_bytes(
            runtime
                .memory
                .read(GuestAddress(D3D9_FIXTURE_OUTPUT.0 + 4), 4)
                .unwrap()
                .try_into()
                .unwrap(),
        );
        assert!(line < 1080);
        assert_eq!(runtime.current_last_error().unwrap(), 0x1234_5678);
    }

    #[test]
    fn default_ime_window_capacity_failure_preserves_both_owners() {
        let mut runtime = d3d9_fixture_runtime(GuestMemory::new(MemoryLimits::default()).unwrap());
        let app = seed_created_d3d9_window(&mut runtime);
        // Exhaust HIMC capacity independently of User32's HWND allocator.
        for index in 0..super::super::imm32::RUNTIME_IME_CONTEXT_LIMIT {
            runtime
                .imm32
                .ensure_window(super::super::user32::RuntimeImeAppWindowTarget {
                    window_id: RuntimeWindowId(100 + index as u64),
                    window_handle: 0x4802_0000 + index as u32,
                    owner_thread_id: RuntimeThreadId(0x10000 + index as u32),
                })
                .unwrap();
        }
        let contexts_before = runtime.ime_contexts().keys().copied().collect::<Vec<_>>();
        let associations_before = runtime.ime_context_associations().clone();
        assert!(runtime.get_default_ime_window(app.handle).is_err());
        assert_eq!(runtime.window(app.handle).unwrap(), &app);
        assert_eq!(
            runtime.ime_contexts().keys().copied().collect::<Vec<_>>(),
            contexts_before
        );
        assert_eq!(runtime.ime_context_associations(), &associations_before);
        assert_eq!(runtime.current_last_error().unwrap(), 0x1234_5678);
        assert_eq!(
            runtime.scheduler_state().unwrap(),
            SchedulerThreadState::Runnable
        );
        assert!(runtime.ime_windows().is_empty());
    }

    #[test]
    fn d3d8_copy_mode_surface_3d_draw_and_incremental_present_semantics() {
        let mut runtime = d3d9_fixture_runtime(d3d9_fixture_memory());
        let window = seed_created_d3d9_window(&mut runtime);
        let root_installed = runtime
            .install_com_object(crate::d3d8::root_spec())
            .expect("install D3D8 root");
        acknowledge_d3d9_fixture_memory(&mut runtime.memory);
        let root_ptr = root_installed
            .interfaces
            .iter()
            .find(|(iid, _)| *iid == crate::d3d8::D3D8_IID)
            .unwrap()
            .1;
        let root_iface = runtime
            .com_interface_for_pointer(root_ptr)
            .unwrap()
            .unwrap()
            .clone();

        let stack = GuestAddress(D3D9_FIXTURE_MEMORY_BASE.0 + 0x200);
        let ret = GuestAddress(0x0042_0e60);
        let pp_addr = GuestAddress(D3D9_FIXTURE_MEMORY_BASE.0 + 0x400);
        let out_addr = GuestAddress(D3D9_FIXTURE_MEMORY_BASE.0 + 0x500);
        let scratch_addr = GuestAddress(D3D9_FIXTURE_MEMORY_BASE.0 + 0x600);

        let call_com = |runtime: &mut Runtime,
                        iface: &RuntimeComInterfaceRecord,
                        slot: u16,
                        args: &[u32]|
         -> crate::com::ComCallCompletion {
            let mut words = vec![ret.0, iface.guest_pointer.0];
            words.extend_from_slice(args);
            write_d3d9_fixture_words(&mut runtime.memory, stack, &words);
            acknowledge_d3d9_fixture_memory(&mut runtime.memory);
            let outcome = runtime.dispatch_com_call(crate::com::ComCallTrap {
                endpoint_address: iface.methods[&slot].endpoint,
                stack_pointer: stack,
                return_address: ret,
                call_site: ret,
            });
            acknowledge_d3d9_fixture_memory(&mut runtime.memory);
            match outcome {
                crate::com::ComDispatchOutcome::Completed(done) => done,
                other => panic!("unexpected COM outcome for slot {slot}: {other:?}"),
            }
        };

        // Reject unsupported presentation flags without publishing a device.
        let mut pp_words = [800_u32, 600, 22, 1, 0, 3, window.handle, 1, 1, 80, 2, 0, 0];
        write_d3d9_fixture_words(&mut runtime.memory, pp_addr, &pp_words);
        acknowledge_d3d9_fixture_memory(&mut runtime.memory);
        let rejected = call_com(
            &mut runtime,
            &root_iface,
            15,
            &[0, 1, window.handle, 0x20, pp_addr.0, out_addr.0],
        );
        assert_eq!(rejected.eax, crate::d3d9::D3D9_D3DERR_NOTAVAILABLE.bits());
        assert!(runtime.graphics.devices().is_empty());

        // Succeed with Flags == 0 (Copy swap effect, D16 depth buffer).
        pp_words[10] = 0;
        write_d3d9_fixture_words(&mut runtime.memory, pp_addr, &pp_words);
        acknowledge_d3d9_fixture_memory(&mut runtime.memory);
        let created = call_com(
            &mut runtime,
            &root_iface,
            15,
            &[0, 1, window.handle, 0x20, pp_addr.0, out_addr.0],
        );
        assert_eq!(created.eax, 0);
        let dev_ptr = GuestAddress(u32::from_le_bytes(
            runtime
                .memory
                .read(out_addr, 4)
                .unwrap()
                .try_into()
                .unwrap(),
        ));
        let dev_iface = runtime
            .com_interface_for_pointer(dev_ptr)
            .unwrap()
            .unwrap()
            .clone();

        // GetAvailableTextureMem (slot 4), ResourceManagerDiscardBytes (slot 5), non-lockable BackBuffer.
        assert!(call_com(&mut runtime, &dev_iface, 4, &[]).eax > 0);
        assert_eq!(call_com(&mut runtime, &dev_iface, 5, &[0]).eax, 0);
        assert_eq!(
            call_com(&mut runtime, &dev_iface, 16, &[0, 0, out_addr.0]).eax,
            0
        );
        let bb_ptr = GuestAddress(u32::from_le_bytes(
            runtime
                .memory
                .read(out_addr, 4)
                .unwrap()
                .try_into()
                .unwrap(),
        ));
        let bb_iface = runtime
            .com_interface_for_pointer(bb_ptr)
            .unwrap()
            .unwrap()
            .clone();
        assert_eq!(
            call_com(&mut runtime, &bb_iface, 9, &[scratch_addr.0, 0, 0]).eax,
            crate::d3d9::D3D9_D3DERR_INVALIDCALL.bits()
        );

        // CreateImageSurface(800, 600, 25 /* A1R5G5B5 */) + GetDesc + LockRect + CopyRects -> full replace.
        assert_eq!(
            call_com(&mut runtime, &dev_iface, 27, &[800, 600, 25, out_addr.0]).eax,
            0
        );
        let surf_ptr = GuestAddress(u32::from_le_bytes(
            runtime
                .memory
                .read(out_addr, 4)
                .unwrap()
                .try_into()
                .unwrap(),
        ));
        let surf_iface = runtime
            .com_interface_for_pointer(surf_ptr)
            .unwrap()
            .unwrap()
            .clone();
        assert_eq!(
            call_com(&mut runtime, &surf_iface, 8, &[scratch_addr.0]).eax,
            0
        );
        let desc_bytes = runtime.memory.read(scratch_addr, 32).unwrap();
        assert_eq!(u32::from_le_bytes(desc_bytes[0..4].try_into().unwrap()), 25);
        assert_eq!(
            u32::from_le_bytes(desc_bytes[16..20].try_into().unwrap()),
            800 * 600 * 2
        );
        assert_eq!(
            call_com(&mut runtime, &surf_iface, 9, &[scratch_addr.0, 0, 0]).eax,
            0
        );
        let locked = runtime.memory.read(scratch_addr, 8).unwrap();
        assert_eq!(
            u32::from_le_bytes(locked[0..4].try_into().unwrap()),
            800 * 2
        );
        let bits_ptr = GuestAddress(u32::from_le_bytes(locked[4..8].try_into().unwrap()));
        runtime.memory.write(bits_ptr, &[0xff, 0xff]).unwrap();
        acknowledge_d3d9_fixture_memory(&mut runtime.memory);
        assert_eq!(call_com(&mut runtime, &surf_iface, 10, &[]).eax, 0);
        assert_eq!(
            call_com(
                &mut runtime,
                &dev_iface,
                28,
                &[surf_ptr.0, 0, 0, bb_ptr.0, 0]
            )
            .eax,
            0
        );
        let p0 = call_com(&mut runtime, &dev_iface, 15, &[0, 0, 0, 0]);
        assert_eq!(p0.eax, 0);
        let dev_id = p0.presented_device.expect("presented device");
        let (_, _, frame0) = runtime.presented_graphics_frame(dev_id).unwrap();
        assert!(!frame0.preserve_previous);
        assert_eq!(
            &frame0.draw_submissions[0].texture_data[0..4],
            &[255, 255, 255, 255]
        );

        // Frame 1: 3D XYZ|TEX1 vertex buffer draw + TFACTOR + additive blend + W-fog + trailing depth clear.
        assert_eq!(
            call_com(&mut runtime, &dev_iface, 23, &[80, 0, 0x102, 1, out_addr.0]).eax,
            0
        );
        let vb_ptr = GuestAddress(u32::from_le_bytes(
            runtime
                .memory
                .read(out_addr, 4)
                .unwrap()
                .try_into()
                .unwrap(),
        ));
        let vb_iface = runtime
            .com_interface_for_pointer(vb_ptr)
            .unwrap()
            .unwrap()
            .clone();
        assert_eq!(
            call_com(&mut runtime, &vb_iface, 11, &[0, 0, scratch_addr.0, 0]).eax,
            0
        );
        let vb_data_ptr = GuestAddress(u32::from_le_bytes(
            runtime
                .memory
                .read(scratch_addr, 4)
                .unwrap()
                .try_into()
                .unwrap(),
        ));
        let mut vb_verts = Vec::with_capacity(80);
        for (vx, vy, vz, u, v) in [
            (-0.5_f32, 0.5_f32, 0.5_f32, 0.0_f32, 0.0_f32),
            (0.5, 0.5, 0.5, 1.0, 0.0),
            (-0.5, -0.5, 0.5, 0.0, 1.0),
            (0.5, -0.5, 0.5, 1.0, 1.0),
        ] {
            for f in [vx, vy, vz, u, v] {
                vb_verts.extend_from_slice(&f.to_le_bytes());
            }
        }
        runtime.memory.write(vb_data_ptr, &vb_verts).unwrap();
        acknowledge_d3d9_fixture_memory(&mut runtime.memory);
        assert_eq!(call_com(&mut runtime, &vb_iface, 12, &[]).eax, 0);

        let mut ident = [0_u32; 16];
        for idx in [0, 5, 10, 15] {
            ident[idx] = 1.0_f32.to_bits();
        }
        write_d3d9_fixture_words(&mut runtime.memory, scratch_addr, &ident);
        acknowledge_d3d9_fixture_memory(&mut runtime.memory);
        for kind in [2_u32, 3, 256] {
            assert_eq!(
                call_com(&mut runtime, &dev_iface, 37, &[kind, scratch_addr.0]).eax,
                0
            );
        }
        for (state, val) in [
            (22_u32, 1_u32),
            (137, 0),
            (27, 1),
            (19, 5),
            (20, 2),
            (28, 1),
            (35, 3),
            (34, 0xff10_2030),
            (36, 0.5_f32.to_bits()),
            (37, 2.0_f32.to_bits()),
            (60, 0x8040_2010),
        ] {
            assert_eq!(call_com(&mut runtime, &dev_iface, 50, &[state, val]).eax, 0);
        }
        for (kind, val) in [(1_u32, 2_u32), (2, 3), (4, 2), (5, 3)] {
            assert_eq!(
                call_com(&mut runtime, &dev_iface, 63, &[0, kind, val]).eax,
                0
            );
        }
        assert_eq!(call_com(&mut runtime, &dev_iface, 76, &[0x102]).eax, 0);
        assert_eq!(
            call_com(&mut runtime, &dev_iface, 83, &[0, vb_ptr.0, 20]).eax,
            0
        );
        assert_eq!(call_com(&mut runtime, &dev_iface, 34, &[]).eax, 0);
        assert_eq!(call_com(&mut runtime, &dev_iface, 70, &[5, 0, 2]).eax, 0);
        assert_eq!(call_com(&mut runtime, &dev_iface, 35, &[]).eax, 0);
        // Present must replay a depth clear issued after EndScene.
        assert_eq!(
            call_com(
                &mut runtime,
                &dev_iface,
                36,
                &[0, 0, 2, 0, 1.0_f32.to_bits(), 0]
            )
            .eax,
            0
        );
        let p1 = call_com(&mut runtime, &dev_iface, 15, &[0, 0, 0, 0]);
        assert_eq!(p1.eax, 0);
        let (_, _, frame1) = runtime.presented_graphics_frame(dev_id).unwrap();
        assert!(frame1.preserve_previous);
        assert_eq!(frame1.draw_submissions.len(), 1);
        assert_eq!(frame1.depth_clears.len(), 1);
        assert_eq!(frame1.depth_clears[0].before_draw, 1);
        let frag = frame1.draw_submissions[0].fragment.as_ref().unwrap();
        assert!(frag.additive_blend);
        assert!(frag.fog.is_some());
    }

    fn d3d9_fixture_runtime(memory: GuestMemory) -> Runtime {
        let mut runtime = Runtime::new(
            RuntimeBootstrap {
                process_id: RuntimeProcessId(0x1000),
                initial_thread_id: RuntimeThreadId(0x2000),
                initial_last_error: 0x1234_5678,
                clock: RuntimeClockState {
                    wall_clock_file_time: 134_282_016_000_000_000,
                    tick_count: 3_600_000,
                    performance_counter: 12_345_678_901,
                    performance_frequency: 10_000_000,
                },
                startup_info: RuntimeStartupInfo {
                    bytes: vec![0; X86_STARTUP_INFO_W_BYTE_LENGTH],
                },
                process_image_path_a: RuntimeProcessImagePathA {
                    bytes_with_nul: b"C:\\Vine\\fixture.exe\0".to_vec(),
                },
                initial_current_directory_a: RuntimeCurrentDirectoryA {
                    bytes_with_nul: b"C:\\Vine\0".to_vec(),
                },
                encoded_pointer_cookie: 0xec01_9d3b,
            },
            memory,
        )
        .expect("D3D9 transaction runtime");
        runtime
            .start_initial_thread()
            .expect("start fixture thread");
        runtime
            .configure_display_profile(RuntimeDisplayProfile {
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

    fn seed_created_d3d9_window(runtime: &mut Runtime) -> RuntimeWindow {
        let module = RuntimeWindowClassModule {
            module_id: RuntimeModuleId(1),
            load_base: GuestAddress(0x1000_0000),
        };
        let class = runtime
            .user32
            .register_window_class(RuntimeWindowClassRegistration {
                unicode: false,
                module: module.clone(),
                style: 0x28,
                window_procedure: RuntimeWindowProcedure {
                    runtime_default: false,
                    module_id: module.module_id,
                    address: GuestAddress(0x1009_2550),
                    rva: Rva(0x0009_2550),
                },
                class_extra_bytes: 0,
                window_extra_bytes: 0,
                icon: None,
                cursor: None,
                background_brush: 0,
                menu_name: None,
                class_name: "Graphics fixture window".to_owned(),
                small_icon: None,
            })
            .expect("register fixture window class");
        let class_id = runtime.user32.window_class(class).unwrap().id;
        let creation_plan = runtime
            .user32
            .checked_window_creation(RuntimeWindowCreationRequest {
                owner_thread_id: RuntimeThreadId(0x2000),
                class: RuntimeWindowClassReference {
                    class_id,
                    atom: class,
                    module_id: module.module_id,
                },
                instance: module,
                name: RuntimeWindowNameA {
                    value: "Graphics fixture".to_owned(),
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
        let created = runtime.user32.publish_window_creation(creation_plan);
        for (message, result) in [
            (RuntimeWindowCreationMessage::NonClientCreate, 1),
            (RuntimeWindowCreationMessage::NonClientCalculate, 0),
            (RuntimeWindowCreationMessage::Create, 0),
            (RuntimeWindowCreationMessage::Size, 0),
            (RuntimeWindowCreationMessage::Move, 0),
        ] {
            runtime
                .user32
                .complete_window_creation_callback(created.0, message, result)
                .expect("complete fixture window callback");
        }
        runtime
            .window(created.1)
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
        runtime: &mut Runtime,
        interface: &RuntimeComInterfaceRecord,
        pointer: GuestAddress,
    ) -> crate::com::ComCallCompletion {
        write_d3d9_fixture_words(
            &mut runtime.memory,
            D3D9_FIXTURE_STACK,
            &[D3D9_FIXTURE_CALL_SITE.0, pointer.0],
        );
        acknowledge_d3d9_fixture_memory(&mut runtime.memory);
        match runtime.dispatch_com_call(ComCallTrap {
            endpoint_address: interface.methods[&2].endpoint,
            stack_pointer: D3D9_FIXTURE_STACK,
            return_address: D3D9_FIXTURE_CALL_SITE,
            call_site: D3D9_FIXTURE_CALL_SITE,
        }) {
            ComDispatchOutcome::Completed(completion) => completion,
            ComDispatchOutcome::AudioPending(pending) => {
                match runtime.complete_com_audio(pending, true) {
                    ComDispatchOutcome::Completed(completion) => completion,
                    other => panic!("audio Release acknowledgement failed: {other:?}"),
                }
            }
            other => panic!("COM Release did not complete: {other:?}"),
        }
    }

    fn dispatch_d3d9_fixture_basic(
        runtime: &mut Runtime,
        interface: &RuntimeComInterfaceRecord,
        slot: u16,
        call_site: GuestAddress,
    ) -> ComDispatchOutcome {
        runtime.dispatch_com_call(ComCallTrap {
            endpoint_address: interface.methods[&slot].endpoint,
            stack_pointer: D3D9_FIXTURE_STACK,
            return_address: call_site,
            call_site,
        })
    }
}

#[cfg(test)]
mod system_module_load_tests {
    use super::*;
    use crate::module_graph::{RuntimeModuleLoadState, RuntimeModuleReferenceOwner};

    const KERNEL32_HANDLE: u32 = 0x4d01_0001;
    const MAIN_HANDLE: u32 = 0x4d01_0002;

    #[test]
    fn module_handle_lookup_distinguishes_absence_from_a_live_module() {
        let runtime = attached_runtime();
        for handle in [0, u32::MAX, 0x4d01_0099] {
            assert!(runtime.module_option(handle).unwrap().is_none());
        }
        assert_eq!(
            runtime
                .module_option(MAIN_HANDLE)
                .unwrap()
                .unwrap()
                .canonical_name,
            "fixture.exe"
        );
    }

    #[test]
    fn dynamic_runtime_system_load_owns_identity_reference_and_attached_state() {
        let mut runtime = attached_runtime();

        let first = runtime
            .load_current_thread_runtime_system_module(
                "D3DX9_43.DLL".to_owned(),
                "d3dx9_43.dll".to_owned(),
            )
            .expect("first runtime-system load");
        assert_eq!(first, 0x4d01_0003);
        assert_eq!(
            runtime
                .object_for_handle(RuntimeHandle(first), RuntimeObjectKind::Module)
                .unwrap()
                .key,
            crate::runtime::RuntimeObjectKey::Module { handle: first }
        );
        let node = runtime
            .module_graph()
            .module_by_handle(first)
            .expect("loaded graph node");
        assert_eq!(node.class, RuntimeModuleClass::RuntimeSystemModule);
        assert_eq!(
            node.state,
            RuntimeModuleLoadState::ProcessAttached { sequence: 3 }
        );
        assert_eq!(
            node.reference_count_for(&RuntimeModuleReferenceOwner::RuntimeService),
            1
        );
        assert_eq!(
            node.reference_count_for(&RuntimeModuleReferenceOwner::DynamicLoad { thread_id: 8192 }),
            1
        );

        let object_id = runtime
            .object_for_handle(RuntimeHandle(first), RuntimeObjectKind::Module)
            .unwrap()
            .id;
        let second = runtime
            .load_current_thread_runtime_system_module(
                "d3dx9_43.dll".to_owned(),
                "d3dx9_43.dll".to_owned(),
            )
            .expect("repeated runtime-system load");
        assert_eq!(second, first);
        assert_eq!(
            runtime
                .object_for_handle(RuntimeHandle(second), RuntimeObjectKind::Module)
                .unwrap()
                .id,
            object_id
        );
        assert_eq!(
            runtime
                .module_graph()
                .module_by_handle(first)
                .unwrap()
                .reference_count_for(&RuntimeModuleReferenceOwner::DynamicLoad { thread_id: 8192 }),
            2
        );
    }

    #[test]
    fn attached_static_system_module_reuse_preserves_graph_order_and_object_identity() {
        let mut runtime = attached_runtime();
        let before = runtime
            .module_graph()
            .module_by_handle(KERNEL32_HANDLE)
            .unwrap()
            .clone();
        let object_id = runtime
            .object_for_handle(RuntimeHandle(KERNEL32_HANDLE), RuntimeObjectKind::Module)
            .unwrap()
            .id;
        for count in [1, 2] {
            assert_eq!(
                runtime
                    .load_current_thread_runtime_system_module(
                        "KERNEL32.DLL".to_owned(),
                        "kernel32.dll".to_owned()
                    )
                    .unwrap(),
                KERNEL32_HANDLE
            );
            let after = runtime
                .module_graph()
                .module_by_handle(KERNEL32_HANDLE)
                .unwrap();
            assert_eq!(after.state, before.state);
            assert_eq!(after.dependencies, before.dependencies);
            assert_eq!(after.dependents, before.dependents);
            assert_eq!(
                after.reference_count_for(&RuntimeModuleReferenceOwner::DynamicLoad {
                    thread_id: 8192
                }),
                count
            );
            assert_eq!(
                runtime
                    .object_for_handle(RuntimeHandle(KERNEL32_HANDLE), RuntimeObjectKind::Module)
                    .unwrap()
                    .id,
                object_id
            );
        }
    }

    fn attached_runtime() -> Runtime {
        let mut runtime = Runtime::new(
            RuntimeBootstrap {
                process_id: RuntimeProcessId(4096),
                initial_thread_id: RuntimeThreadId(8192),
                initial_last_error: 0,
                clock: RuntimeClockState {
                    wall_clock_file_time: 134_282_016_000_000_000,
                    tick_count: 3_600_000,
                    performance_counter: 12_345_678_901,
                    performance_frequency: 10_000_000,
                },
                startup_info: RuntimeStartupInfo {
                    bytes: vec![0; X86_STARTUP_INFO_W_BYTE_LENGTH],
                },
                process_image_path_a: crate::RuntimeProcessImagePathA {
                    bytes_with_nul: b"C:\\Vine\\fixture.exe\0".to_vec(),
                },
                initial_current_directory_a: crate::RuntimeCurrentDirectoryA {
                    bytes_with_nul: b"C:\\Vine\0".to_vec(),
                },
                encoded_pointer_cookie: 0xec01_9d3b,
            },
            GuestMemory::new(crate::MemoryLimits::default()).unwrap(),
        )
        .unwrap();
        runtime.start_initial_thread().unwrap();
        runtime
            .load_module(module(KERNEL32_HANDLE, "kernel32.dll", true))
            .unwrap();
        runtime
            .register_main_module(module(MAIN_HANDLE, "fixture.exe", false))
            .unwrap();
        runtime
            .add_module_dependency(MAIN_HANDLE, KERNEL32_HANDLE)
            .unwrap();
        runtime.mark_module_ready(KERNEL32_HANDLE).unwrap();
        runtime.mark_module_ready(MAIN_HANDLE).unwrap();
        assert_eq!(runtime.attach_modules().unwrap().len(), 2);
        runtime
    }

    fn module(handle: u32, name: &str, runtime_system_module: bool) -> RuntimeModule {
        RuntimeModule {
            handle,
            requested_name: name.to_owned(),
            canonical_name: name.to_owned(),
            runtime_system_module,
        }
    }
}

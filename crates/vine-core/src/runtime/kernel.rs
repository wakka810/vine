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
    GuestRange, MemoryInvalidation, MemoryOwnerId, MemoryRegionKind, PageProtection,
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
use super::com::{
    RUNTIME_COM_ADDRESS_OWNER, RuntimeCom, RuntimeComIid, RuntimeComInterfaceId,
    RuntimeComInterfaceRecord, RuntimeComMethodTarget, RuntimeComMutationPlan, RuntimeComObjectId,
    RuntimeComObjectRegistration, RuntimeComReferenceResult, RuntimeComRegistrationResult,
};
use super::displays::{RuntimeDisplayProfile, RuntimeDisplays, RuntimeLogicalMonitor};
use super::files::{
    RUNTIME_SOURCE_READ_SERVICE, RuntimeFiles, RuntimeSourceFile, RuntimeSourceFileRetirementPlan,
    RuntimeSourceReadBeginPlan, RuntimeSourceReadCancellationPlan, RuntimeSourceReadCompletion,
    RuntimeSourceReadCompletionPlan, RuntimeSourceReadHostResponse, RuntimeSourceReadRequest,
    RuntimeSourceSnapshot, RuntimeSourceSnapshotManifest,
};
use super::gdi::{
    RuntimeDeviceContext, RuntimeDeviceContextId, RuntimeDeviceContextTarget, RuntimeGdi,
    RuntimeGdiComRetirementGuard, RuntimeGdiGraphicsSurfaceTarget, RuntimeGdiObject,
    RuntimeGdiObjectId, RuntimeGdiSurface, RuntimeGdiSurfaceId, RuntimeGdiWindowTarget,
    RuntimeGetDcResult, RuntimeGetGraphicsSurfaceDcPlan, RuntimeGetGraphicsSurfaceDcResult,
    RuntimeGetStockObjectResult,
};
use super::graphics::{
    RUNTIME_GRAPHICS_RESOURCE_ADDRESS_OWNER, RuntimeGraphics, RuntimeGraphicsComRetirementPlan,
    RuntimeGraphicsDevice, RuntimeGraphicsDeviceCreateRequest, RuntimeGraphicsDeviceCreationPlan,
    RuntimeGraphicsDeviceCreationRequest, RuntimeGraphicsDeviceId, RuntimeGraphicsSurface,
    RuntimeGraphicsSurfaceCreateRequest, RuntimeGraphicsSurfaceCreationPlan,
    RuntimeGraphicsSurfaceFillRequest, RuntimeGraphicsSurfaceId, RuntimeGraphicsTexture,
    RuntimeGraphicsTextureCreateRequest, RuntimeGraphicsTextureCreationPlan,
    RuntimeGraphicsTextureId, RuntimeGraphicsVertexBuffer,
    RuntimeGraphicsVertexBufferCreateRequest, RuntimeGraphicsVertexBufferCreationPlan,
    RuntimeGraphicsVertexBufferId, RuntimeGraphicsVertexBufferLock,
    RuntimeGraphicsVertexBufferLockPlan, RuntimeGraphicsVertexBufferLockRequest,
    RuntimeGraphicsVertexBufferUnlockPlan, RuntimeGraphicsVertexBufferUnlockResult,
    RuntimeGraphicsWindowTarget,
};
use super::heaps::{
    RuntimeHeap, RuntimeHeapAllocation, RuntimeHeapAllocationPlan, RuntimeHeapBacking,
    RuntimeHeapCreationPlan, RuntimeHeapDestructionPlan, RuntimeHeapFreePlan,
    RuntimeHeapInitialization, RuntimeHeapPageMaterialization, RuntimeHeapTeardown, RuntimeHeaps,
    RuntimeLargeHeapAllocationBacking,
};
use super::input::{
    RuntimeInput, RuntimeInputProfile, RuntimeMouseButtonMapping, RuntimeSwapMouseButtonResult,
};
use super::messages::{RuntimeMessageClearPlan, RuntimeMessageReceivePlan, RuntimeMessages};
use super::messages::{RuntimeMessageReceiveDisposition, RuntimeMessageWake, RuntimeThreadMessage};
#[cfg(test)]
use super::modules::LOGICAL_LOADED_MODULE_NAMESPACE;
use super::modules::{
    RuntimeDllLifecycleAction, RuntimeDllLifecyclePhase, RuntimeDllLifecyclePlan,
    RuntimeGuestImport, RuntimeGuestImportBinding, RuntimeGuestImportBindingPlan, RuntimeModule,
    RuntimeModuleDependencyEdge, RuntimeModuleRecords, RuntimeModules, RuntimeSystemExportTable,
    RuntimeSystemModuleLoad, runtime_module_image_error, runtime_module_image_state_error,
};
use super::objects::{
    RuntimeCloseHandleObjectMutationPlan, RuntimeHandle, RuntimeHandleEntry, RuntimeObjectId,
    RuntimeObjectKey, RuntimeObjectKind, RuntimeObjectRecord, RuntimeObjectTable, RuntimeWaitState,
};
use super::process::{
    RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING, RUNTIME_PROCESS_IMAGE_PATH_A_MAX_BYTES,
    RuntimeClockState, RuntimeCommandLine, RuntimeCurrentDirectoryA, RuntimeEnvironmentStringCopy,
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
    RuntimeHostCompletion, RuntimeHostRequest, RuntimeScheduler, RuntimeSchedulerEvent,
    RuntimeThreadExecutionSelection, SchedulerThreadState,
};
use super::scheduler::{RuntimeHostRequestCancellationPlan, RuntimeSchedulerLanePlan};
use super::synchronization::RuntimeSynchronization;
use super::synchronization::{
    RuntimeCreateMutexAResult, RuntimeCriticalSection, RuntimeMutex, RuntimeMutexId,
    RuntimeMutexNameA, RuntimeMutexRetirementPlan,
};
use super::thread::{
    RuntimeCallbackFrame, RuntimeCallbackFrameId, RuntimeCallbackFrameIdPlan,
    RuntimeCallbackScratch, RuntimeCancellation, RuntimeComApartmentModel,
    RuntimeComApartmentState, RuntimeComInitializationOutcome, RuntimeComUninitializationOutcome,
    RuntimeFlsSlot, RuntimeThreadId, RuntimeThreadLifecycleState, RuntimeThreadMutationPlan,
    RuntimeThreadStack, RuntimeThreadStackGrowth, RuntimeThreadState, RuntimeThreads,
    RuntimeTlsValue,
};
use super::timers::{
    RuntimeCreateWaitableTimerAResult, RuntimeTimerResolutionMutationResult,
    RuntimeTimerResolutionState, RuntimeTimerResolutionTeardownPlan, RuntimeTimers,
    RuntimeWaitableTimer, RuntimeWaitableTimerId, RuntimeWaitableTimerRetirementPlan,
};
use super::user32::{
    RuntimeCreateWindowExAResult, RuntimeCursor, RuntimeCursorId, RuntimeCursorSource, RuntimeIcon,
    RuntimeIconId, RuntimeIconResource, RuntimeLoadCursorAResult, RuntimeLoadIconAResult,
    RuntimeRegisterClassExAResult, RuntimeSetWindowPosCallbackAction,
    RuntimeSetWindowPosCallbackMessage, RuntimeSetWindowPosDefaultCallbackAction,
    RuntimeSetWindowPosDefaultCallbackMessage, RuntimeSetWindowPosDefaultOperation,
    RuntimeSetWindowPosOperation, RuntimeSetWindowPosRequest, RuntimeSetWindowStyleResult,
    RuntimeUser32, RuntimeWindow, RuntimeWindowClass, RuntimeWindowClassId,
    RuntimeWindowClassRegistration, RuntimeWindowCreationCallbackAction,
    RuntimeWindowCreationMessage, RuntimeWindowCreationRequest, RuntimeWindowId,
    RuntimeWindowLifecycle, RuntimeWindowProcedure,
};
use super::waits::{RuntimeWaitCompletion, RuntimeWaitDisposition, RuntimeWaitRegistration};

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
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum RuntimePageFaultResolution {
    InitialThreadStackGrowth {
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
    timer_resolution_teardown: Option<RuntimeTimerResolutionTeardownPlan>,
    cancellation: RuntimeProcessCancellation,
}

struct RuntimePayloadObjectTeardownPlan {
    object_plan: Option<RuntimeCloseHandleObjectMutationPlan>,
    mutex_plans: Vec<RuntimeMutexRetirementPlan>,
    waitable_timer_plans: Vec<RuntimeWaitableTimerRetirementPlan>,
    source_file_plans: Vec<RuntimeSourceFileRetirementPlan>,
    retired_handles: usize,
}

pub(crate) struct RuntimeSourceReadAggregateCompletionPlan {
    scheduler: super::scheduler::RuntimeHostCompletionPlan,
    source_read: RuntimeSourceReadCompletionPlan,
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
    user32: RuntimeUser32,
    gdi: RuntimeGdi,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    timer_resolution: Option<RuntimeTimerResolutionState>,
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
            timer_resolution: self.timers.state(),
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
    user32: Option<&'a RuntimeUser32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    gdi: Option<&'a RuntimeGdi>,
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
                timer_resolution: self.timers.state(),
            },
            address_space_allocator: &self.address_space_allocator,
            threads: &self.threads,
            scheduler: &self.scheduler,
            messages: &self.messages,
            displays: &self.displays,
            input: &self.input,
            user32: (!self.user32.is_empty()).then_some(&self.user32),
            gdi: (!self.gdi.is_empty()).then_some(&self.gdi),
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
            user32: RuntimeUser32::default(),
            gdi: RuntimeGdi::default(),
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

    pub fn open_source_file_a(
        &mut self,
        requested_path: &[u8],
        desired_access: u32,
        share_mode: u32,
        flags_and_attributes: u32,
    ) -> Result<Option<super::RuntimeOpenSourceFileResult>, RuntimeStateError> {
        self.require_source_file_operation_state()?;
        let Some(plan) = self.files.plan_open(
            requested_path,
            &self.process.current_directory_a().bytes_with_nul,
            desired_access,
            share_mode,
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
        self.objects
            .commit_close_handle_object_mutation(object_plan)?;
        let file = self.files.commit_open(plan);
        Ok(Some(super::RuntimeOpenSourceFileResult { file, handle }))
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

    pub fn begin_source_file_read(
        &mut self,
        handle: RuntimeHandle,
        buffer: GuestAddress,
        bytes_read: GuestAddress,
        requested_bytes: u32,
    ) -> Result<RuntimeSourceReadRequest, RuntimeStateError> {
        self.require_source_file_operation_state()?;
        let file_id = self.source_file_for_handle(handle)?.id;
        let scheduler_plan = self
            .scheduler
            .plan_host_request(RUNTIME_SOURCE_READ_SERVICE.to_owned())?;
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
        debug_assert_eq!(runtime_request.service, RUNTIME_SOURCE_READ_SERVICE);
        Ok(source_request)
    }

    pub(crate) fn plan_source_file_read_completion(
        &self,
        response: RuntimeSourceReadHostResponse,
    ) -> Result<RuntimeSourceReadAggregateCompletionPlan, RuntimeStateError> {
        let scheduler = self.scheduler.plan_host_completion(response.request_id)?;
        if scheduler.request().service != RUNTIME_SOURCE_READ_SERVICE {
            return Err(RuntimeStateError::invalid_input(
                "runtime-vfs",
                "source-read-host-service-mismatch",
                "source read completion belongs to another host service",
            ));
        }
        let source_read = self.files.plan_read_completion(response)?;
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
        let graphics = self
            .graphics
            .plan_com_object_retirements(&retired_object_ids)?;
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
            let (com_allocations, graphics_allocations) =
                plan.allocations().split_at(result.retirements.len());
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
            let graphics_arena = self
                .address_space_allocator
                .allocation_for_purpose(&ProcessAddressAllocationPurpose::GraphicsResourceArena)?;
            for ((backing, usable_byte_length), released) in
                vertex_buffer_backings.iter().zip(graphics_allocations)
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
            vertex_buffer_backings
                .iter()
                .map(|(backing, _)| (backing.memory_owner, backing.range)),
        );
        let gdi = self.gdi.plan_com_object_retirements(&retired_object_ids)?;
        Ok(RuntimeComReleasePlan {
            com,
            address_space,
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
        if !self.process.is_running()
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
                .grow_initial_thread_stack(memory, fault_address, page_fault_error_code)
                .map(|growth| RuntimePageFaultResolution::InitialThreadStackGrowth { growth });
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

    pub fn sample_tick_count(&mut self, increment_per_read: u32) -> Result<u32, RuntimeStateError> {
        let plan = self.process.plan_tick_count_sample(increment_per_read)?;
        self.process.commit_tick_count_sample(plan)
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
        let Some(plan) = self.scheduler.plan_next_executable_selection()? else {
            return Ok(None);
        };
        let thread_id = plan.next();
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

    pub fn cursors(&self) -> &BTreeMap<RuntimeCursorId, RuntimeCursor> {
        self.user32.cursors()
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

    pub fn window_by_id(&self, id: RuntimeWindowId) -> Result<&RuntimeWindow, RuntimeStateError> {
        self.user32.window_by_id(id)
    }

    pub fn windows(&self) -> &BTreeMap<RuntimeWindowId, RuntimeWindow> {
        self.user32.windows()
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
        let plan = self.gdi.plan_release_dc(
            window_handle,
            device_context_handle,
            self.current_thread_id(),
        )?;
        let current_target = self.resolve_gdi_window_target(window_handle)?;
        validate_device_context_target(&device_context, current_target)?;
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
        self.user32.commit_set_window_pos_callback(plan)
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

    fn plan_payload_object_teardown(
        &self,
        cancelling_source_reads: &BTreeSet<u64>,
    ) -> Result<RuntimePayloadObjectTeardownPlan, RuntimeStateError> {
        let mutex_ids = self.synchronization.mutex_ids();
        let waitable_timer_ids = self.timers.waitable_timer_ids();
        let source_file_ids = self.files.open_file_ids().collect::<Vec<_>>();
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
            source_file_ids
                .iter()
                .map(|id| RuntimeObjectKey::File { file_id: id.0 }),
        );
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
        let object_plan = (!keys.is_empty())
            .then(|| self.objects.plan_close_handle_object_teardown(&keys))
            .transpose()?;
        let retired_handles = object_plan
            .as_ref()
            .map_or(0, RuntimeCloseHandleObjectMutationPlan::retired_handles);
        Ok(RuntimePayloadObjectTeardownPlan {
            object_plan,
            mutex_plans,
            waitable_timer_plans,
            source_file_plans,
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
        Ok(self.synchronization.commit(plan))
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
        Ok(self.synchronization.commit(plan))
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
        Ok(self.synchronization.commit(plan))
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
        Ok(self.synchronization.commit(plan))
    }

    pub fn critical_section(
        &self,
        address: GuestAddress,
    ) -> Result<&RuntimeCriticalSection, RuntimeStateError> {
        self.synchronization.critical_section(address)
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
        let user32_teardown = self.user32.plan_process_teardown()?;
        self.process.preflight_mutation_commit(&process_plan)?;
        if let Some(gdi_plan) = gdi_teardown.as_ref() {
            self.gdi.preflight_process_teardown(gdi_plan)?;
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
            .flat_map(|thread| thread.source_read_cancellation.request_ids())
            .collect::<BTreeSet<_>>();
        let payload_teardown = self.plan_payload_object_teardown(&cancelling_source_reads)?;
        let graphics_teardown = self.graphics.plan_process_teardown()?;
        let gdi_teardown = self.gdi.plan_process_teardown()?;
        let user32_teardown = self.user32.plan_process_teardown()?;
        self.process.preflight_mutation_commit(&process_plan)?;
        if let Some(timer_plan) = plan.timer_resolution_teardown.as_ref() {
            self.timers.preflight_process_teardown(timer_plan)?;
        }
        if let Some(gdi_plan) = gdi_teardown.as_ref() {
            self.gdi.preflight_process_teardown(gdi_plan)?;
        }
        if let Some(graphics_plan) = graphics_teardown.as_ref() {
            self.graphics.preflight_process_teardown(graphics_plan)?;
        }
        if let Some(user32_plan) = user32_teardown.as_ref() {
            self.user32.preflight_process_teardown(user32_plan)?;
        }
        let cancellation = plan.cancellation.clone();
        if let Some(timer_plan) = plan.timer_resolution_teardown {
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
        let source_read_cancellation = self
            .files
            .plan_read_cancellation(host_request_cancellation.request_ids());
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
            timer_resolution_teardown: self.timers.plan_process_teardown()?,
            cancellation,
        })
    }

    fn commit_process_exit_request(
        &mut self,
        plan: RuntimeProcessExitRequestPlan,
    ) -> Result<(), RuntimeStateError> {
        self.process.preflight_mutation_commit(&plan.process_plan)?;
        if let Some(timer_plan) = plan.timer_resolution_teardown.as_ref() {
            self.timers.preflight_process_teardown(timer_plan)?;
        }
        if let Some(timer_plan) = plan.timer_resolution_teardown {
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
        install_com_object,
    };
    use crate::d3d9::{
        D3D9_CREATE_HARDWARE_VERTEX_PROCESSING_MULTITHREADED, D3D9_D3DDEVTYPE_HAL,
        D3D9_D3DERR_INVALIDCALL, D3D9_D3DERR_NOTAVAILABLE, D3D9_D3DFMT_D16, D3D9_D3DFMT_UNKNOWN,
        D3D9_D3DFMT_X8R8G8B8, D3D9_D3DMULTISAMPLE_NONE, D3D9_D3DPOOL_DEFAULT, D3D9_D3DPOOL_MANAGED,
        D3D9_D3DPOOL_SYSTEMMEM, D3D9_D3DSWAPEFFECT_DISCARD, D3D9_D3DUSAGE_DYNAMIC,
        D3D9_D3DUSAGE_WRITEONLY, D3D9_IDIRECT3D9_IID, D3D9_IDIRECT3DDEVICE9_IID,
        D3D9_IDIRECT3DSURFACE9_IID, D3D9_IDIRECT3DTEXTURE9_IID, D3D9_IDIRECT3DVERTEXBUFFER9_IID,
        D3d9CompatibilityPolicy, direct3d9_object_spec, dispatch_d3d9_com_call,
    };
    use crate::memory::{MemoryLimits, MemoryOwnerKind};
    use crate::runtime::{
        RUNTIME_GDI_BLACK_BRUSH_INDEX, RUNTIME_GRAPHICS_SURFACE_ID_NAMESPACE,
        RUNTIME_WINDOW_CLASS_NAME_A_ENCODING, RUNTIME_WINDOW_NAME_A_ENCODING,
        RUNTIME_WINDOW_PROCEDURE_ARGUMENT_BYTES, RuntimeGraphicsColorFormat,
        RuntimeGraphicsPresentMode, RuntimeGraphicsTexturePool, RuntimeGraphicsTextureUsage,
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
    const D3D9_FIXTURE_CALL_SITE: GuestAddress = GuestAddress(0x1005_aca5);

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
                &D3d9CompatibilityPolicy::controlled_v13(),
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
        acknowledge_d3d9_fixture_memory(&mut memory);

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
                    &D3d9CompatibilityPolicy::controlled_v13(),
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
        let mut disabled = D3d9CompatibilityPolicy::controlled_v13();
        disabled.create_texture = None;
        disabled.get_surface_level = None;
        disabled.surface_dc = None;
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
                &D3d9CompatibilityPolicy::controlled_v13(),
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
        for (call_site, usage, pool, expected_usage, expected_pool) in [
            (
                GuestAddress(0x1005_a7b7),
                0,
                D3D9_D3DPOOL_DEFAULT,
                RuntimeGraphicsTextureUsage::None,
                RuntimeGraphicsTexturePool::Default,
            ),
            (
                GuestAddress(0x1005_a7dc),
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
                    1_024,
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
                    &D3d9CompatibilityPolicy::controlled_v13(),
                    ComCallTrap {
                        endpoint_address: device_interface.methods[&23].endpoint,
                        stack_pointer: D3D9_FIXTURE_STACK,
                        return_address: call_site,
                        call_site,
                    },
                    &mut session,
                ) {
                    ComDispatchOutcome::Completed(completion) => completion,
                    other => panic!("initial CreateTexture did not complete: {other:?}"),
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
            assert_eq!(logical_texture.descriptor.width, 1_024);
            assert_eq!(logical_texture.descriptor.height, 1_024);
            assert_eq!(logical_texture.descriptor.levels, 1);
            additional_texture_interfaces.push(interface);
        }
        assert_eq!(runtime.graphics_textures().len(), 3);
        assert_eq!(
            runtime
                .com()
                .object(device_object.id)
                .unwrap()
                .reference_count,
            4
        );

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
                    &D3d9CompatibilityPolicy::controlled_v13(),
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
                &D3d9CompatibilityPolicy::controlled_v13(),
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
            5
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
                &D3d9CompatibilityPolicy::controlled_v13(),
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
                &D3d9CompatibilityPolicy::controlled_v13(),
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
                &D3d9CompatibilityPolicy::controlled_v13(),
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
                &D3d9CompatibilityPolicy::controlled_v13(),
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
                &D3d9CompatibilityPolicy::controlled_v13(),
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
            4
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
                &D3d9CompatibilityPolicy::controlled_v13(),
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
                    &D3d9CompatibilityPolicy::controlled_v13(),
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
        let mut disabled_surface = D3d9CompatibilityPolicy::controlled_v13();
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
                &D3d9CompatibilityPolicy::controlled_v13(),
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
            &D3d9CompatibilityPolicy::controlled_v13(),
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

        let mut disabled_surface_dc = D3d9CompatibilityPolicy::controlled_v13();
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
            &D3d9CompatibilityPolicy::controlled_v13(),
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
            &D3d9CompatibilityPolicy::controlled_v13(),
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
            &D3d9CompatibilityPolicy::controlled_v13(),
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
        let filled_surface = runtime.graphics_surfaces().get(&surface.id).unwrap();
        assert_eq!(filled_surface.fills.len(), 1);
        assert_eq!(filled_surface.fills[0].rect, fill_rect);
        assert_eq!(filled_surface.fills[0].color_ref, 0);
        assert_eq!(filled_surface.content_generation, 1);
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
            &D3d9CompatibilityPolicy::controlled_v13(),
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
            &D3d9CompatibilityPolicy::controlled_v13(),
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
        assert_eq!(runtime.graphics_surfaces()[&surface.id].fills.len(), 1);

        let before_repeated_release = runtime.clone();
        let repeated_release_dc = match dispatch_d3d9_fixture_frontend(
            &mut memory,
            &mut runtime,
            &D3d9CompatibilityPolicy::controlled_v13(),
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
            &D3d9CompatibilityPolicy::controlled_v13(),
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
            &D3d9CompatibilityPolicy::controlled_v13(),
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
                &D3d9CompatibilityPolicy::controlled_v13(),
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
            &D3d9CompatibilityPolicy::controlled_v13(),
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
            &D3d9CompatibilityPolicy::controlled_v13(),
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
                id: "single-monitor-1920x1080-v1".to_owned(),
                primary_monitor_rect: RuntimeRect {
                    left: 0,
                    top: 0,
                    right: 1920,
                    bottom: 1080,
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

//! Runtime aggregate root and cross-subsystem transaction coordinator.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::ops::Deref;

use serde::{Serialize, Serializer};

use crate::bootstrap::InitialThreadStack;
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
use crate::pe32::{ExportLookup, LoaderIngressReport};
use crate::snapshot::ModuleIdentity;

use super::address_space::{
    ProcessAddressAllocationClassification, ProcessAddressAllocationLifetime,
    ProcessAddressAllocationPurpose, ProcessAddressAllocationReleasePolicy,
    ProcessAddressAllocationRequest, ProcessAddressReservationPlan, ProcessAddressSpaceAllocator,
    ProcessAddressSpaceLayout, RuntimeAddressRange,
};
use super::com::{
    RUNTIME_COM_ADDRESS_OWNER, RuntimeCom, RuntimeComInterfaceId, RuntimeComInterfaceRecord,
    RuntimeComMethodTarget, RuntimeComMutationPlan, RuntimeComObjectRegistration,
    RuntimeComReferenceResult, RuntimeComRegistrationResult,
};
use super::displays::{RuntimeDisplayProfile, RuntimeDisplays, RuntimeLogicalMonitor};
use super::heaps::{
    RuntimeHeap, RuntimeHeapAllocation, RuntimeHeapAllocationPlan, RuntimeHeapBacking,
    RuntimeHeapCreationPlan, RuntimeHeapDestructionPlan, RuntimeHeapFreePlan,
    RuntimeHeapInitialization, RuntimeHeapTeardown, RuntimeHeaps,
};
use super::messages::{RuntimeMessageClearPlan, RuntimeMessageReceivePlan, RuntimeMessages};
use super::messages::{RuntimeMessageReceiveDisposition, RuntimeMessageWake, RuntimeThreadMessage};
use super::modules::{
    RuntimeDllLifecycleAction, RuntimeDllLifecyclePhase, RuntimeDllLifecyclePlan,
    RuntimeGuestImport, RuntimeGuestImportBinding, RuntimeGuestImportBindingPlan, RuntimeModule,
    RuntimeModuleDependencyEdge, RuntimeModuleRecords, RuntimeModules, RuntimeSystemExportTable,
    runtime_module_image_state_error,
};
use super::objects::{
    RuntimeHandle, RuntimeHandleEntry, RuntimeObjectId, RuntimeObjectKey, RuntimeObjectKind,
    RuntimeObjectRecord, RuntimeObjectTable, RuntimeWaitState,
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
    RuntimeMutexNameA,
};
use super::thread::{
    RuntimeCallbackFrame, RuntimeCallbackFrameId, RuntimeCallbackFrameIdPlan, RuntimeCancellation,
    RuntimeComApartmentModel, RuntimeComApartmentState, RuntimeComInitializationOutcome,
    RuntimeComUninitializationOutcome, RuntimeFlsSlot, RuntimeThreadId,
    RuntimeThreadLifecycleState, RuntimeThreadMutationPlan, RuntimeThreadStack,
    RuntimeThreadStackGrowth, RuntimeThreadState, RuntimeThreads, RuntimeTlsValue,
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
    process_plan: RuntimeProcessMutationPlan,
    thread_plans: Vec<RuntimeThreadExitRequestPlan>,
    cancellation: RuntimeProcessCancellation,
}

struct RuntimeCallbackMutationPlan {
    thread: RuntimeThreadMutationPlan,
    scheduler: RuntimeSchedulerLanePlan,
    callback_frame_id: Option<RuntimeCallbackFrameIdPlan>,
    entered: Option<RuntimeCallbackFrame>,
    exited: Option<RuntimeCallbackFrame>,
}

pub(crate) struct RuntimeComReleasePlan {
    com: RuntimeComMutationPlan,
    previous_address_space: Option<ProcessAddressSpaceAllocator>,
    next_address_space: Option<ProcessAddressSpaceAllocator>,
}

impl RuntimeComReleasePlan {
    pub(crate) fn result(&self) -> RuntimeComReferenceResult {
        self.com.reference_result()
    }
}

pub const RUNTIME_CURRENT_THREAD_PSEUDO_HANDLE: u32 = 0xffff_fffe;

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
    objects: RuntimeObjectTable,
    com: RuntimeCom,
    heaps: RuntimeHeaps,
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
}

#[derive(Clone, Copy)]
pub struct RuntimeProcessView<'a> {
    process: &'a RuntimeProcessState,
    heaps: &'a RuntimeHeaps,
    module_records: &'a RuntimeModuleRecords,
    synchronization: &'a RuntimeSynchronization,
    allocated_tls_indices: &'a BTreeSet<u32>,
    fls_slots: &'a BTreeMap<u32, RuntimeFlsSlot>,
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
            },
            address_space_allocator: &self.address_space_allocator,
            threads: &self.threads,
            scheduler: &self.scheduler,
            messages: &self.messages,
            displays: &self.displays,
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
            objects,
            com: RuntimeCom::default(),
            heaps: RuntimeHeaps::default(),
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

    pub fn process(&self) -> RuntimeProcessView<'_> {
        RuntimeProcessView {
            process: &self.process,
            heaps: &self.heaps,
            module_records: &self.module_records,
            synchronization: &self.synchronization,
            allocated_tls_indices: self.threads.allocated_tls_indices(),
            fls_slots: self.threads.fls_slots(),
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

    pub(crate) fn plan_com_address_reservation(
        &self,
        request: ProcessAddressAllocationRequest,
    ) -> Result<ProcessAddressReservationPlan, RuntimeStateError> {
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
        let object = &result.object;
        let allocation = &reservation.allocation;
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
        let (previous_address_space, next_address_space) =
            if let Some(backing) = &result.retired_backing {
                let mut next = self.address_space_allocator.clone();
                let released = next.release(backing.address_allocation_id)?;
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
                (Some(self.address_space_allocator.clone()), Some(next))
            } else {
                (None, None)
            };
        Ok(RuntimeComReleasePlan {
            com,
            previous_address_space,
            next_address_space,
        })
    }

    pub(crate) fn preflight_com_release(
        &self,
        plan: &RuntimeComReleasePlan,
    ) -> Result<(), RuntimeStateError> {
        self.com.preflight(&plan.com)?;
        if let Some(previous) = &plan.previous_address_space
            && &self.address_space_allocator != previous
        {
            return Err(RuntimeStateError::conflict(
                "runtime-com",
                "com-release-allocation-plan-stale",
                "process address allocations changed after COM release planning",
            ));
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
        if let Some(next) = plan.next_address_space {
            self.address_space_allocator = next;
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
                self.synchronization
                    .plan_mutex_retirement(RuntimeMutexId(mutex_id))
            })
            .transpose()?;
        let object_plan = self
            .objects
            .plan_mutex_handle_close(handle, retirement.as_ref().map(|_| &record.key))?;
        let removed = object_plan.closed_entry();
        self.objects.commit_mutex_mutation(object_plan)?;
        if let Some(retirement) = retirement {
            self.synchronization.commit_mutex_retirement(retirement);
        }
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
        let object_plan = self.objects.plan_mutex_handle_open(key, already_exists)?;
        let handle = object_plan.opened_handle();
        self.objects.commit_mutex_mutation(object_plan)?;
        self.synchronization.commit_mutex_creation(plan);
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

    fn teardown_mutexes(&mut self) -> Result<usize, RuntimeStateError> {
        let ids = self.synchronization.mutex_ids();
        let keys = ids
            .iter()
            .map(|id| RuntimeObjectKey::Mutex { mutex_id: id.0 })
            .collect::<Vec<_>>();
        let plans = ids
            .iter()
            .map(|id| self.synchronization.plan_mutex_retirement(*id))
            .collect::<Result<Vec<_>, _>>()?;
        let object_plan = self.objects.plan_mutex_teardown(&keys)?;
        let retired_handles = object_plan.retired_handles();
        self.objects.commit_mutex_mutation(object_plan)?;
        for plan in plans {
            self.synchronization.commit_mutex_retirement(plan);
        }
        Ok(retired_handles)
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
        let callback_frame_id = self.threads.plan_callback_frame_id()?;
        let frame = RuntimeCallbackFrame {
            id: callback_frame_id.id(),
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
        let callback_frame_id = self.threads.plan_callback_frame_id()?;
        let frame = RuntimeCallbackFrame {
            id: callback_frame_id.id(),
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
            callback_frame_id: Some(callback_frame_id),
            entered: Some(frame),
            exited: Some(exited),
        })
    }

    fn commit_callback_mutation(
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
        self.process.preflight_mutation_commit(&process_plan)?;
        self.teardown_mutexes()?;
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
        self.process.preflight_mutation_commit(&process_plan)?;
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
            cancellation,
        })
    }

    fn commit_process_exit_request(
        &mut self,
        plan: RuntimeProcessExitRequestPlan,
    ) -> Result<(), RuntimeStateError> {
        self.process.preflight_mutation_commit(&plan.process_plan)?;
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

#[cfg(test)]
mod tests {
    use super::*;

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
}

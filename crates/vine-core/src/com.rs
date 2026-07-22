use serde::Serialize;

use crate::memory::{
    GUEST_PAGE_SIZE, GuestAddress, GuestMemory, GuestMemoryError, GuestMemoryErrorClass,
    GuestRange, GuestWriteResult, MemoryAccess, MemoryOwnerKind, MemoryRegionKind, PageProtection,
};
use crate::runtime::RUNTIME_COM_ADDRESS_OWNER;
use crate::runtime::{
    ProcessAddressAllocationClassification, ProcessAddressAllocationLifetime,
    ProcessAddressAllocationPurpose, ProcessAddressAllocationReleasePolicy,
    ProcessAddressAllocationRequest, RUNTIME_COM_IUNKNOWN_IID,
    RUNTIME_COM_MAX_INTERFACES_PER_OBJECT, RuntimeAddressRange, RuntimeAudioDevice,
    RuntimeAudioDeviceCreateRequest, RuntimeComBacking, RuntimeComIid, RuntimeComInterfaceId,
    RuntimeComInterfaceRecord, RuntimeComInterfaceRegistration, RuntimeComMethodImplementation,
    RuntimeComMethodRegistration, RuntimeComMethodTarget, RuntimeComObjectId,
    RuntimeComObjectRecord, RuntimeComObjectRegistration, RuntimeComReturnKind,
    RuntimeGetGraphicsSurfaceDcResult, RuntimeGraphicsSurfaceDcAcquisitionPlan,
    RuntimeGraphicsVertexBuffer, RuntimeGraphicsVertexBufferCreateRequest,
    RuntimeGraphicsVertexBufferLock, RuntimeGraphicsVertexBufferLockRequest,
    RuntimeGraphicsVertexBufferUnlockResult, RuntimeKernel, RuntimeStateError,
    RuntimeStateErrorClass,
};

pub const COM_S_OK: ComHResult = ComHResult(0);
pub const COM_S_FALSE: ComHResult = ComHResult(1);
pub const COM_E_NOINTERFACE: ComHResult = ComHResult(0x8000_4002_u32 as i32);
pub const COM_E_POINTER: ComHResult = ComHResult(0x8000_4003_u32 as i32);
pub const COM_RPC_E_CHANGED_MODE: ComHResult = ComHResult(0x8001_0106_u32 as i32);
pub const RUNTIME_COM_ENDPOINT_SLOT_SIZE: u32 = 16;
pub const RUNTIME_COM_BACKING_BYTES: u32 = GUEST_PAGE_SIZE as u32 * 2;
pub const RUNTIME_COM_MAX_ENDPOINTS_PER_OBJECT: usize =
    GUEST_PAGE_SIZE as usize / RUNTIME_COM_ENDPOINT_SLOT_SIZE as usize;
const RUNTIME_COM_SEARCH_BASE: GuestAddress = GuestAddress(0x0100_0000);
const RUNTIME_COM_SEARCH_BYTES: u64 = 0x0f00_0000;
const RUNTIME_COM_ALLOCATION_ALIGNMENT: u32 = 0x1_0000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct ComHResult(pub i32);

impl ComHResult {
    pub const fn bits(self) -> u32 {
        self.0 as u32
    }

    pub const fn succeeded(self) -> bool {
        self.0 >= 0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComMethodSpec {
    pub name: String,
    pub explicit_argument_words: u16,
    pub return_kind: RuntimeComReturnKind,
    pub implementation: RuntimeComMethodImplementation,
}

impl ComMethodSpec {
    pub fn query_interface() -> Self {
        Self {
            name: "QueryInterface".to_owned(),
            explicit_argument_words: 2,
            return_kind: RuntimeComReturnKind::HResult,
            implementation: RuntimeComMethodImplementation::QueryInterface,
        }
    }

    pub fn add_ref() -> Self {
        Self {
            name: "AddRef".to_owned(),
            explicit_argument_words: 0,
            return_kind: RuntimeComReturnKind::U32,
            implementation: RuntimeComMethodImplementation::AddRef,
        }
    }

    pub fn release() -> Self {
        Self {
            name: "Release".to_owned(),
            explicit_argument_words: 0,
            return_kind: RuntimeComReturnKind::U32,
            implementation: RuntimeComMethodImplementation::Release,
        }
    }

    pub fn unsupported_hresult(name: impl Into<String>, explicit_argument_words: u16) -> Self {
        Self {
            name: name.into(),
            explicit_argument_words,
            return_kind: RuntimeComReturnKind::HResult,
            implementation: RuntimeComMethodImplementation::Unsupported,
        }
    }

    pub fn unsupported_u32(name: impl Into<String>, explicit_argument_words: u16) -> Self {
        Self {
            name: name.into(),
            explicit_argument_words,
            return_kind: RuntimeComReturnKind::U32,
            implementation: RuntimeComMethodImplementation::Unsupported,
        }
    }

    pub fn unsupported_void(name: impl Into<String>, explicit_argument_words: u16) -> Self {
        Self {
            name: name.into(),
            explicit_argument_words,
            return_kind: RuntimeComReturnKind::Void,
            implementation: RuntimeComMethodImplementation::Unsupported,
        }
    }

    pub fn unsupported_x87_f32(name: impl Into<String>, explicit_argument_words: u16) -> Self {
        Self {
            name: name.into(),
            explicit_argument_words,
            return_kind: RuntimeComReturnKind::X87F32,
            implementation: RuntimeComMethodImplementation::Unsupported,
        }
    }

    pub fn frontend_hresult(name: impl Into<String>, explicit_argument_words: u16) -> Self {
        Self {
            name: name.into(),
            explicit_argument_words,
            return_kind: RuntimeComReturnKind::HResult,
            implementation: RuntimeComMethodImplementation::Frontend,
        }
    }

    pub fn frontend_u32(name: impl Into<String>, explicit_argument_words: u16) -> Self {
        Self {
            name: name.into(),
            explicit_argument_words,
            return_kind: RuntimeComReturnKind::U32,
            implementation: RuntimeComMethodImplementation::Frontend,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComInterfaceSpec {
    pub iid: RuntimeComIid,
    pub name: String,
    pub methods: Vec<ComMethodSpec>,
}

impl ComInterfaceSpec {
    pub fn iunknown() -> Self {
        Self {
            iid: RUNTIME_COM_IUNKNOWN_IID,
            name: "IUnknown".to_owned(),
            methods: vec![
                ComMethodSpec::query_interface(),
                ComMethodSpec::add_ref(),
                ComMethodSpec::release(),
            ],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComObjectSpec {
    pub class_identity: String,
    pub interfaces: Vec<ComInterfaceSpec>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledComObject {
    pub object: RuntimeComObjectRecord,
    pub interfaces: Vec<RuntimeComInterfaceRecord>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComCallTrap {
    pub endpoint_address: GuestAddress,
    pub stack_pointer: GuestAddress,
    pub return_address: GuestAddress,
    pub call_site: GuestAddress,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComCallFrame {
    pub object_id: crate::runtime::RuntimeComObjectId,
    pub interface_id: RuntimeComInterfaceId,
    pub interface: String,
    pub iid: RuntimeComIid,
    pub vtable_slot: u16,
    pub method: String,
    pub endpoint_address: GuestAddress,
    pub call_site: GuestAddress,
    pub stack_pointer: GuestAddress,
    pub return_stack_pointer: GuestAddress,
    pub return_address: GuestAddress,
    pub this_pointer: GuestAddress,
    pub argument_bytes: u32,
    pub explicit_arguments: Vec<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "kebab-case")]
pub enum ComReturnValue {
    HResult(ComHResult),
    U32(u32),
}

impl ComReturnValue {
    pub const fn eax(self) -> u32 {
        match self {
            Self::HResult(value) => value.bits(),
            Self::U32(value) => value,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComGuestWriteReceipt {
    pub address: GuestAddress,
    pub bytes: Vec<u8>,
    pub result: GuestWriteResult,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComCallCompletion {
    pub frame: ComCallFrame,
    pub instruction_pointer: GuestAddress,
    pub stack_pointer: GuestAddress,
    pub return_value: ComReturnValue,
    pub reference_count: u32,
    pub object_retired: bool,
    pub guest_writes: Vec<ComGuestWriteReceipt>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ComFaultClass {
    InvalidCall,
    Unsupported,
    ResourceLimit,
    RuntimeDefect,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComFaultTarget {
    pub object_id: crate::runtime::RuntimeComObjectId,
    pub interface_id: RuntimeComInterfaceId,
    pub interface: String,
    pub iid: RuntimeComIid,
    pub vtable_slot: u16,
    pub method: String,
    pub endpoint_address: GuestAddress,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComFault {
    pub class: ComFaultClass,
    pub stage: &'static str,
    pub code: &'static str,
    pub message: String,
    pub call_site: GuestAddress,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<ComFaultTarget>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComDispatchOutcome {
    Completed(ComCallCompletion),
    Fault(ComFault),
}

pub(crate) struct ComFrontendCall<'a> {
    memory: &'a mut GuestMemory,
    runtime: &'a mut RuntimeKernel,
    trap: ComCallTrap,
    target: RuntimeComMethodTarget,
    frame: ComCallFrame,
}

impl ComFrontendCall<'_> {
    pub(crate) fn frame(&self) -> &ComCallFrame {
        &self.frame
    }

    pub(crate) fn class_identity(&self) -> &str {
        &self.target.object.class_identity
    }

    pub(crate) fn explicit_argument_words(&self) -> u16 {
        self.target.method.explicit_argument_words
    }

    pub(crate) fn return_kind(&self) -> RuntimeComReturnKind {
        self.target.method.return_kind
    }

    pub(crate) fn memory(&self) -> &GuestMemory {
        self.memory
    }

    pub(crate) fn memory_mut(&mut self) -> &mut GuestMemory {
        self.memory
    }

    pub(crate) fn runtime(&self) -> &RuntimeKernel {
        self.runtime
    }

    pub(crate) fn runtime_mut(&mut self) -> &mut RuntimeKernel {
        self.runtime
    }

    pub(crate) fn install_child_com_object_with_guest_writes<F>(
        &mut self,
        spec: ComObjectSpec,
        build_guest_writes: F,
    ) -> Result<(InstalledComObject, Vec<ComGuestWriteReceipt>), RuntimeStateError>
    where
        F: FnOnce(&InstalledComObject) -> Result<Vec<(GuestAddress, Vec<u8>)>, RuntimeStateError>,
    {
        install_com_object_with_guest_writes(
            self.memory,
            self.runtime,
            Some(self.frame.interface_id),
            spec,
            build_guest_writes,
        )
    }

    pub(crate) fn install_graphics_vertex_buffer_with_guest_pointer_write(
        &mut self,
        spec: ComObjectSpec,
        output_iid: RuntimeComIid,
        request: RuntimeGraphicsVertexBufferCreateRequest,
        output_address: GuestAddress,
    ) -> Result<
        (
            InstalledComObject,
            RuntimeGraphicsVertexBuffer,
            Vec<ComGuestWriteReceipt>,
        ),
        RuntimeStateError,
    > {
        install_graphics_vertex_buffer_object(
            self.memory,
            self.runtime,
            self.frame.interface_id,
            spec,
            output_iid,
            request,
            output_address,
        )
    }

    pub(crate) fn lock_graphics_vertex_buffer_with_guest_pointer_write(
        &mut self,
        request: RuntimeGraphicsVertexBufferLockRequest,
        output_address: GuestAddress,
    ) -> Result<(RuntimeGraphicsVertexBufferLock, Vec<ComGuestWriteReceipt>), RuntimeStateError>
    {
        self.memory
            .validate_access(output_address, 4, MemoryAccess::Write)
            .map_err(RuntimeStateError::from_memory)?;
        let plan = self.runtime.plan_graphics_vertex_buffer_lock(request)?;
        let lock = plan.lock();
        let backing = self
            .runtime
            .graphics_vertex_buffer(plan.vertex_buffer_id())?
            .backing;
        let bytes = lock.guest_address.0.to_le_bytes().to_vec();
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
        let write_result = transaction
            .write(output_address, &bytes)
            .map_err(RuntimeStateError::from_memory)?;
        self.runtime.preflight_graphics_vertex_buffer_lock(&plan)?;
        let committed = self.runtime.commit_graphics_vertex_buffer_lock(plan);
        transaction.commit_transaction();
        debug_assert_eq!(committed, lock);
        Ok((
            committed,
            vec![ComGuestWriteReceipt {
                address: output_address,
                bytes,
                result: write_result,
            }],
        ))
    }

    pub(crate) fn unlock_graphics_vertex_buffer(
        &mut self,
        vertex_buffer_com_object_id: RuntimeComObjectId,
    ) -> Result<RuntimeGraphicsVertexBufferUnlockResult, RuntimeStateError> {
        let plan = self
            .runtime
            .plan_graphics_vertex_buffer_unlock(vertex_buffer_com_object_id)?;
        let backing = self
            .runtime
            .graphics_vertex_buffer(plan.vertex_buffer_id())?
            .backing;
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
        self.runtime
            .preflight_graphics_vertex_buffer_unlock(&plan)?;
        let result = self.runtime.commit_graphics_vertex_buffer_unlock(plan);
        transaction.commit_transaction();
        Ok(result)
    }

    pub(crate) fn add_ref_interface_with_guest_pointer_write(
        &mut self,
        interface_id: RuntimeComInterfaceId,
        output_address: GuestAddress,
    ) -> Result<(u32, Vec<ComGuestWriteReceipt>), RuntimeStateError> {
        self.memory
            .validate_access(output_address, 4, MemoryAccess::Write)
            .map_err(RuntimeStateError::from_memory)?;
        let interface = self.runtime.com().interface(interface_id)?.clone();
        let bytes = interface.guest_pointer.0.to_le_bytes().to_vec();
        let plan = self.runtime.plan_com_add_ref(interface_id)?;
        let reference_count = plan.reference_result().reference_count;
        let mut transaction = self.memory.begin_transaction();
        let write_result = transaction
            .write(output_address, &bytes)
            .map_err(RuntimeStateError::from_memory)?;
        self.runtime.commit_com_mutation(plan)?;
        transaction.commit_transaction();
        Ok((
            reference_count,
            vec![ComGuestWriteReceipt {
                address: output_address,
                bytes,
                result: write_result,
            }],
        ))
    }

    pub(crate) fn acquire_graphics_surface_dc_with_guest_handle_write(
        &mut self,
        plan: RuntimeGraphicsSurfaceDcAcquisitionPlan,
        output_address: GuestAddress,
    ) -> Result<(RuntimeGetGraphicsSurfaceDcResult, Vec<ComGuestWriteReceipt>), RuntimeStateError>
    {
        self.memory
            .validate_access(output_address, 4, MemoryAccess::Write)
            .map_err(RuntimeStateError::from_memory)?;
        let planned_result = plan.result();
        let bytes = planned_result.handle.to_le_bytes().to_vec();
        let mut transaction = self.memory.begin_transaction();
        let write_result = transaction
            .write(output_address, &bytes)
            .map_err(RuntimeStateError::from_memory)?;
        let result = self.runtime.commit_graphics_surface_dc_acquisition(plan)?;
        transaction.commit_transaction();
        debug_assert_eq!(result, planned_result);
        Ok((
            result,
            vec![ComGuestWriteReceipt {
                address: output_address,
                bytes,
                result: write_result,
            }],
        ))
    }

    pub(crate) fn fault(
        self,
        class: ComFaultClass,
        stage: &'static str,
        code: &'static str,
        message: impl Into<String>,
    ) -> ComDispatchOutcome {
        ComDispatchOutcome::Fault(fault(
            class,
            stage,
            code,
            message,
            self.trap,
            Some(&self.target),
        ))
    }

    pub(crate) fn memory_fault(self, error: GuestMemoryError) -> ComDispatchOutcome {
        ComDispatchOutcome::Fault(fault_from_memory(error, self.trap, Some(&self.target)))
    }

    pub(crate) fn runtime_fault(self, error: RuntimeStateError) -> ComDispatchOutcome {
        ComDispatchOutcome::Fault(fault_from_runtime(error, self.trap, Some(&self.target)))
    }

    pub(crate) fn complete(
        self,
        return_value: ComReturnValue,
        guest_writes: Vec<ComGuestWriteReceipt>,
    ) -> ComDispatchOutcome {
        if !return_value_matches(self.target.method.return_kind, return_value) {
            return self.fault(
                ComFaultClass::RuntimeDefect,
                "com-frontend-gateway",
                "com-frontend-return-kind-mismatch",
                "COM frontend return value does not match its registered x86 ABI",
            );
        }
        let reference_count = self.target.object.reference_count;
        complete_call(
            self.target,
            self.frame,
            return_value,
            reference_count,
            false,
            guest_writes,
        )
    }
}

struct PlannedComLayout {
    interfaces: Vec<RuntimeComInterfaceRegistration>,
    data_writes: Vec<(GuestAddress, Vec<u8>)>,
    endpoint_writes: Vec<(GuestAddress, Vec<u8>)>,
}

pub fn install_com_object(
    memory: &mut GuestMemory,
    runtime: &mut RuntimeKernel,
    spec: ComObjectSpec,
) -> Result<InstalledComObject, RuntimeStateError> {
    install_com_object_with_guest_writes(memory, runtime, None, spec, |_| Ok(Vec::new()))
        .map(|(installed, _)| installed)
}

pub(crate) fn install_com_object_with_guest_writes<F>(
    memory: &mut GuestMemory,
    runtime: &mut RuntimeKernel,
    parent_interface_id: Option<RuntimeComInterfaceId>,
    spec: ComObjectSpec,
    build_guest_writes: F,
) -> Result<(InstalledComObject, Vec<ComGuestWriteReceipt>), RuntimeStateError>
where
    F: FnOnce(&InstalledComObject) -> Result<Vec<(GuestAddress, Vec<u8>)>, RuntimeStateError>,
{
    let (installed, audio, guest_writes) = install_com_object_with_guest_writes_internal(
        memory,
        runtime,
        parent_interface_id,
        spec,
        None,
        build_guest_writes,
    )?;
    debug_assert!(audio.is_none());
    Ok((installed, guest_writes))
}

pub(crate) fn install_audio_device_com_object_with_guest_writes<F>(
    memory: &mut GuestMemory,
    runtime: &mut RuntimeKernel,
    spec: ComObjectSpec,
    audio_request: RuntimeAudioDeviceCreateRequest,
    build_guest_writes: F,
) -> Result<
    (
        InstalledComObject,
        RuntimeAudioDevice,
        Vec<ComGuestWriteReceipt>,
    ),
    RuntimeStateError,
>
where
    F: FnOnce(&InstalledComObject) -> Result<Vec<(GuestAddress, Vec<u8>)>, RuntimeStateError>,
{
    let (installed, audio, guest_writes) = install_com_object_with_guest_writes_internal(
        memory,
        runtime,
        None,
        spec,
        Some(audio_request),
        build_guest_writes,
    )?;
    let audio = audio.ok_or_else(|| {
        RuntimeStateError::runtime_defect(
            "runtime-audio",
            "audio-device-installation-result-missing",
            "audio device installation committed no logical device",
        )
    })?;
    Ok((installed, audio, guest_writes))
}

fn install_com_object_with_guest_writes_internal<F>(
    memory: &mut GuestMemory,
    runtime: &mut RuntimeKernel,
    parent_interface_id: Option<RuntimeComInterfaceId>,
    spec: ComObjectSpec,
    audio_request: Option<RuntimeAudioDeviceCreateRequest>,
    build_guest_writes: F,
) -> Result<
    (
        InstalledComObject,
        Option<RuntimeAudioDevice>,
        Vec<ComGuestWriteReceipt>,
    ),
    RuntimeStateError,
>
where
    F: FnOnce(&InstalledComObject) -> Result<Vec<(GuestAddress, Vec<u8>)>, RuntimeStateError>,
{
    validate_object_spec(&spec)?;
    let reservation = runtime.plan_com_address_reservation(com_address_allocation_request())?;
    let allocation = reservation.allocation.clone();
    if allocation.range.byte_length != u64::from(RUNTIME_COM_BACKING_BYTES)
        || allocation.usable_byte_length != u64::from(RUNTIME_COM_BACKING_BYTES)
    {
        return Err(RuntimeStateError::runtime_defect(
            "runtime-com",
            "com-backing-allocation-size-invalid",
            "COM backing allocation did not preserve its two-page layout",
        ));
    }
    let layout = plan_layout(allocation.range.base, &spec)?;
    let data_range = GuestRange::new(allocation.range.base, GUEST_PAGE_SIZE)
        .map_err(RuntimeStateError::from_memory)?;
    let endpoint_base = add_address(allocation.range.base, GUEST_PAGE_SIZE as u32)?;
    let endpoint_range =
        GuestRange::new(endpoint_base, GUEST_PAGE_SIZE).map_err(RuntimeStateError::from_memory)?;

    let mut transaction = memory.begin_transaction();
    let memory_owner = transaction
        .create_owner(MemoryOwnerKind::Runtime)
        .map_err(RuntimeStateError::from_memory)?;
    transaction
        .reserve_fixed_for_address_allocation(
            allocation.id.0,
            memory_owner,
            MemoryRegionKind::Runtime,
            allocation.range.base,
            allocation.range.byte_length,
        )
        .map_err(RuntimeStateError::from_memory)?;
    let writable = PageProtection {
        read: true,
        write: true,
        execute: false,
    };
    transaction
        .commit(memory_owner, data_range, writable)
        .map_err(RuntimeStateError::from_memory)?;
    transaction
        .commit(memory_owner, endpoint_range, writable)
        .map_err(RuntimeStateError::from_memory)?;
    for (address, bytes) in layout.data_writes.iter().chain(&layout.endpoint_writes) {
        transaction
            .write(*address, bytes)
            .map_err(RuntimeStateError::from_memory)?;
    }
    transaction
        .protect(
            memory_owner,
            data_range,
            PageProtection {
                read: true,
                write: false,
                execute: false,
            },
        )
        .map_err(RuntimeStateError::from_memory)?;
    transaction
        .protect(
            memory_owner,
            endpoint_range,
            PageProtection {
                read: true,
                write: false,
                execute: true,
            },
        )
        .map_err(RuntimeStateError::from_memory)?;
    transaction
        .seal_owner(memory_owner)
        .map_err(RuntimeStateError::from_memory)?;

    let registration = runtime.plan_com_registration(RuntimeComObjectRegistration {
        class_identity: spec.class_identity,
        initial_reference_count: 1,
        parent_interface_id,
        backing: RuntimeComBacking {
            address_allocation_id: allocation.id,
            memory_owner,
            range: allocation.range,
        },
        interfaces: layout.interfaces,
    })?;
    let planned_result = registration.registration_result();
    let planned_installation = InstalledComObject {
        object: planned_result.object,
        interfaces: planned_result.interfaces,
    };
    let audio_plan = audio_request
        .map(|request| runtime.plan_audio_device_creation(request))
        .transpose()?;
    let additional_writes = build_guest_writes(&planned_installation)?;
    let mut guest_writes = Vec::with_capacity(additional_writes.len());
    for (address, bytes) in additional_writes {
        let result = transaction
            .write(address, &bytes)
            .map_err(RuntimeStateError::from_memory)?;
        guest_writes.push(ComGuestWriteReceipt {
            address,
            bytes,
            result,
        });
    }
    let (result, audio) = if let Some(audio_plan) = audio_plan {
        let (result, audio) =
            runtime.commit_com_audio_device_installation(reservation, registration, audio_plan)?;
        (result, Some(audio))
    } else {
        (
            runtime.commit_com_installation(reservation, registration)?,
            None,
        )
    };
    transaction.commit_transaction();
    let installed = InstalledComObject {
        object: result.object,
        interfaces: result.interfaces,
    };
    debug_assert_eq!(installed, planned_installation);
    Ok((installed, audio, guest_writes))
}

fn com_address_allocation_request() -> ProcessAddressAllocationRequest {
    ProcessAddressAllocationRequest {
        purpose: ProcessAddressAllocationPurpose::ComEndpoint,
        owner: RUNTIME_COM_ADDRESS_OWNER.to_owned(),
        preferred_address: None,
        search_range: RuntimeAddressRange {
            base: RUNTIME_COM_SEARCH_BASE,
            byte_length: RUNTIME_COM_SEARCH_BYTES,
        },
        reserve_size: u64::from(RUNTIME_COM_BACKING_BYTES),
        alignment: RUNTIME_COM_ALLOCATION_ALIGNMENT,
        guard_pages: 0,
        classification: ProcessAddressAllocationClassification::Com,
        lifetime: ProcessAddressAllocationLifetime::Object,
        release_policy: ProcessAddressAllocationReleasePolicy::Explicit,
        executable: true,
    }
}

fn install_graphics_vertex_buffer_object(
    memory: &mut GuestMemory,
    runtime: &mut RuntimeKernel,
    parent_interface_id: RuntimeComInterfaceId,
    spec: ComObjectSpec,
    output_iid: RuntimeComIid,
    request: RuntimeGraphicsVertexBufferCreateRequest,
    output_address: GuestAddress,
) -> Result<
    (
        InstalledComObject,
        RuntimeGraphicsVertexBuffer,
        Vec<ComGuestWriteReceipt>,
    ),
    RuntimeStateError,
> {
    validate_object_spec(&spec)?;
    memory
        .validate_access(output_address, 4, MemoryAccess::Write)
        .map_err(RuntimeStateError::from_memory)?;
    let installation = runtime
        .plan_graphics_vertex_buffer_installation(com_address_allocation_request(), request)?;
    let com_allocation = installation.com_allocation().clone();
    let resource_allocation = installation.resource_allocation().clone();
    if com_allocation.range.byte_length != u64::from(RUNTIME_COM_BACKING_BYTES)
        || com_allocation.usable_byte_length != u64::from(RUNTIME_COM_BACKING_BYTES)
    {
        return Err(RuntimeStateError::runtime_defect(
            "runtime-com",
            "com-backing-allocation-size-invalid",
            "COM backing allocation did not preserve its two-page layout",
        ));
    }
    let layout = plan_layout(com_allocation.range.base, &spec)?;
    let data_range = GuestRange::new(com_allocation.range.base, GUEST_PAGE_SIZE)
        .map_err(RuntimeStateError::from_memory)?;
    let endpoint_base = add_address(com_allocation.range.base, GUEST_PAGE_SIZE as u32)?;
    let endpoint_range =
        GuestRange::new(endpoint_base, GUEST_PAGE_SIZE).map_err(RuntimeStateError::from_memory)?;
    let resource_range = GuestRange::new(
        resource_allocation.range.base,
        resource_allocation.range.byte_length,
    )
    .map_err(RuntimeStateError::from_memory)?;

    let mut transaction = memory.begin_transaction();
    let com_memory_owner = transaction
        .create_owner(MemoryOwnerKind::Runtime)
        .map_err(RuntimeStateError::from_memory)?;
    transaction
        .reserve_fixed_for_address_allocation(
            com_allocation.id.0,
            com_memory_owner,
            MemoryRegionKind::Runtime,
            com_allocation.range.base,
            com_allocation.range.byte_length,
        )
        .map_err(RuntimeStateError::from_memory)?;
    let writable = PageProtection {
        read: true,
        write: true,
        execute: false,
    };
    transaction
        .commit(com_memory_owner, data_range, writable)
        .map_err(RuntimeStateError::from_memory)?;
    transaction
        .commit(com_memory_owner, endpoint_range, writable)
        .map_err(RuntimeStateError::from_memory)?;
    for (address, bytes) in layout.data_writes.iter().chain(&layout.endpoint_writes) {
        transaction
            .write(*address, bytes)
            .map_err(RuntimeStateError::from_memory)?;
    }
    transaction
        .protect(
            com_memory_owner,
            data_range,
            PageProtection {
                read: true,
                write: false,
                execute: false,
            },
        )
        .map_err(RuntimeStateError::from_memory)?;
    transaction
        .protect(
            com_memory_owner,
            endpoint_range,
            PageProtection {
                read: true,
                write: false,
                execute: true,
            },
        )
        .map_err(RuntimeStateError::from_memory)?;
    transaction
        .seal_owner(com_memory_owner)
        .map_err(RuntimeStateError::from_memory)?;

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

    let registration = runtime.plan_com_registration(RuntimeComObjectRegistration {
        class_identity: spec.class_identity,
        initial_reference_count: 1,
        parent_interface_id: Some(parent_interface_id),
        backing: RuntimeComBacking {
            address_allocation_id: com_allocation.id,
            memory_owner: com_memory_owner,
            range: com_allocation.range,
        },
        interfaces: layout.interfaces,
    })?;
    let planned_result = registration.registration_result();
    let planned_installation = InstalledComObject {
        object: planned_result.object,
        interfaces: planned_result.interfaces,
    };
    let output_interface = planned_installation
        .interfaces
        .iter()
        .find(|interface| interface.iid == output_iid)
        .ok_or_else(|| {
            RuntimeStateError::runtime_defect(
                "runtime-graphics",
                "graphics-vertex-buffer-installation-interface-missing",
                "installed graphics vertex-buffer object omitted its requested interface",
            )
        })?;
    let bytes = output_interface.guest_pointer.0.to_le_bytes().to_vec();
    let write_result = transaction
        .write(output_address, &bytes)
        .map_err(RuntimeStateError::from_memory)?;
    runtime.preflight_graphics_vertex_buffer_installation(&installation, &registration)?;
    let (result, vertex_buffer) = runtime.commit_graphics_vertex_buffer_installation(
        installation,
        registration,
        resource_memory_owner,
    )?;
    transaction.commit_transaction();
    let installed = InstalledComObject {
        object: result.object,
        interfaces: result.interfaces,
    };
    debug_assert_eq!(installed, planned_installation);
    Ok((
        installed,
        vertex_buffer,
        vec![ComGuestWriteReceipt {
            address: output_address,
            bytes,
            result: write_result,
        }],
    ))
}

pub fn dispatch_com_call(
    memory: &mut GuestMemory,
    runtime: &mut RuntimeKernel,
    trap: ComCallTrap,
) -> ComDispatchOutcome {
    let target = match runtime.resolve_com_method(trap.endpoint_address) {
        Ok(target) => target,
        Err(error) => return ComDispatchOutcome::Fault(fault_from_runtime(error, trap, None)),
    };
    if matches!(
        target.method.implementation,
        RuntimeComMethodImplementation::Frontend | RuntimeComMethodImplementation::Unsupported
    ) {
        return ComDispatchOutcome::Fault(fault(
            ComFaultClass::Unsupported,
            "com-dispatch",
            "com-method-unsupported",
            format!(
                "COM interface {} slot {} ({}) is not implemented",
                target.interface.name, target.method.slot, target.method.name
            ),
            trap,
            Some(&target),
        ));
    }
    dispatch_resolved_com_call(memory, runtime, trap, target, |_| {
        unreachable!("non-frontend COM dispatch cannot reach a frontend handler")
    })
}

pub(crate) fn dispatch_com_call_with_frontend<F>(
    memory: &mut GuestMemory,
    runtime: &mut RuntimeKernel,
    trap: ComCallTrap,
    frontend: F,
) -> ComDispatchOutcome
where
    F: FnOnce(ComFrontendCall<'_>) -> ComDispatchOutcome,
{
    let target = match runtime.resolve_com_method(trap.endpoint_address) {
        Ok(target) => target,
        Err(error) => return ComDispatchOutcome::Fault(fault_from_runtime(error, trap, None)),
    };
    if target.method.implementation == RuntimeComMethodImplementation::Unsupported {
        return ComDispatchOutcome::Fault(fault(
            ComFaultClass::Unsupported,
            "com-dispatch",
            "com-method-unsupported",
            format!(
                "COM interface {} slot {} ({}) is not implemented",
                target.interface.name, target.method.slot, target.method.name
            ),
            trap,
            Some(&target),
        ));
    }
    dispatch_resolved_com_call(memory, runtime, trap, target, frontend)
}

fn dispatch_resolved_com_call<F>(
    memory: &mut GuestMemory,
    runtime: &mut RuntimeKernel,
    trap: ComCallTrap,
    target: RuntimeComMethodTarget,
    frontend: F,
) -> ComDispatchOutcome
where
    F: FnOnce(ComFrontendCall<'_>) -> ComDispatchOutcome,
{
    let frame = match decode_frame(memory, &target, trap) {
        Ok(frame) => frame,
        Err(fault) => return ComDispatchOutcome::Fault(*fault),
    };
    if let Err(fault) = preflight_return_kind(&target, trap) {
        return ComDispatchOutcome::Fault(*fault);
    }
    match target.method.implementation {
        RuntimeComMethodImplementation::QueryInterface => {
            dispatch_query_interface(memory, runtime, trap, target, frame)
        }
        RuntimeComMethodImplementation::AddRef => {
            let plan = match runtime.plan_com_add_ref(target.interface.id) {
                Ok(plan) => plan,
                Err(error) => {
                    return ComDispatchOutcome::Fault(fault_from_runtime(
                        error,
                        trap,
                        Some(&target),
                    ));
                }
            };
            let result = plan.reference_result();
            if let Err(error) = runtime.commit_com_mutation(plan) {
                return ComDispatchOutcome::Fault(fault_from_runtime(error, trap, Some(&target)));
            }
            complete_call(
                target,
                frame,
                ComReturnValue::U32(result.reference_count),
                result.reference_count,
                false,
                Vec::new(),
            )
        }
        RuntimeComMethodImplementation::Release => {
            dispatch_release(memory, runtime, trap, target, frame)
        }
        RuntimeComMethodImplementation::Frontend => frontend(ComFrontendCall {
            memory,
            runtime,
            trap,
            target,
            frame,
        }),
        RuntimeComMethodImplementation::Unsupported => unreachable!(),
    }
}

fn dispatch_query_interface(
    memory: &mut GuestMemory,
    runtime: &mut RuntimeKernel,
    trap: ComCallTrap,
    target: RuntimeComMethodTarget,
    frame: ComCallFrame,
) -> ComDispatchOutcome {
    let iid_pointer = frame.explicit_arguments[0];
    let output_pointer = frame.explicit_arguments[1];
    if iid_pointer == 0 || output_pointer == 0 {
        let reference_count = runtime
            .com()
            .object(frame.object_id)
            .map(|object| object.reference_count)
            .unwrap_or(0);
        return complete_call(
            target,
            frame,
            ComReturnValue::HResult(COM_E_POINTER),
            reference_count,
            false,
            Vec::new(),
        );
    }
    let iid_bytes = match memory.read(GuestAddress(iid_pointer), 16) {
        Ok(bytes) => bytes,
        Err(error) => {
            return ComDispatchOutcome::Fault(fault_from_memory(error, trap, Some(&target)));
        }
    };
    if let Err(error) = memory.validate_access(GuestAddress(output_pointer), 4, MemoryAccess::Write)
    {
        return ComDispatchOutcome::Fault(fault_from_memory(error, trap, Some(&target)));
    }
    let iid = RuntimeComIid::from_windows_bytes(
        iid_bytes
            .try_into()
            .expect("checked COM IID read contains sixteen bytes"),
    );
    let plan = match runtime.plan_com_query_interface(target.interface.id, iid) {
        Ok(plan) => plan,
        Err(error) => {
            return ComDispatchOutcome::Fault(fault_from_runtime(error, trap, Some(&target)));
        }
    };
    let result = plan.query_result();
    let (hresult, output) = match &result.target_interface {
        Some(interface) => (COM_S_OK, interface.guest_pointer.0),
        None => (COM_E_NOINTERFACE, 0),
    };
    let bytes = output.to_le_bytes().to_vec();
    let mut transaction = memory.begin_transaction();
    let write_result = match transaction.write(GuestAddress(output_pointer), &bytes) {
        Ok(result) => result,
        Err(error) => {
            return ComDispatchOutcome::Fault(fault_from_memory(error, trap, Some(&target)));
        }
    };
    if let Err(error) = runtime.commit_com_mutation(plan) {
        return ComDispatchOutcome::Fault(fault_from_runtime(error, trap, Some(&target)));
    }
    transaction.commit_transaction();
    complete_call(
        target,
        frame,
        ComReturnValue::HResult(hresult),
        result.reference_count,
        false,
        vec![ComGuestWriteReceipt {
            address: GuestAddress(output_pointer),
            bytes,
            result: write_result,
        }],
    )
}

fn dispatch_release(
    memory: &mut GuestMemory,
    runtime: &mut RuntimeKernel,
    trap: ComCallTrap,
    target: RuntimeComMethodTarget,
    frame: ComCallFrame,
) -> ComDispatchOutcome {
    let plan = match runtime.plan_com_release(target.interface.id) {
        Ok(plan) => plan,
        Err(error) => {
            return ComDispatchOutcome::Fault(fault_from_runtime(error, trap, Some(&target)));
        }
    };
    let result = plan.result();
    let retired = result
        .retirements
        .iter()
        .any(|retirement| retirement.object_id == result.interface.object_id);
    let memory_retirements = plan.memory_retirements();
    if !memory_retirements.is_empty() {
        let mut transaction = memory.begin_transaction();
        for (memory_owner, range) in memory_retirements {
            if let Err(error) = transaction.release_region(memory_owner, range.start()) {
                return ComDispatchOutcome::Fault(fault_from_memory(error, trap, Some(&target)));
            }
            if let Err(error) = transaction.destroy_owner(memory_owner) {
                return ComDispatchOutcome::Fault(fault_from_memory(error, trap, Some(&target)));
            }
        }
        if let Err(error) = runtime.commit_com_release(plan) {
            return ComDispatchOutcome::Fault(fault_from_runtime(error, trap, Some(&target)));
        }
        transaction.commit_transaction();
    } else if let Err(error) = runtime.commit_com_release(plan) {
        return ComDispatchOutcome::Fault(fault_from_runtime(error, trap, Some(&target)));
    }
    complete_call(
        target,
        frame,
        ComReturnValue::U32(result.reference_count),
        result.reference_count,
        retired,
        Vec::new(),
    )
}

fn decode_frame(
    memory: &GuestMemory,
    target: &RuntimeComMethodTarget,
    trap: ComCallTrap,
) -> Result<ComCallFrame, Box<ComFault>> {
    let argument_words = target
        .method
        .argument_words()
        .map_err(|error| Box::new(fault_from_runtime(error, trap, Some(target))))?;
    let argument_bytes = argument_words.checked_mul(4).ok_or_else(|| {
        Box::new(fault(
            ComFaultClass::ResourceLimit,
            "com-abi-gateway",
            "com-argument-byte-count-overflow",
            "COM argument byte count overflowed x86 accounting",
            trap,
            Some(target),
        ))
    })?;
    let frame_bytes = argument_bytes.checked_add(4).ok_or_else(|| {
        Box::new(fault(
            ComFaultClass::ResourceLimit,
            "com-abi-gateway",
            "com-call-frame-byte-count-overflow",
            "COM call frame byte count overflowed x86 accounting",
            trap,
            Some(target),
        ))
    })?;
    let return_stack_pointer = trap
        .stack_pointer
        .0
        .checked_add(frame_bytes)
        .map(GuestAddress)
        .ok_or_else(|| {
            Box::new(fault(
                ComFaultClass::InvalidCall,
                "com-abi-gateway",
                "com-return-stack-pointer-overflow",
                "COM return stack pointer crosses the x86 address limit",
                trap,
                Some(target),
            ))
        })?;
    let bytes = memory
        .read(trap.stack_pointer, frame_bytes as usize)
        .map_err(|error| Box::new(fault_from_memory(error, trap, Some(target))))?;
    let words = bytes
        .chunks_exact(4)
        .map(|word| u32::from_le_bytes(word.try_into().expect("x86 COM words are four bytes")))
        .collect::<Vec<_>>();
    if words[0] != trap.return_address.0 {
        return Err(Box::new(fault(
            ComFaultClass::InvalidCall,
            "com-abi-gateway",
            "com-return-address-mismatch",
            "guest COM stack return address does not match the trapped CPU state",
            trap,
            Some(target),
        )));
    }
    let this_pointer = GuestAddress(words[1]);
    if this_pointer != target.interface.guest_pointer {
        return Err(Box::new(fault(
            ComFaultClass::InvalidCall,
            "com-abi-gateway",
            "com-this-interface-mismatch",
            "COM this pointer does not identify the interface that owns the trapped endpoint",
            trap,
            Some(target),
        )));
    }
    Ok(ComCallFrame {
        object_id: target.object.id,
        interface_id: target.interface.id,
        interface: target.interface.name.clone(),
        iid: target.interface.iid,
        vtable_slot: target.method.slot,
        method: target.method.name.clone(),
        endpoint_address: target.method.endpoint,
        call_site: trap.call_site,
        stack_pointer: trap.stack_pointer,
        return_stack_pointer,
        return_address: trap.return_address,
        this_pointer,
        argument_bytes,
        explicit_arguments: words[2..].to_vec(),
    })
}

fn complete_call(
    target: RuntimeComMethodTarget,
    frame: ComCallFrame,
    return_value: ComReturnValue,
    reference_count: u32,
    object_retired: bool,
    guest_writes: Vec<ComGuestWriteReceipt>,
) -> ComDispatchOutcome {
    debug_assert!(return_value_matches(
        target.method.return_kind,
        return_value
    ));
    ComDispatchOutcome::Completed(ComCallCompletion {
        instruction_pointer: frame.return_address,
        stack_pointer: frame.return_stack_pointer,
        frame,
        return_value,
        reference_count,
        object_retired,
        guest_writes,
    })
}

fn return_value_matches(kind: RuntimeComReturnKind, value: ComReturnValue) -> bool {
    matches!(
        (kind, value),
        (RuntimeComReturnKind::HResult, ComReturnValue::HResult(_))
            | (RuntimeComReturnKind::U32, ComReturnValue::U32(_))
    )
}

fn preflight_return_kind(
    target: &RuntimeComMethodTarget,
    trap: ComCallTrap,
) -> Result<(), Box<ComFault>> {
    let expected = match target.method.implementation {
        RuntimeComMethodImplementation::QueryInterface => RuntimeComReturnKind::HResult,
        RuntimeComMethodImplementation::AddRef | RuntimeComMethodImplementation::Release => {
            RuntimeComReturnKind::U32
        }
        RuntimeComMethodImplementation::Frontend => return Ok(()),
        RuntimeComMethodImplementation::Unsupported => return Ok(()),
    };
    if target.method.return_kind != expected {
        return Err(Box::new(fault(
            ComFaultClass::RuntimeDefect,
            "com-abi-gateway",
            "com-return-kind-mismatch",
            "COM handler return value does not match its vtable ABI",
            trap,
            Some(target),
        )));
    }
    Ok(())
}

fn validate_object_spec(spec: &ComObjectSpec) -> Result<(), RuntimeStateError> {
    if spec.class_identity.is_empty() {
        return Err(RuntimeStateError::invalid_input(
            "runtime-com",
            "com-class-identity-invalid",
            "COM class identity must be nonempty",
        ));
    }
    if spec.interfaces.is_empty() || spec.interfaces.len() > RUNTIME_COM_MAX_INTERFACES_PER_OBJECT {
        return Err(RuntimeStateError::invalid_input(
            "runtime-com",
            "com-interface-count-invalid",
            "COM object interface count is empty or exceeds the bounded foundation",
        ));
    }
    let endpoint_count = spec
        .interfaces
        .iter()
        .try_fold(0_usize, |total, interface| {
            total.checked_add(interface.methods.len())
        })
        .ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-com",
                "com-endpoint-count-overflow",
                "COM endpoint count overflowed host accounting",
            )
        })?;
    if endpoint_count > RUNTIME_COM_MAX_ENDPOINTS_PER_OBJECT {
        return Err(RuntimeStateError::resource_limit(
            "runtime-com",
            "com-endpoint-capacity-exceeded",
            "COM object exceeds its one-page endpoint capacity",
        ));
    }
    Ok(())
}

fn plan_layout(
    base: GuestAddress,
    spec: &ComObjectSpec,
) -> Result<PlannedComLayout, RuntimeStateError> {
    let interface_bytes = u32::try_from(spec.interfaces.len())
        .ok()
        .and_then(|count| count.checked_mul(4))
        .ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-com",
                "com-interface-layout-overflow",
                "COM interface pointer layout overflowed",
            )
        })?;
    let mut vtable_cursor = align_up(interface_bytes, 16).ok_or_else(|| {
        RuntimeStateError::resource_limit(
            "runtime-com",
            "com-vtable-layout-overflow",
            "COM vtable layout overflowed alignment",
        )
    })?;
    let endpoint_base = add_address(base, GUEST_PAGE_SIZE as u32)?;
    let mut endpoint_index = 0_u32;
    let mut interfaces = Vec::with_capacity(spec.interfaces.len());
    let mut data_writes = Vec::new();
    let mut endpoint_writes = Vec::new();

    for (interface_index, interface) in spec.interfaces.iter().enumerate() {
        let interface_offset = u32::try_from(interface_index)
            .ok()
            .and_then(|index| index.checked_mul(4))
            .ok_or_else(|| {
                RuntimeStateError::resource_limit(
                    "runtime-com",
                    "com-interface-layout-overflow",
                    "COM interface pointer offset overflowed",
                )
            })?;
        let guest_pointer = add_address(base, interface_offset)?;
        let vtable = add_address(base, vtable_cursor)?;
        let vtable_bytes = u32::try_from(interface.methods.len())
            .ok()
            .and_then(|count| count.checked_mul(4))
            .ok_or_else(|| {
                RuntimeStateError::resource_limit(
                    "runtime-com",
                    "com-vtable-layout-overflow",
                    "COM vtable byte count overflowed",
                )
            })?;
        vtable_cursor = vtable_cursor.checked_add(vtable_bytes).ok_or_else(|| {
            RuntimeStateError::resource_limit(
                "runtime-com",
                "com-vtable-layout-overflow",
                "COM vtable layout crossed the data page",
            )
        })?;
        if vtable_cursor > GUEST_PAGE_SIZE as u32 {
            return Err(RuntimeStateError::resource_limit(
                "runtime-com",
                "com-vtable-data-page-exhausted",
                "COM interface pointers and vtables exceed one data page",
            ));
        }
        data_writes.push((guest_pointer, vtable.0.to_le_bytes().to_vec()));
        let mut vtable_image = Vec::with_capacity(vtable_bytes as usize);
        let mut methods = Vec::with_capacity(interface.methods.len());
        for (slot, method) in interface.methods.iter().enumerate() {
            let slot = u16::try_from(slot).expect("bounded COM method index fits u16");
            let endpoint_offset = endpoint_index
                .checked_mul(RUNTIME_COM_ENDPOINT_SLOT_SIZE)
                .ok_or_else(|| {
                    RuntimeStateError::resource_limit(
                        "runtime-com",
                        "com-endpoint-layout-overflow",
                        "COM endpoint offset overflowed",
                    )
                })?;
            let endpoint = add_address(endpoint_base, endpoint_offset)?;
            endpoint_index += 1;
            vtable_image.extend_from_slice(&endpoint.0.to_le_bytes());
            methods.push(RuntimeComMethodRegistration {
                slot,
                name: method.name.clone(),
                endpoint,
                explicit_argument_words: method.explicit_argument_words,
                return_kind: method.return_kind,
                implementation: method.implementation,
            });
            endpoint_writes.push((
                endpoint,
                endpoint_image(
                    u16::try_from(interface_index).expect("bounded COM interface index fits u16"),
                    slot,
                ),
            ));
        }
        data_writes.push((vtable, vtable_image));
        interfaces.push(RuntimeComInterfaceRegistration {
            iid: interface.iid,
            name: interface.name.clone(),
            guest_pointer,
            vtable,
            methods,
        });
    }
    Ok(PlannedComLayout {
        interfaces,
        data_writes,
        endpoint_writes,
    })
}

fn endpoint_image(interface: u16, slot: u16) -> Vec<u8> {
    let mut bytes = vec![0_u8; RUNTIME_COM_ENDPOINT_SLOT_SIZE as usize];
    bytes[0..2].copy_from_slice(&[0x0f, 0x0b]);
    bytes[2..6].copy_from_slice(b"VCOM");
    bytes[6..8].copy_from_slice(&interface.to_le_bytes());
    bytes[8..10].copy_from_slice(&slot.to_le_bytes());
    bytes
}

fn align_up(value: u32, alignment: u32) -> Option<u32> {
    value
        .checked_add(alignment - 1)
        .map(|value| value & !(alignment - 1))
}

fn add_address(base: GuestAddress, offset: u32) -> Result<GuestAddress, RuntimeStateError> {
    base.0.checked_add(offset).map(GuestAddress).ok_or_else(|| {
        RuntimeStateError::resource_limit(
            "runtime-com",
            "com-layout-address-overflow",
            "COM object layout crosses the x86 address limit",
        )
    })
}

fn fault_from_runtime(
    error: RuntimeStateError,
    trap: ComCallTrap,
    target: Option<&RuntimeComMethodTarget>,
) -> ComFault {
    let class = match error.class {
        RuntimeStateErrorClass::NotFound => ComFaultClass::Unsupported,
        RuntimeStateErrorClass::InvalidInput | RuntimeStateErrorClass::Conflict => {
            ComFaultClass::InvalidCall
        }
        RuntimeStateErrorClass::ResourceLimit => ComFaultClass::ResourceLimit,
        RuntimeStateErrorClass::RuntimeDefect => ComFaultClass::RuntimeDefect,
    };
    fault(class, error.stage, error.code, error.message, trap, target)
}

fn fault_from_memory(
    error: GuestMemoryError,
    trap: ComCallTrap,
    target: Option<&RuntimeComMethodTarget>,
) -> ComFault {
    let class = match error.class {
        GuestMemoryErrorClass::ResourceLimit => ComFaultClass::ResourceLimit,
        GuestMemoryErrorClass::RuntimeDefect => ComFaultClass::RuntimeDefect,
        GuestMemoryErrorClass::InvalidRequest
        | GuestMemoryErrorClass::Conflict
        | GuestMemoryErrorClass::SynchronizationRequired
        | GuestMemoryErrorClass::Fault => ComFaultClass::InvalidCall,
    };
    fault(class, error.stage, error.code, error.message, trap, target)
}

fn fault(
    class: ComFaultClass,
    stage: &'static str,
    code: &'static str,
    message: impl Into<String>,
    trap: ComCallTrap,
    target: Option<&RuntimeComMethodTarget>,
) -> ComFault {
    ComFault {
        class,
        stage,
        code,
        message: message.into(),
        call_site: trap.call_site,
        target: target.map(|target| ComFaultTarget {
            object_id: target.object.id,
            interface_id: target.interface.id,
            interface: target.interface.name.clone(),
            iid: target.interface.iid,
            vtable_slot: target.method.slot,
            method: target.method.name.clone(),
            endpoint_address: target.method.endpoint,
        }),
    }
}

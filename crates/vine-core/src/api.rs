use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use crate::callbacks::{
    CALLBACK_SCRATCH_ARENA_BASE, CALLBACK_SCRATCH_ARENA_SIZE, CALLBACK_SCRATCH_SLOT_SIZE,
};
use crate::com::{
    ComCallTrap, ComDispatchOutcome, ComGuestWriteReceipt, ComHResult, ComObjectSpec,
    InstalledComObject, dispatch_com_call, install_com_object,
};
use crate::environment::InitialProcessEnvironment;
use crate::imports::{ImportThunk, ImportThunkDisposition, ImportThunkId, RuntimeExportCatalog};
use crate::materialize::ImageMaterialization;
use crate::memory::{
    GuestAddress, GuestMemory, GuestMemoryError, GuestMemoryErrorClass, GuestMemoryTransaction,
    GuestRange, GuestWriteResult, MemoryAccess, MemoryOwnerKind, MemoryRegionKind, PageProtection,
};
use crate::module_images::{RuntimeModuleImage, RuntimeModuleImageError};
use crate::pe32::{ImportTarget, LoaderIngressReport};
use crate::runtime::{
    LOGICAL_WINDOW_HANDLE_NAMESPACE, ProcessAddressAllocationClassification,
    ProcessAddressAllocationLifetime, ProcessAddressAllocationPurpose,
    ProcessAddressAllocationRecord, ProcessAddressAllocationReleasePolicy, RuntimeAudioDevice,
    RuntimeAudioDeviceCreateRequest, RuntimeCallbackFrameId, RuntimeCallbackScratch,
    RuntimeCreateDibSectionResult, RuntimeCreateThreadRequest, RuntimeCreateThreadResult,
    RuntimeDynamicEndpoint, RuntimeGdiDibSectionDescriptor, RuntimeGdiObject,
    RuntimeGdiTextRasterResult, RuntimeGdiTextRasterTarget, RuntimeGuestImport,
    RuntimeGuestImportBinding, RuntimeGuestImportResolution, RuntimeHeapAllocation,
    RuntimeHeapInitialization, RuntimeHeapTeardown, RuntimeSetWindowPosCallbackAction,
    RuntimeSetWindowPosCallbackMessage, RuntimeSetWindowPosDefaultCallbackAction,
    RuntimeSetWindowPosDefaultCallbackMessage, RuntimeSetWindowPosDefaultOperation,
    RuntimeSetWindowPosOperation, RuntimeState, RuntimeStateError, RuntimeStateErrorClass,
    RuntimeWindowCreationCallbackAction, RuntimeWindowCreationMessage, RuntimeWindowId,
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ApiSymbol {
    Name { name: String },
    Ordinal { ordinal: u16 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ApiEndpointBinding {
    ImportAddressTable,
    DynamicExport,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiEndpointIdentity {
    pub id: ImportThunkId,
    pub address: GuestAddress,
    pub module: String,
    pub symbol: ApiSymbol,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iat_address: Option<GuestAddress>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disposition: Option<ImportThunkDisposition>,
    #[serde(skip)]
    pub binding: ApiEndpointBinding,
}

impl ApiEndpointIdentity {
    pub fn from_import_thunk(thunk: &ImportThunk) -> Self {
        let symbol = match &thunk.target {
            ImportTarget::Name { name, .. } => ApiSymbol::Name { name: name.clone() },
            ImportTarget::Ordinal { ordinal } => ApiSymbol::Ordinal { ordinal: *ordinal },
        };
        Self {
            id: thunk.id,
            address: thunk.address,
            module: thunk.module.clone(),
            symbol,
            iat_address: Some(thunk.iat_address),
            disposition: Some(thunk.disposition),
            binding: ApiEndpointBinding::ImportAddressTable,
        }
    }

    pub fn from_runtime_guest_import(import: &RuntimeGuestImport) -> Option<Self> {
        let RuntimeGuestImportResolution::RuntimeExport { endpoint } = &import.resolution else {
            return None;
        };
        let symbol = match &import.target {
            ImportTarget::Name { name, .. } => ApiSymbol::Name { name: name.clone() },
            ImportTarget::Ordinal { ordinal } => ApiSymbol::Ordinal { ordinal: *ordinal },
        };
        Some(Self {
            id: endpoint.id,
            address: endpoint.address,
            module: import.requested_module.clone(),
            symbol,
            iat_address: Some(import.iat_address),
            disposition: Some(ImportThunkDisposition::DeclaredRuntimeExport),
            binding: ApiEndpointBinding::ImportAddressTable,
        })
    }

    pub fn from_dynamic_endpoint(endpoint: &RuntimeDynamicEndpoint) -> Self {
        Self {
            id: ImportThunkId(endpoint.id),
            address: endpoint.address,
            module: endpoint.module.clone(),
            symbol: ApiSymbol::Name {
                name: endpoint.name.clone(),
            },
            iat_address: None,
            disposition: None,
            binding: ApiEndpointBinding::DynamicExport,
        }
    }

    pub fn matches(&self, module: &str, symbol: &ApiSymbol) -> bool {
        self.module.eq_ignore_ascii_case(module) && &self.symbol == symbol
    }

    fn registry_key(&self) -> (ApiEndpointBinding, ImportThunkId) {
        (self.binding, self.id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CallingConvention {
    Cdecl,
    Stdcall,
    Thiscall,
    Fastcall,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum StackCleanup {
    Caller,
    Callee,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ApiDirection {
    In,
    Out,
    InOut,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ApiWriteRequirement {
    None,
    Optional,
    Required,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ApiStringEncoding {
    Windows1252,
    Windows932,
    Utf16Le,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ApiPointeeSchema {
    Bytes,
    Struct {
        type_id: String,
    },
    String {
        encoding: ApiStringEncoding,
        maximum_code_units: u32,
        nul_terminated: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiPointerConstraint {
    pub nullable: bool,
    pub alignment: u32,
    pub byte_length: u32,
    pub pointee: ApiPointeeSchema,
    pub write_requirement: ApiWriteRequirement,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiDynamicPointerConstraint {
    pub nullable: bool,
    pub alignment: u32,
    pub minimum_byte_length: u32,
    pub maximum_byte_length: u32,
    pub pointee: ApiPointeeSchema,
    pub write_requirement: ApiWriteRequirement,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiScalarOrPointerConstraint {
    pub alignment: u32,
    pub minimum_byte_length: u32,
    pub maximum_byte_length: u32,
    pub pointee: ApiPointeeSchema,
    pub write_requirement: ApiWriteRequirement,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ApiValueSchema {
    U32,
    Bool32,
    OpaquePointer {
        nullable: bool,
        alignment: u32,
    },
    NameOrOrdinal {
        encoding: ApiStringEncoding,
        maximum_code_units: u32,
    },
    Pointer {
        constraint: ApiPointerConstraint,
    },
    DynamicPointer {
        constraint: ApiDynamicPointerConstraint,
    },
    ScalarOrPointer {
        constraint: ApiScalarOrPointerConstraint,
    },
    LogicalHandle {
        namespace: String,
        nullable: bool,
    },
}

impl ApiValueSchema {
    fn stack_words(&self) -> u32 {
        1
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiArgumentSchema {
    pub name: String,
    pub direction: ApiDirection,
    pub value: ApiValueSchema,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ApiReturnSchema {
    Void,
    U16,
    U32,
    HResult,
    Bool32,
    OpaquePointer { nullable: bool },
    LogicalPointer { namespace: String, nullable: bool },
    LogicalHandle { namespace: String, nullable: bool },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AbiSchema {
    pub calling_convention: CallingConvention,
    pub arguments: Vec<ApiArgumentSchema>,
    pub return_value: ApiReturnSchema,
    pub stack_cleanup: StackCleanup,
}

impl AbiSchema {
    pub fn argument_bytes(&self) -> Result<u32, ApiFault> {
        let words = self.arguments.iter().try_fold(0_u32, |total, argument| {
            total
                .checked_add(argument.value.stack_words())
                .ok_or_else(|| {
                    ApiFault::resource_limit(
                        "api-contract",
                        "argument-word-count-overflow",
                        "API argument word count overflowed x86 ABI accounting",
                    )
                })
        })?;
        words.checked_mul(4).ok_or_else(|| {
            ApiFault::resource_limit(
                "api-contract",
                "argument-byte-count-overflow",
                "API argument byte count overflowed x86 ABI accounting",
            )
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum LastErrorPolicy {
    Preserve,
    FromHandler,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReentrancyPolicy {
    Disallowed,
    Allowed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ApiBehavior {
    Immediate,
    ImmediateWithCommittedIndirectGuestWrites,
    Deferred,
    ImmediateOrDeferred,
}

impl ApiBehavior {
    fn permits_immediate(self) -> bool {
        !matches!(self, Self::Deferred)
    }

    fn permits_deferred(self) -> bool {
        matches!(self, Self::Deferred | Self::ImmediateOrDeferred)
    }

    fn permits_committed_indirect_guest_writes(self) -> bool {
        matches!(self, Self::ImmediateWithCommittedIndirectGuestWrites)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CallbackPolicy {
    Forbidden,
    Optional,
    Required,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiContract {
    pub endpoint: ApiEndpointIdentity,
    pub abi: AbiSchema,
    pub last_error_policy: LastErrorPolicy,
    pub reentrancy_policy: ReentrancyPolicy,
    pub behavior: ApiBehavior,
    pub callback_policy: CallbackPolicy,
    pub implementation_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ApiFaultClass {
    InvalidCall,
    Unsupported,
    ResourceLimit,
    RuntimeDefect,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiFault {
    pub class: ApiFaultClass,
    pub stage: &'static str,
    pub code: &'static str,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<Box<ApiEndpointIdentity>>,
}

impl ApiFault {
    pub fn invalid_call(
        stage: &'static str,
        code: &'static str,
        message: impl Into<String>,
    ) -> Self {
        Self::new(ApiFaultClass::InvalidCall, stage, code, message)
    }

    pub fn unsupported(
        stage: &'static str,
        code: &'static str,
        message: impl Into<String>,
    ) -> Self {
        Self::new(ApiFaultClass::Unsupported, stage, code, message)
    }

    pub fn resource_limit(
        stage: &'static str,
        code: &'static str,
        message: impl Into<String>,
    ) -> Self {
        Self::new(ApiFaultClass::ResourceLimit, stage, code, message)
    }

    pub fn runtime_defect(
        stage: &'static str,
        code: &'static str,
        message: impl Into<String>,
    ) -> Self {
        Self::new(ApiFaultClass::RuntimeDefect, stage, code, message)
    }

    fn new(
        class: ApiFaultClass,
        stage: &'static str,
        code: &'static str,
        message: impl Into<String>,
    ) -> Self {
        Self {
            class,
            stage,
            code,
            message: message.into(),
            endpoint: None,
        }
    }

    fn with_endpoint(mut self, endpoint: &ApiEndpointIdentity) -> Self {
        self.endpoint = Some(Box::new(endpoint.clone()));
        self
    }

    pub(crate) fn from_memory(error: GuestMemoryError, endpoint: &ApiEndpointIdentity) -> Self {
        let class = match error.class {
            GuestMemoryErrorClass::ResourceLimit => ApiFaultClass::ResourceLimit,
            GuestMemoryErrorClass::RuntimeDefect => ApiFaultClass::RuntimeDefect,
            GuestMemoryErrorClass::InvalidRequest
            | GuestMemoryErrorClass::Conflict
            | GuestMemoryErrorClass::SynchronizationRequired
            | GuestMemoryErrorClass::Fault => ApiFaultClass::InvalidCall,
        };
        Self {
            class,
            stage: error.stage,
            code: error.code,
            message: error.message,
            endpoint: Some(Box::new(endpoint.clone())),
        }
    }

    pub(crate) fn from_runtime(error: RuntimeStateError, endpoint: &ApiEndpointIdentity) -> Self {
        let class = match error.class {
            RuntimeStateErrorClass::ResourceLimit => ApiFaultClass::ResourceLimit,
            RuntimeStateErrorClass::RuntimeDefect => ApiFaultClass::RuntimeDefect,
            RuntimeStateErrorClass::InvalidInput
            | RuntimeStateErrorClass::NotFound
            | RuntimeStateErrorClass::Conflict => ApiFaultClass::InvalidCall,
        };
        Self {
            class,
            stage: error.stage,
            code: error.code,
            message: error.message,
            endpoint: Some(Box::new(endpoint.clone())),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiTrap {
    pub endpoint_id: ImportThunkId,
    pub endpoint_address: GuestAddress,
    pub stack_pointer: GuestAddress,
    pub return_address: GuestAddress,
    pub argument_bytes: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiOwnedTrap {
    pub endpoint_id: ImportThunkId,
    pub endpoint_address: GuestAddress,
    pub stack_pointer: GuestAddress,
    pub return_address: GuestAddress,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DecodedApiArgument {
    pub schema: ApiArgumentSchema,
    pub words: Vec<u32>,
}

impl DecodedApiArgument {
    pub fn as_u32(&self) -> Option<u32> {
        self.words.first().copied()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiCallFrame {
    pub endpoint: ApiEndpointIdentity,
    pub stack_pointer: GuestAddress,
    pub return_address: GuestAddress,
    pub argument_bytes: u32,
    pub arguments: Vec<DecodedApiArgument>,
}

impl ApiCallFrame {
    pub fn argument_u32(&self, index: usize) -> Option<u32> {
        self.arguments
            .get(index)
            .and_then(DecodedApiArgument::as_u32)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "kebab-case")]
pub enum ApiReturnValue {
    Void,
    U16(u16),
    U32(u32),
    HResult(ComHResult),
    Bool32(bool),
    OpaquePointer(u32),
    LogicalPointer { namespace: String, value: u32 },
    LogicalHandle { namespace: String, value: u32 },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiGuestWrite {
    pub address: GuestAddress,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiImmediate {
    pub return_value: ApiReturnValue,
    pub guest_writes: Vec<ApiGuestWrite>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub committed_guest_writes: Vec<ApiGuestWriteReceipt>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error: Option<u32>,
}

impl ApiImmediate {
    pub fn new(return_value: ApiReturnValue) -> Self {
        Self {
            return_value,
            guest_writes: Vec::new(),
            committed_guest_writes: Vec::new(),
            last_error: None,
        }
    }

    pub fn with_guest_write(mut self, address: GuestAddress, bytes: Vec<u8>) -> Self {
        self.guest_writes.push(ApiGuestWrite { address, bytes });
        self
    }

    pub fn with_committed_guest_write(mut self, receipt: ApiGuestWriteReceipt) -> Self {
        self.committed_guest_writes.push(receipt);
        self
    }

    pub fn with_last_error(mut self, last_error: u32) -> Self {
        self.last_error = Some(last_error);
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostRequest {
    pub request_id: u64,
    pub service: String,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostCompletion {
    pub request_id: u64,
    pub result: ApiImmediate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuestCallbackRequest {
    pub target: GuestAddress,
    pub arguments: Vec<GuestCallbackArgument>,
    pub scratch: Vec<u8>,
    pub scratch_pointer_relocations: Vec<GuestCallbackScratchPointerRelocation>,
    pub return_sentinel: GuestAddress,
    pub completion_policy: GuestCallbackCompletionPolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuestCallbackScratchPointerRelocation {
    pub write_offset: u32,
    pub target_offset: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "kebab-case")]
pub enum GuestCallbackArgument {
    U32(u32),
    ScratchPointer { offset: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum GuestCallbackCompletionPolicy {
    ReturnU32,
    ReturnBool32,
    ReturnConstantU32 {
        value: u32,
    },
    CreateWindowExA {
        window_id: RuntimeWindowId,
        window_handle: u32,
        message: RuntimeWindowCreationMessage,
    },
    SetWindowPos {
        operation: RuntimeSetWindowPosOperation,
        message: RuntimeSetWindowPosCallbackMessage,
    },
    SetWindowPosDefault {
        operation: RuntimeSetWindowPosDefaultOperation,
        message: RuntimeSetWindowPosDefaultCallbackMessage,
    },
}

impl GuestCallbackCompletionPolicy {
    fn matches_return_schema(self, schema: &ApiReturnSchema) -> bool {
        match (self, schema) {
            (Self::ReturnU32, ApiReturnSchema::U32)
            | (Self::ReturnConstantU32 { .. }, ApiReturnSchema::U32)
            | (Self::SetWindowPosDefault { .. }, ApiReturnSchema::U32)
            | (Self::ReturnBool32, ApiReturnSchema::Bool32)
            | (Self::SetWindowPos { .. }, ApiReturnSchema::Bool32) => true,
            (
                Self::CreateWindowExA { .. },
                ApiReturnSchema::LogicalHandle {
                    namespace,
                    nullable: true,
                },
            ) => namespace == LOGICAL_WINDOW_HANDLE_NAMESPACE,
            _ => false,
        }
    }

    fn complete_direct(self, result: u32) -> Option<ApiImmediate> {
        let value = match self {
            Self::ReturnU32 => ApiReturnValue::U32(result),
            Self::ReturnBool32 => ApiReturnValue::Bool32(result != 0),
            Self::ReturnConstantU32 { value } => ApiReturnValue::U32(value),
            Self::CreateWindowExA { .. }
            | Self::SetWindowPos { .. }
            | Self::SetWindowPosDefault { .. } => return None,
        };
        Some(ApiImmediate::new(value))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeGuestCallbackSemanticAction {
    WindowCreation(RuntimeWindowCreationCallbackAction),
    SetWindowPos(RuntimeSetWindowPosCallbackAction),
    SetWindowPosDefault(RuntimeSetWindowPosDefaultCallbackAction),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreparedGuestCallback {
    pub frame_id: RuntimeCallbackFrameId,
    pub allocation_start: GuestAddress,
    pub stack_pointer: GuestAddress,
    pub scratch_address: GuestAddress,
    pub scratch_bytes: u32,
    pub frame_bytes: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GuestCallbackFrameImage {
    allocation_start: GuestAddress,
    stack_pointer: GuestAddress,
    scratch_address: GuestAddress,
    scratch_bytes: u32,
    frame_bytes: u32,
    stack: Vec<u8>,
    scratch: Vec<u8>,
}

impl GuestCallbackFrameImage {
    fn prepared(&self, frame_id: RuntimeCallbackFrameId) -> PreparedGuestCallback {
        PreparedGuestCallback {
            frame_id,
            allocation_start: self.allocation_start,
            stack_pointer: self.stack_pointer,
            scratch_address: self.scratch_address,
            scratch_bytes: self.scratch_bytes,
            frame_bytes: self.frame_bytes,
        }
    }
}

fn callback_scratch_byte_length(call: &ApiGuestCallbackCall) -> Result<u32, ApiFault> {
    let endpoint = &call.frame.endpoint;
    let byte_length = u32::try_from(call.request.scratch.len()).map_err(|_| {
        ApiFault::resource_limit(
            "api-callback",
            "callback-scratch-size-unrepresentable",
            "guest callback scratch does not fit x86 accounting",
        )
        .with_endpoint(endpoint)
    })?;
    if byte_length > CALLBACK_SCRATCH_SLOT_SIZE {
        return Err(ApiFault::resource_limit(
            "api-callback",
            "callback-scratch-slot-capacity-exceeded",
            format!(
                "guest callback scratch requires {byte_length} bytes; slot capacity is {CALLBACK_SCRATCH_SLOT_SIZE}"
            ),
        )
        .with_endpoint(endpoint));
    }
    Ok(byte_length)
}

fn validate_callback_scratch_allocation(
    allocation: &ProcessAddressAllocationRecord,
    endpoint: &ApiEndpointIdentity,
) -> Result<(), ApiFault> {
    if allocation.purpose != ProcessAddressAllocationPurpose::CallbackScratch
        || allocation.owner != "runtime-callback-scratch"
        || allocation.range.base != CALLBACK_SCRATCH_ARENA_BASE
        || allocation.range.byte_length != CALLBACK_SCRATCH_ARENA_SIZE
        || allocation.usable_byte_length != CALLBACK_SCRATCH_ARENA_SIZE
        || allocation.classification != ProcessAddressAllocationClassification::RuntimeControl
        || allocation.lifetime != ProcessAddressAllocationLifetime::Process
        || allocation.release_policy != ProcessAddressAllocationReleasePolicy::Permanent
        || allocation.executable
    {
        return Err(ApiFault::runtime_defect(
            "api-callback",
            "callback-scratch-address-allocation-mismatch",
            "callback scratch address allocation does not match the permanent runtime arena",
        )
        .with_endpoint(endpoint));
    }
    Ok(())
}

fn callback_scratch_slot_range(
    scratch: RuntimeCallbackScratch,
    endpoint: &ApiEndpointIdentity,
) -> Result<GuestRange, ApiFault> {
    if scratch.byte_length == 0 || scratch.byte_length > scratch.capacity {
        return Err(ApiFault::runtime_defect(
            "api-callback",
            "callback-scratch-range-invalid",
            "live callback scratch has an invalid byte length",
        )
        .with_endpoint(endpoint));
    }
    let expected_offset = u32::from(scratch.slot)
        .checked_mul(CALLBACK_SCRATCH_SLOT_SIZE)
        .ok_or_else(|| {
            ApiFault::runtime_defect(
                "api-callback",
                "callback-scratch-slot-offset-overflow",
                "callback scratch slot offset overflowed x86 accounting",
            )
            .with_endpoint(endpoint)
        })?;
    let expected_address = CALLBACK_SCRATCH_ARENA_BASE
        .0
        .checked_add(expected_offset)
        .map(GuestAddress)
        .ok_or_else(|| {
            ApiFault::runtime_defect(
                "api-callback",
                "callback-scratch-slot-address-overflow",
                "callback scratch slot address overflowed x86 accounting",
            )
            .with_endpoint(endpoint)
        })?;
    if scratch.address != expected_address || scratch.capacity != CALLBACK_SCRATCH_SLOT_SIZE {
        return Err(ApiFault::runtime_defect(
            "api-callback",
            "callback-scratch-slot-layout-mismatch",
            "callback scratch slot does not match the versioned arena layout",
        )
        .with_endpoint(endpoint));
    }
    let range = GuestRange::new(scratch.address, u64::from(scratch.capacity))
        .map_err(|error| ApiFault::from_memory(error, endpoint))?;
    let arena_end = u64::from(CALLBACK_SCRATCH_ARENA_BASE.0)
        .checked_add(CALLBACK_SCRATCH_ARENA_SIZE)
        .ok_or_else(|| {
            ApiFault::runtime_defect(
                "api-callback",
                "callback-scratch-arena-end-overflow",
                "callback scratch arena end overflowed x86 accounting",
            )
            .with_endpoint(endpoint)
        })?;
    if range.end() > arena_end {
        return Err(ApiFault::runtime_defect(
            "api-callback",
            "callback-scratch-slot-outside-arena",
            "callback scratch slot extends outside the permanent arena",
        )
        .with_endpoint(endpoint));
    }
    Ok(range)
}

fn callback_scratch_region(
    memory: &GuestMemory,
    allocation: &ProcessAddressAllocationRecord,
    endpoint: &ApiEndpointIdentity,
) -> Result<Option<crate::memory::MemoryRegionSnapshot>, ApiFault> {
    let mut regions = memory
        .region_snapshots()
        .into_iter()
        .filter(|region| region.address_allocation_id == Some(allocation.id.0));
    let region = regions.next();
    if regions.next().is_some() {
        return Err(ApiFault::runtime_defect(
            "api-callback",
            "callback-scratch-region-duplicate",
            "callback scratch address allocation owns more than one guest-memory region",
        )
        .with_endpoint(endpoint));
    }
    if let Some(region) = region {
        let expected_range =
            GuestRange::new(CALLBACK_SCRATCH_ARENA_BASE, CALLBACK_SCRATCH_ARENA_SIZE)
                .map_err(|error| ApiFault::from_memory(error, endpoint))?;
        if region.kind != MemoryRegionKind::Runtime || region.range != expected_range {
            return Err(ApiFault::runtime_defect(
                "api-callback",
                "callback-scratch-region-mismatch",
                "callback scratch guest-memory region does not match its address allocation",
            )
            .with_endpoint(endpoint));
        }
        Ok(Some(region))
    } else {
        Ok(None)
    }
}

fn preflight_callback_scratch_mapping(
    memory: &GuestMemory,
    allocation: &ProcessAddressAllocationRecord,
    scratch: Option<RuntimeCallbackScratch>,
    endpoint: &ApiEndpointIdentity,
) -> Result<(), ApiFault> {
    let Some(scratch) = scratch else {
        return Ok(());
    };
    validate_callback_scratch_allocation(allocation, endpoint)?;
    let slot_range = callback_scratch_slot_range(scratch, endpoint)?;
    let region = callback_scratch_region(memory, allocation, endpoint)?;
    let additional_owners = usize::from(region.is_none());
    let additional_regions = additional_owners;
    let additional_pages = match memory
        .committed_page(slot_range.start())
        .map_err(|error| ApiFault::from_memory(error, endpoint))?
    {
        None => 1,
        Some((owner, protection)) => {
            let region = region.as_ref().ok_or_else(|| {
                ApiFault::runtime_defect(
                    "api-callback",
                    "callback-scratch-page-without-region",
                    "callback scratch page is committed without its permanent region",
                )
                .with_endpoint(endpoint)
            })?;
            if owner != region.owner
                || protection
                    != (PageProtection {
                        read: true,
                        write: true,
                        execute: false,
                    })
            {
                return Err(ApiFault::runtime_defect(
                    "api-callback",
                    "callback-scratch-page-mismatch",
                    "callback scratch page owner or protection does not match its region",
                )
                .with_endpoint(endpoint));
            }
            0
        }
    };
    memory
        .preflight_growth(
            additional_owners,
            additional_regions,
            additional_pages,
            "api-callback",
        )
        .map_err(|error| ApiFault::from_memory(error, endpoint))
}

fn materialize_callback_scratch_mapping(
    memory: &mut GuestMemoryTransaction<'_>,
    allocation: &ProcessAddressAllocationRecord,
    scratch: Option<RuntimeCallbackScratch>,
    endpoint: &ApiEndpointIdentity,
) -> Result<(), ApiFault> {
    let Some(scratch) = scratch else {
        return Ok(());
    };
    let slot_range = callback_scratch_slot_range(scratch, endpoint)?;
    let region = match callback_scratch_region(memory, allocation, endpoint)? {
        Some(region) => region,
        None => {
            let owner = memory
                .create_owner(MemoryOwnerKind::Runtime)
                .map_err(|error| ApiFault::from_memory(error, endpoint))?;
            let range = memory
                .reserve_fixed_for_address_allocation(
                    allocation.id.0,
                    owner,
                    MemoryRegionKind::Runtime,
                    allocation.range.base,
                    allocation.range.byte_length,
                )
                .map_err(|error| ApiFault::from_memory(error, endpoint))?;
            crate::memory::MemoryRegionSnapshot {
                address_allocation_id: Some(allocation.id.0),
                owner,
                kind: MemoryRegionKind::Runtime,
                range,
            }
        }
    };
    match memory
        .committed_page(slot_range.start())
        .map_err(|error| ApiFault::from_memory(error, endpoint))?
    {
        None => {
            memory
                .commit(
                    region.owner,
                    slot_range,
                    PageProtection {
                        read: true,
                        write: true,
                        execute: false,
                    },
                )
                .map_err(|error| ApiFault::from_memory(error, endpoint))?;
        }
        Some((owner, protection))
            if owner == region.owner
                && protection
                    == (PageProtection {
                        read: true,
                        write: true,
                        execute: false,
                    }) => {}
        Some(_) => {
            return Err(ApiFault::runtime_defect(
                "api-callback",
                "callback-scratch-page-stale",
                "callback scratch page changed after mapping preflight",
            )
            .with_endpoint(endpoint));
        }
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApiHandlerResult {
    Immediate(ApiImmediate),
    Deferred(HostRequest),
    GuestCallback(GuestCallbackRequest),
    Fault(ApiFault),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "action", content = "value", rename_all = "kebab-case")]
pub enum RegisterUpdate {
    Preserve,
    Set(u32),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiGuestWriteReceipt {
    pub write: ApiGuestWrite,
    pub result: GuestWriteResult,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiImmediateCompletion {
    pub implementation_id: String,
    pub frame: ApiCallFrame,
    pub instruction_pointer: GuestAddress,
    pub stack_pointer: GuestAddress,
    pub eax: RegisterUpdate,
    pub edx: RegisterUpdate,
    pub eflags: RegisterUpdate,
    pub last_error: u32,
    pub guest_writes: Vec<ApiGuestWriteReceipt>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiDeferredCall {
    pub request: HostRequest,
    pub contract: ApiContract,
    pub frame: ApiCallFrame,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiGuestCallbackCall {
    pub request: GuestCallbackRequest,
    pub contract: ApiContract,
    pub frame: ApiCallFrame,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DispatchOutcome {
    Immediate(ApiImmediateCompletion),
    Deferred(ApiDeferredCall),
    GuestCallback(ApiGuestCallbackCall),
    Fault(ApiFault),
}

pub struct ExecutionSession<'a> {
    memory: &'a mut GuestMemory,
    runtime: &'a mut RuntimeState,
}

impl<'a> ExecutionSession<'a> {
    pub fn new(memory: &'a mut GuestMemory, runtime: &'a mut RuntimeState) -> Self {
        Self { memory, runtime }
    }

    pub fn memory(&self) -> &GuestMemory {
        self.memory
    }

    pub fn runtime(&self) -> &RuntimeState {
        self.runtime
    }

    pub fn runtime_mut(&mut self) -> &mut RuntimeState {
        self.runtime
    }

    pub fn create_guest_thread(
        &mut self,
        request: RuntimeCreateThreadRequest,
    ) -> Result<RuntimeCreateThreadResult, RuntimeStateError> {
        self.runtime.create_guest_thread(self.memory, request)
    }

    pub fn create_gdi_dib_section(
        &mut self,
        device_context_handle: u32,
        descriptor: RuntimeGdiDibSectionDescriptor,
        bits_output: GuestAddress,
    ) -> Result<(RuntimeCreateDibSectionResult, GuestWriteResult), RuntimeStateError> {
        self.runtime.create_gdi_dib_section(
            self.memory,
            device_context_handle,
            descriptor,
            bits_output,
        )
    }

    pub fn delete_gdi_object(
        &mut self,
        object_handle: u32,
    ) -> Result<RuntimeGdiObject, RuntimeStateError> {
        self.runtime.delete_gdi_object(self.memory, object_handle)
    }

    pub fn get_gdi_text_raster_target(
        &self,
        device_context_handle: u32,
    ) -> Result<RuntimeGdiTextRasterTarget, RuntimeStateError> {
        self.runtime
            .get_gdi_text_raster_target(device_context_handle)
    }

    pub fn commit_gdi_text_raster(
        &mut self,
        target: RuntimeGdiTextRasterTarget,
        byte_offset: u32,
        expected_bytes: &[u8],
        replacement_bytes: &[u8],
    ) -> Result<(RuntimeGdiTextRasterResult, GuestWriteResult), RuntimeStateError> {
        self.runtime.commit_gdi_text_raster(
            self.memory,
            target,
            byte_offset,
            expected_bytes,
            replacement_bytes,
        )
    }

    pub fn begin_guest_callback(
        &mut self,
        call: &ApiGuestCallbackCall,
    ) -> Result<PreparedGuestCallback, ApiFault> {
        let endpoint = &call.frame.endpoint;
        let scratch_bytes = callback_scratch_byte_length(call)?;
        let plan = self
            .runtime
            .plan_callback_push(
                call.request.target,
                call.request.return_sentinel,
                scratch_bytes,
            )
            .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
        let runtime_frame = plan.entered_frame().ok_or_else(|| {
            ApiFault::runtime_defect(
                "api-callback",
                "callback-entry-plan-frame-missing",
                "callback entry plan omitted its entered frame",
            )
            .with_endpoint(endpoint)
        })?;
        let image = self.prepare_guest_callback_frame(call, runtime_frame.scratch)?;
        let scratch_allocation = self
            .runtime
            .callback_scratch_allocation()
            .map_err(|error| ApiFault::from_runtime(error, endpoint))?
            .clone();
        preflight_callback_scratch_mapping(
            self.memory,
            &scratch_allocation,
            runtime_frame.scratch,
            endpoint,
        )?;
        let mut transaction = self.memory.begin_transaction();
        materialize_callback_scratch_mapping(
            &mut transaction,
            &scratch_allocation,
            runtime_frame.scratch,
            endpoint,
        )?;
        if !image.scratch.is_empty() {
            transaction
                .write(image.scratch_address, &image.scratch)
                .map_err(|error| ApiFault::from_memory(error, endpoint))?;
        }
        transaction
            .write(image.allocation_start, &image.stack)
            .map_err(|error| ApiFault::from_memory(error, endpoint))?;
        let committed = self
            .runtime
            .commit_callback_mutation(plan)
            .0
            .ok_or_else(|| {
                ApiFault::runtime_defect(
                    "api-callback",
                    "callback-entry-commit-frame-missing",
                    "validated callback entry did not commit an entered frame",
                )
                .with_endpoint(endpoint)
            })?;
        debug_assert_eq!(committed, runtime_frame);
        transaction.commit_transaction();
        Ok(image.prepared(runtime_frame.id))
    }

    pub fn preflight_guest_callback(&self, call: &ApiGuestCallbackCall) -> Result<(), ApiFault> {
        let endpoint = &call.frame.endpoint;
        let scratch_bytes = callback_scratch_byte_length(call)?;
        let plan = self
            .runtime
            .plan_callback_push(
                call.request.target,
                call.request.return_sentinel,
                scratch_bytes,
            )
            .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
        let runtime_frame = plan.entered_frame().ok_or_else(|| {
            ApiFault::runtime_defect(
                "api-callback",
                "callback-entry-plan-frame-missing",
                "callback entry plan omitted its entered frame",
            )
            .with_endpoint(endpoint)
        })?;
        self.prepare_guest_callback_frame(call, runtime_frame.scratch)?;
        let scratch_allocation = self
            .runtime
            .callback_scratch_allocation()
            .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
        preflight_callback_scratch_mapping(
            self.memory,
            scratch_allocation,
            runtime_frame.scratch,
            endpoint,
        )
    }

    pub fn replace_guest_callback(
        &mut self,
        call: &ApiGuestCallbackCall,
        expected: RuntimeCallbackFrameId,
    ) -> Result<PreparedGuestCallback, ApiFault> {
        let endpoint = &call.frame.endpoint;
        let scratch_bytes = callback_scratch_byte_length(call)?;
        let plan = self
            .runtime
            .plan_callback_replace(
                expected,
                call.request.target,
                call.request.return_sentinel,
                scratch_bytes,
            )
            .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
        let runtime_frame = plan.entered_frame().ok_or_else(|| {
            ApiFault::runtime_defect(
                "api-callback",
                "callback-replacement-plan-frame-missing",
                "callback replacement plan omitted its entered frame",
            )
            .with_endpoint(endpoint)
        })?;
        let image = self.prepare_guest_callback_frame(call, runtime_frame.scratch)?;
        let scratch_allocation = self
            .runtime
            .callback_scratch_allocation()
            .map_err(|error| ApiFault::from_runtime(error, endpoint))?
            .clone();
        preflight_callback_scratch_mapping(
            self.memory,
            &scratch_allocation,
            runtime_frame.scratch,
            endpoint,
        )?;
        let mut transaction = self.memory.begin_transaction();
        materialize_callback_scratch_mapping(
            &mut transaction,
            &scratch_allocation,
            runtime_frame.scratch,
            endpoint,
        )?;
        if !image.scratch.is_empty() {
            transaction
                .write(image.scratch_address, &image.scratch)
                .map_err(|error| ApiFault::from_memory(error, endpoint))?;
        }
        transaction
            .write(image.allocation_start, &image.stack)
            .map_err(|error| ApiFault::from_memory(error, endpoint))?;
        let committed = self
            .runtime
            .commit_callback_mutation(plan)
            .0
            .ok_or_else(|| {
                ApiFault::runtime_defect(
                    "api-callback",
                    "callback-replacement-commit-frame-missing",
                    "validated callback replacement did not commit an entered frame",
                )
                .with_endpoint(endpoint)
            })?;
        debug_assert_eq!(committed, runtime_frame);
        transaction.commit_transaction();
        Ok(image.prepared(runtime_frame.id))
    }

    pub fn commit_guest_callback_semantics(
        &mut self,
        completion_policy: GuestCallbackCompletionPolicy,
        result: u32,
        endpoint: &ApiEndpointIdentity,
    ) -> Result<Option<RuntimeGuestCallbackSemanticAction>, ApiFault> {
        match completion_policy {
            GuestCallbackCompletionPolicy::ReturnU32
            | GuestCallbackCompletionPolicy::ReturnBool32
            | GuestCallbackCompletionPolicy::ReturnConstantU32 { .. } => Ok(None),
            GuestCallbackCompletionPolicy::CreateWindowExA {
                window_id,
                window_handle,
                message,
            } => {
                let action = self
                    .runtime
                    .complete_window_creation_callback(window_id, message, result)
                    .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
                let committed_handle = match &action {
                    RuntimeWindowCreationCallbackAction::Continue { window, .. }
                    | RuntimeWindowCreationCallbackAction::Created { window }
                    | RuntimeWindowCreationCallbackAction::Aborted { window } => window.handle,
                };
                if committed_handle != window_handle {
                    return Err(ApiFault::runtime_defect(
                        "api-callback",
                        "callback-semantic-handle-mismatch",
                        "committed callback semantics changed the pending logical HWND identity",
                    )
                    .with_endpoint(endpoint));
                }
                Ok(Some(RuntimeGuestCallbackSemanticAction::WindowCreation(
                    action,
                )))
            }
            GuestCallbackCompletionPolicy::SetWindowPos { operation, message } => {
                let action = self
                    .runtime
                    .complete_set_window_pos_callback(operation, message, result)
                    .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
                Ok(Some(RuntimeGuestCallbackSemanticAction::SetWindowPos(
                    action,
                )))
            }
            GuestCallbackCompletionPolicy::SetWindowPosDefault { operation, message } => {
                let action = self
                    .runtime
                    .complete_set_window_pos_default_callback(operation, message, result)
                    .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
                Ok(Some(
                    RuntimeGuestCallbackSemanticAction::SetWindowPosDefault(action),
                ))
            }
        }
    }

    fn prepare_guest_callback_frame(
        &self,
        call: &ApiGuestCallbackCall,
        runtime_scratch: Option<RuntimeCallbackScratch>,
    ) -> Result<GuestCallbackFrameImage, ApiFault> {
        const MAX_CALLBACK_ARGUMENTS: usize = 64;
        const MAX_CALLBACK_SCRATCH_POINTER_RELOCATIONS: usize = 64;

        let endpoint = &call.frame.endpoint;
        if call.request.arguments.len() > MAX_CALLBACK_ARGUMENTS
            || call.request.scratch_pointer_relocations.len()
                > MAX_CALLBACK_SCRATCH_POINTER_RELOCATIONS
        {
            return Err(ApiFault::resource_limit(
                "api-callback",
                "callback-frame-limit-exceeded",
                "guest callback arguments, scratch bytes, or relocations exceed the bounded callback frame limit",
            )
            .with_endpoint(endpoint));
        }
        let scratch_bytes_u32 = callback_scratch_byte_length(call)?;
        let scratch_address = match (scratch_bytes_u32, runtime_scratch) {
            (0, None) => GuestAddress(0),
            (0, Some(_)) => {
                return Err(ApiFault::runtime_defect(
                    "api-callback",
                    "callback-empty-scratch-slot-assigned",
                    "callback without scratch bytes received a live runtime scratch slot",
                )
                .with_endpoint(endpoint));
            }
            (byte_length, Some(scratch))
                if scratch.byte_length == byte_length
                    && scratch.capacity == CALLBACK_SCRATCH_SLOT_SIZE =>
            {
                scratch.address
            }
            (_, Some(_)) => {
                return Err(ApiFault::runtime_defect(
                    "api-callback",
                    "callback-scratch-slot-mismatch",
                    "runtime callback scratch slot does not match the requested byte length",
                )
                .with_endpoint(endpoint));
            }
            (_, None) => {
                return Err(ApiFault::runtime_defect(
                    "api-callback",
                    "callback-scratch-slot-missing",
                    "callback scratch bytes have no runtime-owned scratch slot",
                )
                .with_endpoint(endpoint));
            }
        };
        let argument_frame_bytes = call
            .request
            .arguments
            .len()
            .checked_add(1)
            .and_then(|words| words.checked_mul(4))
            .ok_or_else(|| {
                ApiFault::resource_limit(
                    "api-callback",
                    "callback-argument-size-overflow",
                    "guest callback argument frame overflowed host accounting",
                )
                .with_endpoint(endpoint)
            })?;
        let argument_frame_bytes_u32 = u32::try_from(argument_frame_bytes).map_err(|_| {
            ApiFault::resource_limit(
                "api-callback",
                "callback-frame-size-unrepresentable",
                "guest callback frame does not fit x86 address accounting",
            )
            .with_endpoint(endpoint)
        })?;
        let allocation_start = GuestAddress(
            call.frame
                .stack_pointer
                .0
                .checked_sub(argument_frame_bytes_u32)
                .ok_or_else(|| {
                    ApiFault::invalid_call(
                        "api-callback",
                        "callback-stack-pointer-underflow",
                        "guest callback frame crosses the start of the x86 address space",
                    )
                    .with_endpoint(endpoint)
                })?,
        );
        let stack_pointer = allocation_start;

        self.memory
            .validate_access(call.request.target, 1, MemoryAccess::Execute)
            .map_err(|error| ApiFault::from_memory(error, endpoint))?;
        self.memory
            .validate_access(call.request.return_sentinel, 1, MemoryAccess::Execute)
            .map_err(|error| ApiFault::from_memory(error, endpoint))?;
        self.memory
            .validate_access(allocation_start, argument_frame_bytes, MemoryAccess::Write)
            .map_err(|error| ApiFault::from_memory(error, endpoint))?;

        let mut stack = Vec::with_capacity(argument_frame_bytes);
        let mut scratch = call.request.scratch.clone();
        stack.extend_from_slice(&call.request.return_sentinel.0.to_le_bytes());
        for argument in &call.request.arguments {
            let value = match *argument {
                GuestCallbackArgument::U32(value) => value,
                GuestCallbackArgument::ScratchPointer { offset } => {
                    let offset_usize = usize::try_from(offset).map_err(|_| {
                        ApiFault::invalid_call(
                            "api-callback",
                            "callback-scratch-offset-unrepresentable",
                            "guest callback scratch offset does not fit host accounting",
                        )
                        .with_endpoint(endpoint)
                    })?;
                    if offset % 4 != 0 || offset_usize >= call.request.scratch.len() {
                        return Err(ApiFault::invalid_call(
                            "api-callback",
                            "callback-scratch-pointer-invalid",
                            "guest callback scratch pointer must be aligned and identify live scratch bytes",
                        )
                        .with_endpoint(endpoint));
                    }
                    scratch_address.0.checked_add(offset).ok_or_else(|| {
                        ApiFault::invalid_call(
                            "api-callback",
                            "callback-scratch-pointer-overflow",
                            "guest callback scratch pointer overflowed x86 accounting",
                        )
                        .with_endpoint(endpoint)
                    })?
                }
            };
            stack.extend_from_slice(&value.to_le_bytes());
        }
        for relocation in &call.request.scratch_pointer_relocations {
            let write_offset = usize::try_from(relocation.write_offset).map_err(|_| {
                ApiFault::invalid_call(
                    "api-callback",
                    "callback-scratch-relocation-unrepresentable",
                    "guest callback scratch relocation does not fit host accounting",
                )
                .with_endpoint(endpoint)
            })?;
            let target_offset = usize::try_from(relocation.target_offset).map_err(|_| {
                ApiFault::invalid_call(
                    "api-callback",
                    "callback-scratch-relocation-unrepresentable",
                    "guest callback scratch relocation target does not fit host accounting",
                )
                .with_endpoint(endpoint)
            })?;
            if relocation.write_offset % 4 != 0
                || write_offset
                    .checked_add(4)
                    .is_none_or(|end| end > call.request.scratch.len())
                || target_offset >= call.request.scratch.len()
            {
                return Err(ApiFault::invalid_call(
                    "api-callback",
                    "callback-scratch-relocation-invalid",
                    "guest callback scratch relocation must write an aligned pointer to live scratch bytes",
                )
                .with_endpoint(endpoint));
            }
            let target = scratch_address
                .0
                .checked_add(relocation.target_offset)
                .ok_or_else(|| {
                    ApiFault::invalid_call(
                        "api-callback",
                        "callback-scratch-relocation-overflow",
                        "guest callback scratch relocation overflowed x86 accounting",
                    )
                    .with_endpoint(endpoint)
                })?;
            scratch[write_offset..write_offset + 4].copy_from_slice(&target.to_le_bytes());
        }

        Ok(GuestCallbackFrameImage {
            allocation_start,
            stack_pointer,
            scratch_address,
            scratch_bytes: scratch_bytes_u32,
            frame_bytes: argument_frame_bytes_u32,
            stack,
            scratch,
        })
    }

    pub fn register_module_image(
        &mut self,
        handle: u32,
        loader: &LoaderIngressReport,
        materialization: &ImageMaterialization,
    ) -> Result<&RuntimeModuleImage, RuntimeModuleImageError> {
        self.runtime
            .register_module_image(self.memory, handle, loader, materialization)
    }

    pub fn bind_guest_module_imports(
        &mut self,
        handle: u32,
        loader: &LoaderIngressReport,
        materialization: &ImageMaterialization,
        runtime_catalog: &RuntimeExportCatalog,
    ) -> Result<RuntimeGuestImportBinding, RuntimeStateError> {
        crate::dll::bind_guest_module_imports(
            self.memory,
            self.runtime,
            handle,
            loader,
            materialization,
            runtime_catalog,
        )
    }

    pub fn install_controlled_process_environment(
        &mut self,
    ) -> Result<InitialProcessEnvironment, RuntimeStateError> {
        crate::environment::install_controlled_process_environment_for_runtime(
            self.memory,
            self.runtime,
            None,
            false,
            false,
        )
    }

    pub fn install_controlled_process_environment_with_command_line(
        &mut self,
        command_line: &str,
    ) -> Result<InitialProcessEnvironment, RuntimeStateError> {
        crate::environment::install_controlled_process_environment_for_runtime(
            self.memory,
            self.runtime,
            Some(command_line),
            false,
            false,
        )
    }

    pub fn install_controlled_process_environment_with_command_line_and_ansi(
        &mut self,
        command_line: &str,
    ) -> Result<InitialProcessEnvironment, RuntimeStateError> {
        crate::environment::install_controlled_process_environment_for_runtime(
            self.memory,
            self.runtime,
            Some(command_line),
            true,
            false,
        )
    }

    pub fn install_controlled_process_environment_with_command_line_ansi_and_environment_strings(
        &mut self,
        command_line: &str,
    ) -> Result<InitialProcessEnvironment, RuntimeStateError> {
        crate::environment::install_controlled_process_environment_for_runtime(
            self.memory,
            self.runtime,
            Some(command_line),
            true,
            true,
        )
    }

    pub fn allocate_private_heap_block(
        &mut self,
        handle: u32,
        byte_length: u32,
        initialization: RuntimeHeapInitialization,
    ) -> Result<RuntimeHeapAllocation, RuntimeStateError> {
        crate::heap::allocate_private_heap_block(
            self.memory,
            self.runtime,
            handle,
            byte_length,
            initialization,
        )
    }

    pub fn free_private_heap_block(
        &mut self,
        handle: u32,
        address: GuestAddress,
    ) -> Result<RuntimeHeapAllocation, RuntimeStateError> {
        crate::heap::free_private_heap_block(self.memory, self.runtime, handle, address)
    }

    pub fn destroy_private_heap(
        &mut self,
        handle: u32,
    ) -> Result<RuntimeHeapTeardown, RuntimeStateError> {
        crate::heap::destroy_private_heap(self.memory, self.runtime, handle)
    }

    pub fn teardown_process_heaps(&mut self) -> Result<RuntimeHeapTeardown, RuntimeStateError> {
        crate::heap::teardown_process_heaps(self.memory, self.runtime)
    }

    pub fn last_error(&self) -> Result<u32, RuntimeStateError> {
        self.runtime.current_last_error()
    }

    pub fn install_com_object(
        &mut self,
        spec: ComObjectSpec,
    ) -> Result<InstalledComObject, RuntimeStateError> {
        install_com_object(self.memory, self.runtime, spec)
    }

    pub(crate) fn install_com_object_with_guest_writes<F>(
        &mut self,
        spec: ComObjectSpec,
        build_guest_writes: F,
    ) -> Result<(InstalledComObject, Vec<ComGuestWriteReceipt>), RuntimeStateError>
    where
        F: FnOnce(&InstalledComObject) -> Result<Vec<(GuestAddress, Vec<u8>)>, RuntimeStateError>,
    {
        crate::com::install_com_object_with_guest_writes(
            self.memory,
            self.runtime,
            None,
            spec,
            build_guest_writes,
        )
    }

    pub(crate) fn install_audio_device_com_object_with_guest_writes<F>(
        &mut self,
        spec: ComObjectSpec,
        request: RuntimeAudioDeviceCreateRequest,
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
        crate::com::install_audio_device_com_object_with_guest_writes(
            self.memory,
            self.runtime,
            spec,
            request,
            build_guest_writes,
        )
    }

    pub fn dispatch_com_call(&mut self, trap: ComCallTrap) -> ComDispatchOutcome {
        dispatch_com_call(self.memory, self.runtime, trap)
    }

    pub(crate) fn dispatch_com_call_with_frontend<F>(
        &mut self,
        trap: ComCallTrap,
        frontend: F,
    ) -> ComDispatchOutcome
    where
        F: FnOnce(crate::com::ComFrontendCall<'_>) -> ComDispatchOutcome,
    {
        crate::com::dispatch_com_call_with_frontend(self.memory, self.runtime, trap, frontend)
    }
}

#[derive(Debug, Clone)]
pub struct ApiDispatcher {
    endpoints_by_address: BTreeMap<GuestAddress, ApiEndpointIdentity>,
    endpoints_by_id: BTreeMap<(ApiEndpointBinding, ImportThunkId), ApiEndpointIdentity>,
    contracts: BTreeMap<GuestAddress, ApiContract>,
}

impl ApiDispatcher {
    pub fn new(
        endpoints: impl IntoIterator<Item = ApiEndpointIdentity>,
        contracts: impl IntoIterator<Item = ApiContract>,
    ) -> Result<Self, ApiFault> {
        let mut endpoints_by_address = BTreeMap::new();
        let mut endpoints_by_id = BTreeMap::new();
        for endpoint in endpoints {
            let registry_key = endpoint.registry_key();
            if endpoints_by_address.contains_key(&endpoint.address)
                || endpoints_by_id.contains_key(&registry_key)
            {
                return Err(ApiFault::invalid_call(
                    "api-dispatch",
                    "endpoint-identity-duplicate",
                    "API endpoint registry contains a duplicate binding/ID or address",
                ));
            }
            endpoints_by_address.insert(endpoint.address, endpoint.clone());
            endpoints_by_id.insert(registry_key, endpoint);
        }

        let mut registered_contracts = BTreeMap::new();
        for contract in contracts {
            validate_contract(&contract)?;
            let Some(endpoint) = endpoints_by_id.get(&contract.endpoint.registry_key()) else {
                return Err(ApiFault::invalid_call(
                    "api-dispatch",
                    "contract-endpoint-unowned",
                    "API contract names an endpoint outside the owned endpoint registry",
                )
                .with_endpoint(&contract.endpoint));
            };
            if endpoint != &contract.endpoint {
                return Err(ApiFault::invalid_call(
                    "api-dispatch",
                    "contract-endpoint-identity-mismatch",
                    "API contract endpoint metadata does not match the owned endpoint",
                )
                .with_endpoint(endpoint));
            }
            match endpoint.binding {
                ApiEndpointBinding::ImportAddressTable
                    if endpoint.iat_address.is_none()
                        || endpoint.disposition
                            != Some(ImportThunkDisposition::DeclaredRuntimeExport) =>
                {
                    return Err(ApiFault::invalid_call(
                        "api-dispatch",
                        "unsupported-endpoint-contract-forbidden",
                        "an unbound or unsupported import endpoint cannot be registered as an implemented API",
                    )
                    .with_endpoint(endpoint));
                }
                ApiEndpointBinding::DynamicExport
                    if endpoint.iat_address.is_some() || endpoint.disposition.is_some() =>
                {
                    return Err(ApiFault::runtime_defect(
                        "api-dispatch",
                        "dynamic-endpoint-binding-invalid",
                        "a dynamic export endpoint cannot carry import binding metadata",
                    )
                    .with_endpoint(endpoint));
                }
                _ => {}
            }
            if registered_contracts
                .insert(endpoint.address, contract)
                .is_some()
            {
                return Err(ApiFault::invalid_call(
                    "api-dispatch",
                    "endpoint-contract-duplicate",
                    "API endpoint has more than one registered contract",
                )
                .with_endpoint(endpoint));
            }
        }

        Ok(Self {
            endpoints_by_address,
            endpoints_by_id,
            contracts: registered_contracts,
        })
    }

    pub fn dispatch<F>(
        &self,
        trap: ApiTrap,
        session: &mut ExecutionSession<'_>,
        handler: F,
    ) -> DispatchOutcome
    where
        F: FnOnce(&ApiContract, &ApiCallFrame, &mut ExecutionSession<'_>) -> ApiHandlerResult,
    {
        let Some(endpoint) = self.endpoints_by_address.get(&trap.endpoint_address) else {
            return DispatchOutcome::Fault(ApiFault::unsupported(
                "api-dispatch",
                "endpoint-address-unowned",
                format!(
                    "trap address {:#010x} is not an owned API endpoint",
                    trap.endpoint_address.0
                ),
            ));
        };
        if endpoint.id != trap.endpoint_id
            || self.endpoints_by_id.get(&endpoint.registry_key()) != Some(endpoint)
        {
            return DispatchOutcome::Fault(
                ApiFault::invalid_call(
                    "api-dispatch",
                    "endpoint-trap-identity-mismatch",
                    "trap endpoint ID and address do not identify the same owned endpoint",
                )
                .with_endpoint(endpoint),
            );
        }
        let Some(contract) = self.contracts.get(&endpoint.address) else {
            return DispatchOutcome::Fault(missing_endpoint_contract(endpoint));
        };
        if let Err(fault) = validate_bound_iat(session.memory(), endpoint) {
            return DispatchOutcome::Fault(fault);
        }
        let frame = match decode_call_frame(session.memory(), contract, trap) {
            Ok(frame) => frame,
            Err(fault) => return DispatchOutcome::Fault(fault),
        };

        match handler(contract, &frame, session) {
            ApiHandlerResult::Immediate(immediate) => {
                if !contract.behavior.permits_immediate() {
                    return DispatchOutcome::Fault(
                        ApiFault::runtime_defect(
                            "api-dispatch",
                            "immediate-handler-contract-mismatch",
                            "handler returned immediately for a deferred API contract",
                        )
                        .with_endpoint(endpoint),
                    );
                }
                if contract.callback_policy == CallbackPolicy::Required {
                    return DispatchOutcome::Fault(
                        ApiFault::runtime_defect(
                            "api-dispatch",
                            "required-guest-callback-missing",
                            "handler returned without the guest callback required by its API contract",
                        )
                        .with_endpoint(endpoint),
                    );
                }
                match complete_immediate(session, contract, frame, immediate) {
                    Ok(completion) => DispatchOutcome::Immediate(completion),
                    Err(fault) => DispatchOutcome::Fault(fault),
                }
            }
            ApiHandlerResult::Deferred(request) => {
                if !contract.behavior.permits_deferred() {
                    DispatchOutcome::Fault(
                        ApiFault::runtime_defect(
                            "api-dispatch",
                            "deferred-handler-contract-mismatch",
                            "handler deferred an immediate API contract",
                        )
                        .with_endpoint(endpoint),
                    )
                } else {
                    DispatchOutcome::Deferred(ApiDeferredCall {
                        request,
                        contract: contract.clone(),
                        frame,
                    })
                }
            }
            ApiHandlerResult::GuestCallback(callback) => {
                if contract.callback_policy == CallbackPolicy::Forbidden {
                    DispatchOutcome::Fault(
                        ApiFault::runtime_defect(
                            "api-dispatch",
                            "guest-callback-forbidden",
                            "handler requested a guest callback for a callback-free API contract",
                        )
                        .with_endpoint(endpoint),
                    )
                } else if !callback
                    .completion_policy
                    .matches_return_schema(&contract.abi.return_value)
                {
                    DispatchOutcome::Fault(
                        ApiFault::runtime_defect(
                            "api-dispatch",
                            "callback-completion-policy-mismatch",
                            "guest callback completion policy does not match the outer API return schema",
                        )
                        .with_endpoint(endpoint),
                    )
                } else {
                    DispatchOutcome::GuestCallback(ApiGuestCallbackCall {
                        request: callback,
                        contract: contract.clone(),
                        frame,
                    })
                }
            }
            ApiHandlerResult::Fault(fault) => DispatchOutcome::Fault(fault.with_endpoint(endpoint)),
        }
    }

    pub fn dispatch_owned<F>(
        &self,
        trap: ApiOwnedTrap,
        session: &mut ExecutionSession<'_>,
        handler: F,
    ) -> DispatchOutcome
    where
        F: FnOnce(&ApiContract, &ApiCallFrame, &mut ExecutionSession<'_>) -> ApiHandlerResult,
    {
        let Some(endpoint) = self.endpoints_by_address.get(&trap.endpoint_address) else {
            return DispatchOutcome::Fault(ApiFault::unsupported(
                "api-dispatch",
                "endpoint-address-unowned",
                format!(
                    "trap address {:#010x} is not an owned API endpoint",
                    trap.endpoint_address.0
                ),
            ));
        };
        if endpoint.id != trap.endpoint_id
            || self.endpoints_by_id.get(&endpoint.registry_key()) != Some(endpoint)
        {
            return DispatchOutcome::Fault(
                ApiFault::invalid_call(
                    "api-dispatch",
                    "endpoint-trap-identity-mismatch",
                    "trap endpoint ID and address do not identify the same owned endpoint",
                )
                .with_endpoint(endpoint),
            );
        }
        let Some(contract) = self.contracts.get(&endpoint.address) else {
            return DispatchOutcome::Fault(missing_endpoint_contract(endpoint));
        };
        let argument_bytes = match contract.abi.argument_bytes() {
            Ok(argument_bytes) => argument_bytes,
            Err(fault) => return DispatchOutcome::Fault(fault.with_endpoint(endpoint)),
        };
        self.dispatch(
            ApiTrap {
                endpoint_id: trap.endpoint_id,
                endpoint_address: trap.endpoint_address,
                stack_pointer: trap.stack_pointer,
                return_address: trap.return_address,
                argument_bytes,
            },
            session,
            handler,
        )
    }

    pub fn complete_deferred(
        &self,
        call: ApiDeferredCall,
        completion: HostCompletion,
        session: &mut ExecutionSession<'_>,
    ) -> DispatchOutcome {
        if call.request.request_id != completion.request_id {
            return DispatchOutcome::Fault(
                ApiFault::invalid_call(
                    "api-dispatch",
                    "host-completion-request-mismatch",
                    "host completion does not identify the suspended API request",
                )
                .with_endpoint(&call.frame.endpoint),
            );
        }
        if !call.contract.behavior.permits_deferred() {
            return DispatchOutcome::Fault(
                ApiFault::runtime_defect(
                    "api-dispatch",
                    "deferred-completion-contract-mismatch",
                    "suspended host call does not carry a deferred API contract",
                )
                .with_endpoint(&call.frame.endpoint),
            );
        }
        self.complete_suspended(call.contract, call.frame, completion.result, session)
    }

    pub fn complete_guest_callback(
        &self,
        call: ApiGuestCallbackCall,
        result: ApiImmediate,
        session: &mut ExecutionSession<'_>,
    ) -> DispatchOutcome {
        if call.contract.callback_policy == CallbackPolicy::Forbidden {
            return DispatchOutcome::Fault(
                ApiFault::runtime_defect(
                    "api-dispatch",
                    "callback-completion-contract-mismatch",
                    "suspended callback call forbids guest callbacks",
                )
                .with_endpoint(&call.frame.endpoint),
            );
        }
        self.complete_suspended(call.contract, call.frame, result, session)
    }

    pub fn complete_guest_callback_result(
        &self,
        call: ApiGuestCallbackCall,
        result: u32,
        session: &mut ExecutionSession<'_>,
    ) -> DispatchOutcome {
        if let Some(completion) = call.request.completion_policy.complete_direct(result) {
            return self.complete_guest_callback(call, completion, session);
        }
        let handler_result = match call.request.completion_policy {
            GuestCallbackCompletionPolicy::CreateWindowExA { .. } => {
                crate::user32::resolve_create_window_ex_a_callback(&call, result, session)
            }
            GuestCallbackCompletionPolicy::SetWindowPos { .. } => {
                crate::user32::resolve_set_window_pos_callback(&call, result, session)
            }
            GuestCallbackCompletionPolicy::SetWindowPosDefault { .. } => {
                crate::user32::resolve_set_window_pos_default_callback(&call, result, session)
            }
            GuestCallbackCompletionPolicy::ReturnU32
            | GuestCallbackCompletionPolicy::ReturnBool32
            | GuestCallbackCompletionPolicy::ReturnConstantU32 { .. } => {
                ApiHandlerResult::Fault(ApiFault::runtime_defect(
                    "api-dispatch",
                    "callback-direct-completion-missing",
                    "direct callback completion policy did not produce an API result",
                ))
            }
        };
        let endpoint = &call.frame.endpoint;
        match handler_result {
            ApiHandlerResult::Immediate(immediate) => {
                self.complete_guest_callback(call, immediate, session)
            }
            ApiHandlerResult::GuestCallback(callback) => {
                if call.contract.callback_policy == CallbackPolicy::Forbidden {
                    DispatchOutcome::Fault(
                        ApiFault::runtime_defect(
                            "api-dispatch",
                            "guest-callback-forbidden",
                            "callback continuation requested a guest callback for a callback-free API contract",
                        )
                        .with_endpoint(endpoint),
                    )
                } else if !callback
                    .completion_policy
                    .matches_return_schema(&call.contract.abi.return_value)
                {
                    DispatchOutcome::Fault(
                        ApiFault::runtime_defect(
                            "api-dispatch",
                            "callback-completion-policy-mismatch",
                            "guest callback continuation policy does not match the outer API return schema",
                        )
                        .with_endpoint(endpoint),
                    )
                } else {
                    DispatchOutcome::GuestCallback(ApiGuestCallbackCall {
                        request: callback,
                        contract: call.contract,
                        frame: call.frame,
                    })
                }
            }
            ApiHandlerResult::Deferred(_) => DispatchOutcome::Fault(
                ApiFault::runtime_defect(
                    "api-dispatch",
                    "callback-continuation-deferred",
                    "synchronous guest callback continuation attempted to defer the outer API",
                )
                .with_endpoint(endpoint),
            ),
            ApiHandlerResult::Fault(fault) => DispatchOutcome::Fault(fault.with_endpoint(endpoint)),
        }
    }

    fn complete_suspended(
        &self,
        contract: ApiContract,
        frame: ApiCallFrame,
        result: ApiImmediate,
        session: &mut ExecutionSession<'_>,
    ) -> DispatchOutcome {
        let endpoint = &frame.endpoint;
        if self.endpoints_by_id.get(&endpoint.registry_key()) != Some(endpoint)
            || self.endpoints_by_address.get(&endpoint.address) != Some(endpoint)
            || self.contracts.get(&endpoint.address) != Some(&contract)
        {
            return DispatchOutcome::Fault(
                ApiFault::invalid_call(
                    "api-dispatch",
                    "suspended-call-identity-mismatch",
                    "suspended API metadata no longer matches the dispatcher registry",
                )
                .with_endpoint(endpoint),
            );
        }
        if let Err(fault) = validate_bound_iat(session.memory(), endpoint) {
            return DispatchOutcome::Fault(fault);
        }
        match complete_immediate(session, &contract, frame, result) {
            Ok(completion) => DispatchOutcome::Immediate(completion),
            Err(fault) => DispatchOutcome::Fault(fault),
        }
    }
}

fn missing_endpoint_contract(endpoint: &ApiEndpointIdentity) -> ApiFault {
    let (code, message) = match endpoint.binding {
        ApiEndpointBinding::ImportAddressTable => (
            "endpoint-contract-missing",
            format!(
                "owned endpoint {} has no API contract",
                endpoint_label(endpoint)
            ),
        ),
        ApiEndpointBinding::DynamicExport => (
            "dynamic-export-contract-missing",
            format!(
                "process-owned dynamic export endpoint {} has no API contract",
                endpoint_label(endpoint)
            ),
        ),
    };
    ApiFault::unsupported("api-dispatch", code, message).with_endpoint(endpoint)
}

fn validate_contract(contract: &ApiContract) -> Result<(), ApiFault> {
    if contract.endpoint.module.is_empty() || contract.implementation_id.is_empty() {
        return Err(ApiFault::invalid_call(
            "api-contract",
            "contract-identity-empty",
            "API contract requires nonempty module and implementation identities",
        )
        .with_endpoint(&contract.endpoint));
    }
    match (&contract.abi.calling_convention, contract.abi.stack_cleanup) {
        (CallingConvention::Stdcall, StackCleanup::Callee)
        | (CallingConvention::Cdecl, StackCleanup::Caller) => {}
        _ => {
            return Err(ApiFault::unsupported(
                "api-contract",
                "calling-convention-cleanup-combination-unsupported",
                "the generic dispatcher currently supports cdecl/caller and stdcall/callee cleanup",
            )
            .with_endpoint(&contract.endpoint));
        }
    }
    contract.abi.argument_bytes()?;
    if contract.behavior.permits_committed_indirect_guest_writes()
        && contract.callback_policy != CallbackPolicy::Forbidden
    {
        return Err(ApiFault::invalid_call(
            "api-contract",
            "indirect-guest-write-callback-policy-invalid",
            "committed indirect guest writes require a callback-free immediate contract",
        )
        .with_endpoint(&contract.endpoint));
    }
    for argument in &contract.abi.arguments {
        if argument.name.is_empty() {
            return Err(ApiFault::invalid_call(
                "api-contract",
                "argument-name-empty",
                "API argument schema requires a nonempty name",
            )
            .with_endpoint(&contract.endpoint));
        }
        match &argument.value {
            ApiValueSchema::Pointer { constraint } => {
                if constraint.alignment == 0
                    || !constraint.alignment.is_power_of_two()
                    || constraint.byte_length == 0
                {
                    return Err(ApiFault::invalid_call(
                        "api-contract",
                        "pointer-constraint-invalid",
                        "pointer alignment must be a nonzero power of two and byte length must be nonzero",
                    )
                    .with_endpoint(&contract.endpoint));
                }
                let writes_allowed =
                    matches!(argument.direction, ApiDirection::Out | ApiDirection::InOut);
                if writes_allowed == (constraint.write_requirement == ApiWriteRequirement::None) {
                    return Err(ApiFault::invalid_call(
                        "api-contract",
                        "pointer-write-policy-invalid",
                        "pointer direction and guest-write requirement disagree",
                    )
                    .with_endpoint(&contract.endpoint));
                }
            }
            ApiValueSchema::DynamicPointer { constraint } => {
                if constraint.alignment == 0
                    || !constraint.alignment.is_power_of_two()
                    || constraint.minimum_byte_length == 0
                    || constraint.maximum_byte_length < constraint.minimum_byte_length
                {
                    return Err(ApiFault::invalid_call(
                        "api-contract",
                        "dynamic-pointer-constraint-invalid",
                        "dynamic pointer alignment and bounded byte lengths must be valid",
                    )
                    .with_endpoint(&contract.endpoint));
                }
                let writes_allowed =
                    matches!(argument.direction, ApiDirection::Out | ApiDirection::InOut);
                if writes_allowed == (constraint.write_requirement == ApiWriteRequirement::None) {
                    return Err(ApiFault::invalid_call(
                        "api-contract",
                        "dynamic-pointer-write-policy-invalid",
                        "dynamic pointer direction and guest-write requirement disagree",
                    )
                    .with_endpoint(&contract.endpoint));
                }
            }
            ApiValueSchema::ScalarOrPointer { constraint } => {
                if constraint.alignment == 0
                    || !constraint.alignment.is_power_of_two()
                    || constraint.minimum_byte_length == 0
                    || constraint.maximum_byte_length < constraint.minimum_byte_length
                    || constraint.write_requirement == ApiWriteRequirement::Required
                {
                    return Err(ApiFault::invalid_call(
                        "api-contract",
                        "scalar-or-pointer-constraint-invalid",
                        "scalar-or-pointer alignment and bounded byte lengths must be valid, and conditional writes cannot be required",
                    )
                    .with_endpoint(&contract.endpoint));
                }
                let writes_allowed =
                    matches!(argument.direction, ApiDirection::Out | ApiDirection::InOut);
                if writes_allowed == (constraint.write_requirement == ApiWriteRequirement::None) {
                    return Err(ApiFault::invalid_call(
                        "api-contract",
                        "scalar-or-pointer-write-policy-invalid",
                        "scalar-or-pointer direction and guest-write requirement disagree",
                    )
                    .with_endpoint(&contract.endpoint));
                }
            }
            ApiValueSchema::OpaquePointer { alignment, .. } => {
                if *alignment == 0 || !alignment.is_power_of_two() {
                    return Err(ApiFault::invalid_call(
                        "api-contract",
                        "opaque-pointer-alignment-invalid",
                        "opaque pointer alignment must be a nonzero power of two",
                    )
                    .with_endpoint(&contract.endpoint));
                }
                if argument.direction != ApiDirection::In {
                    return Err(ApiFault::invalid_call(
                        "api-contract",
                        "opaque-pointer-direction-invalid",
                        "opaque pointers are input values and cannot own dispatcher guest writes",
                    )
                    .with_endpoint(&contract.endpoint));
                }
            }
            ApiValueSchema::NameOrOrdinal {
                maximum_code_units, ..
            } => {
                if *maximum_code_units == 0 || argument.direction != ApiDirection::In {
                    return Err(ApiFault::invalid_call(
                        "api-contract",
                        "name-or-ordinal-schema-invalid",
                        "name-or-ordinal arguments require input direction and a nonzero bound",
                    )
                    .with_endpoint(&contract.endpoint));
                }
            }
            ApiValueSchema::U32 | ApiValueSchema::Bool32 | ApiValueSchema::LogicalHandle { .. } => {
                if argument.direction != ApiDirection::In {
                    return Err(ApiFault::invalid_call(
                        "api-contract",
                        "scalar-direction-invalid",
                        "scalar API arguments must use input direction",
                    )
                    .with_endpoint(&contract.endpoint));
                }
            }
        }
    }
    Ok(())
}

fn validate_bound_iat(
    memory: &GuestMemory,
    endpoint: &ApiEndpointIdentity,
) -> Result<(), ApiFault> {
    if endpoint.binding == ApiEndpointBinding::DynamicExport {
        return Ok(());
    }
    let iat_address = endpoint.iat_address.ok_or_else(|| {
        ApiFault::runtime_defect(
            "api-dispatch",
            "import-endpoint-iat-missing",
            "an import endpoint has no IAT binding address",
        )
        .with_endpoint(endpoint)
    })?;
    let bytes = memory
        .read(iat_address, 4)
        .map_err(|error| ApiFault::from_memory(error, endpoint))?;
    let value = u32::from_le_bytes(bytes.try_into().map_err(|_| {
        ApiFault::runtime_defect(
            "api-dispatch",
            "iat-entry-width-invalid",
            "checked IAT entry did not contain one x86 pointer",
        )
        .with_endpoint(endpoint)
    })?);
    if value != endpoint.address.0 {
        return Err(ApiFault::invalid_call(
            "api-dispatch",
            "iat-endpoint-mismatch",
            "IAT entry no longer points at the owned API endpoint",
        )
        .with_endpoint(endpoint));
    }
    Ok(())
}

fn decode_call_frame(
    memory: &GuestMemory,
    contract: &ApiContract,
    trap: ApiTrap,
) -> Result<ApiCallFrame, ApiFault> {
    let expected_argument_bytes = contract.abi.argument_bytes()?;
    if trap.argument_bytes != expected_argument_bytes {
        return Err(ApiFault::invalid_call(
            "abi-gateway",
            "argument-byte-count-mismatch",
            format!(
                "trap reports {} argument bytes; API contract requires {expected_argument_bytes}",
                trap.argument_bytes
            ),
        )
        .with_endpoint(&contract.endpoint));
    }
    let frame_bytes = expected_argument_bytes.checked_add(4).ok_or_else(|| {
        ApiFault::resource_limit(
            "abi-gateway",
            "call-frame-byte-count-overflow",
            "x86 call frame byte count overflowed",
        )
        .with_endpoint(&contract.endpoint)
    })?;
    let bytes = memory
        .read(trap.stack_pointer, frame_bytes as usize)
        .map_err(|error| ApiFault::from_memory(error, &contract.endpoint))?;
    let words = bytes
        .chunks_exact(4)
        .map(|word| u32::from_le_bytes(word.try_into().expect("chunks_exact yields x86 words")))
        .collect::<Vec<_>>();
    if words.first().copied() != Some(trap.return_address.0) {
        return Err(ApiFault::invalid_call(
            "abi-gateway",
            "return-address-mismatch",
            "guest stack return address does not match the trapped CPU state",
        )
        .with_endpoint(&contract.endpoint));
    }
    let mut cursor = 1_usize;
    let mut arguments = Vec::with_capacity(contract.abi.arguments.len());
    for schema in &contract.abi.arguments {
        let word_count = schema.value.stack_words() as usize;
        let end = cursor.checked_add(word_count).ok_or_else(|| {
            ApiFault::resource_limit(
                "abi-gateway",
                "argument-decode-overflow",
                "argument decoding overflowed host indexing",
            )
            .with_endpoint(&contract.endpoint)
        })?;
        let argument_words = words.get(cursor..end).ok_or_else(|| {
            ApiFault::runtime_defect(
                "abi-gateway",
                "argument-frame-truncated",
                "checked frame did not contain the contract argument words",
            )
            .with_endpoint(&contract.endpoint)
        })?;
        validate_argument(memory, &contract.endpoint, schema, argument_words[0])?;
        arguments.push(DecodedApiArgument {
            schema: schema.clone(),
            words: argument_words.to_vec(),
        });
        cursor = end;
    }
    Ok(ApiCallFrame {
        endpoint: contract.endpoint.clone(),
        stack_pointer: trap.stack_pointer,
        return_address: trap.return_address,
        argument_bytes: expected_argument_bytes,
        arguments,
    })
}

fn validate_argument(
    memory: &GuestMemory,
    endpoint: &ApiEndpointIdentity,
    schema: &ApiArgumentSchema,
    value: u32,
) -> Result<(), ApiFault> {
    match &schema.value {
        ApiValueSchema::OpaquePointer {
            nullable,
            alignment,
        } => validate_pointer_value(endpoint, schema, value, *nullable, *alignment),
        ApiValueSchema::Pointer { constraint } => {
            validate_pointer_value(
                endpoint,
                schema,
                value,
                constraint.nullable,
                constraint.alignment,
            )?;
            if value == 0 {
                return Ok(());
            }
            let address = GuestAddress(value);
            let byte_length = constraint.byte_length as usize;
            match schema.direction {
                ApiDirection::In => memory
                    .validate_access(address, byte_length, MemoryAccess::Read)
                    .map_err(|error| ApiFault::from_memory(error, endpoint)),
                ApiDirection::Out => memory
                    .validate_access(address, byte_length, MemoryAccess::Write)
                    .map_err(|error| ApiFault::from_memory(error, endpoint)),
                ApiDirection::InOut => {
                    memory
                        .validate_access(address, byte_length, MemoryAccess::Read)
                        .map_err(|error| ApiFault::from_memory(error, endpoint))?;
                    memory
                        .validate_access(address, byte_length, MemoryAccess::Write)
                        .map_err(|error| ApiFault::from_memory(error, endpoint))
                }
            }
        }
        ApiValueSchema::DynamicPointer { constraint } => {
            validate_pointer_value(
                endpoint,
                schema,
                value,
                constraint.nullable,
                constraint.alignment,
            )?;
            if value == 0 {
                return Ok(());
            }
            let address = GuestAddress(value);
            let byte_length = constraint.minimum_byte_length as usize;
            match schema.direction {
                ApiDirection::In => memory
                    .validate_access(address, byte_length, MemoryAccess::Read)
                    .map_err(|error| ApiFault::from_memory(error, endpoint)),
                ApiDirection::Out => memory
                    .validate_access(address, byte_length, MemoryAccess::Write)
                    .map_err(|error| ApiFault::from_memory(error, endpoint)),
                ApiDirection::InOut => {
                    memory
                        .validate_access(address, byte_length, MemoryAccess::Read)
                        .map_err(|error| ApiFault::from_memory(error, endpoint))?;
                    memory
                        .validate_access(address, byte_length, MemoryAccess::Write)
                        .map_err(|error| ApiFault::from_memory(error, endpoint))
                }
            }
        }
        ApiValueSchema::ScalarOrPointer { .. } => Ok(()),
        ApiValueSchema::NameOrOrdinal { encoding, .. } => {
            if value <= u32::from(u16::MAX) {
                return Ok(());
            }
            let minimum_byte_length = match encoding {
                ApiStringEncoding::Utf16Le => 2,
                ApiStringEncoding::Windows1252 | ApiStringEncoding::Windows932 => 1,
            };
            memory
                .validate_access(GuestAddress(value), minimum_byte_length, MemoryAccess::Read)
                .map_err(|error| ApiFault::from_memory(error, endpoint))
        }
        ApiValueSchema::LogicalHandle { nullable, .. } if value == 0 && !nullable => {
            Err(ApiFault::invalid_call(
                "abi-gateway",
                "null-logical-handle-forbidden",
                format!("argument {} cannot be a null logical handle", schema.name),
            )
            .with_endpoint(endpoint))
        }
        ApiValueSchema::U32 | ApiValueSchema::Bool32 | ApiValueSchema::LogicalHandle { .. } => {
            Ok(())
        }
    }
}

fn validate_pointer_value(
    endpoint: &ApiEndpointIdentity,
    schema: &ApiArgumentSchema,
    value: u32,
    nullable: bool,
    alignment: u32,
) -> Result<(), ApiFault> {
    if value == 0 && !nullable {
        return Err(ApiFault::invalid_call(
            "abi-gateway",
            "null-pointer-forbidden",
            format!("argument {} cannot be NULL", schema.name),
        )
        .with_endpoint(endpoint));
    }
    if value != 0 && !value.is_multiple_of(alignment) {
        return Err(ApiFault::invalid_call(
            "abi-gateway",
            "pointer-alignment-invalid",
            format!(
                "argument {} at {value:#010x} is not aligned to {alignment} bytes",
                schema.name
            ),
        )
        .with_endpoint(endpoint));
    }
    Ok(())
}

fn complete_immediate(
    session: &mut ExecutionSession<'_>,
    contract: &ApiContract,
    frame: ApiCallFrame,
    immediate: ApiImmediate,
) -> Result<ApiImmediateCompletion, ApiFault> {
    if session.memory.transaction_is_active() {
        return complete_immediate_in_memory(
            session.memory,
            session.runtime,
            contract,
            frame,
            immediate,
        );
    }

    let mut transaction = session.memory.begin_transaction();
    let completion = complete_immediate_in_memory(
        &mut transaction,
        session.runtime,
        contract,
        frame,
        immediate,
    );
    if completion.is_ok() {
        transaction.commit_transaction();
    }
    completion
}

fn complete_immediate_in_memory(
    memory: &mut GuestMemory,
    runtime: &mut RuntimeState,
    contract: &ApiContract,
    frame: ApiCallFrame,
    immediate: ApiImmediate,
) -> Result<ApiImmediateCompletion, ApiFault> {
    let eax = validate_return_value(contract, &immediate.return_value)?;
    let next_last_error = match contract.last_error_policy {
        LastErrorPolicy::Preserve => {
            if immediate.last_error.is_some() {
                return Err(ApiFault::runtime_defect(
                    "api-dispatch",
                    "preserved-last-error-overwritten",
                    "handler supplied LastError for a preserve policy",
                )
                .with_endpoint(&contract.endpoint));
            }
            runtime
                .current_last_error()
                .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?
        }
        LastErrorPolicy::FromHandler => immediate.last_error.ok_or_else(|| {
            ApiFault::runtime_defect(
                "api-dispatch",
                "handler-last-error-missing",
                "handler did not supply LastError for a handler-owned policy",
            )
            .with_endpoint(&contract.endpoint)
        })?,
    };
    validate_guest_writes(
        contract,
        &frame,
        &immediate.guest_writes,
        &immediate.committed_guest_writes,
    )?;

    let mut guest_writes = immediate.committed_guest_writes;
    guest_writes.reserve(immediate.guest_writes.len());
    for write in immediate.guest_writes {
        let result = memory
            .write(write.address, &write.bytes)
            .map_err(|error| ApiFault::from_memory(error, &contract.endpoint))?;
        guest_writes.push(ApiGuestWriteReceipt { write, result });
    }

    let cleanup_bytes = match contract.abi.stack_cleanup {
        StackCleanup::Caller => 4,
        StackCleanup::Callee => frame.argument_bytes.checked_add(4).ok_or_else(|| {
            ApiFault::resource_limit(
                "abi-gateway",
                "return-stack-cleanup-overflow",
                "callee stack cleanup overflowed x86 accounting",
            )
            .with_endpoint(&contract.endpoint)
        })?,
    };
    let stack_pointer = GuestAddress(
        frame
            .stack_pointer
            .0
            .checked_add(cleanup_bytes)
            .ok_or_else(|| {
                ApiFault::invalid_call(
                    "abi-gateway",
                    "return-stack-pointer-overflow",
                    "API return stack pointer crosses the x86 address limit",
                )
                .with_endpoint(&contract.endpoint)
            })?,
    );
    runtime
        .set_current_last_error(next_last_error)
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    Ok(ApiImmediateCompletion {
        implementation_id: contract.implementation_id.clone(),
        instruction_pointer: frame.return_address,
        stack_pointer,
        frame,
        eax,
        edx: RegisterUpdate::Preserve,
        eflags: RegisterUpdate::Preserve,
        last_error: next_last_error,
        guest_writes,
    })
}

fn validate_return_value(
    contract: &ApiContract,
    value: &ApiReturnValue,
) -> Result<RegisterUpdate, ApiFault> {
    let update = match (&contract.abi.return_value, value) {
        (ApiReturnSchema::Void, ApiReturnValue::Void) => RegisterUpdate::Preserve,
        (ApiReturnSchema::U16, ApiReturnValue::U16(value)) => {
            RegisterUpdate::Set(u32::from(*value))
        }
        (ApiReturnSchema::U32, ApiReturnValue::U32(value)) => RegisterUpdate::Set(*value),
        (ApiReturnSchema::HResult, ApiReturnValue::HResult(value)) => {
            RegisterUpdate::Set(value.bits())
        }
        (ApiReturnSchema::Bool32, ApiReturnValue::Bool32(value)) => {
            RegisterUpdate::Set(u32::from(*value))
        }
        (ApiReturnSchema::OpaquePointer { nullable }, ApiReturnValue::OpaquePointer(value))
            if *nullable || *value != 0 =>
        {
            RegisterUpdate::Set(*value)
        }
        (
            ApiReturnSchema::LogicalPointer {
                namespace,
                nullable,
            },
            ApiReturnValue::LogicalPointer {
                namespace: result_namespace,
                value,
            },
        ) if namespace == result_namespace && (*nullable || *value != 0) => {
            RegisterUpdate::Set(*value)
        }
        (
            ApiReturnSchema::LogicalHandle {
                namespace,
                nullable,
            },
            ApiReturnValue::LogicalHandle {
                namespace: result_namespace,
                value,
            },
        ) if namespace == result_namespace && (*nullable || *value != 0) => {
            RegisterUpdate::Set(*value)
        }
        _ => {
            return Err(ApiFault::runtime_defect(
                "api-dispatch",
                "handler-return-schema-mismatch",
                "handler return value does not match the API contract",
            )
            .with_endpoint(&contract.endpoint));
        }
    };
    Ok(update)
}

fn validate_guest_writes(
    contract: &ApiContract,
    frame: &ApiCallFrame,
    pending_writes: &[ApiGuestWrite],
    committed_writes: &[ApiGuestWriteReceipt],
) -> Result<(), ApiFault> {
    let mut written_arguments = BTreeSet::new();
    for (write, committed) in pending_writes.iter().map(|write| (write, false)).chain(
        committed_writes
            .iter()
            .map(|receipt| (&receipt.write, true)),
    ) {
        let matching =
            frame
                .arguments
                .iter()
                .enumerate()
                .find(|(_, argument)| match &argument.schema.value {
                    ApiValueSchema::Pointer { constraint }
                        if matches!(
                            argument.schema.direction,
                            ApiDirection::Out | ApiDirection::InOut
                        ) && constraint.write_requirement != ApiWriteRequirement::None =>
                    {
                        argument.as_u32() == Some(write.address.0)
                            && constraint.byte_length as usize == write.bytes.len()
                    }
                    ApiValueSchema::DynamicPointer { constraint }
                        if matches!(
                            argument.schema.direction,
                            ApiDirection::Out | ApiDirection::InOut
                        ) && constraint.write_requirement != ApiWriteRequirement::None =>
                    {
                        argument.as_u32() == Some(write.address.0)
                            && write.bytes.len() >= constraint.minimum_byte_length as usize
                            && write.bytes.len() <= constraint.maximum_byte_length as usize
                    }
                    ApiValueSchema::ScalarOrPointer { constraint }
                        if matches!(
                            argument.schema.direction,
                            ApiDirection::Out | ApiDirection::InOut
                        ) && constraint.write_requirement != ApiWriteRequirement::None =>
                    {
                        argument.as_u32() == Some(write.address.0)
                            && write.address.0 & (constraint.alignment - 1) == 0
                            && write.bytes.len() >= constraint.minimum_byte_length as usize
                            && write.bytes.len() <= constraint.maximum_byte_length as usize
                    }
                    _ => false,
                });
        let Some((argument_index, _)) = matching else {
            if committed && contract.behavior.permits_committed_indirect_guest_writes() {
                continue;
            }
            return Err(ApiFault::runtime_defect(
                "api-dispatch",
                "handler-guest-write-outside-contract",
                if committed {
                    "handler committed an indirect guest write without an explicit API behavior contract"
                } else {
                    "handler guest write does not exactly match an out/inout pointer contract"
                },
            )
            .with_endpoint(&contract.endpoint));
        };
        if !written_arguments.insert(argument_index) {
            return Err(ApiFault::runtime_defect(
                "api-dispatch",
                "handler-guest-write-duplicate",
                "handler wrote the same output argument more than once",
            )
            .with_endpoint(&contract.endpoint));
        }
    }

    for (index, argument) in frame.arguments.iter().enumerate() {
        let required = match &argument.schema.value {
            ApiValueSchema::Pointer { constraint } => {
                constraint.write_requirement == ApiWriteRequirement::Required
            }
            ApiValueSchema::DynamicPointer { constraint } => {
                constraint.write_requirement == ApiWriteRequirement::Required
            }
            _ => false,
        };
        if required && argument.as_u32() != Some(0) && !written_arguments.contains(&index) {
            return Err(ApiFault::runtime_defect(
                "api-dispatch",
                "required-guest-write-missing",
                format!(
                    "handler did not write required output argument {}",
                    argument.schema.name
                ),
            )
            .with_endpoint(&contract.endpoint));
        }
    }
    Ok(())
}

fn endpoint_label(endpoint: &ApiEndpointIdentity) -> String {
    let symbol = match &endpoint.symbol {
        ApiSymbol::Name { name } => name.clone(),
        ApiSymbol::Ordinal { ordinal } => format!("#{ordinal}"),
    };
    format!("{}!{symbol}", endpoint.module)
}

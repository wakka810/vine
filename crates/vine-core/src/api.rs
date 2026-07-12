use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use crate::imports::{ImportThunk, ImportThunkDisposition, ImportThunkId};
use crate::memory::{
    GuestAddress, GuestMemory, GuestMemoryError, GuestMemoryErrorClass, GuestWriteResult,
    MemoryAccess,
};
use crate::pe32::ImportTarget;
use crate::runtime::{RuntimeState, RuntimeStateError, RuntimeStateErrorClass};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ApiSymbol {
    Name { name: String },
    Ordinal { ordinal: u16 },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiEndpointIdentity {
    pub id: ImportThunkId,
    pub address: GuestAddress,
    pub module: String,
    pub symbol: ApiSymbol,
    pub iat_address: GuestAddress,
    pub disposition: ImportThunkDisposition,
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
            iat_address: thunk.iat_address,
            disposition: thunk.disposition,
        }
    }

    pub fn matches(&self, module: &str, symbol: &ApiSymbol) -> bool {
        self.module.eq_ignore_ascii_case(module) && &self.symbol == symbol
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
    U32,
    Bool32,
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
    Deferred,
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
    U32(u32),
    Bool32(bool),
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error: Option<u32>,
}

impl ApiImmediate {
    pub fn new(return_value: ApiReturnValue) -> Self {
        Self {
            return_value,
            guest_writes: Vec::new(),
            last_error: None,
        }
    }

    pub fn with_guest_write(mut self, address: GuestAddress, bytes: Vec<u8>) -> Self {
        self.guest_writes.push(ApiGuestWrite { address, bytes });
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
    pub arguments: Vec<u32>,
    pub return_sentinel: GuestAddress,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DispatchOutcome {
    Immediate(ApiImmediateCompletion),
    Deferred(HostRequest),
    GuestCallback(GuestCallbackRequest),
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

    pub fn last_error(&self) -> Result<u32, RuntimeStateError> {
        self.runtime.current_last_error()
    }
}

#[derive(Debug, Clone)]
pub struct ApiDispatcher {
    endpoints_by_address: BTreeMap<GuestAddress, ApiEndpointIdentity>,
    endpoints_by_id: BTreeMap<ImportThunkId, ApiEndpointIdentity>,
    contracts: BTreeMap<ImportThunkId, ApiContract>,
}

impl ApiDispatcher {
    pub fn new(
        endpoints: impl IntoIterator<Item = ApiEndpointIdentity>,
        contracts: impl IntoIterator<Item = ApiContract>,
    ) -> Result<Self, ApiFault> {
        let mut endpoints_by_address = BTreeMap::new();
        let mut endpoints_by_id = BTreeMap::new();
        for endpoint in endpoints {
            if endpoints_by_address
                .insert(endpoint.address, endpoint.clone())
                .is_some()
                || endpoints_by_id
                    .insert(endpoint.id, endpoint.clone())
                    .is_some()
            {
                return Err(ApiFault::invalid_call(
                    "api-dispatch",
                    "endpoint-identity-duplicate",
                    "API endpoint registry contains a duplicate ID or address",
                ));
            }
        }

        let mut registered_contracts = BTreeMap::new();
        for contract in contracts {
            validate_contract(&contract)?;
            let Some(endpoint) = endpoints_by_id.get(&contract.endpoint.id) else {
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
            if endpoint.disposition != ImportThunkDisposition::DeclaredRuntimeExport {
                return Err(ApiFault::invalid_call(
                    "api-dispatch",
                    "unsupported-endpoint-contract-forbidden",
                    "an unsupported trap endpoint cannot be registered as an implemented API",
                )
                .with_endpoint(endpoint));
            }
            if registered_contracts.insert(endpoint.id, contract).is_some() {
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
            || self.endpoints_by_id.get(&trap.endpoint_id) != Some(endpoint)
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
        let Some(contract) = self.contracts.get(&endpoint.id) else {
            return DispatchOutcome::Fault(
                ApiFault::unsupported(
                    "api-dispatch",
                    "endpoint-contract-missing",
                    format!(
                        "owned endpoint {} has no API contract",
                        endpoint_label(endpoint)
                    ),
                )
                .with_endpoint(endpoint),
            );
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
                if contract.behavior != ApiBehavior::Immediate {
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
                if contract.behavior != ApiBehavior::Deferred {
                    DispatchOutcome::Fault(
                        ApiFault::runtime_defect(
                            "api-dispatch",
                            "deferred-handler-contract-mismatch",
                            "handler deferred an immediate API contract",
                        )
                        .with_endpoint(endpoint),
                    )
                } else {
                    DispatchOutcome::Deferred(request)
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
                } else {
                    DispatchOutcome::GuestCallback(callback)
                }
            }
            ApiHandlerResult::Fault(fault) => DispatchOutcome::Fault(fault.with_endpoint(endpoint)),
        }
    }
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
    let bytes = memory
        .read(endpoint.iat_address, 4)
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
            session
                .last_error()
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
    validate_guest_writes(contract, &frame, &immediate.guest_writes)?;

    let mut guest_writes = Vec::with_capacity(immediate.guest_writes.len());
    for write in immediate.guest_writes {
        let result = session
            .memory
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
    session
        .runtime
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
        (ApiReturnSchema::U32, ApiReturnValue::U32(value)) => RegisterUpdate::Set(*value),
        (ApiReturnSchema::Bool32, ApiReturnValue::Bool32(value)) => {
            RegisterUpdate::Set(u32::from(*value))
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
    writes: &[ApiGuestWrite],
) -> Result<(), ApiFault> {
    let mut written_arguments = BTreeSet::new();
    for write in writes {
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
                    _ => false,
                });
        let Some((argument_index, _)) = matching else {
            return Err(ApiFault::runtime_defect(
                "api-dispatch",
                "handler-guest-write-outside-contract",
                "handler guest write does not exactly match an out/inout pointer contract",
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
        if let ApiValueSchema::Pointer { constraint } = &argument.schema.value
            && constraint.write_requirement == ApiWriteRequirement::Required
            && argument.as_u32() != Some(0)
            && !written_arguments.contains(&index)
        {
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

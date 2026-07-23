use serde::Serialize;

use crate::api::{
    AbiSchema, ApiArgumentSchema, ApiBehavior, ApiCallFrame, ApiContract, ApiDirection,
    ApiEndpointIdentity, ApiFault, ApiHandlerResult, ApiImmediate, ApiPointeeSchema,
    ApiPointerConstraint, ApiReturnSchema, ApiReturnValue, ApiStringEncoding, ApiSymbol,
    ApiValueSchema, ApiWriteRequirement, CallbackPolicy, CallingConvention, ExecutionSession,
    GuestCallbackArgument, GuestCallbackCompletionPolicy, GuestCallbackRequest, LastErrorPolicy,
    ReentrancyPolicy, StackCleanup,
};
use crate::memory::{GuestAddress, MemoryAccess};
use crate::runtime::{
    LOGICAL_DEVICE_CONTEXT_HANDLE_NAMESPACE, LOGICAL_GDI_OBJECT_HANDLE_NAMESPACE,
    RUNTIME_GDI_DEFAULT_BITMAP_INDEX, RUNTIME_GDI_SYSTEM_FONT_INDEX,
    RUNTIME_LOG_FONT_A_BYTE_LENGTH, RUNTIME_LOG_FONT_A_FACE_CAPACITY, RuntimeLogFontA,
};

pub const GDI32_COMPATIBILITY_POLICY_SCHEMA: &str = "vine.gdi32-compatibility-policy";
pub const GDI32_COMPATIBILITY_POLICY_VERSION: u32 = 3;
pub const GET_STOCK_OBJECT_IMPLEMENTATION: &str = "gdi32.get-stock-object.black-brush.v1";
pub const ENUM_FONT_FAMILIES_EX_A_IMPLEMENTATION: &str =
    "gdi32.enum-font-families-ex-a.cp932-screen.v1";
pub const CREATE_FONT_A_IMPLEMENTATION: &str = "gdi32.create-font-a.cp932-logical-object.v1";
pub const CREATE_COMPATIBLE_DC_IMPLEMENTATION: &str =
    "gdi32.create-compatible-dc.null-screen-memory.v1";
pub const DELETE_DC_IMPLEMENTATION: &str = "gdi32.delete-dc.logical-memory.v1";
pub const X86_LOGFONTA_BYTE_LENGTH: u32 = RUNTIME_LOG_FONT_A_BYTE_LENGTH as u32;
pub const X86_ENUMLOGFONTEXA_BYTE_LENGTH: usize = 188;
pub const X86_NEWTEXTMETRICEXA_BYTE_LENGTH: usize = 96;

const SHIFT_JIS_CHARSET: u8 = 128;
const TRUETYPE_FONTTYPE: u32 = 0x0000_0004;
const MS_GOTHIC_WINDOWS_932: &[u8] = &[
    0x82, 0x6c, 0x82, 0x72, 0x20, 0x83, 0x53, 0x83, 0x56, 0x83, 0x62, 0x83, 0x4e,
];
const REGULAR_WINDOWS_932: &[u8] = &[0x95, 0x57, 0x8f, 0x80];
const JAPANESE_WINDOWS_932: &[u8] = &[0x93, 0xfa, 0x96, 0x7b, 0x8c, 0xea];
const FONT_SIGNATURE: [u8; 24] = [
    0xff, 0x02, 0x00, 0xe0, 0xfb, 0xfd, 0xc7, 0x6a, 0x12, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0x00,
    0x9f, 0x00, 0x02, 0x40, 0x00, 0x00, 0xd7, 0xdf,
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Gdi32FontRecordPolicy {
    pub font_type: u32,
    pub enum_log_font_ex_a: Vec<u8>,
    pub new_text_metric_ex_a: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Gdi32FontEnumerationPolicy {
    pub profile_id: String,
    pub input_character_set: u8,
    pub input_face_name_windows_932: Vec<u8>,
    pub flags: u32,
    pub record: Gdi32FontRecordPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Gdi32FontCreationPolicy {
    pub profile_id: String,
    pub log_font: RuntimeLogFontA,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Gdi32CompatibleDcPolicy {
    pub profile_id: String,
    pub source_device_context: u32,
    pub initial_font_stock_index: i32,
    pub initial_bitmap_stock_index: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Gdi32CompatibilityPolicy {
    pub schema: String,
    pub schema_version: u32,
    pub id: String,
    pub enum_font_families_ex_a: Option<Gdi32FontEnumerationPolicy>,
    pub create_font_a: Option<Gdi32FontCreationPolicy>,
    pub create_compatible_dc: Option<Gdi32CompatibleDcPolicy>,
}

impl Gdi32CompatibilityPolicy {
    pub fn controlled_v3() -> Self {
        Self {
            schema: GDI32_COMPATIBILITY_POLICY_SCHEMA.to_owned(),
            schema_version: GDI32_COMPATIBILITY_POLICY_VERSION,
            id: "gdi32-primary-cp932-memory-dc-v3".to_owned(),
            enum_font_families_ex_a: Some(controlled_font_enumeration_policy()),
            create_font_a: Some(controlled_font_creation_policy()),
            create_compatible_dc: Some(controlled_compatible_dc_policy()),
        }
    }

    pub fn validate(&self) -> Result<(), ApiFault> {
        if self.schema != GDI32_COMPATIBILITY_POLICY_SCHEMA
            || self.schema_version != GDI32_COMPATIBILITY_POLICY_VERSION
            || !valid_policy_id(&self.id)
        {
            return Err(ApiFault::runtime_defect(
                "gdi32-compatibility-policy",
                "gdi32-policy-header-invalid",
                "GDI32 compatibility policy has an unsupported schema, version, or ID",
            ));
        }
        if let Some(enumeration) = self.enum_font_families_ex_a.as_ref()
            && enumeration != &controlled_font_enumeration_policy()
        {
            return Err(ApiFault::runtime_defect(
                "gdi32-compatibility-policy",
                "gdi32-font-enumeration-profile-invalid",
                "GDI32 font enumeration policy must match the evidenced single CP932 MS Gothic record",
            ));
        }
        if let Some(creation) = self.create_font_a.as_ref()
            && creation != &controlled_font_creation_policy()
        {
            return Err(ApiFault::runtime_defect(
                "gdi32-compatibility-policy",
                "gdi32-font-creation-profile-invalid",
                "GDI32 font creation policy must match the evidenced CP932 MS Gothic request",
            ));
        }
        if let Some(compatible_dc) = self.create_compatible_dc.as_ref()
            && compatible_dc != &controlled_compatible_dc_policy()
        {
            return Err(ApiFault::runtime_defect(
                "gdi32-compatibility-policy",
                "gdi32-compatible-dc-profile-invalid",
                "GDI32 compatible DC policy must match the evidenced NULL-screen memory DC request",
            ));
        }
        Ok(())
    }
}

pub fn dispatch_gdi32_api(
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> ApiHandlerResult {
    match contract.implementation_id.as_str() {
        GET_STOCK_OBJECT_IMPLEMENTATION => {
            match handle_get_stock_object(contract, frame, session) {
                Ok(immediate) => ApiHandlerResult::Immediate(immediate),
                Err(fault) => ApiHandlerResult::Fault(fault),
            }
        }
        DELETE_DC_IMPLEMENTATION => match handle_delete_dc(contract, frame, session) {
            Ok(immediate) => ApiHandlerResult::Immediate(immediate),
            Err(fault) => ApiHandlerResult::Fault(fault),
        },
        _ => ApiHandlerResult::Fault(ApiFault::unsupported(
            "gdi32-api-dispatch",
            "implementation-missing",
            format!(
                "no GDI32 handler is registered for {}",
                contract.implementation_id
            ),
        )),
    }
}

pub fn dispatch_gdi32_api_with_policy(
    policy: &Gdi32CompatibilityPolicy,
    callback_return_sentinel: GuestAddress,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> ApiHandlerResult {
    match contract.implementation_id.as_str() {
        CREATE_COMPATIBLE_DC_IMPLEMENTATION => {
            match handle_create_compatible_dc(policy, contract, frame, session) {
                Ok(immediate) => ApiHandlerResult::Immediate(immediate),
                Err(fault) => ApiHandlerResult::Fault(fault),
            }
        }
        CREATE_FONT_A_IMPLEMENTATION => {
            match handle_create_font_a(policy, contract, frame, session) {
                Ok(immediate) => ApiHandlerResult::Immediate(immediate),
                Err(fault) => ApiHandlerResult::Fault(fault),
            }
        }
        ENUM_FONT_FAMILIES_EX_A_IMPLEMENTATION => handle_enum_font_families_ex_a(
            policy,
            callback_return_sentinel,
            contract,
            frame,
            session,
        ),
        _ => dispatch_gdi32_api(contract, frame, session),
    }
}

fn handle_create_compatible_dc(
    policy: &Gdi32CompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let compatible_dc = policy.create_compatible_dc.as_ref().ok_or_else(|| {
        ApiFault::unsupported(
            "gdi32-compatible-dc",
            "create-compatible-dc-policy-disabled",
            format!(
                "compatibility policy {} does not enable logical memory DC creation",
                policy.id
            ),
        )
    })?;
    let source_device_context = required_argument(frame, 0, "source device context")?;
    if source_device_context != compatible_dc.source_device_context {
        return Err(ApiFault::unsupported(
            "gdi32-compatible-dc",
            "create-compatible-dc-source-unsupported",
            "CreateCompatibleDC currently supports only the evidenced NULL screen source",
        ));
    }
    let preserved_last_error = session
        .last_error()
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    let result = session
        .runtime_mut()
        .create_compatible_memory_dc()
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    Ok(ApiImmediate::new(ApiReturnValue::LogicalHandle {
        namespace: LOGICAL_DEVICE_CONTEXT_HANDLE_NAMESPACE.to_owned(),
        value: result.handle,
    })
    .with_last_error(preserved_last_error))
}

fn handle_delete_dc(
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let device_context = required_argument(frame, 0, "device context")?;
    let preserved_last_error = session
        .last_error()
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    match session
        .runtime_mut()
        .delete_compatible_memory_dc(device_context)
    {
        Ok(()) => {
            Ok(ApiImmediate::new(ApiReturnValue::Bool32(true))
                .with_last_error(preserved_last_error))
        }
        Err(error)
            if error.code == "device-context-handle-unknown"
                || error.code == "delete-dc-relation-mismatch" =>
        {
            Ok(ApiImmediate::new(ApiReturnValue::Bool32(false))
                .with_last_error(preserved_last_error))
        }
        Err(error) => Err(ApiFault::from_runtime(error, &contract.endpoint)),
    }
}

fn handle_create_font_a(
    policy: &Gdi32CompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let creation = policy.create_font_a.as_ref().ok_or_else(|| {
        ApiFault::unsupported(
            "gdi32-fonts",
            "create-font-a-policy-disabled",
            format!(
                "compatibility policy {} does not enable logical font creation",
                policy.id
            ),
        )
    })?;
    let face_address = required_argument(frame, 13, "ANSI face name")?;
    let request = RuntimeLogFontA {
        height: required_argument(frame, 0, "height")? as i32,
        width: required_argument(frame, 1, "width")? as i32,
        escapement: required_argument(frame, 2, "escapement")? as i32,
        orientation: required_argument(frame, 3, "orientation")? as i32,
        weight: required_argument(frame, 4, "weight")? as i32,
        italic: required_byte_argument(frame, 5, "italic")?,
        underline: required_byte_argument(frame, 6, "underline")?,
        strike_out: required_byte_argument(frame, 7, "strike out")?,
        character_set: required_byte_argument(frame, 8, "character set")?,
        output_precision: required_byte_argument(frame, 9, "output precision")?,
        clip_precision: required_byte_argument(frame, 10, "clip precision")?,
        quality: required_byte_argument(frame, 11, "quality")?,
        pitch_and_family: required_byte_argument(frame, 12, "pitch and family")?,
        face_name_windows_932: read_font_face_name(contract, session, GuestAddress(face_address))?,
    };
    if request != creation.log_font {
        return Err(ApiFault::unsupported(
            "gdi32-fonts",
            "create-font-a-variant-unsupported",
            "CreateFontA currently supports only the evidenced CP932 MS Gothic request",
        ));
    }
    let preserved_last_error = session
        .last_error()
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    let result = session
        .runtime_mut()
        .create_gdi_font_a(request)
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    Ok(ApiImmediate::new(ApiReturnValue::LogicalHandle {
        namespace: LOGICAL_GDI_OBJECT_HANDLE_NAMESPACE.to_owned(),
        value: result.handle,
    })
    .with_last_error(preserved_last_error))
}

fn read_font_face_name(
    contract: &ApiContract,
    session: &ExecutionSession<'_>,
    address: GuestAddress,
) -> Result<Vec<u8>, ApiFault> {
    let mut bytes = Vec::with_capacity(RUNTIME_LOG_FONT_A_FACE_CAPACITY);
    for offset in 0..RUNTIME_LOG_FONT_A_FACE_CAPACITY {
        let offset = u32::try_from(offset).map_err(|_| {
            ApiFault::runtime_defect(
                "gdi32-fonts",
                "create-font-a-face-offset-invalid",
                "validated LF_FACESIZE does not fit x86 address accounting",
            )
        })?;
        let current = address
            .0
            .checked_add(offset)
            .map(GuestAddress)
            .ok_or_else(|| {
                ApiFault::invalid_call(
                    "gdi32-fonts",
                    "create-font-a-face-address-overflow",
                    "CreateFontA face-name read overflowed x86 address accounting",
                )
            })?;
        let byte = session
            .memory()
            .read(current, 1)
            .map_err(|error| ApiFault::from_memory(error, &contract.endpoint))?[0];
        if byte == 0 {
            return Ok(bytes);
        }
        bytes.push(byte);
    }
    Err(ApiFault::unsupported(
        "gdi32-fonts",
        "create-font-a-face-name-unterminated",
        "CreateFontA face name is not NUL-terminated within LF_FACESIZE",
    ))
}

fn handle_get_stock_object(
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let index = frame.argument_u32(0).ok_or_else(|| {
        ApiFault::runtime_defect(
            "gdi32-api-dispatch",
            "decoded-argument-missing",
            "decoded GetStockObject frame is missing object index 0",
        )
    })? as i32;
    let preserved_last_error = session
        .last_error()
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    match session.runtime_mut().get_gdi_stock_object(index) {
        Ok(result) => Ok(ApiImmediate::new(ApiReturnValue::LogicalHandle {
            namespace: LOGICAL_GDI_OBJECT_HANDLE_NAMESPACE.to_owned(),
            value: result.handle,
        })
        .with_last_error(preserved_last_error)),
        Err(error) if error.code == "stock-object-index-unsupported" => {
            Ok(ApiImmediate::new(ApiReturnValue::LogicalHandle {
                namespace: LOGICAL_GDI_OBJECT_HANDLE_NAMESPACE.to_owned(),
                value: 0,
            })
            .with_last_error(preserved_last_error))
        }
        Err(error) => Err(ApiFault::from_runtime(error, &contract.endpoint)),
    }
}

fn handle_enum_font_families_ex_a(
    policy: &Gdi32CompatibilityPolicy,
    callback_return_sentinel: GuestAddress,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &ExecutionSession<'_>,
) -> ApiHandlerResult {
    let Some(enumeration) = policy.enum_font_families_ex_a.as_ref() else {
        return ApiHandlerResult::Fault(ApiFault::unsupported(
            "gdi32-fonts",
            "enum-font-families-ex-a-policy-disabled",
            format!(
                "compatibility policy {} does not enable logical font enumeration",
                policy.id
            ),
        ));
    };
    let result = (|| {
        let device_context = required_argument(frame, 0, "device context")?;
        let log_font_address = required_argument(frame, 1, "LOGFONTA filter")?;
        let callback = required_argument(frame, 2, "font enumeration callback")?;
        let callback_parameter = required_argument(frame, 3, "callback parameter")?;
        let flags = required_argument(frame, 4, "enumeration flags")?;
        if flags != enumeration.flags {
            return Err(ApiFault::unsupported(
                "gdi32-fonts",
                "enum-font-families-ex-a-flags-unsupported",
                "EnumFontFamiliesExA currently supports only the reached zero-flags variant",
            ));
        }
        let log_font = session
            .memory()
            .read(
                GuestAddress(log_font_address),
                X86_LOGFONTA_BYTE_LENGTH as usize,
            )
            .map_err(|error| ApiFault::from_memory(error, &contract.endpoint))?;
        let face_storage = log_font.get(28..60).ok_or_else(|| {
            ApiFault::runtime_defect(
                "gdi32-fonts",
                "logfonta-layout-invalid",
                "validated LOGFONTA bytes do not contain the fixed face-name field",
            )
        })?;
        let face_length = face_storage
            .iter()
            .position(|&byte| byte == 0)
            .ok_or_else(|| {
                ApiFault::unsupported(
                    "gdi32-fonts",
                    "enum-font-families-ex-a-face-name-unterminated",
                    "EnumFontFamiliesExA face filter is not NUL-terminated within LF_FACESIZE",
                )
            })?;
        if log_font[23] != enumeration.input_character_set
            || face_storage[..face_length] != enumeration.input_face_name_windows_932
        {
            return Err(ApiFault::unsupported(
                "gdi32-fonts",
                "enum-font-families-ex-a-filter-unsupported",
                "EnumFontFamiliesExA currently supports only the reached CP932 MS Gothic filter",
            ));
        }
        session
            .memory()
            .validate_access(GuestAddress(callback), 1, MemoryAccess::Execute)
            .map_err(|error| ApiFault::from_memory(error, &contract.endpoint))?;
        session
            .runtime()
            .validate_screen_dc_operation(device_context)
            .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;

        let mut scratch =
            Vec::with_capacity(X86_ENUMLOGFONTEXA_BYTE_LENGTH + X86_NEWTEXTMETRICEXA_BYTE_LENGTH);
        scratch.extend_from_slice(&enumeration.record.enum_log_font_ex_a);
        scratch.extend_from_slice(&enumeration.record.new_text_metric_ex_a);
        Ok(GuestCallbackRequest {
            target: GuestAddress(callback),
            arguments: vec![
                GuestCallbackArgument::ScratchPointer { offset: 0 },
                GuestCallbackArgument::ScratchPointer {
                    offset: X86_ENUMLOGFONTEXA_BYTE_LENGTH as u32,
                },
                GuestCallbackArgument::U32(enumeration.record.font_type),
                GuestCallbackArgument::U32(callback_parameter),
            ],
            scratch,
            scratch_pointer_relocations: Vec::new(),
            return_sentinel: callback_return_sentinel,
            completion_policy: GuestCallbackCompletionPolicy::ReturnU32,
        })
    })();
    match result {
        Ok(request) => ApiHandlerResult::GuestCallback(request),
        Err(fault) => ApiHandlerResult::Fault(fault),
    }
}

pub fn gdi32_contract_for_endpoint(
    endpoint: ApiEndpointIdentity,
) -> Result<Option<ApiContract>, ApiFault> {
    if !endpoint.module.eq_ignore_ascii_case("gdi32.dll") {
        return Ok(None);
    }
    let ApiSymbol::Name { name } = &endpoint.symbol else {
        return Ok(None);
    };
    match name.as_str() {
        "GetStockObject" => Ok(Some(get_stock_object_contract(endpoint)?)),
        "EnumFontFamiliesExA" => Ok(Some(enum_font_families_ex_a_contract(endpoint)?)),
        "CreateFontA" => Ok(Some(create_font_a_contract(endpoint)?)),
        "CreateCompatibleDC" => Ok(Some(create_compatible_dc_contract(endpoint)?)),
        "DeleteDC" => Ok(Some(delete_dc_contract(endpoint)?)),
        _ => Ok(None),
    }
}

pub fn create_compatible_dc_contract(
    endpoint: ApiEndpointIdentity,
) -> Result<ApiContract, ApiFault> {
    Ok(ApiContract {
        endpoint,
        abi: AbiSchema {
            calling_convention: CallingConvention::Stdcall,
            arguments: vec![logical_handle_argument(
                "sourceDeviceContext",
                LOGICAL_DEVICE_CONTEXT_HANDLE_NAMESPACE,
                true,
            )],
            return_value: ApiReturnSchema::LogicalHandle {
                namespace: LOGICAL_DEVICE_CONTEXT_HANDLE_NAMESPACE.to_owned(),
                nullable: true,
            },
            stack_cleanup: StackCleanup::Callee,
        },
        last_error_policy: LastErrorPolicy::FromHandler,
        reentrancy_policy: ReentrancyPolicy::Allowed,
        behavior: ApiBehavior::Immediate,
        callback_policy: CallbackPolicy::Forbidden,
        implementation_id: CREATE_COMPATIBLE_DC_IMPLEMENTATION.to_owned(),
    })
}

pub fn delete_dc_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    Ok(ApiContract {
        endpoint,
        abi: AbiSchema {
            calling_convention: CallingConvention::Stdcall,
            arguments: vec![logical_handle_argument(
                "deviceContext",
                LOGICAL_DEVICE_CONTEXT_HANDLE_NAMESPACE,
                true,
            )],
            return_value: ApiReturnSchema::Bool32,
            stack_cleanup: StackCleanup::Callee,
        },
        last_error_policy: LastErrorPolicy::FromHandler,
        reentrancy_policy: ReentrancyPolicy::Allowed,
        behavior: ApiBehavior::Immediate,
        callback_policy: CallbackPolicy::Forbidden,
        implementation_id: DELETE_DC_IMPLEMENTATION.to_owned(),
    })
}

pub fn get_stock_object_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    Ok(ApiContract {
        endpoint,
        abi: AbiSchema {
            calling_convention: CallingConvention::Stdcall,
            arguments: vec![ApiArgumentSchema {
                name: "objectIndex".to_owned(),
                direction: ApiDirection::In,
                value: ApiValueSchema::U32,
            }],
            return_value: ApiReturnSchema::LogicalHandle {
                namespace: LOGICAL_GDI_OBJECT_HANDLE_NAMESPACE.to_owned(),
                nullable: true,
            },
            stack_cleanup: StackCleanup::Callee,
        },
        last_error_policy: LastErrorPolicy::FromHandler,
        reentrancy_policy: ReentrancyPolicy::Allowed,
        behavior: ApiBehavior::Immediate,
        callback_policy: CallbackPolicy::Forbidden,
        implementation_id: GET_STOCK_OBJECT_IMPLEMENTATION.to_owned(),
    })
}

pub fn enum_font_families_ex_a_contract(
    endpoint: ApiEndpointIdentity,
) -> Result<ApiContract, ApiFault> {
    Ok(ApiContract {
        endpoint,
        abi: AbiSchema {
            calling_convention: CallingConvention::Stdcall,
            arguments: vec![
                logical_handle_argument(
                    "deviceContext",
                    LOGICAL_DEVICE_CONTEXT_HANDLE_NAMESPACE,
                    false,
                ),
                required_in_struct("logFont", "LOGFONTA", X86_LOGFONTA_BYTE_LENGTH),
                opaque_pointer_argument("enumerationCallback", false, 1),
                u32_argument("callbackParameter"),
                u32_argument("flags"),
            ],
            return_value: ApiReturnSchema::U32,
            stack_cleanup: StackCleanup::Callee,
        },
        last_error_policy: LastErrorPolicy::Preserve,
        reentrancy_policy: ReentrancyPolicy::Allowed,
        behavior: ApiBehavior::Immediate,
        callback_policy: CallbackPolicy::Required,
        implementation_id: ENUM_FONT_FAMILIES_EX_A_IMPLEMENTATION.to_owned(),
    })
}

pub fn create_font_a_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    Ok(ApiContract {
        endpoint,
        abi: AbiSchema {
            calling_convention: CallingConvention::Stdcall,
            arguments: [
                "height",
                "width",
                "escapement",
                "orientation",
                "weight",
                "italic",
                "underline",
                "strikeOut",
                "characterSet",
                "outputPrecision",
                "clipPrecision",
                "quality",
                "pitchAndFamily",
            ]
            .into_iter()
            .map(u32_argument)
            .chain(std::iter::once(input_font_face_argument()))
            .collect(),
            return_value: ApiReturnSchema::LogicalHandle {
                namespace: LOGICAL_GDI_OBJECT_HANDLE_NAMESPACE.to_owned(),
                nullable: true,
            },
            stack_cleanup: StackCleanup::Callee,
        },
        last_error_policy: LastErrorPolicy::FromHandler,
        reentrancy_policy: ReentrancyPolicy::Allowed,
        behavior: ApiBehavior::Immediate,
        callback_policy: CallbackPolicy::Forbidden,
        implementation_id: CREATE_FONT_A_IMPLEMENTATION.to_owned(),
    })
}

fn controlled_font_enumeration_policy() -> Gdi32FontEnumerationPolicy {
    Gdi32FontEnumerationPolicy {
        profile_id: "primary-cp932-ms-gothic-regular-v1".to_owned(),
        input_character_set: SHIFT_JIS_CHARSET,
        input_face_name_windows_932: MS_GOTHIC_WINDOWS_932.to_vec(),
        flags: 0,
        record: Gdi32FontRecordPolicy {
            font_type: TRUETYPE_FONTTYPE,
            enum_log_font_ex_a: controlled_enum_log_font_ex_a(),
            new_text_metric_ex_a: controlled_new_text_metric_ex_a(),
        },
    }
}

fn controlled_font_creation_policy() -> Gdi32FontCreationPolicy {
    Gdi32FontCreationPolicy {
        profile_id: "primary-cp932-ms-gothic-64x32-thin-v1".to_owned(),
        log_font: RuntimeLogFontA {
            height: 64,
            width: 32,
            escapement: 0,
            orientation: 0,
            weight: 100,
            italic: 0,
            underline: 0,
            strike_out: 0,
            character_set: SHIFT_JIS_CHARSET,
            output_precision: 4,
            clip_precision: 0,
            quality: 2,
            pitch_and_family: 0,
            face_name_windows_932: MS_GOTHIC_WINDOWS_932.to_vec(),
        },
    }
}

fn controlled_compatible_dc_policy() -> Gdi32CompatibleDcPolicy {
    Gdi32CompatibleDcPolicy {
        profile_id: "primary-null-screen-memory-dc-v1".to_owned(),
        source_device_context: 0,
        initial_font_stock_index: RUNTIME_GDI_SYSTEM_FONT_INDEX,
        initial_bitmap_stock_index: RUNTIME_GDI_DEFAULT_BITMAP_INDEX,
    }
}

fn controlled_enum_log_font_ex_a() -> Vec<u8> {
    let mut bytes = vec![0; X86_ENUMLOGFONTEXA_BYTE_LENGTH];
    write_i32(&mut bytes, 0, 32);
    write_i32(&mut bytes, 4, 16);
    write_i32(&mut bytes, 16, 400);
    bytes[23] = SHIFT_JIS_CHARSET;
    bytes[24] = 3;
    bytes[25] = 2;
    bytes[26] = 1;
    bytes[27] = 49;
    write_fixed_ansi(&mut bytes, 28, 32, MS_GOTHIC_WINDOWS_932);
    write_fixed_ansi(&mut bytes, 60, 64, MS_GOTHIC_WINDOWS_932);
    write_fixed_ansi(&mut bytes, 124, 32, REGULAR_WINDOWS_932);
    write_fixed_ansi(&mut bytes, 156, 32, JAPANESE_WINDOWS_932);
    bytes
}

fn controlled_new_text_metric_ex_a() -> Vec<u8> {
    let mut bytes = vec![0; X86_NEWTEXTMETRICEXA_BYTE_LENGTH];
    for (offset, value) in [
        (0, 32_i32),
        (4, 28),
        (8, 4),
        (20, 16),
        (24, 63),
        (28, 400),
        (36, 96),
        (40, 96),
    ] {
        write_i32(&mut bytes, offset, value);
    }
    bytes[44] = 0;
    bytes[45] = 255;
    bytes[46] = 165;
    bytes[47] = 2;
    bytes[51] = 54;
    bytes[52] = SHIFT_JIS_CHARSET;
    write_u32(&mut bytes, 56, 2_359_360);
    write_u32(&mut bytes, 60, 256);
    write_u32(&mut bytes, 64, 256);
    write_u32(&mut bytes, 68, 128);
    bytes[72..96].copy_from_slice(&FONT_SIGNATURE);
    bytes
}

fn write_i32(bytes: &mut [u8], offset: usize, value: i32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_fixed_ansi(bytes: &mut [u8], offset: usize, capacity: usize, value: &[u8]) {
    debug_assert!(value.len() < capacity);
    bytes[offset..offset + value.len()].copy_from_slice(value);
}

fn required_argument(
    frame: &ApiCallFrame,
    index: usize,
    description: &str,
) -> Result<u32, ApiFault> {
    frame.argument_u32(index).ok_or_else(|| {
        ApiFault::runtime_defect(
            "gdi32-api-dispatch",
            "decoded-argument-missing",
            format!("decoded GDI32 frame is missing {description}"),
        )
    })
}

fn required_byte_argument(
    frame: &ApiCallFrame,
    index: usize,
    description: &str,
) -> Result<u8, ApiFault> {
    let value = required_argument(frame, index, description)?;
    u8::try_from(value).map_err(|_| {
        ApiFault::unsupported(
            "gdi32-fonts",
            "create-font-a-variant-unsupported",
            format!("CreateFontA {description} is outside the evidenced byte-sized variant"),
        )
    })
}

fn u32_argument(name: &str) -> ApiArgumentSchema {
    ApiArgumentSchema {
        name: name.to_owned(),
        direction: ApiDirection::In,
        value: ApiValueSchema::U32,
    }
}

fn logical_handle_argument(name: &str, namespace: &str, nullable: bool) -> ApiArgumentSchema {
    ApiArgumentSchema {
        name: name.to_owned(),
        direction: ApiDirection::In,
        value: ApiValueSchema::LogicalHandle {
            namespace: namespace.to_owned(),
            nullable,
        },
    }
}

fn opaque_pointer_argument(name: &str, nullable: bool, alignment: u32) -> ApiArgumentSchema {
    ApiArgumentSchema {
        name: name.to_owned(),
        direction: ApiDirection::In,
        value: ApiValueSchema::OpaquePointer {
            nullable,
            alignment,
        },
    }
}

fn input_font_face_argument() -> ApiArgumentSchema {
    ApiArgumentSchema {
        name: "faceName".to_owned(),
        direction: ApiDirection::In,
        value: ApiValueSchema::Pointer {
            constraint: ApiPointerConstraint {
                nullable: false,
                alignment: 1,
                byte_length: 1,
                pointee: ApiPointeeSchema::String {
                    encoding: ApiStringEncoding::Windows932,
                    maximum_code_units: RUNTIME_LOG_FONT_A_FACE_CAPACITY as u32,
                    nul_terminated: true,
                },
                write_requirement: ApiWriteRequirement::None,
            },
        },
    }
}

fn required_in_struct(name: &str, type_id: &str, byte_length: u32) -> ApiArgumentSchema {
    ApiArgumentSchema {
        name: name.to_owned(),
        direction: ApiDirection::In,
        value: ApiValueSchema::Pointer {
            constraint: ApiPointerConstraint {
                nullable: false,
                alignment: 4,
                byte_length,
                pointee: ApiPointeeSchema::Struct {
                    type_id: type_id.to_owned(),
                },
                write_requirement: ApiWriteRequirement::None,
            },
        },
    }
}

fn valid_policy_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn controlled_gdi32_policy_is_canonical_and_bounded() {
        let policy = Gdi32CompatibilityPolicy::controlled_v3();
        policy.validate().unwrap();
        let enumeration = policy.enum_font_families_ex_a.as_ref().unwrap();
        assert_eq!(enumeration.input_character_set, SHIFT_JIS_CHARSET);
        assert_eq!(
            enumeration.input_face_name_windows_932,
            MS_GOTHIC_WINDOWS_932
        );
        assert_eq!(enumeration.record.font_type, TRUETYPE_FONTTYPE);
        assert_eq!(
            enumeration.record.enum_log_font_ex_a.len(),
            X86_ENUMLOGFONTEXA_BYTE_LENGTH
        );
        assert_eq!(
            enumeration.record.new_text_metric_ex_a.len(),
            X86_NEWTEXTMETRICEXA_BYTE_LENGTH
        );
        assert_eq!(
            &enumeration.record.enum_log_font_ex_a[28..41],
            MS_GOTHIC_WINDOWS_932
        );
        assert_eq!(enumeration.record.enum_log_font_ex_a[41], 0);
        assert_eq!(
            enumeration.record.new_text_metric_ex_a[52],
            SHIFT_JIS_CHARSET
        );
        let creation = policy.create_font_a.as_ref().unwrap();
        assert_eq!(creation.log_font.height, 64);
        assert_eq!(creation.log_font.width, 32);
        assert_eq!(creation.log_font.weight, 100);
        assert_eq!(creation.log_font.character_set, SHIFT_JIS_CHARSET);
        assert_eq!(creation.log_font.output_precision, 4);
        assert_eq!(creation.log_font.clip_precision, 0);
        assert_eq!(creation.log_font.quality, 2);
        assert_eq!(creation.log_font.pitch_and_family, 0);
        assert_eq!(
            creation.log_font.face_name_windows_932,
            MS_GOTHIC_WINDOWS_932
        );
        assert_eq!(
            creation.log_font.to_bytes().unwrap()[..46],
            [
                64, 0, 0, 0, 32, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 100, 0, 0, 0, 0, 0, 0, 128, 4, 0,
                2, 0, 130, 108, 130, 114, 32, 131, 83, 131, 86, 131, 98, 131, 78, 0, 0, 0, 0, 0,
            ]
        );
        let compatible_dc = policy.create_compatible_dc.as_ref().unwrap();
        assert_eq!(compatible_dc.source_device_context, 0);
        assert_eq!(compatible_dc.initial_font_stock_index, 13);
        assert_eq!(compatible_dc.initial_bitmap_stock_index, 21);
    }

    #[test]
    fn mutated_font_policy_is_rejected() {
        let mut policy = Gdi32CompatibilityPolicy::controlled_v3();
        policy
            .enum_font_families_ex_a
            .as_mut()
            .unwrap()
            .record
            .enum_log_font_ex_a[188 - 1] = 1;
        let fault = policy.validate().unwrap_err();
        assert_eq!(fault.code, "gdi32-font-enumeration-profile-invalid");

        let mut policy = Gdi32CompatibilityPolicy::controlled_v3();
        policy.create_font_a.as_mut().unwrap().log_font.quality = 1;
        let fault = policy.validate().unwrap_err();
        assert_eq!(fault.code, "gdi32-font-creation-profile-invalid");

        let mut policy = Gdi32CompatibilityPolicy::controlled_v3();
        policy
            .create_compatible_dc
            .as_mut()
            .unwrap()
            .initial_bitmap_stock_index = 20;
        let fault = policy.validate().unwrap_err();
        assert_eq!(fault.code, "gdi32-compatible-dc-profile-invalid");
    }
}

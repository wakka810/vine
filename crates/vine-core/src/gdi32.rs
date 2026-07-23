use serde::Serialize;

use crate::api::{
    AbiSchema, ApiArgumentSchema, ApiBehavior, ApiCallFrame, ApiContract, ApiDirection,
    ApiEndpointIdentity, ApiFault, ApiHandlerResult, ApiImmediate, ApiPointeeSchema,
    ApiPointerConstraint, ApiReturnSchema, ApiReturnValue, ApiSymbol, ApiValueSchema,
    ApiWriteRequirement, CallbackPolicy, CallingConvention, ExecutionSession,
    GuestCallbackArgument, GuestCallbackCompletionPolicy, GuestCallbackRequest, LastErrorPolicy,
    ReentrancyPolicy, StackCleanup,
};
use crate::memory::{GuestAddress, MemoryAccess};
use crate::runtime::LOGICAL_DEVICE_CONTEXT_HANDLE_NAMESPACE;
use crate::runtime::LOGICAL_GDI_OBJECT_HANDLE_NAMESPACE;

pub const GDI32_COMPATIBILITY_POLICY_SCHEMA: &str = "vine.gdi32-compatibility-policy";
pub const GDI32_COMPATIBILITY_POLICY_VERSION: u32 = 1;
pub const GET_STOCK_OBJECT_IMPLEMENTATION: &str = "gdi32.get-stock-object.black-brush.v1";
pub const ENUM_FONT_FAMILIES_EX_A_IMPLEMENTATION: &str =
    "gdi32.enum-font-families-ex-a.cp932-screen.v1";
pub const X86_LOGFONTA_BYTE_LENGTH: u32 = 60;
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
pub struct Gdi32CompatibilityPolicy {
    pub schema: String,
    pub schema_version: u32,
    pub id: String,
    pub enum_font_families_ex_a: Option<Gdi32FontEnumerationPolicy>,
}

impl Gdi32CompatibilityPolicy {
    pub fn controlled_v1() -> Self {
        Self {
            schema: GDI32_COMPATIBILITY_POLICY_SCHEMA.to_owned(),
            schema_version: GDI32_COMPATIBILITY_POLICY_VERSION,
            id: "gdi32-primary-cp932-font-catalog-v1".to_owned(),
            enum_font_families_ex_a: Some(controlled_font_enumeration_policy()),
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
        _ => Ok(None),
    }
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
    fn controlled_font_policy_is_canonical_and_bounded() {
        let policy = Gdi32CompatibilityPolicy::controlled_v1();
        policy.validate().unwrap();
        let enumeration = policy.enum_font_families_ex_a.unwrap();
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
    }

    #[test]
    fn mutated_font_policy_is_rejected() {
        let mut policy = Gdi32CompatibilityPolicy::controlled_v1();
        policy
            .enum_font_families_ex_a
            .as_mut()
            .unwrap()
            .record
            .enum_log_font_ex_a[188 - 1] = 1;
        let fault = policy.validate().unwrap_err();
        assert_eq!(fault.code, "gdi32-font-enumeration-profile-invalid");
    }
}

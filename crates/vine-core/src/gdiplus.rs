use serde::Serialize;

use crate::api::{
    AbiSchema, ApiArgumentSchema, ApiBehavior, ApiCallFrame, ApiContract, ApiDirection,
    ApiEndpointIdentity, ApiFault, ApiGuestWrite, ApiGuestWriteReceipt, ApiHandlerResult,
    ApiImmediate, ApiPointeeSchema, ApiPointerConstraint, ApiReturnSchema, ApiReturnValue,
    ApiSymbol, ApiValueSchema, ApiWriteRequirement, CallbackPolicy, CallingConvention,
    ExecutionSession, LastErrorPolicy, ReentrancyPolicy, StackCleanup,
};
use crate::memory::GuestAddress;
use crate::runtime::{RuntimeGdiplusStartupRequest, RuntimeStateErrorClass};

pub const GDIPLUS_COMPATIBILITY_POLICY_SCHEMA: &str = "vine.gdiplus-compatibility-policy";
pub const GDIPLUS_COMPATIBILITY_POLICY_VERSION: u32 = 1;
pub const GDIPLUS_STARTUP_IMPLEMENTATION: &str = "gdiplus.startup.primary-v1";
pub const GDIPLUS_SHUTDOWN_IMPLEMENTATION: &str = "gdiplus.shutdown.logical-session-v1";
pub const X86_GDIPLUS_STARTUP_INPUT_BYTE_LENGTH: u32 = 16;
pub const X86_GDIPLUS_STARTUP_OUTPUT_BYTE_LENGTH: u32 = 8;

pub const GDIPLUS_STATUS_OK: u32 = 0;
pub const GDIPLUS_STATUS_OUT_OF_MEMORY: u32 = 3;
pub const GDIPLUS_STATUS_UNSUPPORTED_GDIPLUS_VERSION: u32 = 17;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GdiplusStartupPolicy {
    pub profile_id: String,
    pub version: u32,
    pub debug_event_callback: u32,
    pub suppress_background_thread: bool,
    pub suppress_external_codecs: bool,
    pub notification_output: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GdiplusCompatibilityPolicy {
    pub schema: String,
    pub schema_version: u32,
    pub id: String,
    pub startup: Option<GdiplusStartupPolicy>,
}

impl GdiplusCompatibilityPolicy {
    pub fn controlled_v1() -> Self {
        Self {
            schema: GDIPLUS_COMPATIBILITY_POLICY_SCHEMA.to_owned(),
            schema_version: GDIPLUS_COMPATIBILITY_POLICY_VERSION,
            id: "gdiplus-primary-startup-v1".to_owned(),
            startup: Some(controlled_startup_policy()),
        }
    }

    pub fn validate(&self) -> Result<(), ApiFault> {
        if self.schema != GDIPLUS_COMPATIBILITY_POLICY_SCHEMA
            || self.schema_version != GDIPLUS_COMPATIBILITY_POLICY_VERSION
            || !valid_policy_id(&self.id)
        {
            return Err(ApiFault::runtime_defect(
                "gdiplus-compatibility-policy",
                "gdiplus-policy-header-invalid",
                "GDI+ compatibility policy has an unsupported schema, version, or ID",
            ));
        }
        if let Some(startup) = self.startup.as_ref()
            && startup != &controlled_startup_policy()
        {
            return Err(ApiFault::runtime_defect(
                "gdiplus-compatibility-policy",
                "gdiplus-startup-profile-invalid",
                "GDI+ startup policy must match the evidenced x86 primary input",
            ));
        }
        Ok(())
    }
}

pub fn dispatch_gdiplus_api_with_policy(
    policy: &GdiplusCompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> ApiHandlerResult {
    if let Err(fault) = policy.validate() {
        return ApiHandlerResult::Fault(fault);
    }
    let result = match contract.implementation_id.as_str() {
        GDIPLUS_STARTUP_IMPLEMENTATION => handle_gdiplus_startup(policy, contract, frame, session),
        GDIPLUS_SHUTDOWN_IMPLEMENTATION => {
            handle_gdiplus_shutdown(policy, contract, frame, session)
        }
        implementation_id => Err(ApiFault::unsupported(
            "gdiplus-api-dispatch",
            "implementation-id-unsupported",
            format!("GDI+ implementation {implementation_id} is not registered"),
        )),
    };
    match result {
        Ok(immediate) => ApiHandlerResult::Immediate(immediate),
        Err(fault) => ApiHandlerResult::Fault(fault),
    }
}

fn handle_gdiplus_startup(
    policy: &GdiplusCompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let startup = policy.startup.as_ref().ok_or_else(|| {
        ApiFault::unsupported(
            "gdiplus-startup",
            "gdiplus-startup-policy-disabled",
            format!(
                "compatibility policy {} does not enable GDI+ startup",
                policy.id
            ),
        )
    })?;
    let token_output = GuestAddress(required_argument(frame, 0, "token output")?);
    let input_address = GuestAddress(required_argument(frame, 1, "startup input")?);
    let notification_output = required_argument(frame, 2, "startup notification output")?;
    let input = session
        .memory()
        .read(
            input_address,
            X86_GDIPLUS_STARTUP_INPUT_BYTE_LENGTH as usize,
        )
        .map_err(|error| ApiFault::from_memory(error, &contract.endpoint))?;
    let version = read_u32(&input, 0)?;
    let debug_event_callback = read_u32(&input, 4)?;
    let suppress_background_thread = read_u32(&input, 8)?;
    let suppress_external_codecs = read_u32(&input, 12)?;

    if version != startup.version {
        return Ok(ApiImmediate::new(ApiReturnValue::U32(
            GDIPLUS_STATUS_UNSUPPORTED_GDIPLUS_VERSION,
        )));
    }
    if debug_event_callback != startup.debug_event_callback
        || suppress_background_thread != u32::from(startup.suppress_background_thread)
        || suppress_external_codecs != u32::from(startup.suppress_external_codecs)
        || notification_output != startup.notification_output
    {
        return Err(ApiFault::unsupported(
            "gdiplus-startup",
            "gdiplus-startup-variant-unsupported",
            "GdiplusStartup currently supports only the evidenced callback-free primary input",
        ));
    }

    let request = RuntimeGdiplusStartupRequest {
        profile_id: startup.profile_id.clone(),
        version,
        debug_event_callback,
        suppress_background_thread: suppress_background_thread != 0,
        suppress_external_codecs: suppress_external_codecs != 0,
        notification_output_requested: notification_output != 0,
    };
    let (logical_session, token_write) = match session.start_gdiplus_session(request, token_output)
    {
        Ok(result) => result,
        Err(error) if error.class == RuntimeStateErrorClass::ResourceLimit => {
            return Ok(ApiImmediate::new(ApiReturnValue::U32(
                GDIPLUS_STATUS_OUT_OF_MEMORY,
            )));
        }
        Err(error) => return Err(ApiFault::from_runtime(error, &contract.endpoint)),
    };
    Ok(
        ApiImmediate::new(ApiReturnValue::U32(GDIPLUS_STATUS_OK)).with_committed_guest_write(
            ApiGuestWriteReceipt {
                write: ApiGuestWrite {
                    address: token_output,
                    bytes: logical_session.token.to_le_bytes().to_vec(),
                },
                result: token_write,
            },
        ),
    )
}

fn handle_gdiplus_shutdown(
    policy: &GdiplusCompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    if policy.startup.is_none() {
        return Err(ApiFault::unsupported(
            "gdiplus-shutdown",
            "gdiplus-shutdown-policy-disabled",
            format!(
                "compatibility policy {} does not enable GDI+ session lifetime",
                policy.id
            ),
        ));
    }
    let token = required_argument(frame, 0, "session token")?;
    session
        .shutdown_gdiplus_session(token)
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    Ok(ApiImmediate::new(ApiReturnValue::Void))
}

pub fn gdiplus_contract_for_endpoint(
    endpoint: ApiEndpointIdentity,
) -> Result<Option<ApiContract>, ApiFault> {
    if !endpoint.module.eq_ignore_ascii_case("gdiplus.dll") {
        return Ok(None);
    }
    let ApiSymbol::Name { name } = &endpoint.symbol else {
        return Ok(None);
    };
    match name.as_str() {
        "GdiplusStartup" => Ok(Some(gdiplus_startup_contract(endpoint)?)),
        "GdiplusShutdown" => Ok(Some(gdiplus_shutdown_contract(endpoint)?)),
        _ => Ok(None),
    }
}

pub fn gdiplus_startup_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    Ok(ApiContract {
        endpoint,
        abi: AbiSchema {
            calling_convention: CallingConvention::Stdcall,
            arguments: vec![
                output_struct("token", "ULONG_PTR", 4, false),
                input_struct(
                    "input",
                    "GdiplusStartupInput",
                    X86_GDIPLUS_STARTUP_INPUT_BYTE_LENGTH,
                ),
                output_struct(
                    "output",
                    "GdiplusStartupOutput",
                    X86_GDIPLUS_STARTUP_OUTPUT_BYTE_LENGTH,
                    true,
                ),
            ],
            return_value: ApiReturnSchema::U32,
            stack_cleanup: StackCleanup::Callee,
        },
        last_error_policy: LastErrorPolicy::Preserve,
        reentrancy_policy: ReentrancyPolicy::Disallowed,
        behavior: ApiBehavior::Immediate,
        callback_policy: CallbackPolicy::Forbidden,
        implementation_id: GDIPLUS_STARTUP_IMPLEMENTATION.to_owned(),
    })
}

pub fn gdiplus_shutdown_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    Ok(ApiContract {
        endpoint,
        abi: AbiSchema {
            calling_convention: CallingConvention::Stdcall,
            arguments: vec![ApiArgumentSchema {
                name: "token".to_owned(),
                direction: ApiDirection::In,
                value: ApiValueSchema::U32,
            }],
            return_value: ApiReturnSchema::Void,
            stack_cleanup: StackCleanup::Callee,
        },
        last_error_policy: LastErrorPolicy::Preserve,
        reentrancy_policy: ReentrancyPolicy::Disallowed,
        behavior: ApiBehavior::Immediate,
        callback_policy: CallbackPolicy::Forbidden,
        implementation_id: GDIPLUS_SHUTDOWN_IMPLEMENTATION.to_owned(),
    })
}

fn input_struct(name: &str, type_id: &str, byte_length: u32) -> ApiArgumentSchema {
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

fn output_struct(name: &str, type_id: &str, byte_length: u32, nullable: bool) -> ApiArgumentSchema {
    ApiArgumentSchema {
        name: name.to_owned(),
        direction: ApiDirection::Out,
        value: ApiValueSchema::Pointer {
            constraint: ApiPointerConstraint {
                nullable,
                alignment: 4,
                byte_length,
                pointee: ApiPointeeSchema::Struct {
                    type_id: type_id.to_owned(),
                },
                write_requirement: ApiWriteRequirement::Optional,
            },
        },
    }
}

fn required_argument(
    frame: &ApiCallFrame,
    index: usize,
    label: &'static str,
) -> Result<u32, ApiFault> {
    frame.argument_u32(index).ok_or_else(|| {
        ApiFault::runtime_defect(
            "gdiplus-api-dispatch",
            "decoded-argument-missing",
            format!("decoded GDI+ frame is missing {label}"),
        )
    })
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, ApiFault> {
    bytes
        .get(offset..offset + 4)
        .and_then(|value| value.try_into().ok())
        .map(u32::from_le_bytes)
        .ok_or_else(|| {
            ApiFault::runtime_defect(
                "gdiplus-startup",
                "gdiplus-startup-input-truncated",
                "preflighted GdiplusStartupInput bytes are truncated",
            )
        })
}

fn controlled_startup_policy() -> GdiplusStartupPolicy {
    GdiplusStartupPolicy {
        profile_id: "primary-x86-callback-free-v1".to_owned(),
        version: 1,
        debug_event_callback: 0,
        suppress_background_thread: false,
        suppress_external_codecs: false,
        notification_output: 0,
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
    fn controlled_policy_is_exact_and_mutated_profiles_are_rejected() {
        let policy = GdiplusCompatibilityPolicy::controlled_v1();
        policy.validate().unwrap();

        let mut mutated = policy.clone();
        mutated.startup.as_mut().unwrap().version = 2;
        assert_eq!(
            mutated.validate().unwrap_err().code,
            "gdiplus-startup-profile-invalid"
        );

        let mut disabled = policy;
        disabled.startup = None;
        disabled.validate().unwrap();
    }
}

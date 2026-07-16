use serde::Serialize;

use crate::api::{
    AbiSchema, ApiArgumentSchema, ApiBehavior, ApiCallFrame, ApiContract, ApiDirection,
    ApiDynamicPointerConstraint, ApiEndpointIdentity, ApiFault, ApiHandlerResult, ApiImmediate,
    ApiPointeeSchema, ApiReturnSchema, ApiReturnValue, ApiSymbol, ApiValueSchema,
    ApiWriteRequirement, CallbackPolicy, CallingConvention, ExecutionSession, LastErrorPolicy,
    ReentrancyPolicy, StackCleanup,
};
use crate::memory::GuestAddress;

pub const WINMM_COMPATIBILITY_POLICY_SCHEMA: &str = "vine.winmm-compatibility-policy";
pub const WINMM_COMPATIBILITY_POLICY_VERSION: u32 = 1;
pub const TIME_GET_DEV_CAPS_IMPLEMENTATION: &str = "winmm.time-get-dev-caps.v1";
pub const X86_TIMECAPS_BYTE_LENGTH: u32 = 8;
pub const TIMERR_NOERROR: u32 = 0;
pub const TIMERR_STRUCT: u32 = 97;

const MAX_POLICY_ID_BYTES: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WinmmTimeCapabilitiesPolicy {
    pub profile_id: String,
    pub period_minimum_milliseconds: u32,
    pub period_maximum_milliseconds: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WinmmCompatibilityPolicy {
    pub schema: String,
    pub schema_version: u32,
    pub id: String,
    pub time_get_dev_caps: Option<WinmmTimeCapabilitiesPolicy>,
}

impl WinmmCompatibilityPolicy {
    pub fn controlled_v1() -> Self {
        Self {
            schema: WINMM_COMPATIBILITY_POLICY_SCHEMA.to_owned(),
            schema_version: WINMM_COMPATIBILITY_POLICY_VERSION,
            id: "winmm-timecaps-1-1000000-v1".to_owned(),
            time_get_dev_caps: Some(WinmmTimeCapabilitiesPolicy {
                profile_id: "timecaps-1-1000000-v1".to_owned(),
                period_minimum_milliseconds: 1,
                period_maximum_milliseconds: 1_000_000,
            }),
        }
    }

    pub fn validate(&self) -> Result<(), ApiFault> {
        if self.schema != WINMM_COMPATIBILITY_POLICY_SCHEMA
            || self.schema_version != WINMM_COMPATIBILITY_POLICY_VERSION
            || !valid_policy_id(&self.id)
        {
            return Err(ApiFault::runtime_defect(
                "winmm-compatibility-policy",
                "winmm-policy-identity-invalid",
                "WinMM compatibility policy identity or schema is invalid",
            ));
        }
        if let Some(capabilities) = self.time_get_dev_caps.as_ref()
            && (!valid_policy_id(&capabilities.profile_id)
                || capabilities.period_minimum_milliseconds == 0
                || capabilities.period_maximum_milliseconds
                    < capabilities.period_minimum_milliseconds)
        {
            return Err(ApiFault::runtime_defect(
                "winmm-compatibility-policy",
                "winmm-time-capabilities-invalid",
                "WinMM timer capabilities require a bounded identity and an ordered nonzero period range",
            ));
        }
        Ok(())
    }
}

pub fn dispatch_winmm_api_with_policy(
    policy: &WinmmCompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    _session: &mut ExecutionSession<'_>,
) -> ApiHandlerResult {
    if let Err(fault) = policy.validate() {
        return ApiHandlerResult::Fault(fault);
    }
    let result = match contract.implementation_id.as_str() {
        TIME_GET_DEV_CAPS_IMPLEMENTATION => handle_time_get_dev_caps(policy, contract, frame),
        implementation_id => Err(ApiFault::unsupported(
            "winmm-api-dispatch",
            "implementation-id-unsupported",
            format!("WinMM implementation {implementation_id} is not registered"),
        )),
    };
    match result {
        Ok(immediate) => ApiHandlerResult::Immediate(immediate),
        Err(fault) => ApiHandlerResult::Fault(fault),
    }
}

fn handle_time_get_dev_caps(
    policy: &WinmmCompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
) -> Result<ApiImmediate, ApiFault> {
    let output = required_argument(contract, frame, 0, "TIMECAPS output")?;
    let size = required_argument(contract, frame, 1, "TIMECAPS byte length")?;
    let capabilities = policy.time_get_dev_caps.as_ref().ok_or_else(|| {
        ApiFault::unsupported(
            "winmm-timer-capabilities",
            "time-get-dev-caps-policy-disabled",
            format!(
                "compatibility policy {} does not enable timer capability queries",
                policy.id
            ),
        )
    })?;
    if size < X86_TIMECAPS_BYTE_LENGTH {
        return Ok(ApiImmediate::new(ApiReturnValue::U32(TIMERR_STRUCT)));
    }
    let mut bytes = Vec::with_capacity(X86_TIMECAPS_BYTE_LENGTH as usize);
    bytes.extend_from_slice(&capabilities.period_minimum_milliseconds.to_le_bytes());
    bytes.extend_from_slice(&capabilities.period_maximum_milliseconds.to_le_bytes());
    Ok(ApiImmediate::new(ApiReturnValue::U32(TIMERR_NOERROR))
        .with_guest_write(GuestAddress(output), bytes))
}

pub fn winmm_contract_for_endpoint(
    endpoint: ApiEndpointIdentity,
) -> Result<Option<ApiContract>, ApiFault> {
    if !endpoint.module.eq_ignore_ascii_case("winmm.dll") {
        return Ok(None);
    }
    let ApiSymbol::Name { name } = &endpoint.symbol else {
        return Ok(None);
    };
    match name.as_str() {
        "timeGetDevCaps" => Ok(Some(time_get_dev_caps_contract(endpoint)?)),
        _ => Ok(None),
    }
}

pub fn time_get_dev_caps_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    Ok(ApiContract {
        endpoint,
        abi: AbiSchema {
            calling_convention: CallingConvention::Stdcall,
            arguments: vec![
                ApiArgumentSchema {
                    name: "timeCaps".to_owned(),
                    direction: ApiDirection::Out,
                    value: ApiValueSchema::DynamicPointer {
                        constraint: ApiDynamicPointerConstraint {
                            nullable: false,
                            alignment: 4,
                            minimum_byte_length: 1,
                            maximum_byte_length: X86_TIMECAPS_BYTE_LENGTH,
                            pointee: ApiPointeeSchema::Struct {
                                type_id: "TIMECAPS".to_owned(),
                            },
                            write_requirement: ApiWriteRequirement::Optional,
                        },
                    },
                },
                u32_argument("byteLength"),
            ],
            return_value: ApiReturnSchema::U32,
            stack_cleanup: StackCleanup::Callee,
        },
        last_error_policy: LastErrorPolicy::Preserve,
        reentrancy_policy: ReentrancyPolicy::Disallowed,
        behavior: ApiBehavior::Immediate,
        callback_policy: CallbackPolicy::Forbidden,
        implementation_id: TIME_GET_DEV_CAPS_IMPLEMENTATION.to_owned(),
    })
}

fn required_argument(
    contract: &ApiContract,
    frame: &ApiCallFrame,
    index: usize,
    label: &str,
) -> Result<u32, ApiFault> {
    frame.argument_u32(index).ok_or_else(|| {
        ApiFault::runtime_defect(
            "winmm-api-dispatch",
            "decoded-argument-missing",
            format!(
                "decoded {} frame is missing {label} argument {index}",
                contract.implementation_id
            ),
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

fn valid_policy_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_POLICY_ID_BYTES
        && value.is_ascii()
        && !value.as_bytes().contains(&0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn controlled_policy_is_bounded_and_host_independent() {
        let policy = WinmmCompatibilityPolicy::controlled_v1();
        policy.validate().unwrap();
        let capabilities = policy.time_get_dev_caps.as_ref().unwrap();
        assert_eq!(capabilities.period_minimum_milliseconds, 1);
        assert_eq!(capabilities.period_maximum_milliseconds, 1_000_000);

        let mut invalid = policy;
        invalid
            .time_get_dev_caps
            .as_mut()
            .unwrap()
            .period_maximum_milliseconds = 0;
        assert_eq!(
            invalid.validate().unwrap_err().code,
            "winmm-time-capabilities-invalid"
        );
    }
}

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
pub const TIME_BEGIN_PERIOD_IMPLEMENTATION: &str = "winmm.time-begin-period.v1";
pub const TIME_END_PERIOD_IMPLEMENTATION: &str = "winmm.time-end-period.v1";
pub const X86_TIMECAPS_BYTE_LENGTH: u32 = 8;
pub const TIMERR_NOERROR: u32 = 0;
pub const TIMERR_NOCANDO: u32 = 97;

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
pub struct WinmmTimerResolutionPolicy {
    pub profile_id: String,
    pub allowed_periods_milliseconds: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WinmmCompatibilityPolicy {
    pub schema: String,
    pub schema_version: u32,
    pub id: String,
    pub time_get_dev_caps: Option<WinmmTimeCapabilitiesPolicy>,
    pub timer_resolution: Option<WinmmTimerResolutionPolicy>,
}

impl WinmmCompatibilityPolicy {
    pub fn controlled_v1() -> Self {
        Self {
            schema: WINMM_COMPATIBILITY_POLICY_SCHEMA.to_owned(),
            schema_version: WINMM_COMPATIBILITY_POLICY_VERSION,
            id: "winmm-timecaps-1-1000000-period-1-v1".to_owned(),
            time_get_dev_caps: Some(WinmmTimeCapabilitiesPolicy {
                profile_id: "timecaps-1-1000000-v1".to_owned(),
                period_minimum_milliseconds: 1,
                period_maximum_milliseconds: 1_000_000,
            }),
            timer_resolution: Some(WinmmTimerResolutionPolicy {
                profile_id: "timer-resolution-period-1-v1".to_owned(),
                allowed_periods_milliseconds: vec![1],
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
        if let Some(timer_resolution) = self.timer_resolution.as_ref() {
            let Some(capabilities) = self.time_get_dev_caps.as_ref() else {
                return Err(ApiFault::runtime_defect(
                    "winmm-compatibility-policy",
                    "winmm-timer-resolution-capabilities-missing",
                    "WinMM timer resolution policy requires a timer capability profile",
                ));
            };
            if !valid_policy_id(&timer_resolution.profile_id)
                || timer_resolution.allowed_periods_milliseconds.is_empty()
                || timer_resolution.allowed_periods_milliseconds.len()
                    > crate::runtime::RUNTIME_TIMER_RESOLUTION_DISTINCT_PERIOD_LIMIT
                || timer_resolution
                    .allowed_periods_milliseconds
                    .iter()
                    .any(|&period| {
                        period < capabilities.period_minimum_milliseconds
                            || period > capabilities.period_maximum_milliseconds
                    })
                || timer_resolution
                    .allowed_periods_milliseconds
                    .windows(2)
                    .any(|periods| periods[0] >= periods[1])
            {
                return Err(ApiFault::runtime_defect(
                    "winmm-compatibility-policy",
                    "winmm-timer-resolution-policy-invalid",
                    "WinMM timer resolution policy requires bounded sorted unique periods within the advertised capability profile",
                ));
            }
        }
        Ok(())
    }
}

pub fn dispatch_winmm_api_with_policy(
    policy: &WinmmCompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> ApiHandlerResult {
    if let Err(fault) = policy.validate() {
        return ApiHandlerResult::Fault(fault);
    }
    let result = match contract.implementation_id.as_str() {
        TIME_GET_DEV_CAPS_IMPLEMENTATION => handle_time_get_dev_caps(policy, contract, frame),
        TIME_BEGIN_PERIOD_IMPLEMENTATION => {
            handle_time_begin_period(policy, contract, frame, session)
        }
        TIME_END_PERIOD_IMPLEMENTATION => handle_time_end_period(policy, contract, frame, session),
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
        return Ok(ApiImmediate::new(ApiReturnValue::U32(TIMERR_NOCANDO)));
    }
    let mut bytes = Vec::with_capacity(X86_TIMECAPS_BYTE_LENGTH as usize);
    bytes.extend_from_slice(&capabilities.period_minimum_milliseconds.to_le_bytes());
    bytes.extend_from_slice(&capabilities.period_maximum_milliseconds.to_le_bytes());
    Ok(ApiImmediate::new(ApiReturnValue::U32(TIMERR_NOERROR))
        .with_guest_write(GuestAddress(output), bytes))
}

fn handle_time_begin_period(
    policy: &WinmmCompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let period = required_argument(contract, frame, 0, "timer resolution period")?;
    let timer_resolution = timer_resolution_policy(policy)?;
    if timer_resolution
        .allowed_periods_milliseconds
        .binary_search(&period)
        .is_err()
    {
        return Ok(ApiImmediate::new(ApiReturnValue::U32(TIMERR_NOCANDO)));
    }
    session
        .runtime_mut()
        .begin_timer_period(period)
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    Ok(ApiImmediate::new(ApiReturnValue::U32(TIMERR_NOERROR)))
}

fn handle_time_end_period(
    policy: &WinmmCompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let period = required_argument(contract, frame, 0, "timer resolution period")?;
    let timer_resolution = timer_resolution_policy(policy)?;
    if timer_resolution
        .allowed_periods_milliseconds
        .binary_search(&period)
        .is_err()
    {
        return Ok(ApiImmediate::new(ApiReturnValue::U32(TIMERR_NOCANDO)));
    }
    let result = session
        .runtime_mut()
        .end_timer_period(period)
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    Ok(ApiImmediate::new(ApiReturnValue::U32(
        if result.is_some() {
            TIMERR_NOERROR
        } else {
            TIMERR_NOCANDO
        },
    )))
}

fn timer_resolution_policy(
    policy: &WinmmCompatibilityPolicy,
) -> Result<&WinmmTimerResolutionPolicy, ApiFault> {
    policy.timer_resolution.as_ref().ok_or_else(|| {
        ApiFault::unsupported(
            "winmm-timer-resolution",
            "timer-resolution-policy-disabled",
            format!(
                "compatibility policy {} does not enable timer resolution requests",
                policy.id
            ),
        )
    })
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
        "timeBeginPeriod" => Ok(Some(time_begin_period_contract(endpoint)?)),
        "timeEndPeriod" => Ok(Some(time_end_period_contract(endpoint)?)),
        _ => Ok(None),
    }
}

pub fn time_begin_period_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    time_period_contract(endpoint, TIME_BEGIN_PERIOD_IMPLEMENTATION)
}

pub fn time_end_period_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    time_period_contract(endpoint, TIME_END_PERIOD_IMPLEMENTATION)
}

fn time_period_contract(
    endpoint: ApiEndpointIdentity,
    implementation_id: &str,
) -> Result<ApiContract, ApiFault> {
    Ok(ApiContract {
        endpoint,
        abi: AbiSchema {
            calling_convention: CallingConvention::Stdcall,
            arguments: vec![u32_argument("periodMilliseconds")],
            return_value: ApiReturnSchema::U32,
            stack_cleanup: StackCleanup::Callee,
        },
        last_error_policy: LastErrorPolicy::Preserve,
        reentrancy_policy: ReentrancyPolicy::Allowed,
        behavior: ApiBehavior::Immediate,
        callback_policy: CallbackPolicy::Forbidden,
        implementation_id: implementation_id.to_owned(),
    })
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
        assert_eq!(
            policy
                .timer_resolution
                .as_ref()
                .unwrap()
                .allowed_periods_milliseconds,
            [1]
        );

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

        let mut invalid = WinmmCompatibilityPolicy::controlled_v1();
        invalid
            .timer_resolution
            .as_mut()
            .unwrap()
            .allowed_periods_milliseconds = vec![2, 1];
        assert_eq!(
            invalid.validate().unwrap_err().code,
            "winmm-timer-resolution-policy-invalid"
        );
    }
}

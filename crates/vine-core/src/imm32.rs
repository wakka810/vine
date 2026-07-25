use serde::Serialize;

use crate::api::{
    AbiSchema, ApiArgumentSchema, ApiBehavior, ApiCallFrame, ApiContract, ApiDirection,
    ApiEndpointIdentity, ApiFault, ApiHandlerResult, ApiImmediate, ApiReturnSchema, ApiReturnValue,
    ApiSymbol, ApiValueSchema, CallbackPolicy, CallingConvention, ExecutionSession,
    LastErrorPolicy, ReentrancyPolicy, StackCleanup,
};
use crate::runtime::{
    LOGICAL_IME_CONTEXT_HANDLE_NAMESPACE, LOGICAL_WINDOW_HANDLE_NAMESPACE, RuntimeStateErrorClass,
};

pub const IMM32_COMPATIBILITY_POLICY_SCHEMA: &str = "vine.imm32-compatibility-policy";
pub const IMM32_COMPATIBILITY_POLICY_VERSION: u32 = 1;
pub const IMM_GET_DEFAULT_IME_WND_IMPLEMENTATION: &str =
    "imm32.get-default-ime-window.thread-logical-v1";
pub const IMM_ASSOCIATE_CONTEXT_IMPLEMENTATION: &str = "imm32.associate-context.logical-window-v1";

const ERROR_SUCCESS: u32 = 0;
const ERROR_INVALID_WINDOW_HANDLE: u32 = 1400;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Imm32GetDefaultImeWindowPolicy {
    pub profile_id: String,
    pub null_window_result: u32,
    pub invalid_window_last_error: u32,
    pub require_same_thread_identity: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Imm32AssociateContextPolicy {
    pub profile_id: String,
    pub allow_null_detach: bool,
    pub allow_runtime_owned_restore: bool,
    pub require_materialized_default_context: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Imm32CompatibilityPolicy {
    pub schema: String,
    pub schema_version: u32,
    pub id: String,
    pub get_default_ime_window: Option<Imm32GetDefaultImeWindowPolicy>,
    pub associate_context: Option<Imm32AssociateContextPolicy>,
}

impl Imm32CompatibilityPolicy {
    pub fn controlled_v1() -> Self {
        Self {
            schema: IMM32_COMPATIBILITY_POLICY_SCHEMA.to_owned(),
            schema_version: IMM32_COMPATIBILITY_POLICY_VERSION,
            id: "imm32-primary-default-ime-window-context-v1".to_owned(),
            get_default_ime_window: Some(controlled_get_default_ime_window_policy()),
            associate_context: Some(controlled_associate_context_policy()),
        }
    }

    pub fn validate(&self) -> Result<(), ApiFault> {
        if self.schema != IMM32_COMPATIBILITY_POLICY_SCHEMA
            || self.schema_version != IMM32_COMPATIBILITY_POLICY_VERSION
            || !valid_policy_id(&self.id)
        {
            return Err(ApiFault::runtime_defect(
                "imm32-compatibility-policy",
                "imm32-policy-header-invalid",
                "IMM32 compatibility policy has an unsupported schema, version, or ID",
            ));
        }
        if let Some(profile) = self.get_default_ime_window.as_ref()
            && profile != &controlled_get_default_ime_window_policy()
        {
            return Err(ApiFault::runtime_defect(
                "imm32-compatibility-policy",
                "imm32-get-default-ime-window-profile-invalid",
                "IMM32 default IME window policy must match the evidenced same-thread query",
            ));
        }
        if let Some(profile) = self.associate_context.as_ref()
            && (profile != &controlled_associate_context_policy()
                || self.get_default_ime_window.is_none())
        {
            return Err(ApiFault::runtime_defect(
                "imm32-compatibility-policy",
                "imm32-associate-context-profile-invalid",
                "IMM32 context association policy requires the evidenced default-context query",
            ));
        }
        Ok(())
    }
}

pub fn dispatch_imm32_api_with_policy(
    policy: &Imm32CompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> ApiHandlerResult {
    if let Err(fault) = policy.validate() {
        return ApiHandlerResult::Fault(fault);
    }
    let result = match contract.implementation_id.as_str() {
        IMM_GET_DEFAULT_IME_WND_IMPLEMENTATION => {
            handle_get_default_ime_window(policy, contract, frame, session)
        }
        IMM_ASSOCIATE_CONTEXT_IMPLEMENTATION => {
            handle_associate_context(policy, contract, frame, session)
        }
        implementation_id => Err(ApiFault::unsupported(
            "imm32-api-dispatch",
            "implementation-id-unsupported",
            format!("IMM32 implementation {implementation_id} is not registered"),
        )),
    };
    match result {
        Ok(immediate) => ApiHandlerResult::Immediate(immediate),
        Err(fault) => ApiHandlerResult::Fault(fault),
    }
}

fn handle_get_default_ime_window(
    policy: &Imm32CompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let profile = policy.get_default_ime_window.as_ref().ok_or_else(|| {
        ApiFault::unsupported(
            "imm32-default-ime-window",
            "imm32-get-default-ime-window-policy-disabled",
            format!(
                "compatibility policy {} does not enable default IME window lookup",
                policy.id
            ),
        )
    })?;
    let window_handle = required_argument(frame, 0, "window handle")?;
    if window_handle == 0 {
        return Ok(logical_window_result(
            profile.null_window_result,
            ERROR_SUCCESS,
        ));
    }
    match session.runtime_mut().get_default_ime_window(window_handle) {
        Ok(result) => Ok(logical_window_result(result.handle, ERROR_SUCCESS)),
        Err(error) if error.class == RuntimeStateErrorClass::NotFound => {
            Ok(logical_window_result(0, profile.invalid_window_last_error))
        }
        Err(error) => Err(ApiFault::from_runtime(error, &contract.endpoint)),
    }
}

fn handle_associate_context(
    policy: &Imm32CompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let profile = policy.associate_context.as_ref().ok_or_else(|| {
        ApiFault::unsupported(
            "imm32-context-association",
            "imm32-associate-context-policy-disabled",
            format!(
                "compatibility policy {} does not enable logical IME context association",
                policy.id
            ),
        )
    })?;
    let window_handle = required_argument(frame, 0, "window handle")?;
    let context_handle = required_argument(frame, 1, "IME context")?;
    if context_handle == 0 && !profile.allow_null_detach {
        return Err(ApiFault::unsupported(
            "imm32-context-association",
            "imm32-context-detach-disabled",
            "ImmAssociateContext NULL detach is disabled by the selected policy",
        ));
    }
    if context_handle != 0 && !profile.allow_runtime_owned_restore {
        return Err(ApiFault::unsupported(
            "imm32-context-association",
            "imm32-context-restore-disabled",
            "ImmAssociateContext logical context restore is disabled by the selected policy",
        ));
    }
    let result = session
        .runtime_mut()
        .associate_ime_context(window_handle, context_handle)
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    Ok(ApiImmediate::new(ApiReturnValue::LogicalHandle {
        namespace: LOGICAL_IME_CONTEXT_HANDLE_NAMESPACE.to_owned(),
        value: result.previous_context_handle,
    }))
}

pub fn imm32_contract_for_endpoint(
    endpoint: ApiEndpointIdentity,
) -> Result<Option<ApiContract>, ApiFault> {
    if !endpoint.module.eq_ignore_ascii_case("imm32.dll") {
        return Ok(None);
    }
    let ApiSymbol::Name { name } = &endpoint.symbol else {
        return Ok(None);
    };
    match name.as_str() {
        "ImmGetDefaultIMEWnd" => Ok(Some(imm_get_default_ime_wnd_contract(endpoint)?)),
        "ImmAssociateContext" => Ok(Some(imm_associate_context_contract(endpoint)?)),
        _ => Ok(None),
    }
}

pub fn imm_get_default_ime_wnd_contract(
    endpoint: ApiEndpointIdentity,
) -> Result<ApiContract, ApiFault> {
    Ok(ApiContract {
        endpoint,
        abi: AbiSchema {
            calling_convention: CallingConvention::Stdcall,
            arguments: vec![logical_handle_argument(
                "window",
                LOGICAL_WINDOW_HANDLE_NAMESPACE,
                true,
            )],
            return_value: ApiReturnSchema::LogicalHandle {
                namespace: LOGICAL_WINDOW_HANDLE_NAMESPACE.to_owned(),
                nullable: true,
            },
            stack_cleanup: StackCleanup::Callee,
        },
        last_error_policy: LastErrorPolicy::FromHandler,
        reentrancy_policy: ReentrancyPolicy::Allowed,
        behavior: ApiBehavior::Immediate,
        callback_policy: CallbackPolicy::Forbidden,
        implementation_id: IMM_GET_DEFAULT_IME_WND_IMPLEMENTATION.to_owned(),
    })
}

pub fn imm_associate_context_contract(
    endpoint: ApiEndpointIdentity,
) -> Result<ApiContract, ApiFault> {
    Ok(ApiContract {
        endpoint,
        abi: AbiSchema {
            calling_convention: CallingConvention::Stdcall,
            arguments: vec![
                logical_handle_argument("window", LOGICAL_WINDOW_HANDLE_NAMESPACE, false),
                logical_handle_argument("context", LOGICAL_IME_CONTEXT_HANDLE_NAMESPACE, true),
            ],
            return_value: ApiReturnSchema::LogicalHandle {
                namespace: LOGICAL_IME_CONTEXT_HANDLE_NAMESPACE.to_owned(),
                nullable: true,
            },
            stack_cleanup: StackCleanup::Callee,
        },
        last_error_policy: LastErrorPolicy::Preserve,
        reentrancy_policy: ReentrancyPolicy::Allowed,
        behavior: ApiBehavior::Immediate,
        callback_policy: CallbackPolicy::Forbidden,
        implementation_id: IMM_ASSOCIATE_CONTEXT_IMPLEMENTATION.to_owned(),
    })
}

fn logical_window_result(value: u32, last_error: u32) -> ApiImmediate {
    ApiImmediate::new(ApiReturnValue::LogicalHandle {
        namespace: LOGICAL_WINDOW_HANDLE_NAMESPACE.to_owned(),
        value,
    })
    .with_last_error(last_error)
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

fn required_argument(
    frame: &ApiCallFrame,
    index: usize,
    label: &'static str,
) -> Result<u32, ApiFault> {
    frame.argument_u32(index).ok_or_else(|| {
        ApiFault::runtime_defect(
            "imm32-api-dispatch",
            "decoded-argument-missing",
            format!("decoded IMM32 frame is missing {label}"),
        )
    })
}

fn controlled_get_default_ime_window_policy() -> Imm32GetDefaultImeWindowPolicy {
    Imm32GetDefaultImeWindowPolicy {
        profile_id: "same-thread-ime-class-null-invalid-oracle-v1".to_owned(),
        null_window_result: 0,
        invalid_window_last_error: ERROR_INVALID_WINDOW_HANDLE,
        require_same_thread_identity: true,
    }
}

fn controlled_associate_context_policy() -> Imm32AssociateContextPolicy {
    Imm32AssociateContextPolicy {
        profile_id: "default-context-null-detach-runtime-restore-v1".to_owned(),
        allow_null_detach: true,
        allow_runtime_owned_restore: true,
        require_materialized_default_context: true,
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
        let policy = Imm32CompatibilityPolicy::controlled_v1();
        policy.validate().unwrap();

        let mut invalid_query = policy.clone();
        invalid_query
            .get_default_ime_window
            .as_mut()
            .unwrap()
            .invalid_window_last_error = 0;
        assert_eq!(
            invalid_query.validate().unwrap_err().code,
            "imm32-get-default-ime-window-profile-invalid"
        );

        let mut invalid_association = policy;
        invalid_association
            .associate_context
            .as_mut()
            .unwrap()
            .allow_null_detach = false;
        assert_eq!(
            invalid_association.validate().unwrap_err().code,
            "imm32-associate-context-profile-invalid"
        );
    }
}

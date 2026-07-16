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
use crate::runtime::{RuntimeDisplayProfile, RuntimeInputProfile, RuntimeRect};

pub const USER32_COMPATIBILITY_POLICY_SCHEMA: &str = "vine.user32-compatibility-policy";
pub const USER32_COMPATIBILITY_POLICY_VERSION: u32 = 2;
pub const ENUM_DISPLAY_MONITORS_IMPLEMENTATION: &str = "user32.enum-display-monitors.v1";
pub const SWAP_MOUSE_BUTTON_IMPLEMENTATION: &str = "user32.swap-mouse-button.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct User32CompatibilityPolicy {
    pub schema: String,
    pub schema_version: u32,
    pub id: String,
    pub enum_display_monitors: Option<RuntimeDisplayProfile>,
    pub swap_mouse_button: Option<RuntimeInputProfile>,
}

impl User32CompatibilityPolicy {
    pub fn controlled_v2() -> Self {
        Self {
            schema: USER32_COMPATIBILITY_POLICY_SCHEMA.to_owned(),
            schema_version: USER32_COMPATIBILITY_POLICY_VERSION,
            id: "user32-single-monitor-normal-mouse-v2".to_owned(),
            enum_display_monitors: Some(RuntimeDisplayProfile {
                id: "single-monitor-1920x1080-v1".to_owned(),
                primary_monitor_rect: RuntimeRect {
                    left: 0,
                    top: 0,
                    right: 1920,
                    bottom: 1080,
                },
            }),
            swap_mouse_button: Some(RuntimeInputProfile {
                id: "normal-mouse-buttons-v1".to_owned(),
                initial_mouse_buttons_swapped: false,
            }),
        }
    }

    pub fn validate(&self) -> Result<(), ApiFault> {
        if self.schema != USER32_COMPATIBILITY_POLICY_SCHEMA
            || self.schema_version != USER32_COMPATIBILITY_POLICY_VERSION
            || self.id.is_empty()
            || self.id.len() > 128
            || !self.id.is_ascii()
            || self.id.as_bytes().contains(&0)
        {
            return Err(ApiFault::runtime_defect(
                "user32-compatibility-policy",
                "user32-policy-identity-invalid",
                "User32 compatibility policy identity or schema is invalid",
            ));
        }
        if let Some(profile) = self.enum_display_monitors.as_ref() {
            let rect = profile.primary_monitor_rect;
            if profile.id.is_empty()
                || profile.id.len() > 128
                || !profile.id.is_ascii()
                || profile.id.as_bytes().contains(&0)
                || rect.left != 0
                || rect.top != 0
                || rect.right <= 0
                || rect.bottom <= 0
                || rect.right > 32_768
                || rect.bottom > 32_768
            {
                return Err(ApiFault::runtime_defect(
                    "user32-compatibility-policy",
                    "user32-monitor-profile-invalid",
                    "User32 single-monitor profile requires a bounded positive rectangle at the virtual origin",
                ));
            }
        }
        if let Some(profile) = self.swap_mouse_button.as_ref()
            && (profile.id.is_empty()
                || profile.id.len() > 128
                || !profile.id.is_ascii()
                || profile.id.as_bytes().contains(&0))
        {
            return Err(ApiFault::runtime_defect(
                "user32-compatibility-policy",
                "user32-input-profile-invalid",
                "User32 logical input profile requires a bounded nonempty ASCII identity",
            ));
        }
        Ok(())
    }

    pub fn display_profile(&self) -> Option<RuntimeDisplayProfile> {
        self.enum_display_monitors.clone()
    }

    pub fn input_profile(&self) -> Option<RuntimeInputProfile> {
        self.swap_mouse_button.clone()
    }
}

pub fn dispatch_user32_api_with_policy(
    policy: &User32CompatibilityPolicy,
    callback_return_sentinel: GuestAddress,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> ApiHandlerResult {
    if let Err(fault) = policy.validate() {
        return ApiHandlerResult::Fault(fault);
    }
    match contract.implementation_id.as_str() {
        ENUM_DISPLAY_MONITORS_IMPLEMENTATION => {
            handle_enum_display_monitors(policy, callback_return_sentinel, contract, frame, session)
        }
        SWAP_MOUSE_BUTTON_IMPLEMENTATION => {
            match handle_swap_mouse_button(policy, contract, frame, session) {
                Ok(immediate) => ApiHandlerResult::Immediate(immediate),
                Err(fault) => ApiHandlerResult::Fault(fault),
            }
        }
        implementation_id => ApiHandlerResult::Fault(ApiFault::unsupported(
            "user32-api-dispatch",
            "implementation-id-unsupported",
            format!("User32 implementation {implementation_id} is not registered"),
        )),
    }
}

fn handle_swap_mouse_button(
    policy: &User32CompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let profile = policy.swap_mouse_button.as_ref().ok_or_else(|| {
        ApiFault::unsupported(
            "user32-input",
            "swap-mouse-button-policy-disabled",
            format!(
                "compatibility policy {} does not enable logical mouse button mapping",
                policy.id
            ),
        )
    })?;
    let swapped = required_argument(contract, frame, 0, "mouse button swap flag")? != 0;
    let mapping = session.runtime().mouse_button_mapping().ok_or_else(|| {
        ApiFault::runtime_defect(
            "user32-input",
            "logical-mouse-button-profile-missing",
            "User32 policy is enabled but the Rust input subsystem has no mouse button profile",
        )
    })?;
    if mapping.profile_id != profile.id
        || mapping.initial_swapped != profile.initial_mouse_buttons_swapped
    {
        return Err(ApiFault::runtime_defect(
            "user32-input",
            "logical-mouse-button-profile-mismatch",
            "Rust input authority does not match the selected User32 compatibility profile",
        ));
    }
    let result = session
        .runtime_mut()
        .swap_mouse_buttons(swapped)
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    Ok(ApiImmediate::new(ApiReturnValue::Bool32(
        result.previous_swapped,
    )))
}

fn handle_enum_display_monitors(
    policy: &User32CompatibilityPolicy,
    callback_return_sentinel: GuestAddress,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &ExecutionSession<'_>,
) -> ApiHandlerResult {
    let Some(profile) = policy.enum_display_monitors.as_ref() else {
        return ApiHandlerResult::Fault(ApiFault::unsupported(
            "user32-display",
            "enum-display-monitors-policy-disabled",
            format!(
                "compatibility policy {} does not enable logical monitor enumeration",
                policy.id
            ),
        ));
    };
    let hdc = match required_argument(contract, frame, 0, "HDC") {
        Ok(value) => value,
        Err(fault) => return ApiHandlerResult::Fault(fault),
    };
    let clip = match required_argument(contract, frame, 1, "clip rectangle") {
        Ok(value) => value,
        Err(fault) => return ApiHandlerResult::Fault(fault),
    };
    let callback = match required_argument(contract, frame, 2, "monitor callback") {
        Ok(value) => value,
        Err(fault) => return ApiHandlerResult::Fault(fault),
    };
    let data = match required_argument(contract, frame, 3, "callback data") {
        Ok(value) => value,
        Err(fault) => return ApiHandlerResult::Fault(fault),
    };
    if hdc != 0 || clip != 0 {
        return ApiHandlerResult::Fault(ApiFault::unsupported(
            "user32-display",
            "enum-display-monitors-variant-unsupported",
            "EnumDisplayMonitors currently supports only NULL HDC and NULL clip rectangle",
        ));
    }
    if let Err(error) =
        session
            .memory()
            .validate_access(GuestAddress(callback), 1, MemoryAccess::Execute)
    {
        return ApiHandlerResult::Fault(ApiFault::from_memory(error, &contract.endpoint));
    }
    let Some(monitor) = session.runtime().primary_monitor() else {
        return ApiHandlerResult::Fault(ApiFault::runtime_defect(
            "user32-display",
            "logical-monitor-profile-missing",
            "User32 policy is enabled but the Rust display subsystem has no logical monitor",
        ));
    };
    if session.runtime().display_profile_id() != Some(profile.id.as_str())
        || monitor.rect != profile.primary_monitor_rect
    {
        return ApiHandlerResult::Fault(ApiFault::runtime_defect(
            "user32-display",
            "logical-monitor-profile-mismatch",
            "Rust display authority does not match the selected User32 compatibility profile",
        ));
    }

    ApiHandlerResult::GuestCallback(GuestCallbackRequest {
        target: GuestAddress(callback),
        arguments: vec![
            GuestCallbackArgument::U32(monitor.handle),
            GuestCallbackArgument::U32(0),
            GuestCallbackArgument::ScratchPointer { offset: 0 },
            GuestCallbackArgument::U32(data),
        ],
        scratch: monitor.rect.to_x86_bytes(),
        return_sentinel: callback_return_sentinel,
        completion_policy: GuestCallbackCompletionPolicy::ReturnBool32,
    })
}

pub fn user32_contract_for_endpoint(
    endpoint: ApiEndpointIdentity,
) -> Result<Option<ApiContract>, ApiFault> {
    if !endpoint.module.eq_ignore_ascii_case("user32.dll") {
        return Ok(None);
    }
    let ApiSymbol::Name { name } = &endpoint.symbol else {
        return Ok(None);
    };
    match name.as_str() {
        "EnumDisplayMonitors" => Ok(Some(enum_display_monitors_contract(endpoint)?)),
        "SwapMouseButton" => Ok(Some(swap_mouse_button_contract(endpoint)?)),
        _ => Ok(None),
    }
}

pub fn swap_mouse_button_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    Ok(ApiContract {
        endpoint,
        abi: AbiSchema {
            calling_convention: CallingConvention::Stdcall,
            arguments: vec![u32_argument("swap")],
            return_value: ApiReturnSchema::Bool32,
            stack_cleanup: StackCleanup::Callee,
        },
        last_error_policy: LastErrorPolicy::Preserve,
        reentrancy_policy: ReentrancyPolicy::Allowed,
        behavior: ApiBehavior::Immediate,
        callback_policy: CallbackPolicy::Forbidden,
        implementation_id: SWAP_MOUSE_BUTTON_IMPLEMENTATION.to_owned(),
    })
}

pub fn enum_display_monitors_contract(
    endpoint: ApiEndpointIdentity,
) -> Result<ApiContract, ApiFault> {
    let contract = ApiContract {
        endpoint,
        abi: AbiSchema {
            calling_convention: CallingConvention::Stdcall,
            arguments: vec![
                u32_argument("hdc"),
                optional_in_struct("clipRect", "RECT", 16),
                opaque_pointer("monitorCallback", false, 1),
                u32_argument("data"),
            ],
            return_value: ApiReturnSchema::Bool32,
            stack_cleanup: StackCleanup::Callee,
        },
        last_error_policy: LastErrorPolicy::Preserve,
        reentrancy_policy: ReentrancyPolicy::Allowed,
        behavior: ApiBehavior::Immediate,
        callback_policy: CallbackPolicy::Required,
        implementation_id: ENUM_DISPLAY_MONITORS_IMPLEMENTATION.to_owned(),
    };
    Ok(contract)
}

fn required_argument(
    contract: &ApiContract,
    frame: &ApiCallFrame,
    index: usize,
    label: &str,
) -> Result<u32, ApiFault> {
    frame.argument_u32(index).ok_or_else(|| {
        ApiFault::runtime_defect(
            "user32-api-dispatch",
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

fn opaque_pointer(name: &str, nullable: bool, alignment: u32) -> ApiArgumentSchema {
    ApiArgumentSchema {
        name: name.to_owned(),
        direction: ApiDirection::In,
        value: ApiValueSchema::OpaquePointer {
            nullable,
            alignment,
        },
    }
}

fn optional_in_struct(name: &str, type_id: &str, byte_length: u32) -> ApiArgumentSchema {
    ApiArgumentSchema {
        name: name.to_owned(),
        direction: ApiDirection::In,
        value: ApiValueSchema::Pointer {
            constraint: ApiPointerConstraint {
                nullable: true,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn controlled_policy_is_host_independent_and_bounded() {
        let policy = User32CompatibilityPolicy::controlled_v2();
        policy.validate().unwrap();
        let profile = policy.display_profile().unwrap();
        assert_eq!(profile.primary_monitor_rect.right, 1920);
        assert_eq!(profile.primary_monitor_rect.bottom, 1080);

        let mut invalid = policy;
        invalid
            .enum_display_monitors
            .as_mut()
            .unwrap()
            .primary_monitor_rect
            .right = 0;
        assert_eq!(
            invalid.validate().unwrap_err().code,
            "user32-monitor-profile-invalid"
        );

        let mut invalid_input = User32CompatibilityPolicy::controlled_v2();
        invalid_input.swap_mouse_button.as_mut().unwrap().id.clear();
        assert_eq!(
            invalid_input.validate().unwrap_err().code,
            "user32-input-profile-invalid"
        );
    }
}

use crate::api::{
    AbiSchema, ApiArgumentSchema, ApiBehavior, ApiCallFrame, ApiContract, ApiDirection,
    ApiEndpointIdentity, ApiFault, ApiHandlerResult, ApiImmediate, ApiReturnSchema, ApiReturnValue,
    ApiSymbol, ApiValueSchema, CallbackPolicy, CallingConvention, ExecutionSession,
    LastErrorPolicy, ReentrancyPolicy, StackCleanup,
};
use crate::runtime::LOGICAL_WINDOW_HANDLE_NAMESPACE;

pub const DRAG_ACCEPT_FILES_IMPLEMENTATION: &str =
    "shell32.drag-accept-files.logical-window-style-v1";

const ERROR_SUCCESS: u32 = 0;

pub fn dispatch_shell32_api(
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> ApiHandlerResult {
    let result = match contract.implementation_id.as_str() {
        DRAG_ACCEPT_FILES_IMPLEMENTATION => handle_drag_accept_files(contract, frame, session),
        implementation_id => Err(ApiFault::unsupported(
            "shell32-api-dispatch",
            "implementation-id-unsupported",
            format!("Shell32 implementation {implementation_id} is not registered"),
        )),
    };
    match result {
        Ok(immediate) => ApiHandlerResult::Immediate(immediate),
        Err(fault) => ApiHandlerResult::Fault(fault),
    }
}

fn handle_drag_accept_files(
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let window_handle = required_argument(contract, frame, 0, "window handle")?;
    let accepts_files = required_argument(contract, frame, 1, "accept flag")? != 0;
    session
        .runtime_mut()
        .set_window_file_drop_acceptance(window_handle, accepts_files)
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    Ok(ApiImmediate::new(ApiReturnValue::Void).with_last_error(ERROR_SUCCESS))
}

pub fn shell32_contract_for_endpoint(
    endpoint: ApiEndpointIdentity,
) -> Result<Option<ApiContract>, ApiFault> {
    if !endpoint.module.eq_ignore_ascii_case("shell32.dll") {
        return Ok(None);
    }
    let ApiSymbol::Name { name } = &endpoint.symbol else {
        return Ok(None);
    };
    match name.as_str() {
        "DragAcceptFiles" => Ok(Some(drag_accept_files_contract(endpoint)?)),
        _ => Ok(None),
    }
}

pub fn drag_accept_files_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    Ok(ApiContract {
        endpoint,
        abi: AbiSchema {
            calling_convention: CallingConvention::Stdcall,
            arguments: vec![
                logical_window_handle_argument("window"),
                u32_argument("accept"),
            ],
            return_value: ApiReturnSchema::Void,
            stack_cleanup: StackCleanup::Callee,
        },
        last_error_policy: LastErrorPolicy::FromHandler,
        reentrancy_policy: ReentrancyPolicy::Allowed,
        behavior: ApiBehavior::Immediate,
        callback_policy: CallbackPolicy::Forbidden,
        implementation_id: DRAG_ACCEPT_FILES_IMPLEMENTATION.to_owned(),
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
            "shell32-api-dispatch",
            "decoded-argument-missing",
            format!(
                "decoded {} frame is missing {label} argument {index}",
                contract.implementation_id
            ),
        )
    })
}

fn logical_window_handle_argument(name: &str) -> ApiArgumentSchema {
    ApiArgumentSchema {
        name: name.to_owned(),
        direction: ApiDirection::In,
        value: ApiValueSchema::LogicalHandle {
            namespace: LOGICAL_WINDOW_HANDLE_NAMESPACE.to_owned(),
            nullable: false,
        },
    }
}

fn u32_argument(name: &str) -> ApiArgumentSchema {
    ApiArgumentSchema {
        name: name.to_owned(),
        direction: ApiDirection::In,
        value: ApiValueSchema::U32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::RuntimeDynamicEndpoint;

    #[test]
    fn drag_accept_files_contract_is_void_stdcall_with_handler_owned_last_error() {
        let endpoint = ApiEndpointIdentity::from_dynamic_endpoint(&RuntimeDynamicEndpoint {
            namespace: "vine.test-endpoint.v1".to_owned(),
            id: 1,
            address: crate::memory::GuestAddress(0x7f00_1000),
            module: "SHELL32.dll".to_owned(),
            name: "DragAcceptFiles".to_owned(),
            lifetime: "test".to_owned(),
        });
        let contract = shell32_contract_for_endpoint(endpoint)
            .unwrap()
            .expect("DragAcceptFiles contract");
        assert_eq!(contract.abi.calling_convention, CallingConvention::Stdcall);
        assert_eq!(contract.abi.arguments.len(), 2);
        assert_eq!(contract.abi.return_value, ApiReturnSchema::Void);
        assert_eq!(contract.abi.stack_cleanup, StackCleanup::Callee);
        assert_eq!(contract.last_error_policy, LastErrorPolicy::FromHandler);
        assert_eq!(contract.implementation_id, DRAG_ACCEPT_FILES_IMPLEMENTATION);
    }
}

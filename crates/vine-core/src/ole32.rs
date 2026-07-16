use crate::api::{
    AbiSchema, ApiArgumentSchema, ApiBehavior, ApiCallFrame, ApiContract, ApiDirection,
    ApiEndpointIdentity, ApiFault, ApiHandlerResult, ApiImmediate, ApiReturnSchema, ApiReturnValue,
    ApiSymbol, ApiValueSchema, CallbackPolicy, CallingConvention, ExecutionSession,
    LastErrorPolicy, ReentrancyPolicy, StackCleanup,
};
use crate::com::{COM_RPC_E_CHANGED_MODE, COM_S_FALSE, COM_S_OK};
use crate::runtime::{RuntimeComApartmentModel, RuntimeComInitializationDisposition};

pub const CO_INITIALIZE_IMPLEMENTATION: &str = "ole32.co-initialize.v1";

pub fn dispatch_ole32_api(
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> ApiHandlerResult {
    match contract.implementation_id.as_str() {
        CO_INITIALIZE_IMPLEMENTATION => handle_co_initialize(contract, frame, session),
        implementation_id => ApiHandlerResult::Fault(ApiFault::unsupported(
            "ole32-api-dispatch",
            "implementation-id-unsupported",
            format!("Ole32 implementation {implementation_id} is not registered"),
        )),
    }
}

fn handle_co_initialize(
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> ApiHandlerResult {
    let reserved = match frame.argument_u32(0) {
        Some(value) => value,
        None => {
            return ApiHandlerResult::Fault(ApiFault::runtime_defect(
                "ole32-api-dispatch",
                "decoded-argument-missing",
                format!(
                    "decoded {} frame is missing reserved argument 0",
                    contract.implementation_id
                ),
            ));
        }
    };
    if reserved != 0 {
        return ApiHandlerResult::Fault(ApiFault::unsupported(
            "ole32-com-initialization",
            "co-initialize-reserved-unsupported",
            "CoInitialize currently accepts only a NULL reserved pointer",
        ));
    }
    let outcome = match session
        .runtime_mut()
        .initialize_current_com_apartment(RuntimeComApartmentModel::SingleThreaded)
    {
        Ok(outcome) => outcome,
        Err(error) => {
            return ApiHandlerResult::Fault(ApiFault::from_runtime(error, &contract.endpoint));
        }
    };
    let result = match outcome.disposition {
        RuntimeComInitializationDisposition::Initialized => COM_S_OK,
        RuntimeComInitializationDisposition::AlreadyInitialized => COM_S_FALSE,
        RuntimeComInitializationDisposition::ChangedMode => COM_RPC_E_CHANGED_MODE,
    };
    ApiHandlerResult::Immediate(ApiImmediate::new(ApiReturnValue::HResult(result)))
}

pub fn ole32_contract_for_endpoint(
    endpoint: ApiEndpointIdentity,
) -> Result<Option<ApiContract>, ApiFault> {
    if !endpoint.module.eq_ignore_ascii_case("ole32.dll") {
        return Ok(None);
    }
    let ApiSymbol::Name { name } = &endpoint.symbol else {
        return Ok(None);
    };
    match name.as_str() {
        "CoInitialize" => Ok(Some(co_initialize_contract(endpoint)?)),
        _ => Ok(None),
    }
}

pub fn co_initialize_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    Ok(ApiContract {
        endpoint,
        abi: AbiSchema {
            calling_convention: CallingConvention::Stdcall,
            arguments: vec![ApiArgumentSchema {
                name: "reserved".to_owned(),
                direction: ApiDirection::In,
                value: ApiValueSchema::OpaquePointer {
                    nullable: true,
                    alignment: 1,
                },
            }],
            return_value: ApiReturnSchema::HResult,
            stack_cleanup: StackCleanup::Callee,
        },
        last_error_policy: LastErrorPolicy::Preserve,
        reentrancy_policy: ReentrancyPolicy::Disallowed,
        behavior: ApiBehavior::Immediate,
        callback_policy: CallbackPolicy::Forbidden,
        implementation_id: CO_INITIALIZE_IMPLEMENTATION.to_owned(),
    })
}

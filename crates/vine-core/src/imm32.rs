use crate::api::{
    ApiCallFrame, ApiEndpointIdentity, ApiFault, ApiHandlerResult, ApiImmediate, ApiSymbol,
};
const ERROR_SUCCESS: u32 = 0;
const ERROR_INVALID_WINDOW_HANDLE: u32 = 1400;

pub(crate) fn dispatch_imm32_api(
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> ApiHandlerResult {
    let endpoint = &frame.endpoint;
    if !endpoint.module.eq_ignore_ascii_case("imm32.dll") {
        return ApiHandlerResult::Fault(ApiFault::new("IMM32 dispatch requires an IMM32 endpoint"));
    }
    let result = match &endpoint.symbol {
        ApiSymbol::Name { name } if name == "ImmGetDefaultIMEWnd" => {
            handle_get_default_ime_window(endpoint, frame, runtime)
        }
        ApiSymbol::Name { name } if name == "ImmAssociateContext" => {
            handle_associate_context(endpoint, frame, runtime)
        }
        symbol => Err(ApiFault::new(format!(
            "IMM32 endpoint {symbol:?} is not implemented"
        ))),
    };
    match result {
        Ok(immediate) => ApiHandlerResult::Immediate(immediate),
        Err(fault) => ApiHandlerResult::Fault(fault),
    }
}

fn handle_get_default_ime_window(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let window_handle = required_argument(frame, 0, "window handle")?;
    if window_handle == 0 {
        return Ok(logical_window_result(0, ERROR_SUCCESS));
    }
    match runtime
        .get_default_ime_window(window_handle)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?
    {
        Some(handle) => Ok(logical_window_result(handle, ERROR_SUCCESS)),
        None => Ok(logical_window_result(0, ERROR_INVALID_WINDOW_HANDLE)),
    }
}

fn handle_associate_context(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let window_handle = required_argument(frame, 0, "window handle")?;
    let context_handle = required_argument(frame, 1, "IME context")?;
    if context_handle != 0 {
        let window = runtime
            .window(window_handle)
            .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
        let context = runtime
            .ime_context(context_handle)
            .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
        if window.creation.owner_thread_id != runtime.current_thread_id()
            || context.owner_thread_id != runtime.current_thread_id()
        {
            return Ok(ApiImmediate::new(Some(0)));
        }
    }
    let previous = runtime
        .associate_ime_context(window_handle, context_handle)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    Ok(ApiImmediate::new(Some(previous)))
}

pub(crate) fn imm32_argument_words(endpoint: &ApiEndpointIdentity) -> Option<u8> {
    if !endpoint.module.eq_ignore_ascii_case("imm32.dll") {
        return None;
    }
    let ApiSymbol::Name { name } = &endpoint.symbol else {
        return None;
    };
    match name.as_str() {
        "ImmGetDefaultIMEWnd" => Some(1),
        "ImmAssociateContext" => Some(2),
        _ => None,
    }
}

fn logical_window_result(value: u32, last_error: u32) -> ApiImmediate {
    ApiImmediate::new(Some(value)).with_last_error(last_error)
}

fn required_argument(
    frame: &ApiCallFrame,
    index: usize,
    label: &'static str,
) -> Result<u32, ApiFault> {
    frame
        .argument_u32(index)
        .ok_or_else(|| ApiFault::new(format!("decoded IMM32 frame is missing {label}")))
}

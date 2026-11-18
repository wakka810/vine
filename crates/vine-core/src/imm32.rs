use crate::api::{
    ApiCallFrame, ApiEndpointIdentity, ApiFault, ApiHandlerResult, ApiImmediate, ApiSymbol,
};
use crate::memory::{GuestAddress, MemoryAccess};
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
        ApiSymbol::Name { name } if name == "ImmIsIME" => {
            required_argument(frame, 0, "keyboard layout").map(|layout| {
                ApiImmediate::new(Some(u32::from(
                    layout == crate::runtime::RUNTIME_KEYBOARD_LAYOUT,
                )))
            })
        }
        ApiSymbol::Name { name } if name == "ImmGetDefaultIMEWnd" => {
            handle_get_default_ime_window(endpoint, frame, runtime)
        }
        ApiSymbol::Name { name } if name == "ImmAssociateContext" => {
            handle_associate_context(endpoint, frame, runtime)
        }
        ApiSymbol::Name { name } if name == "ImmGetContext" => {
            let handle = required_argument(frame, 0, "window handle");
            handle.and_then(|handle| {
                runtime
                    .get_ime_context(handle)
                    .map(|context| ApiImmediate::new(Some(context.unwrap_or(0))))
                    .map_err(|error| ApiFault::from_runtime(error, endpoint))
            })
        }
        ApiSymbol::Name { name } if name == "ImmReleaseContext" => {
            // HIMC is owned by the thread, not a borrowed memory pointer. Releasing
            // access leaves its identity and association alive, including repeat releases.
            Ok(ApiImmediate::new(Some(1)))
        }
        ApiSymbol::Name { name }
            if matches!(
                name.as_str(),
                "ImmGetOpenStatus" | "ImmSetOpenStatus" | "ImmGetConversionStatus"
            ) =>
        {
            handle_context_status(endpoint, frame, runtime, name)
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

fn handle_context_status(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &crate::runtime::Runtime,
    name: &str,
) -> Result<ApiImmediate, ApiFault> {
    let handle = required_argument(frame, 0, "IME context")?;
    let Ok(context) = runtime.ime_context(handle) else {
        let result = ApiImmediate::new(Some(0));
        return Ok(if handle == 0 && name == "ImmGetOpenStatus" {
            result
        } else {
            result.with_last_error(6)
        });
    };
    if context.owner_thread_id != runtime.current_thread_id() {
        return Err(ApiFault::new(
            "IME status access requires the context owner thread",
        ));
    }
    match name {
        "ImmGetOpenStatus" => Ok(ApiImmediate::new(Some(u32::from(context.open)))),
        "ImmSetOpenStatus" => {
            if required_argument(frame, 1, "open status")? != 0 || context.open {
                return Err(ApiFault::new("opening IME composition is not implemented"));
            }
            Ok(ApiImmediate::new(Some(1)))
        }
        "ImmGetConversionStatus" => {
            let mut result = ApiImmediate::new(Some(1));
            for (index, value) in [(1, context.conversion_mode), (2, context.sentence_mode)] {
                let address = GuestAddress(required_argument(frame, index, "IME mode output")?);
                if address.0 != 0 {
                    runtime
                        .memory
                        .validate_access(address, 4, MemoryAccess::Write)
                        .map_err(|error| ApiFault::from_memory(error, endpoint))?;
                    result = result.with_guest_write(address, value.to_le_bytes().to_vec());
                }
            }
            Ok(result)
        }
        _ => unreachable!(),
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
        "ImmGetDefaultIMEWnd" | "ImmGetContext" | "ImmGetOpenStatus" | "ImmIsIME" => Some(1),
        "ImmAssociateContext" | "ImmReleaseContext" | "ImmSetOpenStatus" => Some(2),
        "ImmGetConversionStatus" => Some(3),
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

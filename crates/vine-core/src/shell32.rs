use crate::api::{
    ApiCallFrame, ApiEndpointIdentity, ApiFault, ApiHandlerResult, ApiImmediate, ApiSymbol,
};

const ERROR_SUCCESS: u32 = 0;

pub(crate) fn dispatch_shell32_api(
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> ApiHandlerResult {
    let endpoint = &frame.endpoint;
    let result = match &endpoint.symbol {
        ApiSymbol::Name { name }
            if name == "DragAcceptFiles" && endpoint.module.eq_ignore_ascii_case("shell32.dll") =>
        {
            handle_drag_accept_files(endpoint, frame, runtime)
        }
        symbol => Err(ApiFault::new(format!(
            "unsupported Shell32 import {symbol:?}"
        ))),
    };
    match result {
        Ok(immediate) => ApiHandlerResult::Immediate(immediate),
        Err(fault) => ApiHandlerResult::Fault(fault),
    }
}

fn handle_drag_accept_files(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let window_handle = required_argument(endpoint, frame, 0, "window handle")?;
    let accepts_files = required_argument(endpoint, frame, 1, "accept flag")? != 0;
    runtime
        .set_window_file_drop_acceptance(window_handle, accepts_files)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    Ok(ApiImmediate::new(None).with_last_error(ERROR_SUCCESS))
}

pub(crate) fn shell32_argument_words(endpoint: &ApiEndpointIdentity) -> Option<u8> {
    if !endpoint.module.eq_ignore_ascii_case("shell32.dll") {
        return None;
    }
    let ApiSymbol::Name { name } = &endpoint.symbol else {
        return None;
    };
    match name.as_str() {
        "DragAcceptFiles" => Some(2),
        _ => None,
    }
}

fn required_argument(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    index: usize,
    label: &str,
) -> Result<u32, ApiFault> {
    frame.argument_u32(index).ok_or_else(|| {
        ApiFault::new(format!(
            "decoded {:?} frame is missing {label} argument {index}",
            endpoint.symbol
        ))
    })
}

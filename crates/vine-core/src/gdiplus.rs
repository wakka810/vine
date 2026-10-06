use crate::api::{
    ApiCallFrame, ApiEndpointIdentity, ApiFault, ApiHandlerResult, ApiImmediate, ApiSymbol,
};
use crate::memory::GuestAddress;

pub const X86_GDIPLUS_STARTUP_INPUT_BYTE_LENGTH: u32 = 16;

pub const GDIPLUS_STATUS_OK: u32 = 0;
pub const GDIPLUS_STATUS_OUT_OF_MEMORY: u32 = 3;
pub const GDIPLUS_STATUS_UNSUPPORTED_GDIPLUS_VERSION: u32 = 17;

pub(crate) fn dispatch_gdiplus_api(
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> ApiHandlerResult {
    let endpoint = &frame.endpoint;
    let result = match &endpoint.symbol {
        ApiSymbol::Name { name } if endpoint.module.eq_ignore_ascii_case("gdiplus.dll") => {
            match name.as_str() {
                "GdiplusStartup" => handle_gdiplus_startup(endpoint, frame, runtime),
                "GdiplusShutdown" => handle_gdiplus_shutdown(endpoint, frame, runtime),
                _ => Err(ApiFault::new(format!(
                    "GDI+ endpoint {name} is not implemented"
                ))),
            }
        }
        _ => Err(ApiFault::new(format!(
            "GDI+ endpoint {:?} is not implemented",
            endpoint
        ))),
    };
    match result {
        Ok(immediate) => ApiHandlerResult::Immediate(immediate),
        Err(fault) => ApiHandlerResult::Fault(fault),
    }
}

fn handle_gdiplus_startup(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let token_output = GuestAddress(required_argument(frame, 0, "token output")?);
    let input_address = GuestAddress(required_argument(frame, 1, "startup input")?);
    let notification_output = required_argument(frame, 2, "startup notification output")?;
    let input = runtime
        .memory
        .read(
            input_address,
            X86_GDIPLUS_STARTUP_INPUT_BYTE_LENGTH as usize,
        )
        .map_err(|error| ApiFault::from_memory(error, endpoint))?;
    let version = read_u32(&input, 0)?;
    let debug_event_callback = read_u32(&input, 4)?;
    let suppress_background_thread = read_u32(&input, 8)?;
    let suppress_external_codecs = read_u32(&input, 12)?;

    if version != 1 {
        return Ok(ApiImmediate::new(Some(
            GDIPLUS_STATUS_UNSUPPORTED_GDIPLUS_VERSION,
        )));
    }
    if debug_event_callback != 0
        || suppress_background_thread != 0
        || suppress_external_codecs != 0
        || notification_output != 0
    {
        return Err(ApiFault::new(
            "GdiplusStartup supports version 1 without debug callbacks, suppression flags or notification output",
        ));
    }

    let status = runtime
        .start_gdiplus_session(token_output)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    Ok(ApiImmediate::new(Some(status)))
}

fn handle_gdiplus_shutdown(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> Result<ApiImmediate, ApiFault> {
    let token = required_argument(frame, 0, "session token")?;
    runtime
        .shutdown_gdiplus_session(token)
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    Ok(ApiImmediate::new(None))
}

pub(crate) fn gdiplus_argument_words(endpoint: &ApiEndpointIdentity) -> Option<u8> {
    if !endpoint.module.eq_ignore_ascii_case("gdiplus.dll") {
        return None;
    }
    let ApiSymbol::Name { name } = &endpoint.symbol else {
        return None;
    };
    match name.as_str() {
        "GdiplusStartup" => Some(3),
        "GdiplusShutdown" => Some(1),
        _ => None,
    }
}

fn required_argument(
    frame: &ApiCallFrame,
    index: usize,
    label: &'static str,
) -> Result<u32, ApiFault> {
    frame
        .argument_u32(index)
        .ok_or_else(|| ApiFault::new(format!("decoded GDI+ frame is missing {label}")))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, ApiFault> {
    bytes
        .get(offset..offset + 4)
        .and_then(|value| value.try_into().ok())
        .map(u32::from_le_bytes)
        .ok_or_else(|| ApiFault::new("preflighted GdiplusStartupInput bytes are truncated"))
}

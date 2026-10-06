use crate::api::{
    ApiCallFrame, ApiEndpointIdentity, ApiFault, ApiHandlerResult, ApiImmediate, ApiSymbol,
};
use crate::memory::GuestAddress;

pub const X86_TIMECAPS_BYTE_LENGTH: u32 = 8;
pub const TIMERR_NOERROR: u32 = 0;
pub const TIMERR_NOCANDO: u32 = 97;

const TIMER_PERIOD_MINIMUM: u32 = 1;
const TIMER_PERIOD_MAXIMUM: u32 = 1_000_000;

pub(crate) fn dispatch_winmm_api(
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> ApiHandlerResult {
    let endpoint = &frame.endpoint;
    let result = match &endpoint.symbol {
        ApiSymbol::Name { name } if endpoint.module.eq_ignore_ascii_case("winmm.dll") => {
            match name.as_str() {
                "timeGetDevCaps" => handle_time_get_dev_caps(endpoint, frame),
                // Clock progression belongs to execution scheduling, not API reads.
                "timeGetTime" => Ok(ApiImmediate::new(Some(runtime.clock().tick_count))),
                "timeBeginPeriod" => handle_time_period(endpoint, frame, runtime, true),
                "timeEndPeriod" => handle_time_period(endpoint, frame, runtime, false),
                _ => Err(ApiFault::new(format!(
                    "unsupported imported function {}!{name}",
                    endpoint.module
                ))),
            }
        }
        _ => Err(ApiFault::new(format!(
            "unsupported imported function {}!{:?}",
            endpoint.module, endpoint.symbol
        ))),
    };
    match result {
        Ok(immediate) => ApiHandlerResult::Immediate(immediate),
        Err(fault) => ApiHandlerResult::Fault(fault),
    }
}

fn handle_time_period(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
    begin: bool,
) -> Result<ApiImmediate, ApiFault> {
    let period = required_argument(endpoint, frame, 0, "timer resolution period")?;
    if !(TIMER_PERIOD_MINIMUM..=TIMER_PERIOD_MAXIMUM).contains(&period) {
        return Ok(ApiImmediate::new(Some(TIMERR_NOCANDO)));
    }
    let result = if begin {
        runtime.begin_timer_period(period)
    } else {
        runtime.end_timer_period(period)
    }
    .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    Ok(ApiImmediate::new(Some(result)))
}

fn handle_time_get_dev_caps(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
) -> Result<ApiImmediate, ApiFault> {
    let output = required_argument(endpoint, frame, 0, "TIMECAPS output")?;
    let size = required_argument(endpoint, frame, 1, "TIMECAPS byte length")?;
    if size < X86_TIMECAPS_BYTE_LENGTH {
        return Ok(ApiImmediate::new(Some(TIMERR_NOCANDO)));
    }
    let mut bytes = Vec::with_capacity(X86_TIMECAPS_BYTE_LENGTH as usize);
    bytes.extend_from_slice(&TIMER_PERIOD_MINIMUM.to_le_bytes());
    bytes.extend_from_slice(&TIMER_PERIOD_MAXIMUM.to_le_bytes());
    Ok(ApiImmediate::new(Some(TIMERR_NOERROR)).with_guest_write(GuestAddress(output), bytes))
}

pub(crate) fn winmm_argument_words(endpoint: &ApiEndpointIdentity) -> Option<u8> {
    if !endpoint.module.eq_ignore_ascii_case("winmm.dll") {
        return None;
    }
    let ApiSymbol::Name { name } = &endpoint.symbol else {
        return None;
    };
    match name.as_str() {
        "timeGetTime" => Some(0),
        "timeBeginPeriod" => Some(1),
        "timeEndPeriod" => Some(1),
        "timeGetDevCaps" => Some(2),
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

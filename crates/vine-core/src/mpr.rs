use crate::api::{
    ApiCallFrame, ApiEndpointIdentity, ApiFault, ApiHandlerResult, ApiImmediate, ApiSymbol,
};
use crate::memory::GuestAddress;
use crate::runtime::Runtime;

pub(crate) fn mpr_argument_words(endpoint: &ApiEndpointIdentity) -> Option<u8> {
    match &endpoint.symbol {
        ApiSymbol::Name { name } if name == "WNetGetUniversalNameW" => Some(4),
        _ => None,
    }
}

pub(crate) fn dispatch_mpr_api(frame: &ApiCallFrame, runtime: &Runtime) -> ApiHandlerResult {
    let result = (|| {
        if mpr_argument_words(&frame.endpoint).is_none() {
            return Err(ApiFault::new("unsupported MPR import"));
        }
        let arg = |index| {
            frame
                .argument_u32(index)
                .ok_or_else(|| ApiFault::new("MPR argument is missing"))
        };
        let path = crate::win32::read_utf16_nul(
            &frame.endpoint,
            &runtime.memory,
            GuestAddress(arg(0)?),
            32768,
        )?;
        let level = arg(1)?;
        if !matches!(level, 1 | 2) {
            return Ok(ApiImmediate::new(Some(124)).with_last_error(124));
        }
        runtime
            .memory
            .read(GuestAddress(arg(3)?), 4)
            .map_err(|error| ApiFault::from_memory(error, &frame.endpoint))?;
        let bytes = path.as_bytes();
        let error = if bytes.len() < 2 || !bytes[0].is_ascii_alphabetic() || bytes[1] != b':' {
            1200
        } else {
            2250
        };
        // The fixed Windows environment has local drives and no redirected devices.
        Ok(ApiImmediate::new(Some(error)).with_last_error(error))
    })();
    match result {
        Ok(immediate) => ApiHandlerResult::Immediate(immediate),
        Err(fault) => ApiHandlerResult::Fault(fault),
    }
}

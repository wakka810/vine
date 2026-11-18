use crate::api::{
    ApiCallFrame, ApiEndpointIdentity, ApiFault, ApiHandlerResult, ApiImmediate, ApiSymbol,
};
use crate::memory::{GuestAddress, MemoryAccess};
use crate::runtime::Runtime;

pub(crate) fn advapi32_argument_words(endpoint: &ApiEndpointIdentity) -> Option<u8> {
    match &endpoint.symbol {
        ApiSymbol::Name { name } => match name.as_str() {
            "RegOpenKeyExW" => Some(5),
            "RegQueryValueExW" => Some(6),
            "RegCloseKey" => Some(1),
            _ => None,
        },
        _ => None,
    }
}

pub(crate) fn dispatch_advapi32_api(
    frame: &ApiCallFrame,
    runtime: &mut Runtime,
) -> ApiHandlerResult {
    let result = (|| {
        let ApiSymbol::Name { name } = &frame.endpoint.symbol else {
            return Err(ApiFault::new("unsupported ADVAPI32 ordinal"));
        };
        let arg = |index| {
            frame
                .argument_u32(index)
                .ok_or_else(|| ApiFault::new("ADVAPI32 argument missing"))
        };
        let key = arg(0)?;
        let wide = |address| {
            if address == 0 {
                Ok(String::new())
            } else {
                crate::win32::read_utf16_nul(
                    &frame.endpoint,
                    &runtime.memory,
                    GuestAddress(address),
                    32768,
                )
            }
        };
        let output = |address| {
            runtime
                .memory
                .validate_access(GuestAddress(address), 4, MemoryAccess::Write)
                .map_err(|e| ApiFault::from_memory(e, &frame.endpoint))
        };
        match name.as_str() {
            "RegOpenKeyExW" => {
                let subkey = wide(arg(1)?)?;
                if arg(2)? != 0 {
                    return Err(ApiFault::new("registry link options are not implemented"));
                }
                let access = arg(3)?;
                if access & 0x300 != 0 {
                    return Err(ApiFault::new(
                        "alternate WOW64 registry views are not implemented",
                    ));
                }
                let address = GuestAddress(arg(4)?);
                output(address.0)?;
                let (status, handle) = match runtime.registry.open(key, &subkey, access) {
                    Ok(handle) => (0, handle),
                    Err(status) => (status, 0),
                };
                Ok(ApiImmediate::new(Some(status))
                    .with_guest_write(address, handle.to_le_bytes().to_vec()))
            }
            "RegQueryValueExW" => {
                let _name = wide(arg(1)?)?;
                if arg(2)? != 0 || (arg(4)? != 0 && arg(5)? == 0) {
                    return Ok(ApiImmediate::new(Some(87)));
                }
                let status = runtime.registry.query(key).err().unwrap_or(0);
                let mut result = ApiImmediate::new(Some(status));
                if status == 2 {
                    // Native missing-value queries clear these two optional
                    // outputs and leave the caller's data buffer untouched.
                    for address in [arg(3)?, arg(5)?].into_iter().filter(|a| *a != 0) {
                        output(address)?;
                        result = result
                            .with_guest_write(GuestAddress(address), 0_u32.to_le_bytes().to_vec());
                    }
                }
                Ok(result)
            }
            "RegCloseKey" => Ok(ApiImmediate::new(Some(runtime.registry.close(key)))),
            _ => Err(ApiFault::new("unsupported ADVAPI32 import")),
        }
    })();
    match result {
        Ok(result) => ApiHandlerResult::Immediate(result),
        Err(fault) => ApiHandlerResult::Fault(fault),
    }
}

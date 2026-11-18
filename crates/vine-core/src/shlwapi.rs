use crate::api::{
    ApiCallFrame, ApiEndpointIdentity, ApiFault, ApiHandlerResult, ApiImmediate, ApiSymbol,
};
use crate::runtime::Runtime;

pub(crate) fn shlwapi_argument_words(endpoint: &ApiEndpointIdentity) -> Option<u8> {
    match &endpoint.symbol {
        ApiSymbol::Name { name }
            if matches!(name.as_str(), "PathFileExistsW" | "PathIsDirectoryW") =>
        {
            Some(1)
        }
        _ => None,
    }
}

pub(crate) fn dispatch_shlwapi_api(
    frame: &ApiCallFrame,
    runtime: &mut Runtime,
) -> ApiHandlerResult {
    let ApiSymbol::Name { name } = &frame.endpoint.symbol else {
        return ApiHandlerResult::Fault(ApiFault::new("unsupported Shlwapi ordinal"));
    };
    if !matches!(name.as_str(), "PathFileExistsW" | "PathIsDirectoryW") {
        return ApiHandlerResult::Fault(ApiFault::new("unsupported Shlwapi import"));
    }
    if frame.argument_u32(0) == Some(0) {
        return ApiHandlerResult::Immediate(ApiImmediate::new(Some(0)));
    }
    match crate::win32::handle_get_file_attributes_a_opfs(&frame.endpoint, frame, runtime) {
        ApiHandlerResult::Immediate(attributes) => ApiHandlerResult::Immediate(
            crate::win32::path_query_result(attributes, name == "PathIsDirectoryW"),
        ),
        other => other,
    }
}

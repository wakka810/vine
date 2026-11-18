use crate::api::{
    ApiCallFrame, ApiDeferredCall, ApiEndpointIdentity, ApiFault, ApiHandlerResult, ApiImmediate,
    ApiSymbol,
};
use crate::memory::{GuestAddress, MemoryAccess};
use crate::runtime::Runtime;

const ERROR_SUCCESS: u32 = 0;

pub(crate) fn dispatch_shell32_api(
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> ApiHandlerResult {
    let endpoint = &frame.endpoint;
    if matches!(&endpoint.symbol, ApiSymbol::Name { name } if name == "SHGetSpecialFolderPathW") {
        return match begin_special_folder(frame, runtime) {
            Ok(result) => result,
            Err(fault) => ApiHandlerResult::Fault(fault),
        };
    }
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
        "SHGetSpecialFolderPathW" => Some(4),
        _ => None,
    }
}

fn special_folder(
    frame: &ApiCallFrame,
    runtime: &Runtime,
) -> Result<(GuestAddress, Vec<u8>, String), ApiFault> {
    let endpoint = &frame.endpoint;
    let output = GuestAddress(required_argument(endpoint, frame, 1, "folder path output")?);
    let relative = match required_argument(endpoint, frame, 2, "CSIDL")? {
        5 => "Documents",
        0x1a => "AppData/Roaming",
        _ => return Err(ApiFault::new("unsupported shell special folder")),
    };
    let mut path = runtime
        .source_root_path_a()
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
    path.pop();
    if path.last() != Some(&b'\\') {
        path.push(b'\\');
    }
    path.extend_from_slice(relative.replace('/', "\\").as_bytes());
    path.push(0);
    if matches!(
        runtime.resolve_file_system_path_a(&path),
        Ok(crate::RuntimePathResolution::RegularFile)
    ) {
        return Err(ApiFault::new(
            "special folder conflicts with an immutable source file",
        ));
    }
    let relative = runtime
        .opfs_relative_path(&path)
        .map_err(|error| ApiFault::new(format!("special folder cannot map to OPFS: {error}")))?;
    let wide = crate::win32::decode_windows_932(&path)
        .into_iter()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    if wide.len() > 520 {
        return Err(ApiFault::new("special folder exceeds MAX_PATH"));
    }
    runtime
        .memory
        .validate_access(output, 520, MemoryAccess::Write)
        .map_err(|error| ApiFault::from_memory(error, endpoint))?;
    Ok((output, wide, relative))
}

fn begin_special_folder(
    frame: &ApiCallFrame,
    runtime: &mut Runtime,
) -> Result<ApiHandlerResult, ApiFault> {
    let (output, _, relative) = special_folder(frame, runtime)?;
    // Provision the fixed Windows folders lazily in this game's durable tree.
    // Both fCreate variants find these default folders once provisioning finishes.
    let request = runtime
        .begin_shell_folder(output)
        .map_err(|error| ApiFault::from_runtime(error, &frame.endpoint))?;
    let mut payload = vec![relative.bytes().filter(|&byte| byte == b'/').count() as u8];
    payload.extend_from_slice(relative.as_bytes());
    Ok(ApiHandlerResult::Deferred(crate::api::HostRequest {
        request_id: request.request_id,
        service: request.service,
        payload,
    }))
}

pub fn complete_shell32_special_folder(
    call: &ApiDeferredCall,
    observation: u32,
    runtime: &Runtime,
) -> Result<ApiImmediate, ApiFault> {
    if call.request.service != "browser.opfs-ensure-directory"
        || !matches!(&call.frame.endpoint.symbol, ApiSymbol::Name { name } if name == "SHGetSpecialFolderPathW")
    {
        return Err(ApiFault::new(
            "special folder completion does not identify its request",
        ));
    }
    let (output, wide, relative) = special_folder(&call.frame, runtime)?;
    runtime
        .validate_shell_folder_output(call.request.request_id, output)
        .map_err(|error| ApiFault::from_runtime(error, &call.frame.endpoint))?;
    let mut expected = vec![relative.bytes().filter(|&byte| byte == b'/').count() as u8];
    expected.extend_from_slice(relative.as_bytes());
    if call.request.payload != expected {
        return Err(ApiFault::new(
            "special folder payload differs from the live call",
        ));
    }
    match observation {
        0 => Ok(ApiImmediate::new(Some(1)).with_guest_write(output, wide)),
        2 => Ok(ApiImmediate::new(Some(0)).with_last_error(3)),
        3 => Ok(ApiImmediate::new(Some(0)).with_last_error(5)),
        4 => Ok(ApiImmediate::new(Some(0)).with_last_error(112)),
        _ => Err(ApiFault::new("invalid OPFS directory provisioning result")),
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

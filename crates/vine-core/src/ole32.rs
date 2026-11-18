use crate::api::{
    ApiCallFrame, ApiEndpointIdentity, ApiFault, ApiHandlerResult, ApiImmediate, ApiSymbol,
};
use crate::memory::{GuestAddress, MemoryAccess};
use crate::runtime::RuntimeComApartmentModel;
use crate::runtime::RuntimeComIid;

pub(crate) fn dispatch_ole32_api(
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> ApiHandlerResult {
    let endpoint = &frame.endpoint;
    if endpoint.module.eq_ignore_ascii_case("ole32.dll")
        && let ApiSymbol::Name { name } = &endpoint.symbol
    {
        match name.as_str() {
            "CoInitialize" => return handle_co_initialize(endpoint, frame, runtime),
            "CoUninitialize" => return handle_co_uninitialize(endpoint, frame, runtime),
            "CoCreateGuid" => {
                let result = (|| {
                    let [output] = frame.arguments.as_slice() else {
                        return Err(ApiFault::new("CoCreateGuid requires one argument"));
                    };
                    let output = GuestAddress(*output);
                    runtime
                        .memory
                        .validate_access(output, 16, MemoryAccess::Write)
                        .map_err(|error| ApiFault::from_memory(error, endpoint))?;
                    let bytes = runtime
                        .create_guid()
                        .map_err(|error| ApiFault::from_runtime(error, endpoint))?;
                    Ok(ApiImmediate::new(Some(0)).with_guest_write(output, bytes.to_vec()))
                })();
                return match result {
                    Ok(value) => ApiHandlerResult::Immediate(value),
                    Err(fault) => ApiHandlerResult::Fault(fault),
                };
            }
            "CoCreateInstance" => return handle_co_create_instance(endpoint, frame, runtime),
            "CoTaskMemAlloc" => return handle_co_task_mem_alloc(endpoint, frame, runtime),
            "CoTaskMemFree" => return handle_co_task_mem_free(endpoint, frame, runtime),
            _ => {}
        }
    }
    ApiHandlerResult::Fault(ApiFault::new(format!(
        "unsupported imported function {}!{:?}",
        endpoint.module, endpoint.symbol
    )))
}

fn handle_co_task_mem_alloc(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> ApiHandlerResult {
    let [byte_length] = frame.arguments.as_slice() else {
        return ApiHandlerResult::Fault(ApiFault::new("CoTaskMemAlloc requires one argument"));
    };
    match runtime.allocate_com_task_memory(*byte_length) {
        Ok(allocation) => ApiHandlerResult::Immediate(ApiImmediate::new(Some(
            allocation.map_or(0, |allocation| allocation.address.0),
        ))),
        Err(error) => ApiHandlerResult::Fault(ApiFault::from_runtime(error, endpoint)),
    }
}

fn handle_co_task_mem_free(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> ApiHandlerResult {
    let [address] = frame.arguments.as_slice() else {
        return ApiHandlerResult::Fault(ApiFault::new("CoTaskMemFree requires one argument"));
    };
    match runtime.free_com_task_memory(GuestAddress(*address)) {
        Ok(()) => ApiHandlerResult::Immediate(ApiImmediate::new(None)),
        Err(error) => ApiHandlerResult::Fault(ApiFault::from_runtime(error, endpoint)),
    }
}

fn handle_co_create_instance(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> ApiHandlerResult {
    let [class, outer, context, iid, output] = frame.arguments.as_slice() else {
        return ApiHandlerResult::Fault(ApiFault::new("CoCreateInstance requires five arguments"));
    };
    if *output == 0 {
        return ApiHandlerResult::Immediate(ApiImmediate::new(Some(0x8000_4003)));
    }
    let output = GuestAddress(*output);
    if let Err(error) = runtime
        .memory
        .validate_access(output, 4, MemoryAccess::Write)
    {
        return ApiHandlerResult::Fault(ApiFault::from_memory(error, endpoint));
    }
    // Read both GUIDs before touching output: Windows callers may alias them.
    let class_bytes = match runtime.memory.read(GuestAddress(*class), 16) {
        Ok(bytes) => bytes,
        Err(error) => return ApiHandlerResult::Fault(ApiFault::from_memory(error, endpoint)),
    };
    let iid_bytes = match runtime.memory.read(GuestAddress(*iid), 16) {
        Ok(bytes) => bytes,
        Err(error) => return ApiHandlerResult::Fault(ApiFault::from_memory(error, endpoint)),
    };
    let class = RuntimeComIid(class_bytes.try_into().expect("checked CLSID"));
    let iid = RuntimeComIid(iid_bytes.try_into().expect("checked IID"));
    let apartment = match runtime.current_com_apartment() {
        Ok(apartment) => apartment.is_some(),
        Err(error) => return ApiHandlerResult::Fault(ApiFault::from_runtime(error, endpoint)),
    };
    let failure = if !apartment {
        Some(0x8004_01f0)
    } else if *outer != 0 {
        Some(0x8004_0110)
    } else if *context == 0 {
        Some(0x8007_0057)
    } else if class != crate::directshow::CLSID_FILTER_GRAPH || *context & 1 == 0 {
        Some(0x8004_0154)
    } else {
        None
    };
    let spec = crate::directshow::filter_graph_spec();
    let failure = failure.or_else(|| {
        (!spec.interfaces.iter().any(|interface| interface.iid == iid)).then_some(0x8000_4002)
    });
    if let Some(hresult) = failure {
        return ApiHandlerResult::Immediate(
            ApiImmediate::new(Some(hresult)).with_guest_write(output, 0u32.to_le_bytes().to_vec()),
        );
    }
    match runtime.install_com_object_with_guest_writes(None, spec, |installed| {
        let pointer = installed
            .interfaces
            .iter()
            .find(|(interface, _)| *interface == iid)
            .expect("validated requested FilterGraph interface")
            .1;
        Ok(vec![(output, pointer.0.to_le_bytes().to_vec())])
    }) {
        Ok(_) => ApiHandlerResult::Immediate(ApiImmediate::new(Some(0))),
        Err(error) => ApiHandlerResult::Fault(ApiFault::from_runtime(error, endpoint)),
    }
}

fn handle_co_initialize(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> ApiHandlerResult {
    let reserved = match frame.argument_u32(0) {
        Some(value) => value,
        None => {
            return ApiHandlerResult::Fault(ApiFault::new(
                "decoded CoInitialize frame is missing reserved argument 0",
            ));
        }
    };
    if reserved != 0 {
        return ApiHandlerResult::Fault(ApiFault::new(
            "CoInitialize currently accepts only a NULL reserved pointer",
        ));
    }
    let result =
        match runtime.initialize_current_com_apartment(RuntimeComApartmentModel::SingleThreaded) {
            Ok(result) => result,
            Err(error) => {
                return ApiHandlerResult::Fault(ApiFault::from_runtime(error, endpoint));
            }
        };
    ApiHandlerResult::Immediate(ApiImmediate::new(Some((result).bits())))
}

fn handle_co_uninitialize(
    endpoint: &ApiEndpointIdentity,
    frame: &ApiCallFrame,
    runtime: &mut crate::runtime::Runtime,
) -> ApiHandlerResult {
    if !frame.arguments.is_empty() {
        return ApiHandlerResult::Fault(ApiFault::new("CoUninitialize takes no arguments"));
    }
    match runtime.uninitialize_current_com_apartment() {
        Ok(()) => ApiHandlerResult::Immediate(ApiImmediate::new(None)),
        Err(error) => ApiHandlerResult::Fault(ApiFault::from_runtime(error, endpoint)),
    }
}

pub(crate) fn ole32_argument_words(endpoint: &ApiEndpointIdentity) -> Option<u8> {
    if !endpoint.module.eq_ignore_ascii_case("ole32.dll") {
        return None;
    }
    let ApiSymbol::Name { name } = &endpoint.symbol else {
        return None;
    };
    match name.as_str() {
        "CoUninitialize" => Some(0),
        "CoInitialize" => Some(1),
        "CoCreateInstance" => Some(5),
        "CoTaskMemAlloc" | "CoTaskMemFree" | "CoCreateGuid" => Some(1),
        _ => None,
    }
}

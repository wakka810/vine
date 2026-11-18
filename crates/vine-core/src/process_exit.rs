use crate::api::{
    ApiCallFrame, ApiFault, ApiHandlerResult, ApiImmediate, GuestCallbackArgument,
    GuestCallbackCompletionPolicy, GuestCallbackRequest,
};
use crate::memory::{GuestAddress, MemoryAccess};
use crate::runtime::{Runtime, RuntimeApiBoundaryDisposition};

pub(crate) fn exit_process(
    frame: &ApiCallFrame,
    runtime: &mut Runtime,
    sentinel: GuestAddress,
) -> ApiHandlerResult {
    let result = (|| {
        let exit_code = frame
            .argument_u32(0)
            .ok_or_else(|| ApiFault::new("missing process exit code"))?;
        let next = detach_next(frame, runtime, sentinel, exit_code, 0)?;
        runtime.begin_process_exit();
        Ok(next)
    })();
    result.unwrap_or_else(ApiHandlerResult::Fault)
}

pub(crate) fn detach_next(
    frame: &ApiCallFrame,
    runtime: &Runtime,
    sentinel: GuestAddress,
    exit_code: u32,
    index: u32,
) -> Result<ApiHandlerResult, ApiFault> {
    let targets = runtime
        .modules
        .process_detach_targets()
        .map_err(|e| ApiFault::from_runtime(e, &frame.endpoint))?;
    if index as usize == targets.len() {
        return Ok(ApiHandlerResult::Immediate(
            ApiImmediate::new(Some(exit_code))
                .with_execution_boundary(RuntimeApiBoundaryDisposition::TerminateProcess),
        ));
    }
    let (module, target) = *targets
        .get(index as usize)
        .ok_or_else(|| ApiFault::new("process detach callback order changed"))?;
    let request = GuestCallbackRequest {
        target,
        arguments: vec![
            GuestCallbackArgument::U32(module),
            GuestCallbackArgument::U32(0),
            GuestCallbackArgument::U32(1),
        ],
        scratch: Vec::new(),
        scratch_pointer_relocations: Vec::new(),
        return_sentinel: sentinel,
        completion_policy: GuestCallbackCompletionPolicy::ExitProcess {
            exit_code,
            next_index: index + 1,
        },
    };
    runtime.preflight_guest_callback(frame, &request)?;
    Ok(ApiHandlerResult::GuestCallback(Box::new(request)))
}

pub(crate) fn unhandled_exception(
    frame: &ApiCallFrame,
    runtime: &Runtime,
    sentinel: GuestAddress,
) -> ApiHandlerResult {
    let result = (|| {
        let pointers = frame
            .argument_u32(0)
            .ok_or_else(|| ApiFault::new("missing EXCEPTION_POINTERS"))?;
        let bytes = runtime
            .memory
            .read(GuestAddress(pointers), 8)
            .map_err(|e| ApiFault::from_memory(e, &frame.endpoint))?;
        for (offset, length) in [(0, 80), (4, 716)] {
            let address = GuestAddress(u32::from_le_bytes(
                bytes[offset..offset + 4].try_into().unwrap(),
            ));
            runtime
                .memory
                .validate_access(address, length, MemoryAccess::Read)
                .map_err(|e| ApiFault::from_memory(e, &frame.endpoint))?;
        }
        let Some(target) = runtime.top_level_exception_filter() else {
            return Ok(ApiHandlerResult::Immediate(ApiImmediate::new(Some(1))));
        };
        let request = GuestCallbackRequest {
            target,
            arguments: vec![GuestCallbackArgument::U32(pointers)],
            scratch: Vec::new(),
            scratch_pointer_relocations: Vec::new(),
            return_sentinel: sentinel,
            completion_policy: GuestCallbackCompletionPolicy::UnhandledExceptionFilter,
        };
        runtime.preflight_guest_callback(frame, &request)?;
        Ok(ApiHandlerResult::GuestCallback(Box::new(request)))
    })();
    result.unwrap_or_else(ApiHandlerResult::Fault)
}

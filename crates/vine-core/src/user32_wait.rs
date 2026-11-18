use crate::api::{
    ApiCallFrame, ApiDeferredCall, ApiDispatcher, ApiFault, ApiHandlerResult, ApiImmediate,
    DispatchOutcome, HostCompletion, HostRequest,
};
use crate::memory::GuestAddress;
use crate::runtime::{Runtime, RuntimeHandle, RuntimeObjectKind, RuntimeWaitState};

pub(crate) const SERVICE: &str = "browser.message-wait";
#[derive(Debug, Clone)]
pub(crate) struct MessageWait {
    handles: Vec<RuntimeHandle>,
    all: bool,
    mask: u32,
    deadline: Option<u64>,
}

fn poll(runtime: &mut Runtime, wait: &MessageWait) -> Result<Option<ApiImmediate>, ApiFault> {
    let err = |e| ApiFault::new(format!("MsgWaitForMultipleObjects: {e}"));
    let mut ready = Vec::with_capacity(wait.handles.len());
    for handle in &wait.handles {
        if !runtime
            .is_live_object_handle(*handle, RuntimeObjectKind::Event)
            .map_err(err)?
        {
            return Ok(Some(ApiImmediate::new(Some(u32::MAX)).with_last_error(6)));
        }
        ready.push(matches!(
            runtime.event_for_handle(*handle).map_err(err)?.wait_state,
            RuntimeWaitState::Signaled { .. }
        ));
    }
    let status = runtime
        .message_wait_queue_status(wait.mask, false)
        .map_err(err)?;
    let message = status & 0xffff != 0;
    let index = if wait.all {
        (message && ready.iter().all(|ready| *ready)).then_some(0)
    } else {
        ready
            .iter()
            .position(|ready| *ready)
            .or_else(|| message.then_some(wait.handles.len()))
    };
    if let Some(index) = index {
        if wait.all {
            for handle in &wait.handles {
                runtime.consume_event_signal(*handle).map_err(err)?;
            }
            runtime
                .message_wait_queue_status(wait.mask, true)
                .map_err(err)?;
        } else if index == wait.handles.len() {
            runtime
                .message_wait_queue_status(wait.mask, true)
                .map_err(err)?;
        } else {
            runtime
                .consume_event_signal(wait.handles[index])
                .map_err(err)?;
        }
        return Ok(Some(ApiImmediate::new(Some(index as u32))));
    }
    if wait
        .deadline
        .is_some_and(|deadline| runtime.timer_time_100ns() >= deadline)
    {
        return Ok(Some(ApiImmediate::new(Some(258))));
    }
    Ok(None)
}

fn delay(runtime: &Runtime, wait: &MessageWait) -> u32 {
    wait.deadline.map_or(10, |deadline| {
        deadline
            .saturating_sub(runtime.timer_time_100ns())
            .div_ceil(10_000)
            .clamp(1, 10) as u32
    })
}

pub(crate) fn begin(frame: &ApiCallFrame, runtime: &mut Runtime) -> ApiHandlerResult {
    let result = (|| {
        let count = frame.argument_u32(0).unwrap();
        let array = frame.argument_u32(1).unwrap();
        let timeout = frame.argument_u32(3).unwrap();
        let mask = frame.argument_u32(4).unwrap();
        if count > 63
            || mask & !crate::runtime::RUNTIME_QUEUE_STATUS_VALID_FLAGS != 0
            || count != 0 && array == 0
        {
            return Ok(ApiHandlerResult::Immediate(
                ApiImmediate::new(Some(u32::MAX)).with_last_error(87),
            ));
        }
        let bytes = runtime
            .memory
            .read(GuestAddress(array), count as usize * 4)
            .map_err(|e| ApiFault::from_memory(e, &frame.endpoint))?;
        let handles: Vec<_> = bytes
            .chunks_exact(4)
            .map(|b| RuntimeHandle(u32::from_le_bytes(b.try_into().unwrap())))
            .collect();
        for (i, handle) in handles.iter().enumerate() {
            if handles[..i].contains(handle) {
                return Ok(ApiHandlerResult::Immediate(
                    ApiImmediate::new(Some(u32::MAX)).with_last_error(87),
                ));
            }
            if !runtime
                .is_live_object_handle(*handle, RuntimeObjectKind::Event)
                .map_err(|e| ApiFault::from_runtime(e, &frame.endpoint))?
            {
                return Ok(ApiHandlerResult::Immediate(
                    ApiImmediate::new(Some(u32::MAX)).with_last_error(6),
                ));
            }
        }
        let wait = MessageWait {
            handles,
            all: frame.argument_u32(2).unwrap() != 0,
            mask,
            deadline: if timeout == u32::MAX {
                None
            } else {
                Some(
                    runtime
                        .timer_time_100ns()
                        .checked_add(u64::from(timeout) * 10_000)
                        .ok_or_else(|| ApiFault::new("message wait deadline overflow"))?,
                )
            },
        };
        if let Some(result) = poll(runtime, &wait)? {
            return Ok(ApiHandlerResult::Immediate(result));
        }
        let milliseconds = delay(runtime, &wait);
        let request = runtime
            .begin_host_request(SERVICE)
            .map_err(|e| ApiFault::from_runtime(e, &frame.endpoint))?;
        runtime.message_waits.insert(request.request_id, wait);
        Ok(ApiHandlerResult::Deferred(HostRequest {
            request_id: request.request_id,
            service: request.service,
            payload: milliseconds.to_le_bytes().to_vec(),
        }))
    })();
    result.unwrap_or_else(ApiHandlerResult::Fault)
}

pub fn complete_user32_message_wait(
    dispatcher: &ApiDispatcher,
    call: &ApiDeferredCall,
    acknowledgement: u32,
    runtime: &mut Runtime,
) -> DispatchOutcome {
    let result = (|| {
        dispatcher.validate_deferred_completion(call, call.request.request_id, &runtime.memory)?;
        runtime
            .validate_owned_host_request(call.request.request_id, SERVICE)
            .map_err(|e| ApiFault::from_runtime(e, &call.frame.endpoint))?;
        if acknowledgement != 1 || call.request.service != SERVICE {
            return Err(ApiFault::new("invalid message wait completion"));
        }
        let wait = runtime
            .message_waits
            .get(&call.request.request_id)
            .cloned()
            .ok_or_else(|| ApiFault::new("message wait is not pending"))?;
        if let Some(result) = poll(runtime, &wait)? {
            let outcome = dispatcher.complete_deferred(
                call,
                HostCompletion {
                    request_id: call.request.request_id,
                    result,
                },
                runtime,
            );
            if matches!(outcome, DispatchOutcome::Immediate(_)) {
                runtime
                    .complete_host_request(call.request.request_id)
                    .map_err(|e| ApiFault::from_runtime(e, &call.frame.endpoint))?;
                runtime.message_waits.remove(&call.request.request_id);
            }
            Ok(outcome)
        } else {
            Ok(DispatchOutcome::Deferred(ApiDeferredCall {
                frame: call.frame.clone(),
                request: HostRequest {
                    request_id: call.request.request_id,
                    service: SERVICE.to_owned(),
                    payload: delay(runtime, &wait).to_le_bytes().to_vec(),
                },
            }))
        }
    })();
    result.unwrap_or_else(DispatchOutcome::Fault)
}

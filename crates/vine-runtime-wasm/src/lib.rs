#![deny(unsafe_op_in_unsafe_fn)]

use std::cell::RefCell;
use std::collections::BTreeMap;

use serde_json::{Value, json};
use vine_core::{
    AbiSchema, ApiArgumentSchema, ApiBehavior, ApiContract, ApiDeferredCall, ApiDirection,
    ApiDispatcher, ApiEndpointIdentity, ApiFault, ApiGuestCallbackCall, ApiHandlerResult,
    ApiImmediate, ApiImmediateCompletion, ApiReturnSchema, ApiReturnValue, ApiTrap, ApiValueSchema,
    CallbackPolicy, CallingConvention, CpuWritebackReceipt, DispatchOutcome, ExecutionSession,
    GuestAddress, GuestCallbackRequest, GuestMemory, HostCompletion, HostRequest, ImportBinding,
    InitialThreadStack, LOGICAL_ENCODED_POINTER_NAMESPACE, LastErrorPolicy, MemoryDeltaBuffers,
    MemoryLimits, ModuleSnapshot, Pe32Limits, ProcessAddressSpaceLayout, ReentrancyPolicy,
    RuntimeBootstrap, RuntimeCallbackFrameId, RuntimeClockState, RuntimeExport,
    RuntimeExportCatalog, RuntimeProcessId, RuntimeSchedulerEvent, RuntimeStartupInfo,
    RuntimeState, RuntimeThreadId, StackCleanup, bind_pe32_imports, build_pe32_load_plan,
    install_controlled_initial_stack, install_runtime_return_sentinels, materialize_pe32_image,
};

const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024;
const FIXTURE_SHA256: &str = "24c98ec43bc76dcd0c1ddbbbf68c2ea1d86207bc2ebea2aea536a4757bf4189f";
const FIXTURE_MODULE: &str = "vine_browser_fixture.dll";
const LOAD_BASE: GuestAddress = GuestAddress(0x1000_0000);
const PROCESS_ID: u32 = 4096;
const THREAD_ID: u32 = 8192;
const CANCEL_EXIT_CODE: u32 = 0xc000_013a;
const DEFERRED_ADD: &str = "browser-fixture-deferred-add-v1";
const CALL_CALLBACK: &str = "browser-fixture-call-callback-v1";
const COMMIT_OUTPUTS: &str = "browser-fixture-commit-outputs-v1";

thread_local! {
    static BRIDGE: RefCell<Bridge> = const { RefCell::new(Bridge::new()) };
}

struct Bridge {
    input: Vec<u8>,
    metadata: Vec<u8>,
    control: Vec<u8>,
    payload: Vec<u8>,
    runtime: Option<BrowserRuntime>,
}

impl Bridge {
    const fn new() -> Self {
        Self {
            input: Vec::new(),
            metadata: Vec::new(),
            control: Vec::new(),
            payload: Vec::new(),
            runtime: None,
        }
    }

    fn clear_output(&mut self) {
        self.metadata.clear();
        self.control.clear();
        self.payload.clear();
    }

    fn set_output(
        &mut self,
        metadata: Value,
        control: Vec<u8>,
        payload: Vec<u8>,
    ) -> Result<(), String> {
        self.metadata = serde_json::to_vec(&metadata).map_err(|error| error.to_string())?;
        self.control = control;
        self.payload = payload;
        Ok(())
    }

    fn set_error(&mut self, message: String) {
        self.control.clear();
        self.payload.clear();
        self.metadata = serde_json::to_vec(&json!({
            "kind": "error",
            "message": message,
        }))
        .unwrap_or_else(|_| b"{\"kind\":\"error\",\"message\":\"serialization failed\"}".to_vec());
    }

    fn prepare(&mut self, input_length: usize) -> Result<(), String> {
        let fixture = self
            .input
            .get(..input_length)
            .ok_or_else(|| "fixture input length exceeds the bridge buffer".to_owned())?
            .to_vec();
        let (runtime, metadata, reflection) = BrowserRuntime::prepare(fixture)?;
        self.runtime = Some(runtime);
        self.set_output(metadata, reflection.control, reflection.page_bytes)
    }

    fn runtime_mut(&mut self) -> Result<&mut BrowserRuntime, String> {
        self.runtime
            .as_mut()
            .ok_or_else(|| "browser runtime has not been prepared".to_owned())
    }
}

struct EndpointRoute {
    id: u32,
    argument_bytes: u32,
}

struct ActiveCallback {
    call: ApiGuestCallbackCall,
    frame_id: RuntimeCallbackFrameId,
    stack_pointer: GuestAddress,
}

struct BrowserRuntime {
    memory: GuestMemory,
    runtime: RuntimeState,
    dispatcher: ApiDispatcher,
    routes: BTreeMap<GuestAddress, EndpointRoute>,
    callback_sentinel: GuestAddress,
    entry_sentinel: GuestAddress,
    stack: InitialThreadStack,
    pending_host: Option<ApiDeferredCall>,
    callbacks: Vec<ActiveCallback>,
}

impl BrowserRuntime {
    fn prepare(
        fixture: Vec<u8>,
    ) -> Result<(Self, Value, vine_core::MemoryReflectionBuffers), String> {
        let loader_limits = Pe32Limits::default();
        let snapshot = ModuleSnapshot::from_bytes(fixture, &loader_limits).map_err(error_string)?;
        snapshot
            .verify_expected_sha256(FIXTURE_SHA256)
            .map_err(error_string)?;
        let plan = build_pe32_load_plan(&snapshot, loader_limits).map_err(error_string)?;
        let mut memory = GuestMemory::new(MemoryLimits {
            max_physical_pages: 16,
            ..MemoryLimits::default()
        })
        .map_err(error_string)?;
        let materialization = materialize_pe32_image(&mut memory, &snapshot, &plan, LOAD_BASE)
            .map_err(error_string)?;
        let catalog = RuntimeExportCatalog::new(
            [
                "fixture_deferred_add",
                "fixture_call_callback",
                "fixture_commit_outputs",
            ]
            .into_iter()
            .map(|name| RuntimeExport::named(FIXTURE_MODULE, name))
            .collect(),
        )
        .map_err(error_string)?;
        let binding = bind_pe32_imports(&mut memory, &plan, &materialization, &catalog)
            .map_err(error_string)?;
        if !binding.complete || !binding.image_owner_sealed {
            return Err("browser fixture import binding did not complete".to_owned());
        }
        let sentinels = install_runtime_return_sentinels(&mut memory).map_err(error_string)?;
        let stack = install_controlled_initial_stack(&mut memory, &sentinels.process_entry_return)
            .map_err(error_string)?;
        let (dispatcher, routes) = build_dispatcher(&binding)?;
        let runtime = controlled_runtime_state()?;
        let reflection = memory.pending_reflection().map_err(error_string)?;
        let reflection_buffers = reflection.encode_binary().map_err(error_string)?;
        let metadata = json!({
            "kind": "prepared",
            "schemaVersion": 1,
            "source": snapshot.identity(),
            "entryPoint": materialization.entry_point_address.0,
            "generation": reflection.invalidation.generation,
            "stackTop": stack.stack_top.0,
            "initialStackPointer": stack.initial_esp.0,
            "callbackSentinel": sentinels.callback_return.address.0,
            "entrySentinel": sentinels.process_entry_return.address.0,
            "pageCount": reflection.pages.len(),
            "endpoints": binding.thunks,
        });
        Ok((
            Self {
                memory,
                runtime,
                dispatcher,
                routes,
                callback_sentinel: sentinels.callback_return.address,
                entry_sentinel: sentinels.process_entry_return.address,
                stack,
                pending_host: None,
                callbacks: Vec::new(),
            },
            metadata,
            reflection_buffers,
        ))
    }

    fn acknowledge(&mut self, generation: u64) -> Result<Value, String> {
        self.memory
            .acknowledge_delta(generation)
            .map_err(error_string)?;
        Ok(json!({
            "kind": "acknowledged",
            "generation": generation,
            "dirtyPages": self.memory.stats().dirty_pages,
        }))
    }

    fn reconcile_writeback(
        &mut self,
        control: &[u8],
        payload: &[u8],
    ) -> Result<(Value, CpuWritebackReceipt), String> {
        let receipt = self
            .memory
            .reconcile_cpu_writeback_binary(control, payload)
            .map_err(error_string)?;
        let metadata = json!({
            "kind": "writeback-reconciled",
            "receipt": receipt,
            "dirtyPages": self.memory.stats().dirty_pages,
        });
        Ok((metadata, receipt))
    }

    fn dispatch(
        &mut self,
        endpoint_address: GuestAddress,
        stack_pointer: GuestAddress,
        return_address: GuestAddress,
    ) -> Result<(Value, Option<MemoryDeltaBuffers>), String> {
        if self.pending_host.is_some() {
            return Err("guest dispatch attempted while a host request is pending".to_owned());
        }
        let route = self
            .routes
            .get(&endpoint_address)
            .ok_or_else(|| format!("unowned API endpoint {:#010x}", endpoint_address.0))?;
        let trap = ApiTrap {
            endpoint_id: vine_core::ImportThunkId(route.id),
            endpoint_address,
            stack_pointer,
            return_address,
            argument_bytes: route.argument_bytes,
        };
        let callback_sentinel = self.callback_sentinel;
        let dispatcher = &self.dispatcher;
        let mut session = ExecutionSession::new(&mut self.memory, &mut self.runtime);
        let outcome =
            dispatcher.dispatch(
                trap,
                &mut session,
                |contract, frame, session| match contract.implementation_id.as_str() {
                    DEFERRED_ADD | COMMIT_OUTPUTS => {
                        let service = if contract.implementation_id == DEFERRED_ADD {
                            "browser.deferred-add"
                        } else {
                            "browser.commit-outputs"
                        };
                        let request = match session.runtime_mut().begin_host_request(service) {
                            Ok(request) => request,
                            Err(error) => {
                                return ApiHandlerResult::Fault(ApiFault::runtime_defect(
                                    "browser-runtime",
                                    "host-request-state-invalid",
                                    error.to_string(),
                                ));
                            }
                        };
                        let payload = frame
                            .arguments
                            .iter()
                            .flat_map(|argument| {
                                argument.words.iter().flat_map(|word| word.to_le_bytes())
                            })
                            .collect();
                        ApiHandlerResult::Deferred(HostRequest {
                            request_id: request.request_id,
                            service: request.service,
                            payload,
                        })
                    }
                    CALL_CALLBACK => {
                        let target = frame.argument_u32(0).unwrap_or(0);
                        let value = frame.argument_u32(1).unwrap_or(0);
                        if let Err(error) = session.memory().validate_access(
                            GuestAddress(target),
                            1,
                            vine_core::MemoryAccess::Execute,
                        ) {
                            return ApiHandlerResult::Fault(ApiFault::invalid_call(
                                "browser-runtime",
                                "callback-target-not-executable",
                                error.to_string(),
                            ));
                        }
                        ApiHandlerResult::GuestCallback(GuestCallbackRequest {
                            target: GuestAddress(target),
                            arguments: vec![value],
                            return_sentinel: callback_sentinel,
                        })
                    }
                    _ => ApiHandlerResult::Fault(ApiFault::unsupported(
                        "browser-runtime",
                        "fixture-service-unsupported",
                        "browser fixture reached an unknown service",
                    )),
                },
            );
        match outcome {
            DispatchOutcome::Deferred(call) => {
                let arguments = decode_u32_payload(&call.request.payload);
                let metadata = json!({
                    "kind": "host-request",
                    "requestId": call.request.request_id,
                    "service": call.request.service,
                    "arguments": arguments,
                    "scheduler": self.runtime.scheduler_state().map_err(error_string)?,
                });
                self.pending_host = Some(call);
                Ok((metadata, None))
            }
            DispatchOutcome::GuestCallback(call) => self.begin_callback(call),
            DispatchOutcome::Immediate(completion) => {
                Ok((completion_metadata("api-complete", &completion), None))
            }
            DispatchOutcome::Fault(fault) => Err(api_fault_string(fault)),
        }
    }

    fn begin_callback(
        &mut self,
        call: ApiGuestCallbackCall,
    ) -> Result<(Value, Option<MemoryDeltaBuffers>), String> {
        let frame_bytes = call
            .request
            .arguments
            .len()
            .checked_add(1)
            .and_then(|words| words.checked_mul(4))
            .ok_or_else(|| "callback frame size overflowed".to_owned())?;
        let stack_pointer = GuestAddress(
            call.frame
                .stack_pointer
                .0
                .checked_sub(u32::try_from(frame_bytes).map_err(|_| "callback frame is too large")?)
                .ok_or_else(|| "callback stack pointer underflowed".to_owned())?,
        );
        let runtime_frame = self
            .runtime
            .push_callback_frame(call.request.target, call.request.return_sentinel)
            .map_err(error_string)?;
        let mut bytes = Vec::with_capacity(frame_bytes);
        bytes.extend_from_slice(&call.request.return_sentinel.0.to_le_bytes());
        for argument in &call.request.arguments {
            bytes.extend_from_slice(&argument.to_le_bytes());
        }
        if let Err(error) = self.memory.write(stack_pointer, &bytes) {
            let _ = self.runtime.pop_callback_frame(runtime_frame.id);
            return Err(error.to_string());
        }
        let delta = self.memory.pending_delta().map_err(error_string)?;
        let generation = delta.generation;
        let buffers = delta.encode_binary().map_err(error_string)?;
        let depth = self.callbacks.len() + 1;
        let metadata = json!({
            "kind": "guest-callback",
            "frameId": runtime_frame.id.0,
            "depth": depth,
            "target": call.request.target.0,
            "stackPointer": stack_pointer.0,
            "returnSentinel": call.request.return_sentinel.0,
            "generation": generation,
        });
        self.callbacks.push(ActiveCallback {
            call,
            frame_id: runtime_frame.id,
            stack_pointer,
        });
        Ok((metadata, Some(buffers)))
    }

    fn complete_host(&mut self, request_id: u64, result: u32) -> Result<Value, String> {
        let call = self
            .pending_host
            .take()
            .ok_or_else(|| "host completion has no pending API call".to_owned())?;
        if call.request.request_id != request_id {
            self.pending_host = Some(call);
            return Err("host completion request ID mismatch".to_owned());
        }
        let runtime_completion = self
            .runtime
            .complete_host_request(request_id)
            .map_err(error_string)?;
        match self.runtime.pop_scheduler_event() {
            Some(RuntimeSchedulerEvent::HostCompleted(completion))
                if completion == runtime_completion => {}
            _ => return Err("host completion scheduler event mismatch".to_owned()),
        }
        let dispatcher = &self.dispatcher;
        let mut session = ExecutionSession::new(&mut self.memory, &mut self.runtime);
        match dispatcher.complete_deferred(
            call,
            HostCompletion {
                request_id,
                result: ApiImmediate::new(ApiReturnValue::U32(result)),
            },
            &mut session,
        ) {
            DispatchOutcome::Immediate(completion) => {
                Ok(completion_metadata("host-complete", &completion))
            }
            DispatchOutcome::Fault(fault) => Err(api_fault_string(fault)),
            _ => Err("deferred API completion produced another suspended outcome".to_owned()),
        }
    }

    fn complete_callback(&mut self, sentinel: GuestAddress, result: u32) -> Result<Value, String> {
        let active = self
            .callbacks
            .pop()
            .ok_or_else(|| "callback return has no active frame".to_owned())?;
        if sentinel != active.call.request.return_sentinel {
            self.callbacks.push(active);
            return Err("callback return sentinel mismatch".to_owned());
        }
        self.runtime
            .pop_callback_frame(active.frame_id)
            .map_err(error_string)?;
        let dispatcher = &self.dispatcher;
        let mut session = ExecutionSession::new(&mut self.memory, &mut self.runtime);
        match dispatcher.complete_guest_callback(
            active.call,
            ApiImmediate::new(ApiReturnValue::U32(result)),
            &mut session,
        ) {
            DispatchOutcome::Immediate(completion) => Ok(json!({
                "kind": "callback-complete",
                "frameId": active.frame_id.0,
                "callbackStackPointer": active.stack_pointer.0,
                "remainingDepth": self.callbacks.len(),
                "completion": completion,
            })),
            DispatchOutcome::Fault(fault) => Err(api_fault_string(fault)),
            _ => Err("callback completion produced another suspended outcome".to_owned()),
        }
    }

    fn cancel(&mut self, exit_code: u32) -> Result<Value, String> {
        self.pending_host = None;
        self.callbacks.clear();
        let cancellation = self
            .runtime
            .terminate_current_process(exit_code)
            .map_err(error_string)?;
        Ok(json!({
            "kind": "cancelled",
            "cancellation": cancellation,
            "pendingHostCalls": usize::from(self.pending_host.is_some()),
            "pendingCallbacks": self.callbacks.len(),
        }))
    }

    fn finish(&mut self, exit_code: u32) -> Result<Value, String> {
        if self.pending_host.is_some() || !self.callbacks.is_empty() {
            return Err("process exit has suspended host or callback work".to_owned());
        }
        let termination = self
            .runtime
            .terminate_current_process(exit_code)
            .map_err(error_string)?;
        if termination.host_requests_cancelled != 0 || termination.callback_frames_cancelled != 0 {
            return Err("clean process exit discarded runtime work".to_owned());
        }
        Ok(json!({
            "kind": "process-exit",
            "exitCode": exit_code,
            "entrySentinel": self.entry_sentinel.0,
            "stackTop": self.stack.stack_top.0,
            "pendingHostCalls": 0,
            "pendingCallbacks": 0,
        }))
    }
}

fn build_dispatcher(
    binding: &ImportBinding,
) -> Result<(ApiDispatcher, BTreeMap<GuestAddress, EndpointRoute>), String> {
    let endpoints = binding
        .thunks
        .iter()
        .map(ApiEndpointIdentity::from_import_thunk)
        .collect::<Vec<_>>();
    let mut contracts = Vec::with_capacity(endpoints.len());
    let mut routes = BTreeMap::new();
    for endpoint in &endpoints {
        let name = match &endpoint.symbol {
            vine_core::ApiSymbol::Name { name } => name.as_str(),
            vine_core::ApiSymbol::Ordinal { .. } => {
                return Err("browser fixture imports must be named".to_owned());
            }
        };
        let (implementation, behavior, callback_policy, arguments) = match name {
            "fixture_deferred_add" => (
                DEFERRED_ADD,
                ApiBehavior::Deferred,
                CallbackPolicy::Forbidden,
                vec![u32_argument("left"), u32_argument("right")],
            ),
            "fixture_call_callback" => (
                CALL_CALLBACK,
                ApiBehavior::Immediate,
                CallbackPolicy::Required,
                vec![callback_argument("callback"), u32_argument("value")],
            ),
            "fixture_commit_outputs" => (
                COMMIT_OUTPUTS,
                ApiBehavior::Deferred,
                CallbackPolicy::Forbidden,
                vec![
                    u32_argument("rgba"),
                    u32_argument("toneHz"),
                    u32_argument("persistentByte"),
                ],
            ),
            _ => return Err(format!("unsupported browser fixture import {name}")),
        };
        let contract = ApiContract {
            endpoint: endpoint.clone(),
            abi: AbiSchema {
                calling_convention: CallingConvention::Cdecl,
                arguments,
                return_value: ApiReturnSchema::U32,
                stack_cleanup: StackCleanup::Caller,
            },
            last_error_policy: LastErrorPolicy::Preserve,
            reentrancy_policy: ReentrancyPolicy::Allowed,
            behavior,
            callback_policy,
            implementation_id: implementation.to_owned(),
        };
        routes.insert(
            endpoint.address,
            EndpointRoute {
                id: endpoint.id.0,
                argument_bytes: contract.abi.argument_bytes().map_err(api_fault_string)?,
            },
        );
        contracts.push(contract);
    }
    let dispatcher = ApiDispatcher::new(endpoints, contracts).map_err(api_fault_string)?;
    Ok((dispatcher, routes))
}

fn u32_argument(name: &str) -> ApiArgumentSchema {
    ApiArgumentSchema {
        name: name.to_owned(),
        direction: ApiDirection::In,
        value: ApiValueSchema::U32,
    }
}

fn callback_argument(name: &str) -> ApiArgumentSchema {
    ApiArgumentSchema {
        name: name.to_owned(),
        direction: ApiDirection::In,
        value: ApiValueSchema::OpaquePointer {
            nullable: false,
            alignment: 1,
        },
    }
}

fn controlled_runtime_state() -> Result<RuntimeState, String> {
    let mut startup_info = vec![0; 68];
    startup_info[..4].copy_from_slice(&68_u32.to_le_bytes());
    RuntimeState::new(RuntimeBootstrap {
        process_id: RuntimeProcessId(PROCESS_ID),
        initial_thread_id: RuntimeThreadId(THREAD_ID),
        initial_last_error: 0,
        address_space: ProcessAddressSpaceLayout::vine_v1(),
        clock: RuntimeClockState {
            wall_clock_file_time: 134_282_016_000_000_000,
            tick_count: 3_600_000,
            performance_counter: 12_345_678_901,
        },
        startup_info: RuntimeStartupInfo {
            profile_id: "browser-vertical-startup-v1".to_owned(),
            bytes: startup_info,
        },
        encoded_pointer_namespace: LOGICAL_ENCODED_POINTER_NAMESPACE.to_owned(),
        encoded_pointer_token_prefix: 0xec02_0000,
    })
    .map_err(error_string)
}

fn completion_metadata(kind: &str, completion: &ApiImmediateCompletion) -> Value {
    json!({
        "kind": kind,
        "completion": completion,
    })
}

fn decode_u32_payload(payload: &[u8]) -> Vec<u32> {
    payload
        .chunks_exact(4)
        .map(|bytes| u32::from_le_bytes(bytes.try_into().expect("four-byte host payload word")))
        .collect()
}

fn api_fault_string(fault: ApiFault) -> String {
    format!("{}:{}: {}", fault.stage, fault.code, fault.message)
}

fn error_string(error: impl std::fmt::Display) -> String {
    error.to_string()
}

fn run_bridge(operation: impl FnOnce(&mut Bridge) -> Result<(), String>) -> u32 {
    BRIDGE.with(|cell| {
        let mut bridge = cell.borrow_mut();
        bridge.clear_output();
        match operation(&mut bridge) {
            Ok(()) => 0,
            Err(error) => {
                bridge.set_error(error);
                1
            }
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_input_resize(byte_length: u32) -> u32 {
    run_bridge(|bridge| {
        let byte_length = byte_length as usize;
        if byte_length > MAX_INPUT_BYTES {
            return Err("bridge input exceeds the 16 MiB limit".to_owned());
        }
        bridge.input.resize(byte_length, 0);
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_input_ptr() -> u32 {
    BRIDGE.with(|cell| cell.borrow_mut().input.as_mut_ptr() as usize as u32)
}

#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_metadata_ptr() -> u32 {
    BRIDGE.with(|cell| cell.borrow().metadata.as_ptr() as usize as u32)
}

#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_metadata_len() -> u32 {
    BRIDGE.with(|cell| cell.borrow().metadata.len() as u32)
}

#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_control_ptr() -> u32 {
    BRIDGE.with(|cell| cell.borrow().control.as_ptr() as usize as u32)
}

#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_control_len() -> u32 {
    BRIDGE.with(|cell| cell.borrow().control.len() as u32)
}

#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_payload_ptr() -> u32 {
    BRIDGE.with(|cell| cell.borrow().payload.as_ptr() as usize as u32)
}

#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_payload_len() -> u32 {
    BRIDGE.with(|cell| cell.borrow().payload.len() as u32)
}

#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_prepare(input_length: u32) -> u32 {
    run_bridge(|bridge| bridge.prepare(input_length as usize))
}

#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_acknowledge(generation_low: u32, generation_high: u32) -> u32 {
    run_bridge(|bridge| {
        let generation = u64::from(generation_low) | (u64::from(generation_high) << 32);
        let metadata = bridge.runtime_mut()?.acknowledge(generation)?;
        bridge.set_output(metadata, Vec::new(), Vec::new())
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_reconcile_writeback(control_length: u32, payload_length: u32) -> u32 {
    run_bridge(|bridge| {
        let control_length = control_length as usize;
        let payload_length = payload_length as usize;
        let total = control_length
            .checked_add(payload_length)
            .ok_or_else(|| "writeback input length overflowed".to_owned())?;
        let input = bridge
            .input
            .get(..total)
            .ok_or_else(|| "writeback input exceeds the bridge buffer".to_owned())?
            .to_vec();
        let (control, payload) = input.split_at(control_length);
        let (metadata, _) = bridge
            .runtime_mut()?
            .reconcile_writeback(control, payload)?;
        bridge.set_output(metadata, Vec::new(), Vec::new())
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_dispatch(
    endpoint_address: u32,
    stack_pointer: u32,
    return_address: u32,
) -> u32 {
    run_bridge(|bridge| {
        let (metadata, delta) = bridge.runtime_mut()?.dispatch(
            GuestAddress(endpoint_address),
            GuestAddress(stack_pointer),
            GuestAddress(return_address),
        )?;
        let (control, payload) = delta
            .map(|buffers| (buffers.control, buffers.page_bytes))
            .unwrap_or_default();
        bridge.set_output(metadata, control, payload)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_complete_host(
    request_low: u32,
    request_high: u32,
    result: u32,
) -> u32 {
    run_bridge(|bridge| {
        let request_id = u64::from(request_low) | (u64::from(request_high) << 32);
        let metadata = bridge.runtime_mut()?.complete_host(request_id, result)?;
        bridge.set_output(metadata, Vec::new(), Vec::new())
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_complete_callback(sentinel: u32, result: u32) -> u32 {
    run_bridge(|bridge| {
        let metadata = bridge
            .runtime_mut()?
            .complete_callback(GuestAddress(sentinel), result)?;
        bridge.set_output(metadata, Vec::new(), Vec::new())
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_cancel() -> u32 {
    run_bridge(|bridge| {
        let metadata = bridge.runtime_mut()?.cancel(CANCEL_EXIT_CODE)?;
        bridge.set_output(metadata, Vec::new(), Vec::new())
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_finish(exit_code: u32) -> u32 {
    run_bridge(|bridge| {
        let metadata = bridge.runtime_mut()?.finish(exit_code)?;
        bridge.set_output(metadata, Vec::new(), Vec::new())
    })
}

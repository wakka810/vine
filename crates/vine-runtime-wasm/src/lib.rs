#![deny(unsafe_op_in_unsafe_fn)]

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};
use vine_core::{
    AbiSchema, ApiArgumentSchema, ApiBehavior, ApiContract, ApiDeferredCall, ApiDirection,
    ApiDispatcher, ApiEndpointIdentity, ApiFault, ApiGuestCallbackCall, ApiHandlerResult,
    ApiImmediate, ApiImmediateCompletion, ApiReturnSchema, ApiReturnValue, ApiTrap, ApiValueSchema,
    CallbackPolicy, CallingConvention, CpuWritebackReceipt, DispatchOutcome, ExecutionSession,
    GUEST_PAGE_SIZE, GuestAddress, GuestCallbackRequest, GuestMemory, GuestRange, HostCompletion,
    HostRequest, ImportBinding, ImportTarget, InitialThreadStack, Kernel32CompatibilityPolicy,
    LOGICAL_ENCODED_POINTER_NAMESPACE, LOGICAL_LOADED_MODULE_NAMESPACE, LastErrorPolicy,
    MappingOperation, MemoryDeltaBuffers, MemoryLimits, ModuleSnapshot, PageProtection, Pe32Limits,
    ProcessAddressSpaceLayout, ReentrancyPolicy, RuntimeBootstrap, RuntimeCallbackFrameId,
    RuntimeClockState, RuntimeDllLifecycleAction, RuntimeDllLifecyclePhase, RuntimeExport,
    RuntimeExportCatalog, RuntimeGuestImportBinding, RuntimeHeapInitialization, RuntimeModule,
    RuntimeModuleId, RuntimeProcessId, RuntimeSchedulerEvent, RuntimeStartupInfo, RuntimeState,
    RuntimeThreadId, SchedulerThreadState, StackCleanup, bind_pe32_imports, build_pe32_load_plan,
    dispatch_kernel32_api_with_policy, install_controlled_initial_stack,
    install_controlled_process_environment_with_command_line,
    install_controlled_process_environment_with_command_line_and_ansi,
    install_controlled_process_environment_with_command_line_ansi_and_environment_strings,
    install_dynamic_export_thunks, install_initial_thread_stack, install_runtime_return_sentinels,
    kernel32_contract_for_endpoint, materialize_pe32_image,
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
const MAPPING_FIXTURE_STAGE_COUNT: u32 = 7;
const BGI_SHA256: &str = "f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1";
const BGI_LOAD_BASE: GuestAddress = GuestAddress(0x1000_0000);
const BGI_MAX_PHYSICAL_PAGES: usize = 400;
const BGI_KERNEL32_HANDLE: u32 = 0x4d01_0001;

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

    fn prepare_bgi(
        &mut self,
        executable_length: usize,
        policy_length: usize,
    ) -> Result<(), String> {
        let total_length = executable_length
            .checked_add(policy_length)
            .ok_or_else(|| "BGI bridge input length overflowed".to_owned())?;
        let input = self
            .input
            .get(..total_length)
            .ok_or_else(|| "BGI input lengths exceed the bridge buffer".to_owned())?;
        let (executable, policy) = input.split_at(executable_length);
        let policy: Kernel32CompatibilityPolicy =
            serde_json::from_slice(policy).map_err(|error| error.to_string())?;
        policy.validate().map_err(error_string)?;
        let (runtime, metadata, reflection) =
            BrowserRuntime::prepare_bgi(executable.to_vec(), policy)?;
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
    endpoint: ApiEndpointIdentity,
}

struct ActiveCallback {
    call: ApiGuestCallbackCall,
    frame_id: RuntimeCallbackFrameId,
    stack_pointer: GuestAddress,
}

#[derive(Default)]
struct BrowserMappingFixture {
    next_stage: u32,
    heap_handle: Option<u32>,
    allocation_address: Option<GuestAddress>,
    backing_owner: Option<vine_core::MemoryOwnerId>,
    backing_page: Option<GuestRange>,
}

#[derive(Clone)]
enum BrowserRuntimeTarget {
    ControlledFixture,
    Bgi {
        kernel32_policy: Box<Kernel32CompatibilityPolicy>,
    },
}

struct BgiRuntimeProcess {
    runtime: RuntimeState,
    binding: RuntimeGuestImportBinding,
    module_handles: Vec<u32>,
    attach_order: Vec<RuntimeModuleId>,
}

type BgiDispatcher = (
    ApiDispatcher,
    BTreeMap<GuestAddress, EndpointRoute>,
    Vec<ApiEndpointIdentity>,
);

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
    mapping_fixture: BrowserMappingFixture,
    target: BrowserRuntimeTarget,
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
        let mut runtime = controlled_runtime_state()?;
        runtime
            .register_initial_thread_stack(&stack)
            .map_err(error_string)?;
        let runtime_stack = runtime
            .thread_stack(runtime.current_thread_id())
            .map_err(error_string)?;
        let stack_allocation = runtime
            .process_address_space_allocator()
            .allocation(runtime_stack.address_allocation_id)
            .map_err(error_string)?;
        let reflection = memory.pending_reflection().map_err(error_string)?;
        let reflection_buffers = reflection.encode_binary().map_err(error_string)?;
        let metadata = json!({
            "kind": "prepared",
            "schemaVersion": 1,
            "workload": "controlled-fixture",
            "source": snapshot.identity(),
            "entryPoint": materialization.entry_point_address.0,
            "fsBase": 0,
            "generation": reflection.invalidation.generation,
            "stackTop": stack.stack_top.0,
            "initialStackPointer": stack.initial_esp.0,
            "initialThreadStack": runtime_stack,
            "initialStackAddressAllocation": stack_allocation,
            "callbackSentinel": sentinels.callback_return.address.0,
            "entrySentinel": sentinels.process_entry_return.address.0,
            "pageCount": reflection.pages.len(),
            "regions": memory.region_snapshots(),
            "endpoints": binding.thunks,
            "runtime": {
                "processId": runtime.process_id().0,
                "threadId": runtime.current_thread_id().0,
                "currentDirectoryA": runtime.current_directory_a(),
                "currentDirectoryGeneration": runtime.current_directory_generation(),
            },
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
                mapping_fixture: BrowserMappingFixture::default(),
                target: BrowserRuntimeTarget::ControlledFixture,
            },
            metadata,
            reflection_buffers,
        ))
    }

    fn prepare_bgi(
        executable: Vec<u8>,
        kernel32_policy: Kernel32CompatibilityPolicy,
    ) -> Result<(Self, Value, vine_core::MemoryReflectionBuffers), String> {
        let loader_limits = Pe32Limits::default();
        let snapshot =
            ModuleSnapshot::from_bytes(executable, &loader_limits).map_err(error_string)?;
        snapshot
            .verify_expected_sha256(BGI_SHA256)
            .map_err(error_string)?;
        let plan = build_pe32_load_plan(&snapshot, loader_limits).map_err(error_string)?;
        let mut memory = GuestMemory::new(MemoryLimits {
            max_physical_pages: BGI_MAX_PHYSICAL_PAGES,
            ..MemoryLimits::default()
        })
        .map_err(error_string)?;
        let materialization = materialize_pe32_image(&mut memory, &snapshot, &plan, BGI_LOAD_BASE)
            .map_err(error_string)?;
        let runtime_catalog = runtime_catalog_for_load_plan(&plan)?;
        let mut process =
            prepare_bgi_runtime_process(&mut memory, &plan, &materialization, &runtime_catalog)?;
        let sentinels = install_runtime_return_sentinels(&mut memory).map_err(error_string)?;
        let stack = install_initial_thread_stack(
            &mut memory,
            &sentinels.process_entry_return,
            plan.load_plan.image.stack_reserve,
            plan.load_plan.image.stack_commit,
        )
        .map_err(error_string)?;
        process
            .runtime
            .register_initial_thread_stack(&stack)
            .map_err(error_string)?;
        let environment = if kernel32_policy.allows_wide_environment_strings() {
            install_controlled_process_environment_with_command_line_ansi_and_environment_strings(
                &mut memory,
                "BGI.exe",
            )
        } else if kernel32_policy.allows_ansi_command_line() {
            install_controlled_process_environment_with_command_line_and_ansi(
                &mut memory,
                "BGI.exe",
            )
        } else {
            install_controlled_process_environment_with_command_line(&mut memory, "BGI.exe")
        }
        .map_err(error_string)?;
        if kernel32_policy.allows_ansi_command_line() {
            process
                .runtime
                .register_initial_process_environment(&environment)
                .map_err(error_string)?;
        }
        let dynamic_thunks = install_dynamic_export_thunks(
            &mut memory,
            &[
                ("kernel32.dll", "FlsAlloc"),
                ("kernel32.dll", "FlsGetValue"),
                ("kernel32.dll", "FlsSetValue"),
                ("kernel32.dll", "FlsFree"),
            ],
        )
        .map_err(error_string)?;
        finalize_bgi_runtime_process(
            &mut process,
            &dynamic_thunks,
            sentinels.callback_return.address,
        )?;
        let (dispatcher, routes, endpoints) = build_bgi_dispatcher(&process, &kernel32_policy)?;
        let runtime_stack = process
            .runtime
            .thread_stack(process.runtime.current_thread_id())
            .map_err(error_string)?;
        let stack_allocation = process
            .runtime
            .process_address_space_allocator()
            .allocation(runtime_stack.address_allocation_id)
            .map_err(error_string)?;
        let reflection = memory.pending_reflection().map_err(error_string)?;
        let reflection_buffers = reflection.encode_binary().map_err(error_string)?;
        let metadata = json!({
            "kind": "prepared",
            "schemaVersion": 1,
            "workload": "private-bgi",
            "source": snapshot.identity(),
            "entryPoint": materialization.entry_point_address.0,
            "fsBase": environment.teb_address.0,
            "generation": reflection.invalidation.generation,
            "contentSha256": reflection.content_sha256,
            "stackTop": stack.stack_top.0,
            "initialStackPointer": stack.initial_esp.0,
            "initialThreadStack": runtime_stack,
            "initialStackAddressAllocation": stack_allocation,
            "callbackSentinel": sentinels.callback_return.address.0,
            "entrySentinel": sentinels.process_entry_return.address.0,
            "pageCount": reflection.pages.len(),
            "regions": memory.region_snapshots(),
            "endpoints": endpoints,
            "runtime": {
                "processId": process.runtime.process_id().0,
                "threadId": process.runtime.current_thread_id().0,
                "currentDirectoryA": process.runtime.current_directory_a(),
                "currentDirectoryGeneration": process.runtime.current_directory_generation(),
                "moduleCount": process.runtime.process().modules.len(),
                "dependencyEdges": process.binding.dependency_edges.len(),
                "attachOrder": process.attach_order,
                "kernel32Policy": kernel32_policy.id,
            },
        });
        Ok((
            Self {
                memory,
                runtime: process.runtime,
                dispatcher,
                routes,
                callback_sentinel: sentinels.callback_return.address,
                entry_sentinel: sentinels.process_entry_return.address,
                stack,
                pending_host: None,
                callbacks: Vec::new(),
                mapping_fixture: BrowserMappingFixture::default(),
                target: BrowserRuntimeTarget::Bgi {
                    kernel32_policy: Box::new(kernel32_policy),
                },
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

    fn advance_mapping_fixture(
        &mut self,
        requested_stage: u32,
    ) -> Result<(Value, MemoryDeltaBuffers), String> {
        if requested_stage != self.mapping_fixture.next_stage
            || requested_stage >= MAPPING_FIXTURE_STAGE_COUNT
        {
            return Err("browser mapping fixture stage is out of order".to_owned());
        }
        let stats = self.memory.stats();
        if stats.generation != stats.acknowledged_generation {
            return Err(
                "browser mapping fixture requires the previous delta acknowledgement".to_owned(),
            );
        }

        let (stage, probe_address) = match requested_stage {
            0 => {
                let handle = self
                    .runtime
                    .allocate_private_heap(0, GUEST_PAGE_SIZE as u32, 0)
                    .map_err(error_string)?
                    .handle;
                let allocation = ExecutionSession::new(&mut self.memory, &mut self.runtime)
                    .allocate_private_heap_block(handle, 0x214, RuntimeHeapInitialization::Zeroed)
                    .map_err(error_string)?;
                let backing = self
                    .runtime
                    .heap_backing(handle)
                    .map_err(error_string)?
                    .ok_or_else(|| "lazy heap allocation did not create a backing".to_owned())?;
                self.mapping_fixture.heap_handle = Some(handle);
                self.mapping_fixture.allocation_address = Some(allocation.address);
                self.mapping_fixture.backing_owner = Some(backing.memory_owner);
                self.mapping_fixture.backing_page = Some(
                    GuestRange::new(backing.range.start(), GUEST_PAGE_SIZE)
                        .map_err(error_string)?,
                );
                ("lazy-heap-map", allocation.address)
            }
            1 => {
                let handle = self.mapping_fixture_heap_handle()?;
                let address = self.mapping_fixture_allocation_address()?;
                ExecutionSession::new(&mut self.memory, &mut self.runtime)
                    .free_private_heap_block(handle, address)
                    .map_err(error_string)?;
                ("heap-dirty-only", address)
            }
            2 => {
                self.memory
                    .protect(
                        self.mapping_fixture_backing_owner()?,
                        self.mapping_fixture_backing_page()?,
                        PageProtection {
                            read: true,
                            write: false,
                            execute: false,
                        },
                    )
                    .map_err(error_string)?;
                (
                    "heap-protect-read-only",
                    self.mapping_fixture_allocation_address()?,
                )
            }
            3 => {
                self.memory
                    .protect(
                        self.mapping_fixture_backing_owner()?,
                        self.mapping_fixture_backing_page()?,
                        PageProtection {
                            read: true,
                            write: true,
                            execute: false,
                        },
                    )
                    .map_err(error_string)?;
                (
                    "heap-protect-read-write",
                    self.mapping_fixture_allocation_address()?,
                )
            }
            4 => {
                self.memory
                    .decommit(
                        self.mapping_fixture_backing_owner()?,
                        self.mapping_fixture_backing_page()?,
                    )
                    .map_err(error_string)?;
                ("heap-decommit", self.mapping_fixture_allocation_address()?)
            }
            5 => {
                self.memory
                    .commit(
                        self.mapping_fixture_backing_owner()?,
                        self.mapping_fixture_backing_page()?,
                        PageProtection {
                            read: true,
                            write: true,
                            execute: false,
                        },
                    )
                    .map_err(error_string)?;
                ("heap-recommit", self.mapping_fixture_allocation_address()?)
            }
            6 => {
                let handle = self.mapping_fixture_heap_handle()?;
                let address = self.mapping_fixture_allocation_address()?;
                ExecutionSession::new(&mut self.memory, &mut self.runtime)
                    .destroy_private_heap(handle)
                    .map_err(error_string)?;
                ("heap-release", address)
            }
            _ => return Err("browser mapping fixture stage is unsupported".to_owned()),
        };

        let delta = self.memory.pending_delta().map_err(error_string)?;
        let mapping_operations = delta
            .mapping_operations
            .iter()
            .map(mapping_operation_name)
            .collect::<Vec<_>>();
        let metadata = json!({
            "kind": "mapping-fixture-delta",
            "stageIndex": requested_stage,
            "stage": stage,
            "fromGeneration": delta.from_generation,
            "generation": delta.generation,
            "mappingOperations": mapping_operations,
            "dirtyPages": delta.dirty_pages.len(),
            "cacheEffects": delta.cache_effects,
            "probeAddress": probe_address.0,
        });
        let buffers = delta.encode_binary().map_err(error_string)?;
        self.mapping_fixture.next_stage += 1;
        Ok((metadata, buffers))
    }

    fn current_reflection(&self) -> Result<(Value, vine_core::MemoryReflectionBuffers), String> {
        let reflection = self.memory.current_reflection().map_err(error_string)?;
        let stats = self.memory.stats();
        let metadata = json!({
            "kind": "current-reflection",
            "generation": reflection.invalidation.generation,
            "acknowledgedGeneration": stats.acknowledged_generation,
            "pageCount": reflection.pages.len(),
            "contentSha256": reflection.content_sha256,
            "regions": self.memory.region_snapshots(),
        });
        let buffers = reflection.encode_binary().map_err(error_string)?;
        Ok((metadata, buffers))
    }

    fn grow_initial_thread_stack(
        &mut self,
        fault_address: GuestAddress,
        page_fault_error_code: u32,
    ) -> Result<(Value, Option<MemoryDeltaBuffers>), String> {
        if !matches!(self.target, BrowserRuntimeTarget::Bgi { .. }) {
            return Err(
                "initial thread stack growth is available only to the BGI runtime".to_owned(),
            );
        }
        let growth = match self.runtime.grow_initial_thread_stack(
            &mut self.memory,
            fault_address,
            page_fault_error_code,
        ) {
            Ok(growth) => growth,
            Err(error) => {
                let stats = self.memory.stats();
                return Ok((
                    json!({
                        "kind": "initial-thread-stack-growth-rejected",
                        "faultAddress": fault_address.0,
                        "pageFaultErrorCode": page_fault_error_code,
                        "error": error,
                        "memory": stats,
                    }),
                    None,
                ));
            }
        };
        let delta = self.memory.pending_delta().map_err(error_string)?;
        let metadata = json!({
            "kind": "initial-thread-stack-growth",
            "growth": growth,
            "fromGeneration": delta.from_generation,
            "generation": delta.generation,
            "mappingOperations": delta.mapping_operations.iter().map(mapping_operation_name).collect::<Vec<_>>(),
            "dirtyPages": delta.dirty_pages.len(),
            "cacheEffects": delta.cache_effects,
        });
        let buffers = delta.encode_binary().map_err(error_string)?;
        Ok((metadata, Some(buffers)))
    }

    fn mapping_fixture_heap_handle(&self) -> Result<u32, String> {
        self.mapping_fixture
            .heap_handle
            .ok_or_else(|| "browser mapping fixture heap is unavailable".to_owned())
    }

    fn mapping_fixture_allocation_address(&self) -> Result<GuestAddress, String> {
        self.mapping_fixture
            .allocation_address
            .ok_or_else(|| "browser mapping fixture allocation is unavailable".to_owned())
    }

    fn mapping_fixture_backing_owner(&self) -> Result<vine_core::MemoryOwnerId, String> {
        self.mapping_fixture
            .backing_owner
            .ok_or_else(|| "browser mapping fixture owner is unavailable".to_owned())
    }

    fn mapping_fixture_backing_page(&self) -> Result<GuestRange, String> {
        self.mapping_fixture
            .backing_page
            .ok_or_else(|| "browser mapping fixture backing page is unavailable".to_owned())
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
        let (route_id, argument_bytes, endpoint) = self
            .routes
            .get(&endpoint_address)
            .map(|route| (route.id, route.argument_bytes, route.endpoint.clone()))
            .ok_or_else(|| format!("unowned API endpoint {:#010x}", endpoint_address.0))?;
        let trap = ApiTrap {
            endpoint_id: vine_core::ImportThunkId(route_id),
            endpoint_address,
            stack_pointer,
            return_address,
            argument_bytes,
        };
        let callback_sentinel = self.callback_sentinel;
        let target = self.target.clone();
        let dispatcher = &self.dispatcher;
        let mut session = ExecutionSession::new(&mut self.memory, &mut self.runtime);
        let outcome =
            dispatcher.dispatch(
                trap,
                &mut session,
                |contract, frame, session| match &target {
                    BrowserRuntimeTarget::Bgi { kernel32_policy } => {
                        dispatch_kernel32_api_with_policy(kernel32_policy, contract, frame, session)
                    }
                    BrowserRuntimeTarget::ControlledFixture => {
                        match contract.implementation_id.as_str() {
                            DEFERRED_ADD | COMMIT_OUTPUTS => {
                                let service = if contract.implementation_id == DEFERRED_ADD {
                                    "browser.deferred-add"
                                } else {
                                    "browser.commit-outputs"
                                };
                                let request = match session
                                    .runtime_mut()
                                    .begin_host_request(service)
                                {
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
                        }
                    }
                },
            );
        match outcome {
            DispatchOutcome::Deferred(call) => {
                let arguments = decode_u32_payload(&call.request.payload);
                let thread_id = self.runtime.current_thread_id();
                match self.runtime.scheduler_state().map_err(error_string)? {
                    SchedulerThreadState::WaitingHost { request_id }
                        if request_id == call.request.request_id => {}
                    _ => {
                        return Err(
                            "Rust scheduler did not authorize the emitted host request".to_owned()
                        );
                    }
                }
                let metadata = json!({
                    "kind": "host-request",
                    "requestId": call.request.request_id.to_string(),
                    "threadId": thread_id.0,
                    "service": call.request.service,
                    "arguments": arguments,
                    "scheduler": {
                        "state": "waiting-host",
                        "requestId": call.request.request_id.to_string(),
                    },
                });
                self.pending_host = Some(call);
                Ok((metadata, None))
            }
            DispatchOutcome::GuestCallback(call) => self.begin_callback(call),
            DispatchOutcome::Immediate(completion) => {
                let delta = self.pending_delta()?;
                let metadata = match &self.target {
                    BrowserRuntimeTarget::ControlledFixture => {
                        completion_metadata("api-complete", &completion)
                    }
                    BrowserRuntimeTarget::Bgi { .. } => {
                        let mut metadata = json!({
                            "kind": "api-complete",
                            "endpoint": endpoint,
                            "completion": completion,
                            "memory": self.memory.stats(),
                        });
                        if completion.implementation_id == "kernel32.set-current-directory-a.v1" {
                            metadata["currentDirectory"] = json!({
                                "directory": self.runtime.current_directory_a(),
                                "generation": self.runtime.current_directory_generation(),
                            });
                        }
                        metadata
                    }
                };
                Ok((metadata, delta))
            }
            DispatchOutcome::Fault(fault) => match &self.target {
                BrowserRuntimeTarget::ControlledFixture => Err(api_fault_string(fault)),
                BrowserRuntimeTarget::Bgi { .. } => {
                    let reflection = self.memory.current_reflection().map_err(error_string)?;
                    Ok((
                        json!({
                            "kind": "api-fault",
                            "endpoint": endpoint,
                            "capabilityCluster": runtime_capability_cluster(&endpoint),
                            "fault": fault,
                            "memory": self.memory.stats(),
                            "memoryContentSha256": reflection.content_sha256,
                        }),
                        None,
                    ))
                }
            },
        }
    }

    fn pending_delta(&mut self) -> Result<Option<MemoryDeltaBuffers>, String> {
        if self.memory.stats().pending_invalidation.is_none() {
            return Ok(None);
        }
        self.memory
            .pending_delta()
            .map_err(error_string)?
            .encode_binary()
            .map(Some)
            .map_err(error_string)
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
        let completion = match dispatcher.complete_deferred(
            call,
            HostCompletion {
                request_id,
                result: ApiImmediate::new(ApiReturnValue::U32(result)),
            },
            &mut session,
        ) {
            DispatchOutcome::Immediate(completion) => Ok(completion),
            DispatchOutcome::Fault(fault) => Err(api_fault_string(fault)),
            _ => Err("deferred API completion produced another suspended outcome".to_owned()),
        }?;
        let thread_id = self.runtime.current_thread_id();
        let scheduler = self.runtime.scheduler_state().map_err(error_string)?;
        if !matches!(
            scheduler,
            SchedulerThreadState::Runnable | SchedulerThreadState::InCallback { .. }
        ) {
            return Err("Rust scheduler did not resume the completed host request".to_owned());
        }
        Ok(json!({
            "kind": "host-complete",
            "threadId": thread_id.0,
            "scheduler": scheduler,
            "completion": completion,
        }))
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
        let cancellation = if matches!(&self.target, BrowserRuntimeTarget::Bgi { .. }) {
            if self.runtime.dll_lifecycle_phase() == RuntimeDllLifecyclePhase::ProcessAttached {
                match self
                    .runtime
                    .begin_dll_process_detach(self.callback_sentinel)
                    .map_err(error_string)?
                {
                    RuntimeDllLifecycleAction::Complete { completion }
                        if completion.phase == RuntimeDllLifecyclePhase::ProcessDetached
                            && completion.callback_order.is_empty() => {}
                    _ => {
                        return Err(
                            "BGI cancellation did not complete callback-free detach".to_owned()
                        );
                    }
                }
            }
            let cancellation = self
                .runtime
                .request_current_process_exit(exit_code)
                .map_err(error_string)?;
            ExecutionSession::new(&mut self.memory, &mut self.runtime)
                .teardown_process_heaps()
                .map_err(error_string)?;
            self.runtime
                .complete_current_process_exit()
                .map_err(error_string)?;
            cancellation
        } else {
            self.runtime
                .terminate_current_process(exit_code)
                .map_err(error_string)?
        };
        Ok(json!({
            "kind": "cancelled",
            "cancellation": cancellation,
            "processLifecycle": self.runtime.process().lifecycle,
            "dllLifecycle": self.runtime.dll_lifecycle_phase(),
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
        if termination.host_requests_cancelled != 0
            || termination.callback_frames_cancelled != 0
            || termination.wait_registrations_cancelled != 0
            || termination.messages_discarded != 0
        {
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

fn mapping_operation_name(operation: &MappingOperation) -> &'static str {
    match operation {
        MappingOperation::Reserve { .. } => "reserve",
        MappingOperation::Commit { .. } => "commit",
        MappingOperation::Protect { .. } => "protect",
        MappingOperation::Decommit { .. } => "decommit",
        MappingOperation::Release { .. } => "release",
    }
}

fn runtime_catalog_for_load_plan(
    loader: &vine_core::LoaderIngressReport,
) -> Result<RuntimeExportCatalog, String> {
    let mut keys = BTreeSet::new();
    let mut exports = Vec::new();
    for module in &loader.load_plan.imports {
        for symbol in &module.symbols {
            let key = match &symbol.target {
                ImportTarget::Name { name, .. } => {
                    format!("{}!name:{name}", module.name.to_ascii_lowercase())
                }
                ImportTarget::Ordinal { ordinal } => {
                    format!("{}!ordinal:{ordinal}", module.name.to_ascii_lowercase())
                }
            };
            if !keys.insert(key) {
                continue;
            }
            exports.push(match &symbol.target {
                ImportTarget::Name { name, .. } => {
                    RuntimeExport::named(module.name.clone(), name.clone())
                }
                ImportTarget::Ordinal { ordinal } => {
                    RuntimeExport::ordinal(module.name.clone(), *ordinal)
                }
            });
        }
    }
    RuntimeExportCatalog::new(exports).map_err(error_string)
}

fn prepare_bgi_runtime_process(
    memory: &mut GuestMemory,
    loader: &vine_core::LoaderIngressReport,
    materialization: &vine_core::ImageMaterialization,
    runtime_catalog: &RuntimeExportCatalog,
) -> Result<BgiRuntimeProcess, String> {
    let mut runtime = controlled_bgi_runtime_state()?;
    let mut module_handles = vec![BGI_KERNEL32_HANDLE];
    let mut canonical_modules = BTreeSet::from(["kernel32.dll".to_owned()]);
    let mut next_module_slot = 2_u16;
    for imported in &loader.load_plan.imports {
        let canonical_name = imported.name.to_ascii_lowercase();
        if !canonical_modules.insert(canonical_name.clone()) {
            continue;
        }
        let handle = bgi_module_handle(next_module_slot)?;
        next_module_slot = next_module_slot
            .checked_add(1)
            .ok_or_else(|| "BGI runtime module handle slots are exhausted".to_owned())?;
        runtime
            .load_module(RuntimeModule {
                owner: "bgi-browser-runtime".to_owned(),
                variant_id: "runtime-system-import-v1".to_owned(),
                namespace: LOGICAL_LOADED_MODULE_NAMESPACE.to_owned(),
                handle,
                requested_name: imported.name.clone(),
                canonical_name,
                runtime_system_module: true,
            })
            .map_err(error_string)?;
        module_handles.push(handle);
    }
    let main_module_handle = bgi_module_handle(next_module_slot)?;
    runtime
        .register_main_module(RuntimeModule {
            owner: "bgi-browser-runtime".to_owned(),
            variant_id: "mapped-main-executable-v1".to_owned(),
            namespace: LOGICAL_LOADED_MODULE_NAMESPACE.to_owned(),
            handle: main_module_handle,
            requested_name: "BGI.exe".to_owned(),
            canonical_name: "bgi.exe".to_owned(),
            runtime_system_module: false,
        })
        .map_err(error_string)?;
    module_handles.push(main_module_handle);
    let binding = ExecutionSession::new(memory, &mut runtime)
        .bind_guest_module_imports(main_module_handle, loader, materialization, runtime_catalog)
        .map_err(error_string)?;
    ExecutionSession::new(memory, &mut runtime)
        .register_module_image(main_module_handle, loader, materialization)
        .map_err(error_string)?;
    Ok(BgiRuntimeProcess {
        runtime,
        binding,
        module_handles,
        attach_order: Vec::new(),
    })
}

fn controlled_bgi_runtime_state() -> Result<RuntimeState, String> {
    let mut startup_info = vec![0; 68];
    startup_info[..4].copy_from_slice(&68_u32.to_le_bytes());
    let mut runtime = RuntimeState::new(RuntimeBootstrap {
        process_id: RuntimeProcessId(PROCESS_ID),
        initial_thread_id: RuntimeThreadId(THREAD_ID),
        initial_last_error: 87,
        address_space: ProcessAddressSpaceLayout::vine_v1(),
        clock: RuntimeClockState {
            wall_clock_file_time: 134_282_016_000_000_000,
            tick_count: 3_600_000,
            performance_counter: 12_345_678_901,
        },
        startup_info: RuntimeStartupInfo {
            profile_id: "startupinfow-v68-zero-optional-fields".to_owned(),
            bytes: startup_info,
        },
        process_image_path_a: vine_core::RuntimeProcessImagePathA {
            source_authority: "browser-private-executable-virtual-path".to_owned(),
            bytes_with_nul: b"C:\\Vine\\BGI.exe\0".to_vec(),
            encoding: vine_core::RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING.to_owned(),
        },
        initial_current_directory_a: vine_core::RuntimeCurrentDirectoryA {
            source_authority: "browser-private-process-bootstrap".to_owned(),
            bytes_with_nul: b"C:\\Vine\0".to_vec(),
            encoding: vine_core::RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING.to_owned(),
        },
        encoded_pointer_namespace: LOGICAL_ENCODED_POINTER_NAMESPACE.to_owned(),
        encoded_pointer_token_prefix: 0xec01_0000,
    })
    .map_err(error_string)?;
    runtime
        .load_module(RuntimeModule {
            owner: "runtime-loaded-modules".to_owned(),
            variant_id: "runtime-system-module-v1".to_owned(),
            namespace: LOGICAL_LOADED_MODULE_NAMESPACE.to_owned(),
            handle: BGI_KERNEL32_HANDLE,
            requested_name: "KERNEL32.DLL".to_owned(),
            canonical_name: "kernel32.dll".to_owned(),
            runtime_system_module: true,
        })
        .map_err(error_string)?;
    runtime.start_initial_thread().map_err(error_string)?;
    Ok(runtime)
}

fn bgi_module_handle(slot: u16) -> Result<u32, String> {
    if slot == 0 {
        return Err("BGI runtime module handle slot cannot be zero".to_owned());
    }
    Ok(0x4d01_0000 | u32::from(slot))
}

fn finalize_bgi_runtime_process(
    process: &mut BgiRuntimeProcess,
    dynamic_thunks: &[vine_core::DynamicExportThunk],
    callback_return_sentinel: GuestAddress,
) -> Result<(), String> {
    for thunk in dynamic_thunks {
        process
            .runtime
            .register_dynamic_export_thunk(thunk)
            .map_err(error_string)?;
    }
    for handle in &process.module_handles {
        process
            .runtime
            .mark_module_ready(*handle)
            .map_err(error_string)?;
    }
    let completion = match process
        .runtime
        .begin_dll_process_attach(callback_return_sentinel)
        .map_err(error_string)?
    {
        RuntimeDllLifecycleAction::Complete { completion } => completion,
        RuntimeDllLifecycleAction::Callback { .. } => {
            return Err("BGI system-module graph unexpectedly requested DllMain".to_owned());
        }
        RuntimeDllLifecycleAction::AttachFailed { .. } => {
            return Err("BGI system-module graph failed process attach".to_owned());
        }
    };
    if completion.phase != RuntimeDllLifecyclePhase::ProcessAttached
        || !completion.callback_order.is_empty()
    {
        return Err("BGI system-module graph did not complete callback-free attach".to_owned());
    }
    process.attach_order = completion.graph_order;
    Ok(())
}

fn build_bgi_dispatcher(
    process: &BgiRuntimeProcess,
    kernel32_policy: &Kernel32CompatibilityPolicy,
) -> Result<BgiDispatcher, String> {
    let mut endpoints = process
        .binding
        .imports
        .iter()
        .filter_map(ApiEndpointIdentity::from_runtime_guest_import)
        .collect::<Vec<_>>();
    if endpoints.len() != process.binding.imports.len() {
        return Err("BGI browser binding contains a non-runtime endpoint".to_owned());
    }
    let mut contracts = endpoints
        .iter()
        .filter(|endpoint| {
            let vine_core::ApiSymbol::Name { name } = &endpoint.symbol else {
                return true;
            };
            match name.as_str() {
                "GetCommandLineA" => kernel32_policy.allows_ansi_command_line(),
                "GetEnvironmentStringsW" | "FreeEnvironmentStringsW" => {
                    kernel32_policy.allows_wide_environment_strings()
                }
                "WideCharToMultiByte" => kernel32_policy.wide_char_to_multi_byte().is_some(),
                "MultiByteToWideChar" => kernel32_policy.multi_byte_to_wide_char().is_some(),
                "GetStringTypeW" => kernel32_policy.get_string_type_w().is_some(),
                "LCMapStringW" => kernel32_policy.lc_map_string_w().is_some(),
                "GetCPInfo" => kernel32_policy.active_code_page_info().is_some(),
                _ => true,
            }
        })
        .filter_map(|endpoint| kernel32_contract_for_endpoint(endpoint.clone()).transpose())
        .collect::<Result<Vec<_>, _>>()
        .map_err(api_fault_string)?;
    for name in ["FlsAlloc", "FlsGetValue", "FlsSetValue", "FlsFree"] {
        let endpoint = ApiEndpointIdentity::from_dynamic_endpoint(
            process
                .runtime
                .dynamic_endpoint_by_name("kernel32.dll", name)
                .map_err(error_string)?,
        );
        if matches!(name, "FlsAlloc" | "FlsGetValue" | "FlsSetValue") {
            let contract = kernel32_contract_for_endpoint(endpoint.clone())
                .map_err(api_fault_string)?
                .ok_or_else(|| format!("BGI dynamic endpoint {name} has no Kernel32 contract"))?;
            contracts.push(contract);
        }
        endpoints.push(endpoint);
    }
    let mut routes = BTreeMap::new();
    for endpoint in &endpoints {
        let argument_bytes = contracts
            .iter()
            .find(|contract| contract.endpoint == *endpoint)
            .map(|contract| contract.abi.argument_bytes().map_err(api_fault_string))
            .transpose()?
            .unwrap_or(0);
        routes.insert(
            endpoint.address,
            EndpointRoute {
                id: endpoint.id.0,
                argument_bytes,
                endpoint: endpoint.clone(),
            },
        );
    }
    let dispatcher = ApiDispatcher::new(endpoints.clone(), contracts).map_err(api_fault_string)?;
    Ok((dispatcher, routes, endpoints))
}

fn runtime_capability_cluster(endpoint: &ApiEndpointIdentity) -> String {
    let module = endpoint.module.to_ascii_lowercase();
    match &endpoint.symbol {
        vine_core::ApiSymbol::Name { name }
            if module == "kernel32.dll"
                && matches!(name.as_str(), "EncodePointer" | "DecodePointer") =>
        {
            "kernel32-pointer-encoding".to_owned()
        }
        vine_core::ApiSymbol::Name { name }
            if module == "kernel32.dll" && name.starts_with("Fls") =>
        {
            "kernel32-fiber-local-storage".to_owned()
        }
        vine_core::ApiSymbol::Name { name }
            if module == "kernel32.dll" && name.contains("CriticalSection") =>
        {
            "kernel32-critical-section-synchronization".to_owned()
        }
        vine_core::ApiSymbol::Name { name }
            if module == "kernel32.dll" && matches!(name.as_str(), "HeapAlloc" | "HeapFree") =>
        {
            "kernel32-private-heap-allocation".to_owned()
        }
        _ => format!("{}-api-contract", module.trim_end_matches(".dll")),
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
                endpoint: endpoint.clone(),
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
    let mut runtime = RuntimeState::new(RuntimeBootstrap {
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
        process_image_path_a: vine_core::RuntimeProcessImagePathA {
            source_authority: "browser-fixture-virtual-path".to_owned(),
            bytes_with_nul: b"C:\\Vine\\browser-vertical.exe\0".to_vec(),
            encoding: vine_core::RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING.to_owned(),
        },
        initial_current_directory_a: vine_core::RuntimeCurrentDirectoryA {
            source_authority: "browser-fixture-process-bootstrap".to_owned(),
            bytes_with_nul: b"C:\\Vine\0".to_vec(),
            encoding: vine_core::RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING.to_owned(),
        },
        encoded_pointer_namespace: LOGICAL_ENCODED_POINTER_NAMESPACE.to_owned(),
        encoded_pointer_token_prefix: 0xec02_0000,
    })
    .map_err(error_string)?;
    runtime.start_initial_thread().map_err(error_string)?;
    Ok(runtime)
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
pub extern "C" fn vine_bridge_prepare_bgi(executable_length: u32, policy_length: u32) -> u32 {
    run_bridge(|bridge| bridge.prepare_bgi(executable_length as usize, policy_length as usize))
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
pub extern "C" fn vine_bridge_advance_mapping_fixture(stage: u32) -> u32 {
    run_bridge(|bridge| {
        let (metadata, delta) = bridge.runtime_mut()?.advance_mapping_fixture(stage)?;
        bridge.set_output(metadata, delta.control, delta.page_bytes)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_current_reflection() -> u32 {
    run_bridge(|bridge| {
        let (metadata, reflection) = bridge.runtime_mut()?.current_reflection()?;
        bridge.set_output(metadata, reflection.control, reflection.page_bytes)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_grow_initial_thread_stack(
    fault_address: u32,
    page_fault_error_code: u32,
) -> u32 {
    run_bridge(|bridge| {
        let (metadata, delta) = bridge
            .runtime_mut()?
            .grow_initial_thread_stack(GuestAddress(fault_address), page_fault_error_code)?;
        let (control, payload) = delta
            .map(|buffers| (buffers.control, buffers.page_bytes))
            .unwrap_or_default();
        bridge.set_output(metadata, control, payload)
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

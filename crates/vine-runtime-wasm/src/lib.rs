#![deny(unsafe_op_in_unsafe_fn)]

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};
use vine_core::{
    AbiSchema, ApiArgumentSchema, ApiBehavior, ApiContract, ApiDeferredCall, ApiDirection,
    ApiDispatcher, ApiEndpointIdentity, ApiFault, ApiGuestCallbackCall, ApiHandlerResult,
    ApiImmediate, ApiImmediateCompletion, ApiReturnSchema, ApiReturnValue, ApiTrap, ApiValueSchema,
    CallbackPolicy, CallingConvention, ComCallTrap, ComDispatchOutcome, CpuWritebackFastReceipt,
    CpuWritebackReceipt, D3d9CompatibilityPolicy, Dinput8CompatibilityPolicy, DispatchOutcome,
    DsoundCompatibilityPolicy, ExecutionSession, ExportTarget, GUEST_PAGE_SIZE,
    Gdi32CompatibilityPolicy, GdiplusCompatibilityPolicy, GuestAddress, GuestCallbackArgument,
    GuestCallbackCompletionPolicy, GuestCallbackRequest, GuestMemory, GuestRange, HostCompletion,
    HostRequest, IMM_ASSOCIATE_CONTEXT_IMPLEMENTATION, IMM_GET_DEFAULT_IME_WND_IMPLEMENTATION,
    Imm32CompatibilityPolicy, ImportBinding, ImportTarget, InitialThreadStack,
    Kernel32CompatibilityPolicy, LOGICAL_ENCODED_POINTER_NAMESPACE,
    LOGICAL_LOADED_MODULE_NAMESPACE, LastErrorPolicy, MappingOperation, MemoryDeltaBuffers,
    MemoryLimits, ModuleSnapshot, PageProtection, Pe32Limits, ProcessAddressAllocationPurpose,
    ProcessAddressAllocationRecord, ProcessAddressSpaceLayout, RUNTIME_WRITABLE_COMMIT_SERVICE,
    ReentrancyPolicy, RegisterUpdate, RuntimeBootstrap, RuntimeCallbackFrameId, RuntimeClockState,
    RuntimeDllLifecycleAction, RuntimeDllLifecyclePhase, RuntimeExport, RuntimeExportCatalog,
    RuntimeGuestImportBinding, RuntimeHandle, RuntimeHeapInitialization, RuntimeModule,
    RuntimeModuleId, RuntimeObjectKind, RuntimeProcessId, RuntimeSchedulerEvent,
    RuntimeSourceReadHostResponse, RuntimeSourceSnapshotManifest, RuntimeStartupInfo, RuntimeState,
    RuntimeStateErrorClass, RuntimeThreadId, RuntimeWritableCommitHostResponse,
    RuntimeWritableProfileManifest, SEND_MESSAGE_A_IMPLEMENTATION, SchedulerThreadState,
    StackCleanup, USER32_WM_IME_CONTROL, User32CompatibilityPolicy, WinmmCompatibilityPolicy,
    bind_pe32_imports, build_pe32_load_plan, complete_kernel32_source_read,
    complete_kernel32_writable_commit, d3d9_contract_for_endpoint, dinput8_contract_for_endpoint,
    dispatch_d3d9_api, dispatch_d3d9_com_call, dispatch_dinput8_api, dispatch_dinput8_com_call,
    dispatch_dsound_api, dispatch_dsound_com_call, dispatch_gdi32_api_with_policy,
    dispatch_gdiplus_api_with_policy, dispatch_imm32_api_with_policy,
    dispatch_kernel32_api_with_policy, dispatch_ole32_api, dispatch_user32_api_with_policy,
    dispatch_winmm_api_with_policy, dsound_contract_for_endpoint, gdi32_contract_for_endpoint,
    gdiplus_contract_for_endpoint, imm32_contract_for_endpoint, install_controlled_initial_stack,
    install_controlled_process_environment_with_command_line, install_dynamic_export_thunks,
    install_initial_thread_stack, install_runtime_return_sentinels, install_thread_stack_at,
    kernel32_contract_for_endpoint, materialize_pe32_image, ole32_contract_for_endpoint,
    user32_contract_for_endpoint, winmm_contract_for_endpoint,
};

const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024;
const FIXTURE_SHA256: &str = "6f3f5d91ee8fe586b70da650c204cc07b1bf2fdbf210b3fa04bc723d09942b56";
const FIXTURE_MODULE: &str = "vine_browser_fixture.dll";
const SOURCE_READ_FIXTURE_SHA256: &str =
    "292d298cb77a8223cb0f6fb66333457b3fab9b8e4f0c1643cf01936b769a1d63";
const LOAD_BASE: GuestAddress = GuestAddress(0x1000_0000);
const PROCESS_ID: u32 = 4096;
const THREAD_ID: u32 = 8192;
const SECOND_THREAD_ID: u32 = 8193;
const SECOND_THREAD_STACK_TOP: GuestAddress = GuestAddress(0x006f_f000);
const SECOND_THREAD_MARKER: u32 = 0x5448_5244;
const SECOND_THREAD_EXIT_CODE: u32 = 7;
const CANCEL_EXIT_CODE: u32 = 0xc000_013a;
const DEFERRED_ADD: &str = "browser-fixture-deferred-add-v1";
const CALL_CALLBACK: &str = "browser-fixture-call-callback-v1";
const COMMIT_OUTPUTS: &str = "browser-fixture-commit-outputs-v1";
const MAPPING_FIXTURE_STAGE_COUNT: u32 = 7;
const FIXTURE_MAX_PHYSICAL_PAGES: usize = 16;
const BGI_SHA256: &str = "f4b2f23ab7cb08d655dca8058863c9bf5f73f36edb188360ab6465fe84c4ffc1";
const BGI_LOAD_BASE: GuestAddress = GuestAddress(0x1000_0000);
const BGI_MAX_PHYSICAL_PAGES: usize = 16 * 1024;
const BGI_KERNEL32_HANDLE: u32 = 0x4d01_0001;

thread_local! {
    static BRIDGE: RefCell<Bridge> = const { RefCell::new(Bridge::new()) };
}

struct Bridge {
    input: Vec<u8>,
    metadata: Vec<u8>,
    control: Vec<u8>,
    payload: Vec<u8>,
    fast_writeback_receipt: Option<CpuWritebackFastReceipt>,
    runtime: Option<BrowserRuntime>,
}

impl Bridge {
    const fn new() -> Self {
        Self {
            input: Vec::new(),
            metadata: Vec::new(),
            control: Vec::new(),
            payload: Vec::new(),
            fast_writeback_receipt: None,
            runtime: None,
        }
    }

    fn clear_output(&mut self) {
        self.metadata.clear();
        self.control.clear();
        self.payload.clear();
        self.fast_writeback_receipt = None;
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
        source_manifest_length: usize,
        writable_profile_length: usize,
    ) -> Result<(), String> {
        let total_length = executable_length
            .checked_add(policy_length)
            .and_then(|length| length.checked_add(source_manifest_length))
            .and_then(|length| length.checked_add(writable_profile_length))
            .ok_or_else(|| "BGI bridge input length overflowed".to_owned())?;
        let input = self
            .input
            .get(..total_length)
            .ok_or_else(|| "BGI input lengths exceed the bridge buffer".to_owned())?;
        let (executable, remaining) = input.split_at(executable_length);
        let (policy, storage) = remaining.split_at(policy_length);
        let (source_manifest, writable_profile) = storage.split_at(source_manifest_length);
        let policy: Kernel32CompatibilityPolicy =
            serde_json::from_slice(policy).map_err(|error| error.to_string())?;
        policy.validate().map_err(error_string)?;
        let source_manifest = (!source_manifest.is_empty())
            .then(|| serde_json::from_slice(source_manifest).map_err(|error| error.to_string()))
            .transpose()?;
        if writable_profile.is_empty() {
            return Err("BGI writable profile snapshot is missing".to_owned());
        }
        let writable_profile: RuntimeWritableProfileManifest =
            serde_json::from_slice(writable_profile).map_err(|error| error.to_string())?;
        let (runtime, metadata, reflection) = BrowserRuntime::prepare_bgi(
            executable.to_vec(),
            policy,
            source_manifest,
            writable_profile,
        )?;
        self.runtime = Some(runtime);
        self.set_output(metadata, reflection.control, reflection.page_bytes)
    }

    fn prepare_source_read(
        &mut self,
        executable_length: usize,
        policy_length: usize,
        source_manifest_length: usize,
    ) -> Result<(), String> {
        let total_length = executable_length
            .checked_add(policy_length)
            .and_then(|length| length.checked_add(source_manifest_length))
            .ok_or_else(|| "source read bridge input length overflowed".to_owned())?;
        let input = self
            .input
            .get(..total_length)
            .ok_or_else(|| "source read input lengths exceed the bridge buffer".to_owned())?;
        let (executable, remaining) = input.split_at(executable_length);
        let (policy, source_manifest) = remaining.split_at(policy_length);
        let policy: Kernel32CompatibilityPolicy =
            serde_json::from_slice(policy).map_err(|error| error.to_string())?;
        policy.validate().map_err(error_string)?;
        let source_manifest: RuntimeSourceSnapshotManifest =
            serde_json::from_slice(source_manifest).map_err(|error| error.to_string())?;
        let (runtime, metadata, reflection) =
            BrowserRuntime::prepare_source_read(executable.to_vec(), policy, source_manifest)?;
        self.runtime = Some(runtime);
        self.set_output(metadata, reflection.control, reflection.page_bytes)
    }

    fn complete_source_read(
        &mut self,
        request_id: u64,
        control_length: usize,
        payload_length: usize,
    ) -> Result<(), String> {
        let total_length = control_length
            .checked_add(payload_length)
            .ok_or_else(|| "source read bridge input length overflowed".to_owned())?;
        let input = self
            .input
            .get(..total_length)
            .ok_or_else(|| "source read input lengths exceed the bridge buffer".to_owned())?;
        let (control, payload) = input.split_at(control_length);
        let response = RuntimeSourceReadHostResponse::decode_binary(request_id, control, payload)
            .map_err(error_string)?;
        let (metadata, delta) = self.runtime_mut()?.complete_source_read(response)?;
        let (control, payload) = delta
            .map(|buffers| (buffers.control, buffers.page_bytes))
            .unwrap_or_default();
        self.set_output(metadata, control, payload)
    }

    fn writable_commit_payload(&mut self, request_id: u64) -> Result<(), String> {
        let (metadata, payload) = self.runtime_mut()?.writable_commit_payload(request_id)?;
        self.set_output(metadata, Vec::new(), payload)
    }

    fn complete_writable_commit(
        &mut self,
        request_id: u64,
        control_length: usize,
    ) -> Result<(), String> {
        let control = self
            .input
            .get(..control_length)
            .ok_or_else(|| "writable commit input length exceeds the bridge buffer".to_owned())?;
        let response = RuntimeWritableCommitHostResponse::decode_binary(request_id, control)
            .map_err(error_string)?;
        let metadata = self.runtime_mut()?.complete_writable_commit(response)?;
        self.set_output(metadata, Vec::new(), Vec::new())
    }

    fn hydrate_writable_profile_object(
        &mut self,
        object_id: u32,
        generation: u64,
        input_length: usize,
    ) -> Result<(), String> {
        let bytes = self
            .input
            .get(..input_length)
            .ok_or_else(|| "writable profile hydration exceeds the bridge buffer".to_owned())?;
        let metadata = self
            .runtime
            .as_mut()
            .ok_or_else(|| "runtime bridge is not prepared".to_owned())?
            .hydrate_writable_profile_object(object_id, generation, bytes)?;
        self.set_output(metadata, Vec::new(), Vec::new())
    }

    fn runtime_mut(&mut self) -> Result<&mut BrowserRuntime, String> {
        self.runtime
            .as_mut()
            .ok_or_else(|| "browser runtime has not been prepared".to_owned())
    }

    fn reconcile_writeback_input(
        &mut self,
        control_length: usize,
        payload_length: usize,
    ) -> Result<(), String> {
        let total = control_length
            .checked_add(payload_length)
            .ok_or_else(|| "writeback input length overflowed".to_owned())?;
        let input = self
            .input
            .get(..total)
            .ok_or_else(|| "writeback input exceeds the bridge buffer".to_owned())?;
        let (control, payload) = input.split_at(control_length);
        let (metadata, _) = self
            .runtime
            .as_mut()
            .ok_or_else(|| "browser runtime has not been prepared".to_owned())?
            .reconcile_writeback(control, payload)?;
        self.set_output(metadata, Vec::new(), Vec::new())
    }

    fn reconcile_writeback_input_fast(
        &mut self,
        control_length: usize,
        payload_length: usize,
    ) -> Result<(), String> {
        let total = control_length
            .checked_add(payload_length)
            .ok_or_else(|| "writeback input length overflowed".to_owned())?;
        let input = self
            .input
            .get(..total)
            .ok_or_else(|| "writeback input exceeds the bridge buffer".to_owned())?;
        let (control, payload) = input.split_at(control_length);
        let receipt = self
            .runtime
            .as_mut()
            .ok_or_else(|| "browser runtime has not been prepared".to_owned())?
            .reconcile_writeback_fast(control, payload)?;
        self.fast_writeback_receipt = Some(receipt);
        Ok(())
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

#[derive(Clone)]
struct PendingHostCall {
    thread_id: RuntimeThreadId,
    call: ApiDeferredCall,
}

struct ControlledGuestThread {
    thread_id: RuntimeThreadId,
    entry_point: GuestAddress,
    initial_stack_pointer: GuestAddress,
    fs_base: GuestAddress,
    return_sentinel: GuestAddress,
    marker_address: GuestAddress,
    marker_value: u32,
    expected_exit_code: u32,
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
    ControlledSourceRead {
        kernel32_policy: Box<Kernel32CompatibilityPolicy>,
    },
    Bgi {
        kernel32_policy: Box<Kernel32CompatibilityPolicy>,
        user32_policy: Box<User32CompatibilityPolicy>,
        gdi32_policy: Box<Gdi32CompatibilityPolicy>,
        gdiplus_policy: Box<GdiplusCompatibilityPolicy>,
        imm32_policy: Box<Imm32CompatibilityPolicy>,
        winmm_policy: Box<WinmmCompatibilityPolicy>,
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
    pending_host: BTreeMap<u64, PendingHostCall>,
    callbacks: BTreeMap<RuntimeThreadId, Vec<ActiveCallback>>,
    controlled_guest_thread: Option<ControlledGuestThread>,
    mapping_fixture: BrowserMappingFixture,
    d3d9_policy: Option<Box<D3d9CompatibilityPolicy>>,
    dinput8_policy: Option<Box<Dinput8CompatibilityPolicy>>,
    dsound_policy: Option<Box<DsoundCompatibilityPolicy>>,
    target: BrowserRuntimeTarget,
}

impl BrowserRuntime {
    fn hydrate_writable_profile_object(
        &mut self,
        object_id: u32,
        generation: u64,
        bytes: &[u8],
    ) -> Result<Value, String> {
        if !matches!(self.target, BrowserRuntimeTarget::Bgi { .. }) {
            return Err("writable profile hydration requires the private BGI runtime".to_owned());
        }
        let sha256 = self
            .runtime
            .hydrate_writable_profile_object(object_id, generation, bytes)
            .map_err(error_string)?
            .sha256
            .clone();
        Ok(json!({
            "kind": "writable-profile-hydrated",
            "objectId": object_id,
            "generation": generation,
            "byteLength": bytes.len(),
            "sha256": sha256,
        }))
    }

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
            max_physical_pages: FIXTURE_MAX_PHYSICAL_PAGES,
            ..MemoryLimits::default()
        })
        .map_err(error_string)?;
        let materialization = materialize_pe32_image(&mut memory, &snapshot, &plan, LOAD_BASE)
            .map_err(error_string)?;
        let second_thread_entry = direct_fixture_export_address(&plan, "fixture_thread_entry")?;
        let second_thread_marker = direct_fixture_export_address(&plan, "fixture_thread_marker")?;
        let mut runtime = controlled_runtime_state()?;
        let user32_policy = User32CompatibilityPolicy::controlled_v20();
        user32_policy.validate().map_err(api_fault_string)?;
        runtime
            .configure_display_profile(
                user32_policy
                    .display_profile()
                    .ok_or_else(|| "controlled User32 policy has no display profile".to_owned())?,
            )
            .map_err(error_string)?;
        let d3d9_policy = D3d9CompatibilityPolicy::controlled_v16();
        d3d9_policy.validate().map_err(api_fault_string)?;
        let mut runtime_exports = [
            "fixture_deferred_add",
            "fixture_call_callback",
            "fixture_commit_outputs",
        ]
        .into_iter()
        .map(|name| RuntimeExport::named(FIXTURE_MODULE, name))
        .collect::<Vec<_>>();
        runtime_exports.push(RuntimeExport::named("d3d9.dll", "Direct3DCreate9"));
        let catalog = RuntimeExportCatalog::new(runtime_exports).map_err(error_string)?;
        let import_allocation =
            control_mapping_allocation(&runtime, ProcessAddressAllocationPurpose::ImportThunks)?;
        let binding = bind_pe32_imports(
            &mut memory,
            &import_allocation,
            &plan,
            &materialization,
            &catalog,
        )
        .map_err(error_string)?;
        if !binding.complete || !binding.image_owner_sealed {
            return Err("browser fixture import binding did not complete".to_owned());
        }
        let callback_allocation =
            control_mapping_allocation(&runtime, ProcessAddressAllocationPurpose::CallbackReturns)?;
        let sentinels = install_runtime_return_sentinels(&mut memory, &callback_allocation)
            .map_err(error_string)?;
        let stack = install_controlled_initial_stack(&mut memory, &sentinels.process_entry_return)
            .map_err(error_string)?;
        let second_stack = install_thread_stack_at(
            &mut memory,
            &sentinels.process_entry_return,
            SECOND_THREAD_STACK_TOP,
            GUEST_PAGE_SIZE,
            GUEST_PAGE_SIZE,
        )
        .map_err(error_string)?;
        let (dispatcher, routes) = build_dispatcher(&binding)?;
        runtime
            .register_initial_thread_stack(&stack)
            .map_err(error_string)?;
        let second_thread_id = RuntimeThreadId(SECOND_THREAD_ID);
        runtime
            .register_thread(second_thread_id, 0)
            .map_err(error_string)?;
        runtime
            .register_thread_stack(second_thread_id, &second_stack)
            .map_err(error_string)?;
        let controlled_guest_thread = ControlledGuestThread {
            thread_id: second_thread_id,
            entry_point: second_thread_entry,
            initial_stack_pointer: second_stack.initial_esp,
            fs_base: GuestAddress(0),
            return_sentinel: sentinels.process_entry_return.address,
            marker_address: second_thread_marker,
            marker_value: SECOND_THREAD_MARKER,
            expected_exit_code: SECOND_THREAD_EXIT_CODE,
        };
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
            "schemaVersion": 2,
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
            "maximumPhysicalPages": FIXTURE_MAX_PHYSICAL_PAGES,
            "regions": memory.region_snapshots(),
            "endpoints": binding.thunks,
            "additionalThreads": [{
                "threadId": controlled_guest_thread.thread_id.0,
                "entryPoint": controlled_guest_thread.entry_point.0,
                "initialStackPointer": controlled_guest_thread.initial_stack_pointer.0,
                "fsBase": controlled_guest_thread.fs_base.0,
                "returnSentinel": controlled_guest_thread.return_sentinel.0,
                "markerAddress": controlled_guest_thread.marker_address.0,
                "expectedMarkerValue": controlled_guest_thread.marker_value,
                "expectedExitCode": controlled_guest_thread.expected_exit_code,
            }],
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
                pending_host: BTreeMap::new(),
                callbacks: BTreeMap::new(),
                controlled_guest_thread: Some(controlled_guest_thread),
                mapping_fixture: BrowserMappingFixture::default(),
                d3d9_policy: Some(Box::new(d3d9_policy)),
                dinput8_policy: None,
                dsound_policy: None,
                target: BrowserRuntimeTarget::ControlledFixture,
            },
            metadata,
            reflection_buffers,
        ))
    }

    fn prepare_bgi(
        executable: Vec<u8>,
        kernel32_policy: Kernel32CompatibilityPolicy,
        source_manifest: Option<RuntimeSourceSnapshotManifest>,
        writable_profile: RuntimeWritableProfileManifest,
    ) -> Result<(Self, Value, vine_core::MemoryReflectionBuffers), String> {
        let loader_limits = Pe32Limits::default();
        let snapshot =
            ModuleSnapshot::from_bytes(executable, &loader_limits).map_err(error_string)?;
        snapshot
            .verify_expected_sha256(BGI_SHA256)
            .map_err(error_string)?;
        let source_identity = snapshot.identity();
        if source_manifest
            .as_ref()
            .is_some_and(|manifest| manifest.title_fingerprint != source_identity.sha256)
        {
            return Err(
                "source snapshot title fingerprint does not match the prepared BGI executable"
                    .to_owned(),
            );
        }
        if writable_profile.title_fingerprint != source_identity.sha256 {
            return Err(
                "writable profile title fingerprint does not match the prepared BGI executable"
                    .to_owned(),
            );
        }
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
        if let Some(source_manifest) = source_manifest {
            process
                .runtime
                .register_source_snapshot(source_manifest)
                .map_err(error_string)?;
        }
        process
            .runtime
            .register_writable_profile(writable_profile)
            .map_err(error_string)?;
        let user32_policy = User32CompatibilityPolicy::controlled_v20();
        user32_policy.validate().map_err(api_fault_string)?;
        let gdi32_policy = Gdi32CompatibilityPolicy::controlled_v6();
        gdi32_policy.validate().map_err(api_fault_string)?;
        let gdiplus_policy = GdiplusCompatibilityPolicy::controlled_v1();
        gdiplus_policy.validate().map_err(api_fault_string)?;
        let imm32_policy = Imm32CompatibilityPolicy::controlled_v1();
        imm32_policy.validate().map_err(api_fault_string)?;
        let winmm_policy = WinmmCompatibilityPolicy::controlled_v1();
        winmm_policy.validate().map_err(api_fault_string)?;
        let d3d9_policy = D3d9CompatibilityPolicy::controlled_v16();
        d3d9_policy.validate().map_err(api_fault_string)?;
        let dinput8_policy = Dinput8CompatibilityPolicy::controlled_v1();
        dinput8_policy.validate().map_err(api_fault_string)?;
        let dsound_policy = DsoundCompatibilityPolicy::controlled_v1();
        dsound_policy.validate().map_err(api_fault_string)?;
        process
            .runtime
            .configure_display_profile(
                user32_policy
                    .display_profile()
                    .ok_or_else(|| "active User32 policy has no display profile".to_owned())?,
            )
            .map_err(error_string)?;
        process
            .runtime
            .configure_input_profile(
                user32_policy
                    .input_profile()
                    .ok_or_else(|| "active User32 policy has no input profile".to_owned())?,
            )
            .map_err(error_string)?;
        let callback_allocation = control_mapping_allocation(
            &process.runtime,
            ProcessAddressAllocationPurpose::CallbackReturns,
        )?;
        let sentinels = install_runtime_return_sentinels(&mut memory, &callback_allocation)
            .map_err(error_string)?;
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
            ExecutionSession::new(&mut memory, &mut process.runtime)
                .install_controlled_process_environment_with_command_line_ansi_and_environment_strings(
                    "BGI.exe",
                )
                .map_err(error_string)
        } else if kernel32_policy.allows_ansi_command_line() {
            ExecutionSession::new(&mut memory, &mut process.runtime)
                .install_controlled_process_environment_with_command_line_and_ansi("BGI.exe")
                .map_err(error_string)
        } else {
            let mut allocator = process.runtime.process_address_space_allocator().clone();
            let allocation = allocator
                .reserve(vine_core::controlled_process_environment_allocation_request(true))
                .map_err(error_string)?;
            install_controlled_process_environment_with_command_line(
                &mut memory,
                &allocation,
                "BGI.exe",
            )
            .map_err(error_string)
        }?;
        let dynamic_allocation = control_mapping_allocation(
            &process.runtime,
            ProcessAddressAllocationPurpose::DynamicExports,
        )?;
        let mut dynamic_exports = vec![
            ("kernel32.dll", "FlsAlloc"),
            ("kernel32.dll", "FlsGetValue"),
            ("kernel32.dll", "FlsSetValue"),
            ("kernel32.dll", "FlsFree"),
        ];
        if let Some(loader) = kernel32_policy.load_library_a() {
            for module in &loader.runtime_system_modules {
                for export in &module.exports {
                    dynamic_exports.push((module.canonical_name.as_str(), export.as_str()));
                }
            }
        }
        let dynamic_thunks =
            install_dynamic_export_thunks(&mut memory, &dynamic_allocation, &dynamic_exports)
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
            "schemaVersion": 2,
            "workload": "private-bgi",
            "source": snapshot.identity(),
            "sourceSnapshot": process.runtime.source_snapshot(),
            "writableProfile": process.runtime.writable_profile(),
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
            "maximumPhysicalPages": BGI_MAX_PHYSICAL_PAGES,
            "regions": memory.region_snapshots(),
            "endpoints": endpoints,
            "runtime": {
                "processId": process.runtime.process_id().0,
                "threadId": process.runtime.current_thread_id().0,
                "currentDirectoryA": process.runtime.current_directory_a(),
                "currentDirectoryGeneration": process.runtime.current_directory_generation(),
                "moduleCount": process.runtime.modules().len(),
                "dependencyEdges": process.binding.dependency_edges.len(),
                "attachOrder": process.attach_order,
                "kernel32Policy": kernel32_policy.id,
                "user32Policy": user32_policy.id,
                "gdi32Policy": gdi32_policy.id,
                "gdiplusPolicy": gdiplus_policy.id,
                "imm32Policy": imm32_policy.id,
                "winmmPolicy": winmm_policy.id,
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
                pending_host: BTreeMap::new(),
                callbacks: BTreeMap::new(),
                controlled_guest_thread: None,
                mapping_fixture: BrowserMappingFixture::default(),
                d3d9_policy: Some(Box::new(d3d9_policy)),
                dinput8_policy: Some(Box::new(dinput8_policy)),
                dsound_policy: Some(Box::new(dsound_policy)),
                target: BrowserRuntimeTarget::Bgi {
                    kernel32_policy: Box::new(kernel32_policy),
                    user32_policy: Box::new(user32_policy),
                    gdi32_policy: Box::new(gdi32_policy),
                    gdiplus_policy: Box::new(gdiplus_policy),
                    imm32_policy: Box::new(imm32_policy),
                    winmm_policy: Box::new(winmm_policy),
                },
            },
            metadata,
            reflection_buffers,
        ))
    }

    fn prepare_source_read(
        executable: Vec<u8>,
        kernel32_policy: Kernel32CompatibilityPolicy,
        source_manifest: RuntimeSourceSnapshotManifest,
    ) -> Result<(Self, Value, vine_core::MemoryReflectionBuffers), String> {
        let loader_limits = Pe32Limits::default();
        let snapshot =
            ModuleSnapshot::from_bytes(executable, &loader_limits).map_err(error_string)?;
        snapshot
            .verify_expected_sha256(SOURCE_READ_FIXTURE_SHA256)
            .map_err(error_string)?;
        let source_identity = snapshot.identity();
        if source_manifest.title_fingerprint != source_identity.sha256 {
            return Err(
                "source snapshot title fingerprint does not match the controlled source fixture"
                    .to_owned(),
            );
        }
        let plan = build_pe32_load_plan(&snapshot, loader_limits).map_err(error_string)?;
        let mut memory = GuestMemory::new(MemoryLimits {
            max_physical_pages: FIXTURE_MAX_PHYSICAL_PAGES,
            ..MemoryLimits::default()
        })
        .map_err(error_string)?;
        let materialization = materialize_pe32_image(&mut memory, &snapshot, &plan, LOAD_BASE)
            .map_err(error_string)?;
        let mut runtime = controlled_runtime_state()?;
        runtime
            .register_source_snapshot(source_manifest)
            .map_err(error_string)?;
        let runtime_catalog = runtime_catalog_for_load_plan(&plan)?;
        let import_allocation =
            control_mapping_allocation(&runtime, ProcessAddressAllocationPurpose::ImportThunks)?;
        let binding = bind_pe32_imports(
            &mut memory,
            &import_allocation,
            &plan,
            &materialization,
            &runtime_catalog,
        )
        .map_err(error_string)?;
        if !binding.complete || !binding.image_owner_sealed {
            return Err("source read fixture import binding did not complete".to_owned());
        }
        let callback_allocation =
            control_mapping_allocation(&runtime, ProcessAddressAllocationPurpose::CallbackReturns)?;
        let sentinels = install_runtime_return_sentinels(&mut memory, &callback_allocation)
            .map_err(error_string)?;
        let stack = install_controlled_initial_stack(&mut memory, &sentinels.process_entry_return)
            .map_err(error_string)?;
        runtime
            .register_initial_thread_stack(&stack)
            .map_err(error_string)?;
        let (dispatcher, routes) = build_source_read_dispatcher(&binding)?;
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
            "schemaVersion": 2,
            "workload": "controlled-source-read",
            "source": source_identity,
            "sourceSnapshot": runtime.source_snapshot(),
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
            "maximumPhysicalPages": FIXTURE_MAX_PHYSICAL_PAGES,
            "regions": memory.region_snapshots(),
            "endpoints": binding.thunks,
            "additionalThreads": [],
            "runtime": {
                "processId": runtime.process_id().0,
                "threadId": runtime.current_thread_id().0,
                "currentDirectoryA": runtime.current_directory_a(),
                "currentDirectoryGeneration": runtime.current_directory_generation(),
                "kernel32Policy": kernel32_policy.id,
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
                pending_host: BTreeMap::new(),
                callbacks: BTreeMap::new(),
                controlled_guest_thread: None,
                mapping_fixture: BrowserMappingFixture::default(),
                d3d9_policy: None,
                dinput8_policy: None,
                dsound_policy: None,
                target: BrowserRuntimeTarget::ControlledSourceRead {
                    kernel32_policy: Box::new(kernel32_policy),
                },
            },
            metadata,
            reflection_buffers,
        ))
    }

    fn acknowledge(&mut self, generation: u64) -> Result<Value, String> {
        self.acknowledge_fast(generation)?;
        Ok(json!({
            "kind": "acknowledged",
            "generation": generation,
            "dirtyPages": 0,
        }))
    }

    fn acknowledge_fast(&mut self, generation: u64) -> Result<(), String> {
        self.memory
            .acknowledge_delta(generation)
            .map_err(error_string)?;
        debug_assert_eq!(self.memory.stats().dirty_pages, 0);
        Ok(())
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
        debug_assert_eq!(self.memory.stats().dirty_pages, 0);
        let metadata = json!({
            "kind": "writeback-reconciled",
            "receipt": receipt,
            "dirtyPages": 0,
        });
        Ok((metadata, receipt))
    }

    fn reconcile_writeback_fast(
        &mut self,
        control: &[u8],
        payload: &[u8],
    ) -> Result<CpuWritebackFastReceipt, String> {
        let receipt = self
            .memory
            .reconcile_cpu_writeback_binary_fast(control, payload)
            .map_err(error_string)?;
        debug_assert_eq!(self.memory.stats().dirty_pages, 0);
        Ok(receipt)
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

    fn service_page_fault(
        &mut self,
        fault_address: GuestAddress,
        page_fault_error_code: u32,
    ) -> Result<(Value, Option<MemoryDeltaBuffers>), String> {
        if !matches!(self.target, BrowserRuntimeTarget::Bgi { .. }) {
            return Err("page-fault service is available only to the BGI runtime".to_owned());
        }
        let resolution = match self.runtime.service_page_fault(
            &mut self.memory,
            fault_address,
            page_fault_error_code,
        ) {
            Ok(resolution) => resolution,
            Err(error) => {
                let stats = self.memory.stats();
                return Ok((
                    json!({
                        "kind": "page-fault-rejected",
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
            "kind": "page-fault-resolved",
            "resolution": resolution,
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
                    BrowserRuntimeTarget::Bgi {
                        kernel32_policy,
                        user32_policy,
                        gdi32_policy,
                        gdiplus_policy,
                        imm32_policy,
                        winmm_policy,
                        ..
                    } => {
                        if contract.endpoint.module.eq_ignore_ascii_case("user32.dll") {
                            dispatch_user32_api_with_policy(
                                user32_policy,
                                callback_sentinel,
                                contract,
                                frame,
                                session,
                            )
                        } else if contract.endpoint.module.eq_ignore_ascii_case("gdi32.dll") {
                            dispatch_gdi32_api_with_policy(
                                gdi32_policy,
                                callback_sentinel,
                                contract,
                                frame,
                                session,
                            )
                        } else if contract.endpoint.module.eq_ignore_ascii_case("gdiplus.dll") {
                            dispatch_gdiplus_api_with_policy(
                                gdiplus_policy,
                                contract,
                                frame,
                                session,
                            )
                        } else if contract.endpoint.module.eq_ignore_ascii_case("imm32.dll") {
                            dispatch_imm32_api_with_policy(imm32_policy, contract, frame, session)
                        } else if contract.endpoint.module.eq_ignore_ascii_case("ole32.dll") {
                            dispatch_ole32_api(contract, frame, session)
                        } else if contract.endpoint.module.eq_ignore_ascii_case("winmm.dll") {
                            dispatch_winmm_api_with_policy(winmm_policy, contract, frame, session)
                        } else if contract.endpoint.module.eq_ignore_ascii_case("d3d9.dll") {
                            dispatch_d3d9_api(contract, frame, session)
                        } else if contract.endpoint.module.eq_ignore_ascii_case("dinput8.dll") {
                            dispatch_dinput8_api(contract, frame, session)
                        } else if contract.endpoint.module.eq_ignore_ascii_case("dsound.dll") {
                            dispatch_dsound_api(contract, frame, session)
                        } else {
                            dispatch_kernel32_api_with_policy(
                                kernel32_policy,
                                contract,
                                frame,
                                session,
                            )
                        }
                    }
                    BrowserRuntimeTarget::ControlledFixture => {
                        if contract.endpoint.module.eq_ignore_ascii_case("d3d9.dll") {
                            dispatch_d3d9_api(contract, frame, session)
                        } else {
                            match contract.implementation_id.as_str() {
                                DEFERRED_ADD | COMMIT_OUTPUTS => {
                                    let service = if contract.implementation_id == DEFERRED_ADD {
                                        "browser.deferred-add"
                                    } else {
                                        "browser.commit-outputs"
                                    };
                                    let request =
                                        match session.runtime_mut().begin_host_request(service) {
                                            Ok(request) => request,
                                            Err(error) => {
                                                return ApiHandlerResult::Fault(
                                                    ApiFault::runtime_defect(
                                                        "browser-runtime",
                                                        "host-request-state-invalid",
                                                        error.to_string(),
                                                    ),
                                                );
                                            }
                                        };
                                    let payload = frame
                                        .arguments
                                        .iter()
                                        .flat_map(|argument| {
                                            argument
                                                .words
                                                .iter()
                                                .flat_map(|word| word.to_le_bytes())
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
                                        arguments: vec![GuestCallbackArgument::U32(value)],
                                        scratch: Vec::new(),
                                        scratch_pointer_relocations: Vec::new(),
                                        return_sentinel: callback_sentinel,
                                        completion_policy: GuestCallbackCompletionPolicy::ReturnU32,
                                    })
                                }
                                _ => ApiHandlerResult::Fault(ApiFault::unsupported(
                                    "browser-runtime",
                                    "fixture-service-unsupported",
                                    "browser fixture reached an unknown service",
                                )),
                            }
                        }
                    }
                    BrowserRuntimeTarget::ControlledSourceRead { kernel32_policy } => {
                        dispatch_kernel32_api_with_policy(kernel32_policy, contract, frame, session)
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
                let request_id = call.request.request_id;
                if self.pending_host.contains_key(&request_id) {
                    return Err(format!("host request ID {request_id} is already pending"));
                }
                let previous = self
                    .pending_host
                    .insert(request_id, PendingHostCall { thread_id, call });
                debug_assert!(previous.is_none());
                Ok((metadata, None))
            }
            DispatchOutcome::GuestCallback(call) => self.begin_callback(call),
            DispatchOutcome::Immediate(completion) => {
                let delta = self.pending_delta()?;
                let mut metadata = match &self.target {
                    BrowserRuntimeTarget::ControlledFixture
                    | BrowserRuntimeTarget::ControlledSourceRead { .. } => {
                        completion_metadata("api-complete", &completion)
                    }
                    BrowserRuntimeTarget::Bgi {
                        user32_policy,
                        gdi32_policy,
                        gdiplus_policy,
                        imm32_policy,
                        ..
                    } => {
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
                        if completion.implementation_id == "kernel32.create-mutex-a.v1"
                            && let RegisterUpdate::Set(handle) = completion.eax
                        {
                            let handle = RuntimeHandle(handle);
                            metadata["mutex"] = json!({
                                "handle": handle,
                                "object": self.runtime.mutex_for_handle(handle).map_err(error_string)?,
                                "objectRecord": self.runtime.object_for_handle(
                                    handle,
                                    RuntimeObjectKind::Mutex,
                                ).map_err(error_string)?,
                                "alreadyExists": completion.last_error == 183,
                            });
                        }
                        let waitable_timer_handle = match completion.implementation_id.as_str() {
                            "kernel32.create-waitable-timer-a.v1" => {
                                let RegisterUpdate::Set(handle) = completion.eax else {
                                    return Err(
                                        "CreateWaitableTimerA completion did not set a handle"
                                            .to_owned(),
                                    );
                                };
                                (handle != 0).then_some(RuntimeHandle(handle))
                            }
                            vine_core::SET_WAITABLE_TIMER_IMPLEMENTATION => {
                                let RegisterUpdate::Set(result) = completion.eax else {
                                    return Err(
                                        "SetWaitableTimer completion did not set BOOL in EAX"
                                            .to_owned(),
                                    );
                                };
                                if result == 0 {
                                    None
                                } else {
                                    Some(RuntimeHandle(
                                        completion.frame.argument_u32(0).ok_or_else(|| {
                                            "SetWaitableTimer completion has no timer handle"
                                                .to_owned()
                                        })?,
                                    ))
                                }
                            }
                            _ => None,
                        };
                        if let Some(handle) = waitable_timer_handle {
                            metadata["waitableTimer"] = json!({
                                "handle": handle,
                                "timer": self
                                    .runtime
                                    .waitable_timer_for_handle(handle)
                                    .map_err(error_string)?,
                                "objectRecord": self.runtime.object_for_handle(
                                    handle,
                                    RuntimeObjectKind::WaitableTimer,
                                ).map_err(error_string)?,
                            });
                        }
                        if completion.implementation_id == vine_core::CREATE_THREAD_IMPLEMENTATION
                            && let RegisterUpdate::Set(handle) = completion.eax
                            && handle != 0
                        {
                            let handle = RuntimeHandle(handle);
                            let object = self
                                .runtime
                                .object_for_handle(handle, RuntimeObjectKind::Thread)
                                .map_err(error_string)?;
                            let vine_core::RuntimeObjectKey::Thread { thread_id } = &object.key
                            else {
                                return Err(
                                    "CreateThread completion resolved to a non-thread identity"
                                        .to_owned(),
                                );
                            };
                            let thread_id = RuntimeThreadId(*thread_id);
                            metadata["createdThread"] = json!({
                                "handle": handle,
                                "object": object,
                                "thread": self.runtime.thread_state(thread_id).map_err(error_string)?,
                                "schedulerState": self
                                    .runtime
                                    .scheduler_state_for_thread(thread_id)
                                    .map_err(error_string)?,
                            });
                        }
                        if completion.implementation_id == vine_core::RESUME_THREAD_IMPLEMENTATION {
                            let handle =
                                RuntimeHandle(completion.frame.argument_u32(0).ok_or_else(
                                    || "ResumeThread completion has no thread handle".to_owned(),
                                )?);
                            if let Ok(object) = self
                                .runtime
                                .object_for_handle(handle, RuntimeObjectKind::Thread)
                                && let vine_core::RuntimeObjectKey::Thread { thread_id } =
                                    &object.key
                            {
                                let thread_id = RuntimeThreadId(*thread_id);
                                metadata["resumedThread"] = json!({
                                    "handle": handle,
                                    "threadId": thread_id,
                                    "previousSuspendCount": match completion.eax {
                                        RegisterUpdate::Set(value) => value,
                                        RegisterUpdate::Preserve => u32::MAX,
                                    },
                                    "schedulerState": self
                                        .runtime
                                        .scheduler_state_for_thread(thread_id)
                                        .map_err(error_string)?,
                                });
                            }
                        }
                        if matches!(
                            completion.implementation_id.as_str(),
                            "user32.load-icon-a.v1" | "user32.load-image-a.v1"
                        ) && let RegisterUpdate::Set(handle) = completion.eax
                        {
                            metadata["icon"] =
                                json!(self.runtime.icon(handle).map_err(error_string)?);
                        }
                        if completion.implementation_id == "user32.load-cursor-a.v1"
                            && let RegisterUpdate::Set(handle) = completion.eax
                        {
                            metadata["cursor"] =
                                json!(self.runtime.cursor(handle).map_err(error_string)?);
                        }
                        if completion.implementation_id == "user32.register-class-ex-a.v1"
                            && let RegisterUpdate::Set(atom) = completion.eax
                        {
                            let atom = u16::try_from(atom).map_err(|_| {
                                "RegisterClassExA completion returned an out-of-range ATOM"
                                    .to_owned()
                            })?;
                            metadata["windowClass"] =
                                json!(self.runtime.window_class(atom).map_err(error_string)?);
                        }
                        if completion.implementation_id == "user32.set-window-long-a.gwl-style.v1" {
                            let handle = completion.frame.argument_u32(0).ok_or_else(|| {
                                "SetWindowLongA completion has no logical HWND argument".to_owned()
                            })?;
                            metadata["window"] =
                                json!(self.runtime.window(handle).map_err(error_string)?);
                        }
                        if completion.implementation_id == "ole32.co-initialize.v1" {
                            metadata["comApartment"] = json!(
                                self.runtime
                                    .current_com_apartment()
                                    .map_err(error_string)?
                                    .copied()
                            );
                        }
                        if let Some(gdi) =
                            bgi_gdi_completion_metadata(&completion, &self.runtime, gdi32_policy)?
                        {
                            metadata["gdi"] = gdi;
                        }
                        if let Some(gdiplus) = bgi_gdiplus_completion_metadata(
                            &completion,
                            &self.runtime,
                            gdiplus_policy,
                        )? {
                            metadata["gdiplus"] = gdiplus;
                        }
                        if let Some(imm32) = bgi_imm32_completion_metadata(
                            &completion,
                            &self.runtime,
                            imm32_policy,
                            user32_policy,
                        )? {
                            metadata["imm32"] = imm32;
                        }
                        if let Some(virtual_memory) = bgi_virtual_memory_completion_metadata(
                            &completion,
                            &self.runtime,
                            &self.memory,
                        )? {
                            metadata["virtualMemory"] = virtual_memory;
                        }
                        metadata
                    }
                };
                if let Some(selection) = self
                    .runtime
                    .complete_api_execution_boundary()
                    .map_err(error_string)?
                {
                    metadata["threadSelection"] = json!(selection);
                }
                Ok((metadata, delta))
            }
            DispatchOutcome::Fault(fault) => match &self.target {
                BrowserRuntimeTarget::ControlledFixture
                | BrowserRuntimeTarget::ControlledSourceRead { .. } => Err(api_fault_string(fault)),
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

    fn resolve_com_endpoint(&self, endpoint_address: GuestAddress) -> Result<Value, String> {
        let live = match self.runtime.resolve_com_method(endpoint_address) {
            Ok(_) => true,
            Err(error) if error.class == RuntimeStateErrorClass::NotFound => false,
            Err(error) => return Err(error_string(error)),
        };
        Ok(json!({
            "kind": "com-endpoint-resolution",
            "endpointAddress": endpoint_address,
            "live": live,
            "memory": self.memory.stats(),
        }))
    }

    fn dispatch_com(
        &mut self,
        endpoint_address: GuestAddress,
        stack_pointer: GuestAddress,
        return_address: GuestAddress,
    ) -> Result<(Value, Option<MemoryDeltaBuffers>), String> {
        let d3d9_policy = self.d3d9_policy.as_deref();
        let dinput8_policy = self.dinput8_policy.as_deref();
        let dsound_policy = self.dsound_policy.as_deref();
        let class_identity = self
            .runtime
            .resolve_com_method(endpoint_address)
            .ok()
            .map(|target| target.object.class_identity);
        let routes_to_d3d9 = class_identity
            .as_deref()
            .is_some_and(|identity| identity.starts_with("d3d9."));
        let routes_to_dinput8 = class_identity
            .as_deref()
            .is_some_and(|identity| identity.starts_with("dinput8."));
        let routes_to_dsound = class_identity
            .as_deref()
            .is_some_and(|identity| identity.starts_with("dsound."));
        let outcome = {
            let mut session = ExecutionSession::new(&mut self.memory, &mut self.runtime);
            let trap = ComCallTrap {
                endpoint_address,
                stack_pointer,
                return_address,
                call_site: return_address,
            };
            if routes_to_d3d9 {
                if let Some(policy) = d3d9_policy {
                    dispatch_d3d9_com_call(policy, trap, &mut session)
                } else {
                    session.dispatch_com_call(trap)
                }
            } else if routes_to_dinput8 {
                if let Some(policy) = dinput8_policy {
                    dispatch_dinput8_com_call(policy, trap, &mut session)
                } else {
                    session.dispatch_com_call(trap)
                }
            } else if routes_to_dsound {
                if let Some(policy) = dsound_policy {
                    dispatch_dsound_com_call(policy, trap, &mut session)
                } else {
                    session.dispatch_com_call(trap)
                }
            } else {
                session.dispatch_com_call(trap)
            }
        };
        match outcome {
            ComDispatchOutcome::Completed(completion) => {
                let delta = self.pending_delta()?;
                let cpu_resume = json!({
                    "instructionPointer": completion.instruction_pointer,
                    "stackPointer": completion.stack_pointer,
                    "eax": {"action": "set", "value": completion.return_value.eax()},
                    "edx": {"action": "preserve"},
                    "eflags": {"action": "preserve"},
                });
                Ok((
                    json!({
                        "kind": "com-complete",
                        "completion": cpu_resume,
                        "call": completion,
                        "memory": self.memory.stats(),
                    }),
                    delta,
                ))
            }
            ComDispatchOutcome::Fault(fault) => {
                if !matches!(&self.target, BrowserRuntimeTarget::Bgi { .. }) {
                    return Err(format!(
                        "COM dispatch fault {}: {}",
                        fault.code, fault.message
                    ));
                }
                let capability_cluster = match fault.target.as_ref() {
                    Some(target) if target.interface == "IDirect3D9" => "d3d9-idirect3d9-method",
                    Some(target) if target.interface == "IDirect3DDevice9" => {
                        "d3d9-idirect3ddevice9-method"
                    }
                    Some(target) if target.interface == "IDirect3DTexture9" => {
                        "d3d9-idirect3dtexture9-method"
                    }
                    Some(target) if target.interface == "IDirect3DSurface9" => {
                        "d3d9-idirect3dsurface9-method"
                    }
                    _ => "com-method-dispatch",
                };
                let reflection = self.memory.current_reflection().map_err(error_string)?;
                Ok((
                    json!({
                        "kind": "com-fault",
                        "endpoint": {
                            "kind": "com-method",
                            "endpointAddress": endpoint_address,
                            "target": fault.target.clone(),
                        },
                        "capabilityCluster": capability_cluster,
                        "fault": fault,
                        "memory": self.memory.stats(),
                        "memoryContentSha256": reflection.content_sha256,
                    }),
                    None,
                ))
            }
        }
    }

    fn pending_delta(&mut self) -> Result<Option<MemoryDeltaBuffers>, String> {
        if self.memory.stats().pending_invalidation.is_none() {
            return Ok(None);
        }
        self.memory
            .pending_delta_binary()
            .map(Some)
            .map_err(error_string)
    }

    fn begin_callback(
        &mut self,
        call: ApiGuestCallbackCall,
    ) -> Result<(Value, Option<MemoryDeltaBuffers>), String> {
        let prepared = ExecutionSession::new(&mut self.memory, &mut self.runtime)
            .begin_guest_callback(&call)
            .map_err(api_fault_string)?;
        let delta = self.memory.pending_delta().map_err(error_string)?;
        let generation = delta.generation;
        let buffers = delta.encode_binary().map_err(error_string)?;
        let thread_id = self.runtime.current_thread_id();
        let callbacks = self.callbacks.entry(thread_id).or_default();
        let depth = callbacks.len() + 1;
        let metadata = json!({
            "kind": "guest-callback",
            "frameId": prepared.frame_id.0,
            "depth": depth,
            "target": call.request.target.0,
            "allocationStart": prepared.allocation_start.0,
            "stackPointer": prepared.stack_pointer.0,
            "scratchAddress": prepared.scratch_address.0,
            "scratchBytes": prepared.scratch_bytes,
            "frameBytes": prepared.frame_bytes,
            "returnSentinel": call.request.return_sentinel.0,
            "generation": generation,
        });
        callbacks.push(ActiveCallback {
            call,
            frame_id: prepared.frame_id,
            stack_pointer: prepared.stack_pointer,
        });
        Ok((metadata, Some(buffers)))
    }

    fn complete_host(&mut self, request_id: u64, result: u32) -> Result<Value, String> {
        let pending = self
            .pending_host
            .remove(&request_id)
            .ok_or_else(|| "host completion has no pending API call".to_owned())?;
        if pending.call.request.request_id != request_id {
            self.pending_host.insert(request_id, pending);
            return Err("host completion request identity is corrupt".to_owned());
        }
        let previous_thread_id = self.runtime.current_thread_id();
        let owner_thread_id = pending.thread_id;
        let runtime_completion = match self.runtime.complete_host_request(request_id) {
            Ok(completion) => completion,
            Err(error) => {
                self.pending_host.insert(request_id, pending);
                return Err(error_string(error));
            }
        };
        match self.runtime.pop_scheduler_event() {
            Some(RuntimeSchedulerEvent::HostCompleted(completion))
                if completion == runtime_completion => {}
            _ => return Err("host completion scheduler event mismatch".to_owned()),
        }
        if self.runtime.current_thread_id() != owner_thread_id {
            self.runtime
                .select_thread(owner_thread_id)
                .map_err(error_string)?;
        }
        let dispatcher = &self.dispatcher;
        let mut session = ExecutionSession::new(&mut self.memory, &mut self.runtime);
        let completion = match dispatcher.complete_deferred(
            pending.call,
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
        let scheduler = self
            .runtime
            .scheduler_state_for_thread(owner_thread_id)
            .map_err(error_string)?;
        if !scheduler.is_executable() {
            return Err("Rust scheduler did not resume the completed host request".to_owned());
        }
        if previous_thread_id != owner_thread_id
            && self
                .runtime
                .thread_state(previous_thread_id)
                .is_ok_and(|thread| {
                    thread.lifecycle == vine_core::RuntimeThreadLifecycleState::Running
                })
        {
            self.runtime
                .select_thread(previous_thread_id)
                .map_err(error_string)?;
        }
        Ok(json!({
            "kind": "host-complete",
            "threadId": owner_thread_id.0,
            "scheduler": scheduler,
            "completion": completion,
        }))
    }

    fn complete_source_read(
        &mut self,
        response: RuntimeSourceReadHostResponse,
    ) -> Result<(Value, Option<MemoryDeltaBuffers>), String> {
        let request_id = response.request_id;
        let pending = self
            .pending_host
            .get(&request_id)
            .cloned()
            .ok_or_else(|| "source read completion has no pending API call".to_owned())?;
        if pending.call.request.request_id != request_id {
            return Err("source read completion request identity is corrupt".to_owned());
        }
        let endpoint = pending.call.frame.endpoint.clone();
        let previous_thread_id = self.runtime.current_thread_id();
        let owner_thread_id = pending.thread_id;
        if previous_thread_id != owner_thread_id {
            self.runtime
                .select_thread(owner_thread_id)
                .map_err(error_string)?;
        }
        let dispatcher = &self.dispatcher;
        let (outcome, source_completion) = complete_kernel32_source_read(
            dispatcher,
            pending.call.clone(),
            response,
            &mut ExecutionSession::new(&mut self.memory, &mut self.runtime),
        );
        let completion = match outcome {
            DispatchOutcome::Immediate(completion) => completion,
            DispatchOutcome::Fault(fault) => {
                restore_selected_thread(&mut self.runtime, previous_thread_id, owner_thread_id)?;
                return Err(api_fault_string(fault));
            }
            _ => {
                restore_selected_thread(&mut self.runtime, previous_thread_id, owner_thread_id)?;
                return Err("source read completion produced a suspended outcome".to_owned());
            }
        };
        let source_completion = source_completion.ok_or_else(|| {
            "source read API completed without committing its VFS state".to_owned()
        })?;
        match self.runtime.pop_scheduler_event() {
            Some(RuntimeSchedulerEvent::HostCompleted(completion))
                if completion.request_id == request_id
                    && completion.thread_id == owner_thread_id => {}
            _ => return Err("source read scheduler completion event mismatch".to_owned()),
        }
        let removed = self
            .pending_host
            .remove(&request_id)
            .ok_or_else(|| "source read pending call disappeared before commit".to_owned())?;
        debug_assert_eq!(removed.call.request.request_id, request_id);
        let scheduler = self
            .runtime
            .scheduler_state_for_thread(owner_thread_id)
            .map_err(error_string)?;
        if !scheduler.is_executable() {
            return Err("Rust scheduler did not resume the source read request".to_owned());
        }
        restore_selected_thread(&mut self.runtime, previous_thread_id, owner_thread_id)?;
        let delta = self.pending_delta()?;
        Ok((
            json!({
                "kind": "host-complete",
                "threadId": owner_thread_id.0,
                "scheduler": scheduler,
                "endpoint": endpoint,
                "completion": completion,
                "sourceRead": source_completion,
                "memory": self.memory.stats(),
            }),
            delta,
        ))
    }

    fn writable_commit_payload(&self, request_id: u64) -> Result<(Value, Vec<u8>), String> {
        let pending = self
            .pending_host
            .get(&request_id)
            .ok_or_else(|| "writable commit payload has no pending API call".to_owned())?;
        if pending.call.request.request_id != request_id
            || pending.call.request.service != RUNTIME_WRITABLE_COMMIT_SERVICE
        {
            return Err("writable commit payload request identity is corrupt".to_owned());
        }
        let request = self
            .runtime
            .writable_file_commit_request(request_id)
            .map_err(error_string)?;
        if pending.call.request.payload != request.encode_host_payload() {
            return Err("writable commit API payload disagrees with VFS authority".to_owned());
        }
        let payload = self
            .runtime
            .writable_file_commit_bytes(request_id)
            .map_err(error_string)?
            .to_vec();
        if u64::try_from(payload.len()) != Ok(request.byte_length) {
            return Err("writable commit content length disagrees with VFS authority".to_owned());
        }
        Ok((
            json!({
                "kind": "writable-commit-payload",
                "requestId": request_id.to_string(),
                "request": request,
            }),
            payload,
        ))
    }

    fn complete_writable_commit(
        &mut self,
        response: RuntimeWritableCommitHostResponse,
    ) -> Result<Value, String> {
        let request_id = response.request_id;
        let pending = self
            .pending_host
            .get(&request_id)
            .cloned()
            .ok_or_else(|| "writable commit completion has no pending API call".to_owned())?;
        if pending.call.request.request_id != request_id
            || pending.call.request.service != RUNTIME_WRITABLE_COMMIT_SERVICE
        {
            return Err("writable commit completion request identity is corrupt".to_owned());
        }
        let endpoint = pending.call.frame.endpoint.clone();
        let previous_thread_id = self.runtime.current_thread_id();
        let owner_thread_id = pending.thread_id;
        if previous_thread_id != owner_thread_id {
            self.runtime
                .select_thread(owner_thread_id)
                .map_err(error_string)?;
        }
        let (outcome, writable_completion) = complete_kernel32_writable_commit(
            &self.dispatcher,
            pending.call.clone(),
            response,
            &mut ExecutionSession::new(&mut self.memory, &mut self.runtime),
        );
        let completion = match outcome {
            DispatchOutcome::Immediate(completion) => completion,
            DispatchOutcome::Fault(fault) => {
                restore_selected_thread(&mut self.runtime, previous_thread_id, owner_thread_id)?;
                return Err(api_fault_string(fault));
            }
            _ => {
                restore_selected_thread(&mut self.runtime, previous_thread_id, owner_thread_id)?;
                return Err("writable commit completion produced a suspended outcome".to_owned());
            }
        };
        let writable_completion = writable_completion.ok_or_else(|| {
            "writable commit API completed without committing its VFS state".to_owned()
        })?;
        match self.runtime.pop_scheduler_event() {
            Some(RuntimeSchedulerEvent::HostCompleted(completion))
                if completion.request_id == request_id
                    && completion.thread_id == owner_thread_id => {}
            _ => return Err("writable commit scheduler completion event mismatch".to_owned()),
        }
        let removed = self
            .pending_host
            .remove(&request_id)
            .ok_or_else(|| "writable commit pending call disappeared before commit".to_owned())?;
        debug_assert_eq!(removed.call.request.request_id, request_id);
        let scheduler = self
            .runtime
            .scheduler_state_for_thread(owner_thread_id)
            .map_err(error_string)?;
        if !scheduler.is_executable() {
            return Err("Rust scheduler did not resume the writable commit request".to_owned());
        }
        restore_selected_thread(&mut self.runtime, previous_thread_id, owner_thread_id)?;
        Ok(json!({
            "kind": "host-complete",
            "threadId": owner_thread_id.0,
            "scheduler": scheduler,
            "endpoint": endpoint,
            "completion": completion,
            "writableCommit": writable_completion,
            "memory": self.memory.stats(),
        }))
    }

    fn complete_callback(
        &mut self,
        sentinel: GuestAddress,
        result: u32,
    ) -> Result<(Value, Option<MemoryDeltaBuffers>), String> {
        let thread_id = self.runtime.current_thread_id();
        let (call, frame_id, callback_stack_pointer, remaining_depth) = {
            let callbacks = self
                .callbacks
                .get(&thread_id)
                .ok_or_else(|| "callback return has no active thread frame".to_owned())?;
            let active = callbacks
                .last()
                .ok_or_else(|| "callback return has no active frame".to_owned())?;
            if sentinel != active.call.request.return_sentinel {
                return Err("callback return sentinel mismatch".to_owned());
            }
            (
                active.call.clone(),
                active.frame_id,
                active.stack_pointer,
                callbacks.len() - 1,
            )
        };
        self.runtime
            .validate_callback_frame_return(frame_id)
            .map_err(error_string)?;
        let endpoint = call.frame.endpoint.clone();
        let completion_policy = call.request.completion_policy;
        let dispatcher = &self.dispatcher;
        let outcome = {
            let mut session = ExecutionSession::new(&mut self.memory, &mut self.runtime);
            dispatcher.complete_guest_callback_result(call, result, &mut session)
        };
        match outcome {
            DispatchOutcome::Immediate(completion) => {
                let semantic_action = {
                    let mut session = ExecutionSession::new(&mut self.memory, &mut self.runtime);
                    session
                        .runtime_mut()
                        .pop_callback_frame(frame_id)
                        .map_err(error_string)?;
                    session
                        .commit_guest_callback_semantics(completion_policy, result, &endpoint)
                        .map_err(api_fault_string)?
                };
                let retired = self
                    .callbacks
                    .get_mut(&thread_id)
                    .and_then(Vec::pop)
                    .ok_or_else(|| {
                        "validated callback adapter frame disappeared before commit".to_owned()
                    })?;
                if retired.frame_id != frame_id {
                    return Err("callback adapter frame changed after return preflight".to_owned());
                }
                if remaining_depth == 0 {
                    self.callbacks.remove(&thread_id);
                }
                let delta = self.pending_delta()?;
                let mut metadata = json!({
                    "kind": "callback-complete",
                    "frameId": frame_id.0,
                    "callbackStackPointer": callback_stack_pointer.0,
                    "remainingDepth": remaining_depth,
                    "endpoint": endpoint,
                    "completion": completion,
                    "memory": self.memory.stats(),
                });
                match semantic_action {
                    Some(vine_core::RuntimeGuestCallbackSemanticAction::WindowCreation(
                        vine_core::RuntimeWindowCreationCallbackAction::Created { window },
                    ))
                    | Some(vine_core::RuntimeGuestCallbackSemanticAction::SetWindowPos(
                        vine_core::RuntimeSetWindowPosCallbackAction::Completed { window },
                    )) => metadata["window"] = json!(window),
                    _ => {}
                }
                if let BrowserRuntimeTarget::Bgi {
                    gdi32_policy,
                    gdiplus_policy,
                    ..
                } = &self.target
                {
                    if let Some(gdi) =
                        bgi_gdi_completion_metadata(&completion, &self.runtime, gdi32_policy)?
                    {
                        metadata["gdi"] = gdi;
                    }
                    if let Some(gdiplus) =
                        bgi_gdiplus_completion_metadata(&completion, &self.runtime, gdiplus_policy)?
                    {
                        metadata["gdiplus"] = gdiplus;
                    }
                }
                Ok((metadata, delta))
            }
            DispatchOutcome::GuestCallback(next) => {
                let (prepared, semantic_action) = {
                    let mut session = ExecutionSession::new(&mut self.memory, &mut self.runtime);
                    let prepared = session
                        .replace_guest_callback(&next, frame_id)
                        .map_err(api_fault_string)?;
                    let semantic_action = session
                        .commit_guest_callback_semantics(completion_policy, result, &endpoint)
                        .map_err(api_fault_string)?;
                    (prepared, semantic_action)
                };
                if !matches!(
                    semantic_action,
                    Some(
                        vine_core::RuntimeGuestCallbackSemanticAction::WindowCreation(
                            vine_core::RuntimeWindowCreationCallbackAction::Continue { .. }
                        )
                    ) | Some(vine_core::RuntimeGuestCallbackSemanticAction::SetWindowPos(
                        vine_core::RuntimeSetWindowPosCallbackAction::Continue { .. }
                    )) | Some(
                        vine_core::RuntimeGuestCallbackSemanticAction::SetWindowPosDefault(
                            vine_core::RuntimeSetWindowPosDefaultCallbackAction::Continue { .. },
                        )
                    )
                ) {
                    return Err(
                        "replacement guest callback did not commit continuation semantics"
                            .to_owned(),
                    );
                }
                let callbacks = self
                    .callbacks
                    .get_mut(&thread_id)
                    .ok_or_else(|| "callback replacement lost its thread stack".to_owned())?;
                let depth = callbacks.len();
                let active = callbacks
                    .last_mut()
                    .ok_or_else(|| "callback replacement lost its active frame".to_owned())?;
                if active.frame_id != frame_id {
                    return Err("callback adapter frame changed before replacement".to_owned());
                }
                *active = ActiveCallback {
                    call: next.clone(),
                    frame_id: prepared.frame_id,
                    stack_pointer: prepared.stack_pointer,
                };
                let delta = self.memory.pending_delta().map_err(error_string)?;
                let generation = delta.generation;
                let buffers = delta.encode_binary().map_err(error_string)?;
                Ok((
                    json!({
                        "kind": "guest-callback",
                        "frameId": prepared.frame_id.0,
                        "replacedFrameId": frame_id.0,
                        "depth": depth,
                        "target": next.request.target.0,
                        "allocationStart": prepared.allocation_start.0,
                        "stackPointer": prepared.stack_pointer.0,
                        "scratchAddress": prepared.scratch_address.0,
                        "scratchBytes": prepared.scratch_bytes,
                        "frameBytes": prepared.frame_bytes,
                        "returnSentinel": next.request.return_sentinel.0,
                        "generation": generation,
                    }),
                    Some(buffers),
                ))
            }
            DispatchOutcome::Fault(fault) => Err(api_fault_string(fault)),
            DispatchOutcome::Deferred(_) => {
                Err("callback completion produced a deferred outcome".to_owned())
            }
        }
    }

    fn schedule_next_thread(&mut self) -> Result<Value, String> {
        let previous_thread_id = self.runtime.current_thread_id();
        let Some(selection) = self
            .runtime
            .select_next_executable_thread()
            .map_err(error_string)?
        else {
            return Ok(json!({
                "kind": "scheduler-idle",
                "previousThreadId": previous_thread_id.0,
            }));
        };
        Ok(json!({
            "kind": "thread-scheduled",
            "previousThreadId": selection.previous_thread_id.0,
            "threadId": selection.thread_id.0,
            "scheduler": selection.state,
        }))
    }

    fn finish_thread(
        &mut self,
        exit_code: u32,
    ) -> Result<(Value, Option<vine_core::MemoryDeltaBuffers>), String> {
        let thread_id = self.runtime.current_thread_id();
        let controlled = self
            .controlled_guest_thread
            .as_ref()
            .filter(|controlled| controlled.thread_id == thread_id);
        let marker_value = if let Some(controlled) = controlled {
            if exit_code != controlled.expected_exit_code {
                return Err(format!(
                    "controlled guest thread returned {exit_code}, expected {}",
                    controlled.expected_exit_code
                ));
            }
            let marker_bytes = self
                .memory
                .read(controlled.marker_address, 4)
                .map_err(error_string)?;
            let marker_value = u32::from_le_bytes(
                marker_bytes
                    .as_slice()
                    .try_into()
                    .map_err(|_| "controlled thread marker length mismatch".to_owned())?,
            );
            if marker_value != controlled.marker_value {
                return Err(format!(
                    "controlled guest thread marker is {marker_value:#010x}, expected {:#010x}",
                    controlled.marker_value
                ));
            }
            Some(marker_value)
        } else {
            let thread = self.runtime.thread_state(thread_id).map_err(error_string)?;
            if thread.guest_bootstrap.is_none() {
                return Err("guest thread exit has no runtime bootstrap authority".to_owned());
            }
            None
        };
        let cancellation = self
            .runtime
            .request_current_thread_exit(exit_code)
            .map_err(error_string)?;
        if cancellation.host_requests_cancelled != 0
            || cancellation.callback_frames_cancelled != 0
            || cancellation.wait_registrations_cancelled != 0
            || cancellation.messages_discarded != 0
            || cancellation.scheduler_events_discarded != 0
        {
            return Err("controlled guest thread exit discarded pending work".to_owned());
        }
        self.runtime
            .complete_current_guest_thread_exit(&mut self.memory)
            .map_err(error_string)?;
        let scheduler = self
            .runtime
            .scheduler_state_for_thread(thread_id)
            .map_err(error_string)?;
        let mut metadata = json!({
            "kind": "thread-exit",
            "threadId": thread_id.0,
            "exitCode": exit_code,
            "scheduler": {
                "state": "terminated",
                "exitCode": match scheduler {
                    SchedulerThreadState::Terminated { exit_code } => exit_code,
                    _ => return Err("controlled guest thread did not reach terminated scheduler state".to_owned()),
                },
            },
        });
        if let (Some(controlled), Some(marker_value)) = (controlled, marker_value) {
            metadata["markerAddress"] = json!(controlled.marker_address.0);
            metadata["markerValue"] = json!(marker_value);
        }
        let delta = if controlled.is_none() {
            Some(
                self.memory
                    .pending_delta()
                    .map_err(error_string)?
                    .encode_binary()
                    .map_err(error_string)?,
            )
        } else {
            None
        };
        Ok((metadata, delta))
    }

    fn cancel(&mut self, exit_code: u32) -> Result<Value, String> {
        self.pending_host.clear();
        self.callbacks.clear();
        if matches!(&self.target, BrowserRuntimeTarget::ControlledFixture)
            && self.runtime.current_thread_id() != RuntimeThreadId(THREAD_ID)
            && self
                .runtime
                .thread_state(RuntimeThreadId(THREAD_ID))
                .is_ok_and(|thread| {
                    thread.lifecycle == vine_core::RuntimeThreadLifecycleState::Running
                })
        {
            self.runtime
                .select_thread(RuntimeThreadId(THREAD_ID))
                .map_err(error_string)?;
        }
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
            "processLifecycle": self.runtime.process().lifecycle(),
            "dllLifecycle": self.runtime.dll_lifecycle_phase(),
            "pendingHostCalls": self.pending_host.len(),
            "pendingCallbacks": self.pending_callback_count(),
        }))
    }

    fn finish(&mut self, exit_code: u32) -> Result<Value, String> {
        if !self.pending_host.is_empty() || self.pending_callback_count() != 0 {
            return Err("process exit has suspended host or callback work".to_owned());
        }
        if let Some(controlled) = &self.controlled_guest_thread {
            if self.runtime.current_thread_id() == controlled.thread_id {
                return Err(
                    "controlled process exit cannot originate from the additional thread"
                        .to_owned(),
                );
            }
            if self
                .runtime
                .scheduler_state_for_thread(controlled.thread_id)
                .map_err(error_string)?
                != (SchedulerThreadState::Terminated {
                    exit_code: controlled.expected_exit_code,
                })
            {
                return Err(
                    "controlled process exit preceded additional thread completion".to_owned(),
                );
            }
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

    fn pending_callback_count(&self) -> usize {
        self.callbacks.values().map(Vec::len).sum()
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
            performance_frequency: 10_000_000,
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
                "LoadLibraryA" => kernel32_policy.load_library_a().is_some(),
                _ => true,
            }
        })
        .filter_map(|endpoint| bgi_contract_for_endpoint(endpoint.clone()).transpose())
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
    if let Some(loader) = kernel32_policy.load_library_a() {
        for module in &loader.runtime_system_modules {
            for export in &module.exports {
                endpoints.push(ApiEndpointIdentity::from_dynamic_endpoint(
                    process
                        .runtime
                        .dynamic_endpoint_by_name(&module.canonical_name, export)
                        .map_err(error_string)?,
                ));
            }
        }
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

fn bgi_contract_for_endpoint(
    endpoint: ApiEndpointIdentity,
) -> Result<Option<ApiContract>, ApiFault> {
    if let Some(contract) = kernel32_contract_for_endpoint(endpoint.clone())? {
        return Ok(Some(contract));
    }
    if let Some(contract) = user32_contract_for_endpoint(endpoint.clone())? {
        return Ok(Some(contract));
    }
    if let Some(contract) = gdi32_contract_for_endpoint(endpoint.clone())? {
        return Ok(Some(contract));
    }
    if let Some(contract) = gdiplus_contract_for_endpoint(endpoint.clone())? {
        return Ok(Some(contract));
    }
    if let Some(contract) = imm32_contract_for_endpoint(endpoint.clone())? {
        return Ok(Some(contract));
    }
    if let Some(contract) = ole32_contract_for_endpoint(endpoint.clone())? {
        return Ok(Some(contract));
    }
    if let Some(contract) = d3d9_contract_for_endpoint(endpoint.clone())? {
        return Ok(Some(contract));
    }
    if let Some(contract) = dinput8_contract_for_endpoint(endpoint.clone())? {
        return Ok(Some(contract));
    }
    if let Some(contract) = dsound_contract_for_endpoint(endpoint.clone())? {
        return Ok(Some(contract));
    }
    winmm_contract_for_endpoint(endpoint)
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

fn direct_fixture_export_address(
    loader: &vine_core::LoaderIngressReport,
    name: &str,
) -> Result<GuestAddress, String> {
    let exports = loader
        .load_plan
        .exports
        .as_ref()
        .ok_or_else(|| "browser fixture has no export directory".to_owned())?;
    let symbol = exports
        .symbols
        .iter()
        .find(|symbol| symbol.names.iter().any(|candidate| candidate == name))
        .ok_or_else(|| format!("browser fixture export {name} is missing"))?;
    let ExportTarget::Address { rva } = &symbol.target else {
        return Err(format!("browser fixture export {name} is forwarded"));
    };
    LOAD_BASE
        .0
        .checked_add(rva.0)
        .map(GuestAddress)
        .ok_or_else(|| format!("browser fixture export {name} address overflowed"))
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
        if let Some(contract) =
            d3d9_contract_for_endpoint(endpoint.clone()).map_err(api_fault_string)?
        {
            routes.insert(
                endpoint.address,
                EndpointRoute {
                    id: endpoint.id.0,
                    argument_bytes: contract.abi.argument_bytes().map_err(api_fault_string)?,
                    endpoint: endpoint.clone(),
                },
            );
            contracts.push(contract);
            continue;
        }
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

fn build_source_read_dispatcher(
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
        let contract = kernel32_contract_for_endpoint(endpoint.clone())
            .map_err(api_fault_string)?
            .ok_or_else(|| {
                "controlled source fixture import has no Kernel32 contract".to_owned()
            })?;
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
            performance_frequency: 10_000_000,
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

fn control_mapping_allocation(
    runtime: &RuntimeState,
    purpose: ProcessAddressAllocationPurpose,
) -> Result<ProcessAddressAllocationRecord, String> {
    runtime
        .process_address_space_allocator()
        .allocation_for_purpose(&purpose)
        .cloned()
        .map_err(error_string)
}

fn completion_metadata(kind: &str, completion: &ApiImmediateCompletion) -> Value {
    json!({
        "kind": kind,
        "completion": completion,
    })
}

fn bgi_imm32_completion_metadata(
    completion: &ApiImmediateCompletion,
    runtime: &RuntimeState,
    imm32_policy: &Imm32CompatibilityPolicy,
    user32_policy: &User32CompatibilityPolicy,
) -> Result<Option<Value>, String> {
    let eax = || match completion.eax {
        RegisterUpdate::Set(value) => Ok(value),
        RegisterUpdate::Preserve => {
            Err("IMM32 completion did not set the expected EAX result".to_owned())
        }
    };
    let app_association = |app_window_handle: u32| {
        runtime
            .ime_context_associations()
            .values()
            .find(|association| association.window_handle == app_window_handle)
            .ok_or_else(|| "completed IMM32 call has no logical app-window association".to_owned())
    };
    let associated_context = |association: &vine_core::RuntimeImeContextAssociation| {
        association
            .context
            .map(|reference| runtime.ime_context(reference.handle).map_err(error_string))
            .transpose()
    };

    match completion.implementation_id.as_str() {
        IMM_GET_DEFAULT_IME_WND_IMPLEMENTATION => {
            let app_window_handle = completion
                .frame
                .argument_u32(0)
                .ok_or_else(|| "ImmGetDefaultIMEWnd completion has no app HWND".to_owned())?;
            let ime_window_handle = eax()?;
            if app_window_handle == 0 || ime_window_handle == 0 {
                return Err(
                    "primary ImmGetDefaultIMEWnd unexpectedly used or returned NULL".to_owned(),
                );
            }
            let ime_window = runtime
                .ime_window(ime_window_handle)
                .map_err(error_string)?;
            let association = app_association(app_window_handle)?;
            let context = associated_context(association)?;
            Ok(Some(json!({
                "action": "default-window-resolved",
                "policyId": imm32_policy.id,
                "appWindowHandle": app_window_handle,
                "imeWindow": ime_window,
                "association": association,
                "context": context,
                "nativeHandleImported": false,
                "browserResourceCreated": false,
            })))
        }
        SEND_MESSAGE_A_IMPLEMENTATION
            if completion.frame.argument_u32(1) == Some(USER32_WM_IME_CONTROL) =>
        {
            let ime_window_handle = completion
                .frame
                .argument_u32(0)
                .ok_or_else(|| "WM_IME_CONTROL completion has no logical IME HWND".to_owned())?;
            if eax()? != 0 {
                return Err("primary WM_IME_CONTROL did not return zero".to_owned());
            }
            let ime_window = runtime
                .ime_window(ime_window_handle)
                .map_err(error_string)?;
            Ok(Some(json!({
                "action": "status-window-control-requested",
                "policyId": user32_policy.id,
                "imeWindow": ime_window,
                "nativeMessageForwarded": false,
                "browserResourceCreated": false,
            })))
        }
        IMM_ASSOCIATE_CONTEXT_IMPLEMENTATION => {
            let app_window_handle = completion
                .frame
                .argument_u32(0)
                .ok_or_else(|| "ImmAssociateContext completion has no app HWND".to_owned())?;
            let requested_context_handle = completion
                .frame
                .argument_u32(1)
                .ok_or_else(|| "ImmAssociateContext completion has no HIMC".to_owned())?;
            let previous_context_handle = eax()?;
            let association = app_association(app_window_handle)?;
            let context = associated_context(association)?;
            if association.context.map_or(0, |reference| reference.handle)
                != requested_context_handle
            {
                return Err(
                    "ImmAssociateContext argument and logical association authority disagree"
                        .to_owned(),
                );
            }
            Ok(Some(json!({
                "action": "context-associated",
                "policyId": imm32_policy.id,
                "appWindowHandle": app_window_handle,
                "requestedContextHandle": requested_context_handle,
                "previousContextHandle": previous_context_handle,
                "association": association,
                "context": context,
                "nativeHandleImported": false,
                "browserResourceCreated": false,
            })))
        }
        _ => Ok(None),
    }
}

fn bgi_virtual_memory_completion_metadata(
    completion: &ApiImmediateCompletion,
    runtime: &RuntimeState,
    memory: &GuestMemory,
) -> Result<Option<Value>, String> {
    if completion.implementation_id != vine_core::VIRTUAL_ALLOC_IMPLEMENTATION {
        return Ok(None);
    }
    let RegisterUpdate::Set(address) = completion.eax else {
        return Err("VirtualAlloc completion did not set EAX".to_owned());
    };
    if address == 0 {
        return Err("primary VirtualAlloc reserve unexpectedly returned NULL".to_owned());
    }
    let allocation = runtime
        .process_address_space_allocator()
        .allocation_for_purpose(&ProcessAddressAllocationPurpose::VirtualMemory)
        .map_err(error_string)?;
    if allocation.range.base.0 != address {
        return Err(
            "VirtualAlloc return value does not identify its process address allocation".to_owned(),
        );
    }
    let region = memory
        .region_snapshots()
        .into_iter()
        .find(|region| region.address_allocation_id == Some(allocation.id.0))
        .ok_or_else(|| {
            "VirtualAlloc process address allocation has no guest memory reservation".to_owned()
        })?;
    if region.range.start() != allocation.range.base
        || region.range.byte_length() != allocation.range.byte_length
    {
        return Err(
            "VirtualAlloc process address allocation and guest memory reservation disagree"
                .to_owned(),
        );
    }
    Ok(Some(json!({
        "allocation": allocation,
        "region": region,
        "preferredAddress": completion.frame.argument_u32(0),
        "allocationType": completion.frame.argument_u32(2),
        "protection": completion.frame.argument_u32(3),
        "reserveOnly": true,
        "topDown": true,
    })))
}

fn bgi_gdiplus_completion_metadata(
    completion: &ApiImmediateCompletion,
    runtime: &RuntimeState,
    policy: &GdiplusCompatibilityPolicy,
) -> Result<Option<Value>, String> {
    match completion.implementation_id.as_str() {
        vine_core::GDIPLUS_STARTUP_IMPLEMENTATION => {
            let RegisterUpdate::Set(status) = completion.eax else {
                return Err("GdiplusStartup completion did not set EAX".to_owned());
            };
            if status != vine_core::GDIPLUS_STATUS_OK {
                return Err(format!(
                    "primary GdiplusStartup completed with status {status}"
                ));
            }
            let token_output = completion.frame.argument_u32(0).ok_or_else(|| {
                "GdiplusStartup completion has no token output argument".to_owned()
            })?;
            if completion.guest_writes.len() != 1 {
                return Err(
                    "successful GdiplusStartup must commit exactly one token write".to_owned(),
                );
            }
            let write = &completion.guest_writes[0].write;
            if write.address.0 != token_output || write.bytes.len() != 4 {
                return Err(
                    "GdiplusStartup token write does not match the decoded x86 output".to_owned(),
                );
            }
            let token = u32::from_le_bytes(
                write.bytes[..4]
                    .try_into()
                    .map_err(|_| "GdiplusStartup token is not one x86 ULONG_PTR".to_owned())?,
            );
            let session = runtime
                .gdiplus_sessions()
                .values()
                .find(|session| session.token == token)
                .ok_or_else(|| {
                    "GdiplusStartup completion has no matching logical session".to_owned()
                })?;
            let startup = policy.startup.as_ref().ok_or_else(|| {
                "GdiplusStartup completed with its compatibility policy disabled".to_owned()
            })?;
            if session.token_namespace != vine_core::RUNTIME_GDIPLUS_SESSION_TOKEN_NAMESPACE
                || session.startup.profile_id != startup.profile_id
            {
                return Err(
                    "logical GDI+ session identity does not match the active startup policy"
                        .to_owned(),
                );
            }
            Ok(Some(json!({
                "action": "session-started",
                "policyId": policy.id,
                "session": session,
                "liveSessions": runtime.gdiplus_sessions().len(),
                "browserResourceCreated": false,
                "nativeTokenImported": false,
            })))
        }
        vine_core::GDIPLUS_SHUTDOWN_IMPLEMENTATION => {
            if !matches!(completion.eax, RegisterUpdate::Preserve) {
                return Err("void GdiplusShutdown completion unexpectedly changed EAX".to_owned());
            }
            let token = completion
                .frame
                .argument_u32(0)
                .ok_or_else(|| "GdiplusShutdown completion has no logical token".to_owned())?;
            if runtime
                .gdiplus_sessions()
                .values()
                .any(|session| session.token == token)
            {
                return Err(
                    "GdiplusShutdown completion did not retire its logical session".to_owned(),
                );
            }
            Ok(Some(json!({
                "action": "session-shutdown",
                "token": token,
                "liveSessions": runtime.gdiplus_sessions().len(),
            })))
        }
        _ => Ok(None),
    }
}

fn bgi_gdi_completion_metadata(
    completion: &ApiImmediateCompletion,
    runtime: &RuntimeState,
    policy: &Gdi32CompatibilityPolicy,
) -> Result<Option<Value>, String> {
    match completion.implementation_id.as_str() {
        vine_core::GET_DC_IMPLEMENTATION => {
            let RegisterUpdate::Set(handle) = completion.eax else {
                return Err("GetDC completion did not set EAX".to_owned());
            };
            let device_context = runtime.gdi_device_context(handle).map_err(error_string)?;
            match device_context.target {
                vine_core::RuntimeDeviceContextTarget::Window { surface_id, .. } => {
                    let surface = runtime.gdi_surfaces().get(&surface_id).ok_or_else(|| {
                        "window GetDC completion has no matching logical surface".to_owned()
                    })?;
                    Ok(Some(json!({
                        "action": "device-context-acquired",
                        "deviceContext": device_context,
                        "surface": surface,
                        "liveDeviceContexts": runtime.gdi_device_contexts().len(),
                    })))
                }
                vine_core::RuntimeDeviceContextTarget::Screen { .. } => Ok(Some(json!({
                    "action": "screen-device-context-acquired",
                    "deviceContext": device_context,
                    "liveDeviceContexts": runtime.gdi_device_contexts().len(),
                }))),
                vine_core::RuntimeDeviceContextTarget::Memory { .. } => {
                    Err("USER32 GetDC completion resolved to a memory HDC".to_owned())
                }
                vine_core::RuntimeDeviceContextTarget::GraphicsSurface { .. } => {
                    Err("USER32 GetDC completion resolved to a graphics-surface HDC".to_owned())
                }
            }
        }
        vine_core::GET_STOCK_OBJECT_IMPLEMENTATION => {
            let RegisterUpdate::Set(handle) = completion.eax else {
                return Err("GetStockObject completion did not set EAX".to_owned());
            };
            let object = runtime
                .gdi_objects()
                .values()
                .find(|object| object.handle == handle)
                .ok_or_else(|| {
                    "GetStockObject completion has no matching logical object".to_owned()
                })?;
            Ok(Some(json!({
                "action": "stock-object-resolved",
                "object": object,
                "liveObjects": runtime.gdi_objects().len(),
            })))
        }
        vine_core::FILL_RECT_IMPLEMENTATION => {
            let handle = completion
                .frame
                .argument_u32(0)
                .ok_or_else(|| "FillRect completion has no logical HDC argument".to_owned())?;
            let device_context = runtime.gdi_device_context(handle).map_err(error_string)?;
            match device_context.target {
                vine_core::RuntimeDeviceContextTarget::Window { surface_id, .. } => {
                    let surface = runtime.gdi_surfaces().get(&surface_id).ok_or_else(|| {
                        "FillRect completion has no matching logical window surface".to_owned()
                    })?;
                    Ok(Some(json!({
                        "action": "surface-filled",
                        "deviceContext": device_context,
                        "surface": surface,
                    })))
                }
                vine_core::RuntimeDeviceContextTarget::GraphicsSurface { surface_id, .. } => {
                    let surface =
                        runtime
                            .graphics_surfaces()
                            .get(&surface_id)
                            .ok_or_else(|| {
                                "FillRect completion has no matching logical graphics surface"
                                    .to_owned()
                            })?;
                    Ok(Some(json!({
                        "action": "graphics-surface-filled",
                        "deviceContext": device_context,
                        "surface": surface,
                    })))
                }
                vine_core::RuntimeDeviceContextTarget::Screen { .. } => {
                    Err("screen FillRect completed outside the reached GDI support band".to_owned())
                }
                vine_core::RuntimeDeviceContextTarget::Memory { .. } => {
                    Err("memory FillRect completed before a logical DIB was selected".to_owned())
                }
            }
        }
        vine_core::ENUM_FONT_FAMILIES_EX_A_IMPLEMENTATION => {
            let handle = completion.frame.argument_u32(0).ok_or_else(|| {
                "EnumFontFamiliesExA completion has no logical HDC argument".to_owned()
            })?;
            let RegisterUpdate::Set(callback_result) = completion.eax else {
                return Err("EnumFontFamiliesExA completion did not set EAX".to_owned());
            };
            let device_context = runtime.gdi_device_context(handle).map_err(error_string)?;
            let enumeration = policy.enum_font_families_ex_a.as_ref().ok_or_else(|| {
                "EnumFontFamiliesExA completed with its GDI32 policy disabled".to_owned()
            })?;
            Ok(Some(json!({
                "action": "font-enumeration-completed",
                "deviceContext": device_context,
                "enumeration": enumeration,
                "callbackResult": callback_result,
                "liveDeviceContexts": runtime.gdi_device_contexts().len(),
            })))
        }
        vine_core::CREATE_FONT_A_IMPLEMENTATION => {
            let RegisterUpdate::Set(handle) = completion.eax else {
                return Err("CreateFontA completion did not set EAX".to_owned());
            };
            let object = runtime
                .gdi_objects()
                .values()
                .find(|object| object.handle == handle)
                .ok_or_else(|| {
                    "CreateFontA completion has no matching logical font object".to_owned()
                })?;
            if !matches!(
                &object.object,
                vine_core::RuntimeGdiObjectKind::FontA { .. }
            ) {
                return Err("CreateFontA completion resolved to a non-font object".to_owned());
            }
            Ok(Some(json!({
                "action": "font-created",
                "object": object,
                "liveObjects": runtime.gdi_objects().len(),
            })))
        }
        vine_core::CREATE_COMPATIBLE_DC_IMPLEMENTATION => {
            let RegisterUpdate::Set(handle) = completion.eax else {
                return Err("CreateCompatibleDC completion did not set EAX".to_owned());
            };
            let device_context = runtime.gdi_device_context(handle).map_err(error_string)?;
            if !matches!(
                device_context.target,
                vine_core::RuntimeDeviceContextTarget::Memory { .. }
            ) {
                return Err("CreateCompatibleDC completion resolved to a non-memory HDC".to_owned());
            }
            let font_id = device_context.selected_font_id.ok_or_else(|| {
                "CreateCompatibleDC completion has no selected default font".to_owned()
            })?;
            let bitmap_id = device_context.selected_bitmap_id.ok_or_else(|| {
                "CreateCompatibleDC completion has no selected default bitmap".to_owned()
            })?;
            let initial_font = runtime.gdi_objects().get(&font_id).ok_or_else(|| {
                "CreateCompatibleDC completion default font is not live".to_owned()
            })?;
            let initial_bitmap = runtime.gdi_objects().get(&bitmap_id).ok_or_else(|| {
                "CreateCompatibleDC completion default bitmap is not live".to_owned()
            })?;
            Ok(Some(json!({
                "action": "compatible-device-context-created",
                "deviceContext": device_context,
                "initialFont": initial_font,
                "initialBitmap": initial_bitmap,
                "liveDeviceContexts": runtime.gdi_device_contexts().len(),
                "liveObjects": runtime.gdi_objects().len(),
            })))
        }
        vine_core::CREATE_DIB_SECTION_IMPLEMENTATION => {
            let RegisterUpdate::Set(handle) = completion.eax else {
                return Err("CreateDIBSection completion did not set EAX".to_owned());
            };
            if handle == 0 {
                return Ok(Some(json!({
                    "action": "dib-section-create-rejected",
                    "created": false,
                    "liveObjects": runtime.gdi_objects().len(),
                })));
            }
            let object = runtime
                .gdi_objects()
                .values()
                .find(|object| object.handle == handle)
                .ok_or_else(|| {
                    "CreateDIBSection completion has no matching logical bitmap object".to_owned()
                })?;
            let vine_core::RuntimeGdiObjectKind::DibSection { backing, .. } = &object.object else {
                return Err("CreateDIBSection completion resolved to a non-DIB object".to_owned());
            };
            let address_allocation = runtime
                .process_address_space_allocator()
                .allocation(backing.address_allocation_id)
                .map_err(error_string)?;
            Ok(Some(json!({
                "action": "dib-section-created",
                "object": object,
                "addressAllocation": address_allocation,
                "liveObjects": runtime.gdi_objects().len(),
            })))
        }
        vine_core::GET_OBJECT_A_IMPLEMENTATION => {
            let RegisterUpdate::Set(byte_length) = completion.eax else {
                return Err("GetObjectA completion did not set EAX".to_owned());
            };
            let handle = completion
                .frame
                .argument_u32(0)
                .ok_or_else(|| "GetObjectA completion has no logical object argument".to_owned())?;
            let object = runtime
                .gdi_objects()
                .values()
                .find(|object| object.handle == handle);
            Ok(Some(json!({
                "action": "object-bytes-read",
                "handle": handle,
                "byteLength": byte_length,
                "object": object,
                "liveObjects": runtime.gdi_objects().len(),
            })))
        }
        vine_core::GET_OUTLINE_TEXT_METRICS_A_IMPLEMENTATION => {
            let RegisterUpdate::Set(returned_byte_length) = completion.eax else {
                return Err("GetOutlineTextMetricsA completion did not set EAX".to_owned());
            };
            let device_context_handle = completion.frame.argument_u32(0).ok_or_else(|| {
                "GetOutlineTextMetricsA completion has no logical HDC argument".to_owned()
            })?;
            let requested_byte_length = completion.frame.argument_u32(1).ok_or_else(|| {
                "GetOutlineTextMetricsA completion has no byte length argument".to_owned()
            })?;
            let output_address = completion.frame.argument_u32(2).ok_or_else(|| {
                "GetOutlineTextMetricsA completion has no output argument".to_owned()
            })?;
            let device_context = runtime
                .gdi_device_context(device_context_handle)
                .map_err(error_string)?;
            let font_id = device_context.selected_font_id.ok_or_else(|| {
                "GetOutlineTextMetricsA completion has no selected font".to_owned()
            })?;
            let selected_font = runtime.gdi_objects().get(&font_id).ok_or_else(|| {
                "GetOutlineTextMetricsA completion selected font is not live".to_owned()
            })?;
            if !matches!(
                &selected_font.object,
                vine_core::RuntimeGdiObjectKind::FontA { .. }
            ) {
                return Err(
                    "GetOutlineTextMetricsA completion selected object is not a FontA".to_owned(),
                );
            }
            let outline_policy = policy.get_outline_text_metrics_a.as_ref().ok_or_else(|| {
                "GetOutlineTextMetricsA completed with its GDI32 policy disabled".to_owned()
            })?;
            let written_byte_length = completion
                .guest_writes
                .first()
                .map(|receipt| receipt.write.bytes.len())
                .unwrap_or(0);
            Ok(Some(json!({
                "action": "outline-text-metrics-read",
                "deviceContext": device_context,
                "selectedFont": selected_font,
                "profileId": outline_policy.profile_id,
                "requestedByteLength": requested_byte_length,
                "returnedByteLength": returned_byte_length,
                "outputAddress": output_address,
                "writtenByteLength": written_byte_length,
            })))
        }
        vine_core::TEXT_OUT_A_IMPLEMENTATION => {
            let RegisterUpdate::Set(result) = completion.eax else {
                return Err("TextOutA completion did not set EAX".to_owned());
            };
            if result != 1 {
                return Err("TextOutA primary raster completion did not succeed".to_owned());
            }
            let device_context_handle = completion
                .frame
                .argument_u32(0)
                .ok_or_else(|| "TextOutA completion has no logical HDC argument".to_owned())?;
            let x = completion
                .frame
                .argument_u32(1)
                .ok_or_else(|| "TextOutA completion has no x argument".to_owned())?
                as i32;
            let y = completion
                .frame
                .argument_u32(2)
                .ok_or_else(|| "TextOutA completion has no y argument".to_owned())?
                as i32;
            let text_address = completion
                .frame
                .argument_u32(3)
                .ok_or_else(|| "TextOutA completion has no text address argument".to_owned())?;
            let character_count = completion
                .frame
                .argument_u32(4)
                .ok_or_else(|| "TextOutA completion has no character count argument".to_owned())?;
            let device_context = runtime
                .gdi_device_context(device_context_handle)
                .map_err(error_string)?;
            let font_id = device_context
                .selected_font_id
                .ok_or_else(|| "TextOutA completion has no selected font".to_owned())?;
            let bitmap_id = device_context
                .selected_bitmap_id
                .ok_or_else(|| "TextOutA completion has no selected bitmap".to_owned())?;
            let selected_font = runtime
                .gdi_objects()
                .get(&font_id)
                .ok_or_else(|| "TextOutA completion selected font is not live".to_owned())?;
            if !matches!(
                &selected_font.object,
                vine_core::RuntimeGdiObjectKind::FontA { .. }
            ) {
                return Err("TextOutA completion selected object is not a FontA".to_owned());
            }
            let selected_dib_section = runtime
                .gdi_objects()
                .get(&bitmap_id)
                .ok_or_else(|| "TextOutA completion selected bitmap is not live".to_owned())?;
            let vine_core::RuntimeGdiObjectKind::DibSection { backing, .. } =
                &selected_dib_section.object
            else {
                return Err("TextOutA completion selected object is not a DIB section".to_owned());
            };
            let text_policy = policy
                .text_out_a
                .as_ref()
                .ok_or_else(|| "TextOutA completed with its GDI32 policy disabled".to_owned())?;
            if completion.guest_writes.len() > 1 {
                return Err("TextOutA completion emitted more than one raster write".to_owned());
            }
            let write = completion
                .guest_writes
                .first()
                .map(|receipt| &receipt.write);
            let changed_byte_count = write
                .map(|write| {
                    write
                        .bytes
                        .iter()
                        .filter(|&&byte| byte != text_policy.initial_pixel)
                        .count()
                })
                .unwrap_or(0);
            Ok(Some(json!({
                "action": "text-raster-written",
                "deviceContext": device_context,
                "selectedFont": selected_font,
                "selectedDibSection": selected_dib_section,
                "profileId": text_policy.profile_id,
                "x": x,
                "y": y,
                "textAddress": text_address,
                "characterCount": character_count,
                "writeAddress": write.map(|write| write.address.0),
                "writtenByteLength": write.map(|write| write.bytes.len()).unwrap_or(0),
                "changedByteCount": changed_byte_count,
                "contentGeneration": backing.content_generation,
                "expectedOutputFnv1a32": text_policy.output_pixel_plane_fnv1a32,
            })))
        }
        vine_core::SELECT_OBJECT_IMPLEMENTATION => {
            let RegisterUpdate::Set(previous_handle) = completion.eax else {
                return Err("SelectObject completion did not set EAX".to_owned());
            };
            let device_context_handle = completion
                .frame
                .argument_u32(0)
                .ok_or_else(|| "SelectObject completion has no logical HDC argument".to_owned())?;
            let selected_handle = completion.frame.argument_u32(1).ok_or_else(|| {
                "SelectObject completion has no logical object argument".to_owned()
            })?;
            let device_context = runtime
                .gdi_device_context(device_context_handle)
                .map_err(error_string)?;
            let selected_object = runtime
                .gdi_objects()
                .values()
                .find(|object| object.handle == selected_handle);
            let previous_object = runtime
                .gdi_objects()
                .values()
                .find(|object| object.handle == previous_handle);
            Ok(Some(json!({
                "action": "object-selected",
                "deviceContext": device_context,
                "selectedObject": selected_object,
                "previousObject": previous_object,
                "succeeded": previous_handle != 0,
                "liveObjects": runtime.gdi_objects().len(),
            })))
        }
        vine_core::DELETE_OBJECT_IMPLEMENTATION => {
            let RegisterUpdate::Set(result) = completion.eax else {
                return Err("DeleteObject completion did not set EAX".to_owned());
            };
            let handle = completion.frame.argument_u32(0).ok_or_else(|| {
                "DeleteObject completion has no logical object argument".to_owned()
            })?;
            Ok(Some(json!({
                "action": "object-deleted",
                "handle": handle,
                "deleted": result != 0,
                "liveObjects": runtime.gdi_objects().len(),
            })))
        }
        vine_core::DELETE_DC_IMPLEMENTATION => {
            let RegisterUpdate::Set(result) = completion.eax else {
                return Err("DeleteDC completion did not set EAX".to_owned());
            };
            let handle = completion
                .frame
                .argument_u32(0)
                .ok_or_else(|| "DeleteDC completion has no logical HDC argument".to_owned())?;
            Ok(Some(json!({
                "action": "compatible-device-context-deleted",
                "handle": handle,
                "deleted": result != 0,
                "liveDeviceContexts": runtime.gdi_device_contexts().len(),
                "liveObjects": runtime.gdi_objects().len(),
            })))
        }
        vine_core::RELEASE_DC_IMPLEMENTATION => {
            let handle = completion
                .frame
                .argument_u32(1)
                .ok_or_else(|| "ReleaseDC completion has no logical HDC argument".to_owned())?;
            Ok(Some(json!({
                "action": "device-context-released",
                "handle": handle,
                "liveDeviceContexts": runtime.gdi_device_contexts().len(),
                "liveSurfaces": runtime.gdi_surfaces().len(),
            })))
        }
        _ => Ok(None),
    }
}

fn decode_u32_payload(payload: &[u8]) -> Vec<u32> {
    payload
        .chunks_exact(4)
        .map(|bytes| u32::from_le_bytes(bytes.try_into().expect("four-byte host payload word")))
        .collect()
}

fn restore_selected_thread(
    runtime: &mut RuntimeState,
    previous_thread_id: RuntimeThreadId,
    owner_thread_id: RuntimeThreadId,
) -> Result<(), String> {
    if previous_thread_id != owner_thread_id
        && runtime
            .thread_state(previous_thread_id)
            .is_ok_and(|thread| thread.lifecycle == vine_core::RuntimeThreadLifecycleState::Running)
    {
        runtime
            .select_thread(previous_thread_id)
            .map_err(error_string)?;
    }
    Ok(())
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
pub extern "C" fn vine_bridge_prepare_bgi(
    executable_length: u32,
    policy_length: u32,
    source_manifest_length: u32,
    writable_profile_length: u32,
) -> u32 {
    run_bridge(|bridge| {
        bridge.prepare_bgi(
            executable_length as usize,
            policy_length as usize,
            source_manifest_length as usize,
            writable_profile_length as usize,
        )
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_prepare_source_read(
    executable_length: u32,
    policy_length: u32,
    source_manifest_length: u32,
) -> u32 {
    run_bridge(|bridge| {
        bridge.prepare_source_read(
            executable_length as usize,
            policy_length as usize,
            source_manifest_length as usize,
        )
    })
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
pub extern "C" fn vine_bridge_fast_path_abi_version() -> u32 {
    1
}

#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_acknowledge_fast_v1(
    generation_low: u32,
    generation_high: u32,
) -> u32 {
    run_bridge(|bridge| {
        let generation = u64::from(generation_low) | (u64::from(generation_high) << 32);
        bridge.runtime_mut()?.acknowledge_fast(generation)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_reconcile_writeback(control_length: u32, payload_length: u32) -> u32 {
    run_bridge(|bridge| {
        bridge.reconcile_writeback_input(control_length as usize, payload_length as usize)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_reconcile_writeback_fast_v1(
    control_length: u32,
    payload_length: u32,
) -> u32 {
    run_bridge(|bridge| {
        bridge.reconcile_writeback_input_fast(control_length as usize, payload_length as usize)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_fast_writeback_generation_low_v1() -> u32 {
    BRIDGE.with(|cell| {
        cell.borrow()
            .fast_writeback_receipt
            .map_or(0, |receipt| receipt.generation as u32)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_fast_writeback_generation_high_v1() -> u32 {
    BRIDGE.with(|cell| {
        cell.borrow()
            .fast_writeback_receipt
            .map_or(0, |receipt| (receipt.generation >> 32) as u32)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_fast_writeback_touched_pages_v1() -> u32 {
    BRIDGE.with(|cell| {
        cell.borrow()
            .fast_writeback_receipt
            .map_or(0, |receipt| receipt.touched_pages as u32)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_fast_writeback_changed_pages_v1() -> u32 {
    BRIDGE.with(|cell| {
        cell.borrow()
            .fast_writeback_receipt
            .map_or(0, |receipt| receipt.changed_pages as u32)
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
pub extern "C" fn vine_bridge_service_page_fault(
    fault_address: u32,
    page_fault_error_code: u32,
) -> u32 {
    run_bridge(|bridge| {
        let (metadata, delta) = bridge
            .runtime_mut()?
            .service_page_fault(GuestAddress(fault_address), page_fault_error_code)?;
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
pub extern "C" fn vine_bridge_resolve_com_endpoint(endpoint_address: u32) -> u32 {
    run_bridge(|bridge| {
        let metadata = bridge
            .runtime_mut()?
            .resolve_com_endpoint(GuestAddress(endpoint_address))?;
        bridge.set_output(metadata, Vec::new(), Vec::new())
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_dispatch_com(
    endpoint_address: u32,
    stack_pointer: u32,
    return_address: u32,
) -> u32 {
    run_bridge(|bridge| {
        let (metadata, delta) = bridge.runtime_mut()?.dispatch_com(
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
pub extern "C" fn vine_bridge_complete_source_read(
    request_low: u32,
    request_high: u32,
    control_length: u32,
    payload_length: u32,
) -> u32 {
    run_bridge(|bridge| {
        let request_id = u64::from(request_low) | (u64::from(request_high) << 32);
        bridge.complete_source_read(request_id, control_length as usize, payload_length as usize)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_writable_commit_payload(request_low: u32, request_high: u32) -> u32 {
    run_bridge(|bridge| {
        let request_id = u64::from(request_low) | (u64::from(request_high) << 32);
        bridge.writable_commit_payload(request_id)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_complete_writable_commit(
    request_low: u32,
    request_high: u32,
    control_length: u32,
) -> u32 {
    run_bridge(|bridge| {
        let request_id = u64::from(request_low) | (u64::from(request_high) << 32);
        bridge.complete_writable_commit(request_id, control_length as usize)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_hydrate_writable_profile_object(
    object_id: u32,
    generation_low: u32,
    generation_high: u32,
    input_length: u32,
) -> u32 {
    run_bridge(|bridge| {
        bridge.hydrate_writable_profile_object(
            object_id,
            u64::from(generation_low) | (u64::from(generation_high) << 32),
            input_length as usize,
        )
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_schedule_next_thread() -> u32 {
    run_bridge(|bridge| {
        let metadata = bridge.runtime_mut()?.schedule_next_thread()?;
        bridge.set_output(metadata, Vec::new(), Vec::new())
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_finish_thread(exit_code: u32) -> u32 {
    run_bridge(|bridge| {
        let (metadata, delta) = bridge.runtime_mut()?.finish_thread(exit_code)?;
        let (control, payload) = delta
            .map(|buffers| (buffers.control, buffers.page_bytes))
            .unwrap_or_default();
        bridge.set_output(metadata, control, payload)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn vine_bridge_complete_callback(sentinel: u32, result: u32) -> u32 {
    run_bridge(|bridge| {
        let (metadata, delta) = bridge
            .runtime_mut()?
            .complete_callback(GuestAddress(sentinel), result)?;
        let (control, payload) = delta
            .map(|buffers| (buffers.control, buffers.page_bytes))
            .unwrap_or_default();
        bridge.set_output(metadata, control, payload)
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

#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, Read, Write};
use std::mem::size_of;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use vine_core::{
    ApiContract, ApiDispatcher, ApiEndpointIdentity, ApiFault, ApiFaultClass, ApiGuestCallbackCall,
    ApiImmediateCompletion, ApiOwnedTrap, ApiSymbol, ApiTrap, CONTROLLED_COMMAND_LINE,
    CREATE_THREAD_IMPLEMENTATION, CallbackReturnSentinel, ComCallCompletion, ComCallTrap,
    ComDispatchOutcome, ComFault, CpuWritebackReceipt, D3d9CompatibilityPolicy,
    DISPATCH_MESSAGE_A_IMPLEMENTATION, Dinput8CompatibilityPolicy, DispatchOutcome,
    DsoundCompatibilityPolicy, DynamicExportThunk, ExecutionSession, Gdi32CompatibilityPolicy,
    GdiplusCompatibilityPolicy, GuestAddress, GuestMemory, GuestMemoryError, GuestMemoryErrorClass,
    GuestPageReconciliation, GuestWriteResult, IMM_ASSOCIATE_CONTEXT_IMPLEMENTATION,
    IMM_GET_DEFAULT_IME_WND_IMPLEMENTATION, ImageMaterialization, ImageMemoryLayout,
    Imm32CompatibilityPolicy, ImportBinding, ImportTarget, ImportThunk, ImportThunkDisposition,
    InitialProcessEnvironment, InitialThreadStack, Kernel32CompatibilityPolicy,
    LOAD_LIBRARY_A_IMPLEMENTATION, LoaderError, LoaderErrorClass, LoaderIngressReport, MemoryDelta,
    MemoryInvalidation, MemoryLimits, MemoryOwnerId, MemoryReflection, MemoryRegionSnapshot,
    MemoryStats, ModuleIdentity, ModuleSnapshot, PEEK_MESSAGE_A_IMPLEMENTATION, Pe32Limits,
    ProcessAddressAllocationPurpose, ProcessAddressAllocationRecord, ProcessAddressSpaceAllocator,
    ProcessAddressSpaceLayout, ProcessEntryReturnSentinel, RESUME_THREAD_IMPLEMENTATION,
    RUNTIME_WAITABLE_TIMER_PACING_SERVICE, RUNTIME_WRITABLE_COMMIT_SERVICE,
    RUNTIME_WRITABLE_DIRECTORY_CREATE_SERVICE, RegisterUpdate, RuntimeBootstrap,
    RuntimeCallbackFrameId, RuntimeClockState, RuntimeComApartmentState, RuntimeComMethodTarget,
    RuntimeDllLifecycleAction, RuntimeDllLifecyclePhase, RuntimeDynamicEndpoint, RuntimeExport,
    RuntimeExportCatalog, RuntimeGuestImportBinding, RuntimeGuestImportResolution, RuntimeHandle,
    RuntimeHeapAllocation, RuntimeImeContext, RuntimeImeContextAssociation, RuntimeImeWindow,
    RuntimeKeyboardEventReceipt, RuntimeKeyboardSourceEvent, RuntimeKeyboardSourceEventKind,
    RuntimeKeyboardState, RuntimeLogicalMonitor, RuntimeModule, RuntimeModuleId,
    RuntimeModuleImageError, RuntimeModuleImageErrorClass, RuntimeModuleNode, RuntimeMutex,
    RuntimeObjectKey, RuntimeObjectKind, RuntimeObjectRecord, RuntimePageFaultResolution,
    RuntimePointerButton, RuntimePointerButtonIngressReceipt, RuntimePointerButtonSourceEvent,
    RuntimePointerButtonSourceEventKind, RuntimeProcessId, RuntimeProcessLifecycleState,
    RuntimeSchedulerEvent, RuntimeSourceReadCompletion, RuntimeSourceReadHostResponse,
    RuntimeSourceSnapshot, RuntimeSourceSnapshotManifest, RuntimeStandardHandle,
    RuntimeStartupInfo, RuntimeState, RuntimeStateError, RuntimeStateErrorClass,
    RuntimeThreadExecutionSelection, RuntimeThreadId, RuntimeThreadMessage, RuntimeThreadPriority,
    RuntimeThreadStack, RuntimeThreadStackGrowth, RuntimeThreadState, RuntimeTimerClockState,
    RuntimeTimerResolutionState, RuntimeTlsValue, RuntimeWaitableTimer, RuntimeWindowId,
    RuntimeWindowTimer, RuntimeWindowTimerDeliveryState, RuntimeWindowTimerId,
    RuntimeWritableCommitCompletion, RuntimeWritableCommitHostResponse,
    RuntimeWritableDirectoryCreateCompletion, RuntimeWritableDirectoryCreateHostResponse,
    RuntimeWritableProfile, RuntimeWritableProfileManifest, SEND_MESSAGE_A_IMPLEMENTATION,
    SET_THREAD_PRIORITY_IMPLEMENTATION, SET_TIMER_IMPLEMENTATION, SET_WINDOW_LONG_A_IMPLEMENTATION,
    SchedulerThreadState, TIME_BEGIN_PERIOD_IMPLEMENTATION, TIME_END_PERIOD_IMPLEMENTATION,
    TIME_GET_TIME_IMPLEMENTATION, TRANSLATE_MESSAGE_IMPLEMENTATION, USER32_WM_IME_CONTROL,
    UnsupportedPoint, User32CompatibilityPolicy, VIRTUAL_ALLOC_IMPLEMENTATION,
    WinmmCompatibilityPolicy, bind_pe32_imports, bind_pe32_imports_with_unsupported_traps,
    build_pe32_load_plan, complete_kernel32_source_read, complete_kernel32_waitable_timer_pacing,
    complete_kernel32_writable_commit, complete_kernel32_writable_directory_create,
    d3d9_contract_for_endpoint, dinput8_contract_for_endpoint, dispatch_d3d9_api,
    dispatch_d3d9_com_call, dispatch_dinput8_api, dispatch_dinput8_com_call, dispatch_dsound_api,
    dispatch_dsound_com_call, dispatch_gdi32_api_with_policy, dispatch_gdiplus_api_with_policy,
    dispatch_imm32_api_with_policy, dispatch_kernel32_api, dispatch_kernel32_api_with_policy,
    dispatch_ole32_api, dispatch_shell32_api, dispatch_user32_api_with_policy,
    dispatch_winmm_api_with_policy, dsound_contract_for_endpoint, encode_pointer_contract,
    gdi32_contract_for_endpoint, gdiplus_contract_for_endpoint, get_current_process_id_contract,
    get_current_thread_id_contract, get_module_handle_w_contract, get_proc_address_contract,
    get_startup_info_w_contract, get_system_time_as_file_time_contract, get_tick_count_contract,
    heap_create_contract, heap_set_information_contract, imm32_contract_for_endpoint,
    install_callback_return_sentinel, install_controlled_initial_stack,
    install_controlled_process_environment,
    install_controlled_process_environment_with_command_line,
    install_controlled_process_environment_with_command_line_and_ansi,
    install_controlled_process_environment_with_command_line_ansi_and_environment_strings,
    install_controlled_process_environment_with_parameters, install_dynamic_export_thunk,
    install_dynamic_export_thunks, install_initial_thread_stack, install_runtime_return_sentinels,
    kernel32_contract_for_endpoint, materialize_pe32_image, ole32_contract_for_endpoint,
    query_performance_counter_contract, shell32_contract_for_endpoint, tls_alloc_contract,
    tls_set_value_contract, user32_contract_for_endpoint, winmm_contract_for_endpoint,
};

#[derive(Debug)]
struct Arguments {
    input: PathBuf,
    output: Option<PathBuf>,
    expected_sha256: Option<String>,
    prepare_memory_pages: Option<usize>,
    materialize_image_base: Option<u32>,
    runtime_exports: Vec<RuntimeExport>,
    trap_unsupported_imports: bool,
    v86_reflection_session: bool,
    checkpoint_session: Option<RuntimeCheckpointSelection>,
    generic_runtime_reentry: bool,
    generic_runtime_capability: Option<GenericRuntimeCapability>,
    generic_runtime_trap_limit: usize,
    kernel32_policy: Option<Kernel32CompatibilityPolicy>,
    source_manifest: Option<RuntimeSourceSnapshotManifest>,
    writable_profile: Option<RuntimeWritableProfileManifest>,
    large_image_reflection: bool,
    install_callback_return_sentinel: bool,
    install_process_entry_return_sentinel: bool,
    install_controlled_initial_stack: bool,
    install_controlled_process_environment: bool,
    install_controlled_process_parameters: bool,
    controlled_command_line: Option<String>,
}

const RUNTIME_FRAME_MAGIC: &[u8; 8] = b"VINEFR01";
const RUNTIME_FRAME_VERSION: u32 = 1;
const RUNTIME_FRAME_HEADER_BYTES: usize = 24;
const RUNTIME_FRAME_ACKNOWLEDGEMENT: u32 = 2;
const RUNTIME_FRAME_CPU_WRITEBACK: u32 = 3;
const RUNTIME_FRAME_WRITEBACK_RECEIPT: u32 = 4;
const RUNTIME_FRAME_HOST_DELTA: u32 = 5;
const RUNTIME_FRAME_SESSION_READY: u32 = 6;
const RUNTIME_FRAME_TRAP: u32 = 7;
const RUNTIME_FRAME_API_COMPLETION: u32 = 8;
const RUNTIME_FRAME_API_FAULT: u32 = 9;
const RUNTIME_FRAME_SESSION_STOP: u32 = 10;
const RUNTIME_FRAME_SESSION_STOPPED: u32 = 11;
const RUNTIME_FRAME_STACK_GROWTH: u32 = 12;
const RUNTIME_FRAME_STACK_GROWTH_COMPLETION: u32 = 13;
const RUNTIME_FRAME_GUEST_CALLBACK: u32 = 14;
const RUNTIME_FRAME_GUEST_CALLBACK_RETURN: u32 = 15;
const RUNTIME_FRAME_PAGE_FAULT_SERVICE: u32 = 16;
const RUNTIME_FRAME_PAGE_FAULT_COMPLETION: u32 = 17;
const RUNTIME_FRAME_HOST_REQUEST: u32 = 18;
const RUNTIME_FRAME_HOST_COMPLETION: u32 = 19;
const RUNTIME_FRAME_THREAD_EXIT: u32 = 20;
const RUNTIME_FRAME_THREAD_EXIT_COMPLETION: u32 = 21;
const RUNTIME_FRAME_KEYBOARD_INPUT: u32 = 22;
const RUNTIME_FRAME_KEYBOARD_INPUT_COMPLETION: u32 = 23;
const RUNTIME_FRAME_POINTER_BUTTON_INPUT: u32 = 24;
const RUNTIME_FRAME_POINTER_BUTTON_INPUT_COMPLETION: u32 = 25;
const RUNTIME_FRAME_MAX_CONTROL_BYTES: usize = 1024 * 1024;
const RUNTIME_FRAME_MAX_PAYLOAD_BYTES: usize = 16 * 1024 * 1024;
const GENERIC_RUNTIME_TRAP_LIMIT: usize = 32_768;
const GENERIC_RUNTIME_MAX_TRAP_LIMIT: usize = 196_608;
const GENERIC_RUNTIME_CALLBACK_DEPTH_LIMIT: usize = 64;
const CONTROLLED_KERNEL32_HANDLE: u32 = 0x4d01_0001;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GenericRuntimeCapability {
    CriticalSectionSynchronization,
    PointerDecoding,
    FiberLocalStorageAllocation,
    PrivateHeapAllocation,
    FiberLocalStorageValue,
    InterlockedIncrement,
    PrivateHeapSecondAllocation,
    StandardInputHandle,
    FileType,
    StandardOutputHandle,
    StandardOutputFileType,
    StandardErrorHandle,
    StandardErrorFileType,
    SetHandleCount,
    CommandLineA,
    EnvironmentStringsW,
    WideCharToMultiByte,
    PrivateHeapSize1,
    GetLastError,
    TlsGetValue,
    FlsGetValue,
    SetLastError,
    GetAcp,
    PrivateHeapSize544,
    GetCpInfo,
    MultiByteToWideChar,
    GetStringTypeW,
    LcMapStringW,
    InterlockedDecrement,
    GetModuleFileNameA,
    PrivateHeapSize16,
    PrivateHeapSize4,
    PrivateHeapFreeSize1,
    PrivateHeapSize128,
    IsProcessorFeaturePresent,
    SetUnhandledExceptionFilter,
    HeapSize,
    InitializeCriticalSection,
    SetCurrentDirectoryA,
    CreateMutexA,
    GetCurrentThread,
    SetThreadAffinityMask,
    GetSystemInfo,
    EnumDisplayMonitors,
    CoInitialize,
    GetSystemDefaultLangId,
    GetDriveTypeA,
    SwapMouseButton,
    TimeGetDevCaps,
    TimeBeginPeriod,
    GetVersionExA,
    CreateWaitableTimerA,
    LoadIconA,
    LoadCursorA,
    RegisterClassExA,
    GetSystemMetrics,
    LargeHeapAllocation,
    SourceFileRead,
    CreateWindowExA,
    GetSystemMetricsSmallIconHeight,
    GetSystemMetricsSmallIconWidth,
    LoadImageASmallIcon,
    SendMessageASmallIcon,
    SetWindowPosInitialPlacement,
    D3d9RootFactoryAndIdentity,
    D3d9GetAdapterDisplayMode,
    D3d9GetAdapterIdentifier,
    D3d9GetDeviceCaps,
    D3d9CheckDeviceType,
    D3d9CheckDeviceFormat,
    LoadLibraryA,
    GetClientRect,
    WindowDcFill,
    SetWindowLongA,
    SetWindowPosActivation,
    MonitorFromWindow,
    D3d9GetAdapterCount,
    D3d9GetAdapterMonitor,
    D3d9CreateDevice,
    D3d9CreateTexture,
    D3d9GetSurfaceLevel,
    D3d9SurfaceGetDc,
    TimeGetTime,
    D3d9InitialTextures,
    D3d9InitialVertexBuffer,
    D3d9InitialRenderStates,
    D3d9RasterStatus,
    QueryPerformanceFrequency,
    GetMonitorInfoA,
    Dinput8Factory,
    Dinput8EnumDevices,
    DsoundRootAndCooperativeLevel,
    DsoundInitialBuffers,
    DsoundBufferLockUnlockPlay,
    SetTimer,
    CreateThread,
    SetThreadPriority,
    ScreenDc,
    FontEnumeration,
    FontCreation,
    CompatibleDc,
    DibLifecycle,
    OutlineTextMetrics,
    TextRaster,
    GdiplusStartup,
    VirtualAlloc,
    Imm32Context,
    SetWaitableTimer,
    WaitForSingleObjectEx,
    DragAcceptFiles,
    SleepZero,
    GlobalMemoryStatus,
    GetFileAttributesA,
    CreateDirectoryA,
    D3d9RecreatedTextures,
    D3d9Clear,
    SetRect,
    D3d9AddDirtyRect,
    D3d9UpdateTexture,
    D3d9BeginScene,
    D3d9InitialSamplerStates,
    D3d9InitialTextureStageStates,
    D3d9SetTexture,
    D3d9SetFvf,
    D3d9SetStreamSource,
    D3d9DrawPrimitive,
    D3d9EndScene,
    D3d9PostEndSceneRasterStatus,
    D3d9PostRasterStatusWaitableTimer,
    D3d9PostScenePresentGateway,
    D3d9Present,
    D3d9PostPresentContinuation,
    User32AsyncKeyStateInput,
    User32AsyncKeyStateInitialScan,
    User32WindowTimerCallbackDelivery,
    User32PointerButtonMessages,
    User32SetFocusSameWindow,
    User32DefWindowProcPointerDown,
    User32DefWindowProcPointerRelease,
}

struct GenericRuntimeProcess {
    runtime: RuntimeState,
    binding: RuntimeGuestImportBinding,
    module_handles: Vec<u32>,
    main_module_handle: u32,
    attach_order: Vec<RuntimeModuleId>,
}

struct GenericRuntimeProfiles {
    source_manifest: Option<RuntimeSourceSnapshotManifest>,
    writable_profile: Option<RuntimeWritableProfileManifest>,
    clock_profile: Option<vine_core::RuntimeClockProfile>,
}

struct RuntimeFrame {
    kind: u32,
    control: Vec<u8>,
    payload: Vec<u8>,
}

struct GenericRuntimeActiveCallback {
    call: ApiGuestCallbackCall,
    frame_id: RuntimeCallbackFrameId,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum GenericRuntimeTrapKind {
    RuntimeSystemExport,
    DynamicExport,
    ComMethod,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GenericRuntimeTrapRequest {
    schema: String,
    schema_version: u32,
    kind: GenericRuntimeTrapKind,
    endpoint_id: u32,
    endpoint_address: u32,
    stack_pointer: u32,
    return_address: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GenericRuntimeCallbackReturnRequest {
    schema: String,
    schema_version: u32,
    frame_id: u64,
    return_sentinel: u32,
    stack_pointer: u32,
    result: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GenericRuntimeCallbackEntry {
    schema: &'static str,
    schema_version: u32,
    endpoint: GenericRuntimeEndpoint,
    frame_id: u64,
    target: u32,
    argument_count: usize,
    api_argument_count: usize,
    allocation_start: u32,
    stack_pointer: u32,
    scratch_address: u32,
    scratch_bytes: u32,
    frame_bytes: u32,
    return_sentinel: u32,
    queued_thread_messages: Vec<RuntimeThreadMessage>,
    memory: MemoryStats,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GenericRuntimeHostRequest {
    schema: &'static str,
    schema_version: u32,
    endpoint: GenericRuntimeEndpoint,
    request_id: u64,
    thread_id: u32,
    service: String,
    arguments: Vec<u32>,
    scheduler_state: GenericRuntimeWaitingHostState,
    memory: MemoryStats,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GenericRuntimeWaitingHostState {
    state: &'static str,
    request_id: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GenericRuntimeSessionStopRequest {
    schema: String,
    schema_version: u32,
    generation: u64,
    reason: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GenericRuntimeSessionStopped {
    schema: &'static str,
    schema_version: u32,
    reason: String,
    memory: MemoryStats,
    memory_content_sha256: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GenericRuntimeKeyboardInputRequest {
    schema: String,
    schema_version: u32,
    event: GenericRuntimeKeyboardInputEvent,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GenericRuntimeKeyboardInputEvent {
    sequence: u64,
    kind: String,
    virtual_key: Option<u16>,
    repeat: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GenericRuntimeKeyboardInputCompletion {
    schema: &'static str,
    schema_version: u32,
    receipt: RuntimeKeyboardEventReceipt,
    keyboard: RuntimeKeyboardState,
    memory: MemoryStats,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GenericRuntimePointerButtonInputRequest {
    schema: String,
    schema_version: u32,
    event: GenericRuntimePointerButtonInputEvent,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GenericRuntimePointerButtonInputEvent {
    sequence: u64,
    kind: String,
    button: String,
    normalized_x: u16,
    normalized_y: u16,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GenericRuntimePointerButtonInputCompletion {
    schema: &'static str,
    schema_version: u32,
    receipt: RuntimePointerButtonIngressReceipt,
    keyboard: RuntimeKeyboardState,
    memory: MemoryStats,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GenericRuntimeThreadExitRequest {
    schema: String,
    schema_version: u32,
    generation: u64,
    thread_id: u32,
    exit_code: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GenericRuntimeThreadExitCompletion {
    schema: &'static str,
    schema_version: u32,
    thread_id: u32,
    exit_code: u32,
    scheduler_state: SchedulerThreadState,
    #[serde(skip_serializing_if = "Option::is_none")]
    thread_selection: Option<RuntimeThreadExecutionSelection>,
    memory: MemoryStats,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GenericRuntimeStackGrowthRequest {
    schema: String,
    schema_version: u32,
    generation: u64,
    fault_address: u32,
    page_fault_error_code: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GenericRuntimeStackGrowthCompletion {
    schema: &'static str,
    schema_version: u32,
    growth: RuntimeThreadStackGrowth,
    memory: MemoryStats,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GenericRuntimePageFaultRequest {
    schema: String,
    schema_version: u32,
    generation: u64,
    fault_address: u32,
    page_fault_error_code: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GenericRuntimePageFaultCompletion {
    schema: &'static str,
    schema_version: u32,
    resolution: RuntimePageFaultResolution,
    memory: MemoryStats,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GenericRuntimeSessionReady {
    schema: &'static str,
    schema_version: u32,
    process_id: u32,
    current_thread_id: u32,
    current_directory_a: vine_core::RuntimeCurrentDirectoryA,
    current_directory_generation: u64,
    process_lifecycle: RuntimeProcessLifecycleState,
    scheduler_state: SchedulerThreadState,
    dll_lifecycle: RuntimeDllLifecyclePhase,
    modules: Vec<RuntimeModule>,
    main_module_handle: u32,
    dependency_edges: usize,
    attach_order: Vec<RuntimeModuleId>,
    runtime_export_endpoints: usize,
    implemented_api_contracts: usize,
    dynamic_export_endpoints: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    user32_compatibility_policy_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    gdi32_compatibility_policy_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    gdiplus_compatibility_policy_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    imm32_compatibility_policy_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    winmm_compatibility_policy_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    display_profile_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    primary_monitor: Option<RuntimeLogicalMonitor>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mouse_button_mapping: Option<vine_core::RuntimeMouseButtonMapping>,
    #[serde(skip_serializing_if = "Option::is_none")]
    keyboard_state: Option<RuntimeKeyboardState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cursor_position: Option<vine_core::RuntimeCursorPosition>,
    #[serde(skip_serializing_if = "Option::is_none")]
    initial_keyboard_event_receipt: Option<RuntimeKeyboardEventReceipt>,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_snapshot: Option<RuntimeSourceSnapshot>,
    #[serde(skip_serializing_if = "Option::is_none")]
    writable_profile: Option<RuntimeWritableProfile>,
    initial_thread_stack: RuntimeThreadStack,
    initial_stack_address_allocation: vine_core::ProcessAddressAllocationRecord,
    memory: MemoryStats,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
enum GenericRuntimeEndpoint {
    RuntimeSystemExport { endpoint: ApiEndpointIdentity },
    DynamicExport { endpoint: RuntimeDynamicEndpoint },
    ComMethod { target: RuntimeComMethodTarget },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GenericRuntimeComCompletion {
    schema: &'static str,
    schema_version: u32,
    endpoint: GenericRuntimeEndpoint,
    completion: ComCallCompletion,
    #[serde(skip_serializing_if = "Option::is_none")]
    graphics_device: Option<vine_core::RuntimeGraphicsDevice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    graphics_texture: Option<vine_core::RuntimeGraphicsTexture>,
    #[serde(skip_serializing_if = "Option::is_none")]
    graphics_texture_update: Option<GenericRuntimeGraphicsTextureUpdate>,
    memory: MemoryStats,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GenericRuntimeGraphicsTextureUpdate {
    source: vine_core::RuntimeGraphicsTexture,
    destination: vine_core::RuntimeGraphicsTexture,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
struct GenericRuntimeWindowTimer {
    id: RuntimeWindowTimerId,
    window_id: RuntimeWindowId,
    window_handle: u32,
    timer_id: u32,
    owner_thread_id: RuntimeThreadId,
    requested_interval_milliseconds: u32,
    effective_interval_milliseconds: u32,
    callback_address: GuestAddress,
    registration_generation: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    armed_at_time_100ns: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    next_deadline_time_100ns: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    delivery_count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_delivery: Option<RuntimeWindowTimerDeliveryState>,
}

impl GenericRuntimeWindowTimer {
    fn snapshot(timer: RuntimeWindowTimer, include_delivery_state: bool) -> Self {
        Self {
            id: timer.id,
            window_id: timer.window_id,
            window_handle: timer.window_handle,
            timer_id: timer.timer_id,
            owner_thread_id: timer.owner_thread_id,
            requested_interval_milliseconds: timer.requested_interval_milliseconds,
            effective_interval_milliseconds: timer.effective_interval_milliseconds,
            callback_address: timer.callback_address,
            registration_generation: timer.registration_generation,
            armed_at_time_100ns: include_delivery_state.then_some(timer.armed_at_time_100ns),
            next_deadline_time_100ns: include_delivery_state
                .then_some(timer.next_deadline_time_100ns),
            delivery_count: include_delivery_state.then_some(timer.delivery_count),
            last_delivery: include_delivery_state
                .then_some(timer.last_delivery)
                .flatten(),
        }
    }
}

fn generic_runtime_successful_peek_message_output(
    completion: &ApiImmediateCompletion,
) -> Result<Option<u32>, LoaderError> {
    const X86_MSG_BYTE_LENGTH: usize = 28;

    if completion.implementation_id != PEEK_MESSAGE_A_IMPLEMENTATION
        || !matches!(completion.eax, RegisterUpdate::Set(value) if value != 0)
    {
        return Ok(None);
    }
    let output_address = completion.frame.argument_u32(0).ok_or_else(|| {
        LoaderError::runtime_defect(
            "runtime-reentry",
            "peek-message-output-argument-missing",
            "successful PeekMessageA completion has no MSG output argument",
        )
    })?;
    let [write] = completion.guest_writes.as_slice() else {
        return Err(LoaderError::runtime_defect(
            "runtime-reentry",
            "peek-message-output-write-invalid",
            "successful PeekMessageA completion must contain exactly one guest write",
        ));
    };
    if write.write.address.0 != output_address
        || write.write.bytes.len() != X86_MSG_BYTE_LENGTH
        || write.result.bytes_written != X86_MSG_BYTE_LENGTH
    {
        return Err(LoaderError::runtime_defect(
            "runtime-reentry",
            "peek-message-output-write-invalid",
            "successful PeekMessageA completion must write one complete x86 MSG to its output pointer",
        ));
    }
    Ok(Some(u32::from_le_bytes(
        write.write.bytes[4..8]
            .try_into()
            .expect("validated x86 MSG message field width"),
    )))
}

fn generic_runtime_translate_message_input(
    completion: &ApiImmediateCompletion,
    memory: &GuestMemory,
) -> Result<Option<u32>, LoaderError> {
    const X86_MSG_BYTE_LENGTH: usize = 28;

    if completion.implementation_id != TRANSLATE_MESSAGE_IMPLEMENTATION {
        return Ok(None);
    }
    let input_address = completion.frame.argument_u32(0).ok_or_else(|| {
        LoaderError::runtime_defect(
            "runtime-reentry",
            "translate-message-input-argument-missing",
            "TranslateMessage completion has no MSG input argument",
        )
    })?;
    let bytes = memory
        .read(GuestAddress(input_address), X86_MSG_BYTE_LENGTH)
        .map_err(memory_error)?;
    Ok(Some(u32::from_le_bytes(
        bytes[4..8]
            .try_into()
            .expect("validated x86 MSG message field width"),
    )))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GenericRuntimeCompletion {
    schema: &'static str,
    schema_version: u32,
    endpoint: GenericRuntimeEndpoint,
    completion: ApiImmediateCompletion,
    #[serde(skip_serializing_if = "Option::is_none")]
    fls_value: Option<GenericRuntimeFlsValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    interlocked_increment: Option<GenericRuntimeLongUpdate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    interlocked_decrement: Option<GenericRuntimeLongUpdate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    heap_allocation: Option<GenericRuntimeHeapAllocation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    heap_free: Option<GenericRuntimeHeapFree>,
    #[serde(skip_serializing_if = "Option::is_none")]
    standard_handle: Option<RuntimeStandardHandle>,
    #[serde(skip_serializing_if = "Option::is_none")]
    file_type: Option<GenericRuntimeFileType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    current_directory: Option<GenericRuntimeCurrentDirectory>,
    #[serde(skip_serializing_if = "Option::is_none")]
    mutex: Option<GenericRuntimeMutex>,
    #[serde(skip_serializing_if = "Option::is_none")]
    waitable_timer: Option<GenericRuntimeWaitableTimer>,
    #[serde(skip_serializing_if = "Option::is_none")]
    icon: Option<vine_core::RuntimeIcon>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cursor: Option<vine_core::RuntimeCursor>,
    #[serde(skip_serializing_if = "Option::is_none")]
    window_class: Option<vine_core::RuntimeWindowClass>,
    #[serde(skip_serializing_if = "Option::is_none")]
    window: Option<vine_core::RuntimeWindow>,
    #[serde(skip_serializing_if = "Option::is_none")]
    paint: Option<GenericRuntimePaintCompletion>,
    #[serde(skip_serializing_if = "Option::is_none")]
    window_timer: Option<GenericRuntimeWindowTimer>,
    #[serde(skip_serializing_if = "Option::is_none")]
    queued_thread_messages: Option<Vec<RuntimeThreadMessage>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    thread_affinity: Option<GenericRuntimeThreadAffinity>,
    #[serde(skip_serializing_if = "Option::is_none")]
    thread_priority: Option<GenericRuntimeThreadPriority>,
    #[serde(skip_serializing_if = "Option::is_none")]
    imm32: Option<GenericRuntimeImm32Completion>,
    #[serde(skip_serializing_if = "Option::is_none")]
    virtual_memory: Option<GenericRuntimeVirtualMemory>,
    #[serde(skip_serializing_if = "Option::is_none")]
    created_thread: Option<GenericRuntimeCreatedThread>,
    #[serde(skip_serializing_if = "Option::is_none")]
    resumed_thread: Option<GenericRuntimeResumedThread>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tick_count: Option<GenericRuntimeTickCount>,
    #[serde(skip_serializing_if = "Option::is_none")]
    com_apartment: Option<RuntimeComApartmentState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    timer_resolution: Option<RuntimeTimerResolutionState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_read: Option<RuntimeSourceReadCompletion>,
    #[serde(skip_serializing_if = "Option::is_none")]
    writable_commit: Option<RuntimeWritableCommitCompletion>,
    #[serde(skip_serializing_if = "Option::is_none")]
    writable_directory_create: Option<RuntimeWritableDirectoryCreateCompletion>,
    #[serde(skip_serializing_if = "Option::is_none")]
    runtime_module: Option<GenericRuntimeModuleLoad>,
    #[serde(skip_serializing_if = "Option::is_none")]
    gdi: Option<GenericRuntimeGdiCompletion>,
    #[serde(skip_serializing_if = "Option::is_none")]
    gdiplus: Option<GenericRuntimeGdiplusCompletion>,
    #[serde(skip_serializing_if = "Option::is_none")]
    thread_selection: Option<RuntimeThreadExecutionSelection>,
    memory: MemoryStats,
}

#[derive(Debug, Serialize)]
#[serde(
    tag = "action",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
enum GenericRuntimeGdiplusCompletion {
    SessionStarted {
        policy_id: String,
        session: vine_core::RuntimeGdiplusSession,
        live_sessions: usize,
        browser_resource_created: bool,
        native_token_imported: bool,
    },
    SessionShutdown {
        token: u32,
        live_sessions: usize,
    },
}

#[derive(Debug, Serialize)]
#[serde(
    tag = "action",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
enum GenericRuntimeGdiCompletion {
    DeviceContextAcquired {
        device_context: vine_core::RuntimeDeviceContext,
        surface: vine_core::RuntimeGdiSurface,
        live_device_contexts: usize,
    },
    ScreenDeviceContextAcquired {
        device_context: vine_core::RuntimeDeviceContext,
        live_device_contexts: usize,
    },
    FontEnumerationCompleted {
        device_context: vine_core::RuntimeDeviceContext,
        enumeration: vine_core::Gdi32FontEnumerationPolicy,
        callback_result: u32,
        live_device_contexts: usize,
    },
    FontCreated {
        object: vine_core::RuntimeGdiObject,
        live_objects: usize,
    },
    CompatibleDeviceContextCreated {
        device_context: vine_core::RuntimeDeviceContext,
        initial_font: vine_core::RuntimeGdiObject,
        initial_bitmap: vine_core::RuntimeGdiObject,
        live_device_contexts: usize,
        live_objects: usize,
    },
    DibSectionCreated {
        object: vine_core::RuntimeGdiObject,
        address_allocation: ProcessAddressAllocationRecord,
        live_objects: usize,
    },
    DibSectionCreateRejected {
        live_objects: usize,
    },
    ObjectBytesRead {
        handle: u32,
        byte_length: u32,
        object: Option<vine_core::RuntimeGdiObject>,
        live_objects: usize,
    },
    OutlineTextMetricsRead {
        device_context: vine_core::RuntimeDeviceContext,
        selected_font: vine_core::RuntimeGdiObject,
        profile_id: String,
        requested_byte_length: u32,
        returned_byte_length: u32,
        output_address: u32,
        written_byte_length: usize,
    },
    TextRasterWritten {
        device_context: vine_core::RuntimeDeviceContext,
        selected_font: vine_core::RuntimeGdiObject,
        selected_dib_section: vine_core::RuntimeGdiObject,
        profile_id: String,
        x: i32,
        y: i32,
        text_address: u32,
        character_count: u32,
        write_address: Option<u32>,
        written_byte_length: usize,
        changed_byte_count: usize,
        content_generation: u64,
        expected_output_fnv1a32: u32,
    },
    ObjectSelected {
        device_context: vine_core::RuntimeDeviceContext,
        selected_object: Option<vine_core::RuntimeGdiObject>,
        previous_object: Option<vine_core::RuntimeGdiObject>,
        succeeded: bool,
        live_objects: usize,
    },
    ObjectDeleted {
        handle: u32,
        deleted: bool,
        live_objects: usize,
    },
    CompatibleDeviceContextDeleted {
        handle: u32,
        deleted: bool,
        live_device_contexts: usize,
        live_objects: usize,
    },
    StockObjectResolved {
        object: vine_core::RuntimeGdiObject,
        live_objects: usize,
    },
    SurfaceFilled {
        device_context: vine_core::RuntimeDeviceContext,
        surface: vine_core::RuntimeGdiSurface,
    },
    GraphicsSurfaceFilled {
        device_context: vine_core::RuntimeDeviceContext,
        surface: vine_core::RuntimeGraphicsSurface,
    },
    DeviceContextReleased {
        handle: u32,
        live_device_contexts: usize,
        live_surfaces: usize,
    },
}

#[derive(Debug, Serialize)]
#[serde(
    tag = "action",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
enum GenericRuntimePaintCompletion {
    Begun {
        window: vine_core::RuntimeWindow,
        session: vine_core::RuntimePaintSession,
        device_context: vine_core::RuntimeDeviceContext,
        live_paint_sessions: usize,
        live_device_contexts: usize,
    },
    Ended {
        window: vine_core::RuntimeWindow,
        paint_struct_address: u32,
        live_paint_sessions: usize,
        live_device_contexts: usize,
    },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GenericRuntimeModuleLoad {
    module: RuntimeModule,
    graph_node: RuntimeModuleNode,
    object_record: RuntimeObjectRecord,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GenericRuntimeCurrentDirectory {
    directory: vine_core::RuntimeCurrentDirectoryA,
    generation: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GenericRuntimeMutex {
    handle: RuntimeHandle,
    object: RuntimeMutex,
    object_record: RuntimeObjectRecord,
    already_exists: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GenericRuntimeWaitableTimer {
    handle: RuntimeHandle,
    timer: RuntimeWaitableTimer,
    object_record: RuntimeObjectRecord,
    #[serde(skip_serializing_if = "Option::is_none")]
    timer_clock: Option<RuntimeTimerClockState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    next_wait_registration_id: Option<u64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GenericRuntimeThreadAffinity {
    thread_id: u32,
    process_mask: u32,
    affinity_mask: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GenericRuntimeThreadPriority {
    handle: RuntimeHandle,
    thread_id: RuntimeThreadId,
    priority: RuntimeThreadPriority,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GenericRuntimeVirtualMemory {
    allocation: ProcessAddressAllocationRecord,
    region: MemoryRegionSnapshot,
    preferred_address: u32,
    allocation_type: u32,
    protection: u32,
    reserve_only: bool,
    top_down: bool,
}

#[derive(Debug, Serialize)]
#[serde(
    tag = "action",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase"
)]
enum GenericRuntimeImm32Completion {
    DefaultWindowResolved {
        policy_id: String,
        app_window_handle: u32,
        ime_window: RuntimeImeWindow,
        association: RuntimeImeContextAssociation,
        context: Option<RuntimeImeContext>,
        native_handle_imported: bool,
        browser_resource_created: bool,
    },
    StatusWindowControlRequested {
        policy_id: String,
        ime_window: RuntimeImeWindow,
        native_message_forwarded: bool,
        browser_resource_created: bool,
    },
    ContextAssociated {
        policy_id: String,
        app_window_handle: u32,
        requested_context_handle: u32,
        previous_context_handle: u32,
        association: RuntimeImeContextAssociation,
        context: Option<RuntimeImeContext>,
        native_handle_imported: bool,
        browser_resource_created: bool,
    },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GenericRuntimeCreatedThread {
    handle: RuntimeHandle,
    object: RuntimeObjectRecord,
    thread: RuntimeThreadState,
    scheduler_state: SchedulerThreadState,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GenericRuntimeResumedThread {
    handle: RuntimeHandle,
    thread_id: RuntimeThreadId,
    previous_suspend_count: u32,
    scheduler_state: SchedulerThreadState,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GenericRuntimeTickCount {
    returned_tick_count: u32,
    next_tick_count: u32,
    increment_per_read: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GenericRuntimeFlsValue {
    thread_id: u32,
    index: u32,
    value: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GenericRuntimeLongUpdate {
    address: u32,
    before: i32,
    after: i32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GenericRuntimeHeapAllocation {
    heap_handle: u32,
    allocation: RuntimeHeapAllocation,
    live_allocations: usize,
}

#[derive(Debug, Clone, Copy)]
struct GenericRuntimeHeapFreeBefore {
    heap_handle: u32,
    allocation: RuntimeHeapAllocation,
    live_allocations: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GenericRuntimeHeapFree {
    heap_handle: u32,
    allocation: RuntimeHeapAllocation,
    live_allocations_before: usize,
    live_allocations_after: usize,
    allocation_retired: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GenericRuntimeFileType {
    standard_handle: RuntimeStandardHandle,
    file_type: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GenericRuntimeFault {
    schema: &'static str,
    schema_version: u32,
    capability_cluster: String,
    endpoint: GenericRuntimeEndpoint,
    fault: ApiFault,
    memory: MemoryStats,
    memory_content_sha256: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct GenericRuntimeComFault {
    schema: &'static str,
    schema_version: u32,
    capability_cluster: String,
    endpoint: GenericRuntimeEndpoint,
    fault: ComFault,
    memory: MemoryStats,
    memory_content_sha256: String,
}

#[derive(Clone, Copy, Debug)]
struct V86ReturnSessionOptions<'a> {
    supported_call_count: usize,
    dynamic_export_thunks: &'a [DynamicExportThunk],
}

impl V86ReturnSessionOptions<'_> {
    fn includes(self, call_number: usize) -> bool {
        self.supported_call_count >= call_number
    }
}

#[derive(Clone, Debug)]
struct RuntimeCheckpointSelection {
    supported_call_count: usize,
    dispatches: Vec<RuntimeCheckpointDispatch>,
}

impl RuntimeCheckpointSelection {
    fn dispatch_occurrences(&self, module: &str, symbol: &str) -> usize {
        self.dispatches
            .iter()
            .filter(|dispatch| {
                dispatch.module.eq_ignore_ascii_case(module) && dispatch.symbol == symbol
            })
            .count()
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeCheckpointDispatch {
    module: String,
    symbol: String,
    occurrence: usize,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeCheckpointManifest {
    schema: String,
    schema_version: u32,
    checkpoints: Vec<RuntimeCheckpointEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeCheckpointEntry {
    id: String,
    extends: Option<String>,
    supported_call_count: usize,
    dispatch: Option<RuntimeCheckpointDispatch>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FailureReport {
    schema: &'static str,
    schema_version: u32,
    error: LoaderError,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MemoryIngressReport {
    schema: &'static str,
    schema_version: u32,
    source: ModuleIdentity,
    loader_schema: &'static str,
    loader_schema_version: u32,
    memory_limits: MemoryLimits,
    image: MemoryImageSummary,
    layout: ImageMemoryLayout,
    stats: MemoryStats,
    stop: UnsupportedPoint,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MemoryImageSummary {
    preferred_image_base: u32,
    size_of_image: u32,
    entry_point_address: u32,
    section_count: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MaterializationIngressReport {
    schema: &'static str,
    schema_version: u32,
    loader_schema: &'static str,
    loader_schema_version: u32,
    memory_limits: MemoryLimits,
    materialization: ImageMaterialization,
    stats: MemoryStats,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ImportBindingIngressReport {
    schema: &'static str,
    schema_version: u32,
    loader_schema: &'static str,
    loader_schema_version: u32,
    memory_limits: MemoryLimits,
    runtime_exports: Vec<RuntimeExport>,
    #[serde(skip_serializing_if = "is_false")]
    trap_unsupported_imports: bool,
    materialization: ImageMaterialization,
    binding: ImportBinding,
    stats: MemoryStats,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct V86ReflectionOffer {
    schema: &'static str,
    schema_version: u32,
    source: ModuleIdentity,
    image_load_base: GuestAddress,
    entry_point_address: GuestAddress,
    image_owner: vine_core::MemoryOwnerId,
    #[serde(skip_serializing_if = "Option::is_none")]
    first_thunk: Option<ImportThunk>,
    thunks: Vec<ImportThunk>,
    #[serde(skip_serializing_if = "Option::is_none")]
    callback_return_sentinel: Option<CallbackReturnSentinel>,
    #[serde(skip_serializing_if = "Option::is_none")]
    process_entry_return_sentinel: Option<ProcessEntryReturnSentinel>,
    #[serde(skip_serializing_if = "Option::is_none")]
    initial_thread_stack: Option<InitialThreadStack>,
    #[serde(skip_serializing_if = "Option::is_none")]
    initial_process_environment: Option<InitialProcessEnvironment>,
    #[serde(skip_serializing_if = "Option::is_none")]
    dynamic_export_thunk: Option<DynamicExportThunk>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    dynamic_export_thunks: Vec<DynamicExportThunk>,
    reflection: MemoryReflection,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct V86ReflectionAck {
    schema: String,
    schema_version: u32,
    generation: u64,
    data_tlb: bool,
    instruction_fetch: bool,
    generated_code: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct V86ReflectionReceipt {
    schema: &'static str,
    schema_version: u32,
    acknowledged_generation: u64,
    stats: MemoryStats,
    #[serde(skip_serializing_if = "Option::is_none")]
    iat_value: Option<u32>,
    entry_bytes: Vec<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    thunk_bytes: Option<Vec<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    callback_return_bytes: Option<Vec<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    process_entry_return_bytes: Option<Vec<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    initial_stack_top_bytes: Option<Vec<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    teb_bytes: Option<Vec<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    peb_bytes: Option<Vec<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    process_parameters_bytes: Option<Vec<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    command_line_bytes: Option<Vec<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    dynamic_export_bytes: Option<Vec<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    dynamic_export_page_prefix_bytes: Option<Vec<u8>>,
    stop: UnsupportedPoint,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct V86SystemTimeWriteRequest {
    schema: String,
    schema_version: u32,
    reflected_generation: u64,
    thunk_id: u32,
    thunk_address: u32,
    stack_pointer: u32,
    return_address: u32,
    argument_bytes: u32,
    output_address: u32,
    stack_page_address: u32,
    stack_page_owner: u32,
    stack_page_bytes: Vec<u8>,
    clock_owner: String,
    clock_sample_id: String,
    clock_utc_iso: String,
    file_time_low: u32,
    file_time_high: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct V86SystemTimeDeltaOffer {
    schema: &'static str,
    schema_version: u32,
    thunk: ImportThunk,
    calling_convention: &'static str,
    argument_bytes: u32,
    stack_pointer: GuestAddress,
    return_address: GuestAddress,
    output_address: GuestAddress,
    clock_owner: String,
    clock_sample_id: String,
    clock_utc_iso: String,
    file_time_ticks: String,
    file_time_low: u32,
    file_time_high: u32,
    stack_reconciliation: GuestPageReconciliation,
    write: GuestWriteResult,
    delta: MemoryDelta,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct V86MemoryDeltaAck {
    schema: String,
    schema_version: u32,
    generation: u64,
    data_tlb: bool,
    instruction_fetch: bool,
    generated_code: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct V86SystemTimeDeltaReceipt {
    schema: &'static str,
    schema_version: u32,
    acknowledged_generation: u64,
    output_address: GuestAddress,
    output_bytes: Vec<u8>,
    stats: MemoryStats,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct V86ProcessIdReturnRequest {
    schema: String,
    schema_version: u32,
    reflected_generation: u64,
    thunk_id: u32,
    thunk_address: u32,
    stack_pointer: u32,
    return_address: u32,
    argument_bytes: u32,
    stack_page_address: u32,
    stack_page_owner: u32,
    stack_page_bytes: Vec<u8>,
    process_identity_owner: String,
    process_id: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct V86ProcessIdReturnReceipt {
    schema: &'static str,
    schema_version: u32,
    thunk: ImportThunk,
    calling_convention: &'static str,
    argument_bytes: u32,
    stack_pointer: GuestAddress,
    return_address: GuestAddress,
    process_identity_owner: String,
    process_id: u32,
    stack_reconciliation: GuestPageReconciliation,
    stats: MemoryStats,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct V86ThreadIdReturnRequest {
    schema: String,
    schema_version: u32,
    reflected_generation: u64,
    thunk_id: u32,
    thunk_address: u32,
    stack_pointer: u32,
    return_address: u32,
    argument_bytes: u32,
    stack_page_address: u32,
    stack_page_owner: u32,
    stack_page_bytes: Vec<u8>,
    thread_identity_owner: String,
    thread_id: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct V86ThreadIdReturnReceipt {
    schema: &'static str,
    schema_version: u32,
    thunk: ImportThunk,
    calling_convention: &'static str,
    argument_bytes: u32,
    stack_pointer: GuestAddress,
    return_address: GuestAddress,
    thread_identity_owner: String,
    thread_id: u32,
    stack_reconciliation: GuestPageReconciliation,
    stats: MemoryStats,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct V86TickCountReturnRequest {
    schema: String,
    schema_version: u32,
    reflected_generation: u64,
    thunk_id: u32,
    thunk_address: u32,
    stack_pointer: u32,
    return_address: u32,
    argument_bytes: u32,
    stack_page_address: u32,
    stack_page_owner: u32,
    stack_page_bytes: Vec<u8>,
    monotonic_clock_owner: String,
    sample_id: String,
    paired_wall_clock_sample_id: String,
    tick_count: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct V86TickCountReturnReceipt {
    schema: &'static str,
    schema_version: u32,
    thunk: ImportThunk,
    calling_convention: &'static str,
    argument_bytes: u32,
    stack_pointer: GuestAddress,
    return_address: GuestAddress,
    monotonic_clock_owner: String,
    sample_id: String,
    paired_wall_clock_sample_id: String,
    tick_count: u32,
    stack_reconciliation: GuestPageReconciliation,
    stats: MemoryStats,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct V86PerformanceCounterWriteRequest {
    schema: String,
    schema_version: u32,
    reflected_generation: u64,
    thunk_id: u32,
    thunk_address: u32,
    stack_pointer: u32,
    return_address: u32,
    argument_bytes: u32,
    output_address: u32,
    stack_page_address: u32,
    stack_page_owner: u32,
    stack_page_bytes: Vec<u8>,
    monotonic_clock_owner: String,
    sample_id: String,
    paired_tick_sample_id: String,
    counter_low: u32,
    counter_high: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct V86PerformanceCounterDeltaOffer {
    schema: &'static str,
    schema_version: u32,
    thunk: ImportThunk,
    calling_convention: &'static str,
    argument_bytes: u32,
    stack_pointer: GuestAddress,
    return_address: GuestAddress,
    output_address: GuestAddress,
    monotonic_clock_owner: String,
    sample_id: String,
    paired_tick_sample_id: String,
    counter_ticks: String,
    counter_low: u32,
    counter_high: u32,
    stack_reconciliation: GuestPageReconciliation,
    write: GuestWriteResult,
    delta: MemoryDelta,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct V86PerformanceCounterDeltaReceipt {
    schema: &'static str,
    schema_version: u32,
    acknowledged_generation: u64,
    output_address: GuestAddress,
    output_bytes: Vec<u8>,
    stats: MemoryStats,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct V86StartupInfoWriteRequest {
    schema: String,
    schema_version: u32,
    reflected_generation: u64,
    thunk_id: u32,
    thunk_address: u32,
    stack_pointer: u32,
    return_address: u32,
    argument_bytes: u32,
    output_address: u32,
    stack_page_address: u32,
    stack_page_owner: u32,
    stack_page_bytes: Vec<u8>,
    process_startup_owner: String,
    profile_id: String,
    structure_size: u32,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct X86StartupInfoW {
    cb: u32,
    lp_reserved: u32,
    lp_desktop: u32,
    lp_title: u32,
    dw_x: u32,
    dw_y: u32,
    dw_x_size: u32,
    dw_y_size: u32,
    dw_x_count_chars: u32,
    dw_y_count_chars: u32,
    dw_fill_attribute: u32,
    dw_flags: u32,
    w_show_window: u16,
    cb_reserved2: u16,
    lp_reserved2: u32,
    h_std_input: u32,
    h_std_output: u32,
    h_std_error: u32,
}

impl X86StartupInfoW {
    const SIZE: u32 = 68;
    const PROFILE_ID: &'static str = "startupinfow-v68-zero-optional-fields";

    fn controlled() -> Self {
        Self {
            cb: Self::SIZE,
            lp_reserved: 0,
            lp_desktop: 0,
            lp_title: 0,
            dw_x: 0,
            dw_y: 0,
            dw_x_size: 0,
            dw_y_size: 0,
            dw_x_count_chars: 0,
            dw_y_count_chars: 0,
            dw_fill_attribute: 0,
            dw_flags: 0,
            w_show_window: 0,
            cb_reserved2: 0,
            lp_reserved2: 0,
            h_std_input: 0,
            h_std_output: 0,
            h_std_error: 0,
        }
    }

    fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(Self::SIZE as usize);
        bytes.extend_from_slice(&self.cb.to_le_bytes());
        bytes.extend_from_slice(&self.lp_reserved.to_le_bytes());
        bytes.extend_from_slice(&self.lp_desktop.to_le_bytes());
        bytes.extend_from_slice(&self.lp_title.to_le_bytes());
        bytes.extend_from_slice(&self.dw_x.to_le_bytes());
        bytes.extend_from_slice(&self.dw_y.to_le_bytes());
        bytes.extend_from_slice(&self.dw_x_size.to_le_bytes());
        bytes.extend_from_slice(&self.dw_y_size.to_le_bytes());
        bytes.extend_from_slice(&self.dw_x_count_chars.to_le_bytes());
        bytes.extend_from_slice(&self.dw_y_count_chars.to_le_bytes());
        bytes.extend_from_slice(&self.dw_fill_attribute.to_le_bytes());
        bytes.extend_from_slice(&self.dw_flags.to_le_bytes());
        bytes.extend_from_slice(&self.w_show_window.to_le_bytes());
        bytes.extend_from_slice(&self.cb_reserved2.to_le_bytes());
        bytes.extend_from_slice(&self.lp_reserved2.to_le_bytes());
        bytes.extend_from_slice(&self.h_std_input.to_le_bytes());
        bytes.extend_from_slice(&self.h_std_output.to_le_bytes());
        bytes.extend_from_slice(&self.h_std_error.to_le_bytes());
        debug_assert_eq!(bytes.len(), Self::SIZE as usize);
        bytes
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct V86StartupInfoDeltaOffer {
    schema: &'static str,
    schema_version: u32,
    thunk: ImportThunk,
    calling_convention: &'static str,
    argument_bytes: u32,
    stack_pointer: GuestAddress,
    return_address: GuestAddress,
    output_address: GuestAddress,
    process_startup_owner: String,
    profile_id: String,
    structure_size: u32,
    startup_info: X86StartupInfoW,
    output_bytes: Vec<u8>,
    stack_reconciliation: GuestPageReconciliation,
    write: GuestWriteResult,
    delta: MemoryDelta,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct V86StartupInfoDeltaReceipt {
    schema: &'static str,
    schema_version: u32,
    acknowledged_generation: u64,
    output_address: GuestAddress,
    output_bytes: Vec<u8>,
    stats: MemoryStats,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct V86HeapSetInformationReturnRequest {
    schema: String,
    schema_version: u32,
    reflected_generation: u64,
    thunk_id: u32,
    thunk_address: u32,
    stack_pointer: u32,
    return_address: u32,
    argument_bytes: u32,
    stack_page_address: u32,
    stack_page_owner: u32,
    stack_page_bytes: Vec<u8>,
    heap_policy_owner: String,
    policy_id: String,
    heap_handle: u32,
    information_class: u32,
    information_address: u32,
    information_length: u32,
    preserve_last_error: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct V86HeapSetInformationReturnReceipt {
    schema: &'static str,
    schema_version: u32,
    thunk: ImportThunk,
    calling_convention: &'static str,
    argument_bytes: u32,
    stack_pointer: GuestAddress,
    return_address: GuestAddress,
    heap_policy_owner: String,
    policy_id: String,
    heap_handle: u32,
    information_class: u32,
    information_address: u32,
    information_length: u32,
    termination_on_corruption_enabled: bool,
    preserve_last_error: bool,
    result: u32,
    stack_reconciliation: GuestPageReconciliation,
    stats: MemoryStats,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct V86HeapCreateReturnRequest {
    schema: String,
    schema_version: u32,
    reflected_generation: u64,
    thunk_id: u32,
    thunk_address: u32,
    stack_pointer: u32,
    return_address: u32,
    argument_bytes: u32,
    stack_page_address: u32,
    stack_page_owner: u32,
    stack_page_bytes: Vec<u8>,
    heap_state_owner: String,
    variant_id: String,
    creation_flags: u32,
    initial_size: u32,
    maximum_size: u32,
    preserve_last_error: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct X86LogicalHeapHandle {
    namespace: &'static str,
    value: u32,
    type_tag: u8,
    generation: u8,
    slot: u16,
}

impl X86LogicalHeapHandle {
    const NAMESPACE: &str = "vine.logical-private-heap.v1";
    const TYPE_TAG: u8 = 0x68;
    const GENERATION: u8 = 1;
    const SLOT: u16 = 1;

    fn first_private_heap() -> Self {
        let value = (u32::from(Self::TYPE_TAG) << 24)
            | (u32::from(Self::GENERATION) << 16)
            | u32::from(Self::SLOT);
        Self {
            namespace: Self::NAMESPACE,
            value,
            type_tag: Self::TYPE_TAG,
            generation: Self::GENERATION,
            slot: Self::SLOT,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct X86LogicalHeap {
    owner: String,
    variant_id: String,
    handle: X86LogicalHeapHandle,
    kind: &'static str,
    lifecycle: &'static str,
    creation_flags: u32,
    initial_size: u32,
    maximum_size: u32,
    growable: bool,
    backing_materialization: &'static str,
    live_allocations: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct V86HeapCreateReturnReceipt {
    schema: &'static str,
    schema_version: u32,
    thunk: ImportThunk,
    calling_convention: &'static str,
    argument_bytes: u32,
    stack_pointer: GuestAddress,
    return_address: GuestAddress,
    logical_heap: X86LogicalHeap,
    preserve_last_error: bool,
    result: u32,
    stack_reconciliation: GuestPageReconciliation,
    stats: MemoryStats,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct V86GetModuleHandleWReturnRequest {
    schema: String,
    schema_version: u32,
    reflected_generation: u64,
    thunk_id: u32,
    thunk_address: u32,
    stack_pointer: u32,
    return_address: u32,
    argument_bytes: u32,
    stack_page_address: u32,
    stack_page_owner: u32,
    stack_page_bytes: Vec<u8>,
    loaded_module_owner: String,
    variant_id: String,
    module_name_address: u32,
    module_name: String,
    preserve_last_error: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct X86LogicalLoadedModuleHandle {
    namespace: &'static str,
    value: u32,
    type_tag: u8,
    generation: u8,
    slot: u16,
}

impl X86LogicalLoadedModuleHandle {
    const NAMESPACE: &str = "vine.logical-loaded-module.v1";
    const TYPE_TAG: u8 = 0x4d;
    const GENERATION: u8 = 1;
    const SLOT: u16 = 1;

    fn kernel32() -> Self {
        let value = (u32::from(Self::TYPE_TAG) << 24)
            | (u32::from(Self::GENERATION) << 16)
            | u32::from(Self::SLOT);
        Self {
            namespace: Self::NAMESPACE,
            value,
            type_tag: Self::TYPE_TAG,
            generation: Self::GENERATION,
            slot: Self::SLOT,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct X86LogicalLoadedModule {
    owner: String,
    variant_id: String,
    handle: X86LogicalLoadedModuleHandle,
    kind: &'static str,
    lifecycle: &'static str,
    requested_name: String,
    canonical_name: &'static str,
    name_match: &'static str,
    module_role: &'static str,
    reference_count_changed: bool,
    guest_image_materialization: &'static str,
    export_resolution: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct V86GetModuleHandleWReturnReceipt {
    schema: &'static str,
    schema_version: u32,
    thunk: ImportThunk,
    calling_convention: &'static str,
    argument_bytes: u32,
    stack_pointer: GuestAddress,
    return_address: GuestAddress,
    module_name_address: GuestAddress,
    module_name_utf16_bytes: Vec<u8>,
    logical_module: X86LogicalLoadedModule,
    preserve_last_error: bool,
    result: u32,
    stack_reconciliation: GuestPageReconciliation,
    stats: MemoryStats,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct V86GetProcAddressReturnRequest {
    schema: String,
    schema_version: u32,
    reflected_generation: u64,
    thunk_id: u32,
    thunk_address: u32,
    stack_pointer: u32,
    return_address: u32,
    argument_bytes: u32,
    stack_page_address: u32,
    stack_page_owner: u32,
    stack_page_bytes: Vec<u8>,
    dynamic_export_owner: String,
    variant_id: String,
    module_handle: u32,
    lookup_kind: String,
    call_variant: String,
    procedure_name_address: u32,
    procedure_name: String,
    preserve_last_error: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct X86DynamicExportResolution {
    owner: String,
    variant_id: String,
    module_handle: X86LogicalLoadedModuleHandle,
    canonical_module: &'static str,
    lookup_kind: &'static str,
    call_variant: &'static str,
    requested_name: String,
    resolved_name: &'static str,
    endpoint_namespace: &'static str,
    endpoint_lifetime: &'static str,
    host_address_imported: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct V86GetProcAddressReturnReceipt {
    schema: &'static str,
    schema_version: u32,
    thunk: ImportThunk,
    calling_convention: &'static str,
    argument_bytes: u32,
    stack_pointer: GuestAddress,
    return_address: GuestAddress,
    procedure_name_address: GuestAddress,
    procedure_name_bytes: Vec<u8>,
    resolution: X86DynamicExportResolution,
    endpoint: DynamicExportThunk,
    preserve_last_error: bool,
    result: u32,
    stack_reconciliation: GuestPageReconciliation,
    stats: MemoryStats,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct V86GetProcAddressSecondReturnRequest {
    schema: String,
    schema_version: u32,
    reflected_generation: u64,
    thunk_id: u32,
    thunk_address: u32,
    stack_pointer: u32,
    return_address: u32,
    argument_bytes: u32,
    stack_page_address: u32,
    stack_page_owner: u32,
    stack_page_bytes: Vec<u8>,
    first_endpoint_store_page_address: u32,
    first_endpoint_store_page_owner: u32,
    first_endpoint_store_page_bytes: Vec<u8>,
    dynamic_export_owner: String,
    variant_id: String,
    module_handle: u32,
    lookup_kind: String,
    call_variant: String,
    indirect_call_register: String,
    indirect_call_target: u32,
    procedure_name_address: u32,
    procedure_name: String,
    first_endpoint_address: u32,
    first_endpoint_store_address: u32,
    preserve_last_error: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct V86GetProcAddressSecondReturnReceipt {
    schema: &'static str,
    schema_version: u32,
    thunk: ImportThunk,
    calling_convention: &'static str,
    argument_bytes: u32,
    stack_pointer: GuestAddress,
    return_address: GuestAddress,
    procedure_name_address: GuestAddress,
    procedure_name_bytes: Vec<u8>,
    indirect_call_register: &'static str,
    indirect_call_target: GuestAddress,
    resolution: X86DynamicExportResolution,
    first_endpoint: DynamicExportThunk,
    first_endpoint_store_address: GuestAddress,
    first_endpoint_store_value: u32,
    endpoint: DynamicExportThunk,
    preserve_last_error: bool,
    result: u32,
    stack_reconciliation: GuestPageReconciliation,
    first_endpoint_store_reconciliation: GuestPageReconciliation,
    stats: MemoryStats,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct V86GetProcAddressThirdReturnRequest {
    schema: String,
    schema_version: u32,
    reflected_generation: u64,
    thunk_id: u32,
    thunk_address: u32,
    stack_pointer: u32,
    return_address: u32,
    argument_bytes: u32,
    stack_page_address: u32,
    stack_page_owner: u32,
    stack_page_bytes: Vec<u8>,
    endpoint_store_page_address: u32,
    endpoint_store_page_owner: u32,
    endpoint_store_page_bytes: Vec<u8>,
    dynamic_export_owner: String,
    variant_id: String,
    module_handle: u32,
    lookup_kind: String,
    call_variant: String,
    indirect_call_register: String,
    indirect_call_target: u32,
    procedure_name_address: u32,
    procedure_name: String,
    first_endpoint_address: u32,
    second_endpoint_address: u32,
    first_endpoint_store_address: u32,
    second_endpoint_store_address: u32,
    preserve_last_error: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct V86GetProcAddressThirdReturnReceipt {
    schema: &'static str,
    schema_version: u32,
    thunk: ImportThunk,
    calling_convention: &'static str,
    argument_bytes: u32,
    stack_pointer: GuestAddress,
    return_address: GuestAddress,
    procedure_name_address: GuestAddress,
    procedure_name_bytes: Vec<u8>,
    indirect_call_register: &'static str,
    indirect_call_target: GuestAddress,
    resolution: X86DynamicExportResolution,
    first_endpoint: DynamicExportThunk,
    first_endpoint_store_address: GuestAddress,
    first_endpoint_store_value: u32,
    second_endpoint: DynamicExportThunk,
    second_endpoint_store_address: GuestAddress,
    second_endpoint_store_value: u32,
    endpoint: DynamicExportThunk,
    preserve_last_error: bool,
    result: u32,
    stack_reconciliation: GuestPageReconciliation,
    endpoint_store_reconciliation: GuestPageReconciliation,
    stats: MemoryStats,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct V86GetProcAddressFourthReturnRequest {
    schema: String,
    schema_version: u32,
    reflected_generation: u64,
    thunk_id: u32,
    thunk_address: u32,
    stack_pointer: u32,
    return_address: u32,
    argument_bytes: u32,
    stack_page_address: u32,
    stack_page_owner: u32,
    stack_page_bytes: Vec<u8>,
    endpoint_store_page_address: u32,
    endpoint_store_page_owner: u32,
    endpoint_store_page_bytes: Vec<u8>,
    dynamic_export_owner: String,
    variant_id: String,
    module_handle: u32,
    lookup_kind: String,
    call_variant: String,
    indirect_call_register: String,
    indirect_call_target: u32,
    procedure_name_address: u32,
    procedure_name: String,
    first_endpoint_address: u32,
    second_endpoint_address: u32,
    third_endpoint_address: u32,
    first_endpoint_store_address: u32,
    second_endpoint_store_address: u32,
    third_endpoint_store_address: u32,
    preserve_last_error: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct V86GetProcAddressFourthReturnReceipt {
    schema: &'static str,
    schema_version: u32,
    thunk: ImportThunk,
    calling_convention: &'static str,
    argument_bytes: u32,
    stack_pointer: GuestAddress,
    return_address: GuestAddress,
    procedure_name_address: GuestAddress,
    procedure_name_bytes: Vec<u8>,
    indirect_call_register: &'static str,
    indirect_call_target: GuestAddress,
    resolution: X86DynamicExportResolution,
    first_endpoint: DynamicExportThunk,
    first_endpoint_store_address: GuestAddress,
    first_endpoint_store_value: u32,
    second_endpoint: DynamicExportThunk,
    second_endpoint_store_address: GuestAddress,
    second_endpoint_store_value: u32,
    third_endpoint: DynamicExportThunk,
    third_endpoint_store_address: GuestAddress,
    third_endpoint_store_value: u32,
    endpoint: DynamicExportThunk,
    preserve_last_error: bool,
    result: u32,
    stack_reconciliation: GuestPageReconciliation,
    endpoint_store_reconciliation: GuestPageReconciliation,
    stats: MemoryStats,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct V86TlsAllocReturnRequest {
    schema: String,
    schema_version: u32,
    reflected_generation: u64,
    thunk_id: u32,
    thunk_address: u32,
    stack_pointer: u32,
    return_address: u32,
    argument_bytes: u32,
    stack_page_address: u32,
    stack_page_owner: u32,
    stack_page_bytes: Vec<u8>,
    endpoint_store_page_address: u32,
    endpoint_store_page_owner: u32,
    endpoint_store_page_bytes: Vec<u8>,
    first_endpoint_address: u32,
    second_endpoint_address: u32,
    third_endpoint_address: u32,
    fourth_endpoint_address: u32,
    first_endpoint_store_address: u32,
    second_endpoint_store_address: u32,
    third_endpoint_store_address: u32,
    fourth_endpoint_store_address: u32,
    tls_state_owner: String,
    variant_id: String,
    logical_index_namespace: String,
    logical_index: u32,
    failure_sentinel: u32,
    preserve_last_error: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct X86LogicalTlsIndex {
    namespace: &'static str,
    value: u32,
    failure_sentinel: u32,
}

impl X86LogicalTlsIndex {
    const NAMESPACE: &str = "vine.logical-tls-index.v1";
    const FIRST_INDEX: u32 = 0;
    const FAILURE_SENTINEL: u32 = u32::MAX;

    fn first() -> Self {
        Self {
            namespace: Self::NAMESPACE,
            value: Self::FIRST_INDEX,
            failure_sentinel: Self::FAILURE_SENTINEL,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct X86LogicalTlsSlot {
    owner: String,
    variant_id: String,
    index: X86LogicalTlsIndex,
    lifecycle: &'static str,
    storage_scope: &'static str,
    initialized_thread_values: u32,
    backing_materialization: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct V86TlsAllocReturnReceipt {
    schema: &'static str,
    schema_version: u32,
    thunk: ImportThunk,
    calling_convention: &'static str,
    argument_bytes: u32,
    stack_pointer: GuestAddress,
    return_address: GuestAddress,
    logical_tls_slot: X86LogicalTlsSlot,
    preserve_last_error: bool,
    result: u32,
    first_endpoint_store_address: GuestAddress,
    first_endpoint_store_value: u32,
    second_endpoint_store_address: GuestAddress,
    second_endpoint_store_value: u32,
    third_endpoint_store_address: GuestAddress,
    third_endpoint_store_value: u32,
    fourth_endpoint_store_address: GuestAddress,
    fourth_endpoint_store_value: u32,
    stack_reconciliation: GuestPageReconciliation,
    endpoint_store_reconciliation: GuestPageReconciliation,
    stats: MemoryStats,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct V86TlsSetValueReturnRequest {
    schema: String,
    schema_version: u32,
    reflected_generation: u64,
    thunk_id: u32,
    thunk_address: u32,
    stack_pointer: u32,
    return_address: u32,
    argument_bytes: u32,
    indirect_call_register: String,
    indirect_call_target: u32,
    stack_page_address: u32,
    stack_page_owner: u32,
    stack_page_bytes: Vec<u8>,
    endpoint_store_page_address: u32,
    endpoint_store_page_owner: u32,
    endpoint_store_page_bytes: Vec<u8>,
    tls_index_store_address: u32,
    tls_index_store_page_address: u32,
    tls_index_store_page_owner: u32,
    tls_index_store_page_bytes: Vec<u8>,
    endpoint_store_addresses: [u32; 4],
    endpoint_addresses: [u32; 4],
    tls_state_owner: String,
    allocation_variant_id: String,
    logical_index_namespace: String,
    logical_index: u32,
    expected_initialized_thread_values: u32,
    thread_identity_owner: String,
    thread_id: u32,
    value_kind: String,
    value_endpoint_namespace: String,
    value_endpoint_id: u32,
    value_address: u32,
    preserve_last_error: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct X86LogicalTlsThreadValue {
    thread_identity_owner: String,
    thread_id: u32,
    index_namespace: &'static str,
    index: u32,
    value_kind: &'static str,
    endpoint: DynamicExportThunk,
    lifecycle: &'static str,
}

#[derive(Clone, Copy)]
struct X86TlsEvidenceState<'a> {
    slot: &'a X86LogicalTlsSlot,
    thread_value: &'a X86LogicalTlsThreadValue,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct V86TlsSetValueReturnReceipt {
    schema: &'static str,
    schema_version: u32,
    thunk: ImportThunk,
    calling_convention: &'static str,
    argument_bytes: u32,
    stack_pointer: GuestAddress,
    return_address: GuestAddress,
    logical_tls_slot_before: X86LogicalTlsSlot,
    logical_tls_slot_after: X86LogicalTlsSlot,
    thread_value: X86LogicalTlsThreadValue,
    preserve_last_error: bool,
    result: u32,
    endpoint_store_addresses: [GuestAddress; 4],
    endpoint_store_values: [u32; 4],
    tls_index_store_address: GuestAddress,
    tls_index_store_value: u32,
    stack_reconciliation: GuestPageReconciliation,
    endpoint_store_reconciliation: GuestPageReconciliation,
    tls_index_store_reconciliation: GuestPageReconciliation,
    stats: MemoryStats,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct V86EncodePointerReturnRequest {
    schema: String,
    schema_version: u32,
    reflected_generation: u64,
    thunk_id: u32,
    thunk_address: u32,
    stack_pointer: u32,
    return_address: u32,
    argument_bytes: u32,
    source_pointer: u32,
    stack_page_address: u32,
    stack_page_owner: u32,
    stack_page_bytes: Vec<u8>,
    pointer_encoding_owner: String,
    variant_id: String,
    encoded_pointer_namespace: String,
    encoded_pointer_value: u32,
    expected_tls_index: u32,
    expected_tls_thread_id: u32,
    expected_tls_value_endpoint: u32,
    preserve_last_error: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct X86LogicalEncodedPointer {
    owner: String,
    variant_id: String,
    namespace: &'static str,
    value: u32,
    source_kind: &'static str,
    source_pointer: u32,
    lifetime: &'static str,
    process_cookie_materialization: &'static str,
    native_result_imported: bool,
}

impl X86LogicalEncodedPointer {
    const NAMESPACE: &str = "vine.logical-encoded-pointer.v1";
    const ENCODED_NULL: u32 = 0xec01_0001;
    const ENCODED_PROCESS_INITIALIZER: u32 = 0xec01_0002;
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct V86EncodePointerReturnReceipt {
    schema: &'static str,
    schema_version: u32,
    thunk: ImportThunk,
    calling_convention: &'static str,
    argument_bytes: u32,
    stack_pointer: GuestAddress,
    return_address: GuestAddress,
    encoded_pointer: X86LogicalEncodedPointer,
    preserved_tls_slot: X86LogicalTlsSlot,
    preserved_tls_thread_value: X86LogicalTlsThreadValue,
    preserve_last_error: bool,
    result: u32,
    stack_reconciliation: GuestPageReconciliation,
    stats: MemoryStats,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct V86EncodePointerSecondReturnRequest {
    schema: String,
    schema_version: u32,
    reflected_generation: u64,
    thunk_id: u32,
    thunk_address: u32,
    stack_pointer: u32,
    return_address: u32,
    argument_bytes: u32,
    source_pointer: u32,
    source_page_address: u32,
    source_page_owner: u32,
    source_function_prefix: Vec<u8>,
    stack_page_address: u32,
    stack_page_owner: u32,
    stack_page_bytes: Vec<u8>,
    pointer_encoding_owner: String,
    variant_id: String,
    encoded_pointer_namespace: String,
    encoded_pointer_value: u32,
    expected_encoded_null_namespace: String,
    expected_encoded_null_value: u32,
    expected_tls_index: u32,
    expected_tls_thread_id: u32,
    expected_tls_value_endpoint: u32,
    preserve_last_error: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct V86EncodePointerSecondReturnReceipt {
    schema: &'static str,
    schema_version: u32,
    thunk: ImportThunk,
    calling_convention: &'static str,
    argument_bytes: u32,
    stack_pointer: GuestAddress,
    return_address: GuestAddress,
    source_function_prefix: Vec<u8>,
    encoded_pointer: X86LogicalEncodedPointer,
    preserved_encoded_null: X86LogicalEncodedPointer,
    preserved_tls_slot: X86LogicalTlsSlot,
    preserved_tls_thread_value: X86LogicalTlsThreadValue,
    preserve_last_error: bool,
    result: u32,
    stack_reconciliation: GuestPageReconciliation,
    stats: MemoryStats,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let failure = FailureReport {
                schema: "vine.pe32-loader-failure",
                schema_version: 1,
                error,
            };
            match serde_json::to_string_pretty(&failure) {
                Ok(json) => eprintln!("{json}"),
                Err(_) => eprintln!("PE probe failed before its diagnostic could be serialized"),
            }
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), LoaderError> {
    let arguments = parse_arguments()?;
    let limits = Pe32Limits::default();
    let bytes = read_bounded(&arguments.input, limits.max_file_bytes)?;
    let snapshot = ModuleSnapshot::from_bytes(bytes, &limits)?;
    if let Some(expected) = &arguments.expected_sha256 {
        snapshot.verify_expected_sha256(expected)?;
    }
    let loader_report = build_pe32_load_plan(&snapshot, limits)?;
    if arguments.v86_reflection_session {
        return run_v86_reflection_session(arguments, &snapshot, &loader_report);
    }
    let mut json = if !arguments.runtime_exports.is_empty() || arguments.trap_unsupported_imports {
        let load_base = arguments.materialize_image_base.ok_or_else(usage_error)?;
        let max_physical_pages = arguments.prepare_memory_pages.ok_or_else(usage_error)?;
        let memory_limits = MemoryLimits {
            max_physical_pages,
            ..MemoryLimits::default()
        };
        let mut memory = GuestMemory::new(memory_limits).map_err(memory_error)?;
        let materialization = materialize_pe32_image(
            &mut memory,
            &snapshot,
            &loader_report,
            GuestAddress(load_base),
        )?;
        let catalog = RuntimeExportCatalog::new(arguments.runtime_exports.clone())?;
        let address_allocator =
            ProcessAddressSpaceAllocator::vine_v1(ProcessAddressSpaceLayout::vine_v1())
                .map_err(runtime_state_error)?;
        let import_allocation = fixed_mapping_allocation(
            &address_allocator,
            ProcessAddressAllocationPurpose::ImportThunks,
        )?;
        let binding = if arguments.trap_unsupported_imports {
            bind_pe32_imports_with_unsupported_traps(
                &mut memory,
                &import_allocation,
                &loader_report,
                &materialization,
                &catalog,
            )?
        } else {
            bind_pe32_imports(
                &mut memory,
                &import_allocation,
                &loader_report,
                &materialization,
                &catalog,
            )?
        };
        let report = ImportBindingIngressReport {
            schema: "vine.pe32-import-binding",
            schema_version: 1,
            loader_schema: loader_report.schema,
            loader_schema_version: loader_report.schema_version,
            memory_limits,
            runtime_exports: arguments.runtime_exports,
            trap_unsupported_imports: arguments.trap_unsupported_imports,
            materialization,
            binding,
            stats: memory.stats(),
        };
        serialize_report(&report)?
    } else if let Some(load_base) = arguments.materialize_image_base {
        let max_physical_pages = arguments.prepare_memory_pages.ok_or_else(usage_error)?;
        let memory_limits = MemoryLimits {
            max_physical_pages,
            ..MemoryLimits::default()
        };
        let mut memory = GuestMemory::new(memory_limits).map_err(memory_error)?;
        let materialization = materialize_pe32_image(
            &mut memory,
            &snapshot,
            &loader_report,
            GuestAddress(load_base),
        )?;
        let report = MaterializationIngressReport {
            schema: "vine.pe32-materialization",
            schema_version: 1,
            loader_schema: loader_report.schema,
            loader_schema_version: loader_report.schema_version,
            memory_limits,
            materialization,
            stats: memory.stats(),
        };
        serialize_report(&report)?
    } else if let Some(max_physical_pages) = arguments.prepare_memory_pages {
        let memory_limits = MemoryLimits {
            max_physical_pages,
            ..MemoryLimits::default()
        };
        let mut memory = GuestMemory::new(memory_limits).map_err(memory_error)?;
        let layout = memory
            .prepare_image_layout(&loader_report.source, &loader_report.load_plan)
            .map_err(memory_error)?;
        let report = MemoryIngressReport {
            schema: "vine.guest-memory-ingress",
            schema_version: 1,
            source: loader_report.source.clone(),
            loader_schema: loader_report.schema,
            loader_schema_version: loader_report.schema_version,
            memory_limits,
            image: MemoryImageSummary {
                preferred_image_base: loader_report.load_plan.image.preferred_image_base,
                size_of_image: loader_report.load_plan.image.size_of_image,
                entry_point_address: loader_report.load_plan.image.entry_point_address,
                section_count: loader_report.load_plan.sections.len(),
            },
            layout,
            stats: memory.stats(),
            stop: UnsupportedPoint {
                stage: "image-materialization",
                code: "image-bytes-relocations-import-binding-unimplemented",
                message: "the sparse image region and zeroed physical pages are owned; source copy, zero-fill verification, relocations, IAT binding, owner sealing, and guest execution were not attempted",
            },
        };
        serialize_report(&report)?
    } else {
        serialize_report(&loader_report)?
    };
    json.push(b'\n');

    if let Some(output) = arguments.output {
        write_report(&output, &json)?;
        println!("wrote runtime ingress report to {}", output.display());
    } else {
        io::stdout().write_all(&json).map_err(|error| {
            io_error(
                "probe-output",
                "stdout-write-failed",
                "could not write the runtime ingress report to stdout",
                error,
            )
        })?;
    }
    Ok(())
}

fn run_v86_reflection_session(
    arguments: Arguments,
    snapshot: &ModuleSnapshot,
    loader_report: &LoaderIngressReport,
) -> Result<(), LoaderError> {
    const MAX_REFLECTION_PAGES: usize = 16;
    const MAX_LARGE_IMAGE_REFLECTION_PAGES: usize = 16 * 1024;
    const MAX_REFLECTION_MESSAGE_BYTES: usize = 512 * 1024;
    const MAX_LARGE_IMAGE_REFLECTION_MESSAGE_BYTES: usize = 8 * 1024 * 1024;
    let generic_runtime_reentry = arguments.generic_runtime_reentry;
    let generic_runtime_capability = arguments.generic_runtime_capability;
    let generic_runtime_trap_limit = arguments.generic_runtime_trap_limit;
    let ansi_command_line_enabled = arguments
        .kernel32_policy
        .as_ref()
        .is_some_and(Kernel32CompatibilityPolicy::allows_ansi_command_line);
    let wide_environment_strings_enabled = arguments
        .kernel32_policy
        .as_ref()
        .is_some_and(Kernel32CompatibilityPolicy::allows_wide_environment_strings);
    let kernel32_policy = arguments.kernel32_policy;
    let source_manifest = arguments.source_manifest;
    let writable_profile = arguments.writable_profile;
    if source_manifest
        .as_ref()
        .is_some_and(|manifest| manifest.title_fingerprint != snapshot.identity().sha256)
    {
        return Err(LoaderError::invalid_input(
            "source-snapshot-manifest",
            "source-snapshot-title-fingerprint-mismatch",
            "source snapshot title fingerprint does not match the prepared PE source",
        ));
    }
    if writable_profile
        .as_ref()
        .is_some_and(|profile| profile.title_fingerprint != snapshot.identity().sha256)
    {
        return Err(LoaderError::invalid_input(
            "writable-profile-manifest",
            "writable-profile-title-fingerprint-mismatch",
            "writable profile title fingerprint does not match the prepared PE source",
        ));
    }
    let supported_call_count = arguments
        .checkpoint_session
        .as_ref()
        .map_or(0, |selection| selection.supported_call_count);
    let get_proc_address_occurrences = arguments
        .checkpoint_session
        .as_ref()
        .map_or(0, |selection| {
            selection.dispatch_occurrences("kernel32.dll", "GetProcAddress")
        });
    let load_base = arguments.materialize_image_base.ok_or_else(usage_error)?;
    let max_physical_pages = arguments.prepare_memory_pages.ok_or_else(usage_error)?;
    let reflection_page_limit = if arguments.large_image_reflection {
        MAX_LARGE_IMAGE_REFLECTION_PAGES
    } else {
        MAX_REFLECTION_PAGES
    };
    if max_physical_pages > reflection_page_limit {
        let message = if arguments.large_image_reflection {
            "the version 1 large-image reflection session is limited to 16384 physical pages"
        } else {
            "the version 1 reflection session is limited to 16 physical pages"
        };
        return Err(LoaderError::resource_limit(
            "memory-reflection",
            "reflection-page-limit-exceeded",
            message,
        ));
    }
    let reflection_message_limit = if arguments.large_image_reflection {
        MAX_LARGE_IMAGE_REFLECTION_MESSAGE_BYTES
    } else {
        MAX_REFLECTION_MESSAGE_BYTES
    };
    let memory_limits = MemoryLimits {
        max_physical_pages,
        ..MemoryLimits::default()
    };
    let mut memory = GuestMemory::new(memory_limits).map_err(memory_error)?;
    let control_address_allocator =
        ProcessAddressSpaceAllocator::vine_v1(ProcessAddressSpaceLayout::vine_v1())
            .map_err(runtime_state_error)?;
    let import_allocation = fixed_mapping_allocation(
        &control_address_allocator,
        ProcessAddressAllocationPurpose::ImportThunks,
    )?;
    let callback_allocation = fixed_mapping_allocation(
        &control_address_allocator,
        ProcessAddressAllocationPurpose::CallbackReturns,
    )?;
    let dynamic_export_allocation = fixed_mapping_allocation(
        &control_address_allocator,
        ProcessAddressAllocationPurpose::DynamicExports,
    )?;
    let materialization = materialize_pe32_image(
        &mut memory,
        snapshot,
        loader_report,
        GuestAddress(load_base),
    )?;
    let trap_unsupported_imports = arguments.trap_unsupported_imports;
    let declared_catalog = RuntimeExportCatalog::new(arguments.runtime_exports)?;
    let mut generic_runtime = None;
    let binding = if generic_runtime_reentry {
        if !trap_unsupported_imports {
            return Err(LoaderError::invalid_input(
                "runtime-reentry",
                "generic-runtime-reentry-trap-policy-missing",
                "generic runtime re-entry requires explicit ownership of every imported endpoint",
            ));
        }
        let runtime_catalog = runtime_catalog_for_load_plan(loader_report)?;
        let (runtime, evidence_binding) = prepare_generic_runtime_process(
            &mut memory,
            loader_report,
            &materialization,
            &runtime_catalog,
            &declared_catalog,
            GenericRuntimeProfiles {
                source_manifest,
                writable_profile,
                clock_profile: kernel32_policy
                    .as_ref()
                    .and_then(|policy| policy.clock_profile().cloned()),
            },
        )?;
        generic_runtime = Some(runtime);
        evidence_binding
    } else if trap_unsupported_imports {
        bind_pe32_imports_with_unsupported_traps(
            &mut memory,
            &import_allocation,
            loader_report,
            &materialization,
            &declared_catalog,
        )?
    } else {
        bind_pe32_imports(
            &mut memory,
            &import_allocation,
            loader_report,
            &materialization,
            &declared_catalog,
        )?
    };
    if !binding.complete || !binding.image_owner_sealed {
        return Err(LoaderError::invalid_input(
            "memory-reflection",
            "image-import-binding-incomplete",
            "v86 reflection requires an image whose imports and owner are fully bound and sealed",
        ));
    }
    let first_thunk = binding.thunks.first().cloned();
    if first_thunk.is_none() && binding.total_imports != 0 {
        return Err(LoaderError::runtime_defect(
            "memory-reflection",
            "complete-import-image-has-no-owned-thunk",
            "a fully bound image with planned imports has no runtime-owned thunk",
        ));
    }
    let (callback_return_sentinel, process_entry_return_sentinel) =
        if arguments.install_process_entry_return_sentinel {
            let sentinels = install_runtime_return_sentinels(&mut memory, &callback_allocation)
                .map_err(memory_error)?;
            (
                Some(sentinels.callback_return),
                Some(sentinels.process_entry_return),
            )
        } else {
            (
                arguments
                    .install_callback_return_sentinel
                    .then(|| {
                        install_callback_return_sentinel(&mut memory, &callback_allocation)
                            .map_err(memory_error)
                    })
                    .transpose()?,
                None,
            )
        };
    let initial_thread_stack = if arguments.install_controlled_initial_stack {
        let entry_return = process_entry_return_sentinel.as_ref().ok_or_else(|| {
            LoaderError::runtime_defect(
                "process-bootstrap",
                "process-entry-return-sentinel-missing",
                "controlled initial stack requires process-entry return metadata",
            )
        })?;
        Some(if generic_runtime_reentry {
            install_initial_thread_stack(
                &mut memory,
                entry_return,
                loader_report.load_plan.image.stack_reserve,
                loader_report.load_plan.image.stack_commit,
            )
            .map_err(memory_error)?
        } else {
            install_controlled_initial_stack(&mut memory, entry_return).map_err(memory_error)?
        })
    } else {
        None
    };
    if let Some(runtime) = generic_runtime.as_mut() {
        runtime
            .runtime
            .register_initial_thread_stack(initial_thread_stack.as_ref().ok_or_else(|| {
                LoaderError::runtime_defect(
                    "runtime-reentry",
                    "initial-thread-stack-missing",
                    "generic runtime process requires an initial thread stack allocation",
                )
            })?)
            .map_err(runtime_state_error)?;
    }
    let initial_process_environment = if arguments.install_controlled_process_parameters {
        let command_line = arguments
            .controlled_command_line
            .as_deref()
            .unwrap_or(CONTROLLED_COMMAND_LINE);
        Some(
            if let Some(runtime) = generic_runtime
                .as_mut()
                .filter(|_| wide_environment_strings_enabled)
            {
                ExecutionSession::new(&mut memory, &mut runtime.runtime)
                    .install_controlled_process_environment_with_command_line_ansi_and_environment_strings(
                        command_line,
                    )
                    .map_err(runtime_state_error)?
            } else if let Some(runtime) = generic_runtime
                .as_mut()
                .filter(|_| ansi_command_line_enabled)
            {
                ExecutionSession::new(&mut memory, &mut runtime.runtime)
                    .install_controlled_process_environment_with_command_line_and_ansi(command_line)
                    .map_err(runtime_state_error)?
            } else {
                let mut allocator = generic_runtime
                    .as_ref()
                    .map(|runtime| runtime.runtime.process_address_space_allocator().clone())
                    .unwrap_or_else(|| control_address_allocator.clone());
                let allocation = allocator
                    .reserve(vine_core::controlled_process_environment_allocation_request(true))
                    .map_err(runtime_state_error)?;
                if wide_environment_strings_enabled {
                    install_controlled_process_environment_with_command_line_ansi_and_environment_strings(
                    &mut memory,
                    &allocation,
                    command_line,
                )
                .map_err(memory_error)?
                } else if ansi_command_line_enabled {
                    install_controlled_process_environment_with_command_line_and_ansi(
                        &mut memory,
                        &allocation,
                        command_line,
                    )
                    .map_err(memory_error)?
                } else if arguments.controlled_command_line.is_some() {
                    install_controlled_process_environment_with_command_line(
                        &mut memory,
                        &allocation,
                        command_line,
                    )
                    .map_err(memory_error)?
                } else {
                    install_controlled_process_environment_with_parameters(&mut memory, &allocation)
                        .map_err(memory_error)?
                }
            },
        )
    } else {
        if arguments.install_controlled_process_environment {
            let mut allocator = control_address_allocator.clone();
            let allocation = allocator
                .reserve(vine_core::controlled_process_environment_allocation_request(false))
                .map_err(runtime_state_error)?;
            Some(
                install_controlled_process_environment(&mut memory, &allocation)
                    .map_err(memory_error)?,
            )
        } else {
            None
        }
    };
    let dynamic_export_thunks = if generic_runtime_reentry {
        let mut exports = vec![
            ("kernel32.dll", "FlsAlloc"),
            ("kernel32.dll", "FlsGetValue"),
            ("kernel32.dll", "FlsSetValue"),
            ("kernel32.dll", "FlsFree"),
        ];
        if matches!(
            generic_runtime_capability,
            Some(
                GenericRuntimeCapability::LoadLibraryA
                    | GenericRuntimeCapability::GetClientRect
                    | GenericRuntimeCapability::WindowDcFill
                    | GenericRuntimeCapability::SetWindowLongA
                    | GenericRuntimeCapability::SetWindowPosActivation
                    | GenericRuntimeCapability::MonitorFromWindow
                    | GenericRuntimeCapability::D3d9GetAdapterCount
                    | GenericRuntimeCapability::D3d9GetAdapterMonitor
                    | GenericRuntimeCapability::D3d9CreateDevice
                    | GenericRuntimeCapability::D3d9CreateTexture
                    | GenericRuntimeCapability::D3d9GetSurfaceLevel
                    | GenericRuntimeCapability::D3d9SurfaceGetDc
                    | GenericRuntimeCapability::TimeGetTime
                    | GenericRuntimeCapability::D3d9InitialTextures
                    | GenericRuntimeCapability::D3d9InitialVertexBuffer
                    | GenericRuntimeCapability::D3d9InitialRenderStates
                    | GenericRuntimeCapability::D3d9RasterStatus
                    | GenericRuntimeCapability::QueryPerformanceFrequency
                    | GenericRuntimeCapability::GetMonitorInfoA
                    | GenericRuntimeCapability::Dinput8Factory
                    | GenericRuntimeCapability::Dinput8EnumDevices
                    | GenericRuntimeCapability::DsoundRootAndCooperativeLevel
                    | GenericRuntimeCapability::DsoundInitialBuffers
                    | GenericRuntimeCapability::DsoundBufferLockUnlockPlay
                    | GenericRuntimeCapability::SetTimer
                    | GenericRuntimeCapability::CreateThread
                    | GenericRuntimeCapability::SetThreadPriority
                    | GenericRuntimeCapability::ScreenDc
                    | GenericRuntimeCapability::FontEnumeration
                    | GenericRuntimeCapability::FontCreation
                    | GenericRuntimeCapability::CompatibleDc
                    | GenericRuntimeCapability::DibLifecycle
                    | GenericRuntimeCapability::OutlineTextMetrics
                    | GenericRuntimeCapability::TextRaster
                    | GenericRuntimeCapability::GdiplusStartup
                    | GenericRuntimeCapability::VirtualAlloc
                    | GenericRuntimeCapability::Imm32Context
                    | GenericRuntimeCapability::SetWaitableTimer
                    | GenericRuntimeCapability::WaitForSingleObjectEx
                    | GenericRuntimeCapability::DragAcceptFiles
                    | GenericRuntimeCapability::SleepZero
                    | GenericRuntimeCapability::GlobalMemoryStatus
                    | GenericRuntimeCapability::GetFileAttributesA
                    | GenericRuntimeCapability::CreateDirectoryA
                    | GenericRuntimeCapability::D3d9RecreatedTextures
                    | GenericRuntimeCapability::D3d9Clear
                    | GenericRuntimeCapability::SetRect
                    | GenericRuntimeCapability::D3d9AddDirtyRect
                    | GenericRuntimeCapability::D3d9UpdateTexture
                    | GenericRuntimeCapability::D3d9BeginScene
                    | GenericRuntimeCapability::D3d9InitialSamplerStates
                    | GenericRuntimeCapability::D3d9InitialTextureStageStates
                    | GenericRuntimeCapability::D3d9SetTexture
                    | GenericRuntimeCapability::D3d9SetFvf
                    | GenericRuntimeCapability::D3d9SetStreamSource
                    | GenericRuntimeCapability::D3d9DrawPrimitive
                    | GenericRuntimeCapability::D3d9EndScene
                    | GenericRuntimeCapability::D3d9PostEndSceneRasterStatus
                    | GenericRuntimeCapability::D3d9PostRasterStatusWaitableTimer
                    | GenericRuntimeCapability::D3d9PostScenePresentGateway
                    | GenericRuntimeCapability::D3d9Present
                    | GenericRuntimeCapability::D3d9PostPresentContinuation
                    | GenericRuntimeCapability::User32AsyncKeyStateInput
                    | GenericRuntimeCapability::User32AsyncKeyStateInitialScan
                    | GenericRuntimeCapability::User32WindowTimerCallbackDelivery
                    | GenericRuntimeCapability::User32PointerButtonMessages
                    | GenericRuntimeCapability::User32SetFocusSameWindow
                    | GenericRuntimeCapability::User32DefWindowProcPointerDown
                    | GenericRuntimeCapability::User32DefWindowProcPointerRelease
            )
        ) {
            let loader = kernel32_policy
                .as_ref()
                .and_then(Kernel32CompatibilityPolicy::load_library_a)
                .ok_or_else(|| {
                    LoaderError::invalid_input(
                        "runtime-reentry",
                        "load-library-a-policy-missing",
                        "LoadLibraryA capability requires a runtime-system module policy",
                    )
                })?;
            for module in &loader.runtime_system_modules {
                for export in &module.exports {
                    exports.push((module.canonical_name.as_str(), export.as_str()));
                }
            }
        }
        install_dynamic_export_thunks(&mut memory, &dynamic_export_allocation, &exports)
            .map_err(memory_error)?
    } else if get_proc_address_occurrences >= 4 {
        install_dynamic_export_thunks(
            &mut memory,
            &dynamic_export_allocation,
            &[
                ("kernel32.dll", "FlsAlloc"),
                ("kernel32.dll", "FlsGetValue"),
                ("kernel32.dll", "FlsSetValue"),
                ("kernel32.dll", "FlsFree"),
            ],
        )
        .map_err(memory_error)?
    } else if get_proc_address_occurrences >= 3 {
        install_dynamic_export_thunks(
            &mut memory,
            &dynamic_export_allocation,
            &[
                ("kernel32.dll", "FlsAlloc"),
                ("kernel32.dll", "FlsGetValue"),
                ("kernel32.dll", "FlsSetValue"),
            ],
        )
        .map_err(memory_error)?
    } else if get_proc_address_occurrences >= 2 {
        install_dynamic_export_thunks(
            &mut memory,
            &dynamic_export_allocation,
            &[
                ("kernel32.dll", "FlsAlloc"),
                ("kernel32.dll", "FlsGetValue"),
            ],
        )
        .map_err(memory_error)?
    } else if get_proc_address_occurrences >= 1 {
        vec![
            install_dynamic_export_thunk(
                &mut memory,
                &dynamic_export_allocation,
                "kernel32.dll",
                "FlsAlloc",
            )
            .map_err(memory_error)?,
        ]
    } else {
        Vec::new()
    };
    if let Some(runtime) = generic_runtime.as_mut() {
        let callback_return = callback_return_sentinel.as_ref().ok_or_else(|| {
            LoaderError::runtime_defect(
                "runtime-reentry",
                "generic-runtime-callback-sentinel-missing",
                "generic runtime process lifecycle requires the process callback sentinel",
            )
        })?;
        finalize_generic_runtime_process(runtime, &dynamic_export_thunks, callback_return.address)?;
    }
    let dynamic_export_thunk = dynamic_export_thunks.first().cloned();
    let expected_invalidation = dynamic_export_thunks
        .last()
        .map(|thunk| thunk.invalidation)
        .or_else(|| {
            initial_process_environment
                .as_ref()
                .map(|environment| environment.invalidation)
        })
        .or_else(|| {
            initial_thread_stack
                .as_ref()
                .map(|stack| stack.invalidation)
        })
        .or_else(|| {
            process_entry_return_sentinel
                .as_ref()
                .map(|sentinel| sentinel.invalidation)
        })
        .or_else(|| {
            callback_return_sentinel
                .as_ref()
                .map(|sentinel| sentinel.invalidation)
        })
        .unwrap_or(binding.invalidation);
    let reflection = memory.pending_reflection().map_err(memory_error)?;
    if reflection.invalidation != expected_invalidation {
        return Err(LoaderError::runtime_defect(
            "memory-reflection",
            "runtime-reflection-generation-mismatch",
            "reflected memory does not carry the completed runtime-control generation",
        ));
    }
    let offer = V86ReflectionOffer {
        schema: "vine.v86-memory-reflection",
        schema_version: 1,
        source: loader_report.source.clone(),
        image_load_base: materialization.load_base,
        entry_point_address: materialization.entry_point_address,
        image_owner: binding.image_owner,
        first_thunk: first_thunk.clone(),
        thunks: binding.thunks.clone(),
        callback_return_sentinel: callback_return_sentinel.clone(),
        process_entry_return_sentinel: process_entry_return_sentinel.clone(),
        initial_thread_stack: initial_thread_stack.clone(),
        initial_process_environment: initial_process_environment.clone(),
        dynamic_export_thunk: dynamic_export_thunk.clone(),
        dynamic_export_thunks: if get_proc_address_occurrences >= 2 {
            dynamic_export_thunks.clone()
        } else {
            Vec::new()
        },
        reflection,
    };
    write_protocol_message(&offer, reflection_message_limit)?;

    let stdin = io::stdin();
    let mut protocol_input = stdin.lock();
    let ack = read_v86_reflection_ack(&mut protocol_input)?;
    validate_v86_reflection_ack(&ack, offer.reflection.invalidation)?;
    memory
        .acknowledge_invalidation(ack.generation)
        .map_err(memory_error)?;
    let iat_value = first_thunk
        .as_ref()
        .map(|thunk| {
            let iat_bytes = memory.read(thunk.iat_address, 4).map_err(memory_error)?;
            let iat_value = u32::from_le_bytes(iat_bytes.try_into().map_err(|_| {
                LoaderError::runtime_defect(
                    "memory-reflection",
                    "acknowledged-iat-width-invalid",
                    "acknowledged IAT read did not return four bytes",
                )
            })?);
            if iat_value != thunk.address.0 {
                return Err(LoaderError::runtime_defect(
                    "memory-reflection",
                    "acknowledged-iat-value-mismatch",
                    "acknowledged IAT no longer points to the first owned thunk",
                ));
            }
            Ok(iat_value)
        })
        .transpose()?;
    let entry_bytes = memory
        .fetch(materialization.entry_point_address, 16)
        .map_err(memory_error)?;
    let thunk_bytes = first_thunk
        .as_ref()
        .map(|thunk| memory.fetch(thunk.address, 16).map_err(memory_error))
        .transpose()?;
    let callback_return_bytes = callback_return_sentinel
        .as_ref()
        .map(|sentinel| memory.fetch(sentinel.address, 16).map_err(memory_error))
        .transpose()?;
    let process_entry_return_bytes = process_entry_return_sentinel
        .as_ref()
        .map(|sentinel| memory.fetch(sentinel.address, 16).map_err(memory_error))
        .transpose()?;
    let initial_stack_top_bytes = initial_thread_stack
        .as_ref()
        .map(|stack| {
            let address = GuestAddress(stack.stack_top.0.checked_sub(16).ok_or_else(|| {
                LoaderError::runtime_defect(
                    "process-bootstrap",
                    "initial-stack-top-underflow",
                    "controlled initial stack cannot expose its top bytes",
                )
            })?);
            memory.read(address, 16).map_err(memory_error)
        })
        .transpose()?;
    let teb_bytes = initial_process_environment
        .as_ref()
        .map(|environment| {
            memory
                .read(environment.teb_address, 64)
                .map_err(memory_error)
        })
        .transpose()?;
    let peb_bytes = initial_process_environment
        .as_ref()
        .map(|environment| {
            memory
                .read(environment.peb_address, 64)
                .map_err(memory_error)
        })
        .transpose()?;
    let process_parameters_bytes = initial_process_environment
        .as_ref()
        .and_then(|environment| environment.process_parameters.as_ref())
        .map(|parameters| {
            memory
                .read(parameters.process_parameters_address, 0x48)
                .map_err(memory_error)
        })
        .transpose()?;
    let command_line_bytes = initial_process_environment
        .as_ref()
        .and_then(|environment| environment.process_parameters.as_ref())
        .map(|parameters| {
            memory
                .read(
                    parameters.command_line_buffer_address,
                    usize::from(parameters.command_line_maximum_length_bytes),
                )
                .map_err(memory_error)
        })
        .transpose()?;
    let dynamic_export_bytes = dynamic_export_thunk
        .as_ref()
        .map(|thunk| memory.fetch(thunk.address, 16).map_err(memory_error))
        .transpose()?;
    let dynamic_export_page_prefix_bytes = (get_proc_address_occurrences >= 2)
        .then(|| {
            memory
                .fetch(
                    vine_core::DYNAMIC_EXPORT_ARENA_BASE,
                    get_proc_address_occurrences * vine_core::DYNAMIC_EXPORT_SLOT_SIZE as usize,
                )
                .map_err(memory_error)
        })
        .transpose()?;
    let process_parameters_installed = initial_process_environment
        .as_ref()
        .is_some_and(|environment| environment.process_parameters.is_some());
    let (reflection_stop_code, reflection_stop_message) = if first_thunk.is_none()
        && process_parameters_installed
    {
        (
            "entry-execution-pending",
            "the sealed import-free image, control page, initial stack, TEB, PEB, process parameters, and command-line pages are synchronized for adapter execution; guest entry execution has not yet been attempted",
        )
    } else if first_thunk.is_none() && initial_process_environment.is_some() {
        (
            "entry-execution-pending",
            "the sealed import-free image, control page, initial stack, TEB, and PEB are synchronized for adapter execution; guest entry execution has not yet been attempted",
        )
    } else if first_thunk.is_none() && initial_thread_stack.is_some() {
        (
            "entry-execution-pending",
            "the sealed import-free image, control page, and initial stack are synchronized for adapter execution; guest entry execution has not yet been attempted",
        )
    } else if first_thunk.is_none() {
        (
            "entry-execution-pending",
            "the sealed import-free image is synchronized for adapter execution; process control, initial stack setup, and guest entry execution have not yet been attempted",
        )
    } else if initial_thread_stack.is_some() {
        (
            "entry-to-first-owned-thunk-pending",
            "the production-owned image, thunk, control, and initial stack pages are synchronized for adapter execution; guest entry execution has not yet been attempted",
        )
    } else {
        (
            "entry-to-first-owned-thunk-pending",
            "the production-owned image and thunk pages are synchronized for adapter execution; initial stack setup and guest entry execution have not yet been attempted",
        )
    };
    let receipt = V86ReflectionReceipt {
        schema: "vine.v86-memory-reflection-receipt",
        schema_version: 1,
        acknowledged_generation: ack.generation,
        stats: memory.stats(),
        iat_value,
        entry_bytes,
        thunk_bytes,
        callback_return_bytes,
        process_entry_return_bytes,
        initial_stack_top_bytes,
        teb_bytes,
        peb_bytes,
        process_parameters_bytes,
        command_line_bytes,
        dynamic_export_bytes,
        dynamic_export_page_prefix_bytes,
        stop: UnsupportedPoint {
            stage: "adapter-execution",
            code: reflection_stop_code,
            message: reflection_stop_message,
        },
    };
    write_protocol_message(&receipt, reflection_message_limit)?;
    if let Some(generic_runtime) = generic_runtime {
        run_v86_generic_runtime_session(
            &mut protocol_input,
            &mut memory,
            generic_runtime,
            generic_runtime_capability,
            generic_runtime_trap_limit,
            kernel32_policy.ok_or_else(usage_error)?,
            callback_return_sentinel
                .as_ref()
                .ok_or_else(usage_error)?
                .address,
        )?;
    } else if supported_call_count >= 1 {
        let mut runtime = controlled_runtime_state(&dynamic_export_thunks)?;
        let return_sessions = V86ReturnSessionOptions {
            supported_call_count,
            dynamic_export_thunks: &dynamic_export_thunks,
        };
        run_v86_system_time_delta_session(
            &mut protocol_input,
            &mut memory,
            &mut runtime,
            &binding,
            reflection_message_limit,
            return_sessions,
        )?;
    }
    Ok(())
}

fn runtime_catalog_for_load_plan(
    loader: &LoaderIngressReport,
) -> Result<RuntimeExportCatalog, LoaderError> {
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
    RuntimeExportCatalog::new(exports)
}

fn prepare_generic_runtime_process(
    memory: &mut GuestMemory,
    loader: &LoaderIngressReport,
    materialization: &ImageMaterialization,
    runtime_catalog: &RuntimeExportCatalog,
    declared_catalog: &RuntimeExportCatalog,
    profiles: GenericRuntimeProfiles,
) -> Result<(GenericRuntimeProcess, ImportBinding), LoaderError> {
    let mut runtime = controlled_runtime_state_with_clock_profile(&[], profiles.clock_profile)?;
    if let Some(source_manifest) = profiles.source_manifest {
        runtime
            .register_source_snapshot(source_manifest)
            .map_err(runtime_state_error)?;
    }
    if let Some(writable_profile) = profiles.writable_profile {
        runtime
            .register_writable_profile(writable_profile)
            .map_err(runtime_state_error)?;
    }
    let mut module_handles = vec![CONTROLLED_KERNEL32_HANDLE];
    let mut canonical_modules = BTreeSet::from(["kernel32.dll".to_owned()]);
    let mut next_module_slot = 2_u16;
    for imported in &loader.load_plan.imports {
        let canonical_name = imported.name.to_ascii_lowercase();
        if !canonical_modules.insert(canonical_name.clone()) {
            continue;
        }
        let handle = runtime_module_handle(next_module_slot)?;
        next_module_slot = next_module_slot.checked_add(1).ok_or_else(|| {
            LoaderError::resource_limit(
                "runtime-reentry",
                "runtime-module-handle-space-exhausted",
                "controlled runtime module handle slots are exhausted",
            )
        })?;
        runtime
            .load_module(RuntimeModule {
                owner: "bgi-generic-runtime-reentry".to_owned(),
                variant_id: "runtime-system-import-v1".to_owned(),
                namespace: vine_core::LOGICAL_LOADED_MODULE_NAMESPACE.to_owned(),
                handle,
                requested_name: imported.name.clone(),
                canonical_name,
                runtime_system_module: true,
            })
            .map_err(runtime_state_error)?;
        module_handles.push(handle);
    }
    let main_module_handle = runtime_module_handle(next_module_slot)?;
    runtime
        .register_main_module(RuntimeModule {
            owner: "bgi-generic-runtime-reentry".to_owned(),
            variant_id: "mapped-main-executable-v1".to_owned(),
            namespace: vine_core::LOGICAL_LOADED_MODULE_NAMESPACE.to_owned(),
            handle: main_module_handle,
            requested_name: "BGI.exe".to_owned(),
            canonical_name: "bgi.exe".to_owned(),
            runtime_system_module: false,
        })
        .map_err(runtime_state_error)?;
    module_handles.push(main_module_handle);

    let binding = ExecutionSession::new(memory, &mut runtime)
        .bind_guest_module_imports(main_module_handle, loader, materialization, runtime_catalog)
        .map_err(runtime_state_error)?;
    ExecutionSession::new(memory, &mut runtime)
        .register_module_image(main_module_handle, loader, materialization)
        .map_err(runtime_module_image_error)?;
    let evidence_binding = evidence_import_binding(
        &runtime,
        loader,
        materialization,
        &binding,
        declared_catalog,
    )?;
    Ok((
        GenericRuntimeProcess {
            runtime,
            binding,
            module_handles,
            main_module_handle,
            attach_order: Vec::new(),
        },
        evidence_binding,
    ))
}

fn runtime_module_handle(slot: u16) -> Result<u32, LoaderError> {
    if slot == 0 {
        return Err(LoaderError::runtime_defect(
            "runtime-reentry",
            "runtime-module-handle-slot-zero",
            "controlled runtime module handle slot cannot be zero",
        ));
    }
    Ok(0x4d01_0000 | u32::from(slot))
}

fn evidence_import_binding(
    runtime: &RuntimeState,
    loader: &LoaderIngressReport,
    materialization: &ImageMaterialization,
    binding: &RuntimeGuestImportBinding,
    declared_catalog: &RuntimeExportCatalog,
) -> Result<ImportBinding, LoaderError> {
    let mut thunks = Vec::with_capacity(binding.imports.len());
    let mut import_index = 0usize;
    let mut unsupported_trap_imports = 0usize;
    for module in &loader.load_plan.imports {
        for symbol in &module.symbols {
            let import = binding.imports.get(import_index).ok_or_else(|| {
                LoaderError::runtime_defect(
                    "runtime-reentry",
                    "runtime-import-binding-short",
                    "runtime import binding has fewer entries than the immutable PE plan",
                )
            })?;
            let RuntimeGuestImportResolution::RuntimeExport { endpoint } = &import.resolution
            else {
                return Err(LoaderError::runtime_defect(
                    "runtime-reentry",
                    "runtime-import-resolution-not-system",
                    "BGI generic re-entry expected every initial dependency to be runtime-owned",
                ));
            };
            if import.iat_address
                != GuestAddress(
                    materialization
                        .load_base
                        .0
                        .checked_add(symbol.iat_rva.0)
                        .ok_or_else(|| {
                            LoaderError::runtime_defect(
                                "runtime-reentry",
                                "runtime-import-iat-overflow",
                                "runtime import IAT address overflowed x86 accounting",
                            )
                        })?,
                )
                || import.target != symbol.target
            {
                return Err(LoaderError::runtime_defect(
                    "runtime-reentry",
                    "runtime-import-binding-order-mismatch",
                    "runtime import binding order differs from the immutable PE plan",
                ));
            }
            let disposition = if declared_catalog.supports(&module.name, &symbol.target) {
                ImportThunkDisposition::DeclaredRuntimeExport
            } else {
                unsupported_trap_imports += 1;
                ImportThunkDisposition::UnsupportedTrap
            };
            thunks.push(ImportThunk {
                id: endpoint.id,
                address: endpoint.address,
                requester_owner: binding.image_owner,
                module: module.name.clone(),
                iat_rva: symbol.iat_rva,
                iat_address: import.iat_address,
                original_iat_value: import.original_iat_value,
                disposition,
                target: symbol.target.clone(),
            });
            import_index += 1;
        }
    }
    if import_index != binding.imports.len() {
        return Err(LoaderError::runtime_defect(
            "runtime-reentry",
            "runtime-import-binding-long",
            "runtime import binding has more entries than the immutable PE plan",
        ));
    }
    let arena = runtime
        .runtime_system_exports()
        .map(|table| table.arena.clone())
        .ok_or_else(|| {
            LoaderError::runtime_defect(
                "runtime-reentry",
                "runtime-export-table-missing",
                "runtime import binding completed without its process export table",
            )
        })?;
    Ok(ImportBinding {
        source: binding.source.clone(),
        image_owner: binding.image_owner,
        image_load_base: binding.image_load_base,
        total_imports: thunks.len(),
        bound_imports: thunks.len(),
        unsupported_trap_imports,
        remaining_imports: 0,
        complete: true,
        image_owner_sealed: binding.image_owner_sealed,
        arena: Some(arena),
        thunks,
        unresolved_imports: Vec::new(),
        first_unresolved_import: None,
        invalidation: binding.invalidation,
        stop: UnsupportedPoint {
            stage: "adapter-execution",
            code: "entry-execution-pending",
            message: "the process-owned image and runtime export table are synchronized for adapter execution",
        },
    })
}

fn finalize_generic_runtime_process(
    process: &mut GenericRuntimeProcess,
    dynamic_export_thunks: &[DynamicExportThunk],
    callback_return_sentinel: GuestAddress,
) -> Result<(), LoaderError> {
    for thunk in dynamic_export_thunks {
        process
            .runtime
            .register_dynamic_export_thunk(thunk)
            .map_err(runtime_state_error)?;
    }
    for handle in &process.module_handles {
        process
            .runtime
            .mark_module_ready(*handle)
            .map_err(runtime_state_error)?;
    }
    let completion = match process
        .runtime
        .begin_dll_process_attach(callback_return_sentinel)
        .map_err(runtime_state_error)?
    {
        RuntimeDllLifecycleAction::Complete { completion } => completion,
        RuntimeDllLifecycleAction::Callback { .. } => {
            return Err(LoaderError::runtime_defect(
                "runtime-reentry",
                "unexpected-bgi-dll-process-callback",
                "BGI initial system-module graph unexpectedly requested a guest DllMain callback",
            ));
        }
        RuntimeDllLifecycleAction::AttachFailed { .. } => {
            return Err(LoaderError::runtime_defect(
                "runtime-reentry",
                "bgi-dll-process-attach-failed",
                "BGI initial system-module graph failed process attach without a guest callback",
            ));
        }
    };
    if completion.phase != RuntimeDllLifecyclePhase::ProcessAttached
        || !completion.callback_order.is_empty()
    {
        return Err(LoaderError::runtime_defect(
            "runtime-reentry",
            "bgi-dll-process-attach-completion-invalid",
            "BGI initial system-module graph did not complete a callback-free process attach",
        ));
    }
    process.attach_order = completion.graph_order;
    Ok(())
}

fn generic_runtime_contract_for_endpoint(
    endpoint: ApiEndpointIdentity,
) -> Result<Option<ApiContract>, LoaderError> {
    if let Some(contract) = kernel32_contract_for_endpoint(endpoint.clone()).map_err(api_fault)? {
        return Ok(Some(contract));
    }
    if let Some(contract) = user32_contract_for_endpoint(endpoint.clone()).map_err(api_fault)? {
        return Ok(Some(contract));
    }
    if let Some(contract) = gdi32_contract_for_endpoint(endpoint.clone()).map_err(api_fault)? {
        return Ok(Some(contract));
    }
    if let Some(contract) = gdiplus_contract_for_endpoint(endpoint.clone()).map_err(api_fault)? {
        return Ok(Some(contract));
    }
    if let Some(contract) = imm32_contract_for_endpoint(endpoint.clone()).map_err(api_fault)? {
        return Ok(Some(contract));
    }
    if let Some(contract) = ole32_contract_for_endpoint(endpoint.clone()).map_err(api_fault)? {
        return Ok(Some(contract));
    }
    if let Some(contract) = shell32_contract_for_endpoint(endpoint.clone()).map_err(api_fault)? {
        return Ok(Some(contract));
    }
    if let Some(contract) = d3d9_contract_for_endpoint(endpoint.clone()).map_err(api_fault)? {
        return Ok(Some(contract));
    }
    if let Some(contract) = dinput8_contract_for_endpoint(endpoint.clone()).map_err(api_fault)? {
        return Ok(Some(contract));
    }
    if let Some(contract) = dsound_contract_for_endpoint(endpoint.clone()).map_err(api_fault)? {
        return Ok(Some(contract));
    }
    winmm_contract_for_endpoint(endpoint).map_err(api_fault)
}

fn run_v86_generic_runtime_session(
    input: &mut impl BufRead,
    memory: &mut GuestMemory,
    mut process: GenericRuntimeProcess,
    enabled_capability: Option<GenericRuntimeCapability>,
    trap_limit: usize,
    kernel32_policy: Kernel32CompatibilityPolicy,
    callback_return_sentinel: GuestAddress,
) -> Result<(), LoaderError> {
    let window_timer_delivery_enabled = matches!(
        enabled_capability,
        Some(
            GenericRuntimeCapability::User32WindowTimerCallbackDelivery
                | GenericRuntimeCapability::User32PointerButtonMessages
                | GenericRuntimeCapability::User32SetFocusSameWindow
                | GenericRuntimeCapability::User32DefWindowProcPointerDown
                | GenericRuntimeCapability::User32DefWindowProcPointerRelease
        )
    );
    let pointer_button_messages_enabled = matches!(
        enabled_capability,
        Some(
            GenericRuntimeCapability::User32PointerButtonMessages
                | GenericRuntimeCapability::User32SetFocusSameWindow
                | GenericRuntimeCapability::User32DefWindowProcPointerDown
                | GenericRuntimeCapability::User32DefWindowProcPointerRelease
        )
    );
    let set_focus_same_window_enabled = matches!(
        enabled_capability,
        Some(
            GenericRuntimeCapability::User32SetFocusSameWindow
                | GenericRuntimeCapability::User32DefWindowProcPointerDown
                | GenericRuntimeCapability::User32DefWindowProcPointerRelease
        )
    );
    let def_window_proc_pointer_down_enabled = matches!(
        enabled_capability,
        Some(
            GenericRuntimeCapability::User32DefWindowProcPointerDown
                | GenericRuntimeCapability::User32DefWindowProcPointerRelease
        )
    );
    let def_window_proc_pointer_release_enabled = matches!(
        enabled_capability,
        Some(GenericRuntimeCapability::User32DefWindowProcPointerRelease)
    );
    let d3d9_present_enabled = matches!(
        enabled_capability,
        Some(
            GenericRuntimeCapability::D3d9Present
                | GenericRuntimeCapability::D3d9PostPresentContinuation
                | GenericRuntimeCapability::User32AsyncKeyStateInput
                | GenericRuntimeCapability::User32AsyncKeyStateInitialScan
                | GenericRuntimeCapability::User32WindowTimerCallbackDelivery
                | GenericRuntimeCapability::User32PointerButtonMessages
                | GenericRuntimeCapability::User32SetFocusSameWindow
                | GenericRuntimeCapability::User32DefWindowProcPointerDown
                | GenericRuntimeCapability::User32DefWindowProcPointerRelease
        )
    );
    let get_async_key_state_enabled = matches!(
        enabled_capability,
        Some(
            GenericRuntimeCapability::User32AsyncKeyStateInput
                | GenericRuntimeCapability::User32AsyncKeyStateInitialScan
                | GenericRuntimeCapability::User32WindowTimerCallbackDelivery
                | GenericRuntimeCapability::User32PointerButtonMessages
                | GenericRuntimeCapability::User32SetFocusSameWindow
                | GenericRuntimeCapability::User32DefWindowProcPointerDown
                | GenericRuntimeCapability::User32DefWindowProcPointerRelease
        )
    );
    let get_async_key_state_input_enabled = matches!(
        enabled_capability,
        Some(GenericRuntimeCapability::User32AsyncKeyStateInput)
    );
    let get_window_rect_enabled = matches!(
        enabled_capability,
        Some(
            GenericRuntimeCapability::User32AsyncKeyStateInitialScan
                | GenericRuntimeCapability::User32WindowTimerCallbackDelivery
                | GenericRuntimeCapability::User32PointerButtonMessages
                | GenericRuntimeCapability::User32SetFocusSameWindow
                | GenericRuntimeCapability::User32DefWindowProcPointerDown
                | GenericRuntimeCapability::User32DefWindowProcPointerRelease
        )
    );
    let d3d9_post_end_scene_raster_status_enabled = matches!(
        enabled_capability,
        Some(
            GenericRuntimeCapability::D3d9PostEndSceneRasterStatus
                | GenericRuntimeCapability::D3d9PostRasterStatusWaitableTimer
                | GenericRuntimeCapability::D3d9PostScenePresentGateway
                | GenericRuntimeCapability::D3d9Present
                | GenericRuntimeCapability::D3d9PostPresentContinuation
                | GenericRuntimeCapability::User32AsyncKeyStateInput
                | GenericRuntimeCapability::User32AsyncKeyStateInitialScan
                | GenericRuntimeCapability::User32WindowTimerCallbackDelivery
                | GenericRuntimeCapability::User32PointerButtonMessages
                | GenericRuntimeCapability::User32SetFocusSameWindow
                | GenericRuntimeCapability::User32DefWindowProcPointerDown
                | GenericRuntimeCapability::User32DefWindowProcPointerRelease
        )
    );
    let d3d9_end_scene_enabled = d3d9_post_end_scene_raster_status_enabled
        || matches!(
            enabled_capability,
            Some(GenericRuntimeCapability::D3d9EndScene)
        );
    let d3d9_draw_primitive_enabled = d3d9_end_scene_enabled
        || matches!(
            enabled_capability,
            Some(GenericRuntimeCapability::D3d9DrawPrimitive)
        );
    let d3d9_set_stream_source_enabled = d3d9_draw_primitive_enabled
        || matches!(
            enabled_capability,
            Some(GenericRuntimeCapability::D3d9SetStreamSource)
        );
    let d3d9_set_fvf_enabled = d3d9_set_stream_source_enabled
        || matches!(
            enabled_capability,
            Some(GenericRuntimeCapability::D3d9SetFvf)
        );
    let d3d9_set_texture_enabled = d3d9_set_fvf_enabled
        || matches!(
            enabled_capability,
            Some(GenericRuntimeCapability::D3d9SetTexture)
        );
    let d3d9_initial_texture_stage_states_enabled = d3d9_set_texture_enabled
        || matches!(
            enabled_capability,
            Some(GenericRuntimeCapability::D3d9InitialTextureStageStates)
        );
    let d3d9_initial_sampler_states_enabled = d3d9_initial_texture_stage_states_enabled
        || matches!(
            enabled_capability,
            Some(GenericRuntimeCapability::D3d9InitialSamplerStates)
        );
    let d3d9_begin_scene_enabled = d3d9_initial_sampler_states_enabled
        || matches!(
            enabled_capability,
            Some(GenericRuntimeCapability::D3d9BeginScene)
        );
    let d3d9_update_texture_enabled = d3d9_begin_scene_enabled
        || matches!(
            enabled_capability,
            Some(GenericRuntimeCapability::D3d9UpdateTexture)
        );
    let d3d9_add_dirty_rect_enabled = d3d9_update_texture_enabled
        || matches!(
            enabled_capability,
            Some(GenericRuntimeCapability::D3d9AddDirtyRect)
        );
    let set_rect_enabled = d3d9_add_dirty_rect_enabled
        || matches!(enabled_capability, Some(GenericRuntimeCapability::SetRect));
    let d3d9_clear_enabled = set_rect_enabled
        || matches!(
            enabled_capability,
            Some(GenericRuntimeCapability::D3d9Clear)
        );
    let d3d9_recreated_textures_enabled = d3d9_clear_enabled
        || matches!(
            enabled_capability,
            Some(GenericRuntimeCapability::D3d9RecreatedTextures)
        );
    let create_directory_a_enabled = d3d9_recreated_textures_enabled
        || matches!(
            enabled_capability,
            Some(GenericRuntimeCapability::CreateDirectoryA)
        );
    let get_file_attributes_a_enabled = create_directory_a_enabled
        || matches!(
            enabled_capability,
            Some(GenericRuntimeCapability::GetFileAttributesA)
        );
    let global_memory_status_enabled = get_file_attributes_a_enabled
        || matches!(
            enabled_capability,
            Some(GenericRuntimeCapability::GlobalMemoryStatus)
        );
    let sleep_zero_enabled = global_memory_status_enabled
        || matches!(
            enabled_capability,
            Some(GenericRuntimeCapability::SleepZero)
        );
    let drag_accept_files_enabled = sleep_zero_enabled
        || matches!(
            enabled_capability,
            Some(GenericRuntimeCapability::DragAcceptFiles)
        );
    let wait_for_single_object_ex_enabled = drag_accept_files_enabled
        || matches!(
            enabled_capability,
            Some(GenericRuntimeCapability::WaitForSingleObjectEx)
        );
    let set_waitable_timer_enabled = wait_for_single_object_ex_enabled
        || matches!(
            enabled_capability,
            Some(GenericRuntimeCapability::SetWaitableTimer)
        );
    let enabled_capability = if set_waitable_timer_enabled {
        Some(GenericRuntimeCapability::Imm32Context)
    } else {
        enabled_capability
    };
    let imm32_context_enabled = matches!(
        enabled_capability,
        Some(GenericRuntimeCapability::Imm32Context)
    );
    let virtual_alloc_enabled = matches!(
        enabled_capability,
        Some(GenericRuntimeCapability::VirtualAlloc | GenericRuntimeCapability::Imm32Context)
    );
    let dinput8_factory_enabled = matches!(
        enabled_capability,
        Some(
            GenericRuntimeCapability::Dinput8Factory
                | GenericRuntimeCapability::Dinput8EnumDevices
                | GenericRuntimeCapability::DsoundRootAndCooperativeLevel
                | GenericRuntimeCapability::DsoundInitialBuffers
                | GenericRuntimeCapability::DsoundBufferLockUnlockPlay
                | GenericRuntimeCapability::SetTimer
                | GenericRuntimeCapability::CreateThread
                | GenericRuntimeCapability::SetThreadPriority
                | GenericRuntimeCapability::ScreenDc
                | GenericRuntimeCapability::FontEnumeration
                | GenericRuntimeCapability::FontCreation
                | GenericRuntimeCapability::CompatibleDc
                | GenericRuntimeCapability::DibLifecycle
                | GenericRuntimeCapability::OutlineTextMetrics
                | GenericRuntimeCapability::TextRaster
                | GenericRuntimeCapability::GdiplusStartup
                | GenericRuntimeCapability::VirtualAlloc
                | GenericRuntimeCapability::Imm32Context
        )
    );
    let dinput8_enum_devices_enabled = matches!(
        enabled_capability,
        Some(
            GenericRuntimeCapability::Dinput8EnumDevices
                | GenericRuntimeCapability::DsoundRootAndCooperativeLevel
                | GenericRuntimeCapability::DsoundInitialBuffers
                | GenericRuntimeCapability::DsoundBufferLockUnlockPlay
                | GenericRuntimeCapability::SetTimer
                | GenericRuntimeCapability::CreateThread
                | GenericRuntimeCapability::SetThreadPriority
                | GenericRuntimeCapability::ScreenDc
                | GenericRuntimeCapability::FontEnumeration
                | GenericRuntimeCapability::FontCreation
                | GenericRuntimeCapability::CompatibleDc
                | GenericRuntimeCapability::DibLifecycle
                | GenericRuntimeCapability::OutlineTextMetrics
                | GenericRuntimeCapability::TextRaster
                | GenericRuntimeCapability::GdiplusStartup
                | GenericRuntimeCapability::VirtualAlloc
                | GenericRuntimeCapability::Imm32Context
        )
    );
    let dsound_root_enabled = matches!(
        enabled_capability,
        Some(
            GenericRuntimeCapability::DsoundRootAndCooperativeLevel
                | GenericRuntimeCapability::DsoundInitialBuffers
                | GenericRuntimeCapability::DsoundBufferLockUnlockPlay
                | GenericRuntimeCapability::SetTimer
                | GenericRuntimeCapability::CreateThread
                | GenericRuntimeCapability::SetThreadPriority
                | GenericRuntimeCapability::ScreenDc
                | GenericRuntimeCapability::FontEnumeration
                | GenericRuntimeCapability::FontCreation
                | GenericRuntimeCapability::CompatibleDc
                | GenericRuntimeCapability::DibLifecycle
                | GenericRuntimeCapability::OutlineTextMetrics
                | GenericRuntimeCapability::TextRaster
                | GenericRuntimeCapability::GdiplusStartup
                | GenericRuntimeCapability::VirtualAlloc
                | GenericRuntimeCapability::Imm32Context
        )
    );
    let dsound_initial_buffers_enabled = matches!(
        enabled_capability,
        Some(
            GenericRuntimeCapability::DsoundInitialBuffers
                | GenericRuntimeCapability::DsoundBufferLockUnlockPlay
                | GenericRuntimeCapability::SetTimer
                | GenericRuntimeCapability::CreateThread
                | GenericRuntimeCapability::SetThreadPriority
                | GenericRuntimeCapability::ScreenDc
                | GenericRuntimeCapability::FontEnumeration
                | GenericRuntimeCapability::FontCreation
                | GenericRuntimeCapability::CompatibleDc
                | GenericRuntimeCapability::DibLifecycle
                | GenericRuntimeCapability::OutlineTextMetrics
                | GenericRuntimeCapability::TextRaster
                | GenericRuntimeCapability::GdiplusStartup
                | GenericRuntimeCapability::VirtualAlloc
                | GenericRuntimeCapability::Imm32Context
        )
    );
    let dsound_buffer_transfer_enabled = matches!(
        enabled_capability,
        Some(
            GenericRuntimeCapability::DsoundBufferLockUnlockPlay
                | GenericRuntimeCapability::SetTimer
                | GenericRuntimeCapability::CreateThread
                | GenericRuntimeCapability::SetThreadPriority
                | GenericRuntimeCapability::ScreenDc
                | GenericRuntimeCapability::FontEnumeration
                | GenericRuntimeCapability::FontCreation
                | GenericRuntimeCapability::CompatibleDc
                | GenericRuntimeCapability::DibLifecycle
                | GenericRuntimeCapability::OutlineTextMetrics
                | GenericRuntimeCapability::TextRaster
                | GenericRuntimeCapability::GdiplusStartup
                | GenericRuntimeCapability::VirtualAlloc
                | GenericRuntimeCapability::Imm32Context
        )
    );
    let requested_capability = match enabled_capability {
        Some(
            GenericRuntimeCapability::Dinput8Factory
            | GenericRuntimeCapability::Dinput8EnumDevices
            | GenericRuntimeCapability::DsoundRootAndCooperativeLevel
            | GenericRuntimeCapability::DsoundInitialBuffers
            | GenericRuntimeCapability::DsoundBufferLockUnlockPlay
            | GenericRuntimeCapability::SetTimer
            | GenericRuntimeCapability::CreateThread
            | GenericRuntimeCapability::SetThreadPriority
            | GenericRuntimeCapability::ScreenDc
            | GenericRuntimeCapability::FontEnumeration
            | GenericRuntimeCapability::FontCreation
            | GenericRuntimeCapability::CompatibleDc
            | GenericRuntimeCapability::DibLifecycle
            | GenericRuntimeCapability::OutlineTextMetrics
            | GenericRuntimeCapability::TextRaster
            | GenericRuntimeCapability::GdiplusStartup
            | GenericRuntimeCapability::VirtualAlloc
            | GenericRuntimeCapability::Imm32Context
            | GenericRuntimeCapability::D3d9AddDirtyRect
            | GenericRuntimeCapability::D3d9UpdateTexture
            | GenericRuntimeCapability::D3d9BeginScene
            | GenericRuntimeCapability::D3d9InitialSamplerStates
            | GenericRuntimeCapability::D3d9InitialTextureStageStates
            | GenericRuntimeCapability::D3d9SetTexture
            | GenericRuntimeCapability::D3d9SetFvf
            | GenericRuntimeCapability::D3d9SetStreamSource
            | GenericRuntimeCapability::D3d9DrawPrimitive
            | GenericRuntimeCapability::D3d9EndScene
            | GenericRuntimeCapability::D3d9PostEndSceneRasterStatus
            | GenericRuntimeCapability::D3d9PostRasterStatusWaitableTimer
            | GenericRuntimeCapability::D3d9PostScenePresentGateway
            | GenericRuntimeCapability::D3d9Present
            | GenericRuntimeCapability::D3d9PostPresentContinuation
            | GenericRuntimeCapability::User32AsyncKeyStateInput
            | GenericRuntimeCapability::User32AsyncKeyStateInitialScan
            | GenericRuntimeCapability::User32WindowTimerCallbackDelivery
            | GenericRuntimeCapability::User32PointerButtonMessages
            | GenericRuntimeCapability::User32SetFocusSameWindow
            | GenericRuntimeCapability::User32DefWindowProcPointerDown
            | GenericRuntimeCapability::User32DefWindowProcPointerRelease,
        ) => Some(GenericRuntimeCapability::GetMonitorInfoA),
        capability => capability,
    };
    let user32_capability = if matches!(
        requested_capability,
        Some(
            GenericRuntimeCapability::D3d9RootFactoryAndIdentity
                | GenericRuntimeCapability::D3d9GetAdapterDisplayMode
                | GenericRuntimeCapability::D3d9GetAdapterIdentifier
                | GenericRuntimeCapability::D3d9GetDeviceCaps
                | GenericRuntimeCapability::D3d9CheckDeviceType
                | GenericRuntimeCapability::D3d9CheckDeviceFormat
                | GenericRuntimeCapability::LoadLibraryA
                | GenericRuntimeCapability::GetClientRect
                | GenericRuntimeCapability::WindowDcFill
                | GenericRuntimeCapability::SetWindowLongA
                | GenericRuntimeCapability::SetWindowPosActivation
                | GenericRuntimeCapability::MonitorFromWindow
                | GenericRuntimeCapability::D3d9GetAdapterCount
                | GenericRuntimeCapability::D3d9GetAdapterMonitor
                | GenericRuntimeCapability::D3d9CreateDevice
                | GenericRuntimeCapability::D3d9CreateTexture
                | GenericRuntimeCapability::D3d9GetSurfaceLevel
                | GenericRuntimeCapability::D3d9SurfaceGetDc
                | GenericRuntimeCapability::TimeGetTime
                | GenericRuntimeCapability::D3d9InitialTextures
                | GenericRuntimeCapability::D3d9InitialVertexBuffer
                | GenericRuntimeCapability::D3d9InitialRenderStates
                | GenericRuntimeCapability::D3d9RasterStatus
                | GenericRuntimeCapability::QueryPerformanceFrequency
                | GenericRuntimeCapability::GetMonitorInfoA
        )
    ) {
        Some(GenericRuntimeCapability::SetWindowPosInitialPlacement)
    } else {
        requested_capability
    };
    let endpoint_capability = match requested_capability {
        Some(
            GenericRuntimeCapability::LoadIconA
            | GenericRuntimeCapability::LoadCursorA
            | GenericRuntimeCapability::RegisterClassExA
            | GenericRuntimeCapability::GetSystemMetrics
            | GenericRuntimeCapability::LargeHeapAllocation
            | GenericRuntimeCapability::SourceFileRead
            | GenericRuntimeCapability::CreateWindowExA
            | GenericRuntimeCapability::GetSystemMetricsSmallIconHeight
            | GenericRuntimeCapability::GetSystemMetricsSmallIconWidth
            | GenericRuntimeCapability::LoadImageASmallIcon
            | GenericRuntimeCapability::SendMessageASmallIcon
            | GenericRuntimeCapability::SetWindowPosInitialPlacement
            | GenericRuntimeCapability::D3d9RootFactoryAndIdentity
            | GenericRuntimeCapability::D3d9GetAdapterDisplayMode
            | GenericRuntimeCapability::D3d9GetAdapterIdentifier
            | GenericRuntimeCapability::D3d9GetDeviceCaps
            | GenericRuntimeCapability::D3d9CheckDeviceType
            | GenericRuntimeCapability::D3d9CheckDeviceFormat
            | GenericRuntimeCapability::LoadLibraryA
            | GenericRuntimeCapability::GetClientRect
            | GenericRuntimeCapability::WindowDcFill
            | GenericRuntimeCapability::SetWindowLongA
            | GenericRuntimeCapability::SetWindowPosActivation
            | GenericRuntimeCapability::MonitorFromWindow
            | GenericRuntimeCapability::D3d9GetAdapterCount
            | GenericRuntimeCapability::D3d9GetAdapterMonitor
            | GenericRuntimeCapability::D3d9CreateDevice
            | GenericRuntimeCapability::D3d9CreateTexture
            | GenericRuntimeCapability::D3d9GetSurfaceLevel
            | GenericRuntimeCapability::D3d9SurfaceGetDc
            | GenericRuntimeCapability::TimeGetTime
            | GenericRuntimeCapability::D3d9InitialTextures
            | GenericRuntimeCapability::D3d9InitialVertexBuffer
            | GenericRuntimeCapability::D3d9InitialRenderStates
            | GenericRuntimeCapability::D3d9RasterStatus
            | GenericRuntimeCapability::QueryPerformanceFrequency
            | GenericRuntimeCapability::GetMonitorInfoA,
        ) => Some(GenericRuntimeCapability::CreateWaitableTimerA),
        capability => capability,
    };
    let font_enumeration_enabled = matches!(
        enabled_capability,
        Some(
            GenericRuntimeCapability::FontEnumeration
                | GenericRuntimeCapability::FontCreation
                | GenericRuntimeCapability::CompatibleDc
                | GenericRuntimeCapability::DibLifecycle
                | GenericRuntimeCapability::OutlineTextMetrics
                | GenericRuntimeCapability::TextRaster
                | GenericRuntimeCapability::GdiplusStartup
                | GenericRuntimeCapability::VirtualAlloc
                | GenericRuntimeCapability::Imm32Context
        )
    );
    let font_creation_enabled = matches!(
        enabled_capability,
        Some(
            GenericRuntimeCapability::FontCreation
                | GenericRuntimeCapability::CompatibleDc
                | GenericRuntimeCapability::DibLifecycle
                | GenericRuntimeCapability::OutlineTextMetrics
                | GenericRuntimeCapability::TextRaster
                | GenericRuntimeCapability::GdiplusStartup
                | GenericRuntimeCapability::VirtualAlloc
                | GenericRuntimeCapability::Imm32Context
        )
    );
    let compatible_dc_enabled = matches!(
        enabled_capability,
        Some(
            GenericRuntimeCapability::CompatibleDc
                | GenericRuntimeCapability::DibLifecycle
                | GenericRuntimeCapability::OutlineTextMetrics
                | GenericRuntimeCapability::TextRaster
                | GenericRuntimeCapability::GdiplusStartup
                | GenericRuntimeCapability::VirtualAlloc
                | GenericRuntimeCapability::Imm32Context
        )
    );
    let dib_lifecycle_enabled = matches!(
        enabled_capability,
        Some(
            GenericRuntimeCapability::DibLifecycle
                | GenericRuntimeCapability::OutlineTextMetrics
                | GenericRuntimeCapability::TextRaster
                | GenericRuntimeCapability::GdiplusStartup
                | GenericRuntimeCapability::VirtualAlloc
                | GenericRuntimeCapability::Imm32Context
        )
    );
    let outline_text_metrics_enabled = matches!(
        enabled_capability,
        Some(
            GenericRuntimeCapability::OutlineTextMetrics
                | GenericRuntimeCapability::TextRaster
                | GenericRuntimeCapability::GdiplusStartup
                | GenericRuntimeCapability::VirtualAlloc
                | GenericRuntimeCapability::Imm32Context
        )
    );
    let text_raster_enabled = matches!(
        enabled_capability,
        Some(
            GenericRuntimeCapability::TextRaster
                | GenericRuntimeCapability::GdiplusStartup
                | GenericRuntimeCapability::VirtualAlloc
                | GenericRuntimeCapability::Imm32Context
        )
    );
    let gdiplus_startup_enabled = matches!(
        enabled_capability,
        Some(
            GenericRuntimeCapability::GdiplusStartup
                | GenericRuntimeCapability::VirtualAlloc
                | GenericRuntimeCapability::Imm32Context
        )
    );
    let imm32_policy = Imm32CompatibilityPolicy::controlled_v1();
    imm32_policy.validate().map_err(api_fault)?;
    let mut user32_policy = if def_window_proc_pointer_release_enabled {
        User32CompatibilityPolicy::controlled_v35()
    } else if def_window_proc_pointer_down_enabled {
        User32CompatibilityPolicy::controlled_v34()
    } else if set_focus_same_window_enabled {
        User32CompatibilityPolicy::controlled_v33()
    } else if pointer_button_messages_enabled {
        User32CompatibilityPolicy::controlled_v32()
    } else if window_timer_delivery_enabled {
        User32CompatibilityPolicy::controlled_v31()
    } else {
        User32CompatibilityPolicy::controlled_v30()
    };
    if !matches!(
        user32_capability,
        Some(
            GenericRuntimeCapability::GetSystemMetricsSmallIconHeight
                | GenericRuntimeCapability::GetSystemMetricsSmallIconWidth
                | GenericRuntimeCapability::LoadImageASmallIcon
                | GenericRuntimeCapability::SendMessageASmallIcon
                | GenericRuntimeCapability::SetWindowPosInitialPlacement
        )
    ) {
        user32_policy
            .get_system_metrics
            .as_mut()
            .expect("controlled User32 policy has system metrics")
            .small_icon_height = None;
    }
    if !matches!(
        user32_capability,
        Some(
            GenericRuntimeCapability::GetSystemMetricsSmallIconWidth
                | GenericRuntimeCapability::LoadImageASmallIcon
                | GenericRuntimeCapability::SendMessageASmallIcon
                | GenericRuntimeCapability::SetWindowPosInitialPlacement
        )
    ) {
        user32_policy
            .get_system_metrics
            .as_mut()
            .expect("controlled User32 policy has system metrics")
            .small_icon_width = None;
    }
    if !matches!(
        user32_capability,
        Some(
            GenericRuntimeCapability::LoadImageASmallIcon
                | GenericRuntimeCapability::SendMessageASmallIcon
                | GenericRuntimeCapability::SetWindowPosInitialPlacement
        )
    ) {
        user32_policy.load_image_a = None;
    }
    if !matches!(
        user32_capability,
        Some(
            GenericRuntimeCapability::SendMessageASmallIcon
                | GenericRuntimeCapability::SetWindowPosInitialPlacement
        )
    ) {
        user32_policy.send_message_a = None;
    }
    if !imm32_context_enabled {
        user32_policy.send_message_a_ime_control = None;
    }
    if !matches!(
        user32_capability,
        Some(GenericRuntimeCapability::SetWindowPosInitialPlacement)
    ) {
        user32_policy.set_window_pos = None;
    }
    if !matches!(
        requested_capability,
        Some(
            GenericRuntimeCapability::SetWindowLongA
                | GenericRuntimeCapability::SetWindowPosActivation
                | GenericRuntimeCapability::MonitorFromWindow
                | GenericRuntimeCapability::D3d9GetAdapterCount
                | GenericRuntimeCapability::D3d9GetAdapterMonitor
                | GenericRuntimeCapability::D3d9CreateDevice
                | GenericRuntimeCapability::D3d9CreateTexture
                | GenericRuntimeCapability::D3d9GetSurfaceLevel
                | GenericRuntimeCapability::D3d9SurfaceGetDc
                | GenericRuntimeCapability::TimeGetTime
                | GenericRuntimeCapability::D3d9InitialTextures
                | GenericRuntimeCapability::D3d9InitialVertexBuffer
                | GenericRuntimeCapability::D3d9InitialRenderStates
                | GenericRuntimeCapability::D3d9RasterStatus
                | GenericRuntimeCapability::QueryPerformanceFrequency
                | GenericRuntimeCapability::GetMonitorInfoA
        )
    ) {
        user32_policy.set_window_long_a = None;
    }
    if !matches!(
        requested_capability,
        Some(
            GenericRuntimeCapability::MonitorFromWindow
                | GenericRuntimeCapability::D3d9GetAdapterCount
                | GenericRuntimeCapability::D3d9GetAdapterMonitor
                | GenericRuntimeCapability::D3d9CreateDevice
                | GenericRuntimeCapability::D3d9CreateTexture
                | GenericRuntimeCapability::D3d9GetSurfaceLevel
                | GenericRuntimeCapability::D3d9SurfaceGetDc
                | GenericRuntimeCapability::TimeGetTime
                | GenericRuntimeCapability::D3d9InitialTextures
                | GenericRuntimeCapability::D3d9InitialVertexBuffer
                | GenericRuntimeCapability::D3d9InitialRenderStates
                | GenericRuntimeCapability::D3d9RasterStatus
                | GenericRuntimeCapability::QueryPerformanceFrequency
                | GenericRuntimeCapability::GetMonitorInfoA
        )
    ) {
        user32_policy.monitor_from_window = None;
    }
    if !matches!(
        requested_capability,
        Some(GenericRuntimeCapability::GetMonitorInfoA)
    ) {
        user32_policy.get_monitor_info_a = None;
    }
    if !matches!(
        enabled_capability,
        Some(
            GenericRuntimeCapability::SetTimer
                | GenericRuntimeCapability::CreateThread
                | GenericRuntimeCapability::SetThreadPriority
                | GenericRuntimeCapability::ScreenDc
                | GenericRuntimeCapability::FontEnumeration
                | GenericRuntimeCapability::FontCreation
                | GenericRuntimeCapability::CompatibleDc
                | GenericRuntimeCapability::DibLifecycle
                | GenericRuntimeCapability::OutlineTextMetrics
                | GenericRuntimeCapability::TextRaster
                | GenericRuntimeCapability::GdiplusStartup
                | GenericRuntimeCapability::VirtualAlloc
                | GenericRuntimeCapability::Imm32Context
        )
    ) {
        user32_policy.set_timer = None;
    }
    if !matches!(
        enabled_capability,
        Some(
            GenericRuntimeCapability::ScreenDc
                | GenericRuntimeCapability::FontEnumeration
                | GenericRuntimeCapability::FontCreation
                | GenericRuntimeCapability::CompatibleDc
                | GenericRuntimeCapability::DibLifecycle
                | GenericRuntimeCapability::OutlineTextMetrics
                | GenericRuntimeCapability::TextRaster
                | GenericRuntimeCapability::GdiplusStartup
                | GenericRuntimeCapability::VirtualAlloc
                | GenericRuntimeCapability::Imm32Context
        )
    ) {
        user32_policy.screen_dc = None;
    }
    user32_policy.validate().map_err(api_fault)?;
    let mut gdi32_policy = Gdi32CompatibilityPolicy::controlled_v6();
    if !font_enumeration_enabled {
        gdi32_policy.enum_font_families_ex_a = None;
    }
    if !font_creation_enabled {
        gdi32_policy.create_font_a = None;
    }
    if !compatible_dc_enabled {
        gdi32_policy.create_compatible_dc = None;
    }
    if !dib_lifecycle_enabled {
        gdi32_policy.create_dib_section = None;
    }
    if !outline_text_metrics_enabled {
        gdi32_policy.get_outline_text_metrics_a = None;
    }
    if !text_raster_enabled {
        gdi32_policy.text_out_a = None;
    }
    gdi32_policy.validate().map_err(api_fault)?;
    let gdiplus_policy = GdiplusCompatibilityPolicy::controlled_v1();
    gdiplus_policy.validate().map_err(api_fault)?;
    let winmm_policy = WinmmCompatibilityPolicy::controlled_v1();
    winmm_policy.validate().map_err(api_fault)?;
    let mut d3d9_policy = if get_window_rect_enabled {
        D3d9CompatibilityPolicy::controlled_v32()
    } else {
        D3d9CompatibilityPolicy::controlled_v31()
    };
    if !d3d9_clear_enabled {
        d3d9_policy.clear = None;
    }
    if !d3d9_add_dirty_rect_enabled {
        d3d9_policy.add_dirty_rect = None;
    }
    if !d3d9_update_texture_enabled {
        d3d9_policy.update_texture = None;
    }
    if !d3d9_begin_scene_enabled {
        d3d9_policy.begin_scene = None;
    }
    if !d3d9_initial_sampler_states_enabled {
        d3d9_policy.initial_sampler_states = None;
    }
    if !d3d9_initial_texture_stage_states_enabled {
        d3d9_policy.initial_texture_stage_states = None;
    }
    if !d3d9_set_texture_enabled {
        d3d9_policy.texture_binding = None;
    }
    if !d3d9_set_fvf_enabled {
        d3d9_policy.flexible_vertex_format = None;
    }
    if !d3d9_set_stream_source_enabled {
        d3d9_policy.stream_source = None;
    }
    if !d3d9_draw_primitive_enabled {
        d3d9_policy.draw_primitive = None;
    }
    if !d3d9_end_scene_enabled {
        d3d9_policy.end_scene = None;
    }
    if !d3d9_present_enabled {
        d3d9_policy.present = None;
    }
    if !matches!(
        requested_capability,
        Some(
            GenericRuntimeCapability::D3d9GetAdapterMonitor
                | GenericRuntimeCapability::D3d9CreateDevice
                | GenericRuntimeCapability::D3d9CreateTexture
                | GenericRuntimeCapability::D3d9GetSurfaceLevel
                | GenericRuntimeCapability::D3d9SurfaceGetDc
                | GenericRuntimeCapability::TimeGetTime
                | GenericRuntimeCapability::D3d9InitialTextures
                | GenericRuntimeCapability::D3d9InitialVertexBuffer
                | GenericRuntimeCapability::D3d9InitialRenderStates
                | GenericRuntimeCapability::D3d9RasterStatus
                | GenericRuntimeCapability::QueryPerformanceFrequency
                | GenericRuntimeCapability::GetMonitorInfoA
        )
    ) {
        d3d9_policy.get_adapter_monitor = None;
    }
    if !matches!(
        requested_capability,
        Some(
            GenericRuntimeCapability::D3d9CreateDevice
                | GenericRuntimeCapability::D3d9CreateTexture
                | GenericRuntimeCapability::D3d9GetSurfaceLevel
                | GenericRuntimeCapability::D3d9SurfaceGetDc
                | GenericRuntimeCapability::TimeGetTime
                | GenericRuntimeCapability::D3d9InitialTextures
                | GenericRuntimeCapability::D3d9InitialVertexBuffer
                | GenericRuntimeCapability::D3d9InitialRenderStates
                | GenericRuntimeCapability::D3d9RasterStatus
                | GenericRuntimeCapability::QueryPerformanceFrequency
                | GenericRuntimeCapability::GetMonitorInfoA
        )
    ) {
        d3d9_policy.create_device = None;
        d3d9_policy.create_texture = None;
        d3d9_policy.update_texture = None;
        d3d9_policy.begin_scene = None;
    }
    if !matches!(
        requested_capability,
        Some(
            GenericRuntimeCapability::D3d9CreateTexture
                | GenericRuntimeCapability::D3d9GetSurfaceLevel
                | GenericRuntimeCapability::D3d9SurfaceGetDc
                | GenericRuntimeCapability::TimeGetTime
                | GenericRuntimeCapability::D3d9InitialTextures
                | GenericRuntimeCapability::D3d9InitialVertexBuffer
                | GenericRuntimeCapability::D3d9InitialRenderStates
                | GenericRuntimeCapability::D3d9RasterStatus
                | GenericRuntimeCapability::QueryPerformanceFrequency
                | GenericRuntimeCapability::GetMonitorInfoA
        )
    ) {
        d3d9_policy.create_texture = None;
        d3d9_policy.get_surface_level = None;
        d3d9_policy.surface_dc = None;
        d3d9_policy.update_texture = None;
    }
    if !matches!(
        requested_capability,
        Some(
            GenericRuntimeCapability::D3d9GetSurfaceLevel
                | GenericRuntimeCapability::D3d9SurfaceGetDc
                | GenericRuntimeCapability::TimeGetTime
                | GenericRuntimeCapability::D3d9InitialTextures
                | GenericRuntimeCapability::D3d9InitialVertexBuffer
                | GenericRuntimeCapability::D3d9InitialRenderStates
                | GenericRuntimeCapability::D3d9RasterStatus
                | GenericRuntimeCapability::QueryPerformanceFrequency
                | GenericRuntimeCapability::GetMonitorInfoA
        )
    ) {
        d3d9_policy.get_surface_level = None;
        d3d9_policy.surface_dc = None;
    }
    if !matches!(
        requested_capability,
        Some(
            GenericRuntimeCapability::D3d9SurfaceGetDc
                | GenericRuntimeCapability::TimeGetTime
                | GenericRuntimeCapability::D3d9InitialTextures
                | GenericRuntimeCapability::D3d9InitialVertexBuffer
                | GenericRuntimeCapability::D3d9InitialRenderStates
                | GenericRuntimeCapability::D3d9RasterStatus
                | GenericRuntimeCapability::QueryPerformanceFrequency
                | GenericRuntimeCapability::GetMonitorInfoA
        )
    ) {
        d3d9_policy.surface_dc = None;
    }
    if d3d9_recreated_textures_enabled {
        // The complete v17 texture band is enabled only at its own checkpoint.
    } else if matches!(
        requested_capability,
        Some(
            GenericRuntimeCapability::D3d9InitialTextures
                | GenericRuntimeCapability::D3d9InitialVertexBuffer
                | GenericRuntimeCapability::D3d9InitialRenderStates
                | GenericRuntimeCapability::D3d9RasterStatus
                | GenericRuntimeCapability::QueryPerformanceFrequency
                | GenericRuntimeCapability::GetMonitorInfoA
        )
    ) {
        d3d9_policy.restrict_to_initial_textures_v1();
    } else {
        d3d9_policy.restrict_to_system_memory_v1();
    }
    if !matches!(
        requested_capability,
        Some(
            GenericRuntimeCapability::D3d9InitialVertexBuffer
                | GenericRuntimeCapability::D3d9InitialRenderStates
                | GenericRuntimeCapability::D3d9RasterStatus
                | GenericRuntimeCapability::QueryPerformanceFrequency
                | GenericRuntimeCapability::GetMonitorInfoA
        )
    ) {
        d3d9_policy.initial_vertex_buffer = None;
    }
    if !matches!(
        requested_capability,
        Some(
            GenericRuntimeCapability::D3d9InitialRenderStates
                | GenericRuntimeCapability::D3d9RasterStatus
                | GenericRuntimeCapability::QueryPerformanceFrequency
                | GenericRuntimeCapability::GetMonitorInfoA
        )
    ) {
        d3d9_policy.initial_render_states = None;
    }
    if !matches!(
        requested_capability,
        Some(
            GenericRuntimeCapability::D3d9RasterStatus
                | GenericRuntimeCapability::QueryPerformanceFrequency
                | GenericRuntimeCapability::GetMonitorInfoA
        )
    ) {
        d3d9_policy.raster_status = None;
    }
    d3d9_policy.validate().map_err(api_fault)?;
    let mut dinput8_policy = Dinput8CompatibilityPolicy::controlled_v1();
    if !dinput8_enum_devices_enabled {
        dinput8_policy.enum_devices = None;
    }
    dinput8_policy.validate().map_err(api_fault)?;
    let mut dsound_policy = DsoundCompatibilityPolicy::controlled_v1();
    if !dsound_initial_buffers_enabled {
        dsound_policy.initial_buffers = None;
    }
    if !dsound_buffer_transfer_enabled {
        dsound_policy.buffer_transfer = None;
    }
    dsound_policy.validate().map_err(api_fault)?;
    if matches!(
        endpoint_capability,
        Some(
            GenericRuntimeCapability::EnumDisplayMonitors
                | GenericRuntimeCapability::CoInitialize
                | GenericRuntimeCapability::GetSystemDefaultLangId
                | GenericRuntimeCapability::GetDriveTypeA
                | GenericRuntimeCapability::SwapMouseButton
                | GenericRuntimeCapability::TimeGetDevCaps
                | GenericRuntimeCapability::TimeBeginPeriod
                | GenericRuntimeCapability::GetVersionExA
                | GenericRuntimeCapability::CreateWaitableTimerA
        )
    ) {
        process
            .runtime
            .configure_display_profile(user32_policy.display_profile().ok_or_else(|| {
                LoaderError::runtime_defect(
                    "runtime-reentry",
                    "user32-display-profile-missing",
                    "active User32 compatibility policy has no logical display profile",
                )
            })?)
            .map_err(runtime_state_error)?;
    }
    if matches!(
        endpoint_capability,
        Some(
            GenericRuntimeCapability::SwapMouseButton
                | GenericRuntimeCapability::TimeGetDevCaps
                | GenericRuntimeCapability::TimeBeginPeriod
                | GenericRuntimeCapability::GetVersionExA
                | GenericRuntimeCapability::CreateWaitableTimerA,
        )
    ) {
        process
            .runtime
            .configure_input_profile(user32_policy.input_profile().ok_or_else(|| {
                LoaderError::runtime_defect(
                    "runtime-reentry",
                    "user32-input-profile-missing",
                    "active User32 compatibility policy has no logical input profile",
                )
            })?)
            .map_err(runtime_state_error)?;
    }
    let initial_keyboard_event_receipt = if get_async_key_state_input_enabled {
        Some(
            process
                .runtime
                .ingest_keyboard_event(RuntimeKeyboardSourceEvent {
                    sequence: 1,
                    kind: RuntimeKeyboardSourceEventKind::KeyDown,
                    virtual_key: Some(0xa2),
                    repeat: false,
                })
                .map_err(runtime_state_error)?,
        )
    } else {
        None
    };
    let fls_get_value_enabled = matches!(
        endpoint_capability,
        Some(
            GenericRuntimeCapability::FlsGetValue
                | GenericRuntimeCapability::SetLastError
                | GenericRuntimeCapability::GetAcp
                | GenericRuntimeCapability::PrivateHeapSize544
                | GenericRuntimeCapability::GetCpInfo
                | GenericRuntimeCapability::MultiByteToWideChar
                | GenericRuntimeCapability::GetStringTypeW
                | GenericRuntimeCapability::LcMapStringW
                | GenericRuntimeCapability::InterlockedDecrement
                | GenericRuntimeCapability::GetModuleFileNameA
                | GenericRuntimeCapability::PrivateHeapSize16
                | GenericRuntimeCapability::PrivateHeapSize4
                | GenericRuntimeCapability::PrivateHeapFreeSize1
                | GenericRuntimeCapability::PrivateHeapSize128
                | GenericRuntimeCapability::IsProcessorFeaturePresent
                | GenericRuntimeCapability::SetUnhandledExceptionFilter
                | GenericRuntimeCapability::HeapSize
                | GenericRuntimeCapability::InitializeCriticalSection
                | GenericRuntimeCapability::SetCurrentDirectoryA
                | GenericRuntimeCapability::CreateMutexA
                | GenericRuntimeCapability::GetCurrentThread
                | GenericRuntimeCapability::SetThreadAffinityMask
                | GenericRuntimeCapability::GetSystemInfo
                | GenericRuntimeCapability::EnumDisplayMonitors
                | GenericRuntimeCapability::CoInitialize
                | GenericRuntimeCapability::GetSystemDefaultLangId
                | GenericRuntimeCapability::GetDriveTypeA
                | GenericRuntimeCapability::SwapMouseButton
                | GenericRuntimeCapability::TimeGetDevCaps
                | GenericRuntimeCapability::TimeBeginPeriod
                | GenericRuntimeCapability::GetVersionExA
                | GenericRuntimeCapability::CreateWaitableTimerA
        )
    );
    let get_module_file_name_a_enabled = matches!(
        endpoint_capability,
        Some(
            GenericRuntimeCapability::GetModuleFileNameA
                | GenericRuntimeCapability::PrivateHeapSize16
                | GenericRuntimeCapability::PrivateHeapSize4
                | GenericRuntimeCapability::PrivateHeapFreeSize1
                | GenericRuntimeCapability::PrivateHeapSize128
                | GenericRuntimeCapability::IsProcessorFeaturePresent
                | GenericRuntimeCapability::SetUnhandledExceptionFilter
                | GenericRuntimeCapability::HeapSize
                | GenericRuntimeCapability::InitializeCriticalSection
                | GenericRuntimeCapability::SetCurrentDirectoryA
                | GenericRuntimeCapability::CreateMutexA
                | GenericRuntimeCapability::GetCurrentThread
                | GenericRuntimeCapability::SetThreadAffinityMask
                | GenericRuntimeCapability::GetSystemInfo
                | GenericRuntimeCapability::EnumDisplayMonitors
                | GenericRuntimeCapability::CoInitialize
                | GenericRuntimeCapability::GetSystemDefaultLangId
                | GenericRuntimeCapability::GetDriveTypeA
                | GenericRuntimeCapability::SwapMouseButton
                | GenericRuntimeCapability::TimeGetDevCaps
                | GenericRuntimeCapability::TimeBeginPeriod
                | GenericRuntimeCapability::GetVersionExA
                | GenericRuntimeCapability::CreateWaitableTimerA
        )
    );
    let interlocked_decrement_enabled = get_module_file_name_a_enabled
        || matches!(
            endpoint_capability,
            Some(GenericRuntimeCapability::InterlockedDecrement)
        );
    let enabled_capability = match endpoint_capability {
        Some(
            GenericRuntimeCapability::CommandLineA
            | GenericRuntimeCapability::EnvironmentStringsW
            | GenericRuntimeCapability::WideCharToMultiByte
            | GenericRuntimeCapability::PrivateHeapSize1
            | GenericRuntimeCapability::GetLastError
            | GenericRuntimeCapability::TlsGetValue
            | GenericRuntimeCapability::FlsGetValue
            | GenericRuntimeCapability::SetLastError
            | GenericRuntimeCapability::GetAcp
            | GenericRuntimeCapability::PrivateHeapSize544
            | GenericRuntimeCapability::GetCpInfo
            | GenericRuntimeCapability::MultiByteToWideChar
            | GenericRuntimeCapability::GetStringTypeW
            | GenericRuntimeCapability::LcMapStringW
            | GenericRuntimeCapability::InterlockedDecrement
            | GenericRuntimeCapability::GetModuleFileNameA
            | GenericRuntimeCapability::PrivateHeapSize16
            | GenericRuntimeCapability::PrivateHeapSize4
            | GenericRuntimeCapability::PrivateHeapFreeSize1
            | GenericRuntimeCapability::PrivateHeapSize128
            | GenericRuntimeCapability::IsProcessorFeaturePresent
            | GenericRuntimeCapability::SetUnhandledExceptionFilter
            | GenericRuntimeCapability::HeapSize
            | GenericRuntimeCapability::InitializeCriticalSection
            | GenericRuntimeCapability::SetCurrentDirectoryA
            | GenericRuntimeCapability::CreateMutexA
            | GenericRuntimeCapability::GetCurrentThread
            | GenericRuntimeCapability::SetThreadAffinityMask
            | GenericRuntimeCapability::GetSystemInfo
            | GenericRuntimeCapability::EnumDisplayMonitors
            | GenericRuntimeCapability::CoInitialize
            | GenericRuntimeCapability::GetSystemDefaultLangId
            | GenericRuntimeCapability::GetDriveTypeA
            | GenericRuntimeCapability::SwapMouseButton
            | GenericRuntimeCapability::TimeGetDevCaps
            | GenericRuntimeCapability::TimeBeginPeriod
            | GenericRuntimeCapability::GetVersionExA
            | GenericRuntimeCapability::CreateWaitableTimerA,
        ) => Some(GenericRuntimeCapability::SetHandleCount),
        capability => capability,
    };
    let endpoints = process
        .binding
        .imports
        .iter()
        .filter_map(ApiEndpointIdentity::from_runtime_guest_import)
        .collect::<Vec<_>>();
    if endpoints.len() != process.binding.imports.len() {
        return Err(LoaderError::runtime_defect(
            "runtime-reentry",
            "non-runtime-endpoint-in-bgi-binding",
            "BGI generic runtime binding contains a non-runtime export endpoint",
        ));
    }
    let mut dispatcher_endpoints = endpoints.clone();
    let mut contracts = Vec::new();
    for endpoint in &endpoints {
        let Some(contract) = generic_runtime_contract_for_endpoint(endpoint.clone())? else {
            continue;
        };
        let endpoint_enabled = if endpoint.matches(
            "dinput8.dll",
            &ApiSymbol::Name {
                name: "DirectInput8Create".to_owned(),
            },
        ) {
            dinput8_factory_enabled
        } else if endpoint.module.eq_ignore_ascii_case("dsound.dll") {
            dsound_root_enabled
        } else if endpoint.matches(
            "gdi32.dll",
            &ApiSymbol::Name {
                name: "EnumFontFamiliesExA".to_owned(),
            },
        ) {
            font_enumeration_enabled
        } else if endpoint.matches(
            "gdi32.dll",
            &ApiSymbol::Name {
                name: "CreateFontA".to_owned(),
            },
        ) {
            font_creation_enabled
        } else if endpoint.module.eq_ignore_ascii_case("gdiplus.dll") {
            gdiplus_startup_enabled
        } else if endpoint.module.eq_ignore_ascii_case("imm32.dll") {
            imm32_context_enabled
        } else if endpoint.module.eq_ignore_ascii_case("shell32.dll") {
            drag_accept_files_enabled
        } else if endpoint.matches(
            "kernel32.dll",
            &ApiSymbol::Name {
                name: "SetWaitableTimer".to_owned(),
            },
        ) {
            set_waitable_timer_enabled
        } else if endpoint.matches(
            "kernel32.dll",
            &ApiSymbol::Name {
                name: "WaitForSingleObjectEx".to_owned(),
            },
        ) {
            wait_for_single_object_ex_enabled
        } else if endpoint.matches(
            "kernel32.dll",
            &ApiSymbol::Name {
                name: "Sleep".to_owned(),
            },
        ) {
            sleep_zero_enabled
        } else if endpoint.matches(
            "kernel32.dll",
            &ApiSymbol::Name {
                name: "GlobalMemoryStatus".to_owned(),
            },
        ) {
            global_memory_status_enabled
        } else if endpoint.matches(
            "kernel32.dll",
            &ApiSymbol::Name {
                name: "GetFileAttributesA".to_owned(),
            },
        ) {
            get_file_attributes_a_enabled
        } else if endpoint.matches(
            "kernel32.dll",
            &ApiSymbol::Name {
                name: "CreateDirectoryA".to_owned(),
            },
        ) {
            create_directory_a_enabled
        } else if endpoint.module.eq_ignore_ascii_case("user32.dll")
            && matches!(
                endpoint.symbol,
                ApiSymbol::Name { ref name }
                    if matches!(name.as_str(), "TranslateMessage" | "DispatchMessageA")
            )
        {
            window_timer_delivery_enabled
        } else {
            runtime_capability_enables_endpoint(
                requested_capability,
                endpoint,
                virtual_alloc_enabled,
                set_rect_enabled,
                get_window_rect_enabled,
            )
        };
        if endpoint_enabled {
            contracts.push(contract);
        }
    }
    if matches!(
        enabled_capability,
        Some(
            GenericRuntimeCapability::FiberLocalStorageAllocation
                | GenericRuntimeCapability::PrivateHeapAllocation
                | GenericRuntimeCapability::FiberLocalStorageValue
                | GenericRuntimeCapability::InterlockedIncrement
                | GenericRuntimeCapability::PrivateHeapSecondAllocation
                | GenericRuntimeCapability::StandardInputHandle
                | GenericRuntimeCapability::FileType
                | GenericRuntimeCapability::StandardOutputHandle
                | GenericRuntimeCapability::StandardOutputFileType
                | GenericRuntimeCapability::StandardErrorHandle
                | GenericRuntimeCapability::StandardErrorFileType
                | GenericRuntimeCapability::SetHandleCount
        )
    ) {
        let endpoint = ApiEndpointIdentity::from_dynamic_endpoint(
            process
                .runtime
                .dynamic_endpoint_by_name("kernel32.dll", "FlsAlloc")
                .map_err(runtime_state_error)?,
        );
        let contract = kernel32_contract_for_endpoint(endpoint.clone())
            .map_err(api_fault)?
            .ok_or_else(|| {
                LoaderError::runtime_defect(
                    "runtime-reentry",
                    "fls-allocation-contract-missing",
                    "the reached dynamic FlsAlloc endpoint has no Kernel32 API contract",
                )
            })?;
        dispatcher_endpoints.push(endpoint);
        contracts.push(contract);
    }
    if fls_get_value_enabled {
        let endpoint = ApiEndpointIdentity::from_dynamic_endpoint(
            process
                .runtime
                .dynamic_endpoint_by_name("kernel32.dll", "FlsGetValue")
                .map_err(runtime_state_error)?,
        );
        let contract = kernel32_contract_for_endpoint(endpoint.clone())
            .map_err(api_fault)?
            .ok_or_else(|| {
                LoaderError::runtime_defect(
                    "runtime-reentry",
                    "fls-get-value-contract-missing",
                    "the reached dynamic FlsGetValue endpoint has no Kernel32 API contract",
                )
            })?;
        dispatcher_endpoints.push(endpoint);
        contracts.push(contract);
    }
    if matches!(
        enabled_capability,
        Some(
            GenericRuntimeCapability::FiberLocalStorageValue
                | GenericRuntimeCapability::InterlockedIncrement
                | GenericRuntimeCapability::PrivateHeapSecondAllocation
                | GenericRuntimeCapability::StandardInputHandle
                | GenericRuntimeCapability::FileType
                | GenericRuntimeCapability::StandardOutputHandle
                | GenericRuntimeCapability::StandardOutputFileType
                | GenericRuntimeCapability::StandardErrorHandle
                | GenericRuntimeCapability::StandardErrorFileType
                | GenericRuntimeCapability::SetHandleCount
        )
    ) {
        let endpoint = ApiEndpointIdentity::from_dynamic_endpoint(
            process
                .runtime
                .dynamic_endpoint_by_name("kernel32.dll", "FlsSetValue")
                .map_err(runtime_state_error)?,
        );
        let contract = kernel32_contract_for_endpoint(endpoint.clone())
            .map_err(api_fault)?
            .ok_or_else(|| {
                LoaderError::runtime_defect(
                    "runtime-reentry",
                    "fls-set-value-contract-missing",
                    "the reached dynamic FlsSetValue endpoint has no Kernel32 API contract",
                )
            })?;
        dispatcher_endpoints.push(endpoint);
        contracts.push(contract);
    }
    if matches!(
        requested_capability,
        Some(
            GenericRuntimeCapability::LoadLibraryA
                | GenericRuntimeCapability::GetClientRect
                | GenericRuntimeCapability::WindowDcFill
                | GenericRuntimeCapability::SetWindowLongA
                | GenericRuntimeCapability::SetWindowPosActivation
                | GenericRuntimeCapability::MonitorFromWindow
                | GenericRuntimeCapability::D3d9GetAdapterCount
                | GenericRuntimeCapability::D3d9GetAdapterMonitor
                | GenericRuntimeCapability::D3d9CreateDevice
                | GenericRuntimeCapability::D3d9CreateTexture
                | GenericRuntimeCapability::D3d9GetSurfaceLevel
                | GenericRuntimeCapability::D3d9SurfaceGetDc
                | GenericRuntimeCapability::TimeGetTime
                | GenericRuntimeCapability::D3d9InitialTextures
                | GenericRuntimeCapability::D3d9InitialVertexBuffer
                | GenericRuntimeCapability::D3d9InitialRenderStates
                | GenericRuntimeCapability::D3d9RasterStatus
                | GenericRuntimeCapability::QueryPerformanceFrequency
                | GenericRuntimeCapability::GetMonitorInfoA
        )
    ) {
        let loader = kernel32_policy.load_library_a().ok_or_else(|| {
            LoaderError::runtime_defect(
                "runtime-reentry",
                "load-library-a-policy-missing",
                "active LoadLibraryA capability has no runtime-system module policy",
            )
        })?;
        for module in &loader.runtime_system_modules {
            for export in &module.exports {
                dispatcher_endpoints.push(ApiEndpointIdentity::from_dynamic_endpoint(
                    process
                        .runtime
                        .dynamic_endpoint_by_name(&module.canonical_name, export)
                        .map_err(runtime_state_error)?,
                ));
            }
        }
    }
    let dispatcher =
        ApiDispatcher::new(dispatcher_endpoints, contracts.clone()).map_err(api_fault)?;
    let mut active_callbacks = Vec::<GenericRuntimeActiveCallback>::new();
    let initial_thread_stack = process
        .runtime
        .thread_stack(process.runtime.current_thread_id())
        .map_err(runtime_state_error)?
        .clone();
    let initial_stack_address_allocation = process
        .runtime
        .process_address_space_allocator()
        .allocation(initial_thread_stack.address_allocation_id)
        .map_err(runtime_state_error)?
        .clone();
    write_runtime_json_frame(
        RUNTIME_FRAME_SESSION_READY,
        &GenericRuntimeSessionReady {
            schema: "vine.v86-generic-runtime-session-ready",
            schema_version: 1,
            process_id: process.runtime.process_id().0,
            current_thread_id: process.runtime.current_thread_id().0,
            current_directory_a: process.runtime.current_directory_a().clone(),
            current_directory_generation: process.runtime.current_directory_generation(),
            process_lifecycle: process.runtime.process().lifecycle(),
            scheduler_state: process
                .runtime
                .scheduler_state()
                .map_err(runtime_state_error)?,
            dll_lifecycle: process.runtime.dll_lifecycle_phase(),
            modules: process.runtime.modules().values().cloned().collect(),
            main_module_handle: process.main_module_handle,
            dependency_edges: process.binding.dependency_edges.len(),
            attach_order: process.attach_order.clone(),
            runtime_export_endpoints: endpoints.len(),
            implemented_api_contracts: contracts.len(),
            dynamic_export_endpoints: process.runtime.registries().dynamic_endpoints().len(),
            user32_compatibility_policy_id: matches!(
                endpoint_capability,
                Some(
                    GenericRuntimeCapability::EnumDisplayMonitors
                        | GenericRuntimeCapability::CoInitialize
                        | GenericRuntimeCapability::GetSystemDefaultLangId
                        | GenericRuntimeCapability::GetDriveTypeA
                        | GenericRuntimeCapability::SwapMouseButton
                        | GenericRuntimeCapability::TimeGetDevCaps
                        | GenericRuntimeCapability::TimeBeginPeriod
                        | GenericRuntimeCapability::GetVersionExA
                        | GenericRuntimeCapability::CreateWaitableTimerA
                )
            )
            .then(|| user32_policy.id.clone()),
            gdi32_compatibility_policy_id: (font_enumeration_enabled || font_creation_enabled)
                .then(|| gdi32_policy.id.clone()),
            gdiplus_compatibility_policy_id: gdiplus_startup_enabled
                .then(|| gdiplus_policy.id.clone()),
            imm32_compatibility_policy_id: imm32_context_enabled.then(|| imm32_policy.id.clone()),
            winmm_compatibility_policy_id: matches!(
                endpoint_capability,
                Some(
                    GenericRuntimeCapability::TimeGetDevCaps
                        | GenericRuntimeCapability::TimeBeginPeriod
                        | GenericRuntimeCapability::GetVersionExA
                        | GenericRuntimeCapability::CreateWaitableTimerA,
                )
            )
            .then(|| winmm_policy.id.clone()),
            display_profile_id: process.runtime.display_profile_id().map(str::to_owned),
            primary_monitor: process.runtime.primary_monitor().cloned(),
            mouse_button_mapping: process.runtime.mouse_button_mapping().cloned(),
            keyboard_state: get_async_key_state_enabled
                .then(|| process.runtime.keyboard_state().cloned())
                .flatten(),
            cursor_position: get_window_rect_enabled
                .then(|| process.runtime.cursor_position().cloned())
                .flatten(),
            initial_keyboard_event_receipt,
            source_snapshot: process.runtime.source_snapshot().cloned(),
            writable_profile: process.runtime.writable_profile().cloned(),
            initial_thread_stack,
            initial_stack_address_allocation,
            memory: memory.stats(),
        },
    )?;

    for _ in 0..trap_limit {
        let mut writeback_frame = read_runtime_frame(input)?;
        while matches!(
            writeback_frame.kind,
            RUNTIME_FRAME_KEYBOARD_INPUT | RUNTIME_FRAME_POINTER_BUTTON_INPUT
        ) {
            if writeback_frame.kind == RUNTIME_FRAME_POINTER_BUTTON_INPUT {
                if !writeback_frame.payload.is_empty() {
                    return Err(LoaderError::invalid_input(
                        "runtime-reentry",
                        "runtime-pointer-button-input-payload-unexpected",
                        "generic runtime pointer-button frames may not contain a byte payload",
                    ));
                }
                let request: GenericRuntimePointerButtonInputRequest = serde_json::from_slice(
                    &writeback_frame.control,
                )
                .map_err(|error| {
                    LoaderError::invalid_input(
                        "runtime-reentry",
                        "runtime-pointer-button-input-json-invalid",
                        format!(
                            "generic runtime pointer-button input frame is invalid JSON: {error}"
                        ),
                    )
                })?;
                if request.schema != "vine.v86-generic-runtime-pointer-button-input"
                    || request.schema_version != 1
                {
                    return Err(LoaderError::invalid_input(
                        "runtime-reentry",
                        "runtime-pointer-button-input-schema-unsupported",
                        "generic runtime pointer-button input requires schema version 1",
                    ));
                }
                let kind = match request.event.kind.as_str() {
                    "button-down" => RuntimePointerButtonSourceEventKind::ButtonDown,
                    "button-up" => RuntimePointerButtonSourceEventKind::ButtonUp,
                    _ => {
                        return Err(LoaderError::invalid_input(
                            "runtime-reentry",
                            "runtime-pointer-button-input-kind-invalid",
                            "generic runtime pointer-button input has an unknown event kind",
                        ));
                    }
                };
                let button = match request.event.button.as_str() {
                    "primary" => RuntimePointerButton::Primary,
                    "secondary" => RuntimePointerButton::Secondary,
                    _ => {
                        return Err(LoaderError::invalid_input(
                            "runtime-reentry",
                            "runtime-pointer-button-input-button-invalid",
                            "generic runtime pointer-button input has an unknown source button",
                        ));
                    }
                };
                let profile = user32_policy.pointer_button_messages.as_ref().ok_or_else(|| {
                    LoaderError::invalid_input(
                        "runtime-reentry",
                        "runtime-pointer-button-input-policy-missing",
                        "generic runtime pointer-button input requires the User32 pointer profile",
                    )
                })?;
                let receipt = process
                    .runtime
                    .ingest_pointer_button_event(
                        RuntimePointerButtonSourceEvent {
                            sequence: request.event.sequence,
                            kind,
                            button,
                            normalized_x: request.event.normalized_x,
                            normalized_y: request.event.normalized_y,
                        },
                        profile,
                    )
                    .map_err(runtime_state_error)?;
                let keyboard = process.runtime.keyboard_state().cloned().ok_or_else(|| {
                    LoaderError::runtime_defect(
                        "runtime-reentry",
                        "runtime-pointer-button-input-state-missing",
                        "pointer-button event committed without authoritative keyboard state",
                    )
                })?;
                write_runtime_json_frame(
                    RUNTIME_FRAME_POINTER_BUTTON_INPUT_COMPLETION,
                    &GenericRuntimePointerButtonInputCompletion {
                        schema: "vine.v86-generic-runtime-pointer-button-input-completion",
                        schema_version: 1,
                        receipt,
                        keyboard,
                        memory: memory.stats(),
                    },
                )?;
                writeback_frame = read_runtime_frame(input)?;
                continue;
            }
            if !writeback_frame.payload.is_empty() {
                return Err(LoaderError::invalid_input(
                    "runtime-reentry",
                    "runtime-keyboard-input-payload-unexpected",
                    "generic runtime keyboard-input frames may not contain a byte payload",
                ));
            }
            let request: GenericRuntimeKeyboardInputRequest =
                serde_json::from_slice(&writeback_frame.control).map_err(|error| {
                    LoaderError::invalid_input(
                        "runtime-reentry",
                        "runtime-keyboard-input-json-invalid",
                        format!("generic runtime keyboard-input frame is invalid JSON: {error}"),
                    )
                })?;
            if request.schema != "vine.v86-generic-runtime-keyboard-input"
                || request.schema_version != 1
            {
                return Err(LoaderError::invalid_input(
                    "runtime-reentry",
                    "runtime-keyboard-input-schema-unsupported",
                    "generic runtime keyboard input requires schema version 1",
                ));
            }
            let kind = match request.event.kind.as_str() {
                "focus-gained" => RuntimeKeyboardSourceEventKind::FocusGained,
                "focus-lost" => RuntimeKeyboardSourceEventKind::FocusLost,
                "key-down" => RuntimeKeyboardSourceEventKind::KeyDown,
                "key-up" => RuntimeKeyboardSourceEventKind::KeyUp,
                _ => {
                    return Err(LoaderError::invalid_input(
                        "runtime-reentry",
                        "runtime-keyboard-input-kind-invalid",
                        "generic runtime keyboard input has an unknown event kind",
                    ));
                }
            };
            let receipt = process
                .runtime
                .ingest_keyboard_event(RuntimeKeyboardSourceEvent {
                    sequence: request.event.sequence,
                    kind,
                    virtual_key: request.event.virtual_key,
                    repeat: request.event.repeat,
                })
                .map_err(runtime_state_error)?;
            let keyboard = process.runtime.keyboard_state().cloned().ok_or_else(|| {
                LoaderError::runtime_defect(
                    "runtime-reentry",
                    "runtime-keyboard-input-state-missing",
                    "keyboard event committed without an authoritative keyboard state",
                )
            })?;
            write_runtime_json_frame(
                RUNTIME_FRAME_KEYBOARD_INPUT_COMPLETION,
                &GenericRuntimeKeyboardInputCompletion {
                    schema: "vine.v86-generic-runtime-keyboard-input-completion",
                    schema_version: 1,
                    receipt,
                    keyboard,
                    memory: memory.stats(),
                },
            )?;
            writeback_frame = read_runtime_frame(input)?;
        }
        require_runtime_frame_kind(&writeback_frame, RUNTIME_FRAME_CPU_WRITEBACK)?;
        let writeback_receipt = memory
            .reconcile_cpu_writeback_binary(&writeback_frame.control, &writeback_frame.payload)
            .map_err(memory_error)?;
        let receipt_control = encode_writeback_receipt(memory, &writeback_receipt)?;
        write_runtime_frame(RUNTIME_FRAME_WRITEBACK_RECEIPT, &receipt_control, &[])?;

        let trap_frame = read_runtime_frame(input)?;
        if trap_frame.kind == RUNTIME_FRAME_SESSION_STOP {
            if !trap_frame.payload.is_empty() {
                return Err(LoaderError::invalid_input(
                    "runtime-reentry",
                    "runtime-session-stop-payload-unexpected",
                    "generic runtime session-stop frames may not contain a byte payload",
                ));
            }
            let stop: GenericRuntimeSessionStopRequest =
                serde_json::from_slice(&trap_frame.control).map_err(|error| {
                    LoaderError::invalid_input(
                        "runtime-reentry",
                        "runtime-session-stop-json-invalid",
                        format!("generic runtime session-stop frame is invalid JSON: {error}"),
                    )
                })?;
            if stop.schema != "vine.v86-generic-runtime-session-stop"
                || stop.schema_version != 1
                || stop.generation != memory.stats().generation
                || stop.reason.is_empty()
            {
                return Err(LoaderError::invalid_input(
                    "runtime-reentry",
                    "runtime-session-stop-invalid",
                    "generic runtime session-stop identity, generation, or reason is invalid",
                ));
            }
            let memory_content_sha256 = memory
                .current_reflection()
                .map_err(memory_error)?
                .content_sha256;
            write_runtime_json_frame(
                RUNTIME_FRAME_SESSION_STOPPED,
                &GenericRuntimeSessionStopped {
                    schema: "vine.v86-generic-runtime-session-stopped",
                    schema_version: 1,
                    reason: stop.reason,
                    memory: memory.stats(),
                    memory_content_sha256,
                },
            )?;
            return Ok(());
        }
        if trap_frame.kind == RUNTIME_FRAME_THREAD_EXIT {
            if !trap_frame.payload.is_empty() {
                return Err(LoaderError::invalid_input(
                    "runtime-reentry",
                    "runtime-thread-exit-payload-unexpected",
                    "generic runtime thread-exit frames may not contain a byte payload",
                ));
            }
            let request: GenericRuntimeThreadExitRequest =
                serde_json::from_slice(&trap_frame.control).map_err(|error| {
                    LoaderError::invalid_input(
                        "runtime-reentry",
                        "runtime-thread-exit-json-invalid",
                        format!("generic runtime thread-exit metadata is invalid JSON: {error}"),
                    )
                })?;
            let thread_id = process.runtime.current_thread_id();
            let thread = process
                .runtime
                .thread_state(thread_id)
                .map_err(runtime_state_error)?;
            if request.schema != "vine.v86-generic-runtime-thread-exit"
                || request.schema_version != 1
                || request.generation != memory.stats().generation
                || request.thread_id != thread_id.0
                || thread.guest_bootstrap.is_none()
            {
                return Err(LoaderError::invalid_input(
                    "runtime-reentry",
                    "runtime-thread-exit-identity-invalid",
                    "generic runtime thread-exit identity, generation, or bootstrap is invalid",
                ));
            }
            process
                .runtime
                .request_current_thread_exit(request.exit_code)
                .map_err(runtime_state_error)?;
            process
                .runtime
                .complete_current_guest_thread_exit(memory)
                .map_err(runtime_state_error)?;
            let scheduler_state = process
                .runtime
                .scheduler_state_for_thread(thread_id)
                .map_err(runtime_state_error)?;
            if scheduler_state
                != (SchedulerThreadState::Terminated {
                    exit_code: request.exit_code,
                })
            {
                return Err(LoaderError::runtime_defect(
                    "runtime-reentry",
                    "runtime-thread-exit-state-mismatch",
                    "generic runtime thread exit did not reach the matching terminated lane",
                ));
            }
            let thread_selection = process
                .runtime
                .select_next_executable_thread()
                .map_err(runtime_state_error)?;
            send_pending_runtime_delta(input, memory)?;
            write_runtime_json_frame(
                RUNTIME_FRAME_THREAD_EXIT_COMPLETION,
                &GenericRuntimeThreadExitCompletion {
                    schema: "vine.v86-generic-runtime-thread-exit-completion",
                    schema_version: 1,
                    thread_id: thread_id.0,
                    exit_code: request.exit_code,
                    scheduler_state,
                    thread_selection,
                    memory: memory.stats(),
                },
            )?;
            continue;
        }
        if trap_frame.kind == RUNTIME_FRAME_GUEST_CALLBACK_RETURN {
            if !trap_frame.payload.is_empty() {
                return Err(LoaderError::invalid_input(
                    "runtime-reentry",
                    "runtime-callback-return-payload-unexpected",
                    "generic runtime callback-return frames may not contain a byte payload",
                ));
            }
            let request: GenericRuntimeCallbackReturnRequest =
                serde_json::from_slice(&trap_frame.control).map_err(|error| {
                    LoaderError::invalid_input(
                        "runtime-reentry",
                        "runtime-callback-return-json-invalid",
                        format!("generic runtime callback-return metadata is invalid: {error}"),
                    )
                })?;
            let active = active_callbacks.last().ok_or_else(|| {
                LoaderError::invalid_input(
                    "runtime-reentry",
                    "runtime-callback-return-unowned",
                    "generic runtime callback return has no active API callback",
                )
            })?;
            if request.schema != "vine.v86-generic-runtime-callback-return"
                || request.schema_version != 1
                || request.frame_id != active.frame_id.0
                || request.return_sentinel != active.call.request.return_sentinel.0
                || request.stack_pointer != active.call.frame.stack_pointer.0
            {
                return Err(LoaderError::invalid_input(
                    "runtime-reentry",
                    "runtime-callback-return-identity-mismatch",
                    "generic runtime callback return does not match the active frame, sentinel, or stdcall stack",
                ));
            }
            let call = active.call.clone();
            let frame_id = active.frame_id;
            let completion_policy = call.request.completion_policy;
            process
                .runtime
                .validate_callback_frame_return(frame_id)
                .map_err(runtime_state_error)?;
            let endpoint = call.frame.endpoint.clone();
            let mut session = ExecutionSession::new(memory, &mut process.runtime);
            match dispatcher.complete_guest_callback_result(call, request.result, &mut session) {
                DispatchOutcome::Immediate(completion) => {
                    session
                        .runtime_mut()
                        .pop_callback_frame(frame_id)
                        .map_err(runtime_state_error)?;
                    let semantic_action = session
                        .commit_guest_callback_semantics(
                            completion_policy,
                            request.result,
                            &endpoint,
                        )
                        .map_err(api_fault)?;
                    let window = match semantic_action {
                        Some(vine_core::RuntimeGuestCallbackSemanticAction::WindowCreation(
                            vine_core::RuntimeWindowCreationCallbackAction::Created { window },
                        ))
                        | Some(vine_core::RuntimeGuestCallbackSemanticAction::SetWindowPos(
                            vine_core::RuntimeSetWindowPosCallbackAction::Completed { window },
                        ))
                        | Some(vine_core::RuntimeGuestCallbackSemanticAction::UpdateWindow(
                            vine_core::RuntimeUpdateWindowCallbackAction::Completed { window },
                        )) => Some(window),
                        _ => None,
                    };
                    let retired = active_callbacks
                        .pop()
                        .expect("validated active callback disappeared before commit");
                    debug_assert_eq!(retired.frame_id, frame_id);
                    let gdi = generic_runtime_gdi_completion(
                        &completion,
                        session.runtime(),
                        &gdi32_policy,
                    )?;
                    let paint = generic_runtime_paint_completion(&completion, session.runtime())?;
                    let gdiplus = generic_runtime_gdiplus_completion(
                        &completion,
                        session.runtime(),
                        &gdiplus_policy,
                    )?;
                    let window_timer = if window_timer_delivery_enabled
                        && completion.implementation_id == DISPATCH_MESSAGE_A_IMPLEMENTATION
                        && session
                            .runtime()
                            .window_timers()
                            .values()
                            .any(|timer| timer.callback_address == retired.call.request.target)
                    {
                        Some(GenericRuntimeWindowTimer::snapshot(
                            *session
                                .runtime()
                                .window_timers()
                                .values()
                                .filter(|timer| timer.last_delivery.is_some())
                                .max_by_key(|timer| {
                                    timer
                                        .last_delivery
                                        .map(|delivery| delivery.delivered_at_time_100ns)
                                })
                                .ok_or_else(|| {
                                    LoaderError::runtime_defect(
                                        "runtime-reentry",
                                        "dispatch-message-window-timer-state-missing-after-callback",
                                        "DispatchMessageA callback completed without delivered window timer state",
                                    )
                                })?,
                            true,
                        ))
                    } else {
                        None
                    };
                    send_pending_runtime_delta(input, memory)?;
                    write_runtime_json_frame(
                        RUNTIME_FRAME_API_COMPLETION,
                        &GenericRuntimeCompletion {
                            schema: "vine.v86-generic-runtime-completion",
                            schema_version: 1,
                            endpoint: GenericRuntimeEndpoint::RuntimeSystemExport { endpoint },
                            completion,
                            fls_value: None,
                            interlocked_increment: None,
                            interlocked_decrement: None,
                            heap_allocation: None,
                            heap_free: None,
                            standard_handle: None,
                            file_type: None,
                            current_directory: None,
                            mutex: None,
                            waitable_timer: None,
                            icon: None,
                            cursor: None,
                            window_class: None,
                            window,
                            paint,
                            window_timer,
                            queued_thread_messages: None,
                            thread_affinity: None,
                            thread_priority: None,
                            imm32: None,
                            virtual_memory: None,
                            created_thread: None,
                            resumed_thread: None,
                            tick_count: None,
                            com_apartment: None,
                            timer_resolution: None,
                            source_read: None,
                            writable_commit: None,
                            writable_directory_create: None,
                            runtime_module: None,
                            gdi,
                            gdiplus,
                            thread_selection: None,
                            memory: memory.stats(),
                        },
                    )?;
                }
                DispatchOutcome::GuestCallback(next) => {
                    let prepared = session
                        .replace_guest_callback(&next, frame_id)
                        .map_err(api_fault)?;
                    let semantic_action = session
                        .commit_guest_callback_semantics(
                            completion_policy,
                            request.result,
                            &endpoint,
                        )
                        .map_err(api_fault)?;
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
                        return Err(LoaderError::runtime_defect(
                            "runtime-reentry",
                            "runtime-callback-continuation-action-mismatch",
                            "replacement guest callback did not commit a continuation action",
                        ));
                    }
                    let queued_thread_messages = session
                        .runtime()
                        .queued_thread_messages(session.runtime().current_thread_id())
                        .map_err(runtime_state_error)?;
                    send_pending_runtime_delta(input, memory)?;
                    let callback_entry = GenericRuntimeCallbackEntry {
                        schema: "vine.v86-generic-runtime-callback",
                        schema_version: 1,
                        endpoint: GenericRuntimeEndpoint::RuntimeSystemExport {
                            endpoint: endpoint.clone(),
                        },
                        frame_id: prepared.frame_id.0,
                        target: next.request.target.0,
                        argument_count: next.request.arguments.len(),
                        api_argument_count: next.frame.arguments.len(),
                        allocation_start: prepared.allocation_start.0,
                        stack_pointer: prepared.stack_pointer.0,
                        scratch_address: prepared.scratch_address.0,
                        scratch_bytes: prepared.scratch_bytes,
                        frame_bytes: prepared.frame_bytes,
                        return_sentinel: next.request.return_sentinel.0,
                        queued_thread_messages,
                        memory: memory.stats(),
                    };
                    *active_callbacks
                        .last_mut()
                        .expect("validated active callback disappeared before replacement") =
                        GenericRuntimeActiveCallback {
                            call: next,
                            frame_id: prepared.frame_id,
                        };
                    write_runtime_json_frame(RUNTIME_FRAME_GUEST_CALLBACK, &callback_entry)?;
                }
                DispatchOutcome::Fault(fault) => {
                    write_generic_runtime_fault(
                        memory,
                        GenericRuntimeEndpoint::RuntimeSystemExport { endpoint },
                        fault,
                    )?;
                    return Ok(());
                }
                DispatchOutcome::Deferred(_) => {
                    return Err(LoaderError::runtime_defect(
                        "runtime-reentry",
                        "runtime-callback-completion-suspended",
                        "completed guest callback produced a deferred API outcome",
                    ));
                }
            }
            continue;
        }
        if trap_frame.kind == RUNTIME_FRAME_PAGE_FAULT_SERVICE {
            if !trap_frame.payload.is_empty() {
                return Err(LoaderError::invalid_input(
                    "runtime-reentry",
                    "runtime-page-fault-payload-unexpected",
                    "generic runtime page-fault frames may not contain a byte payload",
                ));
            }
            let request: GenericRuntimePageFaultRequest =
                serde_json::from_slice(&trap_frame.control).map_err(|error| {
                    LoaderError::invalid_input(
                        "runtime-reentry",
                        "runtime-page-fault-json-invalid",
                        format!("generic runtime page-fault metadata is invalid: {error}"),
                    )
                })?;
            if request.schema != "vine.v86-generic-runtime-page-fault"
                || request.schema_version != 1
                || request.generation != memory.stats().generation
            {
                return Err(LoaderError::invalid_input(
                    "runtime-reentry",
                    "runtime-page-fault-request-invalid",
                    "generic runtime page-fault identity or generation is invalid",
                ));
            }
            let resolution = process
                .runtime
                .service_page_fault(
                    memory,
                    GuestAddress(request.fault_address),
                    request.page_fault_error_code,
                )
                .map_err(runtime_state_error)?;
            send_pending_runtime_delta(input, memory)?;
            write_runtime_json_frame(
                RUNTIME_FRAME_PAGE_FAULT_COMPLETION,
                &GenericRuntimePageFaultCompletion {
                    schema: "vine.v86-generic-runtime-page-fault-completion",
                    schema_version: 1,
                    resolution,
                    memory: memory.stats(),
                },
            )?;
            continue;
        }
        if trap_frame.kind == RUNTIME_FRAME_STACK_GROWTH {
            if !trap_frame.payload.is_empty() {
                return Err(LoaderError::invalid_input(
                    "runtime-reentry",
                    "runtime-stack-growth-payload-unexpected",
                    "generic runtime stack-growth frames may not contain a byte payload",
                ));
            }
            let request: GenericRuntimeStackGrowthRequest =
                serde_json::from_slice(&trap_frame.control).map_err(|error| {
                    LoaderError::invalid_input(
                        "runtime-reentry",
                        "runtime-stack-growth-json-invalid",
                        format!("generic runtime stack-growth metadata is invalid: {error}"),
                    )
                })?;
            if request.schema != "vine.v86-generic-runtime-stack-growth"
                || request.schema_version != 1
                || request.generation != memory.stats().generation
            {
                return Err(LoaderError::invalid_input(
                    "runtime-reentry",
                    "runtime-stack-growth-request-invalid",
                    "generic runtime stack-growth identity or generation is invalid",
                ));
            }
            let growth = process
                .runtime
                .grow_current_thread_stack(
                    memory,
                    GuestAddress(request.fault_address),
                    request.page_fault_error_code,
                )
                .map_err(runtime_state_error)?;
            send_pending_runtime_delta(input, memory)?;
            write_runtime_json_frame(
                RUNTIME_FRAME_STACK_GROWTH_COMPLETION,
                &GenericRuntimeStackGrowthCompletion {
                    schema: "vine.v86-generic-runtime-stack-growth-completion",
                    schema_version: 1,
                    growth,
                    memory: memory.stats(),
                },
            )?;
            continue;
        }
        require_runtime_frame_kind(&trap_frame, RUNTIME_FRAME_TRAP)?;
        if !trap_frame.payload.is_empty() {
            return Err(LoaderError::invalid_input(
                "runtime-reentry",
                "runtime-trap-payload-unexpected",
                "generic runtime trap frames may not contain a byte payload",
            ));
        }
        let trap: GenericRuntimeTrapRequest =
            serde_json::from_slice(&trap_frame.control).map_err(|error| {
                LoaderError::invalid_input(
                    "runtime-reentry",
                    "runtime-trap-json-invalid",
                    format!("generic runtime trap metadata is invalid: {error}"),
                )
            })?;
        if trap.schema != "vine.v86-generic-runtime-trap" || trap.schema_version != 1 {
            return Err(LoaderError::invalid_input(
                "runtime-reentry",
                "runtime-trap-schema-unsupported",
                "generic runtime trap must use vine.v86-generic-runtime-trap version 1",
            ));
        }

        match trap.kind {
            GenericRuntimeTrapKind::RuntimeSystemExport => {
                let endpoint = endpoints
                    .iter()
                    .find(|endpoint| {
                        endpoint.id.0 == trap.endpoint_id
                            && endpoint.address.0 == trap.endpoint_address
                    })
                    .cloned()
                    .ok_or_else(|| {
                        LoaderError::invalid_input(
                            "runtime-reentry",
                            "runtime-trap-endpoint-unowned",
                            "generic runtime trap does not identify a process-owned export endpoint",
                        )
                    })?;
                let interlocked_before = if matches!(
                    enabled_capability,
                    Some(
                        GenericRuntimeCapability::InterlockedIncrement
                            | GenericRuntimeCapability::PrivateHeapSecondAllocation
                            | GenericRuntimeCapability::StandardInputHandle
                            | GenericRuntimeCapability::FileType
                            | GenericRuntimeCapability::StandardOutputHandle
                            | GenericRuntimeCapability::StandardOutputFileType
                            | GenericRuntimeCapability::StandardErrorHandle
                            | GenericRuntimeCapability::StandardErrorFileType
                            | GenericRuntimeCapability::SetHandleCount
                    )
                ) && endpoint.matches(
                    "kernel32.dll",
                    &ApiSymbol::Name {
                        name: "InterlockedIncrement".to_owned(),
                    },
                ) {
                    Some(read_generic_runtime_long_argument(
                        memory,
                        trap.stack_pointer,
                    )?)
                } else {
                    None
                };
                let interlocked_decrement_before = if interlocked_decrement_enabled
                    && endpoint.matches(
                        "kernel32.dll",
                        &ApiSymbol::Name {
                            name: "InterlockedDecrement".to_owned(),
                        },
                    ) {
                    Some(read_generic_runtime_long_argument(
                        memory,
                        trap.stack_pointer,
                    )?)
                } else {
                    None
                };
                let heap_free_before = if kernel32_policy.allows_heap_free(0)
                    && endpoint.matches(
                        "kernel32.dll",
                        &ApiSymbol::Name {
                            name: "HeapFree".to_owned(),
                        },
                    ) {
                    generic_runtime_heap_free_before(&process.runtime, memory, trap.stack_pointer)
                        .ok()
                } else {
                    None
                };
                let mut session = ExecutionSession::new(memory, &mut process.runtime);
                match dispatcher.dispatch_owned(
                    ApiOwnedTrap {
                        endpoint_id: endpoint.id,
                        endpoint_address: endpoint.address,
                        stack_pointer: GuestAddress(trap.stack_pointer),
                        return_address: GuestAddress(trap.return_address),
                    },
                    &mut session,
                    |contract, frame, session| {
                        if contract.endpoint.module.eq_ignore_ascii_case("user32.dll") {
                            dispatch_user32_api_with_policy(
                                &user32_policy,
                                callback_return_sentinel,
                                contract,
                                frame,
                                session,
                            )
                        } else if contract.endpoint.module.eq_ignore_ascii_case("gdi32.dll") {
                            dispatch_gdi32_api_with_policy(
                                &gdi32_policy,
                                callback_return_sentinel,
                                contract,
                                frame,
                                session,
                            )
                        } else if contract.endpoint.module.eq_ignore_ascii_case("gdiplus.dll") {
                            dispatch_gdiplus_api_with_policy(
                                &gdiplus_policy,
                                contract,
                                frame,
                                session,
                            )
                        } else if contract.endpoint.module.eq_ignore_ascii_case("imm32.dll") {
                            dispatch_imm32_api_with_policy(&imm32_policy, contract, frame, session)
                        } else if contract.endpoint.module.eq_ignore_ascii_case("ole32.dll") {
                            dispatch_ole32_api(contract, frame, session)
                        } else if contract.endpoint.module.eq_ignore_ascii_case("shell32.dll") {
                            dispatch_shell32_api(contract, frame, session)
                        } else if contract.endpoint.module.eq_ignore_ascii_case("winmm.dll") {
                            dispatch_winmm_api_with_policy(&winmm_policy, contract, frame, session)
                        } else if contract.endpoint.module.eq_ignore_ascii_case("d3d9.dll") {
                            dispatch_d3d9_api(contract, frame, session)
                        } else if contract.endpoint.module.eq_ignore_ascii_case("dinput8.dll") {
                            dispatch_dinput8_api(contract, frame, session)
                        } else if contract.endpoint.module.eq_ignore_ascii_case("dsound.dll") {
                            dispatch_dsound_api(contract, frame, session)
                        } else {
                            dispatch_kernel32_api_with_policy(
                                &kernel32_policy,
                                contract,
                                frame,
                                session,
                            )
                        }
                    },
                ) {
                    DispatchOutcome::Immediate(completion) => {
                        let interlocked_increment = interlocked_before
                            .map(|before| {
                                generic_runtime_long_update(
                                    &completion,
                                    before,
                                    1,
                                    "InterlockedIncrement",
                                )
                            })
                            .transpose()?;
                        let interlocked_decrement = interlocked_decrement_before
                            .map(|before| {
                                generic_runtime_long_update(
                                    &completion,
                                    before,
                                    -1,
                                    "InterlockedDecrement",
                                )
                            })
                            .transpose()?;
                        let heap_allocation = if matches!(
                            enabled_capability,
                            Some(
                                GenericRuntimeCapability::PrivateHeapSecondAllocation
                                    | GenericRuntimeCapability::StandardInputHandle
                                    | GenericRuntimeCapability::FileType
                                    | GenericRuntimeCapability::StandardOutputHandle
                                    | GenericRuntimeCapability::StandardOutputFileType
                                    | GenericRuntimeCapability::StandardErrorHandle
                                    | GenericRuntimeCapability::StandardErrorFileType
                                    | GenericRuntimeCapability::SetHandleCount
                            )
                        ) && endpoint.matches(
                            "kernel32.dll",
                            &ApiSymbol::Name {
                                name: "HeapAlloc".to_owned(),
                            },
                        ) {
                            Some(generic_runtime_heap_allocation(&session, &completion)?)
                        } else {
                            None
                        };
                        let heap_free = heap_free_before
                            .map(|before| generic_runtime_heap_free(&session, &completion, before))
                            .transpose()?;
                        let standard_handle = if matches!(
                            enabled_capability,
                            Some(
                                GenericRuntimeCapability::StandardInputHandle
                                    | GenericRuntimeCapability::FileType
                                    | GenericRuntimeCapability::StandardOutputHandle
                                    | GenericRuntimeCapability::StandardOutputFileType
                                    | GenericRuntimeCapability::StandardErrorHandle
                                    | GenericRuntimeCapability::StandardErrorFileType
                                    | GenericRuntimeCapability::SetHandleCount
                            )
                        ) && endpoint.matches(
                            "kernel32.dll",
                            &ApiSymbol::Name {
                                name: "GetStdHandle".to_owned(),
                            },
                        ) {
                            Some(generic_runtime_standard_handle(&session, &completion)?)
                        } else {
                            None
                        };
                        let file_type = if matches!(
                            enabled_capability,
                            Some(
                                GenericRuntimeCapability::FileType
                                    | GenericRuntimeCapability::StandardOutputHandle
                                    | GenericRuntimeCapability::StandardOutputFileType
                                    | GenericRuntimeCapability::StandardErrorHandle
                                    | GenericRuntimeCapability::StandardErrorFileType
                                    | GenericRuntimeCapability::SetHandleCount
                            )
                        ) && endpoint.matches(
                            "kernel32.dll",
                            &ApiSymbol::Name {
                                name: "GetFileType".to_owned(),
                            },
                        ) {
                            Some(generic_runtime_file_type(&session, &completion)?)
                        } else {
                            None
                        };
                        let current_directory = (completion.implementation_id
                            == "kernel32.set-current-directory-a.v1")
                            .then(|| GenericRuntimeCurrentDirectory {
                                directory: session.runtime().current_directory_a().clone(),
                                generation: session.runtime().current_directory_generation(),
                            });
                        let mutex = if completion.implementation_id == "kernel32.create-mutex-a.v1"
                        {
                            let RegisterUpdate::Set(handle) = completion.eax else {
                                return Err(LoaderError::runtime_defect(
                                    "runtime-reentry",
                                    "create-mutex-handle-result-invalid",
                                    "CreateMutexA completion did not set a logical handle",
                                ));
                            };
                            let handle = RuntimeHandle(handle);
                            Some(GenericRuntimeMutex {
                                handle,
                                object: session
                                    .runtime()
                                    .mutex_for_handle(handle)
                                    .map_err(runtime_state_error)?
                                    .clone(),
                                object_record: session
                                    .runtime()
                                    .object_for_handle(handle, RuntimeObjectKind::Mutex)
                                    .map_err(runtime_state_error)?
                                    .clone(),
                                already_exists: completion.last_error == 183,
                            })
                        } else {
                            None
                        };
                        let waitable_timer_handle = match completion.implementation_id.as_str() {
                            "kernel32.create-waitable-timer-a.v1" => {
                                let RegisterUpdate::Set(handle) = completion.eax else {
                                    return Err(LoaderError::runtime_defect(
                                        "runtime-reentry",
                                        "create-waitable-timer-handle-result-invalid",
                                        "CreateWaitableTimerA completion did not set a logical handle",
                                    ));
                                };
                                (handle != 0).then_some(RuntimeHandle(handle))
                            }
                            vine_core::SET_WAITABLE_TIMER_IMPLEMENTATION => {
                                let RegisterUpdate::Set(result) = completion.eax else {
                                    return Err(LoaderError::runtime_defect(
                                        "runtime-reentry",
                                        "set-waitable-timer-result-invalid",
                                        "SetWaitableTimer completion did not set BOOL in EAX",
                                    ));
                                };
                                if result == 0 {
                                    None
                                } else {
                                    Some(RuntimeHandle(
                                        completion.frame.argument_u32(0).ok_or_else(|| {
                                            LoaderError::runtime_defect(
                                                "runtime-reentry",
                                                "set-waitable-timer-handle-argument-missing",
                                                "SetWaitableTimer completion has no timer handle argument",
                                            )
                                        })?,
                                    ))
                                }
                            }
                            vine_core::WAIT_FOR_SINGLE_OBJECT_EX_IMPLEMENTATION => {
                                let RegisterUpdate::Set(result) = completion.eax else {
                                    return Err(LoaderError::runtime_defect(
                                        "runtime-reentry",
                                        "wait-for-single-object-ex-result-invalid",
                                        "WaitForSingleObjectEx completion did not set a wait result in EAX",
                                    ));
                                };
                                if result == u32::MAX {
                                    None
                                } else {
                                    Some(RuntimeHandle(
                                        completion.frame.argument_u32(0).ok_or_else(|| {
                                            LoaderError::runtime_defect(
                                                "runtime-reentry",
                                                "wait-for-single-object-ex-handle-argument-missing",
                                                "WaitForSingleObjectEx completion has no timer handle argument",
                                            )
                                        })?,
                                    ))
                                }
                            }
                            _ => None,
                        };
                        let waitable_timer = if let Some(handle) = waitable_timer_handle {
                            let wait_completed = completion.implementation_id
                                == vine_core::WAIT_FOR_SINGLE_OBJECT_EX_IMPLEMENTATION;
                            Some(GenericRuntimeWaitableTimer {
                                handle,
                                timer: *session
                                    .runtime()
                                    .waitable_timer_for_handle(handle)
                                    .map_err(runtime_state_error)?,
                                object_record: session
                                    .runtime()
                                    .object_for_handle(handle, RuntimeObjectKind::WaitableTimer)
                                    .map_err(runtime_state_error)?
                                    .clone(),
                                timer_clock: wait_completed
                                    .then(|| session.runtime().timer_clock_state()),
                                next_wait_registration_id: wait_completed
                                    .then(|| session.runtime().next_wait_registration_id()),
                            })
                        } else {
                            None
                        };
                        let icon = if matches!(
                            completion.implementation_id.as_str(),
                            "user32.load-icon-a.v1" | "user32.load-image-a.v1"
                        ) {
                            let RegisterUpdate::Set(handle) = completion.eax else {
                                return Err(LoaderError::runtime_defect(
                                    "runtime-reentry",
                                    "load-icon-handle-result-invalid",
                                    "icon-loading completion did not set a logical HICON",
                                ));
                            };
                            Some(
                                session
                                    .runtime()
                                    .icon(handle)
                                    .map_err(runtime_state_error)?
                                    .clone(),
                            )
                        } else {
                            None
                        };
                        let cursor = if completion.implementation_id == "user32.load-cursor-a.v1" {
                            let RegisterUpdate::Set(handle) = completion.eax else {
                                return Err(LoaderError::runtime_defect(
                                    "runtime-reentry",
                                    "load-cursor-handle-result-invalid",
                                    "LoadCursorA completion did not set a logical HCURSOR",
                                ));
                            };
                            Some(
                                session
                                    .runtime()
                                    .cursor(handle)
                                    .map_err(runtime_state_error)?
                                    .clone(),
                            )
                        } else {
                            None
                        };
                        let window_class =
                            if completion.implementation_id == "user32.register-class-ex-a.v1" {
                                let RegisterUpdate::Set(atom) = completion.eax else {
                                    return Err(LoaderError::runtime_defect(
                                        "runtime-reentry",
                                        "register-class-atom-result-invalid",
                                        "RegisterClassExA completion did not set a logical ATOM",
                                    ));
                                };
                                let atom = u16::try_from(atom).map_err(|_| {
                                    LoaderError::runtime_defect(
                                        "runtime-reentry",
                                        "register-class-atom-result-invalid",
                                        "RegisterClassExA completion returned an out-of-range ATOM",
                                    )
                                })?;
                                Some(
                                    session
                                        .runtime()
                                        .window_class(atom)
                                        .map_err(runtime_state_error)?
                                        .clone(),
                                )
                            } else {
                                None
                            };
                        let window = if matches!(
                            completion.implementation_id.as_str(),
                            SET_WINDOW_LONG_A_IMPLEMENTATION
                                | vine_core::DRAG_ACCEPT_FILES_IMPLEMENTATION
                                | vine_core::SET_FOCUS_IMPLEMENTATION
                        ) {
                            let handle = completion.frame.argument_u32(0).ok_or_else(|| {
                                LoaderError::runtime_defect(
                                    "runtime-reentry",
                                    "window-state-completion-argument-missing",
                                    "window-state completion has no logical HWND argument",
                                )
                            })?;
                            Some(
                                session
                                    .runtime()
                                    .window(handle)
                                    .map_err(runtime_state_error)?
                                    .clone(),
                            )
                        } else {
                            None
                        };
                        let window_timer = if completion.implementation_id
                            == SET_TIMER_IMPLEMENTATION
                        {
                            let window_handle =
                                completion.frame.argument_u32(0).ok_or_else(|| {
                                    LoaderError::runtime_defect(
                                        "runtime-reentry",
                                        "set-timer-window-argument-missing",
                                        "SetTimer completion has no logical HWND argument",
                                    )
                                })?;
                            let timer_id = completion.frame.argument_u32(1).ok_or_else(|| {
                                LoaderError::runtime_defect(
                                    "runtime-reentry",
                                    "set-timer-id-argument-missing",
                                    "SetTimer completion has no timer identity argument",
                                )
                            })?;
                            Some(GenericRuntimeWindowTimer::snapshot(
                                *session
                                    .runtime()
                                    .window_timers()
                                    .values()
                                    .find(|timer| {
                                        timer.window_handle == window_handle
                                            && timer.timer_id == timer_id
                                    })
                                    .ok_or_else(|| {
                                        LoaderError::runtime_defect(
                                            "runtime-reentry",
                                            "set-timer-state-missing-after-dispatch",
                                            "SetTimer completed without a matching logical window timer",
                                        )
                                    })?,
                                window_timer_delivery_enabled,
                            ))
                        } else if window_timer_delivery_enabled
                            && completion.implementation_id == PEEK_MESSAGE_A_IMPLEMENTATION
                            && matches!(completion.eax, RegisterUpdate::Set(value) if value != 0)
                            && completion.guest_writes.iter().any(|write| {
                                write.write.bytes.get(4..8).is_some_and(|bytes| {
                                    u32::from_le_bytes(
                                        bytes.try_into().expect("checked MSG field width"),
                                    ) == 0x0113
                                })
                            })
                        {
                            Some(GenericRuntimeWindowTimer::snapshot(
                                *session
                                    .runtime()
                                    .window_timers()
                                    .values()
                                    .filter(|timer| timer.last_delivery.is_some())
                                    .max_by_key(|timer| {
                                        timer
                                            .last_delivery
                                            .map(|delivery| delivery.delivered_at_time_100ns)
                                    })
                                    .ok_or_else(|| {
                                        LoaderError::runtime_defect(
                                            "runtime-reentry",
                                            "peek-message-window-timer-state-missing-after-dispatch",
                                            "nonempty timer-enabled PeekMessageA completed without delivered window timer state",
                                        )
                                    })?,
                                true,
                            ))
                        } else {
                            None
                        };
                        let queued_thread_messages = if def_window_proc_pointer_down_enabled {
                            let release_message = user32_policy
                                .pointer_button_messages
                                .as_ref()
                                .ok_or_else(|| {
                                    LoaderError::runtime_defect(
                                        "runtime-reentry",
                                        "pointer-button-message-policy-missing",
                                        "DefWindowProcA continuation requires the User32 pointer-button message profile",
                                    )
                                })?
                                .left_button_up_message;
                            let release_peek =
                                generic_runtime_successful_peek_message_output(&completion)?
                                    == Some(release_message);
                            let release_translate = generic_runtime_translate_message_input(
                                &completion,
                                session.memory(),
                            )? == Some(release_message);
                            let empty_peek = completion.implementation_id
                                == PEEK_MESSAGE_A_IMPLEMENTATION
                                && completion.eax == RegisterUpdate::Set(0)
                                && completion.guest_writes.is_empty();
                            if release_peek || release_translate || empty_peek {
                                Some(
                                    session
                                        .runtime()
                                        .queued_thread_messages(
                                            session.runtime().current_thread_id(),
                                        )
                                        .map_err(runtime_state_error)?,
                                )
                            } else {
                                None
                            }
                        } else {
                            None
                        };
                        let thread_affinity = if completion.implementation_id
                            == "kernel32.set-thread-affinity-mask.v1"
                        {
                            let process_mask = kernel32_policy
                                .thread_affinity_process_mask()
                                .ok_or_else(|| {
                                    LoaderError::runtime_defect(
                                        "runtime-reentry",
                                        "thread-affinity-policy-missing-after-dispatch",
                                        "completed SetThreadAffinityMask has no virtual process mask",
                                    )
                                })?;
                            Some(GenericRuntimeThreadAffinity {
                                thread_id: session.runtime().current_thread_id().0,
                                process_mask,
                                affinity_mask: session
                                    .runtime()
                                    .current_thread_affinity_mask(process_mask)
                                    .map_err(runtime_state_error)?,
                            })
                        } else {
                            None
                        };
                        let thread_priority = if completion.implementation_id
                            == SET_THREAD_PRIORITY_IMPLEMENTATION
                        {
                            let handle = RuntimeHandle(
                                completion.frame.argument_u32(0).ok_or_else(|| {
                                    LoaderError::runtime_defect(
                                        "runtime-reentry",
                                        "set-thread-priority-handle-argument-missing",
                                        "SetThreadPriority completion has no thread handle argument",
                                    )
                                })?,
                            );
                            let object = session
                                .runtime()
                                .object_for_handle(handle, RuntimeObjectKind::Thread)
                                .map_err(runtime_state_error)?;
                            let RuntimeObjectKey::Thread { thread_id } = &object.key else {
                                return Err(LoaderError::runtime_defect(
                                    "runtime-reentry",
                                    "set-thread-priority-object-identity-invalid",
                                    "SetThreadPriority completion resolved to a non-thread object",
                                ));
                            };
                            let thread_id = RuntimeThreadId(*thread_id);
                            Some(GenericRuntimeThreadPriority {
                                handle,
                                thread_id,
                                priority: session
                                    .runtime()
                                    .thread_priority(thread_id)
                                    .map_err(runtime_state_error)?,
                            })
                        } else {
                            None
                        };
                        let created_thread = if completion.implementation_id
                            == CREATE_THREAD_IMPLEMENTATION
                        {
                            let handle = RuntimeHandle(set_eax_result(
                                &completion,
                                "create-thread-return-register-invalid",
                                "CreateThread dispatcher did not set EAX",
                            )?);
                            if handle.0 == 0 {
                                None
                            } else {
                                let object = session
                                    .runtime()
                                    .object_for_handle(handle, RuntimeObjectKind::Thread)
                                    .map_err(runtime_state_error)?
                                    .clone();
                                let RuntimeObjectKey::Thread { thread_id } = &object.key else {
                                    return Err(LoaderError::runtime_defect(
                                        "runtime-reentry",
                                        "create-thread-object-identity-invalid",
                                        "CreateThread completion resolved to a non-thread object",
                                    ));
                                };
                                let thread_id = RuntimeThreadId(*thread_id);
                                Some(GenericRuntimeCreatedThread {
                                    handle,
                                    object,
                                    thread: session
                                        .runtime()
                                        .thread_state(thread_id)
                                        .map_err(runtime_state_error)?
                                        .clone(),
                                    scheduler_state: session
                                        .runtime()
                                        .scheduler_state_for_thread(thread_id)
                                        .map_err(runtime_state_error)?,
                                })
                            }
                        } else {
                            None
                        };
                        let resumed_thread = if completion.implementation_id
                            == RESUME_THREAD_IMPLEMENTATION
                        {
                            let previous_suspend_count = set_eax_result(
                                &completion,
                                "resume-thread-return-register-invalid",
                                "ResumeThread dispatcher did not set EAX",
                            )?;
                            if previous_suspend_count == u32::MAX {
                                None
                            } else {
                                let handle = RuntimeHandle(
                                    completion.frame.argument_u32(0).ok_or_else(|| {
                                        LoaderError::runtime_defect(
                                            "runtime-reentry",
                                            "resume-thread-handle-argument-missing",
                                            "ResumeThread completion has no thread handle argument",
                                        )
                                    })?,
                                );
                                let object = session
                                    .runtime()
                                    .object_for_handle(handle, RuntimeObjectKind::Thread)
                                    .map_err(runtime_state_error)?;
                                let RuntimeObjectKey::Thread { thread_id } = &object.key else {
                                    return Err(LoaderError::runtime_defect(
                                        "runtime-reentry",
                                        "resume-thread-object-identity-invalid",
                                        "ResumeThread completion resolved to a non-thread object",
                                    ));
                                };
                                let thread_id = RuntimeThreadId(*thread_id);
                                Some(GenericRuntimeResumedThread {
                                    handle,
                                    thread_id,
                                    previous_suspend_count,
                                    scheduler_state: session
                                        .runtime()
                                        .scheduler_state_for_thread(thread_id)
                                        .map_err(runtime_state_error)?,
                                })
                            }
                        } else {
                            None
                        };
                        let api_read_clock = session.runtime().clock_profile().progression
                            == vine_core::RuntimeClockProgression::ApiReadIncrement;
                        let tick_count = if !api_read_clock {
                            None
                        } else if completion.implementation_id == "kernel32.get-tick-count.v1" {
                            kernel32_policy
                                .tick_count_increment_per_read()
                                .map(|increment_per_read| {
                                    let returned_tick_count = set_eax_result(
                                        &completion,
                                        "tick-count-return-register-invalid",
                                        "GetTickCount dispatcher did not set EAX",
                                    )?;
                                    Ok(GenericRuntimeTickCount {
                                        returned_tick_count,
                                        next_tick_count: session.runtime().clock().tick_count,
                                        increment_per_read,
                                    })
                                })
                                .transpose()?
                        } else if completion.implementation_id == TIME_GET_TIME_IMPLEMENTATION {
                            let increment_per_read = winmm_policy
                                .time_get_time
                                .as_ref()
                                .ok_or_else(|| {
                                    LoaderError::runtime_defect(
                                        "runtime-reentry",
                                        "time-get-time-policy-missing-after-dispatch",
                                        "completed timeGetTime has no multimedia clock policy",
                                    )
                                })?
                                .increment_per_read;
                            let returned_tick_count = set_eax_result(
                                &completion,
                                "time-get-time-return-register-invalid",
                                "timeGetTime dispatcher did not set EAX",
                            )?;
                            Some(GenericRuntimeTickCount {
                                returned_tick_count,
                                next_tick_count: session.runtime().clock().tick_count,
                                increment_per_read,
                            })
                        } else {
                            None
                        };
                        let com_apartment =
                            if completion.implementation_id == "ole32.co-initialize.v1" {
                                session
                                    .runtime()
                                    .current_com_apartment()
                                    .map_err(runtime_state_error)?
                                    .copied()
                            } else {
                                None
                            };
                        let timer_resolution = matches!(
                            completion.implementation_id.as_str(),
                            TIME_BEGIN_PERIOD_IMPLEMENTATION | TIME_END_PERIOD_IMPLEMENTATION
                        )
                        .then(|| session.runtime().timer_resolution_state())
                        .flatten();
                        let runtime_module = if completion.implementation_id
                            == LOAD_LIBRARY_A_IMPLEMENTATION
                        {
                            let handle = set_eax_result(
                                &completion,
                                "load-library-a-return-register-invalid",
                                "LoadLibraryA dispatcher did not set EAX",
                            )?;
                            let module = session
                                .runtime()
                                .module(handle)
                                .map_err(runtime_state_error)?
                                .clone();
                            let graph_node = session
                                .runtime()
                                .module_graph()
                                .module_by_handle(handle)
                                .map_err(|error| {
                                    LoaderError::runtime_defect(
                                        "runtime-reentry",
                                        error.code,
                                        error.message,
                                    )
                                })?
                                .clone();
                            let object_record = session
                                .runtime()
                                .object_for_handle(RuntimeHandle(handle), RuntimeObjectKind::Module)
                                .map_err(runtime_state_error)?
                                .clone();
                            Some(GenericRuntimeModuleLoad {
                                module,
                                graph_node,
                                object_record,
                            })
                        } else {
                            None
                        };
                        let gdi = generic_runtime_gdi_completion(
                            &completion,
                            session.runtime(),
                            &gdi32_policy,
                        )?;
                        let paint =
                            generic_runtime_paint_completion(&completion, session.runtime())?;
                        let gdiplus = generic_runtime_gdiplus_completion(
                            &completion,
                            session.runtime(),
                            &gdiplus_policy,
                        )?;
                        let imm32 = generic_runtime_imm32_completion(
                            &completion,
                            session.runtime(),
                            &imm32_policy,
                            &user32_policy,
                        )?;
                        let virtual_memory = generic_runtime_virtual_memory_completion(
                            &completion,
                            session.runtime(),
                            session.memory(),
                        )?;
                        let thread_selection = session
                            .runtime_mut()
                            .complete_api_execution_boundary_with_disposition(
                                completion.execution_boundary,
                            )
                            .map_err(runtime_state_error)?;
                        send_pending_runtime_delta(input, memory)?;
                        write_runtime_json_frame(
                            RUNTIME_FRAME_API_COMPLETION,
                            &GenericRuntimeCompletion {
                                schema: "vine.v86-generic-runtime-completion",
                                schema_version: 1,
                                endpoint: GenericRuntimeEndpoint::RuntimeSystemExport { endpoint },
                                completion,
                                fls_value: None,
                                interlocked_increment,
                                interlocked_decrement,
                                heap_allocation,
                                heap_free,
                                standard_handle,
                                file_type,
                                current_directory,
                                mutex,
                                waitable_timer,
                                icon,
                                cursor,
                                window_class,
                                window,
                                paint,
                                window_timer,
                                queued_thread_messages,
                                thread_affinity,
                                thread_priority,
                                imm32,
                                virtual_memory,
                                created_thread,
                                resumed_thread,
                                tick_count,
                                com_apartment,
                                timer_resolution,
                                source_read: None,
                                writable_commit: None,
                                writable_directory_create: None,
                                runtime_module,
                                gdi,
                                gdiplus,
                                thread_selection,
                                memory: memory.stats(),
                            },
                        )?;
                    }
                    DispatchOutcome::Fault(fault) => {
                        write_generic_runtime_fault(
                            memory,
                            GenericRuntimeEndpoint::RuntimeSystemExport { endpoint },
                            fault,
                        )?;
                        return Ok(());
                    }
                    DispatchOutcome::GuestCallback(call) => {
                        if active_callbacks.len() >= GENERIC_RUNTIME_CALLBACK_DEPTH_LIMIT {
                            write_generic_runtime_fault(
                                memory,
                                GenericRuntimeEndpoint::RuntimeSystemExport { endpoint },
                                ApiFault::resource_limit(
                                    "runtime-reentry",
                                    "runtime-api-callback-depth-limit-exceeded",
                                    format!(
                                        "generic BGI re-entry callback depth exceeds {GENERIC_RUNTIME_CALLBACK_DEPTH_LIMIT}"
                                    ),
                                ),
                            )?;
                            return Ok(());
                        }
                        let prepared = match session.begin_guest_callback(&call) {
                            Ok(prepared) => prepared,
                            Err(fault) => {
                                write_generic_runtime_fault(
                                    memory,
                                    GenericRuntimeEndpoint::RuntimeSystemExport { endpoint },
                                    fault,
                                )?;
                                return Ok(());
                            }
                        };
                        let queued_thread_messages = session
                            .runtime()
                            .queued_thread_messages(session.runtime().current_thread_id())
                            .map_err(runtime_state_error)?;
                        send_pending_runtime_delta(input, memory)?;
                        let callback_entry = GenericRuntimeCallbackEntry {
                            schema: "vine.v86-generic-runtime-callback",
                            schema_version: 1,
                            endpoint: GenericRuntimeEndpoint::RuntimeSystemExport {
                                endpoint: endpoint.clone(),
                            },
                            frame_id: prepared.frame_id.0,
                            target: call.request.target.0,
                            argument_count: call.request.arguments.len(),
                            api_argument_count: call.frame.arguments.len(),
                            allocation_start: prepared.allocation_start.0,
                            stack_pointer: prepared.stack_pointer.0,
                            scratch_address: prepared.scratch_address.0,
                            scratch_bytes: prepared.scratch_bytes,
                            frame_bytes: prepared.frame_bytes,
                            return_sentinel: call.request.return_sentinel.0,
                            queued_thread_messages,
                            memory: memory.stats(),
                        };
                        active_callbacks.push(GenericRuntimeActiveCallback {
                            call,
                            frame_id: prepared.frame_id,
                        });
                        write_runtime_json_frame(RUNTIME_FRAME_GUEST_CALLBACK, &callback_entry)?;
                    }
                    DispatchOutcome::Deferred(call) => {
                        let request_id = call.request.request_id;
                        let thread_id = session.runtime().current_thread_id();
                        let scheduler_state = session
                            .runtime()
                            .scheduler_state()
                            .map_err(runtime_state_error)?;
                        if scheduler_state != (SchedulerThreadState::WaitingHost { request_id }) {
                            return Err(LoaderError::runtime_defect(
                                "runtime-reentry",
                                "storage-request-scheduler-state-invalid",
                                "deferred storage request is not owned by the waiting runtime thread",
                            ));
                        }
                        let arguments = decode_runtime_host_payload(&call.request.payload)?;
                        let host_payload =
                            if call.request.service == RUNTIME_WRITABLE_COMMIT_SERVICE {
                                session
                                    .runtime()
                                    .writable_file_commit_bytes(request_id)
                                    .map_err(runtime_state_error)?
                                    .to_vec()
                            } else {
                                Vec::new()
                            };
                        write_runtime_json_payload_frame(
                            RUNTIME_FRAME_HOST_REQUEST,
                            &GenericRuntimeHostRequest {
                                schema: "vine.v86-generic-runtime-host-request",
                                schema_version: 1,
                                endpoint: GenericRuntimeEndpoint::RuntimeSystemExport {
                                    endpoint: endpoint.clone(),
                                },
                                request_id,
                                thread_id: thread_id.0,
                                service: call.request.service.clone(),
                                arguments,
                                scheduler_state: GenericRuntimeWaitingHostState {
                                    state: "waiting-host",
                                    request_id,
                                },
                                memory: session.memory().stats(),
                            },
                            &host_payload,
                        )?;
                        let response_frame = read_runtime_frame(input)?;
                        require_runtime_frame_kind(&response_frame, RUNTIME_FRAME_HOST_COMPLETION)?;
                        let timer_pacing =
                            call.request.service == RUNTIME_WAITABLE_TIMER_PACING_SERVICE;
                        let expected_control_bytes = if timer_pacing { 12 } else { 40 };
                        if response_frame.control.len() != expected_control_bytes {
                            return Err(LoaderError::invalid_input(
                                "runtime-reentry",
                                "host-completion-control-size-invalid",
                                format!(
                                    "host completion for {} requires {expected_control_bytes} control bytes",
                                    call.request.service
                                ),
                            ));
                        }
                        let response_request_id = u64::from_le_bytes(
                            response_frame.control[..8]
                                .try_into()
                                .expect("checked source response request ID"),
                        );
                        if response_request_id != request_id {
                            return Err(LoaderError::invalid_input(
                                "runtime-reentry",
                                "storage-completion-request-mismatch",
                                "storage completion identifies a different pending request",
                            ));
                        }
                        let (
                            outcome,
                            source_read,
                            writable_commit,
                            writable_directory_create,
                            waitable_timer_pacing,
                        ) = if timer_pacing {
                            if !response_frame.payload.is_empty() {
                                return Err(LoaderError::invalid_input(
                                    "runtime-reentry",
                                    "timer-pacing-completion-payload-invalid",
                                    "timer pacing completion must not contain a payload",
                                ));
                            }
                            let acknowledgement = u32::from_le_bytes(
                                response_frame.control[8..12]
                                    .try_into()
                                    .expect("checked timer pacing acknowledgement"),
                            );
                            let (outcome, completion) = complete_kernel32_waitable_timer_pacing(
                                &dispatcher,
                                call,
                                acknowledgement,
                                &mut session,
                            );
                            (outcome, None, None, None, completion)
                        } else if call.request.service == RUNTIME_WRITABLE_COMMIT_SERVICE {
                            if !response_frame.payload.is_empty() {
                                return Err(LoaderError::invalid_input(
                                    "runtime-reentry",
                                    "writable-commit-completion-payload-invalid",
                                    "writable commit completion must not contain a payload",
                                ));
                            }
                            let response = RuntimeWritableCommitHostResponse::decode_binary(
                                request_id,
                                &response_frame.control[8..],
                            )
                            .map_err(runtime_state_error)?;
                            let (outcome, completion) = complete_kernel32_writable_commit(
                                &dispatcher,
                                call,
                                response,
                                &mut session,
                            );
                            (outcome, None, completion, None, None)
                        } else if call.request.service == RUNTIME_WRITABLE_DIRECTORY_CREATE_SERVICE
                        {
                            if !response_frame.payload.is_empty() {
                                return Err(LoaderError::invalid_input(
                                    "runtime-reentry",
                                    "writable-directory-create-completion-payload-invalid",
                                    "writable directory create completion must not contain a payload",
                                ));
                            }
                            let response =
                                RuntimeWritableDirectoryCreateHostResponse::decode_binary(
                                    request_id,
                                    &response_frame.control[8..],
                                )
                                .map_err(runtime_state_error)?;
                            let (outcome, completion) = complete_kernel32_writable_directory_create(
                                &dispatcher,
                                call,
                                response,
                                &mut session,
                            );
                            (outcome, None, None, completion, None)
                        } else {
                            let response = RuntimeSourceReadHostResponse::decode_binary(
                                request_id,
                                &response_frame.control[8..],
                                &response_frame.payload,
                            )
                            .map_err(runtime_state_error)?;
                            let (outcome, completion) = complete_kernel32_source_read(
                                &dispatcher,
                                call,
                                response,
                                &mut session,
                            );
                            (outcome, completion, None, None, None)
                        };
                        let completion = match outcome {
                            DispatchOutcome::Immediate(completion) => completion,
                            DispatchOutcome::Fault(fault) => {
                                write_generic_runtime_fault(
                                    memory,
                                    GenericRuntimeEndpoint::RuntimeSystemExport { endpoint },
                                    fault,
                                )?;
                                return Ok(());
                            }
                            _ => {
                                return Err(LoaderError::runtime_defect(
                                    "runtime-reentry",
                                    "storage-completion-suspended",
                                    "storage completion produced another suspended outcome",
                                ));
                            }
                        };
                        if [
                            source_read.is_some(),
                            writable_commit.is_some(),
                            writable_directory_create.is_some(),
                            waitable_timer_pacing.is_some(),
                        ]
                        .into_iter()
                        .filter(|present| *present)
                        .count()
                            != 1
                        {
                            return Err(LoaderError::runtime_defect(
                                "runtime-reentry",
                                "host-completion-state-missing",
                                "host API completion did not commit exactly one owner state",
                            ));
                        }
                        if source_read
                            .as_ref()
                            .is_some_and(|completion| completion.request.request_id != request_id)
                            || writable_commit.as_ref().is_some_and(|completion| {
                                completion.request.request_id != request_id
                            })
                            || writable_directory_create
                                .as_ref()
                                .is_some_and(|completion| {
                                    completion.request.request_id != request_id
                                })
                            || waitable_timer_pacing.as_ref().is_some_and(|completion| {
                                completion
                                    .registration
                                    .and_then(|registration| registration.waitable_timer_pacing)
                                    .is_none_or(|pacing| pacing.request_id != request_id)
                            })
                        {
                            return Err(LoaderError::runtime_defect(
                                "runtime-reentry",
                                "host-completion-identity-mismatch",
                                "host completion owner state identifies another request",
                            ));
                        }
                        match session.runtime_mut().pop_scheduler_event() {
                            Some(RuntimeSchedulerEvent::HostCompleted(completion))
                                if completion.request_id == request_id
                                    && completion.thread_id == thread_id => {}
                            _ => {
                                return Err(LoaderError::runtime_defect(
                                    "runtime-reentry",
                                    "host-scheduler-event-mismatch",
                                    "host completion emitted a mismatched scheduler event",
                                ));
                            }
                        }
                        let waitable_timer = if waitable_timer_pacing.is_some() {
                            let handle = RuntimeHandle(
                                completion.frame.argument_u32(0).ok_or_else(|| {
                                    LoaderError::runtime_defect(
                                        "runtime-reentry",
                                        "wait-for-single-object-ex-handle-argument-missing",
                                        "paced WaitForSingleObjectEx completion has no timer handle",
                                    )
                                })?,
                            );
                            Some(GenericRuntimeWaitableTimer {
                                handle,
                                timer: *session
                                    .runtime()
                                    .waitable_timer_for_handle(handle)
                                    .map_err(runtime_state_error)?,
                                object_record: session
                                    .runtime()
                                    .object_for_handle(handle, RuntimeObjectKind::WaitableTimer)
                                    .map_err(runtime_state_error)?
                                    .clone(),
                                timer_clock: Some(session.runtime().timer_clock_state()),
                                next_wait_registration_id: Some(
                                    session.runtime().next_wait_registration_id(),
                                ),
                            })
                        } else {
                            None
                        };
                        send_pending_runtime_delta(input, memory)?;
                        write_runtime_json_frame(
                            RUNTIME_FRAME_API_COMPLETION,
                            &GenericRuntimeCompletion {
                                schema: "vine.v86-generic-runtime-completion",
                                schema_version: 1,
                                endpoint: GenericRuntimeEndpoint::RuntimeSystemExport { endpoint },
                                completion,
                                fls_value: None,
                                interlocked_increment: None,
                                interlocked_decrement: None,
                                heap_allocation: None,
                                heap_free: None,
                                standard_handle: None,
                                file_type: None,
                                current_directory: None,
                                mutex: None,
                                waitable_timer,
                                icon: None,
                                cursor: None,
                                window_class: None,
                                window: None,
                                paint: None,
                                window_timer: None,
                                queued_thread_messages: None,
                                thread_affinity: None,
                                thread_priority: None,
                                imm32: None,
                                virtual_memory: None,
                                created_thread: None,
                                resumed_thread: None,
                                tick_count: None,
                                com_apartment: None,
                                timer_resolution: None,
                                source_read,
                                writable_commit,
                                writable_directory_create,
                                runtime_module: None,
                                gdi: None,
                                gdiplus: None,
                                thread_selection: None,
                                memory: memory.stats(),
                            },
                        )?;
                    }
                }
            }
            GenericRuntimeTrapKind::DynamicExport => {
                let endpoint = process
                    .runtime
                    .dynamic_endpoint(trap.endpoint_id)
                    .map_err(runtime_state_error)?
                    .clone();
                if endpoint.address.0 != trap.endpoint_address {
                    return Err(LoaderError::invalid_input(
                        "runtime-reentry",
                        "dynamic-runtime-trap-identity-mismatch",
                        "dynamic runtime trap ID and address identify different endpoints",
                    ));
                }
                if !runtime_capability_enables_dynamic_endpoint(enabled_capability, &endpoint)
                    && !(fls_get_value_enabled && endpoint.name == "FlsGetValue")
                {
                    write_generic_runtime_fault(
                        memory,
                        GenericRuntimeEndpoint::DynamicExport { endpoint },
                        ApiFault::unsupported(
                            "api-dispatch",
                            "dynamic-export-contract-missing",
                            "the process-owned dynamic export endpoint has no callable API contract",
                        ),
                    )?;
                    return Ok(());
                }
                let endpoint_identity = ApiEndpointIdentity::from_dynamic_endpoint(&endpoint);
                let mut session = ExecutionSession::new(memory, &mut process.runtime);
                match dispatcher.dispatch_owned(
                    ApiOwnedTrap {
                        endpoint_id: endpoint_identity.id,
                        endpoint_address: endpoint_identity.address,
                        stack_pointer: GuestAddress(trap.stack_pointer),
                        return_address: GuestAddress(trap.return_address),
                    },
                    &mut session,
                    |contract, frame, session| {
                        dispatch_kernel32_api_with_policy(
                            &kernel32_policy,
                            contract,
                            frame,
                            session,
                        )
                    },
                ) {
                    DispatchOutcome::Immediate(completion) => {
                        let fls_value = if endpoint.name == "FlsSetValue" {
                            let index = completion.frame.argument_u32(0).ok_or_else(|| {
                                LoaderError::runtime_defect(
                                    "runtime-reentry",
                                    "fls-set-value-index-missing",
                                    "completed FlsSetValue frame has no logical index argument",
                                )
                            })?;
                            Some(GenericRuntimeFlsValue {
                                thread_id: session.runtime().current_thread_id().0,
                                index,
                                value: session
                                    .runtime()
                                    .fls_value(index)
                                    .map_err(runtime_state_error)?,
                            })
                        } else {
                            None
                        };
                        let thread_selection = session
                            .runtime_mut()
                            .complete_api_execution_boundary_with_disposition(
                                completion.execution_boundary,
                            )
                            .map_err(runtime_state_error)?;
                        send_pending_runtime_delta(input, memory)?;
                        write_runtime_json_frame(
                            RUNTIME_FRAME_API_COMPLETION,
                            &GenericRuntimeCompletion {
                                schema: "vine.v86-generic-runtime-completion",
                                schema_version: 1,
                                endpoint: GenericRuntimeEndpoint::DynamicExport { endpoint },
                                completion,
                                fls_value,
                                interlocked_increment: None,
                                interlocked_decrement: None,
                                heap_allocation: None,
                                heap_free: None,
                                standard_handle: None,
                                file_type: None,
                                current_directory: None,
                                mutex: None,
                                waitable_timer: None,
                                icon: None,
                                cursor: None,
                                window_class: None,
                                window: None,
                                paint: None,
                                window_timer: None,
                                queued_thread_messages: None,
                                thread_affinity: None,
                                thread_priority: None,
                                imm32: None,
                                virtual_memory: None,
                                created_thread: None,
                                resumed_thread: None,
                                tick_count: None,
                                com_apartment: None,
                                timer_resolution: None,
                                source_read: None,
                                writable_commit: None,
                                writable_directory_create: None,
                                runtime_module: None,
                                gdi: None,
                                gdiplus: None,
                                thread_selection,
                                memory: memory.stats(),
                            },
                        )?;
                    }
                    DispatchOutcome::Fault(fault) => {
                        write_generic_runtime_fault(
                            memory,
                            GenericRuntimeEndpoint::DynamicExport { endpoint },
                            fault,
                        )?;
                        return Ok(());
                    }
                    DispatchOutcome::Deferred(_) | DispatchOutcome::GuestCallback(_) => {
                        write_generic_runtime_fault(
                            memory,
                            GenericRuntimeEndpoint::DynamicExport { endpoint },
                            ApiFault::unsupported(
                                "runtime-reentry",
                                "runtime-api-outcome-unimplemented",
                                "generic BGI re-entry does not yet resume deferred or callback API outcomes",
                            ),
                        )?;
                        return Ok(());
                    }
                }
            }
            GenericRuntimeTrapKind::ComMethod => {
                if !matches!(
                    requested_capability,
                    Some(
                        GenericRuntimeCapability::D3d9RootFactoryAndIdentity
                            | GenericRuntimeCapability::D3d9GetAdapterDisplayMode
                            | GenericRuntimeCapability::D3d9GetAdapterIdentifier
                            | GenericRuntimeCapability::D3d9GetDeviceCaps
                            | GenericRuntimeCapability::D3d9CheckDeviceType
                            | GenericRuntimeCapability::D3d9CheckDeviceFormat
                            | GenericRuntimeCapability::LoadLibraryA
                            | GenericRuntimeCapability::GetClientRect
                            | GenericRuntimeCapability::WindowDcFill
                            | GenericRuntimeCapability::SetWindowLongA
                            | GenericRuntimeCapability::SetWindowPosActivation
                            | GenericRuntimeCapability::MonitorFromWindow
                            | GenericRuntimeCapability::D3d9GetAdapterCount
                            | GenericRuntimeCapability::D3d9GetAdapterMonitor
                            | GenericRuntimeCapability::D3d9CreateDevice
                            | GenericRuntimeCapability::D3d9CreateTexture
                            | GenericRuntimeCapability::D3d9GetSurfaceLevel
                            | GenericRuntimeCapability::D3d9SurfaceGetDc
                            | GenericRuntimeCapability::TimeGetTime
                            | GenericRuntimeCapability::D3d9InitialTextures
                            | GenericRuntimeCapability::D3d9InitialVertexBuffer
                            | GenericRuntimeCapability::D3d9InitialRenderStates
                            | GenericRuntimeCapability::D3d9RasterStatus
                            | GenericRuntimeCapability::QueryPerformanceFrequency
                            | GenericRuntimeCapability::GetMonitorInfoA
                    )
                ) || trap.endpoint_id != 0
                {
                    return Err(LoaderError::invalid_input(
                        "runtime-reentry",
                        "com-runtime-trap-capability-invalid",
                        "generic COM traps require the active D3D9 root capability and endpoint ID zero",
                    ));
                }
                let endpoint_address = GuestAddress(trap.endpoint_address);
                let target = process
                    .runtime
                    .resolve_com_method(endpoint_address)
                    .map_err(runtime_state_error)?;
                let endpoint = GenericRuntimeEndpoint::ComMethod {
                    target: target.clone(),
                };
                let outcome = {
                    let mut session = ExecutionSession::new(memory, &mut process.runtime);
                    let trap = ComCallTrap {
                        endpoint_address,
                        stack_pointer: GuestAddress(trap.stack_pointer),
                        return_address: GuestAddress(trap.return_address),
                        call_site: GuestAddress(trap.return_address),
                    };
                    let d3d9_frontend_enabled = match target.method.slot {
                        3 => d3d9_policy.test_cooperative_level.is_some(),
                        4 => d3d9_policy.get_adapter_count.is_some(),
                        5 => d3d9_policy.get_adapter_identifier.is_some(),
                        8 => d3d9_policy.get_adapter_display_mode.is_some(),
                        9 => d3d9_policy.check_device_type.is_some(),
                        10 => d3d9_policy.check_device_format.is_some(),
                        14 => d3d9_policy.get_device_caps.is_some(),
                        15 => d3d9_policy.get_adapter_monitor.is_some(),
                        16 => d3d9_policy.create_device.is_some(),
                        17 => d3d9_present_enabled && d3d9_policy.present.is_some(),
                        18 => d3d9_policy.get_surface_level.is_some(),
                        19 => d3d9_policy.raster_status.is_some(),
                        21 => d3d9_add_dirty_rect_enabled && d3d9_policy.add_dirty_rect.is_some(),
                        31 => d3d9_update_texture_enabled && d3d9_policy.update_texture.is_some(),
                        41 => d3d9_begin_scene_enabled && d3d9_policy.begin_scene.is_some(),
                        42 => d3d9_end_scene_enabled && d3d9_policy.end_scene.is_some(),
                        23 => d3d9_policy.create_texture.is_some(),
                        26 | 11 | 12 => d3d9_policy.initial_vertex_buffer.is_some(),
                        43 => d3d9_clear_enabled && d3d9_policy.clear.is_some(),
                        57 => d3d9_policy.initial_render_states.is_some(),
                        69 => {
                            d3d9_initial_sampler_states_enabled
                                && d3d9_policy.initial_sampler_states.is_some()
                        }
                        81 => d3d9_draw_primitive_enabled && d3d9_policy.draw_primitive.is_some(),
                        67 => {
                            d3d9_initial_texture_stage_states_enabled
                                && d3d9_policy.initial_texture_stage_states.is_some()
                        }
                        65 => d3d9_set_texture_enabled && d3d9_policy.texture_binding.is_some(),
                        89 => d3d9_set_fvf_enabled && d3d9_policy.flexible_vertex_format.is_some(),
                        100 => {
                            d3d9_set_stream_source_enabled && d3d9_policy.stream_source.is_some()
                        }
                        _ => true,
                    };
                    if target.object.class_identity.starts_with("d3d9.")
                        && d3d9_frontend_enabled
                        && matches!(
                            requested_capability,
                            Some(
                                GenericRuntimeCapability::D3d9GetAdapterDisplayMode
                                    | GenericRuntimeCapability::D3d9GetAdapterIdentifier
                                    | GenericRuntimeCapability::D3d9GetDeviceCaps
                                    | GenericRuntimeCapability::D3d9CheckDeviceType
                                    | GenericRuntimeCapability::D3d9CheckDeviceFormat
                                    | GenericRuntimeCapability::LoadLibraryA
                                    | GenericRuntimeCapability::GetClientRect
                                    | GenericRuntimeCapability::WindowDcFill
                                    | GenericRuntimeCapability::SetWindowLongA
                                    | GenericRuntimeCapability::SetWindowPosActivation
                                    | GenericRuntimeCapability::MonitorFromWindow
                                    | GenericRuntimeCapability::D3d9GetAdapterCount
                                    | GenericRuntimeCapability::D3d9GetAdapterMonitor
                                    | GenericRuntimeCapability::D3d9CreateDevice
                                    | GenericRuntimeCapability::D3d9CreateTexture
                                    | GenericRuntimeCapability::D3d9GetSurfaceLevel
                                    | GenericRuntimeCapability::D3d9SurfaceGetDc
                                    | GenericRuntimeCapability::TimeGetTime
                                    | GenericRuntimeCapability::D3d9InitialTextures
                                    | GenericRuntimeCapability::D3d9InitialVertexBuffer
                                    | GenericRuntimeCapability::D3d9InitialRenderStates
                                    | GenericRuntimeCapability::D3d9RasterStatus
                                    | GenericRuntimeCapability::QueryPerformanceFrequency
                                    | GenericRuntimeCapability::GetMonitorInfoA
                            )
                        )
                    {
                        dispatch_d3d9_com_call(&d3d9_policy, trap, &mut session)
                    } else if target.object.class_identity.starts_with("dinput8.")
                        && dinput8_enum_devices_enabled
                    {
                        dispatch_dinput8_com_call(&dinput8_policy, trap, &mut session)
                    } else if target.object.class_identity.starts_with("dsound.")
                        && dsound_root_enabled
                    {
                        dispatch_dsound_com_call(&dsound_policy, trap, &mut session)
                    } else {
                        session.dispatch_com_call(trap)
                    }
                };
                match outcome {
                    ComDispatchOutcome::Completed(completion) => {
                        let graphics_device = if completion.frame.interface == "IDirect3DDevice9"
                            && matches!(
                                completion.frame.method.as_str(),
                                "Clear"
                                    | "BeginScene"
                                    | "SetSamplerState"
                                    | "SetTextureStageState"
                                    | "SetTexture"
                                    | "SetFVF"
                                    | "SetStreamSource"
                                    | "DrawPrimitive"
                                    | "EndScene"
                                    | "Present"
                            ) {
                            process
                                .runtime
                                .graphics_devices()
                                .values()
                                .find(|device| device.com_object_id == completion.frame.object_id)
                                .cloned()
                        } else {
                            None
                        };
                        let graphics_texture = if completion.frame.interface == "IDirect3DTexture9"
                            && matches!(
                                completion.frame.method.as_str(),
                                "AddDirtyRect" | "LockRect" | "UnlockRect"
                            ) {
                            process
                                .runtime
                                .graphics_textures()
                                .values()
                                .find(|texture| texture.com_object_id == completion.frame.object_id)
                                .cloned()
                        } else {
                            None
                        };
                        let graphics_texture_update = if completion.frame.interface
                            == "IDirect3DDevice9"
                            && completion.frame.method == "UpdateTexture"
                        {
                            match completion.frame.explicit_arguments.as_slice() {
                                [source_pointer, destination_pointer] => {
                                    let source_object_id = process
                                        .runtime
                                        .com_interface_for_pointer(GuestAddress(*source_pointer))
                                        .ok()
                                        .map(|interface| interface.object_id);
                                    let destination_object_id = process
                                        .runtime
                                        .com_interface_for_pointer(GuestAddress(
                                            *destination_pointer,
                                        ))
                                        .ok()
                                        .map(|interface| interface.object_id);
                                    source_object_id.zip(destination_object_id).and_then(
                                        |(source_object_id, destination_object_id)| {
                                            let source = process
                                                .runtime
                                                .graphics_textures()
                                                .values()
                                                .find(|texture| {
                                                    texture.com_object_id == source_object_id
                                                })?
                                                .clone();
                                            let destination = process
                                                .runtime
                                                .graphics_textures()
                                                .values()
                                                .find(|texture| {
                                                    texture.com_object_id == destination_object_id
                                                })?
                                                .clone();
                                            Some(GenericRuntimeGraphicsTextureUpdate {
                                                source,
                                                destination,
                                            })
                                        },
                                    )
                                }
                                _ => None,
                            }
                        } else {
                            None
                        };
                        send_pending_runtime_delta(input, memory)?;
                        write_runtime_json_frame(
                            RUNTIME_FRAME_API_COMPLETION,
                            &GenericRuntimeComCompletion {
                                schema: "vine.v86-generic-runtime-com-completion",
                                schema_version: 1,
                                endpoint,
                                completion,
                                graphics_device,
                                graphics_texture,
                                graphics_texture_update,
                                memory: memory.stats(),
                            },
                        )?;
                    }
                    ComDispatchOutcome::Fault(fault) => {
                        write_generic_runtime_com_fault(memory, endpoint, fault)?;
                        return Ok(());
                    }
                }
            }
        }
    }
    Err(LoaderError::resource_limit(
        "runtime-reentry",
        "generic-runtime-trap-limit-exceeded",
        format!("generic runtime re-entry exceeded its bounded {trap_limit}-trap limit"),
    ))
}

fn runtime_capability_enables_endpoint(
    enabled: Option<GenericRuntimeCapability>,
    endpoint: &ApiEndpointIdentity,
    virtual_alloc_enabled: bool,
    set_rect_enabled: bool,
    get_window_rect_enabled: bool,
) -> bool {
    let get_monitor_info_a_enabled =
        matches!(enabled, Some(GenericRuntimeCapability::GetMonitorInfoA));
    let time_get_time_enabled = matches!(
        enabled,
        Some(
            GenericRuntimeCapability::TimeGetTime
                | GenericRuntimeCapability::D3d9InitialTextures
                | GenericRuntimeCapability::D3d9InitialVertexBuffer
                | GenericRuntimeCapability::D3d9InitialRenderStates
                | GenericRuntimeCapability::D3d9RasterStatus
                | GenericRuntimeCapability::QueryPerformanceFrequency
                | GenericRuntimeCapability::GetMonitorInfoA
        )
    );
    let monitor_from_window_enabled = matches!(
        enabled,
        Some(
            GenericRuntimeCapability::MonitorFromWindow
                | GenericRuntimeCapability::D3d9GetAdapterCount
                | GenericRuntimeCapability::D3d9GetAdapterMonitor
                | GenericRuntimeCapability::D3d9CreateDevice
                | GenericRuntimeCapability::D3d9CreateTexture
                | GenericRuntimeCapability::D3d9GetSurfaceLevel
                | GenericRuntimeCapability::D3d9SurfaceGetDc
                | GenericRuntimeCapability::TimeGetTime
                | GenericRuntimeCapability::D3d9InitialTextures
                | GenericRuntimeCapability::D3d9InitialVertexBuffer
                | GenericRuntimeCapability::D3d9InitialRenderStates
                | GenericRuntimeCapability::D3d9RasterStatus
                | GenericRuntimeCapability::QueryPerformanceFrequency
                | GenericRuntimeCapability::GetMonitorInfoA
        )
    );
    let set_window_long_a_enabled = matches!(
        enabled,
        Some(
            GenericRuntimeCapability::SetWindowLongA
                | GenericRuntimeCapability::SetWindowPosActivation
                | GenericRuntimeCapability::MonitorFromWindow
                | GenericRuntimeCapability::D3d9GetAdapterCount
                | GenericRuntimeCapability::D3d9GetAdapterMonitor
                | GenericRuntimeCapability::D3d9CreateDevice
                | GenericRuntimeCapability::D3d9CreateTexture
                | GenericRuntimeCapability::D3d9GetSurfaceLevel
                | GenericRuntimeCapability::D3d9SurfaceGetDc
                | GenericRuntimeCapability::TimeGetTime
                | GenericRuntimeCapability::D3d9InitialTextures
                | GenericRuntimeCapability::D3d9InitialVertexBuffer
                | GenericRuntimeCapability::D3d9InitialRenderStates
                | GenericRuntimeCapability::D3d9RasterStatus
                | GenericRuntimeCapability::QueryPerformanceFrequency
                | GenericRuntimeCapability::GetMonitorInfoA
        )
    );
    let window_dc_fill_enabled = matches!(
        enabled,
        Some(
            GenericRuntimeCapability::WindowDcFill
                | GenericRuntimeCapability::SetWindowLongA
                | GenericRuntimeCapability::SetWindowPosActivation
                | GenericRuntimeCapability::MonitorFromWindow
                | GenericRuntimeCapability::D3d9GetAdapterCount
                | GenericRuntimeCapability::D3d9GetAdapterMonitor
                | GenericRuntimeCapability::D3d9CreateDevice
                | GenericRuntimeCapability::D3d9CreateTexture
                | GenericRuntimeCapability::D3d9GetSurfaceLevel
                | GenericRuntimeCapability::D3d9SurfaceGetDc
                | GenericRuntimeCapability::TimeGetTime
                | GenericRuntimeCapability::D3d9InitialTextures
                | GenericRuntimeCapability::D3d9InitialVertexBuffer
                | GenericRuntimeCapability::D3d9InitialRenderStates
                | GenericRuntimeCapability::D3d9RasterStatus
                | GenericRuntimeCapability::QueryPerformanceFrequency
                | GenericRuntimeCapability::GetMonitorInfoA
        )
    );
    let get_client_rect_enabled = matches!(
        enabled,
        Some(
            GenericRuntimeCapability::GetClientRect
                | GenericRuntimeCapability::WindowDcFill
                | GenericRuntimeCapability::SetWindowLongA
                | GenericRuntimeCapability::SetWindowPosActivation
                | GenericRuntimeCapability::MonitorFromWindow
                | GenericRuntimeCapability::D3d9GetAdapterCount
                | GenericRuntimeCapability::D3d9GetAdapterMonitor
                | GenericRuntimeCapability::D3d9CreateDevice
                | GenericRuntimeCapability::D3d9CreateTexture
                | GenericRuntimeCapability::D3d9GetSurfaceLevel
                | GenericRuntimeCapability::D3d9SurfaceGetDc
                | GenericRuntimeCapability::TimeGetTime
                | GenericRuntimeCapability::D3d9InitialTextures
                | GenericRuntimeCapability::D3d9InitialVertexBuffer
                | GenericRuntimeCapability::D3d9InitialRenderStates
                | GenericRuntimeCapability::D3d9RasterStatus
                | GenericRuntimeCapability::QueryPerformanceFrequency
                | GenericRuntimeCapability::GetMonitorInfoA
        )
    );
    let load_library_a_enabled = matches!(
        enabled,
        Some(
            GenericRuntimeCapability::LoadLibraryA
                | GenericRuntimeCapability::GetClientRect
                | GenericRuntimeCapability::WindowDcFill
                | GenericRuntimeCapability::SetWindowLongA
                | GenericRuntimeCapability::SetWindowPosActivation
                | GenericRuntimeCapability::MonitorFromWindow
                | GenericRuntimeCapability::D3d9GetAdapterCount
                | GenericRuntimeCapability::D3d9GetAdapterMonitor
                | GenericRuntimeCapability::D3d9CreateDevice
                | GenericRuntimeCapability::D3d9CreateTexture
                | GenericRuntimeCapability::D3d9GetSurfaceLevel
                | GenericRuntimeCapability::D3d9SurfaceGetDc
                | GenericRuntimeCapability::TimeGetTime
                | GenericRuntimeCapability::D3d9InitialTextures
                | GenericRuntimeCapability::D3d9InitialVertexBuffer
                | GenericRuntimeCapability::D3d9InitialRenderStates
                | GenericRuntimeCapability::D3d9RasterStatus
                | GenericRuntimeCapability::QueryPerformanceFrequency
                | GenericRuntimeCapability::GetMonitorInfoA
        )
    );
    let d3d9_root_factory_enabled = matches!(
        enabled,
        Some(
            GenericRuntimeCapability::D3d9RootFactoryAndIdentity
                | GenericRuntimeCapability::D3d9GetAdapterDisplayMode
                | GenericRuntimeCapability::D3d9GetAdapterIdentifier
                | GenericRuntimeCapability::D3d9GetDeviceCaps
                | GenericRuntimeCapability::D3d9CheckDeviceType
                | GenericRuntimeCapability::D3d9CheckDeviceFormat
                | GenericRuntimeCapability::LoadLibraryA
                | GenericRuntimeCapability::GetClientRect
                | GenericRuntimeCapability::WindowDcFill
                | GenericRuntimeCapability::SetWindowLongA
                | GenericRuntimeCapability::SetWindowPosActivation
                | GenericRuntimeCapability::MonitorFromWindow
                | GenericRuntimeCapability::D3d9GetAdapterCount
                | GenericRuntimeCapability::D3d9GetAdapterMonitor
                | GenericRuntimeCapability::D3d9CreateDevice
                | GenericRuntimeCapability::D3d9CreateTexture
                | GenericRuntimeCapability::D3d9GetSurfaceLevel
                | GenericRuntimeCapability::D3d9SurfaceGetDc
                | GenericRuntimeCapability::TimeGetTime
                | GenericRuntimeCapability::D3d9InitialTextures
                | GenericRuntimeCapability::D3d9InitialVertexBuffer
                | GenericRuntimeCapability::D3d9InitialRenderStates
                | GenericRuntimeCapability::D3d9RasterStatus
                | GenericRuntimeCapability::QueryPerformanceFrequency
                | GenericRuntimeCapability::GetMonitorInfoA
        )
    );
    let set_window_pos_enabled = d3d9_root_factory_enabled
        || matches!(
            enabled,
            Some(GenericRuntimeCapability::SetWindowPosInitialPlacement)
        );
    let send_message_a_enabled = set_window_pos_enabled
        || matches!(
            enabled,
            Some(GenericRuntimeCapability::SendMessageASmallIcon)
        );
    let load_image_a_enabled = send_message_a_enabled
        || matches!(enabled, Some(GenericRuntimeCapability::LoadImageASmallIcon));
    let create_window_ex_a_enabled = matches!(
        enabled,
        Some(
            GenericRuntimeCapability::CreateWindowExA
                | GenericRuntimeCapability::GetSystemMetricsSmallIconHeight
                | GenericRuntimeCapability::GetSystemMetricsSmallIconWidth
                | GenericRuntimeCapability::LoadImageASmallIcon
                | GenericRuntimeCapability::SendMessageASmallIcon
                | GenericRuntimeCapability::SetWindowPosInitialPlacement
                | GenericRuntimeCapability::D3d9RootFactoryAndIdentity
                | GenericRuntimeCapability::D3d9GetAdapterDisplayMode
                | GenericRuntimeCapability::D3d9GetAdapterIdentifier
                | GenericRuntimeCapability::D3d9GetDeviceCaps
                | GenericRuntimeCapability::D3d9CheckDeviceType
                | GenericRuntimeCapability::D3d9CheckDeviceFormat
                | GenericRuntimeCapability::LoadLibraryA
                | GenericRuntimeCapability::GetClientRect
                | GenericRuntimeCapability::WindowDcFill
                | GenericRuntimeCapability::SetWindowLongA
                | GenericRuntimeCapability::SetWindowPosActivation
                | GenericRuntimeCapability::MonitorFromWindow
                | GenericRuntimeCapability::D3d9GetAdapterCount
                | GenericRuntimeCapability::D3d9GetAdapterMonitor
                | GenericRuntimeCapability::D3d9CreateDevice
                | GenericRuntimeCapability::D3d9CreateTexture
                | GenericRuntimeCapability::D3d9GetSurfaceLevel
                | GenericRuntimeCapability::D3d9SurfaceGetDc
                | GenericRuntimeCapability::TimeGetTime
                | GenericRuntimeCapability::D3d9InitialTextures
                | GenericRuntimeCapability::D3d9InitialVertexBuffer
                | GenericRuntimeCapability::D3d9InitialRenderStates
                | GenericRuntimeCapability::D3d9RasterStatus
                | GenericRuntimeCapability::QueryPerformanceFrequency
                | GenericRuntimeCapability::GetMonitorInfoA
        )
    );
    let source_file_read_enabled = create_window_ex_a_enabled
        || matches!(enabled, Some(GenericRuntimeCapability::SourceFileRead));
    let enabled = if source_file_read_enabled {
        Some(GenericRuntimeCapability::LargeHeapAllocation)
    } else {
        enabled
    };
    let get_system_metrics_enabled = matches!(
        enabled,
        Some(
            GenericRuntimeCapability::GetSystemMetrics
                | GenericRuntimeCapability::LargeHeapAllocation
        )
    );
    let register_class_ex_a_enabled = get_system_metrics_enabled
        || matches!(enabled, Some(GenericRuntimeCapability::RegisterClassExA));
    let load_cursor_a_enabled = register_class_ex_a_enabled
        || matches!(enabled, Some(GenericRuntimeCapability::LoadCursorA));
    let load_icon_a_enabled =
        load_cursor_a_enabled || matches!(enabled, Some(GenericRuntimeCapability::LoadIconA));
    let enabled = if load_icon_a_enabled {
        Some(GenericRuntimeCapability::CreateWaitableTimerA)
    } else {
        enabled
    };
    let get_module_file_name_a_enabled = matches!(
        enabled,
        Some(
            GenericRuntimeCapability::GetModuleFileNameA
                | GenericRuntimeCapability::PrivateHeapSize16
                | GenericRuntimeCapability::PrivateHeapSize4
                | GenericRuntimeCapability::PrivateHeapFreeSize1
                | GenericRuntimeCapability::PrivateHeapSize128
                | GenericRuntimeCapability::IsProcessorFeaturePresent
                | GenericRuntimeCapability::SetUnhandledExceptionFilter
                | GenericRuntimeCapability::HeapSize
                | GenericRuntimeCapability::InitializeCriticalSection
                | GenericRuntimeCapability::SetCurrentDirectoryA
                | GenericRuntimeCapability::CreateMutexA
                | GenericRuntimeCapability::GetCurrentThread
                | GenericRuntimeCapability::SetThreadAffinityMask
                | GenericRuntimeCapability::GetSystemInfo
                | GenericRuntimeCapability::EnumDisplayMonitors
                | GenericRuntimeCapability::CoInitialize
                | GenericRuntimeCapability::GetSystemDefaultLangId
                | GenericRuntimeCapability::GetDriveTypeA
                | GenericRuntimeCapability::SwapMouseButton
                | GenericRuntimeCapability::TimeGetDevCaps
                | GenericRuntimeCapability::TimeBeginPeriod
                | GenericRuntimeCapability::GetVersionExA
                | GenericRuntimeCapability::CreateWaitableTimerA
        )
    );
    let interlocked_decrement_enabled = get_module_file_name_a_enabled
        || matches!(
            enabled,
            Some(GenericRuntimeCapability::InterlockedDecrement)
        );
    let heap_free_enabled = matches!(
        enabled,
        Some(
            GenericRuntimeCapability::PrivateHeapFreeSize1
                | GenericRuntimeCapability::PrivateHeapSize128
                | GenericRuntimeCapability::IsProcessorFeaturePresent
                | GenericRuntimeCapability::SetUnhandledExceptionFilter
                | GenericRuntimeCapability::HeapSize
                | GenericRuntimeCapability::InitializeCriticalSection
                | GenericRuntimeCapability::SetCurrentDirectoryA
                | GenericRuntimeCapability::CreateMutexA
                | GenericRuntimeCapability::GetCurrentThread
                | GenericRuntimeCapability::SetThreadAffinityMask
                | GenericRuntimeCapability::GetSystemInfo
                | GenericRuntimeCapability::EnumDisplayMonitors
                | GenericRuntimeCapability::CoInitialize
                | GenericRuntimeCapability::GetSystemDefaultLangId
                | GenericRuntimeCapability::GetDriveTypeA
                | GenericRuntimeCapability::SwapMouseButton
                | GenericRuntimeCapability::TimeGetDevCaps
                | GenericRuntimeCapability::TimeBeginPeriod
                | GenericRuntimeCapability::GetVersionExA
                | GenericRuntimeCapability::CreateWaitableTimerA
        )
    );
    let set_current_directory_a_enabled = matches!(
        enabled,
        Some(
            GenericRuntimeCapability::SetCurrentDirectoryA
                | GenericRuntimeCapability::CreateMutexA
                | GenericRuntimeCapability::GetCurrentThread
                | GenericRuntimeCapability::SetThreadAffinityMask
                | GenericRuntimeCapability::GetSystemInfo
                | GenericRuntimeCapability::EnumDisplayMonitors
                | GenericRuntimeCapability::CoInitialize
                | GenericRuntimeCapability::GetSystemDefaultLangId
                | GenericRuntimeCapability::GetDriveTypeA
                | GenericRuntimeCapability::SwapMouseButton
                | GenericRuntimeCapability::TimeGetDevCaps
                | GenericRuntimeCapability::TimeBeginPeriod
                | GenericRuntimeCapability::GetVersionExA
                | GenericRuntimeCapability::CreateWaitableTimerA
        )
    );
    let create_mutex_a_enabled = matches!(
        enabled,
        Some(
            GenericRuntimeCapability::CreateMutexA
                | GenericRuntimeCapability::GetCurrentThread
                | GenericRuntimeCapability::SetThreadAffinityMask
                | GenericRuntimeCapability::GetSystemInfo
                | GenericRuntimeCapability::EnumDisplayMonitors
                | GenericRuntimeCapability::CoInitialize
                | GenericRuntimeCapability::GetSystemDefaultLangId
                | GenericRuntimeCapability::GetDriveTypeA
                | GenericRuntimeCapability::SwapMouseButton
                | GenericRuntimeCapability::TimeGetDevCaps
                | GenericRuntimeCapability::TimeBeginPeriod
                | GenericRuntimeCapability::GetVersionExA
                | GenericRuntimeCapability::CreateWaitableTimerA
        )
    );
    let get_current_thread_enabled = matches!(
        enabled,
        Some(
            GenericRuntimeCapability::GetCurrentThread
                | GenericRuntimeCapability::SetThreadAffinityMask
                | GenericRuntimeCapability::GetSystemInfo
                | GenericRuntimeCapability::EnumDisplayMonitors
                | GenericRuntimeCapability::CoInitialize
                | GenericRuntimeCapability::GetSystemDefaultLangId
                | GenericRuntimeCapability::GetDriveTypeA
                | GenericRuntimeCapability::SwapMouseButton
                | GenericRuntimeCapability::TimeGetDevCaps
                | GenericRuntimeCapability::TimeBeginPeriod
                | GenericRuntimeCapability::GetVersionExA
                | GenericRuntimeCapability::CreateWaitableTimerA
        )
    );
    let set_thread_affinity_mask_enabled = matches!(
        enabled,
        Some(
            GenericRuntimeCapability::SetThreadAffinityMask
                | GenericRuntimeCapability::GetSystemInfo
                | GenericRuntimeCapability::EnumDisplayMonitors
                | GenericRuntimeCapability::CoInitialize
                | GenericRuntimeCapability::GetSystemDefaultLangId
                | GenericRuntimeCapability::GetDriveTypeA
                | GenericRuntimeCapability::SwapMouseButton
                | GenericRuntimeCapability::TimeGetDevCaps
                | GenericRuntimeCapability::TimeBeginPeriod
                | GenericRuntimeCapability::GetVersionExA
                | GenericRuntimeCapability::CreateWaitableTimerA
        )
    );
    let create_waitable_timer_a_enabled = matches!(
        enabled,
        Some(GenericRuntimeCapability::CreateWaitableTimerA)
    );
    let get_version_ex_a_enabled = create_waitable_timer_a_enabled
        || matches!(enabled, Some(GenericRuntimeCapability::GetVersionExA));
    let time_begin_period_enabled = get_version_ex_a_enabled
        || matches!(enabled, Some(GenericRuntimeCapability::TimeBeginPeriod));
    let time_get_dev_caps_enabled = time_begin_period_enabled
        || matches!(enabled, Some(GenericRuntimeCapability::TimeGetDevCaps));
    let swap_mouse_button_enabled = time_get_dev_caps_enabled
        || matches!(enabled, Some(GenericRuntimeCapability::SwapMouseButton));
    let get_drive_type_a_enabled = swap_mouse_button_enabled
        || matches!(enabled, Some(GenericRuntimeCapability::GetDriveTypeA));
    let get_system_default_lang_id_enabled = get_drive_type_a_enabled
        || matches!(
            enabled,
            Some(GenericRuntimeCapability::GetSystemDefaultLangId)
        );
    let co_initialize_enabled = get_system_default_lang_id_enabled
        || matches!(enabled, Some(GenericRuntimeCapability::CoInitialize));
    let enum_display_monitors_enabled = co_initialize_enabled
        || matches!(enabled, Some(GenericRuntimeCapability::EnumDisplayMonitors));
    let get_system_info_enabled = enum_display_monitors_enabled
        || matches!(enabled, Some(GenericRuntimeCapability::GetSystemInfo));
    let initialize_critical_section_enabled = set_current_directory_a_enabled
        || matches!(
            enabled,
            Some(GenericRuntimeCapability::InitializeCriticalSection)
        );
    let heap_size_enabled = initialize_critical_section_enabled
        || matches!(enabled, Some(GenericRuntimeCapability::HeapSize));
    let set_unhandled_exception_filter_enabled = heap_size_enabled
        || matches!(
            enabled,
            Some(GenericRuntimeCapability::SetUnhandledExceptionFilter)
        );
    let processor_feature_enabled = set_unhandled_exception_filter_enabled
        || matches!(
            enabled,
            Some(GenericRuntimeCapability::IsProcessorFeaturePresent)
        );
    let lc_map_string_w_enabled = interlocked_decrement_enabled
        || matches!(enabled, Some(GenericRuntimeCapability::LcMapStringW));
    let get_string_type_w_enabled = lc_map_string_w_enabled
        || matches!(enabled, Some(GenericRuntimeCapability::GetStringTypeW));
    let multi_byte_to_wide_char_enabled = get_string_type_w_enabled
        || matches!(enabled, Some(GenericRuntimeCapability::MultiByteToWideChar));
    let get_cp_info_enabled = multi_byte_to_wide_char_enabled
        || matches!(enabled, Some(GenericRuntimeCapability::GetCpInfo));
    let get_acp_enabled = get_cp_info_enabled
        || matches!(
            enabled,
            Some(GenericRuntimeCapability::GetAcp | GenericRuntimeCapability::PrivateHeapSize544)
        );
    let set_last_error_enabled =
        get_acp_enabled || matches!(enabled, Some(GenericRuntimeCapability::SetLastError));
    let tls_get_value_enabled = matches!(
        enabled,
        Some(GenericRuntimeCapability::TlsGetValue | GenericRuntimeCapability::FlsGetValue)
    ) || set_last_error_enabled;
    let get_last_error_enabled = matches!(
        enabled,
        Some(
            GenericRuntimeCapability::GetLastError
                | GenericRuntimeCapability::TlsGetValue
                | GenericRuntimeCapability::FlsGetValue
        )
    ) || set_last_error_enabled;
    let wide_char_to_multi_byte_enabled = matches!(
        enabled,
        Some(
            GenericRuntimeCapability::WideCharToMultiByte
                | GenericRuntimeCapability::PrivateHeapSize1
                | GenericRuntimeCapability::GetLastError
                | GenericRuntimeCapability::TlsGetValue
                | GenericRuntimeCapability::FlsGetValue
        )
    ) || set_last_error_enabled;
    let environment_strings_w_enabled = matches!(
        enabled,
        Some(
            GenericRuntimeCapability::EnvironmentStringsW
                | GenericRuntimeCapability::WideCharToMultiByte
                | GenericRuntimeCapability::PrivateHeapSize1
                | GenericRuntimeCapability::GetLastError
                | GenericRuntimeCapability::TlsGetValue
                | GenericRuntimeCapability::FlsGetValue
        )
    ) || set_last_error_enabled;
    let command_line_a_enabled = matches!(
        enabled,
        Some(
            GenericRuntimeCapability::CommandLineA
                | GenericRuntimeCapability::EnvironmentStringsW
                | GenericRuntimeCapability::WideCharToMultiByte
                | GenericRuntimeCapability::PrivateHeapSize1
                | GenericRuntimeCapability::GetLastError
                | GenericRuntimeCapability::TlsGetValue
                | GenericRuntimeCapability::FlsGetValue
        )
    ) || set_last_error_enabled;
    let enabled = match enabled {
        Some(
            GenericRuntimeCapability::CommandLineA
            | GenericRuntimeCapability::EnvironmentStringsW
            | GenericRuntimeCapability::WideCharToMultiByte
            | GenericRuntimeCapability::PrivateHeapSize1
            | GenericRuntimeCapability::GetLastError
            | GenericRuntimeCapability::TlsGetValue
            | GenericRuntimeCapability::FlsGetValue
            | GenericRuntimeCapability::SetLastError
            | GenericRuntimeCapability::GetAcp
            | GenericRuntimeCapability::PrivateHeapSize544
            | GenericRuntimeCapability::GetCpInfo
            | GenericRuntimeCapability::MultiByteToWideChar
            | GenericRuntimeCapability::GetStringTypeW
            | GenericRuntimeCapability::LcMapStringW
            | GenericRuntimeCapability::InterlockedDecrement
            | GenericRuntimeCapability::GetModuleFileNameA
            | GenericRuntimeCapability::PrivateHeapSize16
            | GenericRuntimeCapability::PrivateHeapSize4
            | GenericRuntimeCapability::PrivateHeapFreeSize1
            | GenericRuntimeCapability::PrivateHeapSize128
            | GenericRuntimeCapability::IsProcessorFeaturePresent
            | GenericRuntimeCapability::SetUnhandledExceptionFilter
            | GenericRuntimeCapability::HeapSize
            | GenericRuntimeCapability::InitializeCriticalSection
            | GenericRuntimeCapability::SetCurrentDirectoryA
            | GenericRuntimeCapability::CreateMutexA
            | GenericRuntimeCapability::GetCurrentThread
            | GenericRuntimeCapability::SetThreadAffinityMask
            | GenericRuntimeCapability::GetSystemInfo
            | GenericRuntimeCapability::EnumDisplayMonitors
            | GenericRuntimeCapability::CoInitialize
            | GenericRuntimeCapability::GetSystemDefaultLangId
            | GenericRuntimeCapability::GetDriveTypeA
            | GenericRuntimeCapability::SwapMouseButton
            | GenericRuntimeCapability::TimeGetDevCaps
            | GenericRuntimeCapability::TimeBeginPeriod
            | GenericRuntimeCapability::GetVersionExA
            | GenericRuntimeCapability::CreateWaitableTimerA,
        ) => Some(GenericRuntimeCapability::SetHandleCount),
        capability => capability,
    };
    let vine_core::ApiSymbol::Name { name } = &endpoint.symbol else {
        return true;
    };
    match name.as_str() {
        "InitializeCriticalSectionAndSpinCount"
        | "EnterCriticalSection"
        | "LeaveCriticalSection" => matches!(
            enabled,
            Some(
                GenericRuntimeCapability::CriticalSectionSynchronization
                    | GenericRuntimeCapability::PointerDecoding
                    | GenericRuntimeCapability::FiberLocalStorageAllocation
                    | GenericRuntimeCapability::PrivateHeapAllocation
                    | GenericRuntimeCapability::FiberLocalStorageValue
                    | GenericRuntimeCapability::InterlockedIncrement
                    | GenericRuntimeCapability::PrivateHeapSecondAllocation
                    | GenericRuntimeCapability::StandardInputHandle
                    | GenericRuntimeCapability::FileType
                    | GenericRuntimeCapability::StandardOutputHandle
                    | GenericRuntimeCapability::StandardOutputFileType
                    | GenericRuntimeCapability::StandardErrorHandle
                    | GenericRuntimeCapability::StandardErrorFileType
                    | GenericRuntimeCapability::SetHandleCount
            )
        ),
        "DecodePointer" => matches!(
            enabled,
            Some(
                GenericRuntimeCapability::PointerDecoding
                    | GenericRuntimeCapability::FiberLocalStorageAllocation
                    | GenericRuntimeCapability::PrivateHeapAllocation
                    | GenericRuntimeCapability::FiberLocalStorageValue
                    | GenericRuntimeCapability::InterlockedIncrement
                    | GenericRuntimeCapability::PrivateHeapSecondAllocation
                    | GenericRuntimeCapability::StandardInputHandle
                    | GenericRuntimeCapability::FileType
                    | GenericRuntimeCapability::StandardOutputHandle
                    | GenericRuntimeCapability::StandardOutputFileType
                    | GenericRuntimeCapability::StandardErrorHandle
                    | GenericRuntimeCapability::StandardErrorFileType
                    | GenericRuntimeCapability::SetHandleCount
            )
        ),
        "FlsAlloc" => matches!(
            enabled,
            Some(
                GenericRuntimeCapability::FiberLocalStorageAllocation
                    | GenericRuntimeCapability::PrivateHeapAllocation
                    | GenericRuntimeCapability::FiberLocalStorageValue
                    | GenericRuntimeCapability::InterlockedIncrement
                    | GenericRuntimeCapability::PrivateHeapSecondAllocation
                    | GenericRuntimeCapability::StandardInputHandle
                    | GenericRuntimeCapability::FileType
                    | GenericRuntimeCapability::StandardOutputHandle
                    | GenericRuntimeCapability::StandardOutputFileType
                    | GenericRuntimeCapability::StandardErrorHandle
                    | GenericRuntimeCapability::StandardErrorFileType
                    | GenericRuntimeCapability::SetHandleCount
            )
        ),
        "HeapAlloc" => matches!(
            enabled,
            Some(
                GenericRuntimeCapability::PrivateHeapAllocation
                    | GenericRuntimeCapability::FiberLocalStorageValue
                    | GenericRuntimeCapability::InterlockedIncrement
                    | GenericRuntimeCapability::PrivateHeapSecondAllocation
                    | GenericRuntimeCapability::StandardInputHandle
                    | GenericRuntimeCapability::FileType
                    | GenericRuntimeCapability::StandardOutputHandle
                    | GenericRuntimeCapability::StandardOutputFileType
                    | GenericRuntimeCapability::StandardErrorHandle
                    | GenericRuntimeCapability::StandardErrorFileType
                    | GenericRuntimeCapability::SetHandleCount
            )
        ),
        "HeapFree" => heap_free_enabled,
        "FlsSetValue" => matches!(
            enabled,
            Some(
                GenericRuntimeCapability::FiberLocalStorageValue
                    | GenericRuntimeCapability::InterlockedIncrement
                    | GenericRuntimeCapability::PrivateHeapSecondAllocation
                    | GenericRuntimeCapability::StandardInputHandle
                    | GenericRuntimeCapability::FileType
                    | GenericRuntimeCapability::StandardOutputHandle
                    | GenericRuntimeCapability::StandardOutputFileType
                    | GenericRuntimeCapability::StandardErrorHandle
                    | GenericRuntimeCapability::StandardErrorFileType
                    | GenericRuntimeCapability::SetHandleCount
            )
        ),
        "InterlockedIncrement" => matches!(
            enabled,
            Some(
                GenericRuntimeCapability::InterlockedIncrement
                    | GenericRuntimeCapability::PrivateHeapSecondAllocation
                    | GenericRuntimeCapability::StandardInputHandle
                    | GenericRuntimeCapability::FileType
                    | GenericRuntimeCapability::StandardOutputHandle
                    | GenericRuntimeCapability::StandardOutputFileType
                    | GenericRuntimeCapability::StandardErrorHandle
                    | GenericRuntimeCapability::StandardErrorFileType
                    | GenericRuntimeCapability::SetHandleCount
            )
        ),
        "GetStdHandle" => matches!(
            enabled,
            Some(
                GenericRuntimeCapability::StandardInputHandle
                    | GenericRuntimeCapability::FileType
                    | GenericRuntimeCapability::StandardOutputHandle
                    | GenericRuntimeCapability::StandardOutputFileType
                    | GenericRuntimeCapability::StandardErrorHandle
                    | GenericRuntimeCapability::StandardErrorFileType
                    | GenericRuntimeCapability::SetHandleCount
            )
        ),
        "GetFileType" => matches!(
            enabled,
            Some(
                GenericRuntimeCapability::FileType
                    | GenericRuntimeCapability::StandardOutputHandle
                    | GenericRuntimeCapability::StandardOutputFileType
                    | GenericRuntimeCapability::StandardErrorHandle
                    | GenericRuntimeCapability::StandardErrorFileType
                    | GenericRuntimeCapability::SetHandleCount
            )
        ),
        "SetHandleCount" => matches!(enabled, Some(GenericRuntimeCapability::SetHandleCount)),
        "GetCommandLineA" => command_line_a_enabled,
        "GetEnvironmentStringsW" | "FreeEnvironmentStringsW" => environment_strings_w_enabled,
        "WideCharToMultiByte" => wide_char_to_multi_byte_enabled,
        "GetLastError" => get_last_error_enabled,
        "TlsGetValue" => tls_get_value_enabled,
        "SetLastError" => set_last_error_enabled,
        "GetACP" => get_acp_enabled,
        "GetCPInfo" => get_cp_info_enabled,
        "MultiByteToWideChar" => multi_byte_to_wide_char_enabled,
        "GetStringTypeW" => get_string_type_w_enabled,
        "LCMapStringW" => lc_map_string_w_enabled,
        "InterlockedDecrement" => interlocked_decrement_enabled,
        "GetModuleFileNameA" => get_module_file_name_a_enabled,
        "IsProcessorFeaturePresent" => processor_feature_enabled,
        "SetUnhandledExceptionFilter" => set_unhandled_exception_filter_enabled,
        "HeapSize" => heap_size_enabled,
        "InitializeCriticalSection" => initialize_critical_section_enabled,
        "SetCurrentDirectoryA" => set_current_directory_a_enabled,
        "CreateMutexA" => create_mutex_a_enabled,
        "GetCurrentThread" => get_current_thread_enabled,
        "SetThreadAffinityMask" => set_thread_affinity_mask_enabled,
        "GetSystemInfo" => get_system_info_enabled,
        "EnumDisplayMonitors" => enum_display_monitors_enabled,
        "CoInitialize" => co_initialize_enabled,
        "GetSystemDefaultLangID" => get_system_default_lang_id_enabled,
        "GetDriveTypeA" => get_drive_type_a_enabled,
        "SwapMouseButton" => swap_mouse_button_enabled,
        "timeGetDevCaps" => time_get_dev_caps_enabled,
        "timeBeginPeriod" | "timeEndPeriod" => time_begin_period_enabled,
        "timeGetTime" => time_get_time_enabled,
        "GetVersionExA" => get_version_ex_a_enabled,
        "CreateWaitableTimerA" => create_waitable_timer_a_enabled,
        "VirtualAlloc" => virtual_alloc_enabled,
        "LoadIconA" => load_icon_a_enabled,
        "LoadImageA" => load_image_a_enabled,
        "SendMessageA" => send_message_a_enabled,
        "SetWindowPos" => set_window_pos_enabled,
        "LoadCursorA" => load_cursor_a_enabled,
        "Direct3DCreate9" => d3d9_root_factory_enabled,
        "RegisterClassExA" => register_class_ex_a_enabled,
        "GetSystemMetrics" => get_system_metrics_enabled,
        "CreateWindowExA" | "DefWindowProcA" | "GetSystemMenu" | "EnableMenuItem" => {
            create_window_ex_a_enabled
        }
        "CreateFileA" | "GetFileSize" | "ReadFile" | "SetFilePointer" | "CloseHandle" => {
            source_file_read_enabled
        }
        "LoadLibraryA" => load_library_a_enabled,
        "GetClientRect" => get_client_rect_enabled,
        "GetWindowRect" | "GetCursorPos" => get_window_rect_enabled,
        "SetRect" => set_rect_enabled,
        "GetDC" | "GetStockObject" | "FillRect" | "ReleaseDC" => window_dc_fill_enabled,
        "SetWindowLongA" => set_window_long_a_enabled,
        "MonitorFromWindow" => monitor_from_window_enabled,
        "GetMonitorInfoA" => get_monitor_info_a_enabled,
        _ => true,
    }
}

fn runtime_capability_enables_dynamic_endpoint(
    enabled: Option<GenericRuntimeCapability>,
    endpoint: &RuntimeDynamicEndpoint,
) -> bool {
    let enabled = if matches!(
        enabled,
        Some(
            GenericRuntimeCapability::D3d9RootFactoryAndIdentity
                | GenericRuntimeCapability::D3d9GetAdapterDisplayMode
                | GenericRuntimeCapability::D3d9GetAdapterIdentifier
                | GenericRuntimeCapability::D3d9GetDeviceCaps
                | GenericRuntimeCapability::D3d9CheckDeviceType
                | GenericRuntimeCapability::D3d9CheckDeviceFormat
                | GenericRuntimeCapability::LoadLibraryA
                | GenericRuntimeCapability::GetClientRect
                | GenericRuntimeCapability::WindowDcFill
                | GenericRuntimeCapability::SetWindowLongA
                | GenericRuntimeCapability::SetWindowPosActivation
                | GenericRuntimeCapability::MonitorFromWindow
                | GenericRuntimeCapability::D3d9GetAdapterCount
                | GenericRuntimeCapability::D3d9GetAdapterMonitor
                | GenericRuntimeCapability::D3d9CreateDevice
                | GenericRuntimeCapability::D3d9CreateTexture
                | GenericRuntimeCapability::D3d9GetSurfaceLevel
                | GenericRuntimeCapability::D3d9SurfaceGetDc
                | GenericRuntimeCapability::TimeGetTime
                | GenericRuntimeCapability::D3d9InitialTextures
                | GenericRuntimeCapability::D3d9InitialVertexBuffer
                | GenericRuntimeCapability::D3d9InitialRenderStates
                | GenericRuntimeCapability::D3d9RasterStatus
                | GenericRuntimeCapability::QueryPerformanceFrequency
                | GenericRuntimeCapability::GetMonitorInfoA
        )
    ) {
        Some(GenericRuntimeCapability::SetWindowPosInitialPlacement)
    } else {
        enabled
    };
    let enabled = if matches!(
        enabled,
        Some(
            GenericRuntimeCapability::CreateWindowExA
                | GenericRuntimeCapability::GetSystemMetricsSmallIconHeight
                | GenericRuntimeCapability::GetSystemMetricsSmallIconWidth
                | GenericRuntimeCapability::LoadImageASmallIcon
                | GenericRuntimeCapability::SendMessageASmallIcon
                | GenericRuntimeCapability::SetWindowPosInitialPlacement
        )
    ) {
        Some(GenericRuntimeCapability::SourceFileRead)
    } else {
        enabled
    };
    let enabled = if matches!(enabled, Some(GenericRuntimeCapability::SourceFileRead)) {
        Some(GenericRuntimeCapability::LargeHeapAllocation)
    } else {
        enabled
    };
    let enabled = if matches!(
        enabled,
        Some(
            GenericRuntimeCapability::LoadIconA
                | GenericRuntimeCapability::LoadCursorA
                | GenericRuntimeCapability::RegisterClassExA
                | GenericRuntimeCapability::GetSystemMetrics
                | GenericRuntimeCapability::LargeHeapAllocation
        )
    ) {
        Some(GenericRuntimeCapability::CreateWaitableTimerA)
    } else {
        enabled
    };
    if !endpoint.module.eq_ignore_ascii_case("kernel32.dll") {
        return false;
    }
    match enabled {
        Some(
            GenericRuntimeCapability::FiberLocalStorageAllocation
            | GenericRuntimeCapability::PrivateHeapAllocation,
        ) => endpoint.name == "FlsAlloc",
        Some(
            GenericRuntimeCapability::FiberLocalStorageValue
            | GenericRuntimeCapability::InterlockedIncrement
            | GenericRuntimeCapability::PrivateHeapSecondAllocation
            | GenericRuntimeCapability::StandardInputHandle
            | GenericRuntimeCapability::FileType
            | GenericRuntimeCapability::StandardOutputHandle
            | GenericRuntimeCapability::StandardOutputFileType
            | GenericRuntimeCapability::StandardErrorHandle
            | GenericRuntimeCapability::StandardErrorFileType
            | GenericRuntimeCapability::SetHandleCount,
        ) => {
            matches!(endpoint.name.as_str(), "FlsAlloc" | "FlsSetValue")
        }
        Some(
            GenericRuntimeCapability::FlsGetValue
            | GenericRuntimeCapability::SetLastError
            | GenericRuntimeCapability::GetAcp
            | GenericRuntimeCapability::PrivateHeapSize544
            | GenericRuntimeCapability::GetCpInfo
            | GenericRuntimeCapability::MultiByteToWideChar
            | GenericRuntimeCapability::GetStringTypeW
            | GenericRuntimeCapability::LcMapStringW
            | GenericRuntimeCapability::InterlockedDecrement
            | GenericRuntimeCapability::GetModuleFileNameA
            | GenericRuntimeCapability::PrivateHeapSize16
            | GenericRuntimeCapability::PrivateHeapSize4
            | GenericRuntimeCapability::PrivateHeapFreeSize1
            | GenericRuntimeCapability::PrivateHeapSize128
            | GenericRuntimeCapability::IsProcessorFeaturePresent
            | GenericRuntimeCapability::SetUnhandledExceptionFilter
            | GenericRuntimeCapability::HeapSize
            | GenericRuntimeCapability::InitializeCriticalSection
            | GenericRuntimeCapability::SetCurrentDirectoryA
            | GenericRuntimeCapability::CreateMutexA
            | GenericRuntimeCapability::GetCurrentThread
            | GenericRuntimeCapability::SetThreadAffinityMask
            | GenericRuntimeCapability::GetSystemInfo
            | GenericRuntimeCapability::EnumDisplayMonitors
            | GenericRuntimeCapability::CoInitialize
            | GenericRuntimeCapability::GetSystemDefaultLangId
            | GenericRuntimeCapability::GetDriveTypeA
            | GenericRuntimeCapability::SwapMouseButton
            | GenericRuntimeCapability::TimeGetDevCaps
            | GenericRuntimeCapability::TimeBeginPeriod
            | GenericRuntimeCapability::GetVersionExA
            | GenericRuntimeCapability::CreateWaitableTimerA,
        ) => {
            matches!(
                endpoint.name.as_str(),
                "FlsAlloc" | "FlsSetValue" | "FlsGetValue"
            )
        }
        _ => false,
    }
}

fn read_generic_runtime_long_argument(
    memory: &GuestMemory,
    stack_pointer: u32,
) -> Result<(GuestAddress, i32), LoaderError> {
    let argument_address = GuestAddress(stack_pointer.checked_add(4).ok_or_else(|| {
        LoaderError::invalid_input(
            "runtime-reentry",
            "interlocked-frame-address-overflow",
            "interlocked LONG argument address crosses the x86 address limit",
        )
    })?);
    let bytes = memory.read(argument_address, 4).map_err(memory_error)?;
    let address = GuestAddress(u32::from_le_bytes(bytes.try_into().map_err(|_| {
        LoaderError::runtime_defect(
            "runtime-reentry",
            "interlocked-frame-read-size-invalid",
            "checked interlocked frame did not contain one argument",
        )
    })?));
    Ok((address, read_generic_runtime_long(memory, address)?))
}

fn read_generic_runtime_long(
    memory: &GuestMemory,
    address: GuestAddress,
) -> Result<i32, LoaderError> {
    if !address.0.is_multiple_of(size_of::<i32>() as u32) {
        return Err(LoaderError::invalid_input(
            "runtime-reentry",
            "interlocked-target-alignment-invalid",
            "interlocked target is not aligned to one LONG",
        ));
    }
    let bytes = memory
        .read(address, size_of::<i32>())
        .map_err(memory_error)?;
    Ok(i32::from_le_bytes(bytes.try_into().map_err(|_| {
        LoaderError::runtime_defect(
            "runtime-reentry",
            "interlocked-target-read-size-invalid",
            "checked interlocked target did not contain one LONG",
        )
    })?))
}

fn generic_runtime_long_update(
    completion: &ApiImmediateCompletion,
    (address, before): (GuestAddress, i32),
    delta: i32,
    operation: &str,
) -> Result<GenericRuntimeLongUpdate, LoaderError> {
    let [write] = completion.guest_writes.as_slice() else {
        return Err(LoaderError::runtime_defect(
            "runtime-reentry",
            "interlocked-write-count-invalid",
            format!("{operation} completion did not contain exactly one guest write"),
        ));
    };
    if write.write.address != address || write.write.bytes.len() != size_of::<i32>() {
        return Err(LoaderError::runtime_defect(
            "runtime-reentry",
            "interlocked-write-shape-invalid",
            format!("{operation} completion did not write one LONG to its argument"),
        ));
    }
    let after = i32::from_le_bytes(write.write.bytes.as_slice().try_into().map_err(|_| {
        LoaderError::runtime_defect(
            "runtime-reentry",
            "interlocked-write-size-invalid",
            format!("checked {operation} completion did not contain one LONG"),
        )
    })?);
    if after != before.wrapping_add(delta) {
        return Err(LoaderError::runtime_defect(
            "runtime-reentry",
            "interlocked-write-value-invalid",
            format!("{operation} completion did not contain the wrapping result"),
        ));
    }
    Ok(GenericRuntimeLongUpdate {
        address: address.0,
        before,
        after,
    })
}

fn generic_runtime_heap_allocation(
    session: &ExecutionSession<'_>,
    completion: &ApiImmediateCompletion,
) -> Result<GenericRuntimeHeapAllocation, LoaderError> {
    let heap_handle = completion.frame.argument_u32(0).ok_or_else(|| {
        LoaderError::runtime_defect(
            "runtime-reentry",
            "heap-allocation-handle-missing",
            "completed HeapAlloc frame has no logical heap argument",
        )
    })?;
    let RegisterUpdate::Set(address) = completion.eax else {
        return Err(LoaderError::runtime_defect(
            "runtime-reentry",
            "heap-allocation-result-missing",
            "completed HeapAlloc did not return a guest address",
        ));
    };
    let allocation = session
        .runtime()
        .private_heap_allocation(heap_handle, GuestAddress(address))
        .map_err(runtime_state_error)?;
    let live_allocations = session
        .runtime()
        .heap(heap_handle)
        .map_err(runtime_state_error)?
        .live_allocations;
    Ok(GenericRuntimeHeapAllocation {
        heap_handle,
        allocation,
        live_allocations: usize::try_from(live_allocations).map_err(|_| {
            LoaderError::runtime_defect(
                "runtime-reentry",
                "heap-live-allocation-count-not-representable",
                "completed HeapAlloc live allocation count does not fit host accounting",
            )
        })?,
    })
}

fn generic_runtime_heap_free_before(
    runtime: &RuntimeState,
    memory: &GuestMemory,
    stack_pointer: u32,
) -> Result<GenericRuntimeHeapFreeBefore, LoaderError> {
    let heap_handle = read_generic_runtime_stack_u32(memory, stack_pointer, 0, "HeapFree heap")?;
    let address = GuestAddress(read_generic_runtime_stack_u32(
        memory,
        stack_pointer,
        2,
        "HeapFree allocation",
    )?);
    let allocation = runtime
        .private_heap_allocation(heap_handle, address)
        .map_err(runtime_state_error)?;
    let live_allocations = usize::try_from(
        runtime
            .heap(heap_handle)
            .map_err(runtime_state_error)?
            .live_allocations,
    )
    .map_err(|_| {
        LoaderError::runtime_defect(
            "runtime-reentry",
            "heap-live-allocation-count-not-representable",
            "pre-free live allocation count does not fit host accounting",
        )
    })?;
    Ok(GenericRuntimeHeapFreeBefore {
        heap_handle,
        allocation,
        live_allocations,
    })
}

fn generic_runtime_heap_free(
    session: &ExecutionSession<'_>,
    completion: &ApiImmediateCompletion,
    before: GenericRuntimeHeapFreeBefore,
) -> Result<GenericRuntimeHeapFree, LoaderError> {
    if completion.frame.argument_u32(0) != Some(before.heap_handle)
        || completion.frame.argument_u32(2) != Some(before.allocation.address.0)
        || !matches!(completion.eax, RegisterUpdate::Set(value) if value != 0)
    {
        return Err(LoaderError::runtime_defect(
            "runtime-reentry",
            "heap-free-completion-identity-mismatch",
            "completed HeapFree did not retire the preflighted heap allocation",
        ));
    }
    let live_allocations_after = usize::try_from(
        session
            .runtime()
            .heap(before.heap_handle)
            .map_err(runtime_state_error)?
            .live_allocations,
    )
    .map_err(|_| {
        LoaderError::runtime_defect(
            "runtime-reentry",
            "heap-live-allocation-count-not-representable",
            "post-free live allocation count does not fit host accounting",
        )
    })?;
    let allocation_retired = session
        .runtime()
        .private_heap_allocation(before.heap_handle, before.allocation.address)
        .is_err();
    if before.live_allocations.checked_sub(1) != Some(live_allocations_after) || !allocation_retired
    {
        return Err(LoaderError::runtime_defect(
            "runtime-reentry",
            "heap-free-lifecycle-mismatch",
            "completed HeapFree did not atomically retire exactly one live allocation",
        ));
    }
    Ok(GenericRuntimeHeapFree {
        heap_handle: before.heap_handle,
        allocation: before.allocation,
        live_allocations_before: before.live_allocations,
        live_allocations_after,
        allocation_retired,
    })
}

fn read_generic_runtime_stack_u32(
    memory: &GuestMemory,
    stack_pointer: u32,
    argument_index: u32,
    label: &str,
) -> Result<u32, LoaderError> {
    let byte_offset = argument_index
        .checked_add(1)
        .and_then(|index| index.checked_mul(4))
        .ok_or_else(|| {
            LoaderError::invalid_input(
                "runtime-reentry",
                "runtime-frame-address-overflow",
                format!("{label} argument offset crosses the x86 address limit"),
            )
        })?;
    let address = GuestAddress(stack_pointer.checked_add(byte_offset).ok_or_else(|| {
        LoaderError::invalid_input(
            "runtime-reentry",
            "runtime-frame-address-overflow",
            format!("{label} argument address crosses the x86 address limit"),
        )
    })?);
    let bytes = memory.read(address, 4).map_err(memory_error)?;
    Ok(u32::from_le_bytes(bytes.try_into().map_err(|_| {
        LoaderError::runtime_defect(
            "runtime-reentry",
            "runtime-frame-read-size-invalid",
            format!("checked {label} frame did not contain one DWORD"),
        )
    })?))
}

fn generic_runtime_standard_handle(
    session: &ExecutionSession<'_>,
    completion: &ApiImmediateCompletion,
) -> Result<RuntimeStandardHandle, LoaderError> {
    let RegisterUpdate::Set(handle) = completion.eax else {
        return Err(LoaderError::runtime_defect(
            "runtime-reentry",
            "standard-handle-result-missing",
            "completed GetStdHandle did not return a logical handle",
        ));
    };
    session
        .runtime()
        .standard_handle_by_value(handle)
        .cloned()
        .map_err(runtime_state_error)
}

fn generic_runtime_file_type(
    session: &ExecutionSession<'_>,
    completion: &ApiImmediateCompletion,
) -> Result<GenericRuntimeFileType, LoaderError> {
    let handle = completion.frame.argument_u32(0).ok_or_else(|| {
        LoaderError::runtime_defect(
            "runtime-reentry",
            "file-type-handle-missing",
            "completed GetFileType frame has no logical handle argument",
        )
    })?;
    let RegisterUpdate::Set(file_type) = completion.eax else {
        return Err(LoaderError::runtime_defect(
            "runtime-reentry",
            "file-type-result-missing",
            "completed GetFileType did not return a file type",
        ));
    };
    Ok(GenericRuntimeFileType {
        standard_handle: session
            .runtime()
            .standard_handle_by_value(handle)
            .cloned()
            .map_err(runtime_state_error)?,
        file_type,
    })
}

fn send_pending_runtime_delta(
    input: &mut impl BufRead,
    memory: &mut GuestMemory,
) -> Result<(), LoaderError> {
    if memory.stats().pending_invalidation.is_none() {
        return Ok(());
    }
    let delta = memory.pending_delta().map_err(memory_error)?;
    let generation = delta.generation;
    let buffers = delta.encode_binary().map_err(memory_error)?;
    write_runtime_frame(
        RUNTIME_FRAME_HOST_DELTA,
        &buffers.control,
        &buffers.page_bytes,
    )?;
    let acknowledgement = read_runtime_frame(input)?;
    require_runtime_frame_kind(&acknowledgement, RUNTIME_FRAME_ACKNOWLEDGEMENT)?;
    if !acknowledgement.payload.is_empty() || acknowledgement.control.len() != 8 {
        return Err(LoaderError::invalid_input(
            "runtime-reentry",
            "runtime-delta-ack-shape-invalid",
            "runtime host-delta acknowledgement must contain one generation and no payload",
        ));
    }
    let acknowledged =
        u64::from_le_bytes(acknowledgement.control[..8].try_into().map_err(|_| {
            LoaderError::runtime_defect(
                "runtime-reentry",
                "runtime-delta-ack-width-invalid",
                "validated runtime delta acknowledgement has an invalid width",
            )
        })?);
    if acknowledged != generation {
        return Err(LoaderError::invalid_input(
            "runtime-reentry",
            "runtime-delta-ack-generation-mismatch",
            "runtime host-delta acknowledgement names the wrong generation",
        ));
    }
    memory.acknowledge_delta(acknowledged).map_err(memory_error)
}

fn write_generic_runtime_fault(
    memory: &GuestMemory,
    endpoint: GenericRuntimeEndpoint,
    fault: ApiFault,
) -> Result<(), LoaderError> {
    let capability_cluster = runtime_capability_cluster(&endpoint);
    let memory_content_sha256 = memory
        .current_reflection()
        .map_err(memory_error)?
        .content_sha256;
    write_runtime_json_frame(
        RUNTIME_FRAME_API_FAULT,
        &GenericRuntimeFault {
            schema: "vine.v86-generic-runtime-fault",
            schema_version: 1,
            capability_cluster,
            endpoint,
            fault,
            memory: memory.stats(),
            memory_content_sha256,
        },
    )
}

fn write_generic_runtime_com_fault(
    memory: &GuestMemory,
    endpoint: GenericRuntimeEndpoint,
    fault: ComFault,
) -> Result<(), LoaderError> {
    let capability_cluster = runtime_capability_cluster(&endpoint);
    let memory_content_sha256 = memory
        .current_reflection()
        .map_err(memory_error)?
        .content_sha256;
    write_runtime_json_frame(
        RUNTIME_FRAME_API_FAULT,
        &GenericRuntimeComFault {
            schema: "vine.v86-generic-runtime-com-fault",
            schema_version: 1,
            capability_cluster,
            endpoint,
            fault,
            memory: memory.stats(),
            memory_content_sha256,
        },
    )
}

fn runtime_capability_cluster(endpoint: &GenericRuntimeEndpoint) -> String {
    match endpoint {
        GenericRuntimeEndpoint::ComMethod { target } if target.interface.name == "IDirect3D9" => {
            "d3d9-idirect3d9-method".to_owned()
        }
        GenericRuntimeEndpoint::ComMethod { target }
            if target.interface.name == "IDirect3DDevice9" =>
        {
            "d3d9-idirect3ddevice9-method".to_owned()
        }
        GenericRuntimeEndpoint::ComMethod { target }
            if target.interface.name == "IDirect3DTexture9" =>
        {
            "d3d9-idirect3dtexture9-method".to_owned()
        }
        GenericRuntimeEndpoint::ComMethod { target }
            if target.interface.name == "IDirect3DSurface9" =>
        {
            "d3d9-idirect3dsurface9-method".to_owned()
        }
        GenericRuntimeEndpoint::ComMethod { .. } => "com-method-dispatch".to_owned(),
        GenericRuntimeEndpoint::DynamicExport { endpoint }
            if endpoint.module.eq_ignore_ascii_case("kernel32.dll")
                && endpoint.name.starts_with("Fls") =>
        {
            "kernel32-fiber-local-storage".to_owned()
        }
        GenericRuntimeEndpoint::DynamicExport { .. } => "dynamic-export-dispatch".to_owned(),
        GenericRuntimeEndpoint::RuntimeSystemExport { endpoint } => {
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
                    if module == "kernel32.dll"
                        && matches!(name.as_str(), "HeapAlloc" | "HeapFree") =>
                {
                    "kernel32-private-heap-allocation".to_owned()
                }
                _ => format!("{}-api-contract", module.trim_end_matches(".dll")),
            }
        }
    }
}

fn encode_writeback_receipt(
    memory: &GuestMemory,
    receipt: &CpuWritebackReceipt,
) -> Result<Vec<u8>, LoaderError> {
    let mut bytes = Vec::with_capacity(120);
    bytes.extend_from_slice(b"VINEWR01");
    bytes.extend_from_slice(&1u32.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&receipt.from_generation.to_le_bytes());
    bytes.extend_from_slice(&receipt.generation.to_le_bytes());
    for count in [
        receipt.touched_pages,
        receipt.changed_pages,
        receipt.unchanged_pages,
        receipt.executable_pages,
    ] {
        bytes.extend_from_slice(
            &u32::try_from(count)
                .map_err(|_| {
                    LoaderError::resource_limit(
                        "runtime-reentry",
                        "writeback-receipt-count-overflow",
                        "CPU writeback receipt count does not fit the binary protocol",
                    )
                })?
                .to_le_bytes(),
        );
    }
    bytes.extend_from_slice(&receipt.changed_bytes.to_le_bytes());
    bytes.extend_from_slice(&receipt.transferred_bytes.to_le_bytes());
    if receipt.content_sha256.len() != 64 {
        return Err(LoaderError::runtime_defect(
            "runtime-reentry",
            "writeback-receipt-digest-invalid",
            "CPU writeback receipt digest does not contain 32 bytes",
        ));
    }
    for index in (0..receipt.content_sha256.len()).step_by(2) {
        bytes.push(
            u8::from_str_radix(&receipt.content_sha256[index..index + 2], 16).map_err(|_| {
                LoaderError::runtime_defect(
                    "runtime-reentry",
                    "writeback-receipt-digest-invalid",
                    "CPU writeback receipt digest is not hexadecimal",
                )
            })?,
        );
    }
    let stats = memory.stats();
    bytes.extend_from_slice(&stats.generation.to_le_bytes());
    bytes.extend_from_slice(&stats.acknowledged_generation.to_le_bytes());
    bytes.extend_from_slice(
        &u32::try_from(stats.dirty_pages)
            .map_err(|_| {
                LoaderError::resource_limit(
                    "runtime-reentry",
                    "writeback-receipt-dirty-count-overflow",
                    "guest-memory dirty-page count does not fit the binary protocol",
                )
            })?
            .to_le_bytes(),
    );
    bytes.extend_from_slice(&u32::from(stats.pending_invalidation.is_some()).to_le_bytes());
    if bytes.len() != 120 {
        return Err(LoaderError::runtime_defect(
            "runtime-reentry",
            "writeback-receipt-size-invalid",
            "CPU writeback receipt did not produce the fixed binary size",
        ));
    }
    Ok(bytes)
}

fn read_runtime_frame(input: &mut impl Read) -> Result<RuntimeFrame, LoaderError> {
    let mut header = [0u8; RUNTIME_FRAME_HEADER_BYTES];
    input.read_exact(&mut header).map_err(|error| {
        io_error(
            "runtime-reentry",
            "runtime-frame-header-read-failed",
            "could not read a generic runtime frame header",
            error,
        )
    })?;
    if header[..8] != *RUNTIME_FRAME_MAGIC {
        return Err(LoaderError::invalid_input(
            "runtime-reentry",
            "runtime-frame-magic-mismatch",
            "generic runtime frame magic is invalid",
        ));
    }
    let version = u32::from_le_bytes(header[8..12].try_into().map_err(|_| {
        LoaderError::runtime_defect(
            "runtime-reentry",
            "runtime-frame-version-width-invalid",
            "validated runtime frame version has an invalid width",
        )
    })?);
    if version != RUNTIME_FRAME_VERSION {
        return Err(LoaderError::unsupported(
            "runtime-reentry",
            "runtime-frame-version-unsupported",
            "generic runtime frame version is unsupported",
        ));
    }
    let kind = u32::from_le_bytes(header[12..16].try_into().map_err(|_| {
        LoaderError::runtime_defect(
            "runtime-reentry",
            "runtime-frame-kind-width-invalid",
            "validated runtime frame kind has an invalid width",
        )
    })?);
    let control_bytes = u32::from_le_bytes(header[16..20].try_into().map_err(|_| {
        LoaderError::runtime_defect(
            "runtime-reentry",
            "runtime-frame-control-width-invalid",
            "validated runtime frame control length has an invalid width",
        )
    })?) as usize;
    let payload_bytes = u32::from_le_bytes(header[20..24].try_into().map_err(|_| {
        LoaderError::runtime_defect(
            "runtime-reentry",
            "runtime-frame-payload-width-invalid",
            "validated runtime frame payload length has an invalid width",
        )
    })?) as usize;
    if control_bytes > RUNTIME_FRAME_MAX_CONTROL_BYTES
        || payload_bytes > RUNTIME_FRAME_MAX_PAYLOAD_BYTES
    {
        return Err(LoaderError::resource_limit(
            "runtime-reentry",
            "runtime-frame-size-exceeded",
            format!(
                "generic runtime frame kind {kind} has {} control bytes and {} payload bytes; bounds are {} and {}",
                control_bytes,
                payload_bytes,
                RUNTIME_FRAME_MAX_CONTROL_BYTES,
                RUNTIME_FRAME_MAX_PAYLOAD_BYTES,
            ),
        ));
    }
    let mut control = vec![0; control_bytes];
    let mut payload = vec![0; payload_bytes];
    input.read_exact(&mut control).map_err(|error| {
        io_error(
            "runtime-reentry",
            "runtime-frame-control-read-failed",
            "could not read generic runtime frame control bytes",
            error,
        )
    })?;
    input.read_exact(&mut payload).map_err(|error| {
        io_error(
            "runtime-reentry",
            "runtime-frame-payload-read-failed",
            "could not read generic runtime frame payload bytes",
            error,
        )
    })?;
    Ok(RuntimeFrame {
        kind,
        control,
        payload,
    })
}

fn require_runtime_frame_kind(frame: &RuntimeFrame, expected: u32) -> Result<(), LoaderError> {
    if frame.kind != expected {
        return Err(LoaderError::invalid_input(
            "runtime-reentry",
            "runtime-frame-kind-mismatch",
            format!(
                "generic runtime frame kind {} does not match expected kind {expected}",
                frame.kind
            ),
        ));
    }
    Ok(())
}

fn write_runtime_json_frame(kind: u32, value: &impl Serialize) -> Result<(), LoaderError> {
    write_runtime_json_payload_frame(kind, value, &[])
}

fn write_runtime_json_payload_frame(
    kind: u32,
    value: &impl Serialize,
    payload: &[u8],
) -> Result<(), LoaderError> {
    let control = serde_json::to_vec(value).map_err(|error| {
        LoaderError::runtime_defect(
            "runtime-reentry",
            "runtime-frame-json-serialization-failed",
            format!("could not serialize generic runtime frame metadata: {error}"),
        )
    })?;
    write_runtime_frame(kind, &control, payload)
}

fn decode_runtime_host_payload(payload: &[u8]) -> Result<Vec<u32>, LoaderError> {
    if !payload.len().is_multiple_of(4) || payload.len() > 64 * 4 {
        return Err(LoaderError::invalid_input(
            "runtime-reentry",
            "runtime-host-request-payload-invalid",
            "runtime host request payload must contain at most 64 little-endian words",
        ));
    }
    Ok(payload
        .chunks_exact(4)
        .map(|word| {
            u32::from_le_bytes(
                word.try_into()
                    .expect("four-byte runtime host request word"),
            )
        })
        .collect())
}

fn write_runtime_frame(kind: u32, control: &[u8], payload: &[u8]) -> Result<(), LoaderError> {
    if control.len() > RUNTIME_FRAME_MAX_CONTROL_BYTES
        || payload.len() > RUNTIME_FRAME_MAX_PAYLOAD_BYTES
    {
        return Err(LoaderError::resource_limit(
            "runtime-reentry",
            "runtime-frame-size-exceeded",
            format!(
                "generic runtime frame kind {kind} has {} control bytes and {} payload bytes; bounds are {} and {}",
                control.len(),
                payload.len(),
                RUNTIME_FRAME_MAX_CONTROL_BYTES,
                RUNTIME_FRAME_MAX_PAYLOAD_BYTES,
            ),
        ));
    }
    let control_bytes = u32::try_from(control.len()).map_err(|_| {
        LoaderError::resource_limit(
            "runtime-reentry",
            "runtime-frame-control-length-overflow",
            "generic runtime frame control length does not fit 32 bits",
        )
    })?;
    let payload_bytes = u32::try_from(payload.len()).map_err(|_| {
        LoaderError::resource_limit(
            "runtime-reentry",
            "runtime-frame-payload-length-overflow",
            "generic runtime frame payload length does not fit 32 bits",
        )
    })?;
    let mut header = Vec::with_capacity(RUNTIME_FRAME_HEADER_BYTES);
    header.extend_from_slice(RUNTIME_FRAME_MAGIC);
    header.extend_from_slice(&RUNTIME_FRAME_VERSION.to_le_bytes());
    header.extend_from_slice(&kind.to_le_bytes());
    header.extend_from_slice(&control_bytes.to_le_bytes());
    header.extend_from_slice(&payload_bytes.to_le_bytes());
    let mut output = io::stdout().lock();
    output.write_all(&header).map_err(|error| {
        io_error(
            "runtime-reentry",
            "runtime-frame-header-write-failed",
            "could not write generic runtime frame header",
            error,
        )
    })?;
    output.write_all(control).map_err(|error| {
        io_error(
            "runtime-reentry",
            "runtime-frame-control-write-failed",
            "could not write generic runtime frame control bytes",
            error,
        )
    })?;
    output.write_all(payload).map_err(|error| {
        io_error(
            "runtime-reentry",
            "runtime-frame-payload-write-failed",
            "could not write generic runtime frame payload bytes",
            error,
        )
    })?;
    output.flush().map_err(|error| {
        io_error(
            "runtime-reentry",
            "runtime-frame-flush-failed",
            "could not flush generic runtime frame",
            error,
        )
    })
}

fn read_v86_reflection_ack(input: &mut impl BufRead) -> Result<V86ReflectionAck, LoaderError> {
    read_protocol_message(
        input,
        4 * 1024,
        "memory-reflection",
        "reflection-ack",
        "adapter invalidation acknowledgement",
    )
}

fn validate_v86_reflection_ack(
    ack: &V86ReflectionAck,
    pending: MemoryInvalidation,
) -> Result<(), LoaderError> {
    if ack.schema != "vine.v86-memory-reflection-ack" || ack.schema_version != 1 {
        return Err(LoaderError::invalid_input(
            "memory-reflection",
            "reflection-ack-schema-unsupported",
            "adapter acknowledgement must use vine.v86-memory-reflection-ack version 1",
        ));
    }
    if ack.generation != pending.generation {
        return Err(LoaderError::invalid_input(
            "memory-reflection",
            "reflection-ack-generation-mismatch",
            "adapter acknowledgement does not match the pending memory generation",
        ));
    }
    if (pending.data_tlb && !ack.data_tlb)
        || (pending.instruction_fetch && !ack.instruction_fetch)
        || (pending.generated_code && !ack.generated_code)
    {
        return Err(LoaderError::invalid_input(
            "memory-reflection",
            "reflection-ack-incomplete",
            "adapter acknowledgement did not complete every required invalidation class",
        ));
    }
    Ok(())
}

fn run_v86_system_time_delta_session(
    input: &mut impl BufRead,
    memory: &mut GuestMemory,
    runtime: &mut RuntimeState,
    binding: &ImportBinding,
    message_limit: usize,
    return_sessions: V86ReturnSessionOptions,
) -> Result<(), LoaderError> {
    let request: V86SystemTimeWriteRequest = read_protocol_message(
        input,
        64 * 1024,
        "win32-system-time",
        "system-time-write-request",
        "controlled system-time write request",
    )?;
    if request.schema != "vine.v86-system-time-write-request" || request.schema_version != 1 {
        return Err(LoaderError::invalid_input(
            "win32-system-time",
            "system-time-write-request-schema-unsupported",
            "system-time write request must use vine.v86-system-time-write-request version 1",
        ));
    }
    if request.argument_bytes != 4 {
        return Err(LoaderError::invalid_input(
            "abi-gateway",
            "system-time-argument-width-mismatch",
            "GetSystemTimeAsFileTime requires one four-byte stdcall argument",
        ));
    }
    let thunk = binding
        .thunks
        .iter()
        .find(|thunk| thunk.id.0 == request.thunk_id)
        .ok_or_else(|| {
            LoaderError::invalid_input(
                "thunk-dispatch",
                "system-time-thunk-id-unowned",
                "system-time write request does not name an owned import thunk",
            )
        })?
        .clone();
    if thunk.address.0 != request.thunk_address
        || !thunk.module.eq_ignore_ascii_case("KERNEL32.dll")
        || thunk.disposition != ImportThunkDisposition::DeclaredRuntimeExport
        || !matches!(
            &thunk.target,
            ImportTarget::Name { name, .. } if name == "GetSystemTimeAsFileTime"
        )
    {
        return Err(LoaderError::invalid_input(
            "thunk-dispatch",
            "system-time-thunk-identity-mismatch",
            "system-time write request does not match the declared GetSystemTimeAsFileTime thunk",
        ));
    }
    let contract =
        get_system_time_as_file_time_contract(ApiEndpointIdentity::from_import_thunk(&thunk))
            .map_err(api_fault)?;

    let reported_file_time =
        (u64::from(request.file_time_high) << 32) | u64::from(request.file_time_low);
    let file_time_ticks = runtime.clock().wall_clock_file_time;
    if reported_file_time != file_time_ticks {
        return Err(LoaderError::invalid_input(
            "win32-system-time",
            "system-time-evidence-runtime-mismatch",
            "system-time request evidence does not match the Rust runtime clock",
        ));
    }
    let file_time_bytes = file_time_ticks.to_le_bytes();
    let (stack_reconciliation, write, delta) = {
        let mut transaction = memory.begin_transaction();
        let stack_reconciliation = transaction
            .reconcile_reflected_page(
                request.reflected_generation,
                MemoryOwnerId(request.stack_page_owner),
                GuestAddress(request.stack_page_address),
                &request.stack_page_bytes,
            )
            .map_err(memory_error)?;

        validate_direct_iat_callsite(&transaction, &thunk, request.return_address)?;
        let completion = dispatch_runtime_immediate(
            &mut transaction,
            runtime,
            binding,
            contract,
            controlled_api_trap(
                request.thunk_id,
                request.thunk_address,
                request.stack_pointer,
                request.return_address,
                request.argument_bytes,
            ),
        )?;
        if completion.eax != RegisterUpdate::Preserve
            || completion.frame.argument_u32(0) != Some(request.output_address)
        {
            return Err(LoaderError::runtime_defect(
                "api-dispatch",
                "system-time-completion-invalid",
                "GetSystemTimeAsFileTime dispatcher did not preserve EAX and decode its output",
            ));
        }
        let write = completion
            .guest_writes
            .first()
            .filter(|_| completion.guest_writes.len() == 1)
            .map(|receipt| receipt.result)
            .ok_or_else(|| {
                LoaderError::runtime_defect(
                    "api-dispatch",
                    "system-time-write-receipt-invalid",
                    "GetSystemTimeAsFileTime dispatcher did not produce one guest write",
                )
            })?;
        let delta = transaction.pending_delta().map_err(memory_error)?;
        if delta.from_generation != request.reflected_generation
            || delta.dirty_pages.len() != 1
            || delta.dirty_pages[0].address.0 != request.stack_page_address
            || delta.dirty_pages[0].owner.0 != request.stack_page_owner
            || !delta.mapping_operations.is_empty()
            || delta.cache_effects.flush_tlb
            || delta.cache_effects.invalidate_instruction_fetch
            || delta.cache_effects.invalidate_generated_code
        {
            return Err(LoaderError::runtime_defect(
                "memory-delta",
                "system-time-delta-shape-invalid",
                "checked FILETIME write did not produce one data-only stack-page delta",
            ));
        }
        transaction.commit_transaction();
        (stack_reconciliation, write, delta)
    };

    let delta_generation = delta.generation;
    write_protocol_message(
        &V86SystemTimeDeltaOffer {
            schema: "vine.v86-system-time-delta",
            schema_version: 1,
            thunk,
            calling_convention: "stdcall",
            argument_bytes: request.argument_bytes,
            stack_pointer: GuestAddress(request.stack_pointer),
            return_address: GuestAddress(request.return_address),
            output_address: GuestAddress(request.output_address),
            clock_owner: request.clock_owner,
            clock_sample_id: request.clock_sample_id,
            clock_utc_iso: request.clock_utc_iso,
            file_time_ticks: file_time_ticks.to_string(),
            file_time_low: request.file_time_low,
            file_time_high: request.file_time_high,
            stack_reconciliation,
            write,
            delta,
        },
        message_limit,
    )?;

    let ack: V86MemoryDeltaAck = read_protocol_message(
        input,
        4 * 1024,
        "memory-delta",
        "system-time-delta-ack",
        "system-time memory-delta acknowledgement",
    )?;
    if ack.schema != "vine.v86-memory-delta-ack"
        || ack.schema_version != 1
        || ack.generation != delta_generation
        || ack.data_tlb
        || ack.instruction_fetch
        || ack.generated_code
    {
        return Err(LoaderError::invalid_input(
            "memory-delta",
            "system-time-delta-ack-invalid",
            "system-time delta acknowledgement does not match its generation and cache effects",
        ));
    }
    memory
        .acknowledge_invalidation(ack.generation)
        .map_err(memory_error)?;
    let output_bytes = memory
        .read(GuestAddress(request.output_address), file_time_bytes.len())
        .map_err(memory_error)?;
    write_protocol_message(
        &V86SystemTimeDeltaReceipt {
            schema: "vine.v86-system-time-delta-receipt",
            schema_version: 1,
            acknowledged_generation: ack.generation,
            output_address: GuestAddress(request.output_address),
            output_bytes,
            stats: memory.stats(),
        },
        message_limit,
    )?;
    if return_sessions.includes(2) {
        run_v86_process_id_return_session(
            input,
            memory,
            runtime,
            binding,
            message_limit,
            return_sessions,
        )?;
    }
    Ok(())
}

fn run_v86_process_id_return_session(
    input: &mut impl BufRead,
    memory: &mut GuestMemory,
    runtime: &mut RuntimeState,
    binding: &ImportBinding,
    message_limit: usize,
    return_sessions: V86ReturnSessionOptions,
) -> Result<(), LoaderError> {
    let request: V86ProcessIdReturnRequest = read_protocol_message(
        input,
        64 * 1024,
        "win32-process-identity",
        "process-id-return-request",
        "controlled process-ID return request",
    )?;
    if request.schema != "vine.v86-process-id-return-request" || request.schema_version != 1 {
        return Err(LoaderError::invalid_input(
            "win32-process-identity",
            "process-id-return-request-schema-unsupported",
            "process-ID request must use vine.v86-process-id-return-request version 1",
        ));
    }
    if request.argument_bytes != 0 {
        return Err(LoaderError::invalid_input(
            "abi-gateway",
            "process-id-argument-width-mismatch",
            "GetCurrentProcessId requires a zero-argument stdcall frame",
        ));
    }
    if request.process_identity_owner.is_empty() || request.process_id == 0 {
        return Err(LoaderError::invalid_input(
            "win32-process-identity",
            "controlled-process-identity-invalid",
            "controlled process identity requires a nonempty owner and nonzero process ID",
        ));
    }
    if request.process_id != runtime.process_id().0 {
        return Err(LoaderError::invalid_input(
            "win32-process-identity",
            "process-id-evidence-runtime-mismatch",
            "process-ID request evidence does not match the Rust runtime process",
        ));
    }
    let thunk = owned_import_thunk(binding, request.thunk_id)?;
    let contract = get_current_process_id_contract(ApiEndpointIdentity::from_import_thunk(&thunk))
        .map_err(api_fault)?;

    let (stack_reconciliation, completion) = {
        let mut transaction = memory.begin_transaction();
        let reconciliation = transaction
            .reconcile_reflected_page(
                request.reflected_generation,
                MemoryOwnerId(request.stack_page_owner),
                GuestAddress(request.stack_page_address),
                &request.stack_page_bytes,
            )
            .map_err(memory_error)?;
        validate_direct_iat_callsite(&transaction, &thunk, request.return_address)?;
        let completion = dispatch_runtime_immediate(
            &mut transaction,
            runtime,
            binding,
            contract,
            controlled_api_trap(
                request.thunk_id,
                request.thunk_address,
                request.stack_pointer,
                request.return_address,
                request.argument_bytes,
            ),
        )?;
        if !completion.frame.arguments.is_empty() {
            return Err(LoaderError::runtime_defect(
                "api-dispatch",
                "process-id-frame-shape-invalid",
                "GetCurrentProcessId dispatcher frame unexpectedly contains arguments",
            ));
        }
        transaction.commit_transaction();
        (reconciliation, completion)
    };
    let process_id = set_eax_result(
        &completion,
        "process-id-return-register-invalid",
        "GetCurrentProcessId dispatcher did not set EAX",
    )?;

    write_protocol_message(
        &V86ProcessIdReturnReceipt {
            schema: "vine.v86-process-id-return-receipt",
            schema_version: 1,
            thunk,
            calling_convention: "stdcall",
            argument_bytes: request.argument_bytes,
            stack_pointer: GuestAddress(request.stack_pointer),
            return_address: GuestAddress(request.return_address),
            process_identity_owner: request.process_identity_owner,
            process_id,
            stack_reconciliation,
            stats: memory.stats(),
        },
        message_limit,
    )?;
    if return_sessions.includes(3) {
        run_v86_thread_id_return_session(
            input,
            memory,
            runtime,
            binding,
            message_limit,
            return_sessions,
        )?;
    }
    Ok(())
}

fn run_v86_thread_id_return_session(
    input: &mut impl BufRead,
    memory: &mut GuestMemory,
    runtime: &mut RuntimeState,
    binding: &ImportBinding,
    message_limit: usize,
    return_sessions: V86ReturnSessionOptions,
) -> Result<(), LoaderError> {
    let request: V86ThreadIdReturnRequest = read_protocol_message(
        input,
        64 * 1024,
        "win32-thread-identity",
        "thread-id-return-request",
        "controlled thread-ID return request",
    )?;
    if request.schema != "vine.v86-thread-id-return-request" || request.schema_version != 1 {
        return Err(LoaderError::invalid_input(
            "win32-thread-identity",
            "thread-id-return-request-schema-unsupported",
            "thread-ID request must use vine.v86-thread-id-return-request version 1",
        ));
    }
    if request.argument_bytes != 0 {
        return Err(LoaderError::invalid_input(
            "abi-gateway",
            "thread-id-argument-width-mismatch",
            "GetCurrentThreadId requires a zero-argument stdcall frame",
        ));
    }
    if request.thread_identity_owner.is_empty() || request.thread_id == 0 {
        return Err(LoaderError::invalid_input(
            "win32-thread-identity",
            "controlled-thread-identity-invalid",
            "controlled thread identity requires a nonempty owner and nonzero thread ID",
        ));
    }
    let thread_id = runtime.current_thread_id().0;
    if request.thread_id != thread_id {
        return Err(LoaderError::invalid_input(
            "win32-thread-identity",
            "thread-id-evidence-runtime-mismatch",
            "thread-ID request evidence does not match the Rust runtime scheduler",
        ));
    }
    let thunk = binding
        .thunks
        .iter()
        .find(|thunk| thunk.id.0 == request.thunk_id)
        .ok_or_else(|| {
            LoaderError::invalid_input(
                "thunk-dispatch",
                "thread-id-thunk-id-unowned",
                "thread-ID request does not name an owned import thunk",
            )
        })?
        .clone();
    if thunk.address.0 != request.thunk_address
        || !thunk.module.eq_ignore_ascii_case("KERNEL32.dll")
        || thunk.disposition != ImportThunkDisposition::DeclaredRuntimeExport
        || !matches!(
            &thunk.target,
            ImportTarget::Name { name, .. } if name == "GetCurrentThreadId"
        )
    {
        return Err(LoaderError::invalid_input(
            "thunk-dispatch",
            "thread-id-thunk-identity-mismatch",
            "thread-ID request does not match the declared GetCurrentThreadId thunk",
        ));
    }
    let contract = get_current_thread_id_contract(ApiEndpointIdentity::from_import_thunk(&thunk))
        .map_err(api_fault)?;

    let (stack_reconciliation, dispatched_thread_id) = {
        let mut transaction = memory.begin_transaction();
        let reconciliation = transaction
            .reconcile_reflected_page(
                request.reflected_generation,
                MemoryOwnerId(request.stack_page_owner),
                GuestAddress(request.stack_page_address),
                &request.stack_page_bytes,
            )
            .map_err(memory_error)?;
        validate_direct_iat_callsite(&transaction, &thunk, request.return_address)?;
        let completion = dispatch_runtime_immediate(
            &mut transaction,
            runtime,
            binding,
            contract,
            controlled_api_trap(
                request.thunk_id,
                request.thunk_address,
                request.stack_pointer,
                request.return_address,
                request.argument_bytes,
            ),
        )?;
        let dispatched_thread_id = set_eax_result(
            &completion,
            "thread-id-return-register-invalid",
            "GetCurrentThreadId dispatcher did not set EAX",
        )?;
        transaction.commit_transaction();
        (reconciliation, dispatched_thread_id)
    };
    if dispatched_thread_id != thread_id {
        return Err(LoaderError::runtime_defect(
            "api-dispatch",
            "thread-id-runtime-result-mismatch",
            "GetCurrentThreadId dispatcher disagreed with the runtime scheduler",
        ));
    }

    write_protocol_message(
        &V86ThreadIdReturnReceipt {
            schema: "vine.v86-thread-id-return-receipt",
            schema_version: 1,
            thunk,
            calling_convention: "stdcall",
            argument_bytes: request.argument_bytes,
            stack_pointer: GuestAddress(request.stack_pointer),
            return_address: GuestAddress(request.return_address),
            thread_identity_owner: request.thread_identity_owner,
            thread_id: dispatched_thread_id,
            stack_reconciliation,
            stats: memory.stats(),
        },
        message_limit,
    )?;
    if return_sessions.includes(4) {
        run_v86_tick_count_return_session(
            input,
            memory,
            runtime,
            binding,
            message_limit,
            return_sessions,
        )?;
    }
    Ok(())
}

fn run_v86_tick_count_return_session(
    input: &mut impl BufRead,
    memory: &mut GuestMemory,
    runtime: &mut RuntimeState,
    binding: &ImportBinding,
    message_limit: usize,
    return_sessions: V86ReturnSessionOptions,
) -> Result<(), LoaderError> {
    let request: V86TickCountReturnRequest = read_protocol_message(
        input,
        64 * 1024,
        "win32-monotonic-clock",
        "tick-count-return-request",
        "controlled tick-count return request",
    )?;
    if request.schema != "vine.v86-tick-count-return-request" || request.schema_version != 1 {
        return Err(LoaderError::invalid_input(
            "win32-monotonic-clock",
            "tick-count-return-request-schema-unsupported",
            "tick-count request must use vine.v86-tick-count-return-request version 1",
        ));
    }
    if request.argument_bytes != 0 {
        return Err(LoaderError::invalid_input(
            "abi-gateway",
            "tick-count-argument-width-mismatch",
            "GetTickCount requires a zero-argument stdcall frame",
        ));
    }
    if request.monotonic_clock_owner.is_empty()
        || request.sample_id.is_empty()
        || request.paired_wall_clock_sample_id.is_empty()
    {
        return Err(LoaderError::invalid_input(
            "win32-monotonic-clock",
            "controlled-tick-count-sample-invalid",
            "controlled tick-count sample requires nonempty clock ownership and sample identities",
        ));
    }
    let tick_count = runtime.clock().tick_count;
    if request.tick_count != tick_count {
        return Err(LoaderError::invalid_input(
            "win32-monotonic-clock",
            "tick-count-evidence-runtime-mismatch",
            "tick-count request evidence does not match the Rust runtime clock",
        ));
    }
    let thunk = binding
        .thunks
        .iter()
        .find(|thunk| thunk.id.0 == request.thunk_id)
        .ok_or_else(|| {
            LoaderError::invalid_input(
                "thunk-dispatch",
                "tick-count-thunk-id-unowned",
                "tick-count request does not name an owned import thunk",
            )
        })?
        .clone();
    if thunk.address.0 != request.thunk_address
        || !thunk.module.eq_ignore_ascii_case("KERNEL32.dll")
        || thunk.disposition != ImportThunkDisposition::DeclaredRuntimeExport
        || !matches!(
            &thunk.target,
            ImportTarget::Name { name, .. } if name == "GetTickCount"
        )
    {
        return Err(LoaderError::invalid_input(
            "thunk-dispatch",
            "tick-count-thunk-identity-mismatch",
            "tick-count request does not match the declared GetTickCount thunk",
        ));
    }
    let contract = get_tick_count_contract(ApiEndpointIdentity::from_import_thunk(&thunk))
        .map_err(api_fault)?;

    let (stack_reconciliation, dispatched_tick_count) = {
        let mut transaction = memory.begin_transaction();
        let reconciliation = transaction
            .reconcile_reflected_page(
                request.reflected_generation,
                MemoryOwnerId(request.stack_page_owner),
                GuestAddress(request.stack_page_address),
                &request.stack_page_bytes,
            )
            .map_err(memory_error)?;
        validate_direct_iat_callsite(&transaction, &thunk, request.return_address)?;
        let completion = dispatch_runtime_immediate(
            &mut transaction,
            runtime,
            binding,
            contract,
            controlled_api_trap(
                request.thunk_id,
                request.thunk_address,
                request.stack_pointer,
                request.return_address,
                request.argument_bytes,
            ),
        )?;
        let dispatched_tick_count = set_eax_result(
            &completion,
            "tick-count-return-register-invalid",
            "GetTickCount dispatcher did not set EAX",
        )?;
        transaction.commit_transaction();
        (reconciliation, dispatched_tick_count)
    };
    if dispatched_tick_count != tick_count {
        return Err(LoaderError::runtime_defect(
            "api-dispatch",
            "tick-count-runtime-result-mismatch",
            "GetTickCount dispatcher disagreed with the runtime clock",
        ));
    }

    write_protocol_message(
        &V86TickCountReturnReceipt {
            schema: "vine.v86-tick-count-return-receipt",
            schema_version: 1,
            thunk,
            calling_convention: "stdcall",
            argument_bytes: request.argument_bytes,
            stack_pointer: GuestAddress(request.stack_pointer),
            return_address: GuestAddress(request.return_address),
            monotonic_clock_owner: request.monotonic_clock_owner,
            sample_id: request.sample_id,
            paired_wall_clock_sample_id: request.paired_wall_clock_sample_id,
            tick_count: dispatched_tick_count,
            stack_reconciliation,
            stats: memory.stats(),
        },
        message_limit,
    )?;
    if return_sessions.includes(5) {
        run_v86_performance_counter_return_session(
            input,
            memory,
            runtime,
            binding,
            message_limit,
            return_sessions,
        )?;
    }
    Ok(())
}

fn run_v86_performance_counter_return_session(
    input: &mut impl BufRead,
    memory: &mut GuestMemory,
    runtime: &mut RuntimeState,
    binding: &ImportBinding,
    message_limit: usize,
    return_sessions: V86ReturnSessionOptions,
) -> Result<(), LoaderError> {
    let request: V86PerformanceCounterWriteRequest = read_protocol_message(
        input,
        64 * 1024,
        "win32-performance-counter",
        "performance-counter-write-request",
        "controlled performance-counter write request",
    )?;
    if request.schema != "vine.v86-performance-counter-write-request" || request.schema_version != 1
    {
        return Err(LoaderError::invalid_input(
            "win32-performance-counter",
            "performance-counter-write-request-schema-unsupported",
            "performance-counter request must use vine.v86-performance-counter-write-request version 1",
        ));
    }
    if request.argument_bytes != 4 {
        return Err(LoaderError::invalid_input(
            "abi-gateway",
            "performance-counter-argument-width-mismatch",
            "QueryPerformanceCounter requires one four-byte stdcall argument",
        ));
    }
    if request.monotonic_clock_owner.is_empty()
        || request.sample_id.is_empty()
        || request.paired_tick_sample_id.is_empty()
    {
        return Err(LoaderError::invalid_input(
            "win32-performance-counter",
            "controlled-performance-counter-sample-invalid",
            "controlled performance-counter sample requires nonempty clock ownership and sample identities",
        ));
    }
    let thunk = owned_import_thunk(binding, request.thunk_id)?;
    let contract =
        query_performance_counter_contract(ApiEndpointIdentity::from_import_thunk(&thunk))
            .map_err(api_fault)?;

    let reported_counter = (u64::from(request.counter_high) << 32) | u64::from(request.counter_low);
    let counter_ticks = runtime.clock().performance_counter;
    if reported_counter != counter_ticks {
        return Err(LoaderError::invalid_input(
            "win32-performance-counter",
            "performance-counter-evidence-runtime-mismatch",
            "performance-counter request evidence does not match the Rust runtime clock",
        ));
    }
    let counter_bytes = counter_ticks.to_le_bytes();
    let (stack_reconciliation, write, delta) = {
        let mut transaction = memory.begin_transaction();
        let stack_reconciliation = transaction
            .reconcile_reflected_page(
                request.reflected_generation,
                MemoryOwnerId(request.stack_page_owner),
                GuestAddress(request.stack_page_address),
                &request.stack_page_bytes,
            )
            .map_err(memory_error)?;
        validate_direct_iat_callsite(&transaction, &thunk, request.return_address)?;
        let completion = dispatch_runtime_immediate(
            &mut transaction,
            runtime,
            binding,
            contract,
            controlled_api_trap(
                request.thunk_id,
                request.thunk_address,
                request.stack_pointer,
                request.return_address,
                request.argument_bytes,
            ),
        )?;
        let write = completion
            .guest_writes
            .first()
            .filter(|_| completion.guest_writes.len() == 1)
            .map(|receipt| receipt.result)
            .ok_or_else(|| {
                LoaderError::runtime_defect(
                    "api-dispatch",
                    "performance-counter-write-receipt-invalid",
                    "QueryPerformanceCounter dispatcher did not produce one guest write",
                )
            })?;
        if set_eax_result(
            &completion,
            "performance-counter-return-register-invalid",
            "QueryPerformanceCounter dispatcher did not set EAX",
        )? != 1
        {
            return Err(LoaderError::runtime_defect(
                "api-dispatch",
                "performance-counter-result-invalid",
                "QueryPerformanceCounter dispatcher did not return canonical TRUE",
            ));
        }
        let delta = transaction.pending_delta().map_err(memory_error)?;
        if delta.from_generation != request.reflected_generation
            || delta.dirty_pages.len() != 1
            || delta.dirty_pages[0].address.0 != request.stack_page_address
            || delta.dirty_pages[0].owner.0 != request.stack_page_owner
            || !delta.mapping_operations.is_empty()
            || delta.cache_effects.flush_tlb
            || delta.cache_effects.invalidate_instruction_fetch
            || delta.cache_effects.invalidate_generated_code
        {
            return Err(LoaderError::runtime_defect(
                "memory-delta",
                "performance-counter-delta-shape-invalid",
                "checked performance-counter write did not produce one data-only stack-page delta",
            ));
        }
        transaction.commit_transaction();
        (stack_reconciliation, write, delta)
    };

    let delta_generation = delta.generation;
    write_protocol_message(
        &V86PerformanceCounterDeltaOffer {
            schema: "vine.v86-performance-counter-delta",
            schema_version: 1,
            thunk,
            calling_convention: "stdcall",
            argument_bytes: request.argument_bytes,
            stack_pointer: GuestAddress(request.stack_pointer),
            return_address: GuestAddress(request.return_address),
            output_address: GuestAddress(request.output_address),
            monotonic_clock_owner: request.monotonic_clock_owner,
            sample_id: request.sample_id,
            paired_tick_sample_id: request.paired_tick_sample_id,
            counter_ticks: counter_ticks.to_string(),
            counter_low: request.counter_low,
            counter_high: request.counter_high,
            stack_reconciliation,
            write,
            delta,
        },
        message_limit,
    )?;

    let ack: V86MemoryDeltaAck = read_protocol_message(
        input,
        4 * 1024,
        "memory-delta",
        "performance-counter-delta-ack",
        "performance-counter memory-delta acknowledgement",
    )?;
    if ack.schema != "vine.v86-memory-delta-ack"
        || ack.schema_version != 1
        || ack.generation != delta_generation
        || ack.data_tlb
        || ack.instruction_fetch
        || ack.generated_code
    {
        return Err(LoaderError::invalid_input(
            "memory-delta",
            "performance-counter-delta-ack-invalid",
            "performance-counter delta acknowledgement does not match its generation and cache effects",
        ));
    }
    memory
        .acknowledge_invalidation(ack.generation)
        .map_err(memory_error)?;
    let output_bytes = memory
        .read(GuestAddress(request.output_address), counter_bytes.len())
        .map_err(memory_error)?;
    write_protocol_message(
        &V86PerformanceCounterDeltaReceipt {
            schema: "vine.v86-performance-counter-delta-receipt",
            schema_version: 1,
            acknowledged_generation: ack.generation,
            output_address: GuestAddress(request.output_address),
            output_bytes,
            stats: memory.stats(),
        },
        message_limit,
    )?;
    if return_sessions.includes(6) {
        run_v86_startup_info_return_session(
            input,
            memory,
            runtime,
            binding,
            message_limit,
            return_sessions,
        )?;
    }
    Ok(())
}

fn run_v86_startup_info_return_session(
    input: &mut impl BufRead,
    memory: &mut GuestMemory,
    runtime: &mut RuntimeState,
    binding: &ImportBinding,
    message_limit: usize,
    return_sessions: V86ReturnSessionOptions,
) -> Result<(), LoaderError> {
    let request: V86StartupInfoWriteRequest = read_protocol_message(
        input,
        64 * 1024,
        "win32-process-startup",
        "startup-info-write-request",
        "controlled STARTUPINFOW write request",
    )?;
    if request.schema != "vine.v86-startup-info-write-request" || request.schema_version != 1 {
        return Err(LoaderError::invalid_input(
            "win32-process-startup",
            "startup-info-write-request-schema-unsupported",
            "startup-info request must use vine.v86-startup-info-write-request version 1",
        ));
    }
    if request.argument_bytes != 4 {
        return Err(LoaderError::invalid_input(
            "abi-gateway",
            "startup-info-argument-width-mismatch",
            "GetStartupInfoW requires one four-byte stdcall argument",
        ));
    }
    if request.process_startup_owner.is_empty()
        || request.profile_id != X86StartupInfoW::PROFILE_ID
        || request.structure_size != X86StartupInfoW::SIZE
    {
        return Err(LoaderError::invalid_input(
            "win32-process-startup",
            "controlled-startup-info-profile-invalid",
            "controlled startup info requires its nonempty owner and exact x86 profile identity",
        ));
    }
    let thunk = owned_import_thunk(binding, request.thunk_id)?;
    let contract = get_startup_info_w_contract(ApiEndpointIdentity::from_import_thunk(&thunk))
        .map_err(api_fault)?;

    let startup_info = X86StartupInfoW::controlled();
    let output_bytes = runtime.startup_info().bytes.clone();
    if runtime.startup_info().profile_id != request.profile_id
        || output_bytes != startup_info.to_bytes()
    {
        return Err(LoaderError::runtime_defect(
            "win32-process-startup",
            "startup-info-evidence-runtime-mismatch",
            "startup-info request or serializer does not match the Rust runtime startup profile",
        ));
    }
    let (stack_reconciliation, write, delta) = {
        let mut transaction = memory.begin_transaction();
        let stack_reconciliation = transaction
            .reconcile_reflected_page(
                request.reflected_generation,
                MemoryOwnerId(request.stack_page_owner),
                GuestAddress(request.stack_page_address),
                &request.stack_page_bytes,
            )
            .map_err(memory_error)?;
        validate_direct_iat_callsite(&transaction, &thunk, request.return_address)?;
        let completion = dispatch_runtime_immediate(
            &mut transaction,
            runtime,
            binding,
            contract,
            controlled_api_trap(
                request.thunk_id,
                request.thunk_address,
                request.stack_pointer,
                request.return_address,
                request.argument_bytes,
            ),
        )?;
        if completion.eax != RegisterUpdate::Preserve {
            return Err(LoaderError::runtime_defect(
                "api-dispatch",
                "startup-info-return-register-invalid",
                "GetStartupInfoW dispatcher did not preserve EAX for a VOID return",
            ));
        }
        let write = completion
            .guest_writes
            .first()
            .filter(|_| completion.guest_writes.len() == 1)
            .map(|receipt| receipt.result)
            .ok_or_else(|| {
                LoaderError::runtime_defect(
                    "api-dispatch",
                    "startup-info-write-receipt-invalid",
                    "GetStartupInfoW dispatcher did not produce one guest write",
                )
            })?;
        let delta = transaction.pending_delta().map_err(memory_error)?;
        if delta.from_generation != request.reflected_generation
            || delta.dirty_pages.len() != 1
            || delta.dirty_pages[0].address.0 != request.stack_page_address
            || delta.dirty_pages[0].owner.0 != request.stack_page_owner
            || !delta.mapping_operations.is_empty()
            || delta.cache_effects.flush_tlb
            || delta.cache_effects.invalidate_instruction_fetch
            || delta.cache_effects.invalidate_generated_code
        {
            return Err(LoaderError::runtime_defect(
                "memory-delta",
                "startup-info-delta-shape-invalid",
                "checked startup-info write did not produce one data-only stack-page delta",
            ));
        }
        transaction.commit_transaction();
        (stack_reconciliation, write, delta)
    };

    let delta_generation = delta.generation;
    write_protocol_message(
        &V86StartupInfoDeltaOffer {
            schema: "vine.v86-startup-info-delta",
            schema_version: 1,
            thunk,
            calling_convention: "stdcall",
            argument_bytes: request.argument_bytes,
            stack_pointer: GuestAddress(request.stack_pointer),
            return_address: GuestAddress(request.return_address),
            output_address: GuestAddress(request.output_address),
            process_startup_owner: request.process_startup_owner,
            profile_id: request.profile_id,
            structure_size: request.structure_size,
            startup_info,
            output_bytes,
            stack_reconciliation,
            write,
            delta,
        },
        message_limit,
    )?;

    let ack: V86MemoryDeltaAck = read_protocol_message(
        input,
        4 * 1024,
        "memory-delta",
        "startup-info-delta-ack",
        "startup-info memory-delta acknowledgement",
    )?;
    if ack.schema != "vine.v86-memory-delta-ack"
        || ack.schema_version != 1
        || ack.generation != delta_generation
        || ack.data_tlb
        || ack.instruction_fetch
        || ack.generated_code
    {
        return Err(LoaderError::invalid_input(
            "memory-delta",
            "startup-info-delta-ack-invalid",
            "startup-info delta acknowledgement does not match its generation and cache effects",
        ));
    }
    memory
        .acknowledge_invalidation(ack.generation)
        .map_err(memory_error)?;
    let output_bytes = memory
        .read(
            GuestAddress(request.output_address),
            X86StartupInfoW::SIZE as usize,
        )
        .map_err(memory_error)?;
    write_protocol_message(
        &V86StartupInfoDeltaReceipt {
            schema: "vine.v86-startup-info-delta-receipt",
            schema_version: 1,
            acknowledged_generation: ack.generation,
            output_address: GuestAddress(request.output_address),
            output_bytes,
            stats: memory.stats(),
        },
        message_limit,
    )?;
    if return_sessions.includes(7) {
        run_v86_heap_set_information_return_session(
            input,
            memory,
            runtime,
            binding,
            message_limit,
            return_sessions,
        )?;
    }
    Ok(())
}

fn run_v86_heap_set_information_return_session(
    input: &mut impl BufRead,
    memory: &mut GuestMemory,
    runtime: &mut RuntimeState,
    binding: &ImportBinding,
    message_limit: usize,
    return_sessions: V86ReturnSessionOptions,
) -> Result<(), LoaderError> {
    const HEAP_ENABLE_TERMINATION_ON_CORRUPTION: u32 = 1;
    const POLICY_ID: &str = "heap-enable-termination-on-corruption";
    let request: V86HeapSetInformationReturnRequest = read_protocol_message(
        input,
        64 * 1024,
        "win32-heap-policy",
        "heap-set-information-return-request",
        "controlled heap policy return request",
    )?;
    if request.schema != "vine.v86-heap-set-information-return-request"
        || request.schema_version != 1
    {
        return Err(LoaderError::invalid_input(
            "win32-heap-policy",
            "heap-set-information-return-request-schema-unsupported",
            "heap policy request must use vine.v86-heap-set-information-return-request version 1",
        ));
    }
    if request.argument_bytes != 16 {
        return Err(LoaderError::invalid_input(
            "abi-gateway",
            "heap-set-information-argument-width-mismatch",
            "HeapSetInformation requires four four-byte stdcall arguments",
        ));
    }
    if request.heap_policy_owner.is_empty()
        || request.policy_id != POLICY_ID
        || request.heap_handle != 0
        || request.information_class != HEAP_ENABLE_TERMINATION_ON_CORRUPTION
        || request.information_address != 0
        || request.information_length != 0
        || !request.preserve_last_error
    {
        return Err(LoaderError::invalid_input(
            "win32-heap-policy",
            "heap-set-information-variant-unsupported",
            "controlled heap policy supports only HeapSetInformation(NULL, HeapEnableTerminationOnCorruption, NULL, 0) with unchanged LastError",
        ));
    }
    let thunk = binding
        .thunks
        .iter()
        .find(|thunk| thunk.id.0 == request.thunk_id)
        .ok_or_else(|| {
            LoaderError::invalid_input(
                "thunk-dispatch",
                "heap-set-information-thunk-id-unowned",
                "heap policy request does not name an owned import thunk",
            )
        })?
        .clone();
    if thunk.address.0 != request.thunk_address
        || !thunk.module.eq_ignore_ascii_case("KERNEL32.dll")
        || thunk.disposition != ImportThunkDisposition::DeclaredRuntimeExport
        || !matches!(
            &thunk.target,
            ImportTarget::Name { name, .. } if name == "HeapSetInformation"
        )
    {
        return Err(LoaderError::invalid_input(
            "thunk-dispatch",
            "heap-set-information-thunk-identity-mismatch",
            "heap policy request does not match the declared HeapSetInformation thunk",
        ));
    }
    let contract = heap_set_information_contract(ApiEndpointIdentity::from_import_thunk(&thunk))
        .map_err(api_fault)?;

    let last_error_before = runtime.current_last_error().map_err(runtime_state_error)?;
    let (stack_reconciliation, result) = {
        let mut transaction = memory.begin_transaction();
        let reconciliation = transaction
            .reconcile_reflected_page(
                request.reflected_generation,
                MemoryOwnerId(request.stack_page_owner),
                GuestAddress(request.stack_page_address),
                &request.stack_page_bytes,
            )
            .map_err(memory_error)?;
        validate_direct_iat_callsite(&transaction, &thunk, request.return_address)?;
        let completion = dispatch_runtime_immediate(
            &mut transaction,
            runtime,
            binding,
            contract,
            controlled_api_trap(
                request.thunk_id,
                request.thunk_address,
                request.stack_pointer,
                request.return_address,
                request.argument_bytes,
            ),
        )?;
        let result = set_eax_result(
            &completion,
            "heap-set-information-return-register-invalid",
            "HeapSetInformation dispatcher did not set EAX",
        )?;
        transaction.commit_transaction();
        (reconciliation, result)
    };
    let termination_on_corruption_enabled = runtime.process().heap_termination_on_corruption();
    if !termination_on_corruption_enabled
        || result != 1
        || runtime.current_last_error().map_err(runtime_state_error)? != last_error_before
    {
        return Err(LoaderError::runtime_defect(
            "win32-heap-policy",
            "heap-policy-runtime-transition-invalid",
            "HeapSetInformation did not update process policy while preserving thread LastError",
        ));
    }

    write_protocol_message(
        &V86HeapSetInformationReturnReceipt {
            schema: "vine.v86-heap-set-information-return-receipt",
            schema_version: 1,
            thunk,
            calling_convention: "stdcall",
            argument_bytes: request.argument_bytes,
            stack_pointer: GuestAddress(request.stack_pointer),
            return_address: GuestAddress(request.return_address),
            heap_policy_owner: request.heap_policy_owner,
            policy_id: request.policy_id,
            heap_handle: request.heap_handle,
            information_class: request.information_class,
            information_address: request.information_address,
            information_length: request.information_length,
            termination_on_corruption_enabled,
            preserve_last_error: request.preserve_last_error,
            result,
            stack_reconciliation,
            stats: memory.stats(),
        },
        message_limit,
    )?;
    if return_sessions.includes(8) {
        run_v86_heap_create_return_session(
            input,
            memory,
            runtime,
            binding,
            message_limit,
            return_sessions,
        )?;
    }
    Ok(())
}

fn run_v86_heap_create_return_session(
    input: &mut impl BufRead,
    memory: &mut GuestMemory,
    runtime: &mut RuntimeState,
    binding: &ImportBinding,
    message_limit: usize,
    return_sessions: V86ReturnSessionOptions,
) -> Result<(), LoaderError> {
    const OWNER_ID: &str = "checkpoint-process-private-heaps";
    const VARIANT_ID: &str = "heapcreate-growable-initial-4096";
    const INITIAL_SIZE: u32 = 0x1000;
    let request: V86HeapCreateReturnRequest = read_protocol_message(
        input,
        64 * 1024,
        "win32-private-heap",
        "heap-create-return-request",
        "controlled private heap creation request",
    )?;
    if request.schema != "vine.v86-heap-create-return-request" || request.schema_version != 1 {
        return Err(LoaderError::invalid_input(
            "win32-private-heap",
            "heap-create-return-request-schema-unsupported",
            "private heap request must use vine.v86-heap-create-return-request version 1",
        ));
    }
    if request.argument_bytes != 12 {
        return Err(LoaderError::invalid_input(
            "abi-gateway",
            "heap-create-argument-width-mismatch",
            "HeapCreate requires three four-byte stdcall arguments",
        ));
    }
    if request.heap_state_owner != OWNER_ID
        || request.variant_id != VARIANT_ID
        || request.creation_flags != 0
        || request.initial_size != INITIAL_SIZE
        || request.maximum_size != 0
        || !request.preserve_last_error
    {
        return Err(LoaderError::invalid_input(
            "win32-private-heap",
            "heap-create-variant-unsupported",
            "controlled private heap supports only HeapCreate(0, 0x1000, 0) with unchanged LastError",
        ));
    }
    let thunk = binding
        .thunks
        .iter()
        .find(|thunk| thunk.id.0 == request.thunk_id)
        .ok_or_else(|| {
            LoaderError::invalid_input(
                "thunk-dispatch",
                "heap-create-thunk-id-unowned",
                "private heap request does not name an owned import thunk",
            )
        })?
        .clone();
    if thunk.address.0 != request.thunk_address
        || !thunk.module.eq_ignore_ascii_case("KERNEL32.dll")
        || thunk.disposition != ImportThunkDisposition::DeclaredRuntimeExport
        || !matches!(
            &thunk.target,
            ImportTarget::Name { name, .. } if name == "HeapCreate"
        )
    {
        return Err(LoaderError::invalid_input(
            "thunk-dispatch",
            "heap-create-thunk-identity-mismatch",
            "private heap request does not match the declared HeapCreate thunk",
        ));
    }
    let contract =
        heap_create_contract(ApiEndpointIdentity::from_import_thunk(&thunk)).map_err(api_fault)?;

    if !runtime.process().heap_termination_on_corruption() {
        return Err(LoaderError::invalid_input(
            "win32-private-heap",
            "heap-policy-runtime-state-missing",
            "private heap creation requires the earlier process heap policy transition",
        ));
    }
    let logical_handle = X86LogicalHeapHandle::first_private_heap();
    let last_error_before = runtime.current_last_error().map_err(runtime_state_error)?;
    let (stack_reconciliation, result) = {
        let mut transaction = memory.begin_transaction();
        let reconciliation = transaction
            .reconcile_reflected_page(
                request.reflected_generation,
                MemoryOwnerId(request.stack_page_owner),
                GuestAddress(request.stack_page_address),
                &request.stack_page_bytes,
            )
            .map_err(memory_error)?;
        validate_direct_iat_callsite(&transaction, &thunk, request.return_address)?;
        let completion = dispatch_runtime_immediate(
            &mut transaction,
            runtime,
            binding,
            contract,
            controlled_api_trap(
                request.thunk_id,
                request.thunk_address,
                request.stack_pointer,
                request.return_address,
                request.argument_bytes,
            ),
        )?;
        let result = set_eax_result(
            &completion,
            "heap-create-return-register-invalid",
            "HeapCreate dispatcher did not set EAX",
        )?;
        transaction.commit_transaction();
        (reconciliation, result)
    };
    let runtime_heap = runtime.heap(result).map_err(runtime_state_error)?.clone();
    if result != logical_handle.value
        || runtime.current_last_error().map_err(runtime_state_error)? != last_error_before
    {
        return Err(LoaderError::runtime_defect(
            "win32-private-heap",
            "heap-create-last-error-changed",
            "logical heap creation changed the current thread LastError",
        ));
    }

    let logical_heap = X86LogicalHeap {
        owner: request.heap_state_owner,
        variant_id: request.variant_id,
        handle: logical_handle,
        kind: "private-heap",
        lifecycle: "active",
        creation_flags: runtime_heap.creation_flags,
        initial_size: runtime_heap.initial_size,
        maximum_size: runtime_heap.maximum_size,
        growable: runtime_heap.growable,
        backing_materialization: "deferred-until-first-reached-heap-operation",
        live_allocations: runtime_heap.live_allocations,
    };
    write_protocol_message(
        &V86HeapCreateReturnReceipt {
            schema: "vine.v86-heap-create-return-receipt",
            schema_version: 1,
            thunk,
            calling_convention: "stdcall",
            argument_bytes: request.argument_bytes,
            stack_pointer: GuestAddress(request.stack_pointer),
            return_address: GuestAddress(request.return_address),
            logical_heap,
            preserve_last_error: request.preserve_last_error,
            result,
            stack_reconciliation,
            stats: memory.stats(),
        },
        message_limit,
    )?;
    if return_sessions.includes(9) {
        run_v86_get_module_handle_w_return_session(
            input,
            memory,
            runtime,
            binding,
            message_limit,
            return_sessions,
        )?;
    }
    Ok(())
}

fn run_v86_get_module_handle_w_return_session(
    input: &mut impl BufRead,
    memory: &mut GuestMemory,
    runtime: &mut RuntimeState,
    binding: &ImportBinding,
    message_limit: usize,
    return_sessions: V86ReturnSessionOptions<'_>,
) -> Result<(), LoaderError> {
    const OWNER_ID: &str = "checkpoint-loaded-modules";
    const VARIANT_ID: &str = "getmodulehandlew-kernel32-loaded-runtime-module";
    const CANONICAL_NAME: &str = "kernel32.dll";
    let request: V86GetModuleHandleWReturnRequest = read_protocol_message(
        input,
        64 * 1024,
        "win32-loaded-module",
        "get-module-handle-w-return-request",
        "controlled loaded-module lookup request",
    )?;
    if request.schema != "vine.v86-get-module-handle-w-return-request"
        || request.schema_version != 1
    {
        return Err(LoaderError::invalid_input(
            "win32-loaded-module",
            "get-module-handle-w-return-request-schema-unsupported",
            "loaded-module request must use vine.v86-get-module-handle-w-return-request version 1",
        ));
    }
    if request.argument_bytes != 4 {
        return Err(LoaderError::invalid_input(
            "abi-gateway",
            "get-module-handle-w-argument-width-mismatch",
            "GetModuleHandleW requires one four-byte stdcall argument",
        ));
    }
    if request.loaded_module_owner != OWNER_ID
        || request.variant_id != VARIANT_ID
        || request.module_name_address == 0
        || !request.module_name.eq_ignore_ascii_case(CANONICAL_NAME)
        || !request.preserve_last_error
    {
        return Err(LoaderError::invalid_input(
            "win32-loaded-module",
            "get-module-handle-w-variant-unsupported",
            "controlled module lookup supports only GetModuleHandleW(L\"KERNEL32.DLL\") with unchanged LastError",
        ));
    }
    let thunk = binding
        .thunks
        .iter()
        .find(|thunk| thunk.id.0 == request.thunk_id)
        .ok_or_else(|| {
            LoaderError::invalid_input(
                "thunk-dispatch",
                "get-module-handle-w-thunk-id-unowned",
                "loaded-module request does not name an owned import thunk",
            )
        })?
        .clone();
    if thunk.address.0 != request.thunk_address
        || !thunk.module.eq_ignore_ascii_case("KERNEL32.dll")
        || thunk.disposition != ImportThunkDisposition::DeclaredRuntimeExport
        || !matches!(
            &thunk.target,
            ImportTarget::Name { name, .. } if name == "GetModuleHandleW"
        )
    {
        return Err(LoaderError::invalid_input(
            "thunk-dispatch",
            "get-module-handle-w-thunk-identity-mismatch",
            "loaded-module request does not match the declared GetModuleHandleW thunk",
        ));
    }
    let contract = get_module_handle_w_contract(ApiEndpointIdentity::from_import_thunk(&thunk))
        .map_err(api_fault)?;

    let logical_handle = X86LogicalLoadedModuleHandle::kernel32();
    let last_error_before = runtime.current_last_error().map_err(runtime_state_error)?;
    let (stack_reconciliation, module_name_utf16_bytes, result) = {
        let mut transaction = memory.begin_transaction();
        let reconciliation = transaction
            .reconcile_reflected_page(
                request.reflected_generation,
                MemoryOwnerId(request.stack_page_owner),
                GuestAddress(request.stack_page_address),
                &request.stack_page_bytes,
            )
            .map_err(memory_error)?;
        validate_direct_iat_callsite(&transaction, &thunk, request.return_address)?;
        let (observed_name, utf16_bytes) =
            read_bounded_utf16_nul(&transaction, GuestAddress(request.module_name_address), 64)?;
        if observed_name != request.module_name
            || !observed_name.eq_ignore_ascii_case(CANONICAL_NAME)
        {
            return Err(LoaderError::invalid_input(
                "win32-loaded-module",
                "get-module-handle-w-name-mismatch",
                "guest UTF-16 module name does not identify the controlled KERNEL32.dll module",
            ));
        }
        let completion = dispatch_runtime_immediate(
            &mut transaction,
            runtime,
            binding,
            contract,
            controlled_api_trap(
                request.thunk_id,
                request.thunk_address,
                request.stack_pointer,
                request.return_address,
                request.argument_bytes,
            ),
        )?;
        let result = set_eax_result(
            &completion,
            "get-module-handle-w-return-register-invalid",
            "GetModuleHandleW dispatcher did not set EAX",
        )?;
        transaction.commit_transaction();
        (reconciliation, utf16_bytes, result)
    };
    let runtime_module = runtime.module(result).map_err(runtime_state_error)?.clone();
    if result != logical_handle.value
        || runtime.current_last_error().map_err(runtime_state_error)? != last_error_before
    {
        return Err(LoaderError::runtime_defect(
            "win32-loaded-module",
            "module-lookup-last-error-changed",
            "logical module lookup changed the current thread LastError",
        ));
    }

    let logical_module = X86LogicalLoadedModule {
        owner: request.loaded_module_owner,
        variant_id: request.variant_id,
        handle: logical_handle,
        kind: "loaded-module",
        lifecycle: "loaded",
        requested_name: runtime_module.requested_name,
        canonical_name: CANONICAL_NAME,
        name_match: "unicode-case-insensitive-basename",
        module_role: "runtime-system-module",
        reference_count_changed: false,
        guest_image_materialization: "not-present",
        export_resolution: "deferred-until-first-reached-getprocaddress",
    };
    write_protocol_message(
        &V86GetModuleHandleWReturnReceipt {
            schema: "vine.v86-get-module-handle-w-return-receipt",
            schema_version: 1,
            thunk,
            calling_convention: "stdcall",
            argument_bytes: request.argument_bytes,
            stack_pointer: GuestAddress(request.stack_pointer),
            return_address: GuestAddress(request.return_address),
            module_name_address: GuestAddress(request.module_name_address),
            module_name_utf16_bytes,
            logical_module,
            preserve_last_error: request.preserve_last_error,
            result,
            stack_reconciliation,
            stats: memory.stats(),
        },
        message_limit,
    )?;
    if return_sessions.includes(10) {
        run_v86_get_proc_address_return_session(
            input,
            memory,
            runtime,
            binding,
            message_limit,
            return_sessions,
        )?;
    }
    Ok(())
}

fn run_v86_get_proc_address_return_session(
    input: &mut impl BufRead,
    memory: &mut GuestMemory,
    runtime: &mut RuntimeState,
    binding: &ImportBinding,
    message_limit: usize,
    return_sessions: V86ReturnSessionOptions<'_>,
) -> Result<(), LoaderError> {
    const OWNER_ID: &str = "checkpoint-dynamic-exports";
    const VARIANT_ID: &str = "getprocaddress-kernel32-flsalloc-runtime-endpoint";
    const CANONICAL_MODULE: &str = "kernel32.dll";
    const PROCEDURE_NAME: &str = "FlsAlloc";
    let endpoint = return_sessions
        .dynamic_export_thunks
        .first()
        .ok_or_else(|| {
            LoaderError::runtime_defect(
                "win32-dynamic-export",
                "dynamic-export-endpoint-missing",
                "GetProcAddress return session has no installed callable endpoint",
            )
        })?;
    let request: V86GetProcAddressReturnRequest = read_protocol_message(
        input,
        64 * 1024,
        "win32-dynamic-export",
        "get-proc-address-return-request",
        "controlled dynamic-export lookup request",
    )?;
    if request.schema != "vine.v86-get-proc-address-return-request" || request.schema_version != 1 {
        return Err(LoaderError::invalid_input(
            "win32-dynamic-export",
            "get-proc-address-return-request-schema-unsupported",
            "dynamic-export request must use vine.v86-get-proc-address-return-request version 1",
        ));
    }
    if request.argument_bytes != 8 {
        return Err(LoaderError::invalid_input(
            "abi-gateway",
            "get-proc-address-argument-width-mismatch",
            "GetProcAddress requires two four-byte stdcall arguments",
        ));
    }
    let logical_module_handle = X86LogicalLoadedModuleHandle::kernel32();
    if request.dynamic_export_owner != OWNER_ID
        || request.variant_id != VARIANT_ID
        || request.module_handle != logical_module_handle.value
        || request.lookup_kind != "name"
        || request.call_variant != "mov-esi-iat-push-name-push-edi-call-esi"
        || request.procedure_name_address <= u32::from(u16::MAX)
        || request.procedure_name != PROCEDURE_NAME
        || !request.preserve_last_error
    {
        return Err(LoaderError::invalid_input(
            "win32-dynamic-export",
            "get-proc-address-variant-unsupported",
            "controlled export lookup supports only GetProcAddress(logical KERNEL32, \"FlsAlloc\") by name with unchanged LastError",
        ));
    }
    let thunk = binding
        .thunks
        .iter()
        .find(|thunk| thunk.id.0 == request.thunk_id)
        .ok_or_else(|| {
            LoaderError::invalid_input(
                "thunk-dispatch",
                "get-proc-address-thunk-id-unowned",
                "dynamic-export request does not name an owned import thunk",
            )
        })?
        .clone();
    if thunk.address.0 != request.thunk_address
        || !thunk.module.eq_ignore_ascii_case("KERNEL32.dll")
        || thunk.disposition != ImportThunkDisposition::DeclaredRuntimeExport
        || !matches!(
            &thunk.target,
            ImportTarget::Name { name, .. } if name == "GetProcAddress"
        )
    {
        return Err(LoaderError::invalid_input(
            "thunk-dispatch",
            "get-proc-address-thunk-identity-mismatch",
            "dynamic-export request does not match the declared GetProcAddress thunk",
        ));
    }
    let contract = get_proc_address_contract(ApiEndpointIdentity::from_import_thunk(&thunk))
        .map_err(api_fault)?;
    if endpoint.id.0 != 1
        || endpoint.address.0 != 0x7f02_0000
        || !endpoint.module.eq_ignore_ascii_case(CANONICAL_MODULE)
        || endpoint.name != PROCEDURE_NAME
        || endpoint.lifetime != "process-runtime"
        || !endpoint.owner_sealed
    {
        return Err(LoaderError::runtime_defect(
            "win32-dynamic-export",
            "dynamic-export-endpoint-identity-mismatch",
            "installed dynamic-export endpoint does not match the controlled FlsAlloc identity",
        ));
    }
    let runtime_endpoint = runtime_dynamic_endpoint(runtime, request.module_handle, endpoint)?;
    let last_error_before = runtime.current_last_error().map_err(runtime_state_error)?;

    let (stack_reconciliation, observed_name, procedure_name_bytes, result) = {
        let mut transaction = memory.begin_transaction();
        let reconciliation = transaction
            .reconcile_reflected_page(
                request.reflected_generation,
                MemoryOwnerId(request.stack_page_owner),
                GuestAddress(request.stack_page_address),
                &request.stack_page_bytes,
            )
            .map_err(memory_error)?;
        validate_stdcall_register_iat_frame(
            &transaction,
            &thunk,
            request.stack_pointer,
            request.return_address,
            &[request.module_handle, request.procedure_name_address],
            request.procedure_name_address,
        )?;
        let (observed_name, ascii_bytes) = read_bounded_ascii_nul(
            &transaction,
            GuestAddress(request.procedure_name_address),
            64,
        )?;
        if observed_name != request.procedure_name || observed_name != PROCEDURE_NAME {
            return Err(LoaderError::invalid_input(
                "win32-dynamic-export",
                "get-proc-address-name-mismatch",
                "guest ASCII export name does not identify the controlled FlsAlloc endpoint",
            ));
        }
        let completion = dispatch_runtime_immediate(
            &mut transaction,
            runtime,
            binding,
            contract,
            controlled_api_trap(
                request.thunk_id,
                request.thunk_address,
                request.stack_pointer,
                request.return_address,
                request.argument_bytes,
            ),
        )?;
        let result = set_eax_result(
            &completion,
            "get-proc-address-return-register-invalid",
            "GetProcAddress dispatcher did not set EAX",
        )?;
        transaction.commit_transaction();
        (reconciliation, observed_name, ascii_bytes, result)
    };
    if result != runtime_endpoint.address.0
        || runtime.current_last_error().map_err(runtime_state_error)? != last_error_before
    {
        return Err(LoaderError::runtime_defect(
            "win32-dynamic-export",
            "get-proc-address-last-error-changed",
            "dynamic export lookup changed the current thread LastError",
        ));
    }

    write_protocol_message(
        &V86GetProcAddressReturnReceipt {
            schema: "vine.v86-get-proc-address-return-receipt",
            schema_version: 1,
            thunk,
            calling_convention: "stdcall",
            argument_bytes: request.argument_bytes,
            stack_pointer: GuestAddress(request.stack_pointer),
            return_address: GuestAddress(request.return_address),
            procedure_name_address: GuestAddress(request.procedure_name_address),
            procedure_name_bytes,
            resolution: X86DynamicExportResolution {
                owner: request.dynamic_export_owner,
                variant_id: request.variant_id,
                module_handle: logical_module_handle,
                canonical_module: CANONICAL_MODULE,
                lookup_kind: "name",
                call_variant: "mov-esi-iat-push-name-push-edi-call-esi",
                requested_name: observed_name,
                resolved_name: PROCEDURE_NAME,
                endpoint_namespace: endpoint.namespace,
                endpoint_lifetime: endpoint.lifetime,
                host_address_imported: false,
            },
            endpoint: endpoint.clone(),
            preserve_last_error: request.preserve_last_error,
            result,
            stack_reconciliation,
            stats: memory.stats(),
        },
        message_limit,
    )?;
    if return_sessions.includes(11) {
        run_v86_get_proc_address_second_return_session(
            input,
            memory,
            runtime,
            binding,
            message_limit,
            return_sessions,
        )?;
    }
    Ok(())
}

fn run_v86_get_proc_address_second_return_session(
    input: &mut impl BufRead,
    memory: &mut GuestMemory,
    runtime: &mut RuntimeState,
    binding: &ImportBinding,
    message_limit: usize,
    return_sessions: V86ReturnSessionOptions<'_>,
) -> Result<(), LoaderError> {
    const OWNER_ID: &str = "checkpoint-dynamic-exports";
    const VARIANT_ID: &str = "getprocaddress-kernel32-flsgetvalue-runtime-endpoint";
    const CANONICAL_MODULE: &str = "kernel32.dll";
    const PROCEDURE_NAME: &str = "FlsGetValue";
    const CALL_VARIANT: &str = "push-name-push-edi-store-prior-result-call-esi";
    const FIRST_ENDPOINT_STORE_ADDRESS: u32 = 0x100f_eebc;
    const FIRST_ENDPOINT_STORE_PAGE_ADDRESS: u32 = 0x100f_e000;
    let first_endpoint = return_sessions
        .dynamic_export_thunks
        .first()
        .ok_or_else(|| {
            LoaderError::runtime_defect(
                "win32-dynamic-export",
                "first-dynamic-export-endpoint-missing",
                "second GetProcAddress return session has no first callable endpoint",
            )
        })?;
    let endpoint = return_sessions
        .dynamic_export_thunks
        .get(1)
        .ok_or_else(|| {
            LoaderError::runtime_defect(
                "win32-dynamic-export",
                "second-dynamic-export-endpoint-missing",
                "second GetProcAddress return session has no second callable endpoint",
            )
        })?;
    let expected_endpoint_count =
        2 + usize::from(return_sessions.includes(12)) + usize::from(return_sessions.includes(13));
    if return_sessions.dynamic_export_thunks.len() != expected_endpoint_count
        || first_endpoint.id.0 != 1
        || first_endpoint.address.0 != 0x7f02_0000
        || !first_endpoint.module.eq_ignore_ascii_case(CANONICAL_MODULE)
        || first_endpoint.name != "FlsAlloc"
        || endpoint.id.0 != 2
        || endpoint.address.0 != 0x7f02_0010
        || !endpoint.module.eq_ignore_ascii_case(CANONICAL_MODULE)
        || endpoint.name != PROCEDURE_NAME
        || first_endpoint.owner != endpoint.owner
        || first_endpoint.arena_range != endpoint.arena_range
        || first_endpoint.content_sha256 != endpoint.content_sha256
        || first_endpoint.invalidation != endpoint.invalidation
        || first_endpoint.lifetime != "process-runtime"
        || endpoint.lifetime != "process-runtime"
        || !first_endpoint.owner_sealed
        || !endpoint.owner_sealed
    {
        return Err(LoaderError::runtime_defect(
            "win32-dynamic-export",
            "dynamic-export-endpoint-pair-mismatch",
            "installed dynamic-export endpoints do not preserve the controlled FLS identity pair",
        ));
    }

    let request: V86GetProcAddressSecondReturnRequest = read_protocol_message(
        input,
        96 * 1024,
        "win32-dynamic-export",
        "get-proc-address-second-return-request",
        "controlled second dynamic-export lookup request",
    )?;
    if request.schema != "vine.v86-get-proc-address-second-return-request"
        || request.schema_version != 1
    {
        return Err(LoaderError::invalid_input(
            "win32-dynamic-export",
            "get-proc-address-second-return-request-schema-unsupported",
            "second dynamic-export request must use vine.v86-get-proc-address-second-return-request version 1",
        ));
    }
    if request.argument_bytes != 8 {
        return Err(LoaderError::invalid_input(
            "abi-gateway",
            "get-proc-address-second-argument-width-mismatch",
            "GetProcAddress requires two four-byte stdcall arguments",
        ));
    }
    let logical_module_handle = X86LogicalLoadedModuleHandle::kernel32();
    if request.dynamic_export_owner != OWNER_ID
        || request.variant_id != VARIANT_ID
        || request.module_handle != logical_module_handle.value
        || request.lookup_kind != "name"
        || request.call_variant != CALL_VARIANT
        || request.indirect_call_register != "esi"
        || request.procedure_name_address <= u32::from(u16::MAX)
        || request.procedure_name != PROCEDURE_NAME
        || request.first_endpoint_address != first_endpoint.address.0
        || request.first_endpoint_store_address != FIRST_ENDPOINT_STORE_ADDRESS
        || request.first_endpoint_store_page_address != FIRST_ENDPOINT_STORE_PAGE_ADDRESS
        || request.first_endpoint_store_page_owner != binding.image_owner.0
        || !request.preserve_last_error
    {
        return Err(LoaderError::invalid_input(
            "win32-dynamic-export",
            "get-proc-address-second-variant-unsupported",
            "controlled second export lookup supports only GetProcAddress(logical KERNEL32, \"FlsGetValue\") with the preserved FlsAlloc endpoint",
        ));
    }
    let thunk = binding
        .thunks
        .iter()
        .find(|thunk| thunk.id.0 == request.thunk_id)
        .ok_or_else(|| {
            LoaderError::invalid_input(
                "thunk-dispatch",
                "get-proc-address-second-thunk-id-unowned",
                "second dynamic-export request does not name an owned import thunk",
            )
        })?
        .clone();
    if thunk.address.0 != request.thunk_address
        || request.indirect_call_target != thunk.address.0
        || !thunk.module.eq_ignore_ascii_case("KERNEL32.dll")
        || thunk.disposition != ImportThunkDisposition::DeclaredRuntimeExport
        || !matches!(
            &thunk.target,
            ImportTarget::Name { name, .. } if name == "GetProcAddress"
        )
    {
        return Err(LoaderError::invalid_input(
            "thunk-dispatch",
            "get-proc-address-second-thunk-identity-mismatch",
            "second dynamic-export request does not match the declared GetProcAddress thunk or live ESI target",
        ));
    }
    let contract = get_proc_address_contract(ApiEndpointIdentity::from_import_thunk(&thunk))
        .map_err(api_fault)?;
    let runtime_first_endpoint =
        runtime_dynamic_endpoint(runtime, request.module_handle, first_endpoint)?;
    let runtime_endpoint = runtime_dynamic_endpoint(runtime, request.module_handle, endpoint)?;
    let last_error_before = runtime.current_last_error().map_err(runtime_state_error)?;

    let (
        stack_reconciliation,
        first_endpoint_store_reconciliation,
        observed_name,
        procedure_name_bytes,
        first_endpoint_store_value,
        result,
    ) = {
        let mut transaction = memory.begin_transaction();
        let stack_reconciliation = transaction
            .reconcile_reflected_page(
                request.reflected_generation,
                MemoryOwnerId(request.stack_page_owner),
                GuestAddress(request.stack_page_address),
                &request.stack_page_bytes,
            )
            .map_err(memory_error)?;
        let first_endpoint_store_reconciliation = transaction
            .reconcile_reflected_page(
                request.reflected_generation,
                MemoryOwnerId(request.first_endpoint_store_page_owner),
                GuestAddress(request.first_endpoint_store_page_address),
                &request.first_endpoint_store_page_bytes,
            )
            .map_err(memory_error)?;
        validate_stdcall_persistent_register_iat_frame(
            &transaction,
            &thunk,
            &request,
            &[request.module_handle, request.procedure_name_address],
        )?;
        let (observed_name, ascii_bytes) = read_bounded_ascii_nul(
            &transaction,
            GuestAddress(request.procedure_name_address),
            64,
        )?;
        if observed_name != request.procedure_name || observed_name != PROCEDURE_NAME {
            return Err(LoaderError::invalid_input(
                "win32-dynamic-export",
                "get-proc-address-second-name-mismatch",
                "guest ASCII export name does not identify the controlled FlsGetValue endpoint",
            ));
        }
        let stored_bytes = transaction
            .read(GuestAddress(request.first_endpoint_store_address), 4)
            .map_err(memory_error)?;
        let first_endpoint_store_value =
            u32::from_le_bytes(stored_bytes.as_slice().try_into().map_err(|_| {
                LoaderError::runtime_defect(
                    "win32-dynamic-export",
                    "first-endpoint-store-width-invalid",
                    "checked first endpoint store did not contain four bytes",
                )
            })?);
        if first_endpoint_store_value != runtime_first_endpoint.address.0 {
            return Err(LoaderError::invalid_input(
                "win32-dynamic-export",
                "first-endpoint-store-value-mismatch",
                "BGI did not preserve the first dynamic-export endpoint before the second lookup",
            ));
        }
        let completion = dispatch_runtime_immediate(
            &mut transaction,
            runtime,
            binding,
            contract,
            controlled_api_trap(
                request.thunk_id,
                request.thunk_address,
                request.stack_pointer,
                request.return_address,
                request.argument_bytes,
            ),
        )?;
        let result = set_eax_result(
            &completion,
            "get-proc-address-second-return-register-invalid",
            "second GetProcAddress dispatcher did not set EAX",
        )?;
        transaction.commit_transaction();
        (
            stack_reconciliation,
            first_endpoint_store_reconciliation,
            observed_name,
            ascii_bytes,
            first_endpoint_store_value,
            result,
        )
    };
    if result != runtime_endpoint.address.0
        || runtime.current_last_error().map_err(runtime_state_error)? != last_error_before
    {
        return Err(LoaderError::runtime_defect(
            "win32-dynamic-export",
            "get-proc-address-second-last-error-changed",
            "second dynamic export lookup changed the current thread LastError",
        ));
    }

    write_protocol_message(
        &V86GetProcAddressSecondReturnReceipt {
            schema: "vine.v86-get-proc-address-second-return-receipt",
            schema_version: 1,
            thunk,
            calling_convention: "stdcall",
            argument_bytes: request.argument_bytes,
            stack_pointer: GuestAddress(request.stack_pointer),
            return_address: GuestAddress(request.return_address),
            procedure_name_address: GuestAddress(request.procedure_name_address),
            procedure_name_bytes,
            indirect_call_register: "esi",
            indirect_call_target: GuestAddress(request.indirect_call_target),
            resolution: X86DynamicExportResolution {
                owner: request.dynamic_export_owner,
                variant_id: request.variant_id,
                module_handle: logical_module_handle,
                canonical_module: CANONICAL_MODULE,
                lookup_kind: "name",
                call_variant: CALL_VARIANT,
                requested_name: observed_name,
                resolved_name: PROCEDURE_NAME,
                endpoint_namespace: endpoint.namespace,
                endpoint_lifetime: endpoint.lifetime,
                host_address_imported: false,
            },
            first_endpoint: first_endpoint.clone(),
            first_endpoint_store_address: GuestAddress(request.first_endpoint_store_address),
            first_endpoint_store_value,
            endpoint: endpoint.clone(),
            preserve_last_error: request.preserve_last_error,
            result,
            stack_reconciliation,
            first_endpoint_store_reconciliation,
            stats: memory.stats(),
        },
        message_limit,
    )?;
    if return_sessions.includes(12) {
        run_v86_get_proc_address_third_return_session(
            input,
            memory,
            runtime,
            binding,
            message_limit,
            return_sessions,
        )?;
    }
    Ok(())
}

fn run_v86_get_proc_address_third_return_session(
    input: &mut impl BufRead,
    memory: &mut GuestMemory,
    runtime: &mut RuntimeState,
    binding: &ImportBinding,
    message_limit: usize,
    return_sessions: V86ReturnSessionOptions<'_>,
) -> Result<(), LoaderError> {
    const OWNER_ID: &str = "checkpoint-dynamic-exports";
    const VARIANT_ID: &str = "getprocaddress-kernel32-flssetvalue-runtime-endpoint";
    const CANONICAL_MODULE: &str = "kernel32.dll";
    const PROCEDURE_NAME: &str = "FlsSetValue";
    const CALL_VARIANT: &str = "push-name-push-edi-store-prior-result-call-esi";
    const FIRST_ENDPOINT_STORE_ADDRESS: u32 = 0x100f_eebc;
    const SECOND_ENDPOINT_STORE_ADDRESS: u32 = 0x100f_eec0;
    const ENDPOINT_STORE_PAGE_ADDRESS: u32 = 0x100f_e000;
    let first_endpoint = return_sessions
        .dynamic_export_thunks
        .first()
        .ok_or_else(|| {
            LoaderError::runtime_defect(
                "win32-dynamic-export",
                "first-dynamic-export-endpoint-missing",
                "third GetProcAddress return session has no first callable endpoint",
            )
        })?;
    let second_endpoint = return_sessions
        .dynamic_export_thunks
        .get(1)
        .ok_or_else(|| {
            LoaderError::runtime_defect(
                "win32-dynamic-export",
                "second-dynamic-export-endpoint-missing",
                "third GetProcAddress return session has no second callable endpoint",
            )
        })?;
    let endpoint = return_sessions
        .dynamic_export_thunks
        .get(2)
        .ok_or_else(|| {
            LoaderError::runtime_defect(
                "win32-dynamic-export",
                "third-dynamic-export-endpoint-missing",
                "third GetProcAddress return session has no third callable endpoint",
            )
        })?;
    let expected_endpoints = [
        (1, 0x7f02_0000, "FlsAlloc"),
        (2, 0x7f02_0010, "FlsGetValue"),
        (3, 0x7f02_0020, PROCEDURE_NAME),
    ];
    let expected_endpoint_count =
        expected_endpoints.len() + usize::from(return_sessions.includes(13));
    if return_sessions.dynamic_export_thunks.len() != expected_endpoint_count
        || return_sessions
            .dynamic_export_thunks
            .iter()
            .zip(expected_endpoints)
            .any(|(observed, (id, address, name))| {
                observed.id.0 != id
                    || observed.address.0 != address
                    || !observed.module.eq_ignore_ascii_case(CANONICAL_MODULE)
                    || observed.name != name
                    || observed.owner != first_endpoint.owner
                    || observed.arena_range != first_endpoint.arena_range
                    || observed.content_sha256 != first_endpoint.content_sha256
                    || observed.invalidation != first_endpoint.invalidation
                    || observed.lifetime != "process-runtime"
                    || !observed.owner_sealed
            })
    {
        return Err(LoaderError::runtime_defect(
            "win32-dynamic-export",
            "dynamic-export-endpoint-triple-mismatch",
            "installed dynamic-export endpoints do not preserve the controlled FLS identity triple",
        ));
    }

    let request: V86GetProcAddressThirdReturnRequest = read_protocol_message(
        input,
        96 * 1024,
        "win32-dynamic-export",
        "get-proc-address-third-return-request",
        "controlled third dynamic-export lookup request",
    )?;
    if request.schema != "vine.v86-get-proc-address-third-return-request"
        || request.schema_version != 1
    {
        return Err(LoaderError::invalid_input(
            "win32-dynamic-export",
            "get-proc-address-third-return-request-schema-unsupported",
            "third dynamic-export request must use vine.v86-get-proc-address-third-return-request version 1",
        ));
    }
    if request.argument_bytes != 8 {
        return Err(LoaderError::invalid_input(
            "abi-gateway",
            "get-proc-address-third-argument-width-mismatch",
            "GetProcAddress requires two four-byte stdcall arguments",
        ));
    }
    let logical_module_handle = X86LogicalLoadedModuleHandle::kernel32();
    if request.dynamic_export_owner != OWNER_ID
        || request.variant_id != VARIANT_ID
        || request.module_handle != logical_module_handle.value
        || request.lookup_kind != "name"
        || request.call_variant != CALL_VARIANT
        || request.indirect_call_register != "esi"
        || request.procedure_name_address <= u32::from(u16::MAX)
        || request.procedure_name != PROCEDURE_NAME
        || request.first_endpoint_address != first_endpoint.address.0
        || request.second_endpoint_address != second_endpoint.address.0
        || request.first_endpoint_store_address != FIRST_ENDPOINT_STORE_ADDRESS
        || request.second_endpoint_store_address != SECOND_ENDPOINT_STORE_ADDRESS
        || request.endpoint_store_page_address != ENDPOINT_STORE_PAGE_ADDRESS
        || request.endpoint_store_page_owner != binding.image_owner.0
        || !request.preserve_last_error
    {
        return Err(LoaderError::invalid_input(
            "win32-dynamic-export",
            "get-proc-address-third-variant-unsupported",
            "controlled third export lookup supports only GetProcAddress(logical KERNEL32, \"FlsSetValue\") with the preserved first two FLS endpoints",
        ));
    }
    let thunk = binding
        .thunks
        .iter()
        .find(|thunk| thunk.id.0 == request.thunk_id)
        .ok_or_else(|| {
            LoaderError::invalid_input(
                "thunk-dispatch",
                "get-proc-address-third-thunk-id-unowned",
                "third dynamic-export request does not name an owned import thunk",
            )
        })?
        .clone();
    if thunk.address.0 != request.thunk_address
        || request.indirect_call_target != thunk.address.0
        || !thunk.module.eq_ignore_ascii_case("KERNEL32.dll")
        || thunk.disposition != ImportThunkDisposition::DeclaredRuntimeExport
        || !matches!(
            &thunk.target,
            ImportTarget::Name { name, .. } if name == "GetProcAddress"
        )
    {
        return Err(LoaderError::invalid_input(
            "thunk-dispatch",
            "get-proc-address-third-thunk-identity-mismatch",
            "third dynamic-export request does not match the declared GetProcAddress thunk or live ESI target",
        ));
    }
    let contract = get_proc_address_contract(ApiEndpointIdentity::from_import_thunk(&thunk))
        .map_err(api_fault)?;
    let runtime_first_endpoint =
        runtime_dynamic_endpoint(runtime, request.module_handle, first_endpoint)?;
    let runtime_second_endpoint =
        runtime_dynamic_endpoint(runtime, request.module_handle, second_endpoint)?;
    let runtime_endpoint = runtime_dynamic_endpoint(runtime, request.module_handle, endpoint)?;
    let last_error_before = runtime.current_last_error().map_err(runtime_state_error)?;

    let (
        stack_reconciliation,
        endpoint_store_reconciliation,
        observed_name,
        procedure_name_bytes,
        first_endpoint_store_value,
        second_endpoint_store_value,
        result,
    ) = {
        let mut transaction = memory.begin_transaction();
        let stack_reconciliation = transaction
            .reconcile_reflected_page(
                request.reflected_generation,
                MemoryOwnerId(request.stack_page_owner),
                GuestAddress(request.stack_page_address),
                &request.stack_page_bytes,
            )
            .map_err(memory_error)?;
        let endpoint_store_reconciliation = transaction
            .reconcile_reflected_page(
                request.reflected_generation,
                MemoryOwnerId(request.endpoint_store_page_owner),
                GuestAddress(request.endpoint_store_page_address),
                &request.endpoint_store_page_bytes,
            )
            .map_err(memory_error)?;
        validate_stdcall_third_persistent_register_iat_frame(
            &transaction,
            &thunk,
            &request,
            &[request.module_handle, request.procedure_name_address],
        )?;
        let (observed_name, ascii_bytes) = read_bounded_ascii_nul(
            &transaction,
            GuestAddress(request.procedure_name_address),
            64,
        )?;
        if observed_name != request.procedure_name || observed_name != PROCEDURE_NAME {
            return Err(LoaderError::invalid_input(
                "win32-dynamic-export",
                "get-proc-address-third-name-mismatch",
                "guest ASCII export name does not identify the controlled FlsSetValue endpoint",
            ));
        }
        let read_stored_endpoint =
            |address: u32, label: &'static str| -> Result<u32, LoaderError> {
                let stored_bytes = transaction
                    .read(GuestAddress(address), 4)
                    .map_err(memory_error)?;
                Ok(u32::from_le_bytes(
                    stored_bytes.as_slice().try_into().map_err(|_| {
                        LoaderError::runtime_defect(
                            "win32-dynamic-export",
                            "endpoint-store-width-invalid",
                            format!("checked {label} endpoint store did not contain four bytes"),
                        )
                    })?,
                ))
            };
        let first_endpoint_store_value =
            read_stored_endpoint(request.first_endpoint_store_address, "first")?;
        let second_endpoint_store_value =
            read_stored_endpoint(request.second_endpoint_store_address, "second")?;
        if first_endpoint_store_value != runtime_first_endpoint.address.0
            || second_endpoint_store_value != runtime_second_endpoint.address.0
        {
            return Err(LoaderError::invalid_input(
                "win32-dynamic-export",
                "prior-endpoint-store-value-mismatch",
                "BGI did not preserve the first two dynamic-export endpoints before the third lookup",
            ));
        }
        let completion = dispatch_runtime_immediate(
            &mut transaction,
            runtime,
            binding,
            contract,
            controlled_api_trap(
                request.thunk_id,
                request.thunk_address,
                request.stack_pointer,
                request.return_address,
                request.argument_bytes,
            ),
        )?;
        let result = set_eax_result(
            &completion,
            "get-proc-address-third-return-register-invalid",
            "third GetProcAddress dispatcher did not set EAX",
        )?;
        transaction.commit_transaction();
        (
            stack_reconciliation,
            endpoint_store_reconciliation,
            observed_name,
            ascii_bytes,
            first_endpoint_store_value,
            second_endpoint_store_value,
            result,
        )
    };
    if result != runtime_endpoint.address.0
        || runtime.current_last_error().map_err(runtime_state_error)? != last_error_before
    {
        return Err(LoaderError::runtime_defect(
            "win32-dynamic-export",
            "get-proc-address-third-last-error-changed",
            "third dynamic export lookup changed the current thread LastError",
        ));
    }

    write_protocol_message(
        &V86GetProcAddressThirdReturnReceipt {
            schema: "vine.v86-get-proc-address-third-return-receipt",
            schema_version: 1,
            thunk,
            calling_convention: "stdcall",
            argument_bytes: request.argument_bytes,
            stack_pointer: GuestAddress(request.stack_pointer),
            return_address: GuestAddress(request.return_address),
            procedure_name_address: GuestAddress(request.procedure_name_address),
            procedure_name_bytes,
            indirect_call_register: "esi",
            indirect_call_target: GuestAddress(request.indirect_call_target),
            resolution: X86DynamicExportResolution {
                owner: request.dynamic_export_owner,
                variant_id: request.variant_id,
                module_handle: logical_module_handle,
                canonical_module: CANONICAL_MODULE,
                lookup_kind: "name",
                call_variant: CALL_VARIANT,
                requested_name: observed_name,
                resolved_name: PROCEDURE_NAME,
                endpoint_namespace: endpoint.namespace,
                endpoint_lifetime: endpoint.lifetime,
                host_address_imported: false,
            },
            first_endpoint: first_endpoint.clone(),
            first_endpoint_store_address: GuestAddress(request.first_endpoint_store_address),
            first_endpoint_store_value,
            second_endpoint: second_endpoint.clone(),
            second_endpoint_store_address: GuestAddress(request.second_endpoint_store_address),
            second_endpoint_store_value,
            endpoint: endpoint.clone(),
            preserve_last_error: request.preserve_last_error,
            result,
            stack_reconciliation,
            endpoint_store_reconciliation,
            stats: memory.stats(),
        },
        message_limit,
    )?;
    if return_sessions.includes(13) {
        run_v86_get_proc_address_fourth_return_session(
            input,
            memory,
            runtime,
            binding,
            message_limit,
            return_sessions,
        )?;
    }
    Ok(())
}

fn run_v86_get_proc_address_fourth_return_session(
    input: &mut impl BufRead,
    memory: &mut GuestMemory,
    runtime: &mut RuntimeState,
    binding: &ImportBinding,
    message_limit: usize,
    return_sessions: V86ReturnSessionOptions<'_>,
) -> Result<(), LoaderError> {
    const OWNER_ID: &str = "checkpoint-dynamic-exports";
    const VARIANT_ID: &str = "getprocaddress-kernel32-flsfree-runtime-endpoint";
    const CANONICAL_MODULE: &str = "kernel32.dll";
    const PROCEDURE_NAME: &str = "FlsFree";
    const CALL_VARIANT: &str = "push-name-push-edi-store-prior-result-call-esi";
    const FIRST_ENDPOINT_STORE_ADDRESS: u32 = 0x100f_eebc;
    const SECOND_ENDPOINT_STORE_ADDRESS: u32 = 0x100f_eec0;
    const THIRD_ENDPOINT_STORE_ADDRESS: u32 = 0x100f_eec4;
    const ENDPOINT_STORE_PAGE_ADDRESS: u32 = 0x100f_e000;
    let endpoints = return_sessions.dynamic_export_thunks;
    let first_endpoint = endpoints.first().ok_or_else(|| {
        LoaderError::runtime_defect(
            "win32-dynamic-export",
            "first-dynamic-export-endpoint-missing",
            "fourth GetProcAddress return session has no first callable endpoint",
        )
    })?;
    let second_endpoint = endpoints.get(1).ok_or_else(|| {
        LoaderError::runtime_defect(
            "win32-dynamic-export",
            "second-dynamic-export-endpoint-missing",
            "fourth GetProcAddress return session has no second callable endpoint",
        )
    })?;
    let third_endpoint = endpoints.get(2).ok_or_else(|| {
        LoaderError::runtime_defect(
            "win32-dynamic-export",
            "third-dynamic-export-endpoint-missing",
            "fourth GetProcAddress return session has no third callable endpoint",
        )
    })?;
    let endpoint = endpoints.get(3).ok_or_else(|| {
        LoaderError::runtime_defect(
            "win32-dynamic-export",
            "fourth-dynamic-export-endpoint-missing",
            "fourth GetProcAddress return session has no fourth callable endpoint",
        )
    })?;
    let expected_endpoints = [
        (1, 0x7f02_0000, "FlsAlloc"),
        (2, 0x7f02_0010, "FlsGetValue"),
        (3, 0x7f02_0020, "FlsSetValue"),
        (4, 0x7f02_0030, PROCEDURE_NAME),
    ];
    if endpoints.len() != expected_endpoints.len()
        || endpoints
            .iter()
            .zip(expected_endpoints)
            .any(|(observed, (id, address, name))| {
                observed.id.0 != id
                    || observed.address.0 != address
                    || !observed.module.eq_ignore_ascii_case(CANONICAL_MODULE)
                    || observed.name != name
                    || observed.owner != first_endpoint.owner
                    || observed.arena_range != first_endpoint.arena_range
                    || observed.content_sha256 != first_endpoint.content_sha256
                    || observed.invalidation != first_endpoint.invalidation
                    || observed.lifetime != "process-runtime"
                    || !observed.owner_sealed
            })
    {
        return Err(LoaderError::runtime_defect(
            "win32-dynamic-export",
            "dynamic-export-endpoint-quadruple-mismatch",
            "installed dynamic-export endpoints do not preserve the controlled FLS identity quadruple",
        ));
    }
    let request: V86GetProcAddressFourthReturnRequest = read_protocol_message(
        input,
        96 * 1024,
        "win32-dynamic-export",
        "get-proc-address-fourth-return-request",
        "controlled fourth dynamic-export lookup request",
    )?;
    if request.schema != "vine.v86-get-proc-address-fourth-return-request"
        || request.schema_version != 1
    {
        return Err(LoaderError::invalid_input(
            "win32-dynamic-export",
            "get-proc-address-fourth-return-request-schema-unsupported",
            "fourth dynamic-export request must use vine.v86-get-proc-address-fourth-return-request version 1",
        ));
    }
    if request.argument_bytes != 8 {
        return Err(LoaderError::invalid_input(
            "abi-gateway",
            "get-proc-address-fourth-argument-width-mismatch",
            "GetProcAddress requires two four-byte stdcall arguments",
        ));
    }
    let logical_module_handle = X86LogicalLoadedModuleHandle::kernel32();
    if request.dynamic_export_owner != OWNER_ID
        || request.variant_id != VARIANT_ID
        || request.module_handle != logical_module_handle.value
        || request.lookup_kind != "name"
        || request.call_variant != CALL_VARIANT
        || request.indirect_call_register != "esi"
        || request.procedure_name_address <= u32::from(u16::MAX)
        || request.procedure_name != PROCEDURE_NAME
        || request.first_endpoint_address != first_endpoint.address.0
        || request.second_endpoint_address != second_endpoint.address.0
        || request.third_endpoint_address != third_endpoint.address.0
        || request.first_endpoint_store_address != FIRST_ENDPOINT_STORE_ADDRESS
        || request.second_endpoint_store_address != SECOND_ENDPOINT_STORE_ADDRESS
        || request.third_endpoint_store_address != THIRD_ENDPOINT_STORE_ADDRESS
        || request.endpoint_store_page_address != ENDPOINT_STORE_PAGE_ADDRESS
        || request.endpoint_store_page_owner != binding.image_owner.0
        || !request.preserve_last_error
    {
        return Err(LoaderError::invalid_input(
            "win32-dynamic-export",
            "get-proc-address-fourth-variant-unsupported",
            "controlled fourth export lookup supports only GetProcAddress(logical KERNEL32, \"FlsFree\") with the preserved first three FLS endpoints",
        ));
    }
    let thunk = binding
        .thunks
        .iter()
        .find(|thunk| thunk.id.0 == request.thunk_id)
        .ok_or_else(|| {
            LoaderError::invalid_input(
                "thunk-dispatch",
                "get-proc-address-fourth-thunk-id-unowned",
                "fourth dynamic-export request does not name an owned import thunk",
            )
        })?
        .clone();
    if thunk.address.0 != request.thunk_address
        || request.indirect_call_target != thunk.address.0
        || !thunk.module.eq_ignore_ascii_case("KERNEL32.dll")
        || thunk.disposition != ImportThunkDisposition::DeclaredRuntimeExport
        || !matches!(
            &thunk.target,
            ImportTarget::Name { name, .. } if name == "GetProcAddress"
        )
    {
        return Err(LoaderError::invalid_input(
            "thunk-dispatch",
            "get-proc-address-fourth-thunk-identity-mismatch",
            "fourth dynamic-export request does not match the declared GetProcAddress thunk or live ESI target",
        ));
    }
    let contract = get_proc_address_contract(ApiEndpointIdentity::from_import_thunk(&thunk))
        .map_err(api_fault)?;
    let runtime_first_endpoint =
        runtime_dynamic_endpoint(runtime, request.module_handle, first_endpoint)?;
    let runtime_second_endpoint =
        runtime_dynamic_endpoint(runtime, request.module_handle, second_endpoint)?;
    let runtime_third_endpoint =
        runtime_dynamic_endpoint(runtime, request.module_handle, third_endpoint)?;
    let runtime_endpoint = runtime_dynamic_endpoint(runtime, request.module_handle, endpoint)?;
    let last_error_before = runtime.current_last_error().map_err(runtime_state_error)?;

    let (
        stack_reconciliation,
        endpoint_store_reconciliation,
        observed_name,
        procedure_name_bytes,
        first_endpoint_store_value,
        second_endpoint_store_value,
        third_endpoint_store_value,
        result,
    ) = {
        let mut transaction = memory.begin_transaction();
        let stack_reconciliation = transaction
            .reconcile_reflected_page(
                request.reflected_generation,
                MemoryOwnerId(request.stack_page_owner),
                GuestAddress(request.stack_page_address),
                &request.stack_page_bytes,
            )
            .map_err(memory_error)?;
        let endpoint_store_reconciliation = transaction
            .reconcile_reflected_page(
                request.reflected_generation,
                MemoryOwnerId(request.endpoint_store_page_owner),
                GuestAddress(request.endpoint_store_page_address),
                &request.endpoint_store_page_bytes,
            )
            .map_err(memory_error)?;
        validate_stdcall_fourth_persistent_register_iat_frame(
            &transaction,
            &thunk,
            &request,
            &[request.module_handle, request.procedure_name_address],
        )?;
        let (observed_name, ascii_bytes) = read_bounded_ascii_nul(
            &transaction,
            GuestAddress(request.procedure_name_address),
            64,
        )?;
        if observed_name != request.procedure_name || observed_name != PROCEDURE_NAME {
            return Err(LoaderError::invalid_input(
                "win32-dynamic-export",
                "get-proc-address-fourth-name-mismatch",
                "guest ASCII export name does not identify the controlled FlsFree endpoint",
            ));
        }
        let read_stored_endpoint =
            |address: u32, label: &'static str| -> Result<u32, LoaderError> {
                let stored_bytes = transaction
                    .read(GuestAddress(address), 4)
                    .map_err(memory_error)?;
                Ok(u32::from_le_bytes(
                    stored_bytes.as_slice().try_into().map_err(|_| {
                        LoaderError::runtime_defect(
                            "win32-dynamic-export",
                            "endpoint-store-width-invalid",
                            format!("checked {label} endpoint store did not contain four bytes"),
                        )
                    })?,
                ))
            };
        let first_endpoint_store_value =
            read_stored_endpoint(request.first_endpoint_store_address, "first")?;
        let second_endpoint_store_value =
            read_stored_endpoint(request.second_endpoint_store_address, "second")?;
        let third_endpoint_store_value =
            read_stored_endpoint(request.third_endpoint_store_address, "third")?;
        if first_endpoint_store_value != runtime_first_endpoint.address.0
            || second_endpoint_store_value != runtime_second_endpoint.address.0
            || third_endpoint_store_value != runtime_third_endpoint.address.0
        {
            return Err(LoaderError::invalid_input(
                "win32-dynamic-export",
                "prior-endpoint-store-value-mismatch",
                "BGI did not preserve the first three dynamic-export endpoints before the fourth lookup",
            ));
        }
        let completion = dispatch_runtime_immediate(
            &mut transaction,
            runtime,
            binding,
            contract,
            controlled_api_trap(
                request.thunk_id,
                request.thunk_address,
                request.stack_pointer,
                request.return_address,
                request.argument_bytes,
            ),
        )?;
        let result = set_eax_result(
            &completion,
            "get-proc-address-fourth-return-register-invalid",
            "fourth GetProcAddress dispatcher did not set EAX",
        )?;
        transaction.commit_transaction();
        (
            stack_reconciliation,
            endpoint_store_reconciliation,
            observed_name,
            ascii_bytes,
            first_endpoint_store_value,
            second_endpoint_store_value,
            third_endpoint_store_value,
            result,
        )
    };
    if result != runtime_endpoint.address.0
        || runtime.current_last_error().map_err(runtime_state_error)? != last_error_before
    {
        return Err(LoaderError::runtime_defect(
            "win32-dynamic-export",
            "get-proc-address-fourth-last-error-changed",
            "fourth dynamic export lookup changed the current thread LastError",
        ));
    }

    write_protocol_message(
        &V86GetProcAddressFourthReturnReceipt {
            schema: "vine.v86-get-proc-address-fourth-return-receipt",
            schema_version: 1,
            thunk,
            calling_convention: "stdcall",
            argument_bytes: request.argument_bytes,
            stack_pointer: GuestAddress(request.stack_pointer),
            return_address: GuestAddress(request.return_address),
            procedure_name_address: GuestAddress(request.procedure_name_address),
            procedure_name_bytes,
            indirect_call_register: "esi",
            indirect_call_target: GuestAddress(request.indirect_call_target),
            resolution: X86DynamicExportResolution {
                owner: request.dynamic_export_owner,
                variant_id: request.variant_id,
                module_handle: logical_module_handle,
                canonical_module: CANONICAL_MODULE,
                lookup_kind: "name",
                call_variant: CALL_VARIANT,
                requested_name: observed_name,
                resolved_name: PROCEDURE_NAME,
                endpoint_namespace: endpoint.namespace,
                endpoint_lifetime: endpoint.lifetime,
                host_address_imported: false,
            },
            first_endpoint: first_endpoint.clone(),
            first_endpoint_store_address: GuestAddress(request.first_endpoint_store_address),
            first_endpoint_store_value,
            second_endpoint: second_endpoint.clone(),
            second_endpoint_store_address: GuestAddress(request.second_endpoint_store_address),
            second_endpoint_store_value,
            third_endpoint: third_endpoint.clone(),
            third_endpoint_store_address: GuestAddress(request.third_endpoint_store_address),
            third_endpoint_store_value,
            endpoint: endpoint.clone(),
            preserve_last_error: request.preserve_last_error,
            result,
            stack_reconciliation,
            endpoint_store_reconciliation,
            stats: memory.stats(),
        },
        message_limit,
    )?;
    if return_sessions.includes(14) {
        run_v86_tls_alloc_return_session(
            input,
            memory,
            runtime,
            binding,
            message_limit,
            return_sessions,
        )?;
    }
    Ok(())
}

fn run_v86_tls_alloc_return_session(
    input: &mut impl BufRead,
    memory: &mut GuestMemory,
    runtime: &mut RuntimeState,
    binding: &ImportBinding,
    message_limit: usize,
    return_sessions: V86ReturnSessionOptions<'_>,
) -> Result<(), LoaderError> {
    const OWNER_ID: &str = "checkpoint-thread-local-storage";
    const VARIANT_ID: &str = "tlsalloc-first-controlled-index";
    const FIRST_ENDPOINT_STORE_ADDRESS: u32 = 0x100f_eebc;
    const SECOND_ENDPOINT_STORE_ADDRESS: u32 = 0x100f_eec0;
    const THIRD_ENDPOINT_STORE_ADDRESS: u32 = 0x100f_eec4;
    const FOURTH_ENDPOINT_STORE_ADDRESS: u32 = 0x100f_eec8;
    const ENDPOINT_STORE_PAGE_ADDRESS: u32 = 0x100f_e000;

    let endpoints = return_sessions.dynamic_export_thunks;
    let expected_endpoints = [
        (1, 0x7f02_0000, "FlsAlloc"),
        (2, 0x7f02_0010, "FlsGetValue"),
        (3, 0x7f02_0020, "FlsSetValue"),
        (4, 0x7f02_0030, "FlsFree"),
    ];
    let first_endpoint = endpoints.first().ok_or_else(|| {
        LoaderError::runtime_defect(
            "win32-thread-local-storage",
            "dynamic-export-endpoints-missing",
            "TlsAlloc return session has no preserved FLS endpoints",
        )
    })?;
    if endpoints.len() != expected_endpoints.len()
        || endpoints
            .iter()
            .zip(expected_endpoints)
            .any(|(observed, (id, address, name))| {
                observed.id.0 != id
                    || observed.address.0 != address
                    || !observed.module.eq_ignore_ascii_case("kernel32.dll")
                    || observed.name != name
                    || observed.owner != first_endpoint.owner
                    || observed.arena_range != first_endpoint.arena_range
                    || observed.content_sha256 != first_endpoint.content_sha256
                    || observed.invalidation != first_endpoint.invalidation
                    || observed.lifetime != "process-runtime"
                    || !observed.owner_sealed
            })
    {
        return Err(LoaderError::runtime_defect(
            "win32-thread-local-storage",
            "dynamic-export-endpoint-quadruple-mismatch",
            "TlsAlloc return session did not retain the controlled FLS identity quadruple",
        ));
    }
    let module_handle = X86LogicalLoadedModuleHandle::kernel32().value;
    let runtime_endpoints = endpoints
        .iter()
        .map(|endpoint| runtime_dynamic_endpoint(runtime, module_handle, endpoint))
        .collect::<Result<Vec<_>, _>>()?;

    let request: V86TlsAllocReturnRequest = read_protocol_message(
        input,
        96 * 1024,
        "win32-thread-local-storage",
        "tls-alloc-return-request",
        "controlled TLS index allocation request",
    )?;
    if request.schema != "vine.v86-tls-alloc-return-request" || request.schema_version != 1 {
        return Err(LoaderError::invalid_input(
            "win32-thread-local-storage",
            "tls-alloc-return-request-schema-unsupported",
            "TLS allocation request must use vine.v86-tls-alloc-return-request version 1",
        ));
    }
    if request.argument_bytes != 0 {
        return Err(LoaderError::invalid_input(
            "abi-gateway",
            "tls-alloc-argument-width-mismatch",
            "TlsAlloc requires a zero-argument stdcall frame",
        ));
    }
    let expected_addresses = [
        runtime_endpoints[0].address.0,
        runtime_endpoints[1].address.0,
        runtime_endpoints[2].address.0,
        runtime_endpoints[3].address.0,
    ];
    if request.tls_state_owner != OWNER_ID
        || request.variant_id != VARIANT_ID
        || request.logical_index_namespace != X86LogicalTlsIndex::NAMESPACE
        || request.logical_index != X86LogicalTlsIndex::FIRST_INDEX
        || request.failure_sentinel != X86LogicalTlsIndex::FAILURE_SENTINEL
        || !request.preserve_last_error
        || [
            request.first_endpoint_address,
            request.second_endpoint_address,
            request.third_endpoint_address,
            request.fourth_endpoint_address,
        ] != expected_addresses
        || request.first_endpoint_store_address != FIRST_ENDPOINT_STORE_ADDRESS
        || request.second_endpoint_store_address != SECOND_ENDPOINT_STORE_ADDRESS
        || request.third_endpoint_store_address != THIRD_ENDPOINT_STORE_ADDRESS
        || request.fourth_endpoint_store_address != FOURTH_ENDPOINT_STORE_ADDRESS
        || request.endpoint_store_page_address != ENDPOINT_STORE_PAGE_ADDRESS
        || request.endpoint_store_page_owner != binding.image_owner.0
    {
        return Err(LoaderError::invalid_input(
            "win32-thread-local-storage",
            "tls-alloc-variant-unsupported",
            "controlled TLS allocation supports only the first logical index with all four FLS endpoints preserved",
        ));
    }

    let thunk = binding
        .thunks
        .iter()
        .find(|thunk| thunk.id.0 == request.thunk_id)
        .ok_or_else(|| {
            LoaderError::invalid_input(
                "thunk-dispatch",
                "tls-alloc-thunk-id-unowned",
                "TLS allocation request does not name an owned import thunk",
            )
        })?
        .clone();
    if thunk.address.0 != request.thunk_address
        || !thunk.module.eq_ignore_ascii_case("KERNEL32.dll")
        || thunk.disposition != ImportThunkDisposition::DeclaredRuntimeExport
        || !matches!(&thunk.target, ImportTarget::Name { name, .. } if name == "TlsAlloc")
    {
        return Err(LoaderError::invalid_input(
            "thunk-dispatch",
            "tls-alloc-thunk-identity-mismatch",
            "TLS allocation request does not match the declared TlsAlloc thunk",
        ));
    }
    let contract =
        tls_alloc_contract(ApiEndpointIdentity::from_import_thunk(&thunk)).map_err(api_fault)?;

    let last_error_before = runtime.current_last_error().map_err(runtime_state_error)?;
    let (
        stack_reconciliation,
        endpoint_store_reconciliation,
        first_endpoint_store_value,
        second_endpoint_store_value,
        third_endpoint_store_value,
        fourth_endpoint_store_value,
        logical_index,
    ) = {
        let mut transaction = memory.begin_transaction();
        let stack_reconciliation = transaction
            .reconcile_reflected_page(
                request.reflected_generation,
                MemoryOwnerId(request.stack_page_owner),
                GuestAddress(request.stack_page_address),
                &request.stack_page_bytes,
            )
            .map_err(memory_error)?;
        let endpoint_store_reconciliation = transaction
            .reconcile_reflected_page(
                request.reflected_generation,
                MemoryOwnerId(request.endpoint_store_page_owner),
                GuestAddress(request.endpoint_store_page_address),
                &request.endpoint_store_page_bytes,
            )
            .map_err(memory_error)?;
        validate_direct_iat_callsite(&transaction, &thunk, request.return_address)?;
        let read_stored_endpoint =
            |address: u32, label: &'static str| -> Result<u32, LoaderError> {
                let stored_bytes = transaction
                    .read(GuestAddress(address), 4)
                    .map_err(memory_error)?;
                Ok(u32::from_le_bytes(
                    stored_bytes.as_slice().try_into().map_err(|_| {
                        LoaderError::runtime_defect(
                            "win32-thread-local-storage",
                            "endpoint-store-width-invalid",
                            format!("checked {label} endpoint store did not contain four bytes"),
                        )
                    })?,
                ))
            };
        let first_endpoint_store_value =
            read_stored_endpoint(request.first_endpoint_store_address, "first")?;
        let second_endpoint_store_value =
            read_stored_endpoint(request.second_endpoint_store_address, "second")?;
        let third_endpoint_store_value =
            read_stored_endpoint(request.third_endpoint_store_address, "third")?;
        let fourth_endpoint_store_value =
            read_stored_endpoint(request.fourth_endpoint_store_address, "fourth")?;
        if [
            first_endpoint_store_value,
            second_endpoint_store_value,
            third_endpoint_store_value,
            fourth_endpoint_store_value,
        ] != expected_addresses
        {
            return Err(LoaderError::invalid_input(
                "win32-thread-local-storage",
                "endpoint-store-value-mismatch",
                "BGI did not preserve all four dynamic-export endpoints before TlsAlloc",
            ));
        }
        let completion = dispatch_runtime_immediate(
            &mut transaction,
            runtime,
            binding,
            contract,
            controlled_api_trap(
                request.thunk_id,
                request.thunk_address,
                request.stack_pointer,
                request.return_address,
                request.argument_bytes,
            ),
        )?;
        let logical_index = set_eax_result(
            &completion,
            "tls-alloc-return-register-invalid",
            "TlsAlloc dispatcher did not set EAX",
        )?;
        if logical_index != request.logical_index {
            return Err(LoaderError::runtime_defect(
                "win32-thread-local-storage",
                "tls-index-runtime-allocation-mismatch",
                "Rust runtime TLS allocation did not produce the evidenced logical index",
            ));
        }
        transaction.commit_transaction();
        (
            stack_reconciliation,
            endpoint_store_reconciliation,
            first_endpoint_store_value,
            second_endpoint_store_value,
            third_endpoint_store_value,
            fourth_endpoint_store_value,
            logical_index,
        )
    };
    if runtime.current_last_error().map_err(runtime_state_error)? != last_error_before {
        return Err(LoaderError::runtime_defect(
            "win32-thread-local-storage",
            "tls-alloc-last-error-changed",
            "logical TLS allocation changed the current thread LastError",
        ));
    }

    let mut logical_tls_slot = X86LogicalTlsSlot {
        owner: request.tls_state_owner,
        variant_id: request.variant_id,
        index: X86LogicalTlsIndex::first(),
        lifecycle: "allocated",
        storage_scope: "thread",
        initialized_thread_values: runtime.initialized_tls_thread_count(logical_index) as u32,
        backing_materialization: "metadata-only-until-first-reached-tls-operation",
    };
    let result = logical_index;
    write_protocol_message(
        &V86TlsAllocReturnReceipt {
            schema: "vine.v86-tls-alloc-return-receipt",
            schema_version: 1,
            thunk,
            calling_convention: "stdcall",
            argument_bytes: request.argument_bytes,
            stack_pointer: GuestAddress(request.stack_pointer),
            return_address: GuestAddress(request.return_address),
            logical_tls_slot: logical_tls_slot.clone(),
            preserve_last_error: request.preserve_last_error,
            result,
            first_endpoint_store_address: GuestAddress(request.first_endpoint_store_address),
            first_endpoint_store_value,
            second_endpoint_store_address: GuestAddress(request.second_endpoint_store_address),
            second_endpoint_store_value,
            third_endpoint_store_address: GuestAddress(request.third_endpoint_store_address),
            third_endpoint_store_value,
            fourth_endpoint_store_address: GuestAddress(request.fourth_endpoint_store_address),
            fourth_endpoint_store_value,
            stack_reconciliation,
            endpoint_store_reconciliation,
            stats: memory.stats(),
        },
        message_limit,
    )?;
    if return_sessions.includes(15) {
        run_v86_tls_set_value_return_session(
            input,
            memory,
            runtime,
            binding,
            message_limit,
            return_sessions,
            &mut logical_tls_slot,
        )?;
    }
    Ok(())
}

fn run_v86_tls_set_value_return_session(
    input: &mut impl BufRead,
    memory: &mut GuestMemory,
    runtime: &mut RuntimeState,
    binding: &ImportBinding,
    message_limit: usize,
    return_sessions: V86ReturnSessionOptions<'_>,
    logical_tls_slot: &mut X86LogicalTlsSlot,
) -> Result<(), LoaderError> {
    const TLS_STATE_OWNER: &str = "checkpoint-thread-local-storage";
    const ALLOCATION_VARIANT_ID: &str = "tlsalloc-first-controlled-index";
    const THREAD_IDENTITY_OWNER: &str = "checkpoint-thread-identity";
    const VALUE_KIND: &str = "dynamic-export-endpoint";
    const ENDPOINT_STORE_PAGE_ADDRESS: u32 = 0x100f_e000;
    const TLS_INDEX_STORE_ADDRESS: u32 = 0x100f_0764;
    const TLS_INDEX_STORE_PAGE_ADDRESS: u32 = 0x100f_0000;
    const ENDPOINT_STORE_ADDRESSES: [u32; 4] = [0x100f_eebc, 0x100f_eec0, 0x100f_eec4, 0x100f_eec8];

    let endpoints = return_sessions.dynamic_export_thunks;
    let expected_endpoints = [
        (1, 0x7f02_0000, "FlsAlloc"),
        (2, 0x7f02_0010, "FlsGetValue"),
        (3, 0x7f02_0020, "FlsSetValue"),
        (4, 0x7f02_0030, "FlsFree"),
    ];
    let first_endpoint = endpoints.first().ok_or_else(|| {
        LoaderError::runtime_defect(
            "win32-thread-local-storage",
            "dynamic-export-endpoints-missing",
            "TlsSetValue return session has no preserved FLS endpoints",
        )
    })?;
    if endpoints.len() != expected_endpoints.len()
        || endpoints
            .iter()
            .zip(expected_endpoints)
            .any(|(observed, (id, address, name))| {
                observed.id.0 != id
                    || observed.address.0 != address
                    || !observed.module.eq_ignore_ascii_case("kernel32.dll")
                    || observed.name != name
                    || observed.owner != first_endpoint.owner
                    || observed.arena_range != first_endpoint.arena_range
                    || observed.content_sha256 != first_endpoint.content_sha256
                    || observed.invalidation != first_endpoint.invalidation
                    || observed.lifetime != "process-runtime"
                    || !observed.owner_sealed
            })
    {
        return Err(LoaderError::runtime_defect(
            "win32-thread-local-storage",
            "dynamic-export-endpoint-quadruple-mismatch",
            "TlsSetValue return session did not retain the controlled FLS identity quadruple",
        ));
    }
    let module_handle = X86LogicalLoadedModuleHandle::kernel32().value;
    let runtime_endpoints = endpoints
        .iter()
        .map(|endpoint| runtime_dynamic_endpoint(runtime, module_handle, endpoint))
        .collect::<Result<Vec<_>, _>>()?;
    let endpoint_addresses = [
        runtime_endpoints[0].address.0,
        runtime_endpoints[1].address.0,
        runtime_endpoints[2].address.0,
        runtime_endpoints[3].address.0,
    ];
    let value_endpoint = &endpoints[1];
    let runtime_value_endpoint = &runtime_endpoints[1];
    let initialized_thread_values =
        runtime.initialized_tls_thread_count(logical_tls_slot.index.value) as u32;
    let current_thread_id = runtime.current_thread_id().0;

    let request: V86TlsSetValueReturnRequest = read_protocol_message(
        input,
        96 * 1024,
        "win32-thread-local-storage",
        "tls-set-value-return-request",
        "controlled initial-thread TLS value request",
    )?;
    if request.schema != "vine.v86-tls-set-value-return-request" || request.schema_version != 1 {
        return Err(LoaderError::invalid_input(
            "win32-thread-local-storage",
            "tls-set-value-return-request-schema-unsupported",
            "TLS value request must use vine.v86-tls-set-value-return-request version 1",
        ));
    }
    if request.argument_bytes != 8 {
        return Err(LoaderError::invalid_input(
            "abi-gateway",
            "tls-set-value-argument-width-mismatch",
            "TlsSetValue requires two four-byte stdcall arguments",
        ));
    }
    if logical_tls_slot.owner != TLS_STATE_OWNER
        || logical_tls_slot.variant_id != ALLOCATION_VARIANT_ID
        || logical_tls_slot.index.namespace != X86LogicalTlsIndex::NAMESPACE
        || logical_tls_slot.index.value != X86LogicalTlsIndex::FIRST_INDEX
        || logical_tls_slot.lifecycle != "allocated"
        || logical_tls_slot.storage_scope != "thread"
        || logical_tls_slot.initialized_thread_values != initialized_thread_values
        || request.tls_state_owner != TLS_STATE_OWNER
        || request.allocation_variant_id != ALLOCATION_VARIANT_ID
        || request.logical_index_namespace != X86LogicalTlsIndex::NAMESPACE
        || request.logical_index != logical_tls_slot.index.value
        || request.expected_initialized_thread_values != initialized_thread_values
        || request.thread_identity_owner != THREAD_IDENTITY_OWNER
        || request.thread_id != current_thread_id
        || request.value_kind != VALUE_KIND
        || request.value_endpoint_namespace != runtime_value_endpoint.namespace
        || request.value_endpoint_id != runtime_value_endpoint.id
        || request.value_address != runtime_value_endpoint.address.0
        || request.endpoint_store_addresses != ENDPOINT_STORE_ADDRESSES
        || request.endpoint_addresses != endpoint_addresses
        || request.endpoint_store_page_address != ENDPOINT_STORE_PAGE_ADDRESS
        || request.endpoint_store_page_owner != binding.image_owner.0
        || request.tls_index_store_address != TLS_INDEX_STORE_ADDRESS
        || request.tls_index_store_page_address != TLS_INDEX_STORE_PAGE_ADDRESS
        || request.tls_index_store_page_owner != binding.image_owner.0
        || !request.preserve_last_error
    {
        return Err(LoaderError::invalid_input(
            "win32-thread-local-storage",
            "tls-set-value-variant-unsupported",
            "controlled TLS value supports only logical index 0, initial thread 8192, and the FlsGetValue endpoint",
        ));
    }

    let thunk = owned_import_thunk(binding, request.thunk_id)?;
    let contract = tls_set_value_contract(ApiEndpointIdentity::from_import_thunk(&thunk))
        .map_err(api_fault)?;

    let last_error_before = runtime.current_last_error().map_err(runtime_state_error)?;
    let (
        stack_reconciliation,
        endpoint_store_reconciliation,
        tls_index_store_reconciliation,
        endpoint_store_values,
        tls_index_store_value,
        result,
    ) = {
        let mut transaction = memory.begin_transaction();
        let stack_reconciliation = transaction
            .reconcile_reflected_page(
                request.reflected_generation,
                MemoryOwnerId(request.stack_page_owner),
                GuestAddress(request.stack_page_address),
                &request.stack_page_bytes,
            )
            .map_err(memory_error)?;
        let endpoint_store_reconciliation = transaction
            .reconcile_reflected_page(
                request.reflected_generation,
                MemoryOwnerId(request.endpoint_store_page_owner),
                GuestAddress(request.endpoint_store_page_address),
                &request.endpoint_store_page_bytes,
            )
            .map_err(memory_error)?;
        let tls_index_store_reconciliation = transaction
            .reconcile_reflected_page(
                request.reflected_generation,
                MemoryOwnerId(request.tls_index_store_page_owner),
                GuestAddress(request.tls_index_store_page_address),
                &request.tls_index_store_page_bytes,
            )
            .map_err(memory_error)?;
        validate_esi_iat_callsite(
            &transaction,
            &thunk,
            request.return_address,
            &request.indirect_call_register,
            request.indirect_call_target,
        )?;
        let mut endpoint_store_values = [0; 4];
        for (index, address) in request.endpoint_store_addresses.into_iter().enumerate() {
            let bytes = transaction
                .read(GuestAddress(address), 4)
                .map_err(memory_error)?;
            endpoint_store_values[index] =
                u32::from_le_bytes(bytes.as_slice().try_into().map_err(|_| {
                    LoaderError::runtime_defect(
                        "win32-thread-local-storage",
                        "endpoint-store-width-invalid",
                        "checked endpoint store did not contain four bytes",
                    )
                })?);
        }
        if endpoint_store_values != endpoint_addresses {
            return Err(LoaderError::invalid_input(
                "win32-thread-local-storage",
                "endpoint-store-value-mismatch",
                "BGI did not preserve all four dynamic-export endpoints before TlsSetValue",
            ));
        }
        let tls_index_store_bytes = transaction
            .read(GuestAddress(request.tls_index_store_address), 4)
            .map_err(memory_error)?;
        let tls_index_store_value =
            u32::from_le_bytes(tls_index_store_bytes.as_slice().try_into().map_err(|_| {
                LoaderError::runtime_defect(
                    "win32-thread-local-storage",
                    "tls-index-store-width-invalid",
                    "checked TLS index store did not contain four bytes",
                )
            })?);
        if tls_index_store_value != request.logical_index {
            return Err(LoaderError::invalid_input(
                "win32-thread-local-storage",
                "tls-index-store-value-mismatch",
                "BGI did not preserve the allocated logical TLS index before TlsSetValue",
            ));
        }
        let completion = dispatch_runtime_immediate(
            &mut transaction,
            runtime,
            binding,
            contract,
            controlled_api_trap(
                request.thunk_id,
                request.thunk_address,
                request.stack_pointer,
                request.return_address,
                request.argument_bytes,
            ),
        )?;
        let result = set_eax_result(
            &completion,
            "tls-set-value-return-register-invalid",
            "TlsSetValue dispatcher did not set EAX",
        )?;
        if result != 1 {
            return Err(LoaderError::runtime_defect(
                "api-dispatch",
                "tls-set-value-result-invalid",
                "TlsSetValue dispatcher did not return canonical TRUE",
            ));
        }
        transaction.commit_transaction();
        (
            stack_reconciliation,
            endpoint_store_reconciliation,
            tls_index_store_reconciliation,
            endpoint_store_values,
            tls_index_store_value,
            result,
        )
    };
    let runtime_tls_value = runtime
        .tls_value(request.logical_index)
        .map_err(runtime_state_error)?
        .clone();
    if runtime_tls_value
        != (RuntimeTlsValue::DynamicEndpoint {
            endpoint_id: runtime_value_endpoint.id,
            address: runtime_value_endpoint.address,
        })
        || runtime.current_last_error().map_err(runtime_state_error)? != last_error_before
    {
        return Err(LoaderError::runtime_defect(
            "win32-thread-local-storage",
            "tls-set-value-runtime-transition-invalid",
            "TlsSetValue did not update initial-thread TLS metadata while preserving LastError",
        ));
    }

    let logical_tls_slot_before = logical_tls_slot.clone();
    logical_tls_slot.initialized_thread_values =
        runtime.initialized_tls_thread_count(request.logical_index) as u32;
    logical_tls_slot.backing_materialization = "initial-thread-value-metadata";
    let thread_value = X86LogicalTlsThreadValue {
        thread_identity_owner: request.thread_identity_owner,
        thread_id: current_thread_id,
        index_namespace: X86LogicalTlsIndex::NAMESPACE,
        index: request.logical_index,
        value_kind: VALUE_KIND,
        endpoint: value_endpoint.clone(),
        lifecycle: "set",
    };
    write_protocol_message(
        &V86TlsSetValueReturnReceipt {
            schema: "vine.v86-tls-set-value-return-receipt",
            schema_version: 1,
            thunk,
            calling_convention: "stdcall",
            argument_bytes: request.argument_bytes,
            stack_pointer: GuestAddress(request.stack_pointer),
            return_address: GuestAddress(request.return_address),
            logical_tls_slot_before,
            logical_tls_slot_after: logical_tls_slot.clone(),
            thread_value: thread_value.clone(),
            preserve_last_error: request.preserve_last_error,
            result,
            endpoint_store_addresses: ENDPOINT_STORE_ADDRESSES.map(GuestAddress),
            endpoint_store_values,
            tls_index_store_address: GuestAddress(request.tls_index_store_address),
            tls_index_store_value,
            stack_reconciliation,
            endpoint_store_reconciliation,
            tls_index_store_reconciliation,
            stats: memory.stats(),
        },
        message_limit,
    )?;
    if return_sessions.includes(16) {
        run_v86_encode_pointer_return_session(
            input,
            memory,
            runtime,
            binding,
            message_limit,
            return_sessions,
            X86TlsEvidenceState {
                slot: logical_tls_slot,
                thread_value: &thread_value,
            },
        )?;
    }
    Ok(())
}

fn run_v86_encode_pointer_return_session(
    input: &mut impl BufRead,
    memory: &mut GuestMemory,
    runtime: &mut RuntimeState,
    binding: &ImportBinding,
    message_limit: usize,
    return_sessions: V86ReturnSessionOptions<'_>,
    tls_evidence: X86TlsEvidenceState<'_>,
) -> Result<(), LoaderError> {
    const OWNER_ID: &str = "checkpoint-pointer-encoding";
    const VARIANT_ID: &str = "encodepointer-null-process-token";
    let logical_tls_slot = tls_evidence.slot;
    let tls_thread_value = tls_evidence.thread_value;
    let runtime_tls_value = runtime
        .tls_value(logical_tls_slot.index.value)
        .map_err(runtime_state_error)?;
    let (runtime_endpoint_id, runtime_endpoint_address) = match runtime_tls_value {
        RuntimeTlsValue::DynamicEndpoint {
            endpoint_id,
            address,
        } => (*endpoint_id, *address),
        RuntimeTlsValue::Raw { .. } => {
            return Err(LoaderError::runtime_defect(
                "win32-pointer-encoding",
                "runtime-tls-endpoint-value-missing",
                "EncodePointer requires the initial thread TLS value to identify a dynamic endpoint",
            ));
        }
    };
    let runtime_tls_endpoint = runtime
        .dynamic_endpoint(runtime_endpoint_id)
        .map_err(runtime_state_error)?;
    let runtime_thread_id = runtime.current_thread_id().0;
    if logical_tls_slot.owner != "checkpoint-thread-local-storage"
        || logical_tls_slot.variant_id != "tlsalloc-first-controlled-index"
        || logical_tls_slot.index.namespace != X86LogicalTlsIndex::NAMESPACE
        || logical_tls_slot.index.value != X86LogicalTlsIndex::FIRST_INDEX
        || logical_tls_slot.index.failure_sentinel != X86LogicalTlsIndex::FAILURE_SENTINEL
        || logical_tls_slot.lifecycle != "allocated"
        || logical_tls_slot.storage_scope != "thread"
        || logical_tls_slot.initialized_thread_values
            != runtime.initialized_tls_thread_count(logical_tls_slot.index.value) as u32
        || logical_tls_slot.backing_materialization != "initial-thread-value-metadata"
        || tls_thread_value.thread_identity_owner != "checkpoint-thread-identity"
        || tls_thread_value.thread_id != runtime_thread_id
        || tls_thread_value.index_namespace != X86LogicalTlsIndex::NAMESPACE
        || tls_thread_value.index != logical_tls_slot.index.value
        || tls_thread_value.value_kind != "dynamic-export-endpoint"
        || tls_thread_value.endpoint.namespace != "vine.dynamic-export-endpoint.v1"
        || tls_thread_value.endpoint.id.0 != runtime_tls_endpoint.id
        || tls_thread_value.endpoint.address != runtime_endpoint_address
        || !tls_thread_value
            .endpoint
            .module
            .eq_ignore_ascii_case("kernel32.dll")
        || tls_thread_value.endpoint.name != "FlsGetValue"
        || tls_thread_value.endpoint.name != runtime_tls_endpoint.name
        || tls_thread_value.endpoint.lifetime != "process-runtime"
        || !tls_thread_value.endpoint.owner_sealed
        || tls_thread_value.lifecycle != "set"
    {
        return Err(LoaderError::runtime_defect(
            "win32-pointer-encoding",
            "prior-tls-state-mismatch",
            "EncodePointer return session did not retain the controlled initial-thread TLS state",
        ));
    }

    let request: V86EncodePointerReturnRequest = read_protocol_message(
        input,
        48 * 1024,
        "win32-pointer-encoding",
        "encode-pointer-return-request",
        "controlled encoded NULL request",
    )?;
    if request.schema != "vine.v86-encode-pointer-return-request" || request.schema_version != 1 {
        return Err(LoaderError::invalid_input(
            "win32-pointer-encoding",
            "encode-pointer-return-request-schema-unsupported",
            "pointer encoding request must use vine.v86-encode-pointer-return-request version 1",
        ));
    }
    if request.argument_bytes != 4 || request.source_pointer != 0 {
        return Err(LoaderError::invalid_input(
            "abi-gateway",
            "encode-pointer-argument-mismatch",
            "controlled EncodePointer supports only one NULL argument",
        ));
    }
    if request.pointer_encoding_owner != OWNER_ID
        || request.variant_id != VARIANT_ID
        || request.encoded_pointer_namespace != runtime.encoded_pointer_namespace()
        || request.encoded_pointer_value != X86LogicalEncodedPointer::ENCODED_NULL
        || request.expected_tls_index != logical_tls_slot.index.value
        || request.expected_tls_thread_id != runtime_thread_id
        || request.expected_tls_value_endpoint != runtime_endpoint_address.0
        || !request.preserve_last_error
    {
        return Err(LoaderError::invalid_input(
            "win32-pointer-encoding",
            "encode-pointer-variant-unsupported",
            "controlled pointer encoding supports only the retained TLS state and deterministic encoded NULL token",
        ));
    }

    let thunk = owned_import_thunk(binding, request.thunk_id)?;
    let contract = encode_pointer_contract(ApiEndpointIdentity::from_import_thunk(&thunk))
        .map_err(api_fault)?;

    let last_error_before = runtime.current_last_error().map_err(runtime_state_error)?;
    let (stack_reconciliation, result) = {
        let mut transaction = memory.begin_transaction();
        let reconciliation = transaction
            .reconcile_reflected_page(
                request.reflected_generation,
                MemoryOwnerId(request.stack_page_owner),
                GuestAddress(request.stack_page_address),
                &request.stack_page_bytes,
            )
            .map_err(memory_error)?;
        validate_direct_iat_callsite(&transaction, &thunk, request.return_address)?;
        let completion = dispatch_runtime_immediate(
            &mut transaction,
            runtime,
            binding,
            contract,
            controlled_api_trap(
                request.thunk_id,
                request.thunk_address,
                request.stack_pointer,
                request.return_address,
                request.argument_bytes,
            ),
        )?;
        let result = set_eax_result(
            &completion,
            "encode-pointer-return-register-invalid",
            "EncodePointer dispatcher did not set EAX",
        )?;
        if result != X86LogicalEncodedPointer::ENCODED_NULL {
            return Err(LoaderError::runtime_defect(
                "api-dispatch",
                "encode-pointer-result-invalid",
                "EncodePointer dispatcher returned the wrong controlled token",
            ));
        }
        transaction.commit_transaction();
        (reconciliation, result)
    };
    let runtime_encoded_pointer = runtime
        .encoded_pointer_for_source(request.source_pointer)
        .map_err(runtime_state_error)?;
    if result != runtime_encoded_pointer.token
        || request.encoded_pointer_value != runtime_encoded_pointer.token
        || request.encoded_pointer_namespace != runtime_encoded_pointer.namespace
        || runtime.current_last_error().map_err(runtime_state_error)? != last_error_before
    {
        return Err(LoaderError::runtime_defect(
            "win32-pointer-encoding",
            "encode-pointer-runtime-transition-invalid",
            "EncodePointer did not create the evidenced process token while preserving LastError",
        ));
    }

    let encoded_pointer = X86LogicalEncodedPointer {
        owner: request.pointer_encoding_owner,
        variant_id: request.variant_id,
        namespace: X86LogicalEncodedPointer::NAMESPACE,
        value: result,
        source_kind: "null",
        source_pointer: request.source_pointer,
        lifetime: "process-runtime",
        process_cookie_materialization: "not-imported",
        native_result_imported: false,
    };
    write_protocol_message(
        &V86EncodePointerReturnReceipt {
            schema: "vine.v86-encode-pointer-return-receipt",
            schema_version: 1,
            thunk,
            calling_convention: "stdcall",
            argument_bytes: request.argument_bytes,
            stack_pointer: GuestAddress(request.stack_pointer),
            return_address: GuestAddress(request.return_address),
            encoded_pointer: encoded_pointer.clone(),
            preserved_tls_slot: logical_tls_slot.clone(),
            preserved_tls_thread_value: tls_thread_value.clone(),
            preserve_last_error: request.preserve_last_error,
            result,
            stack_reconciliation,
            stats: memory.stats(),
        },
        message_limit,
    )?;
    if return_sessions.includes(17) {
        run_v86_encode_pointer_second_return_session(
            input,
            memory,
            runtime,
            binding,
            message_limit,
            tls_evidence,
            &encoded_pointer,
        )?;
    }
    Ok(())
}

fn run_v86_encode_pointer_second_return_session(
    input: &mut impl BufRead,
    memory: &mut GuestMemory,
    runtime: &mut RuntimeState,
    binding: &ImportBinding,
    message_limit: usize,
    tls_evidence: X86TlsEvidenceState<'_>,
    encoded_null: &X86LogicalEncodedPointer,
) -> Result<(), LoaderError> {
    const OWNER_ID: &str = "checkpoint-pointer-encoding";
    const VARIANT_ID: &str = "encodepointer-process-initializer-function-token";
    const SOURCE_POINTER: u32 = 0x100a_9608;
    const SOURCE_PAGE_ADDRESS: u32 = 0x100a_9000;
    const SOURCE_FUNCTION_PREFIX: [u8; 17] = [
        0x6a, 0x08, 0x68, 0xf8, 0xb2, 0x0e, 0x10, 0xe8, 0x5c, 0xbc, 0xff, 0xff, 0xe8, 0x33, 0xc0,
        0xff, 0xff,
    ];
    let logical_tls_slot = tls_evidence.slot;
    let tls_thread_value = tls_evidence.thread_value;
    let runtime_encoded_null = runtime
        .encoded_pointer_for_source(0)
        .map_err(runtime_state_error)?
        .clone();
    let runtime_tls_value = runtime
        .tls_value(logical_tls_slot.index.value)
        .map_err(runtime_state_error)?;
    let (runtime_endpoint_id, runtime_endpoint_address) = match runtime_tls_value {
        RuntimeTlsValue::DynamicEndpoint {
            endpoint_id,
            address,
        } => (*endpoint_id, *address),
        RuntimeTlsValue::Raw { .. } => {
            return Err(LoaderError::runtime_defect(
                "win32-pointer-encoding",
                "runtime-tls-endpoint-value-missing",
                "second EncodePointer requires the initial thread TLS value to identify a dynamic endpoint",
            ));
        }
    };
    let runtime_tls_endpoint = runtime
        .dynamic_endpoint(runtime_endpoint_id)
        .map_err(runtime_state_error)?;
    let runtime_thread_id = runtime.current_thread_id().0;

    if encoded_null.owner != OWNER_ID
        || encoded_null.variant_id != "encodepointer-null-process-token"
        || encoded_null.namespace != X86LogicalEncodedPointer::NAMESPACE
        || encoded_null.value != runtime_encoded_null.token
        || encoded_null.source_kind != "null"
        || encoded_null.source_pointer != 0
        || encoded_null.lifetime != "process-runtime"
        || encoded_null.process_cookie_materialization != "not-imported"
        || encoded_null.native_result_imported
        || logical_tls_slot.owner != "checkpoint-thread-local-storage"
        || logical_tls_slot.variant_id != "tlsalloc-first-controlled-index"
        || logical_tls_slot.index.namespace != X86LogicalTlsIndex::NAMESPACE
        || logical_tls_slot.index.value != X86LogicalTlsIndex::FIRST_INDEX
        || logical_tls_slot.index.failure_sentinel != X86LogicalTlsIndex::FAILURE_SENTINEL
        || logical_tls_slot.lifecycle != "allocated"
        || logical_tls_slot.storage_scope != "thread"
        || logical_tls_slot.initialized_thread_values
            != runtime.initialized_tls_thread_count(logical_tls_slot.index.value) as u32
        || logical_tls_slot.backing_materialization != "initial-thread-value-metadata"
        || tls_thread_value.thread_identity_owner != "checkpoint-thread-identity"
        || tls_thread_value.thread_id != runtime_thread_id
        || tls_thread_value.index_namespace != X86LogicalTlsIndex::NAMESPACE
        || tls_thread_value.index != logical_tls_slot.index.value
        || tls_thread_value.value_kind != "dynamic-export-endpoint"
        || tls_thread_value.endpoint.namespace != "vine.dynamic-export-endpoint.v1"
        || tls_thread_value.endpoint.id.0 != runtime_tls_endpoint.id
        || tls_thread_value.endpoint.address != runtime_endpoint_address
        || !tls_thread_value
            .endpoint
            .module
            .eq_ignore_ascii_case("kernel32.dll")
        || tls_thread_value.endpoint.name != "FlsGetValue"
        || tls_thread_value.endpoint.name != runtime_tls_endpoint.name
        || tls_thread_value.endpoint.lifetime != "process-runtime"
        || !tls_thread_value.endpoint.owner_sealed
        || tls_thread_value.lifecycle != "set"
    {
        return Err(LoaderError::runtime_defect(
            "win32-pointer-encoding",
            "prior-pointer-or-tls-state-mismatch",
            "second EncodePointer return session did not retain the encoded NULL and initial-thread TLS state",
        ));
    }

    let request: V86EncodePointerSecondReturnRequest = read_protocol_message(
        input,
        48 * 1024,
        "win32-pointer-encoding",
        "encode-pointer-second-return-request",
        "controlled process-initializer function encoding request",
    )?;
    if request.schema != "vine.v86-encode-pointer-second-return-request"
        || request.schema_version != 1
    {
        return Err(LoaderError::invalid_input(
            "win32-pointer-encoding",
            "encode-pointer-second-return-request-schema-unsupported",
            "second pointer encoding request must use vine.v86-encode-pointer-second-return-request version 1",
        ));
    }
    if request.argument_bytes != 4 || request.source_pointer != SOURCE_POINTER {
        return Err(LoaderError::invalid_input(
            "abi-gateway",
            "encode-pointer-second-argument-mismatch",
            "controlled second EncodePointer supports only the process-initializer guest function",
        ));
    }
    if request.source_page_address != SOURCE_PAGE_ADDRESS
        || request.source_page_owner != binding.image_owner.0
        || request.source_function_prefix != SOURCE_FUNCTION_PREFIX
        || request.pointer_encoding_owner != OWNER_ID
        || request.variant_id != VARIANT_ID
        || request.encoded_pointer_namespace != runtime.encoded_pointer_namespace()
        || request.encoded_pointer_value != X86LogicalEncodedPointer::ENCODED_PROCESS_INITIALIZER
        || request.expected_encoded_null_namespace != runtime_encoded_null.namespace
        || request.expected_encoded_null_value != runtime_encoded_null.token
        || request.expected_tls_index != logical_tls_slot.index.value
        || request.expected_tls_thread_id != runtime_thread_id
        || request.expected_tls_value_endpoint != runtime_endpoint_address.0
        || !request.preserve_last_error
    {
        return Err(LoaderError::invalid_input(
            "win32-pointer-encoding",
            "encode-pointer-second-variant-unsupported",
            "controlled pointer encoding supports only the retained state and process-initializer guest function token",
        ));
    }

    let thunk = owned_import_thunk(binding, request.thunk_id)?;
    let contract = encode_pointer_contract(ApiEndpointIdentity::from_import_thunk(&thunk))
        .map_err(api_fault)?;

    let last_error_before = runtime.current_last_error().map_err(runtime_state_error)?;
    let (stack_reconciliation, source_function_prefix, result) = {
        let mut transaction = memory.begin_transaction();
        let stack_reconciliation = transaction
            .reconcile_reflected_page(
                request.reflected_generation,
                MemoryOwnerId(request.stack_page_owner),
                GuestAddress(request.stack_page_address),
                &request.stack_page_bytes,
            )
            .map_err(memory_error)?;
        validate_direct_iat_callsite(&transaction, &thunk, request.return_address)?;
        let source_function_prefix = transaction
            .fetch(
                GuestAddress(request.source_pointer),
                SOURCE_FUNCTION_PREFIX.len(),
            )
            .map_err(memory_error)?;
        if source_function_prefix != SOURCE_FUNCTION_PREFIX {
            return Err(LoaderError::invalid_input(
                "win32-pointer-encoding",
                "process-initializer-function-prefix-mismatch",
                "encoded source did not match the controlled executable process-initializer function",
            ));
        }
        let completion = dispatch_runtime_immediate(
            &mut transaction,
            runtime,
            binding,
            contract,
            controlled_api_trap(
                request.thunk_id,
                request.thunk_address,
                request.stack_pointer,
                request.return_address,
                request.argument_bytes,
            ),
        )?;
        let result = set_eax_result(
            &completion,
            "encode-pointer-second-return-register-invalid",
            "second EncodePointer dispatcher did not set EAX",
        )?;
        if result != X86LogicalEncodedPointer::ENCODED_PROCESS_INITIALIZER {
            return Err(LoaderError::runtime_defect(
                "api-dispatch",
                "encode-pointer-second-result-invalid",
                "second EncodePointer dispatcher returned the wrong controlled token",
            ));
        }
        transaction.commit_transaction();
        (stack_reconciliation, source_function_prefix, result)
    };
    let runtime_encoded_pointer = runtime
        .encoded_pointer_for_source(request.source_pointer)
        .map_err(runtime_state_error)?;
    if result != runtime_encoded_pointer.token
        || request.encoded_pointer_value != runtime_encoded_pointer.token
        || request.encoded_pointer_namespace != runtime_encoded_pointer.namespace
        || runtime.current_last_error().map_err(runtime_state_error)? != last_error_before
    {
        return Err(LoaderError::runtime_defect(
            "win32-pointer-encoding",
            "encode-pointer-second-runtime-transition-invalid",
            "second EncodePointer did not create the evidenced process token while preserving LastError",
        ));
    }

    let encoded_pointer = X86LogicalEncodedPointer {
        owner: request.pointer_encoding_owner,
        variant_id: request.variant_id,
        namespace: X86LogicalEncodedPointer::NAMESPACE,
        value: result,
        source_kind: "guest-executable-function",
        source_pointer: request.source_pointer,
        lifetime: "process-runtime",
        process_cookie_materialization: "not-imported",
        native_result_imported: false,
    };
    write_protocol_message(
        &V86EncodePointerSecondReturnReceipt {
            schema: "vine.v86-encode-pointer-second-return-receipt",
            schema_version: 1,
            thunk,
            calling_convention: "stdcall",
            argument_bytes: request.argument_bytes,
            stack_pointer: GuestAddress(request.stack_pointer),
            return_address: GuestAddress(request.return_address),
            source_function_prefix,
            encoded_pointer,
            preserved_encoded_null: encoded_null.clone(),
            preserved_tls_slot: logical_tls_slot.clone(),
            preserved_tls_thread_value: tls_thread_value.clone(),
            preserve_last_error: request.preserve_last_error,
            result,
            stack_reconciliation,
            stats: memory.stats(),
        },
        message_limit,
    )
}

fn controlled_runtime_state(
    dynamic_export_thunks: &[DynamicExportThunk],
) -> Result<RuntimeState, LoaderError> {
    controlled_runtime_state_with_clock_profile(dynamic_export_thunks, None)
}

fn controlled_runtime_state_with_clock_profile(
    dynamic_export_thunks: &[DynamicExportThunk],
    clock_profile: Option<vine_core::RuntimeClockProfile>,
) -> Result<RuntimeState, LoaderError> {
    const PROCESS_ID: u32 = 4096;
    const INITIAL_THREAD_ID: u32 = 8192;
    const INITIAL_LAST_ERROR: u32 = 87;
    const WALL_CLOCK_FILE_TIME: u64 = 134_282_016_000_000_000;
    const TICK_COUNT: u32 = 3_600_000;
    const PERFORMANCE_COUNTER: u64 = 12_345_678_901;
    const PERFORMANCE_FREQUENCY: u64 = 10_000_000;
    const ENCODED_POINTER_TOKEN_PREFIX: u32 = 0xec01_0000;

    let startup_info = X86StartupInfoW::controlled();
    let mut runtime = RuntimeState::new(RuntimeBootstrap {
        process_id: RuntimeProcessId(PROCESS_ID),
        initial_thread_id: RuntimeThreadId(INITIAL_THREAD_ID),
        initial_last_error: INITIAL_LAST_ERROR,
        address_space: ProcessAddressSpaceLayout::vine_v1(),
        clock: RuntimeClockState {
            wall_clock_file_time: WALL_CLOCK_FILE_TIME,
            tick_count: TICK_COUNT,
            performance_counter: PERFORMANCE_COUNTER,
            performance_frequency: PERFORMANCE_FREQUENCY,
        },
        startup_info: RuntimeStartupInfo {
            profile_id: X86StartupInfoW::PROFILE_ID.to_owned(),
            bytes: startup_info.to_bytes(),
        },
        process_image_path_a: vine_core::RuntimeProcessImagePathA {
            source_authority: "node-private-executable-virtual-path".to_owned(),
            bytes_with_nul: b"C:\\Vine\\BGI.exe\0".to_vec(),
            encoding: vine_core::RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING.to_owned(),
        },
        initial_current_directory_a: vine_core::RuntimeCurrentDirectoryA {
            source_authority: "node-private-process-bootstrap".to_owned(),
            bytes_with_nul: b"C:\\Vine\0".to_vec(),
            encoding: vine_core::RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING.to_owned(),
        },
        encoded_pointer_namespace: X86LogicalEncodedPointer::NAMESPACE.to_owned(),
        encoded_pointer_token_prefix: ENCODED_POINTER_TOKEN_PREFIX,
    })
    .map_err(runtime_state_error)?;
    if let Some(clock_profile) = clock_profile {
        runtime
            .configure_clock_profile(clock_profile)
            .map_err(runtime_state_error)?;
    }
    let kernel32_handle = X86LogicalLoadedModuleHandle::kernel32();
    runtime
        .load_module(RuntimeModule {
            owner: "runtime-loaded-modules".to_owned(),
            variant_id: "runtime-system-module-v1".to_owned(),
            namespace: kernel32_handle.namespace.to_owned(),
            handle: kernel32_handle.value,
            requested_name: "KERNEL32.DLL".to_owned(),
            canonical_name: "kernel32.dll".to_owned(),
            runtime_system_module: true,
        })
        .map_err(runtime_state_error)?;
    for thunk in dynamic_export_thunks {
        runtime
            .register_dynamic_export_thunk(thunk)
            .map_err(runtime_state_error)?;
    }
    runtime
        .start_initial_thread()
        .map_err(runtime_state_error)?;
    Ok(runtime)
}

fn runtime_dynamic_endpoint(
    runtime: &RuntimeState,
    module_handle: u32,
    thunk: &DynamicExportThunk,
) -> Result<RuntimeDynamicEndpoint, LoaderError> {
    let module = runtime.module(module_handle).map_err(runtime_state_error)?;
    let endpoint = runtime
        .dynamic_endpoint(thunk.id.0)
        .map_err(runtime_state_error)?;
    if !module.runtime_system_module
        || !module.canonical_name.eq_ignore_ascii_case(&endpoint.module)
        || endpoint.address != thunk.address
        || endpoint.module != thunk.module
        || endpoint.name != thunk.name
        || endpoint.namespace != thunk.namespace
        || endpoint.lifetime != thunk.lifetime
    {
        return Err(LoaderError::runtime_defect(
            "runtime-endpoint",
            "dynamic-endpoint-registry-mismatch",
            "runtime module and endpoint registries disagree with the installed guest thunk",
        ));
    }
    Ok(endpoint.clone())
}

fn dispatch_runtime_immediate(
    memory: &mut GuestMemory,
    runtime: &mut RuntimeState,
    binding: &ImportBinding,
    contract: ApiContract,
    trap: ApiTrap,
) -> Result<ApiImmediateCompletion, LoaderError> {
    let endpoints = binding
        .thunks
        .iter()
        .map(ApiEndpointIdentity::from_import_thunk)
        .collect::<Vec<_>>();
    let dispatcher = ApiDispatcher::new(endpoints, [contract]).map_err(api_fault)?;
    let mut session = ExecutionSession::new(memory, runtime);
    match dispatcher.dispatch(trap, &mut session, dispatch_kernel32_api) {
        DispatchOutcome::Immediate(completion) => Ok(completion),
        DispatchOutcome::Fault(fault) => Err(api_fault(fault)),
        DispatchOutcome::Deferred(_) | DispatchOutcome::GuestCallback(_) => {
            Err(LoaderError::runtime_defect(
                "api-dispatch",
                "controlled-immediate-outcome-invalid",
                "controlled synchronous API returned a deferred or callback outcome",
            ))
        }
    }
}

fn controlled_api_trap(
    thunk_id: u32,
    thunk_address: u32,
    stack_pointer: u32,
    return_address: u32,
    argument_bytes: u32,
) -> ApiTrap {
    ApiTrap {
        endpoint_id: vine_core::ImportThunkId(thunk_id),
        endpoint_address: GuestAddress(thunk_address),
        stack_pointer: GuestAddress(stack_pointer),
        return_address: GuestAddress(return_address),
        argument_bytes,
    }
}

fn owned_import_thunk(binding: &ImportBinding, thunk_id: u32) -> Result<ImportThunk, LoaderError> {
    binding
        .thunks
        .iter()
        .find(|thunk| thunk.id.0 == thunk_id)
        .cloned()
        .ok_or_else(|| {
            LoaderError::invalid_input(
                "api-dispatch",
                "endpoint-id-unowned",
                "API request does not name an owned import endpoint",
            )
        })
}

fn set_eax_result(
    completion: &ApiImmediateCompletion,
    code: &'static str,
    message: &'static str,
) -> Result<u32, LoaderError> {
    match completion.eax {
        RegisterUpdate::Set(value) => Ok(value),
        RegisterUpdate::Preserve => Err(LoaderError::runtime_defect("api-dispatch", code, message)),
    }
}

fn generic_runtime_imm32_completion(
    completion: &ApiImmediateCompletion,
    runtime: &RuntimeState,
    imm32_policy: &Imm32CompatibilityPolicy,
    user32_policy: &User32CompatibilityPolicy,
) -> Result<Option<GenericRuntimeImm32Completion>, LoaderError> {
    let app_association = |app_window_handle: u32| {
        runtime
            .ime_context_associations()
            .values()
            .find(|association| association.window_handle == app_window_handle)
            .cloned()
            .ok_or_else(|| {
                LoaderError::runtime_defect(
                    "runtime-imm32",
                    "ime-context-association-missing-after-dispatch",
                    "completed IMM32 call has no logical app-window association",
                )
            })
    };
    let associated_context = |association: &RuntimeImeContextAssociation| {
        association
            .context
            .map(|reference| {
                runtime
                    .ime_context(reference.handle)
                    .map_err(runtime_state_error)
                    .cloned()
            })
            .transpose()
    };

    match completion.implementation_id.as_str() {
        IMM_GET_DEFAULT_IME_WND_IMPLEMENTATION => {
            let app_window_handle = completion.frame.argument_u32(0).ok_or_else(|| {
                LoaderError::runtime_defect(
                    "runtime-imm32",
                    "ime-default-window-argument-missing-after-dispatch",
                    "ImmGetDefaultIMEWnd completion has no logical app HWND argument",
                )
            })?;
            let ime_window_handle = set_eax_result(
                completion,
                "ime-default-window-return-register-invalid",
                "ImmGetDefaultIMEWnd dispatcher did not set EAX",
            )?;
            if app_window_handle == 0 || ime_window_handle == 0 {
                return Err(LoaderError::runtime_defect(
                    "runtime-imm32",
                    "ime-default-window-primary-result-null",
                    "primary ImmGetDefaultIMEWnd unexpectedly used or returned NULL",
                ));
            }
            let ime_window = runtime
                .ime_window(ime_window_handle)
                .map_err(runtime_state_error)?
                .clone();
            let association = app_association(app_window_handle)?;
            let context = associated_context(&association)?;
            Ok(Some(GenericRuntimeImm32Completion::DefaultWindowResolved {
                policy_id: imm32_policy.id.clone(),
                app_window_handle,
                ime_window,
                association,
                context,
                native_handle_imported: false,
                browser_resource_created: false,
            }))
        }
        SEND_MESSAGE_A_IMPLEMENTATION
            if completion.frame.argument_u32(1) == Some(USER32_WM_IME_CONTROL) =>
        {
            let ime_window_handle = completion.frame.argument_u32(0).ok_or_else(|| {
                LoaderError::runtime_defect(
                    "runtime-imm32",
                    "ime-control-window-argument-missing-after-dispatch",
                    "WM_IME_CONTROL completion has no logical IME HWND argument",
                )
            })?;
            let result = set_eax_result(
                completion,
                "ime-control-return-register-invalid",
                "WM_IME_CONTROL dispatcher did not set EAX",
            )?;
            if result != 0 {
                return Err(LoaderError::runtime_defect(
                    "runtime-imm32",
                    "ime-control-result-invalid",
                    "primary WM_IME_CONTROL did not return zero",
                ));
            }
            let ime_window = runtime
                .ime_window(ime_window_handle)
                .map_err(runtime_state_error)?
                .clone();
            Ok(Some(
                GenericRuntimeImm32Completion::StatusWindowControlRequested {
                    policy_id: user32_policy.id.clone(),
                    ime_window,
                    native_message_forwarded: false,
                    browser_resource_created: false,
                },
            ))
        }
        IMM_ASSOCIATE_CONTEXT_IMPLEMENTATION => {
            let app_window_handle = completion.frame.argument_u32(0).ok_or_else(|| {
                LoaderError::runtime_defect(
                    "runtime-imm32",
                    "ime-association-window-argument-missing-after-dispatch",
                    "ImmAssociateContext completion has no logical app HWND argument",
                )
            })?;
            let requested_context_handle = completion.frame.argument_u32(1).ok_or_else(|| {
                LoaderError::runtime_defect(
                    "runtime-imm32",
                    "ime-association-context-argument-missing-after-dispatch",
                    "ImmAssociateContext completion has no logical HIMC argument",
                )
            })?;
            let previous_context_handle = set_eax_result(
                completion,
                "ime-association-return-register-invalid",
                "ImmAssociateContext dispatcher did not set EAX",
            )?;
            let association = app_association(app_window_handle)?;
            let context = associated_context(&association)?;
            if association.context.map_or(0, |reference| reference.handle)
                != requested_context_handle
            {
                return Err(LoaderError::runtime_defect(
                    "runtime-imm32",
                    "ime-association-authority-mismatch",
                    "ImmAssociateContext argument and logical association authority disagree",
                ));
            }
            Ok(Some(GenericRuntimeImm32Completion::ContextAssociated {
                policy_id: imm32_policy.id.clone(),
                app_window_handle,
                requested_context_handle,
                previous_context_handle,
                association,
                context,
                native_handle_imported: false,
                browser_resource_created: false,
            }))
        }
        _ => Ok(None),
    }
}

fn generic_runtime_virtual_memory_completion(
    completion: &ApiImmediateCompletion,
    runtime: &RuntimeState,
    memory: &GuestMemory,
) -> Result<Option<GenericRuntimeVirtualMemory>, LoaderError> {
    if completion.implementation_id != VIRTUAL_ALLOC_IMPLEMENTATION {
        return Ok(None);
    }
    let address = GuestAddress(set_eax_result(
        completion,
        "virtual-alloc-return-register-invalid",
        "VirtualAlloc dispatcher did not set EAX",
    )?);
    if address.0 == 0 {
        return Err(LoaderError::runtime_defect(
            "runtime-virtual-memory",
            "virtual-alloc-primary-result-null",
            "primary VirtualAlloc reserve unexpectedly returned NULL",
        ));
    }
    let allocation = runtime
        .process_address_space_allocator()
        .allocation_for_purpose(&ProcessAddressAllocationPurpose::VirtualMemory)
        .map_err(runtime_state_error)?
        .clone();
    if allocation.range.base != address {
        return Err(LoaderError::runtime_defect(
            "runtime-virtual-memory",
            "virtual-alloc-address-authority-mismatch",
            "VirtualAlloc return value does not identify its process address allocation",
        ));
    }
    let region = memory
        .region_snapshots()
        .into_iter()
        .find(|region| region.address_allocation_id == Some(allocation.id.0))
        .ok_or_else(|| {
            LoaderError::runtime_defect(
                "runtime-virtual-memory",
                "virtual-alloc-region-missing-after-dispatch",
                "VirtualAlloc process address allocation has no guest memory reservation",
            )
        })?;
    if region.range.start() != allocation.range.base
        || region.range.byte_length() != allocation.range.byte_length
    {
        return Err(LoaderError::runtime_defect(
            "runtime-virtual-memory",
            "virtual-alloc-region-authority-mismatch",
            "VirtualAlloc process address allocation and guest memory reservation disagree",
        ));
    }
    Ok(Some(GenericRuntimeVirtualMemory {
        allocation,
        region,
        preferred_address: completion.frame.argument_u32(0).ok_or_else(|| {
            LoaderError::runtime_defect(
                "runtime-virtual-memory",
                "virtual-alloc-preferred-address-missing-after-dispatch",
                "VirtualAlloc completion has no preferred-address argument",
            )
        })?,
        allocation_type: completion.frame.argument_u32(2).ok_or_else(|| {
            LoaderError::runtime_defect(
                "runtime-virtual-memory",
                "virtual-alloc-allocation-type-missing-after-dispatch",
                "VirtualAlloc completion has no allocation-type argument",
            )
        })?,
        protection: completion.frame.argument_u32(3).ok_or_else(|| {
            LoaderError::runtime_defect(
                "runtime-virtual-memory",
                "virtual-alloc-protection-missing-after-dispatch",
                "VirtualAlloc completion has no protection argument",
            )
        })?,
        reserve_only: true,
        top_down: true,
    }))
}

fn generic_runtime_gdiplus_completion(
    completion: &ApiImmediateCompletion,
    runtime: &RuntimeState,
    policy: &GdiplusCompatibilityPolicy,
) -> Result<Option<GenericRuntimeGdiplusCompletion>, LoaderError> {
    match completion.implementation_id.as_str() {
        vine_core::GDIPLUS_STARTUP_IMPLEMENTATION => {
            let status = set_eax_result(
                completion,
                "gdiplus-startup-return-register-invalid",
                "GdiplusStartup dispatcher did not set EAX",
            )?;
            if status != vine_core::GDIPLUS_STATUS_OK {
                return Err(LoaderError::runtime_defect(
                    "runtime-gdiplus",
                    "gdiplus-startup-primary-status-invalid",
                    format!("primary GdiplusStartup completed with status {status}"),
                ));
            }
            let token_output = completion.frame.argument_u32(0).ok_or_else(|| {
                LoaderError::runtime_defect(
                    "runtime-gdiplus",
                    "gdiplus-startup-token-output-missing-after-dispatch",
                    "completed GdiplusStartup frame has no token output argument",
                )
            })?;
            if completion.guest_writes.len() != 1 {
                return Err(LoaderError::runtime_defect(
                    "runtime-gdiplus",
                    "gdiplus-startup-token-write-count-invalid",
                    "successful GdiplusStartup must commit exactly one token write",
                ));
            }
            let write = &completion.guest_writes[0].write;
            if write.address.0 != token_output || write.bytes.len() != 4 {
                return Err(LoaderError::runtime_defect(
                    "runtime-gdiplus",
                    "gdiplus-startup-token-write-invalid",
                    "GdiplusStartup token write does not match the decoded x86 output",
                ));
            }
            let token = u32::from_le_bytes(write.bytes[..4].try_into().map_err(|_| {
                LoaderError::runtime_defect(
                    "runtime-gdiplus",
                    "gdiplus-startup-token-bytes-invalid",
                    "GdiplusStartup token write is not one x86 ULONG_PTR",
                )
            })?);
            let session = runtime
                .gdiplus_sessions()
                .values()
                .find(|session| session.token == token)
                .cloned()
                .ok_or_else(|| {
                    LoaderError::runtime_defect(
                        "runtime-gdiplus",
                        "gdiplus-startup-session-missing-after-dispatch",
                        "completed GdiplusStartup has no matching logical session",
                    )
                })?;
            let startup = policy.startup.as_ref().ok_or_else(|| {
                LoaderError::runtime_defect(
                    "runtime-gdiplus",
                    "gdiplus-startup-policy-missing-after-dispatch",
                    "GdiplusStartup completed with its compatibility policy disabled",
                )
            })?;
            if session.token_namespace != vine_core::RUNTIME_GDIPLUS_SESSION_TOKEN_NAMESPACE
                || session.startup.profile_id != startup.profile_id
            {
                return Err(LoaderError::runtime_defect(
                    "runtime-gdiplus",
                    "gdiplus-startup-session-policy-mismatch",
                    "logical GDI+ session identity does not match the active startup policy",
                ));
            }
            Ok(Some(GenericRuntimeGdiplusCompletion::SessionStarted {
                policy_id: policy.id.clone(),
                session,
                live_sessions: runtime.gdiplus_sessions().len(),
                browser_resource_created: false,
                native_token_imported: false,
            }))
        }
        vine_core::GDIPLUS_SHUTDOWN_IMPLEMENTATION => {
            if !matches!(completion.eax, RegisterUpdate::Preserve) {
                return Err(LoaderError::runtime_defect(
                    "runtime-gdiplus",
                    "gdiplus-shutdown-return-register-invalid",
                    "void GdiplusShutdown dispatcher unexpectedly changed EAX",
                ));
            }
            let token = completion.frame.argument_u32(0).ok_or_else(|| {
                LoaderError::runtime_defect(
                    "runtime-gdiplus",
                    "gdiplus-shutdown-token-missing-after-dispatch",
                    "completed GdiplusShutdown frame has no logical token",
                )
            })?;
            if runtime
                .gdiplus_sessions()
                .values()
                .any(|session| session.token == token)
            {
                return Err(LoaderError::runtime_defect(
                    "runtime-gdiplus",
                    "gdiplus-shutdown-session-still-live",
                    "completed GdiplusShutdown did not retire its logical session",
                ));
            }
            Ok(Some(GenericRuntimeGdiplusCompletion::SessionShutdown {
                token,
                live_sessions: runtime.gdiplus_sessions().len(),
            }))
        }
        _ => Ok(None),
    }
}

fn generic_runtime_gdi_completion(
    completion: &ApiImmediateCompletion,
    runtime: &RuntimeState,
    policy: &Gdi32CompatibilityPolicy,
) -> Result<Option<GenericRuntimeGdiCompletion>, LoaderError> {
    match completion.implementation_id.as_str() {
        vine_core::GET_DC_IMPLEMENTATION => {
            let handle = set_eax_result(
                completion,
                "get-dc-return-register-invalid",
                "GetDC dispatcher did not set EAX",
            )?;
            let device_context = runtime
                .gdi_device_context(handle)
                .map_err(runtime_state_error)?
                .clone();
            match device_context.target {
                vine_core::RuntimeDeviceContextTarget::Window { surface_id, .. } => {
                    let surface = runtime
                        .gdi_surfaces()
                        .get(&surface_id)
                        .cloned()
                        .ok_or_else(|| {
                            LoaderError::runtime_defect(
                                "runtime-gdi",
                                "get-dc-surface-missing-after-dispatch",
                                "completed window GetDC has no matching logical surface",
                            )
                        })?;
                    Ok(Some(GenericRuntimeGdiCompletion::DeviceContextAcquired {
                        device_context,
                        surface,
                        live_device_contexts: runtime.gdi_device_contexts().len(),
                    }))
                }
                vine_core::RuntimeDeviceContextTarget::Screen { .. } => Ok(Some(
                    GenericRuntimeGdiCompletion::ScreenDeviceContextAcquired {
                        device_context,
                        live_device_contexts: runtime.gdi_device_contexts().len(),
                    },
                )),
                vine_core::RuntimeDeviceContextTarget::Memory { .. } => {
                    Err(LoaderError::runtime_defect(
                        "runtime-gdi",
                        "get-dc-memory-target-invalid-after-dispatch",
                        "USER32 GetDC resolved to a memory logical HDC",
                    ))
                }
                vine_core::RuntimeDeviceContextTarget::GraphicsSurface { .. } => {
                    Err(LoaderError::runtime_defect(
                        "runtime-gdi",
                        "get-dc-target-invalid-after-dispatch",
                        "USER32 GetDC resolved to a graphics-surface logical HDC",
                    ))
                }
            }
        }
        vine_core::GET_STOCK_OBJECT_IMPLEMENTATION => {
            let handle = set_eax_result(
                completion,
                "get-stock-object-return-register-invalid",
                "GetStockObject dispatcher did not set EAX",
            )?;
            let object = runtime
                .gdi_objects()
                .values()
                .find(|object| object.handle == handle)
                .cloned()
                .ok_or_else(|| {
                    LoaderError::runtime_defect(
                        "runtime-gdi",
                        "stock-object-missing-after-dispatch",
                        "completed GetStockObject has no matching logical object",
                    )
                })?;
            Ok(Some(GenericRuntimeGdiCompletion::StockObjectResolved {
                object,
                live_objects: runtime.gdi_objects().len(),
            }))
        }
        vine_core::FILL_RECT_IMPLEMENTATION => {
            let handle = completion.frame.argument_u32(0).ok_or_else(|| {
                LoaderError::runtime_defect(
                    "runtime-gdi",
                    "fill-rect-hdc-missing-after-dispatch",
                    "completed FillRect frame has no logical HDC",
                )
            })?;
            let device_context = runtime
                .gdi_device_context(handle)
                .map_err(runtime_state_error)?
                .clone();
            match device_context.target {
                vine_core::RuntimeDeviceContextTarget::Window { surface_id, .. } => {
                    let surface = runtime
                        .gdi_surfaces()
                        .get(&surface_id)
                        .cloned()
                        .ok_or_else(|| {
                            LoaderError::runtime_defect(
                                "runtime-gdi",
                                "fill-rect-surface-missing-after-dispatch",
                                "completed FillRect has no matching logical window surface",
                            )
                        })?;
                    Ok(Some(GenericRuntimeGdiCompletion::SurfaceFilled {
                        device_context,
                        surface,
                    }))
                }
                vine_core::RuntimeDeviceContextTarget::GraphicsSurface { surface_id, .. } => {
                    let surface = runtime
                        .graphics_surfaces()
                        .get(&surface_id)
                        .cloned()
                        .ok_or_else(|| {
                            LoaderError::runtime_defect(
                                "runtime-gdi",
                                "fill-rect-graphics-surface-missing-after-dispatch",
                                "completed FillRect has no matching logical graphics surface",
                            )
                        })?;
                    Ok(Some(GenericRuntimeGdiCompletion::GraphicsSurfaceFilled {
                        device_context,
                        surface,
                    }))
                }
                vine_core::RuntimeDeviceContextTarget::Screen { .. } => {
                    Err(LoaderError::runtime_defect(
                        "runtime-gdi",
                        "fill-rect-screen-target-unexpected",
                        "screen FillRect completed outside the reached GDI support band",
                    ))
                }
                vine_core::RuntimeDeviceContextTarget::Memory { .. } => {
                    Err(LoaderError::runtime_defect(
                        "runtime-gdi",
                        "fill-rect-memory-target-unexpected",
                        "memory FillRect completed before a logical DIB was selected",
                    ))
                }
            }
        }
        vine_core::ENUM_FONT_FAMILIES_EX_A_IMPLEMENTATION => {
            let handle = completion.frame.argument_u32(0).ok_or_else(|| {
                LoaderError::runtime_defect(
                    "runtime-gdi",
                    "enum-font-families-ex-a-hdc-missing-after-dispatch",
                    "completed EnumFontFamiliesExA frame has no logical HDC",
                )
            })?;
            let callback_result = set_eax_result(
                completion,
                "enum-font-families-ex-a-return-register-invalid",
                "EnumFontFamiliesExA dispatcher did not set EAX",
            )?;
            let device_context = runtime
                .gdi_device_context(handle)
                .map_err(runtime_state_error)?
                .clone();
            let enumeration = policy
                .enum_font_families_ex_a
                .as_ref()
                .cloned()
                .ok_or_else(|| {
                    LoaderError::runtime_defect(
                        "runtime-gdi",
                        "font-enumeration-policy-missing-after-dispatch",
                        "completed EnumFontFamiliesExA has no active GDI32 compatibility policy",
                    )
                })?;
            Ok(Some(
                GenericRuntimeGdiCompletion::FontEnumerationCompleted {
                    device_context,
                    enumeration,
                    callback_result,
                    live_device_contexts: runtime.gdi_device_contexts().len(),
                },
            ))
        }
        vine_core::CREATE_FONT_A_IMPLEMENTATION => {
            let handle = set_eax_result(
                completion,
                "create-font-a-return-register-invalid",
                "CreateFontA dispatcher did not set EAX",
            )?;
            let object = runtime
                .gdi_objects()
                .values()
                .find(|object| object.handle == handle)
                .cloned()
                .ok_or_else(|| {
                    LoaderError::runtime_defect(
                        "runtime-gdi",
                        "font-object-missing-after-dispatch",
                        "completed CreateFontA has no matching logical font object",
                    )
                })?;
            if !matches!(
                &object.object,
                vine_core::RuntimeGdiObjectKind::FontA { .. }
            ) {
                return Err(LoaderError::runtime_defect(
                    "runtime-gdi",
                    "font-object-kind-invalid-after-dispatch",
                    "completed CreateFontA resolved to a non-font logical object",
                ));
            }
            Ok(Some(GenericRuntimeGdiCompletion::FontCreated {
                object,
                live_objects: runtime.gdi_objects().len(),
            }))
        }
        vine_core::CREATE_COMPATIBLE_DC_IMPLEMENTATION => {
            let handle = set_eax_result(
                completion,
                "create-compatible-dc-return-register-invalid",
                "CreateCompatibleDC dispatcher did not set EAX",
            )?;
            let device_context = runtime
                .gdi_device_context(handle)
                .map_err(runtime_state_error)?
                .clone();
            if !matches!(
                device_context.target,
                vine_core::RuntimeDeviceContextTarget::Memory { .. }
            ) {
                return Err(LoaderError::runtime_defect(
                    "runtime-gdi",
                    "compatible-dc-target-invalid-after-dispatch",
                    "completed CreateCompatibleDC resolved to a non-memory logical HDC",
                ));
            }
            let font_id = device_context.selected_font_id.ok_or_else(|| {
                LoaderError::runtime_defect(
                    "runtime-gdi",
                    "compatible-dc-default-font-missing-after-dispatch",
                    "completed CreateCompatibleDC has no selected default font",
                )
            })?;
            let bitmap_id = device_context.selected_bitmap_id.ok_or_else(|| {
                LoaderError::runtime_defect(
                    "runtime-gdi",
                    "compatible-dc-default-bitmap-missing-after-dispatch",
                    "completed CreateCompatibleDC has no selected default bitmap",
                )
            })?;
            let initial_font = runtime
                .gdi_objects()
                .get(&font_id)
                .cloned()
                .ok_or_else(|| {
                    LoaderError::runtime_defect(
                        "runtime-gdi",
                        "compatible-dc-default-font-dangling-after-dispatch",
                        "completed CreateCompatibleDC default font is not live",
                    )
                })?;
            let initial_bitmap =
                runtime
                    .gdi_objects()
                    .get(&bitmap_id)
                    .cloned()
                    .ok_or_else(|| {
                        LoaderError::runtime_defect(
                            "runtime-gdi",
                            "compatible-dc-default-bitmap-dangling-after-dispatch",
                            "completed CreateCompatibleDC default bitmap is not live",
                        )
                    })?;
            Ok(Some(
                GenericRuntimeGdiCompletion::CompatibleDeviceContextCreated {
                    device_context,
                    initial_font,
                    initial_bitmap,
                    live_device_contexts: runtime.gdi_device_contexts().len(),
                    live_objects: runtime.gdi_objects().len(),
                },
            ))
        }
        vine_core::CREATE_DIB_SECTION_IMPLEMENTATION => {
            let handle = set_eax_result(
                completion,
                "create-dib-section-return-register-invalid",
                "CreateDIBSection dispatcher did not set EAX",
            )?;
            if handle == 0 {
                return Ok(Some(
                    GenericRuntimeGdiCompletion::DibSectionCreateRejected {
                        live_objects: runtime.gdi_objects().len(),
                    },
                ));
            }
            let object = runtime
                .gdi_objects()
                .values()
                .find(|object| object.handle == handle)
                .cloned()
                .ok_or_else(|| {
                    LoaderError::runtime_defect(
                        "runtime-gdi",
                        "dib-section-object-missing-after-dispatch",
                        "completed CreateDIBSection has no matching logical bitmap object",
                    )
                })?;
            let vine_core::RuntimeGdiObjectKind::DibSection { backing, .. } = &object.object else {
                return Err(LoaderError::runtime_defect(
                    "runtime-gdi",
                    "dib-section-object-kind-invalid-after-dispatch",
                    "completed CreateDIBSection resolved to a non-DIB logical object",
                ));
            };
            let address_allocation = runtime
                .process_address_space_allocator()
                .allocation(backing.address_allocation_id)
                .map_err(runtime_state_error)?
                .clone();
            Ok(Some(GenericRuntimeGdiCompletion::DibSectionCreated {
                object,
                address_allocation,
                live_objects: runtime.gdi_objects().len(),
            }))
        }
        vine_core::GET_OBJECT_A_IMPLEMENTATION => {
            let byte_length = set_eax_result(
                completion,
                "get-object-a-return-register-invalid",
                "GetObjectA dispatcher did not set EAX",
            )?;
            let handle = completion.frame.argument_u32(0).ok_or_else(|| {
                LoaderError::runtime_defect(
                    "runtime-gdi",
                    "get-object-a-handle-missing-after-dispatch",
                    "completed GetObjectA frame has no logical object",
                )
            })?;
            let object = runtime
                .gdi_objects()
                .values()
                .find(|object| object.handle == handle)
                .cloned();
            Ok(Some(GenericRuntimeGdiCompletion::ObjectBytesRead {
                handle,
                byte_length,
                object,
                live_objects: runtime.gdi_objects().len(),
            }))
        }
        vine_core::GET_OUTLINE_TEXT_METRICS_A_IMPLEMENTATION => {
            let returned_byte_length = set_eax_result(
                completion,
                "get-outline-text-metrics-a-return-register-invalid",
                "GetOutlineTextMetricsA dispatcher did not set EAX",
            )?;
            let device_context_handle = completion.frame.argument_u32(0).ok_or_else(|| {
                LoaderError::runtime_defect(
                    "runtime-gdi",
                    "outline-text-metrics-hdc-missing-after-dispatch",
                    "completed GetOutlineTextMetricsA frame has no logical HDC",
                )
            })?;
            let requested_byte_length = completion.frame.argument_u32(1).ok_or_else(|| {
                LoaderError::runtime_defect(
                    "runtime-gdi",
                    "outline-text-metrics-size-missing-after-dispatch",
                    "completed GetOutlineTextMetricsA frame has no byte length",
                )
            })?;
            let output_address = completion.frame.argument_u32(2).ok_or_else(|| {
                LoaderError::runtime_defect(
                    "runtime-gdi",
                    "outline-text-metrics-output-missing-after-dispatch",
                    "completed GetOutlineTextMetricsA frame has no output address",
                )
            })?;
            let device_context = runtime
                .gdi_device_context(device_context_handle)
                .map_err(runtime_state_error)?
                .clone();
            let font_id = device_context.selected_font_id.ok_or_else(|| {
                LoaderError::runtime_defect(
                    "runtime-gdi",
                    "outline-text-metrics-font-missing-after-dispatch",
                    "completed GetOutlineTextMetricsA has no selected logical font",
                )
            })?;
            let selected_font = runtime
                .gdi_objects()
                .get(&font_id)
                .cloned()
                .ok_or_else(|| {
                    LoaderError::runtime_defect(
                        "runtime-gdi",
                        "outline-text-metrics-font-dangling-after-dispatch",
                        "completed GetOutlineTextMetricsA selected font is not live",
                    )
                })?;
            if !matches!(
                &selected_font.object,
                vine_core::RuntimeGdiObjectKind::FontA { .. }
            ) {
                return Err(LoaderError::runtime_defect(
                    "runtime-gdi",
                    "outline-text-metrics-font-kind-invalid-after-dispatch",
                    "completed GetOutlineTextMetricsA selected object is not a FontA",
                ));
            }
            let profile_id = policy
                .get_outline_text_metrics_a
                .as_ref()
                .ok_or_else(|| {
                    LoaderError::runtime_defect(
                        "runtime-gdi",
                        "outline-text-metrics-policy-missing-after-dispatch",
                        "completed GetOutlineTextMetricsA has no active compatibility policy",
                    )
                })?
                .profile_id
                .clone();
            let written_byte_length = completion
                .guest_writes
                .first()
                .map(|receipt| receipt.write.bytes.len())
                .unwrap_or(0);
            Ok(Some(GenericRuntimeGdiCompletion::OutlineTextMetricsRead {
                device_context,
                selected_font,
                profile_id,
                requested_byte_length,
                returned_byte_length,
                output_address,
                written_byte_length,
            }))
        }
        vine_core::TEXT_OUT_A_IMPLEMENTATION => {
            let result = set_eax_result(
                completion,
                "text-out-a-return-register-invalid",
                "TextOutA dispatcher did not set EAX",
            )?;
            if result != 1 {
                return Err(LoaderError::runtime_defect(
                    "runtime-gdi",
                    "text-out-a-primary-result-invalid",
                    "completed primary TextOutA raster call did not succeed",
                ));
            }
            let device_context_handle = completion.frame.argument_u32(0).ok_or_else(|| {
                LoaderError::runtime_defect(
                    "runtime-gdi",
                    "text-out-a-hdc-missing-after-dispatch",
                    "completed TextOutA frame has no logical HDC",
                )
            })?;
            let x = completion.frame.argument_u32(1).ok_or_else(|| {
                LoaderError::runtime_defect(
                    "runtime-gdi",
                    "text-out-a-x-missing-after-dispatch",
                    "completed TextOutA frame has no x coordinate",
                )
            })? as i32;
            let y = completion.frame.argument_u32(2).ok_or_else(|| {
                LoaderError::runtime_defect(
                    "runtime-gdi",
                    "text-out-a-y-missing-after-dispatch",
                    "completed TextOutA frame has no y coordinate",
                )
            })? as i32;
            let text_address = completion.frame.argument_u32(3).ok_or_else(|| {
                LoaderError::runtime_defect(
                    "runtime-gdi",
                    "text-out-a-text-missing-after-dispatch",
                    "completed TextOutA frame has no text address",
                )
            })?;
            let character_count = completion.frame.argument_u32(4).ok_or_else(|| {
                LoaderError::runtime_defect(
                    "runtime-gdi",
                    "text-out-a-count-missing-after-dispatch",
                    "completed TextOutA frame has no character count",
                )
            })?;
            let device_context = runtime
                .gdi_device_context(device_context_handle)
                .map_err(runtime_state_error)?
                .clone();
            let font_id = device_context.selected_font_id.ok_or_else(|| {
                LoaderError::runtime_defect(
                    "runtime-gdi",
                    "text-out-a-font-missing-after-dispatch",
                    "completed TextOutA has no selected logical font",
                )
            })?;
            let bitmap_id = device_context.selected_bitmap_id.ok_or_else(|| {
                LoaderError::runtime_defect(
                    "runtime-gdi",
                    "text-out-a-bitmap-missing-after-dispatch",
                    "completed TextOutA has no selected logical bitmap",
                )
            })?;
            let selected_font = runtime
                .gdi_objects()
                .get(&font_id)
                .cloned()
                .ok_or_else(|| {
                    LoaderError::runtime_defect(
                        "runtime-gdi",
                        "text-out-a-font-dangling-after-dispatch",
                        "completed TextOutA selected font is not live",
                    )
                })?;
            if !matches!(
                &selected_font.object,
                vine_core::RuntimeGdiObjectKind::FontA { .. }
            ) {
                return Err(LoaderError::runtime_defect(
                    "runtime-gdi",
                    "text-out-a-font-kind-invalid-after-dispatch",
                    "completed TextOutA selected object is not a FontA",
                ));
            }
            let selected_dib_section =
                runtime
                    .gdi_objects()
                    .get(&bitmap_id)
                    .cloned()
                    .ok_or_else(|| {
                        LoaderError::runtime_defect(
                            "runtime-gdi",
                            "text-out-a-bitmap-dangling-after-dispatch",
                            "completed TextOutA selected bitmap is not live",
                        )
                    })?;
            let vine_core::RuntimeGdiObjectKind::DibSection { backing, .. } =
                &selected_dib_section.object
            else {
                return Err(LoaderError::runtime_defect(
                    "runtime-gdi",
                    "text-out-a-bitmap-kind-invalid-after-dispatch",
                    "completed TextOutA selected object is not a DIB section",
                ));
            };
            let content_generation = backing.content_generation;
            let text_policy = policy.text_out_a.as_ref().ok_or_else(|| {
                LoaderError::runtime_defect(
                    "runtime-gdi",
                    "text-out-a-policy-missing-after-dispatch",
                    "completed TextOutA has no active compatibility policy",
                )
            })?;
            if completion.guest_writes.len() > 1 {
                return Err(LoaderError::runtime_defect(
                    "runtime-gdi",
                    "text-out-a-write-count-invalid-after-dispatch",
                    "completed TextOutA emitted more than one raster write",
                ));
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
            Ok(Some(GenericRuntimeGdiCompletion::TextRasterWritten {
                device_context,
                selected_font,
                selected_dib_section,
                profile_id: text_policy.profile_id.clone(),
                x,
                y,
                text_address,
                character_count,
                write_address: write.map(|write| write.address.0),
                written_byte_length: write.map(|write| write.bytes.len()).unwrap_or(0),
                changed_byte_count,
                content_generation,
                expected_output_fnv1a32: text_policy.output_pixel_plane_fnv1a32,
            }))
        }
        vine_core::SELECT_OBJECT_IMPLEMENTATION => {
            let previous_handle = set_eax_result(
                completion,
                "select-object-return-register-invalid",
                "SelectObject dispatcher did not set EAX",
            )?;
            let device_context_handle = completion.frame.argument_u32(0).ok_or_else(|| {
                LoaderError::runtime_defect(
                    "runtime-gdi",
                    "select-object-hdc-missing-after-dispatch",
                    "completed SelectObject frame has no logical HDC",
                )
            })?;
            let selected_handle = completion.frame.argument_u32(1).ok_or_else(|| {
                LoaderError::runtime_defect(
                    "runtime-gdi",
                    "select-object-handle-missing-after-dispatch",
                    "completed SelectObject frame has no logical object",
                )
            })?;
            let device_context = runtime
                .gdi_device_context(device_context_handle)
                .map_err(runtime_state_error)?
                .clone();
            let selected_object = runtime
                .gdi_objects()
                .values()
                .find(|object| object.handle == selected_handle)
                .cloned();
            let previous_object = runtime
                .gdi_objects()
                .values()
                .find(|object| object.handle == previous_handle)
                .cloned();
            Ok(Some(GenericRuntimeGdiCompletion::ObjectSelected {
                device_context,
                selected_object,
                previous_object,
                succeeded: previous_handle != 0,
                live_objects: runtime.gdi_objects().len(),
            }))
        }
        vine_core::DELETE_OBJECT_IMPLEMENTATION => {
            let deleted = set_eax_result(
                completion,
                "delete-object-return-register-invalid",
                "DeleteObject dispatcher did not set EAX",
            )? != 0;
            let handle = completion.frame.argument_u32(0).ok_or_else(|| {
                LoaderError::runtime_defect(
                    "runtime-gdi",
                    "delete-object-handle-missing-after-dispatch",
                    "completed DeleteObject frame has no logical object",
                )
            })?;
            Ok(Some(GenericRuntimeGdiCompletion::ObjectDeleted {
                handle,
                deleted,
                live_objects: runtime.gdi_objects().len(),
            }))
        }
        vine_core::DELETE_DC_IMPLEMENTATION => {
            let deleted = set_eax_result(
                completion,
                "delete-dc-return-register-invalid",
                "DeleteDC dispatcher did not set EAX",
            )? != 0;
            let handle = completion.frame.argument_u32(0).ok_or_else(|| {
                LoaderError::runtime_defect(
                    "runtime-gdi",
                    "delete-dc-hdc-missing-after-dispatch",
                    "completed DeleteDC frame has no logical HDC",
                )
            })?;
            Ok(Some(
                GenericRuntimeGdiCompletion::CompatibleDeviceContextDeleted {
                    handle,
                    deleted,
                    live_device_contexts: runtime.gdi_device_contexts().len(),
                    live_objects: runtime.gdi_objects().len(),
                },
            ))
        }
        vine_core::RELEASE_DC_IMPLEMENTATION => {
            let handle = completion.frame.argument_u32(1).ok_or_else(|| {
                LoaderError::runtime_defect(
                    "runtime-gdi",
                    "release-dc-hdc-missing-after-dispatch",
                    "completed ReleaseDC frame has no logical HDC",
                )
            })?;
            Ok(Some(GenericRuntimeGdiCompletion::DeviceContextReleased {
                handle,
                live_device_contexts: runtime.gdi_device_contexts().len(),
                live_surfaces: runtime.gdi_surfaces().len(),
            }))
        }
        _ => Ok(None),
    }
}

fn generic_runtime_paint_completion(
    completion: &ApiImmediateCompletion,
    runtime: &RuntimeState,
) -> Result<Option<GenericRuntimePaintCompletion>, LoaderError> {
    let window_handle = || {
        completion.frame.argument_u32(0).ok_or_else(|| {
            LoaderError::runtime_defect(
                "runtime-user32",
                "paint-window-argument-missing-after-dispatch",
                "completed paint frame has no logical HWND argument",
            )
        })
    };
    match completion.implementation_id.as_str() {
        vine_core::BEGIN_PAINT_IMPLEMENTATION => {
            let returned_handle = set_eax_result(
                completion,
                "begin-paint-return-register-invalid",
                "BeginPaint dispatcher did not set EAX",
            )?;
            let window = runtime
                .window(window_handle()?)
                .map_err(runtime_state_error)?
                .clone();
            let session = runtime
                .paint_sessions()
                .get(&window.id)
                .cloned()
                .ok_or_else(|| {
                    LoaderError::runtime_defect(
                        "runtime-user32",
                        "begin-paint-session-missing-after-dispatch",
                        "completed BeginPaint has no matching logical paint session",
                    )
                })?;
            if returned_handle != session.device_context_handle {
                return Err(LoaderError::runtime_defect(
                    "runtime-user32",
                    "begin-paint-device-context-mismatch-after-dispatch",
                    "completed BeginPaint returned a different logical HDC than its paint session",
                ));
            }
            let device_context = runtime
                .gdi_device_context(returned_handle)
                .map_err(runtime_state_error)?
                .clone();
            Ok(Some(GenericRuntimePaintCompletion::Begun {
                window,
                session,
                device_context,
                live_paint_sessions: runtime.paint_sessions().len(),
                live_device_contexts: runtime.gdi_device_contexts().len(),
            }))
        }
        vine_core::END_PAINT_IMPLEMENTATION => {
            let result = set_eax_result(
                completion,
                "end-paint-return-register-invalid",
                "EndPaint dispatcher did not set EAX",
            )?;
            if result != 1 {
                return Err(LoaderError::runtime_defect(
                    "runtime-user32",
                    "end-paint-primary-result-invalid",
                    "completed primary EndPaint call did not succeed",
                ));
            }
            let window = runtime
                .window(window_handle()?)
                .map_err(runtime_state_error)?
                .clone();
            if runtime.paint_sessions().contains_key(&window.id) {
                return Err(LoaderError::runtime_defect(
                    "runtime-user32",
                    "end-paint-session-still-live-after-dispatch",
                    "completed EndPaint did not retire its logical paint session",
                ));
            }
            let paint_struct_address = completion.frame.argument_u32(1).ok_or_else(|| {
                LoaderError::runtime_defect(
                    "runtime-user32",
                    "end-paint-struct-argument-missing-after-dispatch",
                    "completed EndPaint frame has no PAINTSTRUCT address",
                )
            })?;
            Ok(Some(GenericRuntimePaintCompletion::Ended {
                window,
                paint_struct_address,
                live_paint_sessions: runtime.paint_sessions().len(),
                live_device_contexts: runtime.gdi_device_contexts().len(),
            }))
        }
        _ => Ok(None),
    }
}

fn api_fault(fault: ApiFault) -> LoaderError {
    match fault.class {
        ApiFaultClass::InvalidCall => {
            LoaderError::invalid_input(fault.stage, fault.code, fault.message)
        }
        ApiFaultClass::Unsupported => {
            LoaderError::unsupported(fault.stage, fault.code, fault.message)
        }
        ApiFaultClass::ResourceLimit => {
            LoaderError::resource_limit(fault.stage, fault.code, fault.message)
        }
        ApiFaultClass::RuntimeDefect => {
            LoaderError::runtime_defect(fault.stage, fault.code, fault.message)
        }
    }
}

fn fixed_mapping_allocation(
    allocator: &ProcessAddressSpaceAllocator,
    purpose: ProcessAddressAllocationPurpose,
) -> Result<ProcessAddressAllocationRecord, LoaderError> {
    allocator
        .allocation_for_purpose(&purpose)
        .cloned()
        .map_err(runtime_state_error)
}

fn runtime_state_error(error: RuntimeStateError) -> LoaderError {
    match error.class {
        RuntimeStateErrorClass::InvalidInput
        | RuntimeStateErrorClass::NotFound
        | RuntimeStateErrorClass::Conflict => {
            LoaderError::invalid_input(error.stage, error.code, error.message)
        }
        RuntimeStateErrorClass::ResourceLimit => {
            LoaderError::resource_limit(error.stage, error.code, error.message)
        }
        RuntimeStateErrorClass::RuntimeDefect => {
            LoaderError::runtime_defect(error.stage, error.code, error.message)
        }
    }
}

fn runtime_module_image_error(error: RuntimeModuleImageError) -> LoaderError {
    match error.class {
        RuntimeModuleImageErrorClass::InvalidInput
        | RuntimeModuleImageErrorClass::NotFound
        | RuntimeModuleImageErrorClass::Conflict => {
            LoaderError::invalid_input("runtime-module-image", error.code, error.message)
        }
        RuntimeModuleImageErrorClass::ResourceLimit => {
            LoaderError::resource_limit("runtime-module-image", error.code, error.message)
        }
        RuntimeModuleImageErrorClass::RuntimeDefect => {
            LoaderError::runtime_defect("runtime-module-image", error.code, error.message)
        }
    }
}

fn validate_direct_iat_callsite(
    memory: &GuestMemory,
    thunk: &ImportThunk,
    return_address: u32,
) -> Result<(), LoaderError> {
    let call_site = return_address.checked_sub(6).ok_or_else(|| {
        LoaderError::invalid_input(
            "abi-gateway",
            "stdcall-return-address-underflow",
            "stdcall return address cannot identify a six-byte indirect call",
        )
    })?;
    let call = memory
        .fetch(GuestAddress(call_site), 6)
        .map_err(memory_error)?;
    let iat_operand = u32::from_le_bytes(call[2..6].try_into().map_err(|_| {
        LoaderError::runtime_defect(
            "abi-gateway",
            "stdcall-call-width-invalid",
            "checked import call did not contain a four-byte IAT operand",
        )
    })?);
    if call[0..2] != [0xff, 0x15] || iat_operand != thunk.iat_address.0 {
        return Err(LoaderError::invalid_input(
            "abi-gateway",
            "stdcall-callsite-mismatch",
            "stdcall return address does not follow its expected indirect IAT call",
        ));
    }
    Ok(())
}

fn validate_esi_iat_callsite(
    memory: &GuestMemory,
    thunk: &ImportThunk,
    return_address: u32,
    indirect_call_register: &str,
    indirect_call_target: u32,
) -> Result<(), LoaderError> {
    if indirect_call_register != "esi" || indirect_call_target != thunk.address.0 {
        return Err(LoaderError::invalid_input(
            "abi-gateway",
            "stdcall-esi-call-target-mismatch",
            "stdcall register-indirect call requires live ESI to match its owned import thunk",
        ));
    }
    let call_site = return_address.checked_sub(2).ok_or_else(|| {
        LoaderError::invalid_input(
            "abi-gateway",
            "stdcall-esi-return-address-underflow",
            "stdcall return address cannot identify a two-byte CALL ESI",
        )
    })?;
    let call = memory
        .fetch(GuestAddress(call_site), 2)
        .map_err(memory_error)?;
    if call != [0xff, 0xd6] {
        return Err(LoaderError::invalid_input(
            "abi-gateway",
            "stdcall-esi-callsite-mismatch",
            "stdcall return address does not follow the expected CALL ESI",
        ));
    }
    Ok(())
}

fn validate_stdcall_register_iat_frame(
    memory: &GuestMemory,
    thunk: &ImportThunk,
    stack_pointer: u32,
    return_address: u32,
    expected_arguments: &[u32],
    procedure_name_address: u32,
) -> Result<(), LoaderError> {
    validate_stdcall_frame(memory, stack_pointer, return_address, expected_arguments)?;
    let sequence_address = return_address.checked_sub(14).ok_or_else(|| {
        LoaderError::invalid_input(
            "abi-gateway",
            "register-iat-call-address-underflow",
            "GetProcAddress return cannot identify its register-indirect IAT sequence",
        )
    })?;
    let sequence = memory
        .fetch(GuestAddress(sequence_address), 14)
        .map_err(memory_error)?;
    let iat_operand = u32::from_le_bytes(sequence[2..6].try_into().map_err(|_| {
        LoaderError::runtime_defect(
            "abi-gateway",
            "register-iat-operand-width-invalid",
            "register-indirect import sequence has a partial IAT operand",
        )
    })?);
    let name_operand = u32::from_le_bytes(sequence[7..11].try_into().map_err(|_| {
        LoaderError::runtime_defect(
            "abi-gateway",
            "register-iat-name-width-invalid",
            "register-indirect import sequence has a partial procedure-name operand",
        )
    })?);
    if sequence[0..2] != [0x8b, 0x35]
        || iat_operand != thunk.iat_address.0
        || sequence[6] != 0x68
        || name_operand != procedure_name_address
        || sequence[11] != 0x57
        || sequence[12..14] != [0xff, 0xd6]
    {
        return Err(LoaderError::invalid_input(
            "abi-gateway",
            "register-iat-callsite-mismatch",
            "GetProcAddress return does not follow the controlled MOV ESI,[IAT] / PUSH name / PUSH EDI / CALL ESI sequence",
        ));
    }
    validate_bound_iat(memory, thunk)
}

fn validate_stdcall_persistent_register_iat_frame(
    memory: &GuestMemory,
    thunk: &ImportThunk,
    request: &V86GetProcAddressSecondReturnRequest,
    expected_arguments: &[u32],
) -> Result<(), LoaderError> {
    validate_stdcall_frame(
        memory,
        request.stack_pointer,
        request.return_address,
        expected_arguments,
    )?;
    if request.indirect_call_register != "esi" || request.indirect_call_target != thunk.address.0 {
        return Err(LoaderError::invalid_input(
            "abi-gateway",
            "persistent-register-call-target-mismatch",
            "second GetProcAddress call requires live ESI to retain the owned import thunk",
        ));
    }
    let sequence_address = request.return_address.checked_sub(13).ok_or_else(|| {
        LoaderError::invalid_input(
            "abi-gateway",
            "persistent-register-call-address-underflow",
            "second GetProcAddress return cannot identify its register-indirect sequence",
        )
    })?;
    let sequence = memory
        .fetch(GuestAddress(sequence_address), 13)
        .map_err(memory_error)?;
    let name_operand = u32::from_le_bytes(sequence[1..5].try_into().map_err(|_| {
        LoaderError::runtime_defect(
            "abi-gateway",
            "persistent-register-name-width-invalid",
            "register-indirect sequence has a partial procedure-name operand",
        )
    })?);
    let store_operand = u32::from_le_bytes(sequence[7..11].try_into().map_err(|_| {
        LoaderError::runtime_defect(
            "abi-gateway",
            "persistent-register-store-width-invalid",
            "register-indirect sequence has a partial prior-result store operand",
        )
    })?);
    if sequence[0] != 0x68
        || name_operand != request.procedure_name_address
        || sequence[5] != 0x57
        || sequence[6] != 0xa3
        || store_operand != request.first_endpoint_store_address
        || sequence[11..13] != [0xff, 0xd6]
    {
        return Err(LoaderError::invalid_input(
            "abi-gateway",
            "persistent-register-callsite-mismatch",
            "second GetProcAddress return does not follow the controlled PUSH name / PUSH EDI / STORE prior result / CALL ESI sequence",
        ));
    }
    validate_bound_iat(memory, thunk)
}

fn validate_stdcall_third_persistent_register_iat_frame(
    memory: &GuestMemory,
    thunk: &ImportThunk,
    request: &V86GetProcAddressThirdReturnRequest,
    expected_arguments: &[u32],
) -> Result<(), LoaderError> {
    validate_stdcall_frame(
        memory,
        request.stack_pointer,
        request.return_address,
        expected_arguments,
    )?;
    if request.indirect_call_register != "esi" || request.indirect_call_target != thunk.address.0 {
        return Err(LoaderError::invalid_input(
            "abi-gateway",
            "third-persistent-register-call-target-mismatch",
            "third GetProcAddress call requires live ESI to retain the owned import thunk",
        ));
    }
    let sequence_address = request.return_address.checked_sub(13).ok_or_else(|| {
        LoaderError::invalid_input(
            "abi-gateway",
            "third-persistent-register-call-address-underflow",
            "third GetProcAddress return cannot identify its register-indirect sequence",
        )
    })?;
    let sequence = memory
        .fetch(GuestAddress(sequence_address), 13)
        .map_err(memory_error)?;
    let name_operand = u32::from_le_bytes(sequence[1..5].try_into().map_err(|_| {
        LoaderError::runtime_defect(
            "abi-gateway",
            "third-persistent-register-name-width-invalid",
            "register-indirect sequence has a partial procedure-name operand",
        )
    })?);
    let store_operand = u32::from_le_bytes(sequence[7..11].try_into().map_err(|_| {
        LoaderError::runtime_defect(
            "abi-gateway",
            "third-persistent-register-store-width-invalid",
            "register-indirect sequence has a partial prior-result store operand",
        )
    })?);
    if sequence[0] != 0x68
        || name_operand != request.procedure_name_address
        || sequence[5] != 0x57
        || sequence[6] != 0xa3
        || store_operand != request.second_endpoint_store_address
        || sequence[11..13] != [0xff, 0xd6]
    {
        return Err(LoaderError::invalid_input(
            "abi-gateway",
            "third-persistent-register-callsite-mismatch",
            "third GetProcAddress return does not follow the controlled PUSH name / PUSH EDI / STORE prior result / CALL ESI sequence",
        ));
    }
    validate_bound_iat(memory, thunk)
}

fn validate_stdcall_fourth_persistent_register_iat_frame(
    memory: &GuestMemory,
    thunk: &ImportThunk,
    request: &V86GetProcAddressFourthReturnRequest,
    expected_arguments: &[u32],
) -> Result<(), LoaderError> {
    validate_stdcall_frame(
        memory,
        request.stack_pointer,
        request.return_address,
        expected_arguments,
    )?;
    if request.indirect_call_register != "esi" || request.indirect_call_target != thunk.address.0 {
        return Err(LoaderError::invalid_input(
            "abi-gateway",
            "fourth-persistent-register-call-target-mismatch",
            "fourth GetProcAddress call requires live ESI to retain the owned import thunk",
        ));
    }
    let sequence_address = request.return_address.checked_sub(13).ok_or_else(|| {
        LoaderError::invalid_input(
            "abi-gateway",
            "fourth-persistent-register-call-address-underflow",
            "fourth GetProcAddress return cannot identify its register-indirect sequence",
        )
    })?;
    let sequence = memory
        .fetch(GuestAddress(sequence_address), 13)
        .map_err(memory_error)?;
    let name_operand = u32::from_le_bytes(sequence[1..5].try_into().map_err(|_| {
        LoaderError::runtime_defect(
            "abi-gateway",
            "fourth-persistent-register-name-width-invalid",
            "register-indirect sequence has a partial procedure-name operand",
        )
    })?);
    let store_operand = u32::from_le_bytes(sequence[7..11].try_into().map_err(|_| {
        LoaderError::runtime_defect(
            "abi-gateway",
            "fourth-persistent-register-store-width-invalid",
            "register-indirect sequence has a partial prior-result store operand",
        )
    })?);
    if sequence[0] != 0x68
        || name_operand != request.procedure_name_address
        || sequence[5] != 0x57
        || sequence[6] != 0xa3
        || store_operand != request.third_endpoint_store_address
        || sequence[11..13] != [0xff, 0xd6]
    {
        return Err(LoaderError::invalid_input(
            "abi-gateway",
            "fourth-persistent-register-callsite-mismatch",
            "fourth GetProcAddress return does not follow the controlled PUSH name / PUSH EDI / STORE prior result / CALL ESI sequence",
        ));
    }
    validate_bound_iat(memory, thunk)
}

fn validate_stdcall_frame(
    memory: &GuestMemory,
    stack_pointer: u32,
    return_address: u32,
    expected_arguments: &[u32],
) -> Result<(), LoaderError> {
    let frame_words = expected_arguments.len().checked_add(1).ok_or_else(|| {
        LoaderError::resource_limit(
            "abi-gateway",
            "stdcall-frame-word-count-overflow",
            "stdcall frame word count overflowed host accounting",
        )
    })?;
    let frame_bytes = frame_words.checked_mul(4).ok_or_else(|| {
        LoaderError::resource_limit(
            "abi-gateway",
            "stdcall-frame-byte-count-overflow",
            "stdcall frame byte count overflowed host accounting",
        )
    })?;
    let frame = memory
        .read(GuestAddress(stack_pointer), frame_bytes)
        .map_err(memory_error)?;
    let observed_words = frame
        .chunks_exact(4)
        .map(|word| {
            let word = word.try_into().map_err(|_| {
                LoaderError::runtime_defect(
                    "abi-gateway",
                    "stdcall-frame-word-width-invalid",
                    "checked stdcall frame contained a partial word",
                )
            })?;
            Ok(u32::from_le_bytes(word))
        })
        .collect::<Result<Vec<_>, LoaderError>>()?;
    if observed_words.first().copied() != Some(return_address)
        || observed_words.get(1..) != Some(expected_arguments)
    {
        return Err(LoaderError::invalid_input(
            "abi-gateway",
            "stdcall-frame-mismatch",
            "reconciled stdcall frame does not match the adapter-observed return and arguments",
        ));
    }

    Ok(())
}

fn validate_bound_iat(memory: &GuestMemory, thunk: &ImportThunk) -> Result<(), LoaderError> {
    let iat = memory.read(thunk.iat_address, 4).map_err(memory_error)?;
    let iat_value = u32::from_le_bytes(iat.try_into().map_err(|_| {
        LoaderError::runtime_defect(
            "abi-gateway",
            "stdcall-iat-width-invalid",
            "checked stdcall IAT entry did not contain four bytes",
        )
    })?);
    if iat_value != thunk.address.0 {
        return Err(LoaderError::invalid_input(
            "abi-gateway",
            "stdcall-iat-value-mismatch",
            "stdcall IAT entry no longer points to its owned thunk",
        ));
    }
    Ok(())
}

fn read_bounded_utf16_nul(
    memory: &GuestMemory,
    address: GuestAddress,
    maximum_units: usize,
) -> Result<(String, Vec<u8>), LoaderError> {
    let mut units = Vec::new();
    let mut bytes = Vec::new();
    for index in 0..maximum_units {
        let byte_offset = index.checked_mul(2).ok_or_else(|| {
            LoaderError::resource_limit(
                "win32-loaded-module",
                "utf16-offset-overflow",
                "bounded UTF-16 offset overflowed host accounting",
            )
        })?;
        let byte_offset = u32::try_from(byte_offset).map_err(|_| {
            LoaderError::resource_limit(
                "win32-loaded-module",
                "utf16-offset-unrepresentable",
                "bounded UTF-16 offset does not fit the guest address width",
            )
        })?;
        let unit_address = address.0.checked_add(byte_offset).ok_or_else(|| {
            LoaderError::invalid_input(
                "win32-loaded-module",
                "utf16-address-overflow",
                "bounded UTF-16 read crosses the guest address limit",
            )
        })?;
        let unit_bytes = memory
            .read(GuestAddress(unit_address), 2)
            .map_err(memory_error)?;
        let unit = u16::from_le_bytes(unit_bytes.as_slice().try_into().map_err(|_| {
            LoaderError::runtime_defect(
                "win32-loaded-module",
                "utf16-unit-width-invalid",
                "checked UTF-16 read did not return one complete code unit",
            )
        })?);
        bytes.extend_from_slice(&unit_bytes);
        if unit == 0 {
            let text = String::from_utf16(&units).map_err(|_| {
                LoaderError::invalid_input(
                    "win32-loaded-module",
                    "utf16-name-invalid",
                    "loaded-module name is not valid UTF-16",
                )
            })?;
            return Ok((text, bytes));
        }
        units.push(unit);
    }
    Err(LoaderError::invalid_input(
        "win32-loaded-module",
        "utf16-name-unterminated",
        "loaded-module name has no NUL terminator within the controlled bound",
    ))
}

fn read_bounded_ascii_nul(
    memory: &GuestMemory,
    address: GuestAddress,
    maximum_bytes: usize,
) -> Result<(String, Vec<u8>), LoaderError> {
    let mut bytes = Vec::new();
    for index in 0..maximum_bytes {
        let byte_offset = u32::try_from(index).map_err(|_| {
            LoaderError::resource_limit(
                "win32-dynamic-export",
                "ascii-offset-unrepresentable",
                "bounded ASCII offset does not fit the guest address width",
            )
        })?;
        let byte_address = address.0.checked_add(byte_offset).ok_or_else(|| {
            LoaderError::invalid_input(
                "win32-dynamic-export",
                "ascii-address-overflow",
                "bounded ASCII read crosses the guest address limit",
            )
        })?;
        let byte = memory
            .read(GuestAddress(byte_address), 1)
            .map_err(memory_error)?
            .into_iter()
            .next()
            .ok_or_else(|| {
                LoaderError::runtime_defect(
                    "win32-dynamic-export",
                    "ascii-byte-width-invalid",
                    "checked ASCII read did not return one byte",
                )
            })?;
        bytes.push(byte);
        if byte == 0 {
            let text = std::str::from_utf8(&bytes[..bytes.len() - 1]).map_err(|_| {
                LoaderError::invalid_input(
                    "win32-dynamic-export",
                    "ascii-name-invalid",
                    "dynamic-export name is not ASCII",
                )
            })?;
            if !text.is_ascii() {
                return Err(LoaderError::invalid_input(
                    "win32-dynamic-export",
                    "ascii-name-invalid",
                    "dynamic-export name is not ASCII",
                ));
            }
            return Ok((text.to_owned(), bytes));
        }
    }
    Err(LoaderError::invalid_input(
        "win32-dynamic-export",
        "ascii-name-unterminated",
        "dynamic-export name has no NUL terminator within the controlled bound",
    ))
}

fn read_protocol_message<T: DeserializeOwned>(
    input: &mut impl BufRead,
    maximum_bytes: usize,
    stage: &'static str,
    code: &'static str,
    label: &'static str,
) -> Result<T, LoaderError> {
    let mut bytes = Vec::new();
    input
        .take((maximum_bytes + 1) as u64)
        .read_until(b'\n', &mut bytes)
        .map_err(|error| io_error(stage, "protocol-message-read-failed", label, error))?;
    if bytes.is_empty() {
        return Err(LoaderError::invalid_input(
            stage,
            "protocol-message-missing",
            format!("{label} was not provided"),
        ));
    }
    if bytes.len() > maximum_bytes {
        return Err(LoaderError::resource_limit(
            stage,
            "protocol-message-size-exceeded",
            format!("{label} exceeds its {maximum_bytes}-byte limit"),
        ));
    }
    serde_json::from_slice(&bytes).map_err(|error| {
        LoaderError::invalid_input(stage, code, format!("{label} is not valid JSON: {error}"))
    })
}

fn write_protocol_message(
    message: &impl Serialize,
    maximum_bytes: usize,
) -> Result<(), LoaderError> {
    let bytes = serde_json::to_vec(message).map_err(|error| {
        LoaderError::runtime_defect(
            "memory-reflection",
            "reflection-message-serialization-failed",
            format!("could not serialize the reflection protocol message: {error}"),
        )
    })?;
    if bytes.len() > maximum_bytes {
        return Err(LoaderError::resource_limit(
            "memory-reflection",
            "reflection-message-size-exceeded",
            format!(
                "reflection protocol message has {} bytes; limit is {maximum_bytes} bytes",
                bytes.len()
            ),
        ));
    }
    let mut stdout = io::stdout().lock();
    stdout.write_all(&bytes).map_err(|error| {
        io_error(
            "memory-reflection",
            "reflection-message-write-failed",
            "could not write the reflection protocol message",
            error,
        )
    })?;
    stdout.write_all(b"\n").map_err(|error| {
        io_error(
            "memory-reflection",
            "reflection-message-write-failed",
            "could not write the reflection protocol message",
            error,
        )
    })?;
    stdout.flush().map_err(|error| {
        io_error(
            "memory-reflection",
            "reflection-message-flush-failed",
            "could not flush the reflection protocol message",
            error,
        )
    })
}

fn serialize_report(report: &impl Serialize) -> Result<Vec<u8>, LoaderError> {
    serde_json::to_vec_pretty(report).map_err(|error| {
        LoaderError::runtime_defect(
            "probe-output",
            "report-serialization-failed",
            format!("could not serialize the deterministic ingress report: {error}"),
        )
    })
}

fn write_report(output: &Path, json: &[u8]) -> Result<(), LoaderError> {
    if let Some(parent) = output.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(|error| {
            io_error(
                "probe-output",
                "output-directory-create-failed",
                "could not create the report output directory",
                error,
            )
        })?;
    }
    let mut output_file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)
        .map_err(|error| {
            let (code, context) = if error.kind() == io::ErrorKind::AlreadyExists {
                (
                    "report-output-already-exists",
                    "refusing to replace an existing report or source file",
                )
            } else {
                (
                    "report-create-failed",
                    "could not create the ingress report",
                )
            };
            io_error("probe-output", code, context, error)
        })?;
    output_file.write_all(json).map_err(|error| {
        io_error(
            "probe-output",
            "report-write-failed",
            "could not write the ingress report",
            error,
        )
    })
}

fn load_runtime_checkpoint_selection(
    path: &Path,
    checkpoint_id: &str,
) -> Result<RuntimeCheckpointSelection, LoaderError> {
    const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;
    const MAX_CHECKPOINTS: usize = 64;
    const MAX_SUPPORTED_CALLS: usize = 17;

    let bytes = read_bounded(path, MAX_MANIFEST_BYTES).map_err(|error| LoaderError {
        stage: "checkpoint-manifest",
        ..error
    })?;
    let manifest: RuntimeCheckpointManifest = serde_json::from_slice(&bytes).map_err(|error| {
        LoaderError::invalid_input(
            "checkpoint-manifest",
            "checkpoint-manifest-json-invalid",
            format!("could not parse runtime checkpoint manifest: {error}"),
        )
    })?;
    if manifest.schema != "vine.runtime-checkpoint-manifest" || manifest.schema_version != 1 {
        return Err(LoaderError::invalid_input(
            "checkpoint-manifest",
            "checkpoint-manifest-schema-unsupported",
            "runtime checkpoint manifest must use vine.runtime-checkpoint-manifest version 1",
        ));
    }
    if manifest.checkpoints.is_empty() || manifest.checkpoints.len() > MAX_CHECKPOINTS {
        return Err(LoaderError::resource_limit(
            "checkpoint-manifest",
            "checkpoint-manifest-count-invalid",
            "runtime checkpoint manifest must contain between 1 and 64 ordered checkpoints",
        ));
    }

    let mut ids = BTreeMap::new();
    let mut occurrences = BTreeMap::new();
    let mut dispatches = Vec::new();
    let mut selection = None;
    for (index, checkpoint) in manifest.checkpoints.iter().enumerate() {
        if checkpoint.id.is_empty() || ids.insert(checkpoint.id.as_str(), index).is_some() {
            return Err(LoaderError::invalid_input(
                "checkpoint-manifest",
                "checkpoint-manifest-id-invalid",
                "runtime checkpoint IDs must be non-empty and unique",
            ));
        }
        let expected_extends = index
            .checked_sub(1)
            .map(|previous| manifest.checkpoints[previous].id.as_str());
        if checkpoint.extends.as_deref() != expected_extends
            || checkpoint.supported_call_count != index
        {
            return Err(LoaderError::invalid_input(
                "checkpoint-manifest",
                "checkpoint-manifest-order-invalid",
                "runtime checkpoints must form one ordered extends chain with contiguous call counts",
            ));
        }

        match (&checkpoint.dispatch, index) {
            (None, 0) => {}
            (Some(dispatch), index) if index > 0 => {
                if dispatch.module.is_empty()
                    || dispatch.symbol.is_empty()
                    || dispatch.occurrence == 0
                {
                    return Err(LoaderError::invalid_input(
                        "checkpoint-manifest",
                        "checkpoint-manifest-dispatch-invalid",
                        "checkpoint dispatch identities and occurrences must be non-empty",
                    ));
                }
                let key = format!(
                    "{}!{}",
                    dispatch.module.to_ascii_lowercase(),
                    dispatch.symbol
                );
                let occurrence = occurrences.entry(key).or_insert(0usize);
                *occurrence += 1;
                if dispatch.occurrence != *occurrence {
                    return Err(LoaderError::invalid_input(
                        "checkpoint-manifest",
                        "checkpoint-manifest-occurrence-invalid",
                        "checkpoint dispatch occurrences must be sequential per API identity",
                    ));
                }
                dispatches.push(dispatch.clone());
            }
            _ => {
                return Err(LoaderError::invalid_input(
                    "checkpoint-manifest",
                    "checkpoint-manifest-dispatch-order-invalid",
                    "only the initial trap checkpoint may omit a dispatch identity",
                ));
            }
        }

        if checkpoint.id == checkpoint_id {
            selection = Some(RuntimeCheckpointSelection {
                supported_call_count: checkpoint.supported_call_count,
                dispatches: dispatches.clone(),
            });
        }
    }

    let selection = selection.ok_or_else(|| {
        LoaderError::invalid_input(
            "checkpoint-manifest",
            "checkpoint-manifest-selection-missing",
            format!("runtime checkpoint manifest does not define {checkpoint_id}"),
        )
    })?;
    if selection.supported_call_count > MAX_SUPPORTED_CALLS {
        return Err(LoaderError::unsupported(
            "checkpoint-manifest",
            "checkpoint-manifest-call-count-unsupported",
            "this probe supports at most the existing 17-call checkpoint sequence",
        ));
    }
    Ok(selection)
}

fn load_kernel32_compatibility_policy(
    path: &Path,
) -> Result<Kernel32CompatibilityPolicy, LoaderError> {
    const MAX_POLICY_BYTES: u64 = 64 * 1024;

    let bytes = read_bounded(path, MAX_POLICY_BYTES).map_err(|error| LoaderError {
        stage: "win32-compatibility-policy",
        ..error
    })?;
    let policy: Kernel32CompatibilityPolicy = serde_json::from_slice(&bytes).map_err(|error| {
        LoaderError::invalid_input(
            "win32-compatibility-policy",
            "kernel32-policy-json-invalid",
            format!("could not parse Kernel32 compatibility policy: {error}"),
        )
    })?;
    policy.validate().map_err(|error| {
        LoaderError::invalid_input("win32-compatibility-policy", error.code, error.message)
    })?;
    Ok(policy)
}

fn load_source_snapshot_manifest(
    path: &Path,
) -> Result<RuntimeSourceSnapshotManifest, LoaderError> {
    const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;

    let bytes = read_bounded(path, MAX_MANIFEST_BYTES).map_err(|error| LoaderError {
        stage: "source-snapshot-manifest",
        ..error
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        LoaderError::invalid_input(
            "source-snapshot-manifest",
            "source-snapshot-json-invalid",
            format!("could not parse source snapshot manifest: {error}"),
        )
    })
}

fn load_writable_profile_manifest(
    path: &Path,
) -> Result<RuntimeWritableProfileManifest, LoaderError> {
    const MAX_MANIFEST_BYTES: u64 = 16 * 1024 * 1024;

    let bytes = read_bounded(path, MAX_MANIFEST_BYTES).map_err(|error| LoaderError {
        stage: "writable-profile-manifest",
        ..error
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        LoaderError::invalid_input(
            "writable-profile-manifest",
            "writable-profile-json-invalid",
            format!("could not parse writable profile manifest: {error}"),
        )
    })
}

fn parse_arguments() -> Result<Arguments, LoaderError> {
    let mut input = None;
    let mut output = None;
    let mut expected_sha256 = None;
    let mut prepare_memory_pages = None;
    let mut materialize_image_base = None;
    let mut runtime_exports = Vec::new();
    let mut trap_unsupported_imports = false;
    let mut v86_reflection_session = false;
    let mut generic_runtime_reentry = false;
    let mut generic_runtime_capability = None;
    let mut generic_runtime_trap_limit = None;
    let mut runtime_policy = None;
    let mut source_manifest = None;
    let mut writable_profile = None;
    let mut checkpoint_manifest = None;
    let mut checkpoint_id = None;
    let mut large_image_reflection = false;
    let mut install_callback_return_sentinel = false;
    let mut install_process_entry_return_sentinel = false;
    let mut install_controlled_initial_stack = false;
    let mut install_controlled_process_environment = false;
    let mut install_controlled_process_parameters = false;
    let mut controlled_command_line = None;
    let mut arguments = env::args_os().skip(1);

    while let Some(argument) = arguments.next() {
        if argument == "--output" {
            let value = arguments.next().ok_or_else(usage_error)?;
            if output.replace(PathBuf::from(value)).is_some() {
                return Err(usage_error());
            }
        } else if argument == "--expect-sha256" {
            let value = arguments.next().ok_or_else(usage_error)?;
            let value = value.into_string().map_err(|_| usage_error())?;
            if expected_sha256.replace(value).is_some() {
                return Err(usage_error());
            }
        } else if argument == "--prepare-memory-pages" {
            let value = arguments.next().ok_or_else(usage_error)?;
            let value = value.into_string().map_err(|_| usage_error())?;
            let value = value.parse::<usize>().map_err(|_| usage_error())?;
            if value == 0 || prepare_memory_pages.replace(value).is_some() {
                return Err(usage_error());
            }
        } else if argument == "--materialize-image-base" {
            let value = arguments.next().ok_or_else(usage_error)?;
            let value = value.into_string().map_err(|_| usage_error())?;
            let value = parse_u32_argument(&value)?;
            if materialize_image_base.replace(value).is_some() {
                return Err(usage_error());
            }
        } else if argument == "--bind-name" {
            let module = arguments.next().ok_or_else(usage_error)?;
            let module = module.into_string().map_err(|_| usage_error())?;
            let name = arguments.next().ok_or_else(usage_error)?;
            let name = name.into_string().map_err(|_| usage_error())?;
            runtime_exports.push(RuntimeExport::named(module, name));
        } else if argument == "--bind-ordinal" {
            let module = arguments.next().ok_or_else(usage_error)?;
            let module = module.into_string().map_err(|_| usage_error())?;
            let ordinal = arguments.next().ok_or_else(usage_error)?;
            let ordinal = ordinal.into_string().map_err(|_| usage_error())?;
            let ordinal = ordinal.parse::<u16>().map_err(|_| usage_error())?;
            runtime_exports.push(RuntimeExport::ordinal(module, ordinal));
        } else if argument == "--trap-unsupported-imports" {
            if trap_unsupported_imports {
                return Err(usage_error());
            }
            trap_unsupported_imports = true;
        } else if argument == "--v86-reflection-session" {
            if v86_reflection_session {
                return Err(usage_error());
            }
            v86_reflection_session = true;
        } else if argument == "--generic-runtime-reentry" {
            if generic_runtime_reentry {
                return Err(usage_error());
            }
            generic_runtime_reentry = true;
        } else if argument == "--runtime-capability" {
            let value = arguments.next().ok_or_else(usage_error)?;
            let value = value.into_string().map_err(|_| usage_error())?;
            let capability = match value.as_str() {
                "kernel32-critical-section-synchronization" => {
                    GenericRuntimeCapability::CriticalSectionSynchronization
                }
                "kernel32-pointer-decoding" => GenericRuntimeCapability::PointerDecoding,
                "kernel32-fiber-local-storage-allocation" => {
                    GenericRuntimeCapability::FiberLocalStorageAllocation
                }
                "kernel32-private-heap-allocation" => {
                    GenericRuntimeCapability::PrivateHeapAllocation
                }
                "kernel32-fiber-local-storage-value" => {
                    GenericRuntimeCapability::FiberLocalStorageValue
                }
                "kernel32-interlocked-increment" => GenericRuntimeCapability::InterlockedIncrement,
                "kernel32-private-heap-second-allocation" => {
                    GenericRuntimeCapability::PrivateHeapSecondAllocation
                }
                "kernel32-standard-input-handle" => GenericRuntimeCapability::StandardInputHandle,
                "kernel32-file-type" => GenericRuntimeCapability::FileType,
                "kernel32-standard-output-handle" => GenericRuntimeCapability::StandardOutputHandle,
                "kernel32-standard-output-file-type" => {
                    GenericRuntimeCapability::StandardOutputFileType
                }
                "kernel32-standard-error-handle" => GenericRuntimeCapability::StandardErrorHandle,
                "kernel32-standard-error-file-type" => {
                    GenericRuntimeCapability::StandardErrorFileType
                }
                "kernel32-set-handle-count" => GenericRuntimeCapability::SetHandleCount,
                "kernel32-command-line-a" => GenericRuntimeCapability::CommandLineA,
                "kernel32-environment-strings-w" => GenericRuntimeCapability::EnvironmentStringsW,
                "kernel32-wide-char-to-multi-byte" => GenericRuntimeCapability::WideCharToMultiByte,
                "kernel32-private-heap-size-1" => GenericRuntimeCapability::PrivateHeapSize1,
                "kernel32-get-last-error" => GenericRuntimeCapability::GetLastError,
                "kernel32-tls-get-value" => GenericRuntimeCapability::TlsGetValue,
                "kernel32-fls-get-value" => GenericRuntimeCapability::FlsGetValue,
                "kernel32-set-last-error" => GenericRuntimeCapability::SetLastError,
                "kernel32-get-acp" => GenericRuntimeCapability::GetAcp,
                "kernel32-private-heap-size-544" => GenericRuntimeCapability::PrivateHeapSize544,
                "kernel32-get-cp-info" => GenericRuntimeCapability::GetCpInfo,
                "kernel32-multi-byte-to-wide-char" => GenericRuntimeCapability::MultiByteToWideChar,
                "kernel32-get-string-type-w" => GenericRuntimeCapability::GetStringTypeW,
                "kernel32-lc-map-string-w" => GenericRuntimeCapability::LcMapStringW,
                "kernel32-interlocked-decrement" => GenericRuntimeCapability::InterlockedDecrement,
                "kernel32-get-module-file-name-a" => GenericRuntimeCapability::GetModuleFileNameA,
                "kernel32-private-heap-size-16" => GenericRuntimeCapability::PrivateHeapSize16,
                "kernel32-private-heap-size-4" => GenericRuntimeCapability::PrivateHeapSize4,
                "kernel32-private-heap-free-size-1" => {
                    GenericRuntimeCapability::PrivateHeapFreeSize1
                }
                "kernel32-private-heap-size-128" => GenericRuntimeCapability::PrivateHeapSize128,
                "kernel32-is-processor-feature-present" => {
                    GenericRuntimeCapability::IsProcessorFeaturePresent
                }
                "kernel32-set-unhandled-exception-filter" => {
                    GenericRuntimeCapability::SetUnhandledExceptionFilter
                }
                "kernel32-heap-size" => GenericRuntimeCapability::HeapSize,
                "kernel32-initialize-critical-section"
                | "kernel32-multi-byte-to-wide-char-cp-acp" => {
                    GenericRuntimeCapability::InitializeCriticalSection
                }
                "kernel32-set-current-directory-a" => {
                    GenericRuntimeCapability::SetCurrentDirectoryA
                }
                "kernel32-create-mutex-a" => GenericRuntimeCapability::CreateMutexA,
                "kernel32-get-current-thread" => GenericRuntimeCapability::GetCurrentThread,
                "kernel32-set-thread-affinity-mask" => {
                    GenericRuntimeCapability::SetThreadAffinityMask
                }
                "kernel32-get-system-info" => GenericRuntimeCapability::GetSystemInfo,
                "user32-enum-display-monitors" => GenericRuntimeCapability::EnumDisplayMonitors,
                "ole32-co-initialize" => GenericRuntimeCapability::CoInitialize,
                "kernel32-get-system-default-lang-id" => {
                    GenericRuntimeCapability::GetSystemDefaultLangId
                }
                "kernel32-get-drive-type-a" => GenericRuntimeCapability::GetDriveTypeA,
                "user32-swap-mouse-button" => GenericRuntimeCapability::SwapMouseButton,
                "winmm-time-get-dev-caps" => GenericRuntimeCapability::TimeGetDevCaps,
                "winmm-time-begin-period" => GenericRuntimeCapability::TimeBeginPeriod,
                "kernel32-get-version-ex-a" => GenericRuntimeCapability::GetVersionExA,
                "kernel32-create-waitable-timer-a" => {
                    GenericRuntimeCapability::CreateWaitableTimerA
                }
                "user32-load-icon-a" => GenericRuntimeCapability::LoadIconA,
                "user32-load-cursor-a" => GenericRuntimeCapability::LoadCursorA,
                "user32-register-class-ex-a" => GenericRuntimeCapability::RegisterClassExA,
                "user32-get-system-metrics" => GenericRuntimeCapability::GetSystemMetrics,
                "kernel32-large-heap-allocation" => GenericRuntimeCapability::LargeHeapAllocation,
                "kernel32-source-file-read" => GenericRuntimeCapability::SourceFileRead,
                "user32-create-window-ex-a" => GenericRuntimeCapability::CreateWindowExA,
                "user32-get-system-metrics-small-icon-height" => {
                    GenericRuntimeCapability::GetSystemMetricsSmallIconHeight
                }
                "user32-get-system-metrics-small-icon-width" => {
                    GenericRuntimeCapability::GetSystemMetricsSmallIconWidth
                }
                "user32-load-image-a-small-icon" => GenericRuntimeCapability::LoadImageASmallIcon,
                "user32-send-message-a-small-icon" => {
                    GenericRuntimeCapability::SendMessageASmallIcon
                }
                "user32-set-window-pos-initial-placement" => {
                    GenericRuntimeCapability::SetWindowPosInitialPlacement
                }
                "d3d9-root-factory-and-identity" => {
                    GenericRuntimeCapability::D3d9RootFactoryAndIdentity
                }
                "d3d9-get-adapter-display-mode" => {
                    GenericRuntimeCapability::D3d9GetAdapterDisplayMode
                }
                "d3d9-get-adapter-identifier" => GenericRuntimeCapability::D3d9GetAdapterIdentifier,
                "d3d9-get-device-caps" => GenericRuntimeCapability::D3d9GetDeviceCaps,
                "d3d9-check-device-type" => GenericRuntimeCapability::D3d9CheckDeviceType,
                "d3d9-check-device-format" => GenericRuntimeCapability::D3d9CheckDeviceFormat,
                "kernel32-load-library-a" => GenericRuntimeCapability::LoadLibraryA,
                "user32-get-client-rect" => GenericRuntimeCapability::GetClientRect,
                "gdi-window-dc-fill" => GenericRuntimeCapability::WindowDcFill,
                "user32-set-window-long-a" => GenericRuntimeCapability::SetWindowLongA,
                "user32-set-window-pos-activation" => {
                    GenericRuntimeCapability::SetWindowPosActivation
                }
                "user32-set-window-pos-repeat-activation" => {
                    GenericRuntimeCapability::CreateDirectoryA
                }
                "user32-monitor-from-window" => GenericRuntimeCapability::MonitorFromWindow,
                "d3d9-get-adapter-count" => GenericRuntimeCapability::D3d9GetAdapterCount,
                "d3d9-get-adapter-monitor" => GenericRuntimeCapability::D3d9GetAdapterMonitor,
                "d3d9-create-device" => GenericRuntimeCapability::D3d9CreateDevice,
                "d3d9-create-texture" => GenericRuntimeCapability::D3d9CreateTexture,
                "d3d9-get-surface-level" => GenericRuntimeCapability::D3d9GetSurfaceLevel,
                "d3d9-surface-get-dc" => GenericRuntimeCapability::D3d9SurfaceGetDc,
                "winmm-time-get-time" => GenericRuntimeCapability::TimeGetTime,
                "d3d9-initial-textures" => GenericRuntimeCapability::D3d9InitialTextures,
                "d3d9-initial-vertex-buffer" => GenericRuntimeCapability::D3d9InitialVertexBuffer,
                "d3d9-initial-render-states" => GenericRuntimeCapability::D3d9InitialRenderStates,
                "d3d9-raster-status" => GenericRuntimeCapability::D3d9RasterStatus,
                "kernel32-query-performance-frequency" => {
                    GenericRuntimeCapability::QueryPerformanceFrequency
                }
                "user32-get-monitor-info-a" => GenericRuntimeCapability::GetMonitorInfoA,
                "dinput8-api-contract" => GenericRuntimeCapability::Dinput8Factory,
                "dinput8-enum-devices" => GenericRuntimeCapability::Dinput8EnumDevices,
                "dsound-root-and-cooperative-level" => {
                    GenericRuntimeCapability::DsoundRootAndCooperativeLevel
                }
                "dsound-initial-buffers" => GenericRuntimeCapability::DsoundInitialBuffers,
                "dsound-buffer-lock-unlock-play" => {
                    GenericRuntimeCapability::DsoundBufferLockUnlockPlay
                }
                "user32-set-timer" => GenericRuntimeCapability::SetTimer,
                "kernel32-create-thread" => GenericRuntimeCapability::CreateThread,
                "kernel32-set-thread-priority" => GenericRuntimeCapability::SetThreadPriority,
                "gdi-screen-dc" => GenericRuntimeCapability::ScreenDc,
                "gdi-font-enumeration" => GenericRuntimeCapability::FontEnumeration,
                "gdi-font-creation" => GenericRuntimeCapability::FontCreation,
                "gdi-compatible-dc" => GenericRuntimeCapability::CompatibleDc,
                "gdi-dib-lifecycle" => GenericRuntimeCapability::DibLifecycle,
                "gdi-outline-text-metrics" => GenericRuntimeCapability::OutlineTextMetrics,
                "gdi-text-raster" => GenericRuntimeCapability::TextRaster,
                "gdiplus-startup" => GenericRuntimeCapability::GdiplusStartup,
                "kernel32-virtual-alloc" => GenericRuntimeCapability::VirtualAlloc,
                "imm32-default-ime-context" => GenericRuntimeCapability::Imm32Context,
                "kernel32-set-waitable-timer" => GenericRuntimeCapability::SetWaitableTimer,
                "kernel32-wait-for-single-object-ex" => {
                    GenericRuntimeCapability::WaitForSingleObjectEx
                }
                "kernel32-enter-critical-section" => {
                    GenericRuntimeCapability::WaitForSingleObjectEx
                }
                "kernel32-enter-critical-section-continuation" => {
                    GenericRuntimeCapability::WaitForSingleObjectEx
                }
                "kernel32-leave-critical-section-continuation" => {
                    GenericRuntimeCapability::WaitForSingleObjectEx
                }
                "primary-main-loop-continuation" => GenericRuntimeCapability::WaitForSingleObjectEx,
                "primary-main-loop-progression" => GenericRuntimeCapability::WaitForSingleObjectEx,
                "shell32-drag-accept-files" => GenericRuntimeCapability::DragAcceptFiles,
                "kernel32-archive-source-read" => GenericRuntimeCapability::DragAcceptFiles,
                "kernel32-sleep-zero" => GenericRuntimeCapability::SleepZero,
                "kernel32-global-memory-status" => GenericRuntimeCapability::GlobalMemoryStatus,
                "kernel32-get-file-attributes-a" => GenericRuntimeCapability::GetFileAttributesA,
                "kernel32-create-directory-a" => GenericRuntimeCapability::CreateDirectoryA,
                "d3d9-recreated-textures" => GenericRuntimeCapability::D3d9RecreatedTextures,
                "user32-set-window-pos-frame-change-move" => {
                    GenericRuntimeCapability::D3d9RecreatedTextures
                }
                "user32-show-window-normal" => GenericRuntimeCapability::D3d9RecreatedTextures,
                "user32-update-window" => GenericRuntimeCapability::D3d9RecreatedTextures,
                "d3d9-clear" => GenericRuntimeCapability::D3d9Clear,
                "user32-set-rect" => GenericRuntimeCapability::SetRect,
                "d3d9-add-dirty-rect" => GenericRuntimeCapability::D3d9AddDirtyRect,
                "d3d9-update-texture" => GenericRuntimeCapability::D3d9UpdateTexture,
                "d3d9-begin-scene" => GenericRuntimeCapability::D3d9BeginScene,
                "d3d9-initial-sampler-states" => GenericRuntimeCapability::D3d9InitialSamplerStates,
                "d3d9-initial-texture-stage-states" => {
                    GenericRuntimeCapability::D3d9InitialTextureStageStates
                }
                "d3d9-set-texture" => GenericRuntimeCapability::D3d9SetTexture,
                "d3d9-set-fvf" => GenericRuntimeCapability::D3d9SetFvf,
                "d3d9-set-stream-source" => GenericRuntimeCapability::D3d9SetStreamSource,
                "d3d9-draw-primitive" => GenericRuntimeCapability::D3d9DrawPrimitive,
                "d3d9-end-scene" => GenericRuntimeCapability::D3d9EndScene,
                "d3d9-post-end-scene-raster-status" => {
                    GenericRuntimeCapability::D3d9PostEndSceneRasterStatus
                }
                "d3d9-post-raster-status-waitable-timer" => {
                    GenericRuntimeCapability::D3d9PostRasterStatusWaitableTimer
                }
                "d3d9-post-scene-present-gateway" => {
                    GenericRuntimeCapability::D3d9PostScenePresentGateway
                }
                "d3d9-first-present" => GenericRuntimeCapability::D3d9Present,
                "d3d9-post-present-continuation" => {
                    GenericRuntimeCapability::D3d9PostPresentContinuation
                }
                "user32-async-key-state-input" | "user32-async-key-state-input-continuation" => {
                    GenericRuntimeCapability::User32AsyncKeyStateInput
                }
                "user32-async-key-state-initial-scan"
                | "user32-async-key-state-initial-scan-continuation"
                | "user32-peek-message-a"
                | "user32-is-iconic"
                | "runtime-thread-stack-growth"
                | "runtime-trapless-execution-budget"
                | "kernel32-heap-free-large-buffer"
                | "kernel32-heap-alloc-24-byte-buffer"
                | "winmm-time-get-time-post-heap-reuse"
                | "kernel32-enter-critical-section-post-clock"
                | "kernel32-enter-critical-section-post-wait"
                | "kernel32-set-waitable-timer-post-critical-section-batch"
                | "kernel32-set-waitable-timer-post-timer-wait-batch"
                | "kernel32-set-waitable-timer-post-timer-rearm-wait-batch"
                | "primary-main-loop-recurring-timer-cycle-batch"
                | "primary-main-loop-recurring-timer-multi-cycle-batch"
                | "primary-main-loop-recurring-timer-window-batch"
                | "primary-main-loop-presentation-progress-gateway"
                | "primary-main-loop-input-progress-gateway"
                | "primary-main-loop-action-input-progress-gateway"
                | "kernel32-wait-for-single-object-ex-post-timer-arm-batch"
                | "kernel32-wait-for-single-object-ex-post-timer-rearm-batch"
                | "kernel32-enter-critical-section-post-timer-wait-batch"
                | "kernel32-enter-critical-section-post-timer-rearm-wait-batch"
                | "kernel32-wait-for-single-object-ex-post-critical-section" => {
                    GenericRuntimeCapability::User32AsyncKeyStateInitialScan
                }
                "primary-main-loop-window-timer-callback-delivery-gateway" => {
                    GenericRuntimeCapability::User32WindowTimerCallbackDelivery
                }
                "primary-main-loop-window-timer-callback-recurring-batch" => {
                    GenericRuntimeCapability::User32WindowTimerCallbackDelivery
                }
                "primary-main-loop-window-timer-callback-action-input-progress-gateway" => {
                    GenericRuntimeCapability::User32WindowTimerCallbackDelivery
                }
                "primary-main-loop-pointer-message-action-progress-gateway" => {
                    GenericRuntimeCapability::User32PointerButtonMessages
                }
                "user32-set-focus-action-continuation"
                | "winmm-time-get-time-action-continuation" => {
                    GenericRuntimeCapability::User32SetFocusSameWindow
                }
                "user32-def-window-proc-a-action-continuation"
                | "user32-peek-message-a-pointer-release-continuation"
                | "user32-translate-message-pointer-release-continuation"
                | "user32-dispatch-message-a-pointer-release-continuation" => {
                    GenericRuntimeCapability::User32DefWindowProcPointerDown
                }
                "user32-def-window-proc-a-pointer-release-continuation"
                | "user32-peek-message-a-post-pointer-release-continuation"
                | "winmm-time-get-time-post-pointer-release-continuation"
                | "winmm-time-get-time-post-pointer-release-repeat-batch"
                | "user32-get-window-rect-post-pointer-release-continuation"
                | "user32-get-cursor-pos-post-pointer-release-continuation"
                | "user32-get-system-metrics-post-pointer-release-continuation"
                | "user32-get-system-metrics-caption-post-pointer-release-continuation"
                | "user32-get-system-metrics-fixed-frame-height-post-pointer-release-continuation"
                | "user32-is-iconic-post-pointer-release-continuation"
                | "winmm-time-get-time-post-is-iconic-continuation"
                | "user32-get-async-key-state-post-is-iconic-clock-continuation"
                | "d3d9-test-cooperative-level-post-input-scan-continuation"
                | "d3d9-post-input-scan-waitable-timer-cycle"
                | "user32-window-timer-callback-post-waitable-timer-cycle"
                | "user32-peek-message-a-post-window-timer-callback-continuation" => {
                    GenericRuntimeCapability::User32DefWindowProcPointerRelease
                }
                "kernel32-leave-critical-section" => {
                    GenericRuntimeCapability::WaitForSingleObjectEx
                }
                _ => return Err(usage_error()),
            };
            if generic_runtime_capability.replace(capability).is_some() {
                return Err(usage_error());
            }
        } else if argument == "--generic-runtime-trap-limit" {
            let value = arguments.next().ok_or_else(usage_error)?;
            let value = value.into_string().map_err(|_| usage_error())?;
            let value = value.parse::<usize>().map_err(|_| usage_error())?;
            if value == 0
                || value > GENERIC_RUNTIME_MAX_TRAP_LIMIT
                || generic_runtime_trap_limit.replace(value).is_some()
            {
                return Err(usage_error());
            }
        } else if argument == "--runtime-policy" {
            let value = arguments.next().ok_or_else(usage_error)?;
            if runtime_policy.replace(PathBuf::from(value)).is_some() {
                return Err(usage_error());
            }
        } else if argument == "--source-manifest" {
            let value = arguments.next().ok_or_else(usage_error)?;
            if source_manifest.replace(PathBuf::from(value)).is_some() {
                return Err(usage_error());
            }
        } else if argument == "--writable-profile" {
            let value = arguments.next().ok_or_else(usage_error)?;
            if writable_profile.replace(PathBuf::from(value)).is_some() {
                return Err(usage_error());
            }
        } else if argument == "--checkpoint-manifest" {
            let value = arguments.next().ok_or_else(usage_error)?;
            if checkpoint_manifest.replace(PathBuf::from(value)).is_some() {
                return Err(usage_error());
            }
        } else if argument == "--checkpoint" {
            let value = arguments.next().ok_or_else(usage_error)?;
            let value = value.into_string().map_err(|_| usage_error())?;
            if checkpoint_id.replace(value).is_some() {
                return Err(usage_error());
            }
        } else if argument == "--large-image-reflection" {
            if large_image_reflection {
                return Err(usage_error());
            }
            large_image_reflection = true;
        } else if argument == "--install-callback-return-sentinel" {
            if install_callback_return_sentinel {
                return Err(usage_error());
            }
            install_callback_return_sentinel = true;
        } else if argument == "--install-process-entry-return-sentinel" {
            if install_process_entry_return_sentinel {
                return Err(usage_error());
            }
            install_process_entry_return_sentinel = true;
        } else if argument == "--install-controlled-initial-stack" {
            if install_controlled_initial_stack {
                return Err(usage_error());
            }
            install_controlled_initial_stack = true;
        } else if argument == "--install-controlled-process-environment" {
            if install_controlled_process_environment {
                return Err(usage_error());
            }
            install_controlled_process_environment = true;
        } else if argument == "--install-controlled-process-parameters" {
            if install_controlled_process_parameters {
                return Err(usage_error());
            }
            install_controlled_process_parameters = true;
        } else if argument == "--controlled-command-line" {
            let value = arguments.next().ok_or_else(usage_error)?;
            let value = value.into_string().map_err(|_| usage_error())?;
            if controlled_command_line.replace(value).is_some() {
                return Err(usage_error());
            }
        } else if argument.to_string_lossy().starts_with('-') || input.is_some() {
            return Err(usage_error());
        } else {
            input = Some(PathBuf::from(argument));
        }
    }

    if materialize_image_base.is_some() && prepare_memory_pages.is_none() {
        return Err(usage_error());
    }
    if !runtime_exports.is_empty() && materialize_image_base.is_none() {
        return Err(usage_error());
    }
    if trap_unsupported_imports && materialize_image_base.is_none() {
        return Err(usage_error());
    }
    if v86_reflection_session
        && (output.is_some() || materialize_image_base.is_none() || prepare_memory_pages.is_none())
    {
        return Err(usage_error());
    }
    if install_callback_return_sentinel && !v86_reflection_session {
        return Err(usage_error());
    }
    if install_process_entry_return_sentinel
        && (!v86_reflection_session || !install_callback_return_sentinel)
    {
        return Err(usage_error());
    }
    if install_controlled_initial_stack
        && (!v86_reflection_session || !install_process_entry_return_sentinel)
    {
        return Err(usage_error());
    }
    if install_controlled_process_environment && !install_controlled_initial_stack {
        return Err(usage_error());
    }
    if install_controlled_process_parameters && !install_controlled_process_environment {
        return Err(usage_error());
    }
    if large_image_reflection && !v86_reflection_session {
        return Err(usage_error());
    }
    if generic_runtime_reentry && (!v86_reflection_session || checkpoint_manifest.is_none()) {
        return Err(usage_error());
    }
    if generic_runtime_capability.is_some() && !generic_runtime_reentry {
        return Err(usage_error());
    }
    if generic_runtime_trap_limit.is_some() && !generic_runtime_reentry {
        return Err(usage_error());
    }
    if generic_runtime_reentry != runtime_policy.is_some() {
        return Err(usage_error());
    }
    if source_manifest.is_some() && !generic_runtime_reentry {
        return Err(usage_error());
    }
    if writable_profile.is_some() && !generic_runtime_reentry {
        return Err(usage_error());
    }
    if checkpoint_manifest.is_some() != checkpoint_id.is_some()
        || (checkpoint_manifest.is_some() && !v86_reflection_session)
    {
        return Err(usage_error());
    }
    if controlled_command_line.is_some() && !install_controlled_process_parameters {
        return Err(usage_error());
    }

    let checkpoint_session = checkpoint_manifest
        .as_deref()
        .zip(checkpoint_id.as_deref())
        .map(|(manifest, checkpoint)| load_runtime_checkpoint_selection(manifest, checkpoint))
        .transpose()?;
    let kernel32_policy = runtime_policy
        .as_deref()
        .map(load_kernel32_compatibility_policy)
        .transpose()?;
    let source_manifest = source_manifest
        .as_deref()
        .map(load_source_snapshot_manifest)
        .transpose()?;
    let writable_profile = writable_profile
        .as_deref()
        .map(load_writable_profile_manifest)
        .transpose()?;
    if kernel32_policy
        .as_ref()
        .is_some_and(|policy| policy.source_files().is_some())
        != (source_manifest.is_some() || writable_profile.is_some())
    {
        return Err(usage_error());
    }

    Ok(Arguments {
        input: input.ok_or_else(usage_error)?,
        output,
        expected_sha256,
        prepare_memory_pages,
        materialize_image_base,
        runtime_exports,
        trap_unsupported_imports,
        v86_reflection_session,
        checkpoint_session,
        generic_runtime_reentry,
        generic_runtime_capability,
        generic_runtime_trap_limit: generic_runtime_trap_limit
            .unwrap_or(GENERIC_RUNTIME_TRAP_LIMIT),
        kernel32_policy,
        source_manifest,
        writable_profile,
        large_image_reflection,
        install_callback_return_sentinel,
        install_process_entry_return_sentinel,
        install_controlled_initial_stack,
        install_controlled_process_environment,
        install_controlled_process_parameters,
        controlled_command_line,
    })
}

fn parse_u32_argument(value: &str) -> Result<u32, LoaderError> {
    let parsed = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
        .map_or_else(
            || value.parse::<u32>(),
            |hexadecimal| u32::from_str_radix(hexadecimal, 16),
        );
    parsed.map_err(|_| usage_error())
}

fn read_bounded(path: &Path, max_file_bytes: u64) -> Result<Vec<u8>, LoaderError> {
    let file = File::open(path).map_err(|error| {
        io_error(
            "module-snapshot",
            "source-open-failed",
            "could not open the requested PE source",
            error,
        )
    })?;
    let metadata = file.metadata().map_err(|error| {
        io_error(
            "module-snapshot",
            "source-metadata-failed",
            "could not inspect the requested PE source",
            error,
        )
    })?;
    if metadata.len() > max_file_bytes {
        return Err(LoaderError::resource_limit(
            "module-snapshot",
            "source-file-limit-exceeded",
            format!(
                "source has {} bytes; limit is {max_file_bytes} bytes",
                metadata.len()
            ),
        ));
    }

    let capacity = usize::try_from(metadata.len()).map_err(|_| {
        LoaderError::resource_limit(
            "module-snapshot",
            "source-length-not-representable",
            "source length does not fit the host index type",
        )
    })?;
    let mut bytes = Vec::with_capacity(capacity);
    file.take(max_file_bytes + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| {
            io_error(
                "module-snapshot",
                "source-read-failed",
                "could not read the requested PE source into an owned snapshot",
                error,
            )
        })?;
    if bytes.len() as u64 > max_file_bytes {
        return Err(LoaderError::resource_limit(
            "module-snapshot",
            "source-file-limit-exceeded",
            "source grew beyond the configured limit while it was read",
        ));
    }
    Ok(bytes)
}

fn usage_error() -> LoaderError {
    LoaderError {
        class: LoaderErrorClass::InvalidInput,
        stage: "probe-arguments",
        code: "usage",
        message: "usage: vine-pe-probe [--expect-sha256 HEX] [--prepare-memory-pages COUNT [--materialize-image-base ADDRESS [--bind-name MODULE SYMBOL | --bind-ordinal MODULE ORDINAL | --trap-unsupported-imports]...]] [--v86-reflection-session [--checkpoint-manifest FILE --checkpoint ID] [--generic-runtime-reentry [--runtime-capability ID] --runtime-policy FILE] [--large-image-reflection] [--install-callback-return-sentinel [--install-process-entry-return-sentinel [--install-controlled-initial-stack [--install-controlled-process-environment [--install-controlled-process-parameters [--controlled-command-line TEXT]]]]]]] | --output FILE] INPUT".to_owned(),
    }
}

fn is_false(value: &bool) -> bool {
    !*value
}

fn memory_error(error: GuestMemoryError) -> LoaderError {
    let class = match error.class {
        GuestMemoryErrorClass::ResourceLimit => LoaderErrorClass::ResourceLimit,
        GuestMemoryErrorClass::RuntimeDefect => LoaderErrorClass::RuntimeDefect,
        GuestMemoryErrorClass::InvalidRequest
        | GuestMemoryErrorClass::Conflict
        | GuestMemoryErrorClass::SynchronizationRequired
        | GuestMemoryErrorClass::Fault => LoaderErrorClass::InvalidInput,
    };
    LoaderError {
        class,
        stage: error.stage,
        code: error.code,
        message: error.message,
    }
}

fn io_error(
    stage: &'static str,
    code: &'static str,
    context: &'static str,
    error: io::Error,
) -> LoaderError {
    LoaderError {
        class: LoaderErrorClass::InvalidInput,
        stage,
        code,
        message: format!("{context}: {error}"),
    }
}

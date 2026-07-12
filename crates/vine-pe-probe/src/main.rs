#![forbid(unsafe_code)]

use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, Read, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use vine_core::{
    CallbackReturnSentinel, DynamicExportThunk, GuestAddress, GuestMemory, GuestMemoryError,
    GuestMemoryErrorClass, GuestPageReconciliation, GuestWriteResult, ImageMaterialization,
    ImageMemoryLayout, ImportBinding, ImportTarget, ImportThunk, ImportThunkDisposition,
    InitialProcessEnvironment, InitialThreadStack, LoaderError, LoaderErrorClass,
    LoaderIngressReport, MemoryDelta, MemoryInvalidation, MemoryLimits, MemoryOwnerId,
    MemoryReflection, MemoryStats, ModuleIdentity, ModuleSnapshot, Pe32Limits,
    ProcessEntryReturnSentinel, RuntimeExport, RuntimeExportCatalog, UnsupportedPoint,
    bind_pe32_imports, bind_pe32_imports_with_unsupported_traps, build_pe32_load_plan,
    install_callback_return_sentinel, install_controlled_initial_stack,
    install_controlled_process_environment,
    install_controlled_process_environment_with_command_line,
    install_controlled_process_environment_with_parameters, install_dynamic_export_thunk,
    install_dynamic_export_thunks, install_runtime_return_sentinels, materialize_pe32_image,
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
    system_time_delta_session: bool,
    process_id_return_session: bool,
    thread_id_return_session: bool,
    tick_count_return_session: bool,
    performance_counter_return_session: bool,
    startup_info_return_session: bool,
    heap_set_information_return_session: bool,
    heap_create_return_session: bool,
    get_module_handle_w_return_session: bool,
    get_proc_address_return_session: bool,
    get_proc_address_second_return_session: bool,
    get_proc_address_third_return_session: bool,
    get_proc_address_fourth_return_session: bool,
    large_image_reflection: bool,
    install_callback_return_sentinel: bool,
    install_process_entry_return_sentinel: bool,
    install_controlled_initial_stack: bool,
    install_controlled_process_environment: bool,
    install_controlled_process_parameters: bool,
    controlled_command_line: Option<String>,
}

#[derive(Clone, Copy, Debug)]
struct V86ReturnSessionOptions<'a> {
    process_id: bool,
    thread_id: bool,
    tick_count: bool,
    performance_counter: bool,
    startup_info: bool,
    heap_set_information: bool,
    heap_create: bool,
    get_module_handle_w: bool,
    get_proc_address: bool,
    get_proc_address_second: bool,
    get_proc_address_third: bool,
    get_proc_address_fourth: bool,
    dynamic_export_thunks: &'a [DynamicExportThunk],
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
        let binding = if arguments.trap_unsupported_imports {
            bind_pe32_imports_with_unsupported_traps(
                &mut memory,
                &loader_report,
                &materialization,
                &catalog,
            )?
        } else {
            bind_pe32_imports(&mut memory, &loader_report, &materialization, &catalog)?
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
    const MAX_LARGE_IMAGE_REFLECTION_PAGES: usize = 400;
    const MAX_REFLECTION_MESSAGE_BYTES: usize = 512 * 1024;
    const MAX_LARGE_IMAGE_REFLECTION_MESSAGE_BYTES: usize = 8 * 1024 * 1024;
    let load_base = arguments.materialize_image_base.ok_or_else(usage_error)?;
    let max_physical_pages = arguments.prepare_memory_pages.ok_or_else(usage_error)?;
    let reflection_page_limit = if arguments.large_image_reflection {
        MAX_LARGE_IMAGE_REFLECTION_PAGES
    } else {
        MAX_REFLECTION_PAGES
    };
    if max_physical_pages > reflection_page_limit {
        let message = if arguments.large_image_reflection {
            "the version 1 large-image reflection session is limited to 400 physical pages"
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
    let materialization = materialize_pe32_image(
        &mut memory,
        snapshot,
        loader_report,
        GuestAddress(load_base),
    )?;
    let trap_unsupported_imports = arguments.trap_unsupported_imports;
    let catalog = RuntimeExportCatalog::new(arguments.runtime_exports)?;
    let binding = if trap_unsupported_imports {
        bind_pe32_imports_with_unsupported_traps(
            &mut memory,
            loader_report,
            &materialization,
            &catalog,
        )?
    } else {
        bind_pe32_imports(&mut memory, loader_report, &materialization, &catalog)?
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
            let sentinels = install_runtime_return_sentinels(&mut memory).map_err(memory_error)?;
            (
                Some(sentinels.callback_return),
                Some(sentinels.process_entry_return),
            )
        } else {
            (
                arguments
                    .install_callback_return_sentinel
                    .then(|| install_callback_return_sentinel(&mut memory).map_err(memory_error))
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
        Some(install_controlled_initial_stack(&mut memory, entry_return).map_err(memory_error)?)
    } else {
        None
    };
    let initial_process_environment = if arguments.install_controlled_process_parameters {
        Some(
            if let Some(command_line) = arguments.controlled_command_line.as_deref() {
                install_controlled_process_environment_with_command_line(&mut memory, command_line)
                    .map_err(memory_error)?
            } else {
                install_controlled_process_environment_with_parameters(&mut memory)
                    .map_err(memory_error)?
            },
        )
    } else {
        arguments
            .install_controlled_process_environment
            .then(|| install_controlled_process_environment(&mut memory).map_err(memory_error))
            .transpose()?
    };
    let dynamic_export_thunks = if arguments.get_proc_address_fourth_return_session {
        install_dynamic_export_thunks(
            &mut memory,
            &[
                ("kernel32.dll", "FlsAlloc"),
                ("kernel32.dll", "FlsGetValue"),
                ("kernel32.dll", "FlsSetValue"),
                ("kernel32.dll", "FlsFree"),
            ],
        )
        .map_err(memory_error)?
    } else if arguments.get_proc_address_third_return_session {
        install_dynamic_export_thunks(
            &mut memory,
            &[
                ("kernel32.dll", "FlsAlloc"),
                ("kernel32.dll", "FlsGetValue"),
                ("kernel32.dll", "FlsSetValue"),
            ],
        )
        .map_err(memory_error)?
    } else if arguments.get_proc_address_second_return_session {
        install_dynamic_export_thunks(
            &mut memory,
            &[
                ("kernel32.dll", "FlsAlloc"),
                ("kernel32.dll", "FlsGetValue"),
            ],
        )
        .map_err(memory_error)?
    } else if arguments.get_proc_address_return_session {
        vec![
            install_dynamic_export_thunk(&mut memory, "kernel32.dll", "FlsAlloc")
                .map_err(memory_error)?,
        ]
    } else {
        Vec::new()
    };
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
        dynamic_export_thunks: if arguments.get_proc_address_second_return_session
            || arguments.get_proc_address_third_return_session
            || arguments.get_proc_address_fourth_return_session
        {
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
    let dynamic_export_page_prefix_bytes = (arguments.get_proc_address_second_return_session
        || arguments.get_proc_address_third_return_session
        || arguments.get_proc_address_fourth_return_session)
        .then(|| {
            memory
                .fetch(
                    vine_core::DYNAMIC_EXPORT_ARENA_BASE,
                    if arguments.get_proc_address_fourth_return_session {
                        64
                    } else if arguments.get_proc_address_third_return_session {
                        48
                    } else {
                        32
                    },
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
    if arguments.system_time_delta_session {
        let return_sessions = V86ReturnSessionOptions {
            process_id: arguments.process_id_return_session,
            thread_id: arguments.thread_id_return_session,
            tick_count: arguments.tick_count_return_session,
            performance_counter: arguments.performance_counter_return_session,
            startup_info: arguments.startup_info_return_session,
            heap_set_information: arguments.heap_set_information_return_session,
            heap_create: arguments.heap_create_return_session,
            get_module_handle_w: arguments.get_module_handle_w_return_session,
            get_proc_address: arguments.get_proc_address_return_session,
            get_proc_address_second: arguments.get_proc_address_second_return_session,
            get_proc_address_third: arguments.get_proc_address_third_return_session,
            get_proc_address_fourth: arguments.get_proc_address_fourth_return_session,
            dynamic_export_thunks: &dynamic_export_thunks,
        };
        run_v86_system_time_delta_session(
            &mut protocol_input,
            &mut memory,
            &binding,
            reflection_message_limit,
            return_sessions,
        )?;
    }
    Ok(())
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

    let file_time_ticks =
        (u64::from(request.file_time_high) << 32) | u64::from(request.file_time_low);
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

        validate_stdcall_iat_frame(
            &transaction,
            &thunk,
            request.stack_pointer,
            request.return_address,
            &[request.output_address],
        )?;

        let write = transaction
            .write(GuestAddress(request.output_address), &file_time_bytes)
            .map_err(memory_error)?;
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
    if return_sessions.process_id {
        run_v86_process_id_return_session(input, memory, binding, message_limit, return_sessions)?;
    }
    Ok(())
}

fn run_v86_process_id_return_session(
    input: &mut impl BufRead,
    memory: &mut GuestMemory,
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
    let thunk = binding
        .thunks
        .iter()
        .find(|thunk| thunk.id.0 == request.thunk_id)
        .ok_or_else(|| {
            LoaderError::invalid_input(
                "thunk-dispatch",
                "process-id-thunk-id-unowned",
                "process-ID request does not name an owned import thunk",
            )
        })?
        .clone();
    if thunk.address.0 != request.thunk_address
        || !thunk.module.eq_ignore_ascii_case("KERNEL32.dll")
        || thunk.disposition != ImportThunkDisposition::DeclaredRuntimeExport
        || !matches!(
            &thunk.target,
            ImportTarget::Name { name, .. } if name == "GetCurrentProcessId"
        )
    {
        return Err(LoaderError::invalid_input(
            "thunk-dispatch",
            "process-id-thunk-identity-mismatch",
            "process-ID request does not match the declared GetCurrentProcessId thunk",
        ));
    }

    let stack_reconciliation = {
        let mut transaction = memory.begin_transaction();
        let reconciliation = transaction
            .reconcile_reflected_page(
                request.reflected_generation,
                MemoryOwnerId(request.stack_page_owner),
                GuestAddress(request.stack_page_address),
                &request.stack_page_bytes,
            )
            .map_err(memory_error)?;
        validate_stdcall_iat_frame(
            &transaction,
            &thunk,
            request.stack_pointer,
            request.return_address,
            &[],
        )?;
        transaction.commit_transaction();
        reconciliation
    };

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
            process_id: request.process_id,
            stack_reconciliation,
            stats: memory.stats(),
        },
        message_limit,
    )?;
    if return_sessions.thread_id {
        run_v86_thread_id_return_session(input, memory, binding, message_limit, return_sessions)?;
    }
    Ok(())
}

fn run_v86_thread_id_return_session(
    input: &mut impl BufRead,
    memory: &mut GuestMemory,
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

    let stack_reconciliation = {
        let mut transaction = memory.begin_transaction();
        let reconciliation = transaction
            .reconcile_reflected_page(
                request.reflected_generation,
                MemoryOwnerId(request.stack_page_owner),
                GuestAddress(request.stack_page_address),
                &request.stack_page_bytes,
            )
            .map_err(memory_error)?;
        validate_stdcall_iat_frame(
            &transaction,
            &thunk,
            request.stack_pointer,
            request.return_address,
            &[],
        )?;
        transaction.commit_transaction();
        reconciliation
    };

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
            thread_id: request.thread_id,
            stack_reconciliation,
            stats: memory.stats(),
        },
        message_limit,
    )?;
    if return_sessions.tick_count {
        run_v86_tick_count_return_session(input, memory, binding, message_limit, return_sessions)?;
    }
    Ok(())
}

fn run_v86_tick_count_return_session(
    input: &mut impl BufRead,
    memory: &mut GuestMemory,
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

    let stack_reconciliation = {
        let mut transaction = memory.begin_transaction();
        let reconciliation = transaction
            .reconcile_reflected_page(
                request.reflected_generation,
                MemoryOwnerId(request.stack_page_owner),
                GuestAddress(request.stack_page_address),
                &request.stack_page_bytes,
            )
            .map_err(memory_error)?;
        validate_stdcall_iat_frame(
            &transaction,
            &thunk,
            request.stack_pointer,
            request.return_address,
            &[],
        )?;
        transaction.commit_transaction();
        reconciliation
    };

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
            tick_count: request.tick_count,
            stack_reconciliation,
            stats: memory.stats(),
        },
        message_limit,
    )?;
    if return_sessions.performance_counter {
        run_v86_performance_counter_return_session(
            input,
            memory,
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
    let thunk = binding
        .thunks
        .iter()
        .find(|thunk| thunk.id.0 == request.thunk_id)
        .ok_or_else(|| {
            LoaderError::invalid_input(
                "thunk-dispatch",
                "performance-counter-thunk-id-unowned",
                "performance-counter request does not name an owned import thunk",
            )
        })?
        .clone();
    if thunk.address.0 != request.thunk_address
        || !thunk.module.eq_ignore_ascii_case("KERNEL32.dll")
        || thunk.disposition != ImportThunkDisposition::DeclaredRuntimeExport
        || !matches!(
            &thunk.target,
            ImportTarget::Name { name, .. } if name == "QueryPerformanceCounter"
        )
    {
        return Err(LoaderError::invalid_input(
            "thunk-dispatch",
            "performance-counter-thunk-identity-mismatch",
            "performance-counter request does not match the declared QueryPerformanceCounter thunk",
        ));
    }

    let counter_ticks = (u64::from(request.counter_high) << 32) | u64::from(request.counter_low);
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
        validate_stdcall_iat_frame(
            &transaction,
            &thunk,
            request.stack_pointer,
            request.return_address,
            &[request.output_address],
        )?;
        let write = transaction
            .write(GuestAddress(request.output_address), &counter_bytes)
            .map_err(memory_error)?;
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
    if return_sessions.startup_info {
        run_v86_startup_info_return_session(
            input,
            memory,
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
    let thunk = binding
        .thunks
        .iter()
        .find(|thunk| thunk.id.0 == request.thunk_id)
        .ok_or_else(|| {
            LoaderError::invalid_input(
                "thunk-dispatch",
                "startup-info-thunk-id-unowned",
                "startup-info write request does not name an owned import thunk",
            )
        })?
        .clone();
    if thunk.address.0 != request.thunk_address
        || !thunk.module.eq_ignore_ascii_case("KERNEL32.dll")
        || thunk.disposition != ImportThunkDisposition::DeclaredRuntimeExport
        || !matches!(
            &thunk.target,
            ImportTarget::Name { name, .. } if name == "GetStartupInfoW"
        )
    {
        return Err(LoaderError::invalid_input(
            "thunk-dispatch",
            "startup-info-thunk-identity-mismatch",
            "startup-info write request does not match the declared GetStartupInfoW thunk",
        ));
    }

    let startup_info = X86StartupInfoW::controlled();
    let output_bytes = startup_info.to_bytes();
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
        validate_stdcall_iat_frame(
            &transaction,
            &thunk,
            request.stack_pointer,
            request.return_address,
            &[request.output_address],
        )?;
        let write = transaction
            .write(GuestAddress(request.output_address), &output_bytes)
            .map_err(memory_error)?;
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
    if return_sessions.heap_set_information {
        run_v86_heap_set_information_return_session(
            input,
            memory,
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

    let stack_reconciliation = {
        let mut transaction = memory.begin_transaction();
        let reconciliation = transaction
            .reconcile_reflected_page(
                request.reflected_generation,
                MemoryOwnerId(request.stack_page_owner),
                GuestAddress(request.stack_page_address),
                &request.stack_page_bytes,
            )
            .map_err(memory_error)?;
        validate_stdcall_iat_frame(
            &transaction,
            &thunk,
            request.stack_pointer,
            request.return_address,
            &[
                request.heap_handle,
                request.information_class,
                request.information_address,
                request.information_length,
            ],
        )?;
        transaction.commit_transaction();
        reconciliation
    };

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
            termination_on_corruption_enabled: true,
            preserve_last_error: request.preserve_last_error,
            result: 1,
            stack_reconciliation,
            stats: memory.stats(),
        },
        message_limit,
    )?;
    if return_sessions.heap_create {
        run_v86_heap_create_return_session(input, memory, binding, message_limit, return_sessions)?;
    }
    Ok(())
}

fn run_v86_heap_create_return_session(
    input: &mut impl BufRead,
    memory: &mut GuestMemory,
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

    let stack_reconciliation = {
        let mut transaction = memory.begin_transaction();
        let reconciliation = transaction
            .reconcile_reflected_page(
                request.reflected_generation,
                MemoryOwnerId(request.stack_page_owner),
                GuestAddress(request.stack_page_address),
                &request.stack_page_bytes,
            )
            .map_err(memory_error)?;
        validate_stdcall_iat_frame(
            &transaction,
            &thunk,
            request.stack_pointer,
            request.return_address,
            &[
                request.creation_flags,
                request.initial_size,
                request.maximum_size,
            ],
        )?;
        transaction.commit_transaction();
        reconciliation
    };

    let logical_heap = X86LogicalHeap {
        owner: request.heap_state_owner,
        variant_id: request.variant_id,
        handle: X86LogicalHeapHandle::first_private_heap(),
        kind: "private-heap",
        lifecycle: "active",
        creation_flags: request.creation_flags,
        initial_size: request.initial_size,
        maximum_size: request.maximum_size,
        growable: true,
        backing_materialization: "deferred-until-first-reached-heap-operation",
        live_allocations: 0,
    };
    let result = logical_heap.handle.value;
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
    if return_sessions.get_module_handle_w {
        run_v86_get_module_handle_w_return_session(
            input,
            memory,
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

    let (stack_reconciliation, observed_name, module_name_utf16_bytes) = {
        let mut transaction = memory.begin_transaction();
        let reconciliation = transaction
            .reconcile_reflected_page(
                request.reflected_generation,
                MemoryOwnerId(request.stack_page_owner),
                GuestAddress(request.stack_page_address),
                &request.stack_page_bytes,
            )
            .map_err(memory_error)?;
        validate_stdcall_iat_frame(
            &transaction,
            &thunk,
            request.stack_pointer,
            request.return_address,
            &[request.module_name_address],
        )?;
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
        transaction.commit_transaction();
        (reconciliation, observed_name, utf16_bytes)
    };

    let logical_module = X86LogicalLoadedModule {
        owner: request.loaded_module_owner,
        variant_id: request.variant_id,
        handle: X86LogicalLoadedModuleHandle::kernel32(),
        kind: "loaded-module",
        lifecycle: "loaded",
        requested_name: observed_name,
        canonical_name: CANONICAL_NAME,
        name_match: "unicode-case-insensitive-basename",
        module_role: "runtime-system-module",
        reference_count_changed: false,
        guest_image_materialization: "not-present",
        export_resolution: "deferred-until-first-reached-getprocaddress",
    };
    let result = logical_module.handle.value;
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
    if return_sessions.get_proc_address {
        run_v86_get_proc_address_return_session(
            input,
            memory,
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

    let (stack_reconciliation, observed_name, procedure_name_bytes) = {
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
        transaction.commit_transaction();
        (reconciliation, observed_name, ascii_bytes)
    };

    let result = endpoint.address.0;
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
    if return_sessions.get_proc_address_second {
        run_v86_get_proc_address_second_return_session(
            input,
            memory,
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
    let expected_endpoint_count = 2
        + usize::from(return_sessions.get_proc_address_third)
        + usize::from(return_sessions.get_proc_address_fourth);
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

    let (
        stack_reconciliation,
        first_endpoint_store_reconciliation,
        observed_name,
        procedure_name_bytes,
        first_endpoint_store_value,
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
        if first_endpoint_store_value != first_endpoint.address.0 {
            return Err(LoaderError::invalid_input(
                "win32-dynamic-export",
                "first-endpoint-store-value-mismatch",
                "BGI did not preserve the first dynamic-export endpoint before the second lookup",
            ));
        }
        transaction.commit_transaction();
        (
            stack_reconciliation,
            first_endpoint_store_reconciliation,
            observed_name,
            ascii_bytes,
            first_endpoint_store_value,
        )
    };

    let result = endpoint.address.0;
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
    if return_sessions.get_proc_address_third {
        run_v86_get_proc_address_third_return_session(
            input,
            memory,
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
        expected_endpoints.len() + usize::from(return_sessions.get_proc_address_fourth);
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

    let (
        stack_reconciliation,
        endpoint_store_reconciliation,
        observed_name,
        procedure_name_bytes,
        first_endpoint_store_value,
        second_endpoint_store_value,
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
        if first_endpoint_store_value != first_endpoint.address.0
            || second_endpoint_store_value != second_endpoint.address.0
        {
            return Err(LoaderError::invalid_input(
                "win32-dynamic-export",
                "prior-endpoint-store-value-mismatch",
                "BGI did not preserve the first two dynamic-export endpoints before the third lookup",
            ));
        }
        transaction.commit_transaction();
        (
            stack_reconciliation,
            endpoint_store_reconciliation,
            observed_name,
            ascii_bytes,
            first_endpoint_store_value,
            second_endpoint_store_value,
        )
    };

    let result = endpoint.address.0;
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
    if return_sessions.get_proc_address_fourth {
        run_v86_get_proc_address_fourth_return_session(
            input,
            memory,
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

    let (
        stack_reconciliation,
        endpoint_store_reconciliation,
        observed_name,
        procedure_name_bytes,
        first_endpoint_store_value,
        second_endpoint_store_value,
        third_endpoint_store_value,
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
        if first_endpoint_store_value != first_endpoint.address.0
            || second_endpoint_store_value != second_endpoint.address.0
            || third_endpoint_store_value != third_endpoint.address.0
        {
            return Err(LoaderError::invalid_input(
                "win32-dynamic-export",
                "prior-endpoint-store-value-mismatch",
                "BGI did not preserve the first three dynamic-export endpoints before the fourth lookup",
            ));
        }
        transaction.commit_transaction();
        (
            stack_reconciliation,
            endpoint_store_reconciliation,
            observed_name,
            ascii_bytes,
            first_endpoint_store_value,
            second_endpoint_store_value,
            third_endpoint_store_value,
        )
    };

    let result = endpoint.address.0;
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
    )
}

fn validate_stdcall_iat_frame(
    memory: &GuestMemory,
    thunk: &ImportThunk,
    stack_pointer: u32,
    return_address: u32,
    expected_arguments: &[u32],
) -> Result<(), LoaderError> {
    validate_stdcall_frame(memory, stack_pointer, return_address, expected_arguments)?;
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
    validate_bound_iat(memory, thunk)
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

fn parse_arguments() -> Result<Arguments, LoaderError> {
    let mut input = None;
    let mut output = None;
    let mut expected_sha256 = None;
    let mut prepare_memory_pages = None;
    let mut materialize_image_base = None;
    let mut runtime_exports = Vec::new();
    let mut trap_unsupported_imports = false;
    let mut v86_reflection_session = false;
    let mut system_time_delta_session = false;
    let mut process_id_return_session = false;
    let mut thread_id_return_session = false;
    let mut tick_count_return_session = false;
    let mut performance_counter_return_session = false;
    let mut startup_info_return_session = false;
    let mut heap_set_information_return_session = false;
    let mut heap_create_return_session = false;
    let mut get_module_handle_w_return_session = false;
    let mut get_proc_address_return_session = false;
    let mut get_proc_address_second_return_session = false;
    let mut get_proc_address_third_return_session = false;
    let mut get_proc_address_fourth_return_session = false;
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
        } else if argument == "--system-time-delta-session" {
            if system_time_delta_session {
                return Err(usage_error());
            }
            system_time_delta_session = true;
        } else if argument == "--process-id-return-session" {
            if process_id_return_session {
                return Err(usage_error());
            }
            process_id_return_session = true;
        } else if argument == "--thread-id-return-session" {
            if thread_id_return_session {
                return Err(usage_error());
            }
            thread_id_return_session = true;
        } else if argument == "--tick-count-return-session" {
            if tick_count_return_session {
                return Err(usage_error());
            }
            tick_count_return_session = true;
        } else if argument == "--performance-counter-return-session" {
            if performance_counter_return_session {
                return Err(usage_error());
            }
            performance_counter_return_session = true;
        } else if argument == "--startup-info-return-session" {
            if startup_info_return_session {
                return Err(usage_error());
            }
            startup_info_return_session = true;
        } else if argument == "--heap-set-information-return-session" {
            if heap_set_information_return_session {
                return Err(usage_error());
            }
            heap_set_information_return_session = true;
        } else if argument == "--heap-create-return-session" {
            if heap_create_return_session {
                return Err(usage_error());
            }
            heap_create_return_session = true;
        } else if argument == "--get-module-handle-w-return-session" {
            if get_module_handle_w_return_session {
                return Err(usage_error());
            }
            get_module_handle_w_return_session = true;
        } else if argument == "--get-proc-address-return-session" {
            if get_proc_address_return_session {
                return Err(usage_error());
            }
            get_proc_address_return_session = true;
        } else if argument == "--get-proc-address-second-return-session" {
            if get_proc_address_second_return_session {
                return Err(usage_error());
            }
            get_proc_address_second_return_session = true;
        } else if argument == "--get-proc-address-third-return-session" {
            if get_proc_address_third_return_session {
                return Err(usage_error());
            }
            get_proc_address_third_return_session = true;
        } else if argument == "--get-proc-address-fourth-return-session" {
            if get_proc_address_fourth_return_session {
                return Err(usage_error());
            }
            get_proc_address_fourth_return_session = true;
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
    if system_time_delta_session && !v86_reflection_session {
        return Err(usage_error());
    }
    if process_id_return_session && !system_time_delta_session {
        return Err(usage_error());
    }
    if thread_id_return_session && !process_id_return_session {
        return Err(usage_error());
    }
    if tick_count_return_session && !thread_id_return_session {
        return Err(usage_error());
    }
    if performance_counter_return_session && !tick_count_return_session {
        return Err(usage_error());
    }
    if startup_info_return_session && !performance_counter_return_session {
        return Err(usage_error());
    }
    if heap_set_information_return_session && !startup_info_return_session {
        return Err(usage_error());
    }
    if heap_create_return_session && !heap_set_information_return_session {
        return Err(usage_error());
    }
    if get_module_handle_w_return_session && !heap_create_return_session {
        return Err(usage_error());
    }
    if get_proc_address_return_session && !get_module_handle_w_return_session {
        return Err(usage_error());
    }
    if get_proc_address_second_return_session && !get_proc_address_return_session {
        return Err(usage_error());
    }
    if get_proc_address_third_return_session && !get_proc_address_second_return_session {
        return Err(usage_error());
    }
    if get_proc_address_fourth_return_session && !get_proc_address_third_return_session {
        return Err(usage_error());
    }
    if controlled_command_line.is_some() && !install_controlled_process_parameters {
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
        system_time_delta_session,
        process_id_return_session,
        thread_id_return_session,
        tick_count_return_session,
        performance_counter_return_session,
        startup_info_return_session,
        heap_set_information_return_session,
        heap_create_return_session,
        get_module_handle_w_return_session,
        get_proc_address_return_session,
        get_proc_address_second_return_session,
        get_proc_address_third_return_session,
        get_proc_address_fourth_return_session,
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
        message: "usage: vine-pe-probe [--expect-sha256 HEX] [--prepare-memory-pages COUNT [--materialize-image-base ADDRESS [--bind-name MODULE SYMBOL | --bind-ordinal MODULE ORDINAL | --trap-unsupported-imports]...]] [--v86-reflection-session [--system-time-delta-session [--process-id-return-session [--thread-id-return-session [--tick-count-return-session [--performance-counter-return-session [--startup-info-return-session [--heap-set-information-return-session [--heap-create-return-session [--get-module-handle-w-return-session]]]]]]]]] [--large-image-reflection] [--install-callback-return-sentinel [--install-process-entry-return-sentinel [--install-controlled-initial-stack [--install-controlled-process-environment [--install-controlled-process-parameters [--controlled-command-line TEXT]]]]]]] | --output FILE] INPUT".to_owned(),
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

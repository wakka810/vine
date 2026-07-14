use std::mem::size_of;

use encoding_rs::{EncoderResult, SHIFT_JIS};

use crate::api::{
    AbiSchema, ApiArgumentSchema, ApiBehavior, ApiCallFrame, ApiContract, ApiDirection,
    ApiDynamicPointerConstraint, ApiEndpointIdentity, ApiFault, ApiHandlerResult, ApiImmediate,
    ApiPointeeSchema, ApiPointerConstraint, ApiReturnSchema, ApiReturnValue, ApiStringEncoding,
    ApiSymbol, ApiValueSchema, ApiWriteRequirement, CallbackPolicy, CallingConvention,
    ExecutionSession, LastErrorPolicy, ReentrancyPolicy, StackCleanup,
};
use crate::dynamic_exports::DYNAMIC_EXPORT_ENDPOINT_NAMESPACE;
use crate::memory::{GuestAddress, MemoryAccess};
use crate::runtime::{
    LOGICAL_ENCODED_POINTER_NAMESPACE, LOGICAL_LOADED_MODULE_NAMESPACE,
    LOGICAL_PRIVATE_HEAP_NAMESPACE, LOGICAL_STANDARD_HANDLE_NAMESPACE,
    RUNTIME_PRIVATE_HEAP_ALIGNMENT, RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING, RuntimeCriticalSection,
    RuntimeHeapInitialization, X86_CRITICAL_SECTION_BYTE_LENGTH, X86_STARTUP_INFO_W_BYTE_LENGTH,
};

mod policy;

pub use policy::{
    KERNEL32_COMPATIBILITY_POLICY_SCHEMA, KERNEL32_COMPATIBILITY_POLICY_VERSION,
    Kernel32CodePageInfoPolicy, Kernel32CompatibilityPolicy, Kernel32CompatibilityPolicyError,
    Kernel32GetModuleFileNameAPolicy, Kernel32GetStringTypeWPolicy, Kernel32HeapAllocationPolicy,
    Kernel32InitializeCriticalSectionPolicy, Kernel32LcMapStringWPolicy,
    Kernel32MultiByteToWideCharPolicy, Kernel32ProcessorFeaturePolicy,
    Kernel32SetCurrentDirectoryAPolicy, Kernel32StandardHandleFileType,
    Kernel32WideCharToMultiBytePolicy,
};

pub const GET_SYSTEM_TIME_AS_FILE_TIME_IMPLEMENTATION: &str =
    "kernel32.get-system-time-as-file-time.v1";
pub const GET_CURRENT_PROCESS_ID_IMPLEMENTATION: &str = "kernel32.get-current-process-id.v1";
pub const GET_CURRENT_THREAD_ID_IMPLEMENTATION: &str = "kernel32.get-current-thread-id.v1";
pub const GET_LAST_ERROR_IMPLEMENTATION: &str = "kernel32.get-last-error.v1";
pub const SET_LAST_ERROR_IMPLEMENTATION: &str = "kernel32.set-last-error.v1";
pub const GET_ACP_IMPLEMENTATION: &str = "kernel32.get-acp.v1";
pub const GET_CP_INFO_IMPLEMENTATION: &str = "kernel32.get-cp-info.v1";
pub const IS_PROCESSOR_FEATURE_PRESENT_IMPLEMENTATION: &str =
    "kernel32.is-processor-feature-present.v1";
pub const SET_UNHANDLED_EXCEPTION_FILTER_IMPLEMENTATION: &str =
    "kernel32.set-unhandled-exception-filter.v1";
pub const GET_TICK_COUNT_IMPLEMENTATION: &str = "kernel32.get-tick-count.v1";
pub const GET_STD_HANDLE_IMPLEMENTATION: &str = "kernel32.get-std-handle.v2";
pub const GET_FILE_TYPE_IMPLEMENTATION: &str = "kernel32.get-file-type.v2";
pub const SET_HANDLE_COUNT_IMPLEMENTATION: &str = "kernel32.set-handle-count.v1";
pub const GET_COMMAND_LINE_A_IMPLEMENTATION: &str = "kernel32.get-command-line-a.v1";
pub const GET_MODULE_FILE_NAME_A_IMPLEMENTATION: &str = "kernel32.get-module-file-name-a.v1";
pub const GET_ENVIRONMENT_STRINGS_W_IMPLEMENTATION: &str = "kernel32.get-environment-strings-w.v1";
pub const FREE_ENVIRONMENT_STRINGS_W_IMPLEMENTATION: &str =
    "kernel32.free-environment-strings-w.v1";
pub const WIDE_CHAR_TO_MULTI_BYTE_IMPLEMENTATION: &str = "kernel32.wide-char-to-multi-byte.v1";
pub const MULTI_BYTE_TO_WIDE_CHAR_IMPLEMENTATION: &str = "kernel32.multi-byte-to-wide-char.v1";
pub const SET_CURRENT_DIRECTORY_A_IMPLEMENTATION: &str = "kernel32.set-current-directory-a.v1";
pub const GET_STRING_TYPE_W_IMPLEMENTATION: &str = "kernel32.get-string-type-w.v1";
pub const LC_MAP_STRING_W_IMPLEMENTATION: &str = "kernel32.lc-map-string-w.v1";
pub const QUERY_PERFORMANCE_COUNTER_IMPLEMENTATION: &str = "kernel32.query-performance-counter.v1";
pub const GET_STARTUP_INFO_W_IMPLEMENTATION: &str = "kernel32.get-startup-info-w.v1";
pub const HEAP_SET_INFORMATION_IMPLEMENTATION: &str = "kernel32.heap-set-information.v1";
pub const HEAP_CREATE_IMPLEMENTATION: &str = "kernel32.heap-create.v1";
pub const HEAP_ALLOC_IMPLEMENTATION: &str = "kernel32.heap-alloc.v2";
pub const HEAP_FREE_IMPLEMENTATION: &str = "kernel32.heap-free.v1";
pub const HEAP_SIZE_IMPLEMENTATION: &str = "kernel32.heap-size.v1";
pub const GET_MODULE_HANDLE_W_IMPLEMENTATION: &str = "kernel32.get-module-handle-w.v1";
pub const GET_PROC_ADDRESS_IMPLEMENTATION: &str = "kernel32.get-proc-address.v1";
pub const TLS_ALLOC_IMPLEMENTATION: &str = "kernel32.tls-alloc.v1";
pub const TLS_GET_VALUE_IMPLEMENTATION: &str = "kernel32.tls-get-value.v1";
pub const TLS_SET_VALUE_IMPLEMENTATION: &str = "kernel32.tls-set-value.v1";
pub const ENCODE_POINTER_IMPLEMENTATION: &str = "kernel32.encode-pointer.v1";
pub const DECODE_POINTER_IMPLEMENTATION: &str = "kernel32.decode-pointer.v1";
pub const FLS_ALLOC_IMPLEMENTATION: &str = "kernel32.fls-alloc.v1";
pub const FLS_GET_VALUE_IMPLEMENTATION: &str = "kernel32.fls-get-value.v1";
pub const FLS_SET_VALUE_IMPLEMENTATION: &str = "kernel32.fls-set-value.v1";
pub const INTERLOCKED_INCREMENT_IMPLEMENTATION: &str = "kernel32.interlocked-increment.v1";
pub const INTERLOCKED_DECREMENT_IMPLEMENTATION: &str = "kernel32.interlocked-decrement.v1";
pub const INITIALIZE_CRITICAL_SECTION_AND_SPIN_COUNT_IMPLEMENTATION: &str =
    "kernel32.initialize-critical-section-and-spin-count.v1";
pub const INITIALIZE_CRITICAL_SECTION_IMPLEMENTATION: &str =
    "kernel32.initialize-critical-section.v1";
pub const ENTER_CRITICAL_SECTION_IMPLEMENTATION: &str = "kernel32.enter-critical-section.v1";
pub const LEAVE_CRITICAL_SECTION_IMPLEMENTATION: &str = "kernel32.leave-critical-section.v1";

const HEAP_ENABLE_TERMINATION_ON_CORRUPTION: u32 = 1;
const HEAP_ZERO_MEMORY: u32 = 0x0000_0008;
const MAX_MODULE_NAME_CODE_UNITS: usize = 64;
const MAX_EXPORT_NAME_BYTES: usize = 64;
const CP_ACP: u32 = 0;
const WINDOWS_932_CODE_PAGE: u32 = 932;
const ERROR_SUCCESS: u32 = 0;
const ERROR_INVALID_PARAMETER: u32 = 87;
const ERROR_INSUFFICIENT_BUFFER: u32 = 122;
const ERROR_INVALID_NAME: u32 = 123;

pub fn dispatch_kernel32_api(
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> ApiHandlerResult {
    dispatch_kernel32_api_with_policy(
        &Kernel32CompatibilityPolicy::controlled_v1(),
        contract,
        frame,
        session,
    )
}

pub fn dispatch_kernel32_api_with_policy(
    policy: &Kernel32CompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> ApiHandlerResult {
    if let Err(error) = policy.validate() {
        return ApiHandlerResult::Fault(ApiFault::runtime_defect(
            "win32-compatibility-policy",
            error.code,
            error.message,
        ));
    }
    let result = match contract.implementation_id.as_str() {
        GET_SYSTEM_TIME_AS_FILE_TIME_IMPLEMENTATION => {
            handle_get_system_time_as_file_time(contract, frame, session)
        }
        GET_CURRENT_PROCESS_ID_IMPLEMENTATION => handle_get_current_process_id(session),
        GET_CURRENT_THREAD_ID_IMPLEMENTATION => handle_get_current_thread_id(session),
        GET_LAST_ERROR_IMPLEMENTATION => handle_get_last_error(contract, session),
        SET_LAST_ERROR_IMPLEMENTATION => handle_set_last_error(contract, frame),
        GET_ACP_IMPLEMENTATION => handle_get_acp(policy),
        GET_CP_INFO_IMPLEMENTATION => handle_get_cp_info(policy, contract, frame),
        IS_PROCESSOR_FEATURE_PRESENT_IMPLEMENTATION => {
            handle_is_processor_feature_present(policy, contract, frame)
        }
        SET_UNHANDLED_EXCEPTION_FILTER_IMPLEMENTATION => {
            handle_set_unhandled_exception_filter(contract, frame, session)
        }
        GET_TICK_COUNT_IMPLEMENTATION => handle_get_tick_count(session),
        GET_STD_HANDLE_IMPLEMENTATION => handle_get_std_handle(policy, contract, frame, session),
        GET_FILE_TYPE_IMPLEMENTATION => handle_get_file_type(policy, contract, frame, session),
        SET_HANDLE_COUNT_IMPLEMENTATION => handle_set_handle_count(policy, contract, frame),
        GET_COMMAND_LINE_A_IMPLEMENTATION => handle_get_command_line_a(contract, session),
        GET_MODULE_FILE_NAME_A_IMPLEMENTATION => {
            handle_get_module_file_name_a(policy, contract, frame, session)
        }
        GET_ENVIRONMENT_STRINGS_W_IMPLEMENTATION => {
            handle_get_environment_strings_w(contract, session)
        }
        FREE_ENVIRONMENT_STRINGS_W_IMPLEMENTATION => {
            handle_free_environment_strings_w(contract, frame, session)
        }
        WIDE_CHAR_TO_MULTI_BYTE_IMPLEMENTATION => {
            handle_wide_char_to_multi_byte(policy, contract, frame, session)
        }
        MULTI_BYTE_TO_WIDE_CHAR_IMPLEMENTATION => {
            handle_multi_byte_to_wide_char(policy, contract, frame, session)
        }
        SET_CURRENT_DIRECTORY_A_IMPLEMENTATION => {
            handle_set_current_directory_a(policy, contract, frame, session)
        }
        GET_STRING_TYPE_W_IMPLEMENTATION => {
            handle_get_string_type_w(policy, contract, frame, session)
        }
        LC_MAP_STRING_W_IMPLEMENTATION => handle_lc_map_string_w(policy, contract, frame, session),
        QUERY_PERFORMANCE_COUNTER_IMPLEMENTATION => {
            handle_query_performance_counter(contract, frame, session)
        }
        GET_STARTUP_INFO_W_IMPLEMENTATION => handle_get_startup_info_w(contract, frame, session),
        HEAP_SET_INFORMATION_IMPLEMENTATION => {
            handle_heap_set_information(contract, frame, session)
        }
        HEAP_CREATE_IMPLEMENTATION => handle_heap_create(contract, frame, session),
        HEAP_ALLOC_IMPLEMENTATION => handle_heap_alloc(policy, contract, frame, session),
        HEAP_FREE_IMPLEMENTATION => handle_heap_free(policy, contract, frame, session),
        HEAP_SIZE_IMPLEMENTATION => handle_heap_size(policy, contract, frame, session),
        GET_MODULE_HANDLE_W_IMPLEMENTATION => handle_get_module_handle_w(contract, frame, session),
        GET_PROC_ADDRESS_IMPLEMENTATION => handle_get_proc_address(contract, frame, session),
        TLS_ALLOC_IMPLEMENTATION => handle_tls_alloc(contract, session),
        TLS_GET_VALUE_IMPLEMENTATION => handle_tls_get_value(contract, frame, session),
        TLS_SET_VALUE_IMPLEMENTATION => handle_tls_set_value(contract, frame, session),
        ENCODE_POINTER_IMPLEMENTATION => handle_encode_pointer(contract, frame, session),
        DECODE_POINTER_IMPLEMENTATION => handle_decode_pointer(contract, frame, session),
        FLS_ALLOC_IMPLEMENTATION => handle_fls_alloc(contract, frame, session),
        FLS_GET_VALUE_IMPLEMENTATION => handle_fls_get_value(contract, frame, session),
        FLS_SET_VALUE_IMPLEMENTATION => handle_fls_set_value(contract, frame, session),
        INTERLOCKED_INCREMENT_IMPLEMENTATION => {
            handle_interlocked_increment(contract, frame, session)
        }
        INTERLOCKED_DECREMENT_IMPLEMENTATION => {
            handle_interlocked_decrement(contract, frame, session)
        }
        INITIALIZE_CRITICAL_SECTION_AND_SPIN_COUNT_IMPLEMENTATION => {
            handle_initialize_critical_section_and_spin_count(contract, frame, session)
        }
        INITIALIZE_CRITICAL_SECTION_IMPLEMENTATION => {
            handle_initialize_critical_section(policy, contract, frame, session)
        }
        ENTER_CRITICAL_SECTION_IMPLEMENTATION => {
            handle_enter_critical_section(contract, frame, session)
        }
        LEAVE_CRITICAL_SECTION_IMPLEMENTATION => {
            handle_leave_critical_section(contract, frame, session)
        }
        implementation_id => Err(ApiFault::unsupported(
            "win32-api-dispatch",
            "implementation-id-unsupported",
            format!("Win32 implementation {implementation_id} is not registered"),
        )),
    };
    match result {
        Ok(immediate) => ApiHandlerResult::Immediate(immediate),
        Err(fault) => ApiHandlerResult::Fault(fault),
    }
}

pub fn kernel32_contract_for_endpoint(
    endpoint: ApiEndpointIdentity,
) -> Result<Option<ApiContract>, ApiFault> {
    if !endpoint.module.eq_ignore_ascii_case("kernel32.dll") {
        return Ok(None);
    }
    let ApiSymbol::Name { name } = &endpoint.symbol else {
        return Ok(None);
    };
    let contract = match name.as_str() {
        "GetSystemTimeAsFileTime" => get_system_time_as_file_time_contract(endpoint)?,
        "GetCurrentProcessId" => get_current_process_id_contract(endpoint)?,
        "GetCurrentThreadId" => get_current_thread_id_contract(endpoint)?,
        "GetLastError" => get_last_error_contract(endpoint)?,
        "SetLastError" => set_last_error_contract(endpoint)?,
        "GetACP" => get_acp_contract(endpoint)?,
        "GetCPInfo" => get_cp_info_contract(endpoint)?,
        "IsProcessorFeaturePresent" => is_processor_feature_present_contract(endpoint)?,
        "SetUnhandledExceptionFilter" => set_unhandled_exception_filter_contract(endpoint)?,
        "GetTickCount" => get_tick_count_contract(endpoint)?,
        "GetStdHandle" => get_std_handle_contract(endpoint)?,
        "GetFileType" => get_file_type_contract(endpoint)?,
        "SetHandleCount" => set_handle_count_contract(endpoint)?,
        "GetCommandLineA" => get_command_line_a_contract(endpoint)?,
        "GetModuleFileNameA" => get_module_file_name_a_contract(endpoint)?,
        "GetEnvironmentStringsW" => get_environment_strings_w_contract(endpoint)?,
        "FreeEnvironmentStringsW" => free_environment_strings_w_contract(endpoint)?,
        "WideCharToMultiByte" => wide_char_to_multi_byte_contract(endpoint)?,
        "MultiByteToWideChar" => multi_byte_to_wide_char_contract(endpoint)?,
        "SetCurrentDirectoryA" => set_current_directory_a_contract(endpoint)?,
        "GetStringTypeW" => get_string_type_w_contract(endpoint)?,
        "LCMapStringW" => lc_map_string_w_contract(endpoint)?,
        "QueryPerformanceCounter" => query_performance_counter_contract(endpoint)?,
        "GetStartupInfoW" => get_startup_info_w_contract(endpoint)?,
        "HeapSetInformation" => heap_set_information_contract(endpoint)?,
        "HeapCreate" => heap_create_contract(endpoint)?,
        "HeapAlloc" => heap_alloc_contract(endpoint)?,
        "HeapFree" => heap_free_contract(endpoint)?,
        "HeapSize" => heap_size_contract(endpoint)?,
        "GetModuleHandleW" => get_module_handle_w_contract(endpoint)?,
        "GetProcAddress" => get_proc_address_contract(endpoint)?,
        "TlsAlloc" => tls_alloc_contract(endpoint)?,
        "TlsGetValue" => tls_get_value_contract(endpoint)?,
        "TlsSetValue" => tls_set_value_contract(endpoint)?,
        "EncodePointer" => encode_pointer_contract(endpoint)?,
        "DecodePointer" => decode_pointer_contract(endpoint)?,
        "FlsAlloc" => fls_alloc_contract(endpoint)?,
        "FlsGetValue" => fls_get_value_contract(endpoint)?,
        "FlsSetValue" => fls_set_value_contract(endpoint)?,
        "InterlockedIncrement" => interlocked_increment_contract(endpoint)?,
        "InterlockedDecrement" => interlocked_decrement_contract(endpoint)?,
        "InitializeCriticalSectionAndSpinCount" => {
            initialize_critical_section_and_spin_count_contract(endpoint)?
        }
        "InitializeCriticalSection" => initialize_critical_section_contract(endpoint)?,
        "EnterCriticalSection" => enter_critical_section_contract(endpoint)?,
        "LeaveCriticalSection" => leave_critical_section_contract(endpoint)?,
        _ => return Ok(None),
    };
    Ok(Some(contract))
}

fn handle_get_system_time_as_file_time(
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let output = required_argument(contract, frame, 0, "FILETIME output")?;
    Ok(ApiImmediate::new(ApiReturnValue::Void).with_guest_write(
        GuestAddress(output),
        session
            .runtime()
            .clock()
            .wall_clock_file_time
            .to_le_bytes()
            .to_vec(),
    ))
}

fn handle_get_current_process_id(session: &ExecutionSession<'_>) -> Result<ApiImmediate, ApiFault> {
    Ok(ApiImmediate::new(ApiReturnValue::U32(
        session.runtime().process_id().0,
    )))
}

fn handle_get_current_thread_id(session: &ExecutionSession<'_>) -> Result<ApiImmediate, ApiFault> {
    Ok(ApiImmediate::new(ApiReturnValue::U32(
        session.runtime().current_thread_id().0,
    )))
}

fn handle_get_last_error(
    contract: &ApiContract,
    session: &ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    Ok(ApiImmediate::new(ApiReturnValue::U32(
        session
            .last_error()
            .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?,
    )))
}

fn handle_set_last_error(
    contract: &ApiContract,
    frame: &ApiCallFrame,
) -> Result<ApiImmediate, ApiFault> {
    let error = required_argument(contract, frame, 0, "LastError value")?;
    Ok(ApiImmediate::new(ApiReturnValue::Void).with_last_error(error))
}

fn handle_get_acp(policy: &Kernel32CompatibilityPolicy) -> Result<ApiImmediate, ApiFault> {
    let active_code_page = policy.active_code_page().ok_or_else(|| {
        ApiFault::unsupported(
            "win32-code-page",
            "get-acp-policy-disabled",
            format!(
                "compatibility policy {} does not enable an active ANSI code page",
                policy.id
            ),
        )
    })?;
    Ok(ApiImmediate::new(ApiReturnValue::U32(active_code_page)))
}

fn handle_get_cp_info(
    policy: &Kernel32CompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
) -> Result<ApiImmediate, ApiFault> {
    let code_page = required_argument(contract, frame, 0, "code page")?;
    let output = required_argument(contract, frame, 1, "CPINFO output")?;
    let (active_code_page, info) = policy.active_code_page_info().ok_or_else(|| {
        ApiFault::unsupported(
            "win32-code-page",
            "get-cp-info-policy-disabled",
            format!(
                "compatibility policy {} does not enable active code-page information",
                policy.id
            ),
        )
    })?;
    if code_page != active_code_page {
        return Err(ApiFault::unsupported(
            "win32-code-page",
            "get-cp-info-code-page-unsupported",
            format!(
                "compatibility policy {} enables GetCPInfo only for active code page {active_code_page}, not {code_page}",
                policy.id
            ),
        ));
    }
    Ok(ApiImmediate::new(ApiReturnValue::Bool32(true))
        .with_guest_write(GuestAddress(output), x86_cp_info_bytes(info)))
}

fn x86_cp_info_bytes(info: &Kernel32CodePageInfoPolicy) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(18);
    bytes.extend_from_slice(&info.max_char_size.to_le_bytes());
    bytes.extend_from_slice(&info.default_char);
    bytes.extend_from_slice(&info.lead_byte);
    bytes
}

fn handle_is_processor_feature_present(
    policy: &Kernel32CompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
) -> Result<ApiImmediate, ApiFault> {
    let index = required_argument(contract, frame, 0, "processor feature index")?;
    let present = policy.processor_feature_presence(index).ok_or_else(|| {
        ApiFault::unsupported(
            "win32-processor-feature",
            "processor-feature-index-unsupported",
            format!(
                "compatibility policy {} does not define processor feature index {index}",
                policy.id
            ),
        )
    })?;
    Ok(ApiImmediate::new(ApiReturnValue::Bool32(present)))
}

fn handle_set_unhandled_exception_filter(
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let filter_value = required_argument(contract, frame, 0, "unhandled exception filter")?;
    let filter = (filter_value != 0).then_some(GuestAddress(filter_value));
    if let Some(address) = filter {
        session
            .memory()
            .validate_access(address, 1, MemoryAccess::Execute)
            .map_err(|error| ApiFault::from_memory(error, &contract.endpoint))?;
    }
    let previous = session
        .runtime_mut()
        .set_unhandled_exception_filter(filter)
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    Ok(ApiImmediate::new(ApiReturnValue::OpaquePointer(
        previous.map_or(0, |address| address.0),
    )))
}

fn handle_get_tick_count(session: &ExecutionSession<'_>) -> Result<ApiImmediate, ApiFault> {
    Ok(ApiImmediate::new(ApiReturnValue::U32(
        session.runtime().clock().tick_count,
    )))
}

fn handle_get_std_handle(
    policy: &Kernel32CompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let selector = required_argument(contract, frame, 0, "standard handle selector")?;
    if !policy.allows_standard_handle(selector) {
        return Err(ApiFault::unsupported(
            "win32-standard-handle",
            "standard-handle-selector-unsupported",
            format!(
                "compatibility policy {} does not enable selector {selector:#010x}",
                policy.id
            ),
        ));
    }
    let standard_handle = session
        .runtime_mut()
        .ensure_standard_handle(selector)
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?
        .clone();
    Ok(ApiImmediate::new(ApiReturnValue::LogicalHandle {
        namespace: standard_handle.namespace,
        value: standard_handle.handle,
    }))
}

fn handle_get_file_type(
    policy: &Kernel32CompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let handle = required_argument(contract, frame, 0, "standard handle")?;
    let standard_handle = session
        .runtime()
        .standard_handle_by_value(handle)
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    let file_type = policy
        .standard_handle_file_type(standard_handle.selector)
        .ok_or_else(|| {
            ApiFault::unsupported(
                "win32-file-type",
                "file-type-standard-handle-unsupported",
                format!(
                    "compatibility policy {} does not classify selector {:#010x}",
                    policy.id, standard_handle.selector
                ),
            )
        })?;
    Ok(ApiImmediate::new(ApiReturnValue::U32(file_type)))
}

fn handle_set_handle_count(
    policy: &Kernel32CompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
) -> Result<ApiImmediate, ApiFault> {
    let count = required_argument(contract, frame, 0, "handle count")?;
    if !policy.allows_set_handle_count(count) {
        return Err(ApiFault::unsupported(
            "win32-handle-table",
            "set-handle-count-unsupported",
            format!(
                "compatibility policy {} does not enable SetHandleCount({count})",
                policy.id
            ),
        ));
    }
    Ok(ApiImmediate::new(ApiReturnValue::U32(count)))
}

fn handle_get_command_line_a(
    contract: &ApiContract,
    session: &ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let command_line = session
        .runtime()
        .process_command_line()
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    let bytes = session
        .memory()
        .read(
            command_line.ansi_buffer_address,
            usize::try_from(command_line.ansi_byte_length_with_nul).map_err(|_| {
                ApiFault::runtime_defect(
                    "win32-command-line",
                    "ansi-command-line-length-not-representable",
                    "runtime ANSI command-line length does not fit host accounting",
                )
            })?,
        )
        .map_err(|error| ApiFault::from_memory(error, &contract.endpoint))?;
    if bytes.last() != Some(&0) || bytes[..bytes.len().saturating_sub(1)].contains(&0) {
        return Err(ApiFault::unsupported(
            "win32-command-line",
            "ansi-command-line-mutation-unsupported",
            "the process-owned ANSI command-line view no longer has its bounded NUL-terminated shape",
        ));
    }
    Ok(ApiImmediate::new(ApiReturnValue::OpaquePointer(
        command_line.ansi_buffer_address.0,
    )))
}

fn handle_get_module_file_name_a(
    policy: &Kernel32CompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let module_handle = required_argument(contract, frame, 0, "module handle")?;
    let output_address = required_argument(contract, frame, 1, "ANSI module file-name output")?;
    let capacity = required_argument(contract, frame, 2, "ANSI module file-name capacity")?;
    let file_name_policy = policy.get_module_file_name_a().ok_or_else(|| {
        ApiFault::unsupported(
            "win32-module-file-name",
            "get-module-file-name-a-policy-missing",
            format!(
                "compatibility policy {} does not enable GetModuleFileNameA",
                policy.id
            ),
        )
    })?;
    if module_handle != 0 || !file_name_policy.main_module_only {
        return Err(ApiFault::unsupported(
            "win32-module-file-name",
            "get-module-file-name-a-explicit-module-unsupported",
            "the selected GetModuleFileNameA policy enables only the current process image",
        ));
    }
    if capacity > file_name_policy.maximum_capacity_bytes {
        return Err(ApiFault::resource_limit(
            "win32-module-file-name",
            "get-module-file-name-a-capacity-limit-exceeded",
            format!(
                "ANSI module file-name capacity {capacity} exceeds the policy limit of {} bytes",
                file_name_policy.maximum_capacity_bytes
            ),
        ));
    }
    if capacity == 0 {
        return Ok(
            ApiImmediate::new(ApiReturnValue::U32(0)).with_last_error(ERROR_INSUFFICIENT_BUFFER)
        );
    }
    if output_address == 0 {
        return Ok(
            ApiImmediate::new(ApiReturnValue::U32(0)).with_last_error(ERROR_INVALID_PARAMETER)
        );
    }

    let image_path = session.runtime().process_image_path_a();
    if image_path.encoding != RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING
        || image_path.bytes_with_nul.len() < 2
        || image_path.bytes_with_nul.last() != Some(&0)
        || image_path.bytes_with_nul[..image_path.bytes_with_nul.len() - 1].contains(&0)
    {
        return Err(ApiFault::runtime_defect(
            "win32-module-file-name",
            "process-image-path-a-invalid",
            "runtime process image path lost its bounded Windows-932 NUL-terminated shape",
        ));
    }
    let capacity = usize::try_from(capacity).map_err(|_| {
        ApiFault::runtime_defect(
            "win32-module-file-name",
            "get-module-file-name-a-capacity-not-representable",
            "validated ANSI module file-name capacity does not fit host accounting",
        )
    })?;
    let output = GuestAddress(output_address);
    session
        .memory()
        .validate_access(output, capacity, MemoryAccess::Write)
        .map_err(|error| ApiFault::from_memory(error, &contract.endpoint))?;
    let path_length = image_path.bytes_with_nul.len() - 1;
    if capacity <= path_length {
        let mut truncated = image_path.bytes_with_nul[..capacity].to_vec();
        *truncated
            .last_mut()
            .expect("positive GetModuleFileNameA capacity") = 0;
        return Ok(ApiImmediate::new(ApiReturnValue::U32(capacity as u32))
            .with_last_error(ERROR_INSUFFICIENT_BUFFER)
            .with_guest_write(output, truncated));
    }
    Ok(ApiImmediate::new(ApiReturnValue::U32(path_length as u32))
        .with_last_error(ERROR_SUCCESS)
        .with_guest_write(output, image_path.bytes_with_nul.clone()))
}

fn handle_get_environment_strings_w(
    contract: &ApiContract,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let environment = session
        .runtime()
        .process_environment_strings()
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?
        .clone();
    let byte_length = usize::try_from(environment.byte_length_with_double_nul).map_err(|_| {
        ApiFault::runtime_defect(
            "win32-environment-strings",
            "wide-environment-length-not-representable",
            "runtime wide environment length does not fit host accounting",
        )
    })?;
    let authority = session
        .memory()
        .read(environment.authority_buffer_address, byte_length)
        .map_err(|error| ApiFault::from_memory(error, &contract.endpoint))?;
    let copy = session
        .memory()
        .read(environment.copy_buffer_address, byte_length)
        .map_err(|error| ApiFault::from_memory(error, &contract.endpoint))?;
    if authority != [0, 0, 0, 0] || copy != authority {
        return Err(ApiFault::unsupported(
            "win32-environment-strings",
            "wide-environment-mutation-unsupported",
            "the controlled wide environment authority or copy no longer has its empty double-NUL shape",
        ));
    }
    let copy = session
        .runtime_mut()
        .acquire_environment_strings_w()
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    Ok(ApiImmediate::new(ApiReturnValue::OpaquePointer(
        copy.buffer_address.0,
    )))
}

fn handle_free_environment_strings_w(
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let buffer_address = GuestAddress(required_argument(
        contract,
        frame,
        0,
        "wide environment-string copy",
    )?);
    session
        .runtime_mut()
        .free_environment_strings_w(buffer_address)
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    Ok(ApiImmediate::new(ApiReturnValue::Bool32(true)))
}

#[derive(Debug)]
struct Windows932Unit {
    bytes: Vec<u8>,
    used_default: bool,
}

fn handle_wide_char_to_multi_byte(
    policy: &Kernel32CompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let code_page = required_argument(contract, frame, 0, "code page")?;
    let flags = required_argument(contract, frame, 1, "conversion flags")?;
    let source_address = required_argument(contract, frame, 2, "UTF-16 source")?;
    let source_count = required_argument(contract, frame, 3, "UTF-16 source length")? as i32;
    let destination_address = required_argument(contract, frame, 4, "byte destination")?;
    let destination_bytes = required_argument(contract, frame, 5, "byte destination length")?;
    let default_char = required_argument(contract, frame, 6, "default character")?;
    let used_default_char = required_argument(contract, frame, 7, "used-default output")?;
    let conversion = policy.wide_char_to_multi_byte().ok_or_else(|| {
        ApiFault::unsupported(
            "win32-code-page",
            "wide-char-to-multi-byte-policy-disabled",
            format!(
                "compatibility policy {} does not enable WideCharToMultiByte",
                policy.id
            ),
        )
    })?;
    let effective_code_page = if code_page == CP_ACP {
        conversion.active_code_page
    } else {
        code_page
    };
    if effective_code_page != WINDOWS_932_CODE_PAGE || flags != 0 {
        return Err(ApiFault::unsupported(
            "win32-code-page",
            "wide-char-to-multi-byte-variant-unsupported",
            format!(
                "compatibility policy {} enables CP_ACP/Windows-932 with flags 0, not code page {code_page} and flags {flags:#010x}",
                policy.id
            ),
        ));
    }
    if default_char != 0 {
        return Err(ApiFault::unsupported(
            "win32-code-page",
            "wide-char-explicit-default-unsupported",
            "the Windows-932 handler currently uses only the system default character",
        ));
    }

    let preserved_last_error = session
        .last_error()
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    if source_address == 0 || source_count == 0 || source_count < -1 {
        return Ok(wide_char_failure(ERROR_INVALID_PARAMETER));
    }
    let query = destination_address == 0 && destination_bytes == 0;
    let conversion_call = destination_address != 0 && destination_bytes != 0;
    if !query && !conversion_call {
        return Ok(wide_char_failure(ERROR_INVALID_PARAMETER));
    }
    if used_default_char != 0 {
        session
            .memory()
            .validate_access(GuestAddress(used_default_char), 4, MemoryAccess::Write)
            .map_err(|error| ApiFault::from_memory(error, &contract.endpoint))?;
    }

    let source_units = read_wide_char_source(
        contract,
        session,
        GuestAddress(source_address),
        source_count,
        conversion,
    )?;
    let source_byte_length = source_units.len().checked_mul(2).ok_or_else(|| {
        ApiFault::resource_limit(
            "win32-code-page",
            "wide-char-source-length-overflow",
            "UTF-16 source length overflowed host accounting",
        )
    })?;
    if conversion_call
        && guest_ranges_overlap(
            source_address,
            source_byte_length,
            destination_address,
            destination_bytes as usize,
        )?
    {
        return Ok(wide_char_failure(ERROR_INVALID_PARAMETER));
    }

    let encoded = encode_windows_932(&source_units)?;
    let encoded_length = encoded.iter().try_fold(0_usize, |length, unit| {
        length.checked_add(unit.bytes.len()).ok_or_else(|| {
            ApiFault::resource_limit(
                "win32-code-page",
                "wide-char-output-length-overflow",
                "Windows-932 output length overflowed host accounting",
            )
        })
    })?;
    let encoded_length_u32 = u32::try_from(encoded_length).map_err(|_| {
        ApiFault::resource_limit(
            "win32-code-page",
            "wide-char-output-length-not-representable",
            "Windows-932 output length does not fit the x86 return value",
        )
    })?;
    let used_default = encoded.iter().any(|unit| unit.used_default);

    if query {
        let mut immediate = ApiImmediate::new(ApiReturnValue::U32(encoded_length_u32))
            .with_last_error(preserved_last_error);
        if used_default_char != 0 {
            immediate = immediate.with_guest_write(
                GuestAddress(used_default_char),
                u32::from(used_default).to_le_bytes().to_vec(),
            );
        }
        return Ok(immediate);
    }

    let capacity = destination_bytes as usize;
    let mut output = Vec::with_capacity(encoded_length.min(capacity));
    let mut prefix_used_default = false;
    for unit in &encoded {
        let Some(next_length) = output.len().checked_add(unit.bytes.len()) else {
            return Err(ApiFault::resource_limit(
                "win32-code-page",
                "wide-char-output-length-overflow",
                "Windows-932 prefix length overflowed host accounting",
            ));
        };
        if next_length > capacity {
            break;
        }
        output.extend_from_slice(&unit.bytes);
        prefix_used_default |= unit.used_default;
    }
    if !output.is_empty() {
        session
            .memory()
            .validate_access(
                GuestAddress(destination_address),
                output.len(),
                MemoryAccess::Write,
            )
            .map_err(|error| ApiFault::from_memory(error, &contract.endpoint))?;
    }
    let complete = output.len() == encoded_length;
    let mut immediate = ApiImmediate::new(ApiReturnValue::U32(if complete {
        encoded_length_u32
    } else {
        0
    }))
    .with_last_error(if complete {
        preserved_last_error
    } else {
        ERROR_INSUFFICIENT_BUFFER
    });
    if !output.is_empty() {
        immediate = immediate.with_guest_write(GuestAddress(destination_address), output);
    }
    if used_default_char != 0 {
        immediate = immediate.with_guest_write(
            GuestAddress(used_default_char),
            u32::from(if complete {
                used_default
            } else {
                prefix_used_default
            })
            .to_le_bytes()
            .to_vec(),
        );
    }
    Ok(immediate)
}

fn read_wide_char_source(
    contract: &ApiContract,
    session: &ExecutionSession<'_>,
    source: GuestAddress,
    source_count: i32,
    policy: &Kernel32WideCharToMultiBytePolicy,
) -> Result<Vec<u16>, ApiFault> {
    let maximum = policy.maximum_source_code_units as usize;
    let unit_count = if source_count == -1 {
        let mut units = Vec::new();
        for offset in 0..maximum {
            let address = add_guest_offset(
                contract,
                source,
                offset.checked_mul(2).ok_or_else(|| {
                    ApiFault::resource_limit(
                        "win32-code-page",
                        "wide-char-source-offset-overflow",
                        "UTF-16 source offset overflowed host accounting",
                    )
                })?,
            )?;
            let bytes = session
                .memory()
                .read(address, 2)
                .map_err(|error| ApiFault::from_memory(error, &contract.endpoint))?;
            let unit = u16::from_le_bytes([bytes[0], bytes[1]]);
            units.push(unit);
            if unit == 0 {
                return Ok(units);
            }
        }
        return Err(ApiFault::resource_limit(
            "win32-code-page",
            "wide-char-source-not-terminated",
            format!(
                "UTF-16 source did not terminate within the policy limit of {} code units",
                policy.maximum_source_code_units
            ),
        ));
    } else {
        usize::try_from(source_count).map_err(|_| {
            ApiFault::runtime_defect(
                "win32-code-page",
                "wide-char-source-count-not-representable",
                "validated positive UTF-16 source count does not fit host accounting",
            )
        })?
    };
    if unit_count > maximum {
        return Err(ApiFault::resource_limit(
            "win32-code-page",
            "wide-char-source-limit-exceeded",
            format!(
                "UTF-16 source count {unit_count} exceeds the policy limit of {}",
                policy.maximum_source_code_units
            ),
        ));
    }
    let byte_length = unit_count.checked_mul(2).ok_or_else(|| {
        ApiFault::resource_limit(
            "win32-code-page",
            "wide-char-source-length-overflow",
            "UTF-16 source length overflowed host accounting",
        )
    })?;
    session
        .memory()
        .validate_access(source, byte_length, MemoryAccess::Read)
        .map_err(|error| ApiFault::from_memory(error, &contract.endpoint))?;
    let bytes = session
        .memory()
        .read(source, byte_length)
        .map_err(|error| ApiFault::from_memory(error, &contract.endpoint))?;
    Ok(bytes
        .chunks_exact(2)
        .map(|unit| u16::from_le_bytes([unit[0], unit[1]]))
        .collect())
}

fn encode_windows_932(source: &[u16]) -> Result<Vec<Windows932Unit>, ApiFault> {
    let mut output = Vec::with_capacity(source.len());
    for &unit in source {
        if (0xd800..=0xdfff).contains(&unit) {
            output.push(Windows932Unit {
                bytes: vec![b'?'],
                used_default: true,
            });
            continue;
        }
        let mut encoder = SHIFT_JIS.new_encoder();
        let mut bytes = [0_u8; 4];
        let (result, read, written) =
            encoder.encode_from_utf16_without_replacement(&[unit], &mut bytes, true);
        match result {
            EncoderResult::InputEmpty if read == 1 => output.push(Windows932Unit {
                bytes: bytes[..written].to_vec(),
                used_default: false,
            }),
            EncoderResult::Unmappable(_) if read == 1 => output.push(Windows932Unit {
                bytes: vec![b'?'],
                used_default: true,
            }),
            EncoderResult::OutputFull => {
                return Err(ApiFault::runtime_defect(
                    "win32-code-page",
                    "windows-932-unit-buffer-too-small",
                    "Windows-932 encoder exceeded its bounded per-unit output buffer",
                ));
            }
            _ => {
                return Err(ApiFault::runtime_defect(
                    "win32-code-page",
                    "windows-932-encoder-progress-invalid",
                    "Windows-932 encoder returned an invalid per-unit progress result",
                ));
            }
        }
    }
    Ok(output)
}

fn handle_multi_byte_to_wide_char(
    policy: &Kernel32CompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let code_page = required_argument(contract, frame, 0, "code page")?;
    let flags = required_argument(contract, frame, 1, "conversion flags")?;
    let source_address = required_argument(contract, frame, 2, "multibyte source")?;
    let source_count = required_argument(contract, frame, 3, "multibyte source length")? as i32;
    let destination_address = required_argument(contract, frame, 4, "UTF-16 destination")?;
    let destination_units =
        required_argument(contract, frame, 5, "UTF-16 destination length")? as i32;
    let conversion = policy.multi_byte_to_wide_char().ok_or_else(|| {
        ApiFault::unsupported(
            "win32-code-page",
            "multi-byte-to-wide-char-policy-disabled",
            format!(
                "compatibility policy {} does not enable MultiByteToWideChar",
                policy.id
            ),
        )
    })?;
    let active_code_page = policy.active_code_page().ok_or_else(|| {
        ApiFault::runtime_defect(
            "win32-code-page",
            "multi-byte-to-wide-char-policy-layer-invalid",
            "validated multibyte conversion policy has no active code page",
        )
    })?;
    let effective_code_page = if code_page == CP_ACP && conversion.allow_active_code_page_selector {
        active_code_page
    } else {
        code_page
    };
    if effective_code_page != active_code_page || !conversion.allowed_flags.contains(&flags) {
        return Err(ApiFault::unsupported(
            "win32-code-page",
            "multi-byte-to-wide-char-variant-unsupported",
            format!(
                "compatibility policy {} enables explicit code page {active_code_page}{} with flags {:?}, not code page {code_page} and flags {flags:#010x}",
                policy.id,
                if conversion.allow_active_code_page_selector {
                    " or CP_ACP"
                } else {
                    ""
                },
                conversion.allowed_flags
            ),
        ));
    }

    let preserved_last_error = session
        .last_error()
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    if source_address == 0 || source_count == 0 || source_count < -1 || destination_units < 0 {
        return Ok(multi_byte_failure(ERROR_INVALID_PARAMETER));
    }
    let query = destination_address == 0 && destination_units == 0;
    let conversion_call = destination_address != 0 && destination_units > 0;
    if !query && !conversion_call {
        return Ok(multi_byte_failure(ERROR_INVALID_PARAMETER));
    }

    let source = read_multi_byte_source(
        contract,
        session,
        GuestAddress(source_address),
        source_count,
        conversion,
    )?;
    let destination_byte_capacity = usize::try_from(destination_units)
        .map_err(|_| {
            ApiFault::runtime_defect(
                "win32-code-page",
                "multi-byte-destination-count-not-representable",
                "validated nonnegative UTF-16 destination count does not fit host accounting",
            )
        })?
        .checked_mul(2)
        .ok_or_else(|| {
            ApiFault::resource_limit(
                "win32-code-page",
                "multi-byte-destination-length-overflow",
                "UTF-16 destination length overflowed host accounting",
            )
        })?;
    if conversion_call
        && guest_ranges_overlap(
            source_address,
            source.len(),
            destination_address,
            destination_byte_capacity,
        )?
    {
        return Ok(multi_byte_failure(ERROR_INVALID_PARAMETER));
    }

    let decoded = decode_windows_932(&source);
    let decoded_length = u32::try_from(decoded.len()).map_err(|_| {
        ApiFault::resource_limit(
            "win32-code-page",
            "multi-byte-output-length-not-representable",
            "Windows-932 UTF-16 output length does not fit the x86 return value",
        )
    })?;
    if query {
        return Ok(ApiImmediate::new(ApiReturnValue::U32(decoded_length))
            .with_last_error(preserved_last_error));
    }

    let capacity = usize::try_from(destination_units).map_err(|_| {
        ApiFault::runtime_defect(
            "win32-code-page",
            "multi-byte-destination-count-not-representable",
            "validated positive UTF-16 destination count does not fit host accounting",
        )
    })?;
    let prefix = &decoded[..decoded.len().min(capacity)];
    let mut output = Vec::with_capacity(prefix.len() * 2);
    for &unit in prefix {
        output.extend_from_slice(&unit.to_le_bytes());
    }
    if !output.is_empty() {
        session
            .memory()
            .validate_access(
                GuestAddress(destination_address),
                output.len(),
                MemoryAccess::Write,
            )
            .map_err(|error| ApiFault::from_memory(error, &contract.endpoint))?;
    }
    let complete = prefix.len() == decoded.len();
    let mut immediate = ApiImmediate::new(ApiReturnValue::U32(if complete {
        decoded_length
    } else {
        0
    }))
    .with_last_error(if complete {
        preserved_last_error
    } else {
        ERROR_INSUFFICIENT_BUFFER
    });
    if !output.is_empty() {
        immediate = immediate.with_guest_write(GuestAddress(destination_address), output);
    }
    Ok(immediate)
}

fn handle_set_current_directory_a(
    policy: &Kernel32CompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let directory_policy = policy.set_current_directory_a().ok_or_else(|| {
        ApiFault::unsupported(
            "win32-process-directory",
            "set-current-directory-a-policy-disabled",
            format!(
                "compatibility policy {} does not enable SetCurrentDirectoryA",
                policy.id
            ),
        )
    })?;
    let path_address = required_argument(contract, frame, 0, "ANSI directory path")?;
    let preserved_last_error = session
        .last_error()
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    if path_address == 0 {
        return Ok(ApiImmediate::new(ApiReturnValue::Bool32(false))
            .with_last_error(ERROR_INVALID_PARAMETER));
    }
    let path = read_nul_terminated_ansi_path(
        contract,
        session,
        GuestAddress(path_address),
        directory_policy,
    )?;
    if path == [0] {
        return Ok(
            ApiImmediate::new(ApiReturnValue::Bool32(false)).with_last_error(ERROR_INVALID_NAME)
        );
    }
    let is_process_image_directory = session
        .runtime()
        .is_process_image_directory_a(&path)
        .map_err(|error| {
            ApiFault::unsupported(
                "win32-process-directory",
                "set-current-directory-a-path-unsupported",
                format!(
                    "compatibility policy {} enables only the normalized process image directory: {}",
                    policy.id, error.message
                ),
            )
        })?;
    if !directory_policy.process_image_directory_only || !is_process_image_directory {
        return Err(ApiFault::unsupported(
            "win32-process-directory",
            "set-current-directory-a-path-unsupported",
            format!(
                "compatibility policy {} enables only the normalized process image directory",
                policy.id
            ),
        ));
    }
    let plan = session
        .runtime()
        .plan_set_current_directory_a(&path)
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    session
        .runtime_mut()
        .commit_set_current_directory_a(plan)
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    Ok(ApiImmediate::new(ApiReturnValue::Bool32(true)).with_last_error(preserved_last_error))
}

fn read_nul_terminated_ansi_path(
    contract: &ApiContract,
    session: &ExecutionSession<'_>,
    source: GuestAddress,
    policy: Kernel32SetCurrentDirectoryAPolicy,
) -> Result<Vec<u8>, ApiFault> {
    let maximum = usize::try_from(policy.maximum_path_bytes).map_err(|_| {
        ApiFault::runtime_defect(
            "win32-process-directory",
            "set-current-directory-a-limit-not-representable",
            "validated ANSI directory path limit does not fit host accounting",
        )
    })?;
    let mut bytes = Vec::new();
    for offset in 0..maximum {
        let address = add_guest_offset(contract, source, offset)?;
        let byte = session
            .memory()
            .read(address, 1)
            .map_err(|error| ApiFault::from_memory(error, &contract.endpoint))?[0];
        bytes.push(byte);
        if byte == 0 {
            return Ok(bytes);
        }
    }
    Err(ApiFault::resource_limit(
        "win32-process-directory",
        "set-current-directory-a-path-not-terminated",
        format!(
            "ANSI directory path did not terminate within the policy limit of {} bytes",
            policy.maximum_path_bytes
        ),
    ))
}

fn handle_get_string_type_w(
    policy: &Kernel32CompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let info_type = required_argument(contract, frame, 0, "character information type")?;
    let source_address = required_argument(contract, frame, 1, "UTF-16 source")?;
    let source_count = required_argument(contract, frame, 2, "UTF-16 source length")? as i32;
    let output_address = required_argument(contract, frame, 3, "character-type output")?;
    let classification = policy.get_string_type_w().ok_or_else(|| {
        ApiFault::unsupported(
            "win32-string-classification",
            "get-string-type-w-policy-disabled",
            format!(
                "compatibility policy {} does not enable GetStringTypeW",
                policy.id
            ),
        )
    })?;
    if !classification.allowed_info_types.contains(&info_type) {
        return Err(ApiFault::unsupported(
            "win32-string-classification",
            "get-string-type-w-info-type-unsupported",
            format!(
                "compatibility policy {} enables information types {:?}, not {info_type:#010x}",
                policy.id, classification.allowed_info_types
            ),
        ));
    }

    let preserved_last_error = session
        .last_error()
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    if source_count == 0 {
        return Ok(get_string_type_failure(ERROR_INVALID_PARAMETER));
    }
    if source_count < 0 {
        return Err(ApiFault::unsupported(
            "win32-string-classification",
            "get-string-type-w-negative-count-unsupported",
            "the selected GetStringTypeW policy enables only explicit positive source lengths",
        ));
    }
    let source_units = usize::try_from(source_count).map_err(|_| {
        ApiFault::runtime_defect(
            "win32-string-classification",
            "get-string-type-w-count-not-representable",
            "validated positive UTF-16 source count does not fit host accounting",
        )
    })?;
    if source_units > classification.maximum_source_code_units as usize {
        return Err(ApiFault::resource_limit(
            "win32-string-classification",
            "get-string-type-w-source-limit-exceeded",
            format!(
                "UTF-16 source count {source_units} exceeds the policy limit of {}",
                classification.maximum_source_code_units
            ),
        ));
    }
    let byte_length = source_units.checked_mul(2).ok_or_else(|| {
        ApiFault::resource_limit(
            "win32-string-classification",
            "get-string-type-w-byte-length-overflow",
            "UTF-16 classification length overflowed host accounting",
        )
    })?;
    let source = GuestAddress(source_address);
    let output = GuestAddress(output_address);
    session
        .memory()
        .validate_access(source, byte_length, MemoryAccess::Read)
        .map_err(|error| ApiFault::from_memory(error, &contract.endpoint))?;
    session
        .memory()
        .validate_access(output, byte_length, MemoryAccess::Write)
        .map_err(|error| ApiFault::from_memory(error, &contract.endpoint))?;
    if guest_ranges_overlap(source_address, byte_length, output_address, byte_length)? {
        return Err(ApiFault::unsupported(
            "win32-string-classification",
            "get-string-type-w-overlap-unsupported",
            "the selected GetStringTypeW policy does not enable overlapping input and output",
        ));
    }

    let source_bytes = session
        .memory()
        .read(source, byte_length)
        .map_err(|error| ApiFault::from_memory(error, &contract.endpoint))?;
    let mut output_bytes = Vec::with_capacity(byte_length);
    for (index, bytes) in source_bytes.chunks_exact(2).enumerate() {
        let unit = u16::from_le_bytes([bytes[0], bytes[1]]);
        let character_type = classify_windows_ctype1(unit).ok_or_else(|| {
            ApiFault::unsupported(
                "win32-string-classification",
                "get-string-type-w-code-unit-unsupported",
                format!(
                    "the selected CT_CTYPE1 classifier does not cover UTF-16 unit {unit:#06x} at index {index}"
                ),
            )
        })?;
        output_bytes.extend_from_slice(&character_type.to_le_bytes());
    }
    Ok(ApiImmediate::new(ApiReturnValue::Bool32(true))
        .with_last_error(preserved_last_error)
        .with_guest_write(output, output_bytes))
}

fn classify_windows_ctype1(unit: u16) -> Option<u16> {
    const C1_UPPER: u16 = 0x0001;
    const C1_LOWER: u16 = 0x0002;
    const C1_DIGIT: u16 = 0x0004;
    const C1_SPACE: u16 = 0x0008;
    const C1_PUNCT: u16 = 0x0010;
    const C1_CNTRL: u16 = 0x0020;
    const C1_BLANK: u16 = 0x0040;
    const C1_XDIGIT: u16 = 0x0080;
    const C1_ALPHA: u16 = 0x0100;
    const C1_DEFINED: u16 = 0x0200;

    if matches!(unit, 0x0000..=0x001f | 0x007f..=0x009f) {
        let whitespace = if matches!(unit, 0x0009..=0x000d) {
            C1_SPACE
        } else {
            0
        };
        let blank = if unit == 0x0009 { C1_BLANK } else { 0 };
        return Some(C1_DEFINED | C1_CNTRL | whitespace | blank);
    }
    if matches!(unit, 0x0020 | 0x00a0 | 0x3000) {
        return Some(C1_DEFINED | C1_SPACE | C1_BLANK);
    }
    if matches!(unit, 0x0030..=0x0039 | 0xff10..=0xff19) {
        return Some(C1_DEFINED | C1_DIGIT | C1_XDIGIT);
    }
    if matches!(unit, 0x0041..=0x005a | 0xff21..=0xff3a) {
        let ascii_offset = if unit >= 0xff21 {
            unit - 0xff21
        } else {
            unit - 0x0041
        };
        let xdigit = if ascii_offset < 6 { C1_XDIGIT } else { 0 };
        return Some(C1_DEFINED | C1_ALPHA | C1_UPPER | xdigit);
    }
    if matches!(unit, 0x0061..=0x007a | 0xff41..=0xff5a) {
        let ascii_offset = if unit >= 0xff41 {
            unit - 0xff41
        } else {
            unit - 0x0061
        };
        let xdigit = if ascii_offset < 6 { C1_XDIGIT } else { 0 };
        return Some(C1_DEFINED | C1_ALPHA | C1_LOWER | xdigit);
    }
    if matches!(
        unit,
        0x0021..=0x002f
            | 0x003a..=0x0040
            | 0x005b..=0x0060
            | 0x007b..=0x007e
            | 0x00a5
            | 0x30fb
            | 0xff01..=0xff0f
            | 0xff1a..=0xff20
            | 0xff3b..=0xff40
            | 0xff5b..=0xff65
    ) {
        return Some(C1_DEFINED | C1_PUNCT);
    }
    if matches!(
        unit,
        0x3040..=0x309f
            | 0x30a0..=0x30fa
            | 0x30fc..=0x30ff
            | 0x3400..=0x4dbf
            | 0x4e00..=0x9fff
            | 0xff66..=0xff9f
    ) {
        return Some(C1_DEFINED | C1_ALPHA);
    }
    if matches!(unit, 0xf8f0..=0xf8f3) {
        return Some(C1_DEFINED);
    }
    None
}

fn get_string_type_failure(last_error: u32) -> ApiImmediate {
    ApiImmediate::new(ApiReturnValue::Bool32(false)).with_last_error(last_error)
}

fn handle_lc_map_string_w(
    policy: &Kernel32CompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let locale = required_argument(contract, frame, 0, "locale")?;
    let flags = required_argument(contract, frame, 1, "mapping flags")?;
    let source_address = required_argument(contract, frame, 2, "UTF-16 source")?;
    let source_count = required_argument(contract, frame, 3, "UTF-16 source length")? as i32;
    let destination_address = required_argument(contract, frame, 4, "UTF-16 destination")?;
    let destination_count =
        required_argument(contract, frame, 5, "UTF-16 destination capacity")? as i32;
    let mapping = policy.lc_map_string_w().ok_or_else(|| {
        ApiFault::unsupported(
            "win32-locale-mapping",
            "lc-map-string-w-policy-disabled",
            format!(
                "compatibility policy {} does not enable LCMapStringW",
                policy.id
            ),
        )
    })?;
    if !mapping.allowed_locales.contains(&locale) {
        return Err(ApiFault::unsupported(
            "win32-locale-mapping",
            "lc-map-string-w-locale-unsupported",
            format!(
                "compatibility policy {} enables locales {:?}, not {locale:#010x}",
                policy.id, mapping.allowed_locales
            ),
        ));
    }
    if !mapping.allowed_flags.contains(&flags) {
        return Err(ApiFault::unsupported(
            "win32-locale-mapping",
            "lc-map-string-w-flags-unsupported",
            format!(
                "compatibility policy {} enables mapping flags {:?}, not {flags:#010x}",
                policy.id, mapping.allowed_flags
            ),
        ));
    }

    let preserved_last_error = session
        .last_error()
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    if source_count == 0 {
        return Ok(lc_map_string_failure(ERROR_INVALID_PARAMETER));
    }
    if source_count < 0 {
        return Err(ApiFault::unsupported(
            "win32-locale-mapping",
            "lc-map-string-w-negative-source-count-unsupported",
            "the selected LCMapStringW policy enables only explicit positive source lengths",
        ));
    }
    if destination_count < 0 {
        return Ok(lc_map_string_failure(ERROR_INVALID_PARAMETER));
    }
    let source_units = usize::try_from(source_count).map_err(|_| {
        ApiFault::runtime_defect(
            "win32-locale-mapping",
            "lc-map-string-w-source-count-not-representable",
            "validated positive UTF-16 source count does not fit host accounting",
        )
    })?;
    if source_units > mapping.maximum_source_code_units as usize {
        return Err(ApiFault::resource_limit(
            "win32-locale-mapping",
            "lc-map-string-w-source-limit-exceeded",
            format!(
                "UTF-16 source count {source_units} exceeds the policy limit of {}",
                mapping.maximum_source_code_units
            ),
        ));
    }
    let byte_length = source_units.checked_mul(2).ok_or_else(|| {
        ApiFault::resource_limit(
            "win32-locale-mapping",
            "lc-map-string-w-byte-length-overflow",
            "UTF-16 mapping length overflowed host accounting",
        )
    })?;
    let source = GuestAddress(source_address);
    session
        .memory()
        .validate_access(source, byte_length, MemoryAccess::Read)
        .map_err(|error| ApiFault::from_memory(error, &contract.endpoint))?;
    let source_bytes = session
        .memory()
        .read(source, byte_length)
        .map_err(|error| ApiFault::from_memory(error, &contract.endpoint))?;
    let mut output_bytes = Vec::with_capacity(byte_length);
    for (index, bytes) in source_bytes.chunks_exact(2).enumerate() {
        let unit = u16::from_le_bytes([bytes[0], bytes[1]]);
        let mapped = map_windows_lc_string_unit(flags, unit).ok_or_else(|| {
            ApiFault::unsupported(
                "win32-locale-mapping",
                "lc-map-string-w-code-unit-unsupported",
                format!(
                    "the selected locale mapper does not cover UTF-16 unit {unit:#06x} at index {index} for flags {flags:#010x}"
                ),
            )
        })?;
        output_bytes.extend_from_slice(&mapped.to_le_bytes());
    }

    if destination_address == 0 && destination_count == 0 {
        return Ok(ApiImmediate::new(ApiReturnValue::U32(source_count as u32))
            .with_last_error(preserved_last_error));
    }
    if destination_address == 0 || destination_count == 0 {
        return Ok(lc_map_string_failure(ERROR_INVALID_PARAMETER));
    }
    let destination_units = usize::try_from(destination_count).map_err(|_| {
        ApiFault::runtime_defect(
            "win32-locale-mapping",
            "lc-map-string-w-destination-count-not-representable",
            "validated positive UTF-16 destination count does not fit host accounting",
        )
    })?;
    if destination_units < source_units {
        return Ok(lc_map_string_failure(ERROR_INSUFFICIENT_BUFFER));
    }
    let destination = GuestAddress(destination_address);
    session
        .memory()
        .validate_access(destination, byte_length, MemoryAccess::Write)
        .map_err(|error| ApiFault::from_memory(error, &contract.endpoint))?;
    if guest_ranges_overlap(
        source_address,
        byte_length,
        destination_address,
        byte_length,
    )? {
        return Err(ApiFault::unsupported(
            "win32-locale-mapping",
            "lc-map-string-w-overlap-unsupported",
            "the selected LCMapStringW policy does not enable overlapping input and output",
        ));
    }
    Ok(ApiImmediate::new(ApiReturnValue::U32(source_count as u32))
        .with_last_error(preserved_last_error)
        .with_guest_write(destination, output_bytes))
}

fn map_windows_lc_string_unit(flags: u32, unit: u16) -> Option<u16> {
    let mapped = match flags {
        0x0100 if matches!(unit, 0x0041..=0x005a | 0xff21..=0xff3a) => unit + 0x20,
        0x0200 if matches!(unit, 0x0061..=0x007a) => unit - 0x20,
        0x0200 if matches!(unit, 0xff41..=0xff5a) => return None,
        0x0100 | 0x0200 => unit,
        _ => return None,
    };
    classify_windows_ctype1(unit).map(|_| mapped)
}

fn lc_map_string_failure(last_error: u32) -> ApiImmediate {
    ApiImmediate::new(ApiReturnValue::U32(0)).with_last_error(last_error)
}

fn read_multi_byte_source(
    contract: &ApiContract,
    session: &ExecutionSession<'_>,
    source: GuestAddress,
    source_count: i32,
    policy: &Kernel32MultiByteToWideCharPolicy,
) -> Result<Vec<u8>, ApiFault> {
    let maximum = policy.maximum_source_bytes as usize;
    if source_count == -1 {
        let mut bytes = Vec::new();
        for offset in 0..maximum {
            let address = add_guest_offset(contract, source, offset)?;
            let byte = session
                .memory()
                .read(address, 1)
                .map_err(|error| ApiFault::from_memory(error, &contract.endpoint))?[0];
            bytes.push(byte);
            if byte == 0 {
                return Ok(bytes);
            }
        }
        return Err(ApiFault::resource_limit(
            "win32-code-page",
            "multi-byte-source-not-terminated",
            format!(
                "multibyte source did not terminate within the policy limit of {} bytes",
                policy.maximum_source_bytes
            ),
        ));
    }
    let byte_length = usize::try_from(source_count).map_err(|_| {
        ApiFault::runtime_defect(
            "win32-code-page",
            "multi-byte-source-count-not-representable",
            "validated positive multibyte source count does not fit host accounting",
        )
    })?;
    if byte_length > maximum {
        return Err(ApiFault::resource_limit(
            "win32-code-page",
            "multi-byte-source-limit-exceeded",
            format!(
                "multibyte source count {byte_length} exceeds the policy limit of {}",
                policy.maximum_source_bytes
            ),
        ));
    }
    session
        .memory()
        .validate_access(source, byte_length, MemoryAccess::Read)
        .map_err(|error| ApiFault::from_memory(error, &contract.endpoint))?;
    session
        .memory()
        .read(source, byte_length)
        .map_err(|error| ApiFault::from_memory(error, &contract.endpoint))
}

fn decode_windows_932(source: &[u8]) -> Vec<u16> {
    let mut output = Vec::with_capacity(source.len());
    let mut index = 0;
    while index < source.len() {
        let first = source[index];
        if is_windows_932_lead_byte(first)
            && source
                .get(index + 1)
                .is_some_and(|&second| is_windows_932_trail_byte(second))
        {
            let pair = &source[index..index + 2];
            let (decoded, _) = SHIFT_JIS.decode_without_bom_handling(pair);
            output.extend(decoded.encode_utf16());
            index += 2;
            continue;
        }
        match first {
            0xa0 => output.push(0xf8f0),
            0xfd => output.push(0xf8f1),
            0xfe => output.push(0xf8f2),
            0xff => output.push(0xf8f3),
            _ => {
                let (decoded, _) = SHIFT_JIS.decode_without_bom_handling(&source[index..index + 1]);
                output.extend(decoded.encode_utf16());
            }
        }
        index += 1;
    }
    output
}

fn is_windows_932_lead_byte(byte: u8) -> bool {
    matches!(byte, 0x81..=0x9f | 0xe0..=0xfc)
}

fn is_windows_932_trail_byte(byte: u8) -> bool {
    matches!(byte, 0x40..=0x7e | 0x80..=0xfc)
}

fn multi_byte_failure(last_error: u32) -> ApiImmediate {
    ApiImmediate::new(ApiReturnValue::U32(0)).with_last_error(last_error)
}

fn guest_ranges_overlap(
    left_address: u32,
    left_length: usize,
    right_address: u32,
    right_length: usize,
) -> Result<bool, ApiFault> {
    let left_end = u64::from(left_address)
        .checked_add(u64::try_from(left_length).map_err(|_| {
            ApiFault::resource_limit(
                "win32-code-page",
                "wide-char-range-length-not-representable",
                "guest range length does not fit x86 accounting",
            )
        })?)
        .ok_or_else(|| {
            ApiFault::resource_limit(
                "win32-code-page",
                "wide-char-range-overflow",
                "UTF-16 source range overflowed x86 accounting",
            )
        })?;
    let right_end = u64::from(right_address)
        .checked_add(u64::try_from(right_length).map_err(|_| {
            ApiFault::resource_limit(
                "win32-code-page",
                "wide-char-range-length-not-representable",
                "guest range length does not fit x86 accounting",
            )
        })?)
        .ok_or_else(|| {
            ApiFault::resource_limit(
                "win32-code-page",
                "wide-char-range-overflow",
                "byte destination range overflowed x86 accounting",
            )
        })?;
    if left_end > u64::from(u32::MAX) + 1 || right_end > u64::from(u32::MAX) + 1 {
        return Err(ApiFault::resource_limit(
            "win32-code-page",
            "wide-char-range-overflow",
            "conversion range exceeds the x86 address space",
        ));
    }
    Ok(u64::from(left_address) < right_end && u64::from(right_address) < left_end)
}

fn wide_char_failure(last_error: u32) -> ApiImmediate {
    ApiImmediate::new(ApiReturnValue::U32(0)).with_last_error(last_error)
}

fn handle_query_performance_counter(
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let output = required_argument(contract, frame, 0, "performance counter output")?;
    let counter = session.runtime().clock().performance_counter;
    Ok(ApiImmediate::new(ApiReturnValue::Bool32(true))
        .with_guest_write(GuestAddress(output), counter.to_le_bytes().to_vec()))
}

fn handle_get_startup_info_w(
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let output = required_argument(contract, frame, 0, "STARTUPINFOW output")?;
    Ok(ApiImmediate::new(ApiReturnValue::Void).with_guest_write(
        GuestAddress(output),
        session.runtime().startup_info().bytes.clone(),
    ))
}

fn handle_heap_set_information(
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let heap = required_argument(contract, frame, 0, "heap handle")?;
    let information_class = required_argument(contract, frame, 1, "heap information class")?;
    let information = required_argument(contract, frame, 2, "heap information")?;
    let information_length = required_argument(contract, frame, 3, "heap information length")?;
    if heap != 0
        || information_class != HEAP_ENABLE_TERMINATION_ON_CORRUPTION
        || information != 0
        || information_length != 0
    {
        return Err(ApiFault::unsupported(
            "win32-heap",
            "heap-set-information-variant-unsupported",
            "only process-wide HeapEnableTerminationOnCorruption is implemented",
        ));
    }
    session
        .runtime_mut()
        .set_heap_termination_on_corruption(true);
    Ok(ApiImmediate::new(ApiReturnValue::Bool32(true)))
}

fn handle_heap_create(
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let creation_flags = required_argument(contract, frame, 0, "heap creation flags")?;
    let initial_size = required_argument(contract, frame, 1, "heap initial size")?;
    let maximum_size = required_argument(contract, frame, 2, "heap maximum size")?;
    let heap = session
        .runtime_mut()
        .allocate_private_heap(creation_flags, initial_size, maximum_size)
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?
        .clone();
    Ok(ApiImmediate::new(ApiReturnValue::LogicalHandle {
        namespace: heap.namespace,
        value: heap.handle,
    }))
}

fn handle_heap_alloc(
    policy: &Kernel32CompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let heap = required_argument(contract, frame, 0, "heap handle")?;
    let flags = required_argument(contract, frame, 1, "heap allocation flags")?;
    let byte_length = required_argument(contract, frame, 2, "heap allocation byte length")?;
    if !policy.allows_heap_allocation(flags, byte_length) {
        return Err(ApiFault::unsupported(
            "win32-heap",
            "heap-alloc-variant-unsupported",
            format!(
                "compatibility policy {} does not enable HeapAlloc flags {flags:#010x} and size {byte_length:#x}",
                policy.id
            ),
        ));
    }
    let initialization = match flags {
        0 => RuntimeHeapInitialization::DeterministicUninitialized,
        HEAP_ZERO_MEMORY => RuntimeHeapInitialization::Zeroed,
        _ => {
            return Err(ApiFault::runtime_defect(
                "win32-heap",
                "validated-heap-flags-unimplemented",
                "validated HeapAlloc policy selected flags without semantic initialization",
            ));
        }
    };
    let allocation = session
        .allocate_private_heap_block(heap, byte_length, initialization)
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    Ok(ApiImmediate::new(ApiReturnValue::OpaquePointer(
        allocation.address.0,
    )))
}

fn handle_heap_free(
    policy: &Kernel32CompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let heap = required_argument(contract, frame, 0, "heap handle")?;
    let flags = required_argument(contract, frame, 1, "heap free flags")?;
    let allocation = required_argument(contract, frame, 2, "heap allocation")?;
    if !policy.allows_heap_free(flags) {
        return Err(ApiFault::unsupported(
            "win32-heap",
            "heap-free-variant-unsupported",
            format!(
                "compatibility policy {} does not enable HeapFree flags {flags:#010x}",
                policy.id
            ),
        ));
    }
    session
        .free_private_heap_block(heap, GuestAddress(allocation))
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    Ok(ApiImmediate::new(ApiReturnValue::Bool32(true)))
}

fn handle_heap_size(
    policy: &Kernel32CompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let heap = required_argument(contract, frame, 0, "heap handle")?;
    let flags = required_argument(contract, frame, 1, "heap size flags")?;
    let address = required_argument(contract, frame, 2, "heap allocation")?;
    if !policy.allows_heap_size(flags) {
        return Err(ApiFault::unsupported(
            "win32-heap",
            "heap-size-flags-unsupported",
            format!(
                "compatibility policy {} does not enable HeapSize flags {flags:#010x}",
                policy.id
            ),
        ));
    }
    let allocation = match session
        .runtime()
        .private_heap_allocation(heap, GuestAddress(address))
    {
        Ok(allocation) => allocation,
        Err(error) if error.code == "heap-allocation-pointer-invalid" => {
            return Ok(ApiImmediate::new(ApiReturnValue::U32(u32::MAX))
                .with_last_error(ERROR_INVALID_PARAMETER));
        }
        Err(error) => return Err(ApiFault::from_runtime(error, &contract.endpoint)),
    };
    let last_error = session
        .last_error()
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    Ok(ApiImmediate::new(ApiReturnValue::U32(allocation.byte_length)).with_last_error(last_error))
}

fn handle_get_module_handle_w(
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let name_address = required_argument(contract, frame, 0, "module name")?;
    let name = read_utf16_nul(
        contract,
        session,
        GuestAddress(name_address),
        MAX_MODULE_NAME_CODE_UNITS,
    )?;
    let module = session
        .runtime()
        .module_by_name(&name)
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    Ok(ApiImmediate::new(ApiReturnValue::LogicalHandle {
        namespace: module.namespace.clone(),
        value: module.handle,
    }))
}

fn handle_get_proc_address(
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let module_handle = required_argument(contract, frame, 0, "module handle")?;
    let name_address = required_argument(contract, frame, 1, "procedure name")?;
    if name_address <= u32::from(u16::MAX) {
        return Err(ApiFault::unsupported(
            "win32-export",
            "get-proc-address-ordinal-unsupported",
            "ordinal export lookup is not implemented",
        ));
    }
    let name = read_ascii_nul(
        contract,
        session,
        GuestAddress(name_address),
        MAX_EXPORT_NAME_BYTES,
    )?;
    let module = session
        .runtime()
        .module(module_handle)
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    let endpoint = session
        .runtime()
        .dynamic_endpoint_by_name(&module.canonical_name, &name)
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    Ok(ApiImmediate::new(ApiReturnValue::LogicalPointer {
        namespace: endpoint.namespace.clone(),
        value: endpoint.address.0,
    }))
}

fn handle_tls_alloc(
    contract: &ApiContract,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let index = session
        .runtime_mut()
        .allocate_tls_index()
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    Ok(ApiImmediate::new(ApiReturnValue::U32(index)))
}

fn handle_tls_get_value(
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let index = required_argument(contract, frame, 0, "TLS index")?;
    match session.runtime().tls_value_or_unset(index) {
        Ok(value) => Ok(ApiImmediate::new(ApiReturnValue::OpaquePointer(
            value.map_or(0, |value| value.raw_value()),
        ))
        .with_last_error(ERROR_SUCCESS)),
        Err(error) if error.code == "tls-index-unallocated" => {
            Ok(ApiImmediate::new(ApiReturnValue::OpaquePointer(0))
                .with_last_error(ERROR_INVALID_PARAMETER))
        }
        Err(error) => Err(ApiFault::from_runtime(error, &contract.endpoint)),
    }
}

fn handle_tls_set_value(
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let index = required_argument(contract, frame, 0, "TLS index")?;
    let value = required_argument(contract, frame, 1, "TLS value")?;
    session
        .runtime_mut()
        .set_tls_value(index, value)
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    Ok(ApiImmediate::new(ApiReturnValue::Bool32(true)))
}

fn handle_encode_pointer(
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let source = required_argument(contract, frame, 0, "pointer source")?;
    let encoded = session
        .runtime_mut()
        .encode_pointer(source)
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?
        .clone();
    Ok(ApiImmediate::new(ApiReturnValue::LogicalPointer {
        namespace: encoded.namespace,
        value: encoded.token,
    }))
}

fn handle_decode_pointer(
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let token = required_argument(contract, frame, 0, "encoded pointer")?;
    let source = session
        .runtime()
        .decode_pointer(token)
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?
        .source;
    Ok(ApiImmediate::new(ApiReturnValue::OpaquePointer(source)))
}

fn handle_fls_alloc(
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let callback_value = required_argument(contract, frame, 0, "FLS callback")?;
    let callback = (callback_value != 0).then_some(GuestAddress(callback_value));
    if let Some(address) = callback {
        session
            .memory()
            .validate_access(address, 1, MemoryAccess::Execute)
            .map_err(|error| ApiFault::from_memory(error, &contract.endpoint))?;
    }
    let slot = session
        .runtime_mut()
        .allocate_fls_slot(callback)
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    Ok(ApiImmediate::new(ApiReturnValue::U32(slot.index)))
}

fn handle_fls_get_value(
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let index = required_argument(contract, frame, 0, "FLS index")?;
    match session.runtime().fls_value_or_unset(index) {
        Ok(value) => Ok(
            ApiImmediate::new(ApiReturnValue::OpaquePointer(value.unwrap_or(0)))
                .with_last_error(ERROR_SUCCESS),
        ),
        Err(error)
            if matches!(
                error.code,
                "fls-index-unallocated" | "fls-thread-storage-uninitialized"
            ) =>
        {
            Ok(ApiImmediate::new(ApiReturnValue::OpaquePointer(0))
                .with_last_error(ERROR_INVALID_PARAMETER))
        }
        Err(error) => Err(ApiFault::from_runtime(error, &contract.endpoint)),
    }
}

fn handle_fls_set_value(
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let index = required_argument(contract, frame, 0, "FLS index")?;
    let value = required_argument(contract, frame, 1, "FLS value")?;
    session
        .runtime_mut()
        .set_fls_value(index, value)
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    Ok(ApiImmediate::new(ApiReturnValue::Bool32(true)))
}

fn handle_interlocked_increment(
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    handle_interlocked_long_update(contract, frame, session, 1, "InterlockedIncrement")
}

fn handle_interlocked_decrement(
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    handle_interlocked_long_update(contract, frame, session, -1, "InterlockedDecrement")
}

fn handle_interlocked_long_update(
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &ExecutionSession<'_>,
    delta: i32,
    operation: &str,
) -> Result<ApiImmediate, ApiFault> {
    let address = GuestAddress(required_argument(contract, frame, 0, "LONG target")?);
    let bytes = session
        .memory()
        .read(address, size_of::<i32>())
        .map_err(|error| ApiFault::from_memory(error, &contract.endpoint))?;
    let value = i32::from_le_bytes(bytes.try_into().map_err(|_| {
        ApiFault::runtime_defect(
            "win32-interlocked",
            "interlocked-long-read-size-invalid",
            format!("checked {operation} target did not contain one LONG"),
        )
    })?);
    let updated = value.wrapping_add(delta);
    Ok(ApiImmediate::new(ApiReturnValue::U32(updated as u32))
        .with_guest_write(address, updated.to_le_bytes().to_vec()))
}

fn handle_initialize_critical_section_and_spin_count(
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let address = GuestAddress(required_argument(
        contract,
        frame,
        0,
        "critical section storage",
    )?);
    let spin_count = required_argument(contract, frame, 1, "critical section spin count")?;
    let section = session
        .runtime_mut()
        .initialize_critical_section(address, spin_count)
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    Ok(ApiImmediate::new(ApiReturnValue::Bool32(true))
        .with_guest_write(address, x86_critical_section_bytes(section)))
}

fn handle_initialize_critical_section(
    policy: &Kernel32CompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let address = GuestAddress(required_argument(
        contract,
        frame,
        0,
        "critical section storage",
    )?);
    let spin_count_field = policy
        .initialize_critical_section_spin_count_field()
        .ok_or_else(|| {
            ApiFault::unsupported(
                "win32-critical-section",
                "initialize-critical-section-policy-unsupported",
                format!(
                    "compatibility policy {} does not enable InitializeCriticalSection",
                    policy.id
                ),
            )
        })?;
    let section = session
        .runtime_mut()
        .initialize_critical_section_with_spin_state(address, spin_count_field)
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    Ok(ApiImmediate::new(ApiReturnValue::Void)
        .with_guest_write(address, x86_critical_section_bytes(section)))
}

fn handle_enter_critical_section(
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let address = GuestAddress(required_argument(
        contract,
        frame,
        0,
        "critical section storage",
    )?);
    validate_x86_critical_section(contract, session, address)?;
    let current_thread_id = session.runtime().current_thread_id();
    if let Some(owner) = session
        .runtime()
        .critical_section(address)
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?
        .owning_thread_id
        && owner != current_thread_id
    {
        return Err(ApiFault::unsupported(
            "win32-critical-section",
            "critical-section-contention-unimplemented",
            format!(
                "critical section {:#010x} is owned by thread {}; scheduler blocking is not implemented",
                address.0, owner.0
            ),
        ));
    }
    let section = session
        .runtime_mut()
        .enter_critical_section(address)
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    Ok(ApiImmediate::new(ApiReturnValue::Void)
        .with_guest_write(address, x86_critical_section_bytes(section)))
}

fn handle_leave_critical_section(
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let address = GuestAddress(required_argument(
        contract,
        frame,
        0,
        "critical section storage",
    )?);
    validate_x86_critical_section(contract, session, address)?;
    let section = session
        .runtime_mut()
        .leave_critical_section(address)
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    Ok(ApiImmediate::new(ApiReturnValue::Void)
        .with_guest_write(address, x86_critical_section_bytes(section)))
}

fn validate_x86_critical_section(
    contract: &ApiContract,
    session: &ExecutionSession<'_>,
    address: GuestAddress,
) -> Result<(), ApiFault> {
    let section = session
        .runtime()
        .critical_section(address)
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    let expected = x86_critical_section_bytes(section);
    let actual = session
        .memory()
        .read(address, X86_CRITICAL_SECTION_BYTE_LENGTH)
        .map_err(|error| ApiFault::from_memory(error, &contract.endpoint))?;
    if actual != expected {
        return Err(ApiFault::invalid_call(
            "win32-critical-section",
            "critical-section-guest-state-mismatch",
            "guest CRITICAL_SECTION bytes differ from the process-owned synchronization state",
        ));
    }
    Ok(())
}

fn x86_critical_section_bytes(section: &RuntimeCriticalSection) -> Vec<u8> {
    let lock_count = if section.recursion_count == 0 {
        -1_i32
    } else {
        -2_i32
    };
    let owning_thread = section.owning_thread_id.map_or(0, |thread| thread.0);
    let mut bytes = Vec::with_capacity(X86_CRITICAL_SECTION_BYTE_LENGTH);
    bytes.extend_from_slice(&u32::MAX.to_le_bytes());
    bytes.extend_from_slice(&lock_count.to_le_bytes());
    bytes.extend_from_slice(&section.recursion_count.to_le_bytes());
    bytes.extend_from_slice(&owning_thread.to_le_bytes());
    bytes.extend_from_slice(&0_u32.to_le_bytes());
    bytes.extend_from_slice(&section.spin_count.to_le_bytes());
    bytes
}

fn required_argument(
    contract: &ApiContract,
    frame: &ApiCallFrame,
    index: usize,
    label: &str,
) -> Result<u32, ApiFault> {
    frame.argument_u32(index).ok_or_else(|| {
        ApiFault::runtime_defect(
            "win32-api-dispatch",
            "decoded-argument-missing",
            format!(
                "{} handler did not receive its {label}",
                contract.implementation_id
            ),
        )
    })
}

fn read_ascii_nul(
    contract: &ApiContract,
    session: &ExecutionSession<'_>,
    address: GuestAddress,
    maximum_bytes: usize,
) -> Result<String, ApiFault> {
    let mut bytes = Vec::new();
    for offset in 0..maximum_bytes {
        let current = add_guest_offset(contract, address, offset)?;
        let byte = session
            .memory()
            .read(current, 1)
            .map_err(|error| ApiFault::from_memory(error, &contract.endpoint))?[0];
        if byte == 0 {
            return String::from_utf8(bytes).map_err(|_| {
                ApiFault::invalid_call(
                    "win32-string",
                    "guest-ascii-string-invalid",
                    "guest export name is not ASCII",
                )
            });
        }
        if !byte.is_ascii() {
            return Err(ApiFault::invalid_call(
                "win32-string",
                "guest-ascii-string-invalid",
                "guest export name is not ASCII",
            ));
        }
        bytes.push(byte);
    }
    Err(ApiFault::invalid_call(
        "win32-string",
        "guest-ascii-string-unterminated",
        "guest export name exceeds its bounded read",
    ))
}

fn read_utf16_nul(
    contract: &ApiContract,
    session: &ExecutionSession<'_>,
    address: GuestAddress,
    maximum_code_units: usize,
) -> Result<String, ApiFault> {
    let mut code_units = Vec::new();
    for index in 0..maximum_code_units {
        let offset = index.checked_mul(2).ok_or_else(|| {
            ApiFault::resource_limit(
                "win32-string",
                "guest-utf16-offset-overflow",
                "UTF-16 string offset overflowed host accounting",
            )
        })?;
        let current = add_guest_offset(contract, address, offset)?;
        let bytes = session
            .memory()
            .read(current, 2)
            .map_err(|error| ApiFault::from_memory(error, &contract.endpoint))?;
        let code_unit = u16::from_le_bytes([bytes[0], bytes[1]]);
        if code_unit == 0 {
            return String::from_utf16(&code_units).map_err(|_| {
                ApiFault::invalid_call(
                    "win32-string",
                    "guest-utf16-string-invalid",
                    "guest module name is not valid UTF-16",
                )
            });
        }
        code_units.push(code_unit);
    }
    Err(ApiFault::invalid_call(
        "win32-string",
        "guest-utf16-string-unterminated",
        "guest module name exceeds its bounded read",
    ))
}

fn add_guest_offset(
    contract: &ApiContract,
    address: GuestAddress,
    offset: usize,
) -> Result<GuestAddress, ApiFault> {
    let offset = u32::try_from(offset).map_err(|_| {
        ApiFault::resource_limit(
            "win32-string",
            "guest-string-offset-not-representable",
            "guest string offset does not fit x86 address accounting",
        )
    })?;
    address
        .0
        .checked_add(offset)
        .map(GuestAddress)
        .ok_or_else(|| {
            ApiFault::invalid_call(
                "win32-string",
                "guest-string-address-overflow",
                format!(
                    "{} string argument crosses the x86 address limit",
                    contract.implementation_id
                ),
            )
        })
}

pub fn get_system_time_as_file_time_contract(
    endpoint: ApiEndpointIdentity,
) -> Result<ApiContract, ApiFault> {
    kernel32_contract(
        endpoint,
        "GetSystemTimeAsFileTime",
        vec![out_struct("systemTime", "FILETIME", 8)],
        ApiReturnSchema::Void,
        GET_SYSTEM_TIME_AS_FILE_TIME_IMPLEMENTATION,
    )
}

pub fn get_current_process_id_contract(
    endpoint: ApiEndpointIdentity,
) -> Result<ApiContract, ApiFault> {
    kernel32_contract(
        endpoint,
        "GetCurrentProcessId",
        Vec::new(),
        ApiReturnSchema::U32,
        GET_CURRENT_PROCESS_ID_IMPLEMENTATION,
    )
}

pub fn get_current_thread_id_contract(
    endpoint: ApiEndpointIdentity,
) -> Result<ApiContract, ApiFault> {
    kernel32_contract(
        endpoint,
        "GetCurrentThreadId",
        Vec::new(),
        ApiReturnSchema::U32,
        GET_CURRENT_THREAD_ID_IMPLEMENTATION,
    )
}

pub fn get_last_error_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    kernel32_contract(
        endpoint,
        "GetLastError",
        Vec::new(),
        ApiReturnSchema::U32,
        GET_LAST_ERROR_IMPLEMENTATION,
    )
}

pub fn set_last_error_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    let mut contract = kernel32_contract(
        endpoint,
        "SetLastError",
        vec![u32_argument("error")],
        ApiReturnSchema::Void,
        SET_LAST_ERROR_IMPLEMENTATION,
    )?;
    contract.last_error_policy = LastErrorPolicy::FromHandler;
    Ok(contract)
}

pub fn get_acp_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    kernel32_contract(
        endpoint,
        "GetACP",
        Vec::new(),
        ApiReturnSchema::U32,
        GET_ACP_IMPLEMENTATION,
    )
}

pub fn get_cp_info_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    kernel32_contract(
        endpoint,
        "GetCPInfo",
        vec![
            u32_argument("codePage"),
            out_struct("codePageInfo", "CPINFO.fields", 18),
        ],
        ApiReturnSchema::Bool32,
        GET_CP_INFO_IMPLEMENTATION,
    )
}

pub fn is_processor_feature_present_contract(
    endpoint: ApiEndpointIdentity,
) -> Result<ApiContract, ApiFault> {
    kernel32_contract(
        endpoint,
        "IsProcessorFeaturePresent",
        vec![u32_argument("processorFeature")],
        ApiReturnSchema::Bool32,
        IS_PROCESSOR_FEATURE_PRESENT_IMPLEMENTATION,
    )
}

pub fn set_unhandled_exception_filter_contract(
    endpoint: ApiEndpointIdentity,
) -> Result<ApiContract, ApiFault> {
    kernel32_contract(
        endpoint,
        "SetUnhandledExceptionFilter",
        vec![opaque_pointer("filter", true, 1)],
        ApiReturnSchema::OpaquePointer { nullable: true },
        SET_UNHANDLED_EXCEPTION_FILTER_IMPLEMENTATION,
    )
}

pub fn get_tick_count_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    kernel32_contract(
        endpoint,
        "GetTickCount",
        Vec::new(),
        ApiReturnSchema::U32,
        GET_TICK_COUNT_IMPLEMENTATION,
    )
}

pub fn get_std_handle_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    kernel32_contract(
        endpoint,
        "GetStdHandle",
        vec![u32_argument("standardHandle")],
        ApiReturnSchema::LogicalHandle {
            namespace: LOGICAL_STANDARD_HANDLE_NAMESPACE.to_owned(),
            nullable: false,
        },
        GET_STD_HANDLE_IMPLEMENTATION,
    )
}

pub fn get_file_type_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    kernel32_contract(
        endpoint,
        "GetFileType",
        vec![logical_handle(
            "handle",
            LOGICAL_STANDARD_HANDLE_NAMESPACE,
            false,
        )],
        ApiReturnSchema::U32,
        GET_FILE_TYPE_IMPLEMENTATION,
    )
}

pub fn set_handle_count_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    kernel32_contract(
        endpoint,
        "SetHandleCount",
        vec![u32_argument("handleCount")],
        ApiReturnSchema::U32,
        SET_HANDLE_COUNT_IMPLEMENTATION,
    )
}

pub fn get_command_line_a_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    kernel32_contract(
        endpoint,
        "GetCommandLineA",
        Vec::new(),
        ApiReturnSchema::OpaquePointer { nullable: false },
        GET_COMMAND_LINE_A_IMPLEMENTATION,
    )
}

pub fn get_module_file_name_a_contract(
    endpoint: ApiEndpointIdentity,
) -> Result<ApiContract, ApiFault> {
    let mut contract = kernel32_contract(
        endpoint,
        "GetModuleFileNameA",
        vec![
            logical_handle("module", LOGICAL_LOADED_MODULE_NAMESPACE, true),
            dynamic_out_bytes("fileName", true, 1, 1, 32_768),
            u32_argument("capacity"),
        ],
        ApiReturnSchema::U32,
        GET_MODULE_FILE_NAME_A_IMPLEMENTATION,
    )?;
    contract.last_error_policy = LastErrorPolicy::FromHandler;
    Ok(contract)
}

pub fn get_environment_strings_w_contract(
    endpoint: ApiEndpointIdentity,
) -> Result<ApiContract, ApiFault> {
    kernel32_contract(
        endpoint,
        "GetEnvironmentStringsW",
        Vec::new(),
        ApiReturnSchema::OpaquePointer { nullable: false },
        GET_ENVIRONMENT_STRINGS_W_IMPLEMENTATION,
    )
}

pub fn free_environment_strings_w_contract(
    endpoint: ApiEndpointIdentity,
) -> Result<ApiContract, ApiFault> {
    kernel32_contract(
        endpoint,
        "FreeEnvironmentStringsW",
        vec![opaque_pointer("environmentBlock", false, 2)],
        ApiReturnSchema::Bool32,
        FREE_ENVIRONMENT_STRINGS_W_IMPLEMENTATION,
    )
}

pub fn wide_char_to_multi_byte_contract(
    endpoint: ApiEndpointIdentity,
) -> Result<ApiContract, ApiFault> {
    let mut contract = kernel32_contract(
        endpoint,
        "WideCharToMultiByte",
        vec![
            u32_argument("codePage"),
            u32_argument("flags"),
            opaque_pointer("wideCharSource", true, 2),
            u32_argument("wideCharCount"),
            dynamic_out_bytes("multiByteDestination", true, 1, 1, 2_097_152),
            u32_argument("multiByteCapacity"),
            opaque_pointer("defaultChar", true, 1),
            optional_out_struct("usedDefaultChar", "BOOL", 4, 1),
        ],
        ApiReturnSchema::U32,
        WIDE_CHAR_TO_MULTI_BYTE_IMPLEMENTATION,
    )?;
    contract.last_error_policy = LastErrorPolicy::FromHandler;
    Ok(contract)
}

pub fn multi_byte_to_wide_char_contract(
    endpoint: ApiEndpointIdentity,
) -> Result<ApiContract, ApiFault> {
    let mut contract = kernel32_contract(
        endpoint,
        "MultiByteToWideChar",
        vec![
            u32_argument("codePage"),
            u32_argument("flags"),
            dynamic_in_bytes("multiByteSource", true, 1, 1, 1_048_576),
            u32_argument("multiByteCount"),
            dynamic_out_bytes("wideCharDestination", true, 2, 2, 2_097_152),
            u32_argument("wideCharCapacity"),
        ],
        ApiReturnSchema::U32,
        MULTI_BYTE_TO_WIDE_CHAR_IMPLEMENTATION,
    )?;
    contract.last_error_policy = LastErrorPolicy::FromHandler;
    Ok(contract)
}

pub fn set_current_directory_a_contract(
    endpoint: ApiEndpointIdentity,
) -> Result<ApiContract, ApiFault> {
    let mut contract = kernel32_contract(
        endpoint,
        "SetCurrentDirectoryA",
        vec![dynamic_in_bytes("directoryPath", true, 1, 1, 32_768)],
        ApiReturnSchema::Bool32,
        SET_CURRENT_DIRECTORY_A_IMPLEMENTATION,
    )?;
    contract.last_error_policy = LastErrorPolicy::FromHandler;
    Ok(contract)
}

pub fn get_string_type_w_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    let mut contract = kernel32_contract(
        endpoint,
        "GetStringTypeW",
        vec![
            u32_argument("infoType"),
            opaque_pointer("wideCharSource", false, 2),
            u32_argument("wideCharCount"),
            dynamic_out_bytes("characterTypes", false, 2, 2, 2_097_152),
        ],
        ApiReturnSchema::Bool32,
        GET_STRING_TYPE_W_IMPLEMENTATION,
    )?;
    contract.last_error_policy = LastErrorPolicy::FromHandler;
    Ok(contract)
}

pub fn lc_map_string_w_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    let mut contract = kernel32_contract(
        endpoint,
        "LCMapStringW",
        vec![
            u32_argument("locale"),
            u32_argument("mapFlags"),
            opaque_pointer("wideCharSource", false, 2),
            u32_argument("wideCharCount"),
            dynamic_out_bytes("wideCharDestination", true, 2, 2, 2_097_152),
            u32_argument("wideCharCapacity"),
        ],
        ApiReturnSchema::U32,
        LC_MAP_STRING_W_IMPLEMENTATION,
    )?;
    contract.last_error_policy = LastErrorPolicy::FromHandler;
    Ok(contract)
}

pub fn query_performance_counter_contract(
    endpoint: ApiEndpointIdentity,
) -> Result<ApiContract, ApiFault> {
    kernel32_contract(
        endpoint,
        "QueryPerformanceCounter",
        vec![out_struct("performanceCounter", "LARGE_INTEGER", 8)],
        ApiReturnSchema::Bool32,
        QUERY_PERFORMANCE_COUNTER_IMPLEMENTATION,
    )
}

pub fn get_startup_info_w_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    kernel32_contract(
        endpoint,
        "GetStartupInfoW",
        vec![out_struct(
            "startupInfo",
            "STARTUPINFOW",
            X86_STARTUP_INFO_W_BYTE_LENGTH as u32,
        )],
        ApiReturnSchema::Void,
        GET_STARTUP_INFO_W_IMPLEMENTATION,
    )
}

pub fn heap_set_information_contract(
    endpoint: ApiEndpointIdentity,
) -> Result<ApiContract, ApiFault> {
    kernel32_contract(
        endpoint,
        "HeapSetInformation",
        vec![
            logical_handle("heap", LOGICAL_PRIVATE_HEAP_NAMESPACE, true),
            u32_argument("informationClass"),
            opaque_pointer("information", true, 1),
            u32_argument("informationLength"),
        ],
        ApiReturnSchema::Bool32,
        HEAP_SET_INFORMATION_IMPLEMENTATION,
    )
}

pub fn heap_create_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    kernel32_contract(
        endpoint,
        "HeapCreate",
        vec![
            u32_argument("options"),
            u32_argument("initialSize"),
            u32_argument("maximumSize"),
        ],
        ApiReturnSchema::LogicalHandle {
            namespace: LOGICAL_PRIVATE_HEAP_NAMESPACE.to_owned(),
            nullable: false,
        },
        HEAP_CREATE_IMPLEMENTATION,
    )
}

pub fn heap_alloc_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    kernel32_contract(
        endpoint,
        "HeapAlloc",
        vec![
            logical_handle("heap", LOGICAL_PRIVATE_HEAP_NAMESPACE, false),
            u32_argument("flags"),
            u32_argument("byteLength"),
        ],
        ApiReturnSchema::OpaquePointer { nullable: false },
        HEAP_ALLOC_IMPLEMENTATION,
    )
}

pub fn heap_free_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    kernel32_contract(
        endpoint,
        "HeapFree",
        vec![
            logical_handle("heap", LOGICAL_PRIVATE_HEAP_NAMESPACE, false),
            u32_argument("flags"),
            opaque_pointer("allocation", false, RUNTIME_PRIVATE_HEAP_ALIGNMENT),
        ],
        ApiReturnSchema::Bool32,
        HEAP_FREE_IMPLEMENTATION,
    )
}

pub fn heap_size_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    let mut contract = kernel32_contract(
        endpoint,
        "HeapSize",
        vec![
            logical_handle("heap", LOGICAL_PRIVATE_HEAP_NAMESPACE, false),
            u32_argument("flags"),
            opaque_pointer("allocation", true, 1),
        ],
        ApiReturnSchema::U32,
        HEAP_SIZE_IMPLEMENTATION,
    )?;
    contract.last_error_policy = LastErrorPolicy::FromHandler;
    Ok(contract)
}

pub fn get_module_handle_w_contract(
    endpoint: ApiEndpointIdentity,
) -> Result<ApiContract, ApiFault> {
    kernel32_contract(
        endpoint,
        "GetModuleHandleW",
        vec![input_string(
            "moduleName",
            ApiStringEncoding::Utf16Le,
            2,
            2,
            MAX_MODULE_NAME_CODE_UNITS as u32,
        )],
        ApiReturnSchema::LogicalHandle {
            namespace: LOGICAL_LOADED_MODULE_NAMESPACE.to_owned(),
            nullable: false,
        },
        GET_MODULE_HANDLE_W_IMPLEMENTATION,
    )
}

pub fn get_proc_address_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    kernel32_contract(
        endpoint,
        "GetProcAddress",
        vec![
            logical_handle("module", LOGICAL_LOADED_MODULE_NAMESPACE, false),
            name_or_ordinal(
                "procedureName",
                ApiStringEncoding::Windows1252,
                MAX_EXPORT_NAME_BYTES as u32,
            ),
        ],
        ApiReturnSchema::LogicalPointer {
            namespace: DYNAMIC_EXPORT_ENDPOINT_NAMESPACE.to_owned(),
            nullable: false,
        },
        GET_PROC_ADDRESS_IMPLEMENTATION,
    )
}

pub fn tls_alloc_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    kernel32_contract(
        endpoint,
        "TlsAlloc",
        Vec::new(),
        ApiReturnSchema::U32,
        TLS_ALLOC_IMPLEMENTATION,
    )
}

pub fn tls_get_value_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    let mut contract = kernel32_contract(
        endpoint,
        "TlsGetValue",
        vec![u32_argument("tlsIndex")],
        ApiReturnSchema::OpaquePointer { nullable: true },
        TLS_GET_VALUE_IMPLEMENTATION,
    )?;
    contract.last_error_policy = LastErrorPolicy::FromHandler;
    Ok(contract)
}

pub fn tls_set_value_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    kernel32_contract(
        endpoint,
        "TlsSetValue",
        vec![u32_argument("tlsIndex"), opaque_pointer("value", true, 1)],
        ApiReturnSchema::Bool32,
        TLS_SET_VALUE_IMPLEMENTATION,
    )
}

pub fn encode_pointer_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    kernel32_contract(
        endpoint,
        "EncodePointer",
        vec![opaque_pointer("pointer", true, 1)],
        ApiReturnSchema::LogicalPointer {
            namespace: LOGICAL_ENCODED_POINTER_NAMESPACE.to_owned(),
            nullable: false,
        },
        ENCODE_POINTER_IMPLEMENTATION,
    )
}

pub fn decode_pointer_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    kernel32_contract(
        endpoint,
        "DecodePointer",
        vec![opaque_pointer("encodedPointer", false, 1)],
        ApiReturnSchema::OpaquePointer { nullable: true },
        DECODE_POINTER_IMPLEMENTATION,
    )
}

pub fn fls_alloc_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    kernel32_contract(
        endpoint,
        "FlsAlloc",
        vec![opaque_pointer("callback", true, 1)],
        ApiReturnSchema::U32,
        FLS_ALLOC_IMPLEMENTATION,
    )
}

pub fn fls_get_value_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    let mut contract = kernel32_contract(
        endpoint,
        "FlsGetValue",
        vec![u32_argument("flsIndex")],
        ApiReturnSchema::OpaquePointer { nullable: true },
        FLS_GET_VALUE_IMPLEMENTATION,
    )?;
    contract.last_error_policy = LastErrorPolicy::FromHandler;
    Ok(contract)
}

pub fn fls_set_value_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    kernel32_contract(
        endpoint,
        "FlsSetValue",
        vec![u32_argument("flsIndex"), opaque_pointer("value", true, 1)],
        ApiReturnSchema::Bool32,
        FLS_SET_VALUE_IMPLEMENTATION,
    )
}

pub fn interlocked_increment_contract(
    endpoint: ApiEndpointIdentity,
) -> Result<ApiContract, ApiFault> {
    kernel32_contract(
        endpoint,
        "InterlockedIncrement",
        vec![inout_struct("addend", "LONG", size_of::<i32>() as u32)],
        ApiReturnSchema::U32,
        INTERLOCKED_INCREMENT_IMPLEMENTATION,
    )
}

pub fn interlocked_decrement_contract(
    endpoint: ApiEndpointIdentity,
) -> Result<ApiContract, ApiFault> {
    kernel32_contract(
        endpoint,
        "InterlockedDecrement",
        vec![inout_struct("addend", "LONG", size_of::<i32>() as u32)],
        ApiReturnSchema::U32,
        INTERLOCKED_DECREMENT_IMPLEMENTATION,
    )
}

pub fn initialize_critical_section_and_spin_count_contract(
    endpoint: ApiEndpointIdentity,
) -> Result<ApiContract, ApiFault> {
    kernel32_contract(
        endpoint,
        "InitializeCriticalSectionAndSpinCount",
        vec![
            out_struct(
                "criticalSection",
                "RTL_CRITICAL_SECTION",
                X86_CRITICAL_SECTION_BYTE_LENGTH as u32,
            ),
            u32_argument("spinCount"),
        ],
        ApiReturnSchema::Bool32,
        INITIALIZE_CRITICAL_SECTION_AND_SPIN_COUNT_IMPLEMENTATION,
    )
}

pub fn initialize_critical_section_contract(
    endpoint: ApiEndpointIdentity,
) -> Result<ApiContract, ApiFault> {
    kernel32_contract(
        endpoint,
        "InitializeCriticalSection",
        vec![out_struct(
            "criticalSection",
            "RTL_CRITICAL_SECTION",
            X86_CRITICAL_SECTION_BYTE_LENGTH as u32,
        )],
        ApiReturnSchema::Void,
        INITIALIZE_CRITICAL_SECTION_IMPLEMENTATION,
    )
}

pub fn enter_critical_section_contract(
    endpoint: ApiEndpointIdentity,
) -> Result<ApiContract, ApiFault> {
    kernel32_contract(
        endpoint,
        "EnterCriticalSection",
        vec![inout_struct(
            "criticalSection",
            "RTL_CRITICAL_SECTION",
            X86_CRITICAL_SECTION_BYTE_LENGTH as u32,
        )],
        ApiReturnSchema::Void,
        ENTER_CRITICAL_SECTION_IMPLEMENTATION,
    )
}

pub fn leave_critical_section_contract(
    endpoint: ApiEndpointIdentity,
) -> Result<ApiContract, ApiFault> {
    kernel32_contract(
        endpoint,
        "LeaveCriticalSection",
        vec![inout_struct(
            "criticalSection",
            "RTL_CRITICAL_SECTION",
            X86_CRITICAL_SECTION_BYTE_LENGTH as u32,
        )],
        ApiReturnSchema::Void,
        LEAVE_CRITICAL_SECTION_IMPLEMENTATION,
    )
}

fn kernel32_contract(
    endpoint: ApiEndpointIdentity,
    name: &str,
    arguments: Vec<ApiArgumentSchema>,
    return_value: ApiReturnSchema,
    implementation_id: &str,
) -> Result<ApiContract, ApiFault> {
    let symbol = ApiSymbol::Name {
        name: name.to_owned(),
    };
    if !endpoint.matches("kernel32.dll", &symbol) {
        return Err(ApiFault::invalid_call(
            "win32-api-contract",
            "kernel32-endpoint-identity-mismatch",
            format!("endpoint does not identify kernel32.dll!{name}"),
        ));
    }
    Ok(ApiContract {
        endpoint,
        abi: AbiSchema {
            calling_convention: CallingConvention::Stdcall,
            arguments,
            return_value,
            stack_cleanup: StackCleanup::Callee,
        },
        last_error_policy: LastErrorPolicy::Preserve,
        reentrancy_policy: ReentrancyPolicy::Disallowed,
        behavior: ApiBehavior::Immediate,
        callback_policy: CallbackPolicy::Forbidden,
        implementation_id: implementation_id.to_owned(),
    })
}

fn u32_argument(name: &str) -> ApiArgumentSchema {
    ApiArgumentSchema {
        name: name.to_owned(),
        direction: ApiDirection::In,
        value: ApiValueSchema::U32,
    }
}

fn opaque_pointer(name: &str, nullable: bool, alignment: u32) -> ApiArgumentSchema {
    ApiArgumentSchema {
        name: name.to_owned(),
        direction: ApiDirection::In,
        value: ApiValueSchema::OpaquePointer {
            nullable,
            alignment,
        },
    }
}

fn logical_handle(name: &str, namespace: &str, nullable: bool) -> ApiArgumentSchema {
    ApiArgumentSchema {
        name: name.to_owned(),
        direction: ApiDirection::In,
        value: ApiValueSchema::LogicalHandle {
            namespace: namespace.to_owned(),
            nullable,
        },
    }
}

fn name_or_ordinal(
    name: &str,
    encoding: ApiStringEncoding,
    maximum_code_units: u32,
) -> ApiArgumentSchema {
    ApiArgumentSchema {
        name: name.to_owned(),
        direction: ApiDirection::In,
        value: ApiValueSchema::NameOrOrdinal {
            encoding,
            maximum_code_units,
        },
    }
}

fn input_string(
    name: &str,
    encoding: ApiStringEncoding,
    alignment: u32,
    minimum_byte_length: u32,
    maximum_code_units: u32,
) -> ApiArgumentSchema {
    ApiArgumentSchema {
        name: name.to_owned(),
        direction: ApiDirection::In,
        value: ApiValueSchema::Pointer {
            constraint: ApiPointerConstraint {
                nullable: false,
                alignment,
                byte_length: minimum_byte_length,
                pointee: ApiPointeeSchema::String {
                    encoding,
                    maximum_code_units,
                    nul_terminated: true,
                },
                write_requirement: ApiWriteRequirement::None,
            },
        },
    }
}

fn out_struct(name: &str, type_id: &str, byte_length: u32) -> ApiArgumentSchema {
    ApiArgumentSchema {
        name: name.to_owned(),
        direction: ApiDirection::Out,
        value: ApiValueSchema::Pointer {
            constraint: ApiPointerConstraint {
                nullable: false,
                alignment: 4,
                byte_length,
                pointee: ApiPointeeSchema::Struct {
                    type_id: type_id.to_owned(),
                },
                write_requirement: ApiWriteRequirement::Required,
            },
        },
    }
}

fn optional_out_struct(
    name: &str,
    type_id: &str,
    byte_length: u32,
    alignment: u32,
) -> ApiArgumentSchema {
    ApiArgumentSchema {
        name: name.to_owned(),
        direction: ApiDirection::Out,
        value: ApiValueSchema::Pointer {
            constraint: ApiPointerConstraint {
                nullable: true,
                alignment,
                byte_length,
                pointee: ApiPointeeSchema::Struct {
                    type_id: type_id.to_owned(),
                },
                write_requirement: ApiWriteRequirement::Optional,
            },
        },
    }
}

fn dynamic_out_bytes(
    name: &str,
    nullable: bool,
    alignment: u32,
    minimum_byte_length: u32,
    maximum_byte_length: u32,
) -> ApiArgumentSchema {
    ApiArgumentSchema {
        name: name.to_owned(),
        direction: ApiDirection::Out,
        value: ApiValueSchema::DynamicPointer {
            constraint: ApiDynamicPointerConstraint {
                nullable,
                alignment,
                minimum_byte_length,
                maximum_byte_length,
                pointee: ApiPointeeSchema::Bytes,
                write_requirement: ApiWriteRequirement::Optional,
            },
        },
    }
}

fn dynamic_in_bytes(
    name: &str,
    nullable: bool,
    alignment: u32,
    minimum_byte_length: u32,
    maximum_byte_length: u32,
) -> ApiArgumentSchema {
    ApiArgumentSchema {
        name: name.to_owned(),
        direction: ApiDirection::In,
        value: ApiValueSchema::DynamicPointer {
            constraint: ApiDynamicPointerConstraint {
                nullable,
                alignment,
                minimum_byte_length,
                maximum_byte_length,
                pointee: ApiPointeeSchema::Bytes,
                write_requirement: ApiWriteRequirement::None,
            },
        },
    }
}

fn inout_struct(name: &str, type_id: &str, byte_length: u32) -> ApiArgumentSchema {
    ApiArgumentSchema {
        name: name.to_owned(),
        direction: ApiDirection::InOut,
        value: ApiValueSchema::Pointer {
            constraint: ApiPointerConstraint {
                nullable: false,
                alignment: 4,
                byte_length,
                pointee: ApiPointeeSchema::Struct {
                    type_id: type_id.to_owned(),
                },
                write_requirement: ApiWriteRequirement::Required,
            },
        },
    }
}

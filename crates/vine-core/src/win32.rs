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
    LOGICAL_PRIVATE_HEAP_NAMESPACE, LOGICAL_STANDARD_HANDLE_NAMESPACE, RuntimeCriticalSection,
    RuntimeHeapInitialization, X86_CRITICAL_SECTION_BYTE_LENGTH, X86_STARTUP_INFO_W_BYTE_LENGTH,
};

mod policy;

pub use policy::{
    KERNEL32_COMPATIBILITY_POLICY_SCHEMA, KERNEL32_COMPATIBILITY_POLICY_VERSION,
    Kernel32CodePageInfoPolicy, Kernel32CompatibilityPolicy, Kernel32CompatibilityPolicyError,
    Kernel32HeapAllocationPolicy, Kernel32StandardHandleFileType,
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
pub const GET_TICK_COUNT_IMPLEMENTATION: &str = "kernel32.get-tick-count.v1";
pub const GET_STD_HANDLE_IMPLEMENTATION: &str = "kernel32.get-std-handle.v2";
pub const GET_FILE_TYPE_IMPLEMENTATION: &str = "kernel32.get-file-type.v2";
pub const SET_HANDLE_COUNT_IMPLEMENTATION: &str = "kernel32.set-handle-count.v1";
pub const GET_COMMAND_LINE_A_IMPLEMENTATION: &str = "kernel32.get-command-line-a.v1";
pub const GET_ENVIRONMENT_STRINGS_W_IMPLEMENTATION: &str = "kernel32.get-environment-strings-w.v1";
pub const FREE_ENVIRONMENT_STRINGS_W_IMPLEMENTATION: &str =
    "kernel32.free-environment-strings-w.v1";
pub const WIDE_CHAR_TO_MULTI_BYTE_IMPLEMENTATION: &str = "kernel32.wide-char-to-multi-byte.v1";
pub const QUERY_PERFORMANCE_COUNTER_IMPLEMENTATION: &str = "kernel32.query-performance-counter.v1";
pub const GET_STARTUP_INFO_W_IMPLEMENTATION: &str = "kernel32.get-startup-info-w.v1";
pub const HEAP_SET_INFORMATION_IMPLEMENTATION: &str = "kernel32.heap-set-information.v1";
pub const HEAP_CREATE_IMPLEMENTATION: &str = "kernel32.heap-create.v1";
pub const HEAP_ALLOC_IMPLEMENTATION: &str = "kernel32.heap-alloc.v2";
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
pub const INITIALIZE_CRITICAL_SECTION_AND_SPIN_COUNT_IMPLEMENTATION: &str =
    "kernel32.initialize-critical-section-and-spin-count.v1";
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
        GET_TICK_COUNT_IMPLEMENTATION => handle_get_tick_count(session),
        GET_STD_HANDLE_IMPLEMENTATION => handle_get_std_handle(policy, contract, frame, session),
        GET_FILE_TYPE_IMPLEMENTATION => handle_get_file_type(policy, contract, frame, session),
        SET_HANDLE_COUNT_IMPLEMENTATION => handle_set_handle_count(policy, contract, frame),
        GET_COMMAND_LINE_A_IMPLEMENTATION => handle_get_command_line_a(contract, session),
        GET_ENVIRONMENT_STRINGS_W_IMPLEMENTATION => {
            handle_get_environment_strings_w(contract, session)
        }
        FREE_ENVIRONMENT_STRINGS_W_IMPLEMENTATION => {
            handle_free_environment_strings_w(contract, frame, session)
        }
        WIDE_CHAR_TO_MULTI_BYTE_IMPLEMENTATION => {
            handle_wide_char_to_multi_byte(policy, contract, frame, session)
        }
        QUERY_PERFORMANCE_COUNTER_IMPLEMENTATION => {
            handle_query_performance_counter(contract, frame, session)
        }
        GET_STARTUP_INFO_W_IMPLEMENTATION => handle_get_startup_info_w(contract, frame, session),
        HEAP_SET_INFORMATION_IMPLEMENTATION => {
            handle_heap_set_information(contract, frame, session)
        }
        HEAP_CREATE_IMPLEMENTATION => handle_heap_create(contract, frame, session),
        HEAP_ALLOC_IMPLEMENTATION => handle_heap_alloc(policy, contract, frame, session),
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
        INITIALIZE_CRITICAL_SECTION_AND_SPIN_COUNT_IMPLEMENTATION => {
            handle_initialize_critical_section_and_spin_count(contract, frame, session)
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
        "GetTickCount" => get_tick_count_contract(endpoint)?,
        "GetStdHandle" => get_std_handle_contract(endpoint)?,
        "GetFileType" => get_file_type_contract(endpoint)?,
        "SetHandleCount" => set_handle_count_contract(endpoint)?,
        "GetCommandLineA" => get_command_line_a_contract(endpoint)?,
        "GetEnvironmentStringsW" => get_environment_strings_w_contract(endpoint)?,
        "FreeEnvironmentStringsW" => free_environment_strings_w_contract(endpoint)?,
        "WideCharToMultiByte" => wide_char_to_multi_byte_contract(endpoint)?,
        "QueryPerformanceCounter" => query_performance_counter_contract(endpoint)?,
        "GetStartupInfoW" => get_startup_info_w_contract(endpoint)?,
        "HeapSetInformation" => heap_set_information_contract(endpoint)?,
        "HeapCreate" => heap_create_contract(endpoint)?,
        "HeapAlloc" => heap_alloc_contract(endpoint)?,
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
        "InitializeCriticalSectionAndSpinCount" => {
            initialize_critical_section_and_spin_count_contract(endpoint)?
        }
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
    let address = GuestAddress(required_argument(contract, frame, 0, "LONG target")?);
    let bytes = session
        .memory()
        .read(address, size_of::<i32>())
        .map_err(|error| ApiFault::from_memory(error, &contract.endpoint))?;
    let value = i32::from_le_bytes(bytes.try_into().map_err(|_| {
        ApiFault::runtime_defect(
            "win32-interlocked",
            "interlocked-long-read-size-invalid",
            "checked InterlockedIncrement target did not contain one LONG",
        )
    })?);
    let incremented = value.wrapping_add(1);
    Ok(ApiImmediate::new(ApiReturnValue::U32(incremented as u32))
        .with_guest_write(address, incremented.to_le_bytes().to_vec()))
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

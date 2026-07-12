use std::mem::size_of;

use crate::api::{
    AbiSchema, ApiArgumentSchema, ApiBehavior, ApiCallFrame, ApiContract, ApiDirection,
    ApiEndpointIdentity, ApiFault, ApiHandlerResult, ApiImmediate, ApiPointeeSchema,
    ApiPointerConstraint, ApiReturnSchema, ApiReturnValue, ApiStringEncoding, ApiSymbol,
    ApiValueSchema, ApiWriteRequirement, CallbackPolicy, CallingConvention, ExecutionSession,
    LastErrorPolicy, ReentrancyPolicy, StackCleanup,
};
use crate::dynamic_exports::DYNAMIC_EXPORT_ENDPOINT_NAMESPACE;
use crate::memory::{GuestAddress, MemoryAccess};
use crate::runtime::{
    LOGICAL_ENCODED_POINTER_NAMESPACE, LOGICAL_LOADED_MODULE_NAMESPACE,
    LOGICAL_PRIVATE_HEAP_NAMESPACE, LOGICAL_STANDARD_HANDLE_NAMESPACE,
    RUNTIME_STANDARD_INPUT_SELECTOR, RuntimeCriticalSection, RuntimeHeapInitialization,
    X86_CRITICAL_SECTION_BYTE_LENGTH, X86_STARTUP_INFO_W_BYTE_LENGTH,
};

pub const GET_SYSTEM_TIME_AS_FILE_TIME_IMPLEMENTATION: &str =
    "kernel32.get-system-time-as-file-time.v1";
pub const GET_CURRENT_PROCESS_ID_IMPLEMENTATION: &str = "kernel32.get-current-process-id.v1";
pub const GET_CURRENT_THREAD_ID_IMPLEMENTATION: &str = "kernel32.get-current-thread-id.v1";
pub const GET_TICK_COUNT_IMPLEMENTATION: &str = "kernel32.get-tick-count.v1";
pub const GET_STD_HANDLE_IMPLEMENTATION: &str = "kernel32.get-std-handle-input.v1";
pub const GET_FILE_TYPE_IMPLEMENTATION: &str = "kernel32.get-file-type-standard-input.v1";
pub const QUERY_PERFORMANCE_COUNTER_IMPLEMENTATION: &str = "kernel32.query-performance-counter.v1";
pub const GET_STARTUP_INFO_W_IMPLEMENTATION: &str = "kernel32.get-startup-info-w.v1";
pub const HEAP_SET_INFORMATION_IMPLEMENTATION: &str = "kernel32.heap-set-information.v1";
pub const HEAP_CREATE_IMPLEMENTATION: &str = "kernel32.heap-create.v1";
pub const HEAP_ALLOC_IMPLEMENTATION: &str = "kernel32.heap-alloc.v1";
pub const HEAP_ALLOC_FIRST_TWO_REACHED_IMPLEMENTATION: &str =
    "kernel32.heap-alloc-first-two-reached.v1";
pub const GET_MODULE_HANDLE_W_IMPLEMENTATION: &str = "kernel32.get-module-handle-w.v1";
pub const GET_PROC_ADDRESS_IMPLEMENTATION: &str = "kernel32.get-proc-address.v1";
pub const TLS_ALLOC_IMPLEMENTATION: &str = "kernel32.tls-alloc.v1";
pub const TLS_SET_VALUE_IMPLEMENTATION: &str = "kernel32.tls-set-value.v1";
pub const ENCODE_POINTER_IMPLEMENTATION: &str = "kernel32.encode-pointer.v1";
pub const DECODE_POINTER_IMPLEMENTATION: &str = "kernel32.decode-pointer.v1";
pub const FLS_ALLOC_IMPLEMENTATION: &str = "kernel32.fls-alloc.v1";
pub const FLS_SET_VALUE_IMPLEMENTATION: &str = "kernel32.fls-set-value.v1";
pub const INTERLOCKED_INCREMENT_IMPLEMENTATION: &str = "kernel32.interlocked-increment.v1";
pub const INITIALIZE_CRITICAL_SECTION_AND_SPIN_COUNT_IMPLEMENTATION: &str =
    "kernel32.initialize-critical-section-and-spin-count.v1";
pub const ENTER_CRITICAL_SECTION_IMPLEMENTATION: &str = "kernel32.enter-critical-section.v1";
pub const LEAVE_CRITICAL_SECTION_IMPLEMENTATION: &str = "kernel32.leave-critical-section.v1";

const HEAP_ENABLE_TERMINATION_ON_CORRUPTION: u32 = 1;
const HEAP_ZERO_MEMORY: u32 = 0x0000_0008;
const REACHED_HEAP_ALLOC_BYTE_LENGTH: u32 = 0x0000_0214;
const REACHED_SECOND_HEAP_ALLOC_BYTE_LENGTH: u32 = 0x0000_0800;
const MAX_MODULE_NAME_CODE_UNITS: usize = 64;
const MAX_EXPORT_NAME_BYTES: usize = 64;

pub fn dispatch_kernel32_api(
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> ApiHandlerResult {
    let result = match contract.implementation_id.as_str() {
        GET_SYSTEM_TIME_AS_FILE_TIME_IMPLEMENTATION => {
            handle_get_system_time_as_file_time(contract, frame, session)
        }
        GET_CURRENT_PROCESS_ID_IMPLEMENTATION => handle_get_current_process_id(session),
        GET_CURRENT_THREAD_ID_IMPLEMENTATION => handle_get_current_thread_id(session),
        GET_TICK_COUNT_IMPLEMENTATION => handle_get_tick_count(session),
        GET_STD_HANDLE_IMPLEMENTATION => handle_get_std_handle(contract, frame, session),
        GET_FILE_TYPE_IMPLEMENTATION => handle_get_file_type(contract, frame, session),
        QUERY_PERFORMANCE_COUNTER_IMPLEMENTATION => {
            handle_query_performance_counter(contract, frame, session)
        }
        GET_STARTUP_INFO_W_IMPLEMENTATION => handle_get_startup_info_w(contract, frame, session),
        HEAP_SET_INFORMATION_IMPLEMENTATION => {
            handle_heap_set_information(contract, frame, session)
        }
        HEAP_CREATE_IMPLEMENTATION => handle_heap_create(contract, frame, session),
        HEAP_ALLOC_IMPLEMENTATION => {
            handle_heap_alloc(contract, frame, session, ReachedHeapAllocProfile::FirstOnly)
        }
        HEAP_ALLOC_FIRST_TWO_REACHED_IMPLEMENTATION => {
            handle_heap_alloc(contract, frame, session, ReachedHeapAllocProfile::FirstTwo)
        }
        GET_MODULE_HANDLE_W_IMPLEMENTATION => handle_get_module_handle_w(contract, frame, session),
        GET_PROC_ADDRESS_IMPLEMENTATION => handle_get_proc_address(contract, frame, session),
        TLS_ALLOC_IMPLEMENTATION => handle_tls_alloc(contract, session),
        TLS_SET_VALUE_IMPLEMENTATION => handle_tls_set_value(contract, frame, session),
        ENCODE_POINTER_IMPLEMENTATION => handle_encode_pointer(contract, frame, session),
        DECODE_POINTER_IMPLEMENTATION => handle_decode_pointer(contract, frame, session),
        FLS_ALLOC_IMPLEMENTATION => handle_fls_alloc(contract, frame, session),
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
        "GetTickCount" => get_tick_count_contract(endpoint)?,
        "GetStdHandle" => get_std_handle_contract(endpoint)?,
        "GetFileType" => get_file_type_contract(endpoint)?,
        "QueryPerformanceCounter" => query_performance_counter_contract(endpoint)?,
        "GetStartupInfoW" => get_startup_info_w_contract(endpoint)?,
        "HeapSetInformation" => heap_set_information_contract(endpoint)?,
        "HeapCreate" => heap_create_contract(endpoint)?,
        "HeapAlloc" => heap_alloc_contract(endpoint)?,
        "GetModuleHandleW" => get_module_handle_w_contract(endpoint)?,
        "GetProcAddress" => get_proc_address_contract(endpoint)?,
        "TlsAlloc" => tls_alloc_contract(endpoint)?,
        "TlsSetValue" => tls_set_value_contract(endpoint)?,
        "EncodePointer" => encode_pointer_contract(endpoint)?,
        "DecodePointer" => decode_pointer_contract(endpoint)?,
        "FlsAlloc" => fls_alloc_contract(endpoint)?,
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

fn handle_get_tick_count(session: &ExecutionSession<'_>) -> Result<ApiImmediate, ApiFault> {
    Ok(ApiImmediate::new(ApiReturnValue::U32(
        session.runtime().clock().tick_count,
    )))
}

fn handle_get_std_handle(
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let selector = required_argument(contract, frame, 0, "standard handle selector")?;
    if selector != RUNTIME_STANDARD_INPUT_SELECTOR {
        return Err(ApiFault::unsupported(
            "win32-standard-handle",
            "standard-handle-selector-unsupported",
            "only the reached STD_INPUT_HANDLE selector is implemented",
        ));
    }
    let standard_handle = session
        .runtime_mut()
        .ensure_standard_input_handle(selector)
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?
        .clone();
    Ok(ApiImmediate::new(ApiReturnValue::LogicalHandle {
        namespace: standard_handle.namespace,
        value: standard_handle.handle,
    }))
}

fn handle_get_file_type(
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    const FILE_TYPE_CHAR: u32 = 2;

    let handle = required_argument(contract, frame, 0, "standard handle")?;
    session
        .runtime()
        .standard_handle_by_value(handle)
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    Ok(ApiImmediate::new(ApiReturnValue::U32(FILE_TYPE_CHAR)))
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
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
    profile: ReachedHeapAllocProfile,
) -> Result<ApiImmediate, ApiFault> {
    let heap = required_argument(contract, frame, 0, "heap handle")?;
    let flags = required_argument(contract, frame, 1, "heap allocation flags")?;
    let byte_length = required_argument(contract, frame, 2, "heap allocation byte length")?;
    let supported_length = byte_length == REACHED_HEAP_ALLOC_BYTE_LENGTH
        || (profile == ReachedHeapAllocProfile::FirstTwo
            && byte_length == REACHED_SECOND_HEAP_ALLOC_BYTE_LENGTH);
    if flags != HEAP_ZERO_MEMORY || !supported_length {
        let message = match profile {
            ReachedHeapAllocProfile::FirstOnly => {
                "only the reached HEAP_ZERO_MEMORY 0x214-byte allocation is implemented"
            }
            ReachedHeapAllocProfile::FirstTwo => {
                "only the first two reached HEAP_ZERO_MEMORY allocations (0x214 and 0x800 bytes) are implemented"
            }
        };
        return Err(ApiFault::unsupported(
            "win32-heap",
            "heap-alloc-variant-unsupported",
            message,
        ));
    }
    let allocation = session
        .allocate_private_heap_block(heap, byte_length, RuntimeHeapInitialization::Zeroed)
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    Ok(ApiImmediate::new(ApiReturnValue::OpaquePointer(
        allocation.address.0,
    )))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReachedHeapAllocProfile {
    FirstOnly,
    FirstTwo,
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
    heap_alloc_contract_with_implementation(endpoint, HEAP_ALLOC_IMPLEMENTATION)
}

pub fn heap_alloc_first_two_reached_contract(
    endpoint: ApiEndpointIdentity,
) -> Result<ApiContract, ApiFault> {
    heap_alloc_contract_with_implementation(endpoint, HEAP_ALLOC_FIRST_TWO_REACHED_IMPLEMENTATION)
}

fn heap_alloc_contract_with_implementation(
    endpoint: ApiEndpointIdentity,
    implementation_id: &str,
) -> Result<ApiContract, ApiFault> {
    kernel32_contract(
        endpoint,
        "HeapAlloc",
        vec![
            logical_handle("heap", LOGICAL_PRIVATE_HEAP_NAMESPACE, false),
            u32_argument("flags"),
            u32_argument("byteLength"),
        ],
        ApiReturnSchema::OpaquePointer { nullable: false },
        implementation_id,
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

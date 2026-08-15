use std::mem::size_of;

use encoding_rs::{EncoderResult, SHIFT_JIS};

use crate::api::{
    AbiSchema, ApiArgumentSchema, ApiBehavior, ApiCallFrame, ApiContract, ApiDeferredCall,
    ApiDirection, ApiDispatcher, ApiDynamicPointerConstraint, ApiEndpointIdentity, ApiFault,
    ApiGuestCallbackCall, ApiGuestWrite, ApiGuestWriteReceipt, ApiHandlerResult, ApiImmediate,
    ApiPointeeSchema, ApiPointerConstraint, ApiReturnSchema, ApiReturnValue, ApiStringEncoding,
    ApiSymbol, ApiValueSchema, ApiWriteRequirement, CallbackPolicy, CallingConvention,
    DispatchOutcome, ExecutionSession, GuestCallbackArgument, GuestCallbackCompletionPolicy,
    GuestCallbackRequest, HostCompletion, LastErrorPolicy, ReentrancyPolicy, StackCleanup,
};
use crate::dynamic_exports::DYNAMIC_EXPORT_ENDPOINT_NAMESPACE;
use crate::memory::{GuestAddress, MemoryAccess};
use crate::runtime::{
    LOGICAL_ENCODED_POINTER_NAMESPACE, LOGICAL_LOADED_MODULE_NAMESPACE,
    LOGICAL_PRIVATE_HEAP_NAMESPACE, LOGICAL_STANDARD_HANDLE_NAMESPACE,
    RUNTIME_AUDIO_NOTIFICATION_LIMIT, RUNTIME_AUDIO_NOTIFICATION_PACING_SERVICE,
    RUNTIME_CURRENT_THREAD_PSEUDO_HANDLE, RUNTIME_EVENT_OBJECT_NAMESPACE,
    RUNTIME_FILE_SEARCH_NAMESPACE, RUNTIME_PRIVATE_HEAP_ALIGNMENT,
    RUNTIME_PRIVATE_HEAP_ARENA_STRIDE, RUNTIME_PROCESS_IMAGE_PATH_A_ENCODING,
    RUNTIME_SOURCE_FILE_NAMESPACE, RUNTIME_SOURCE_READ_MAX_BYTES, RUNTIME_SOURCE_READ_SERVICE,
    RUNTIME_THREAD_OBJECT_NAMESPACE, RUNTIME_WAITABLE_TIMER_OBJECT_NAMESPACE,
    RUNTIME_WAITABLE_TIMER_PACING_SERVICE, RUNTIME_WRITABLE_COMMIT_SERVICE,
    RUNTIME_WRITABLE_DIRECTORY_CREATE_SERVICE, RUNTIME_WRITABLE_FILE_MAX_IO_BYTES,
    RUNTIME_WRITABLE_READ_SERVICE, RuntimeApiBoundaryDisposition, RuntimeCreateThreadRequest,
    RuntimeCriticalSection, RuntimeFileBacking, RuntimeFileSearchEntry, RuntimeFileSearchEntryKind,
    RuntimeHandle, RuntimeHeapInitialization, RuntimeHostCompletion, RuntimeMutexNameA,
    RuntimeObjectKind, RuntimePathResolution, RuntimeSourceReadHostResponse,
    RuntimeStateErrorClass, RuntimeThreadPriority, RuntimeVirtualMemoryReserveRequest,
    RuntimeWaitDisposition, RuntimeWaitForSingleObjectExBegin, RuntimeWaitForSingleObjectExOutcome,
    RuntimeWaitForSingleObjectExResult, RuntimeWaitState, RuntimeWritableCommitHostResponse,
    RuntimeWritableDirectoryCreateHostResponse, X86_CRITICAL_SECTION_BYTE_LENGTH,
    X86_STARTUP_INFO_W_BYTE_LENGTH,
};

mod policy;

pub use policy::{
    KERNEL32_COMPATIBILITY_POLICY_SCHEMA, KERNEL32_COMPATIBILITY_POLICY_VERSION,
    Kernel32CodePageInfoPolicy, Kernel32CompatibilityPolicy, Kernel32CompatibilityPolicyError,
    Kernel32CreateMutexAPolicy, Kernel32CreateThreadPolicy, Kernel32CreateWaitableTimerAPolicy,
    Kernel32DeleteFileAPolicy, Kernel32FindFileAPolicy, Kernel32GetDriveTypeAPolicy,
    Kernel32GetFileAttributesAPolicy, Kernel32GetModuleFileNameAPolicy,
    Kernel32GetStringTypeWPolicy, Kernel32HeapAllocationPolicy, Kernel32HeapAllocatorPolicy,
    Kernel32InitializeCriticalSectionPolicy, Kernel32LcMapStringWPolicy,
    Kernel32LoadLibraryAPolicy, Kernel32MemoryStatusPolicy, Kernel32MultiByteToWideCharPolicy,
    Kernel32OsVersionInfoAPolicy, Kernel32PerformanceCounterPolicy, Kernel32ProcessorFeaturePolicy,
    Kernel32RuntimeSystemModulePolicy, Kernel32SetCurrentDirectoryAPolicy,
    Kernel32SetWaitableTimerPolicy, Kernel32SourceFilePolicy, Kernel32StandardHandleFileType,
    Kernel32SystemInfoPolicy, Kernel32ThreadAffinityPolicy, Kernel32ThreadPriorityPolicy,
    Kernel32TickCountPolicy, Kernel32VirtualAllocPolicy, Kernel32WaitForSingleObjectExPolicy,
    Kernel32WideCharToMultiBytePolicy,
};

pub const GET_SYSTEM_TIME_AS_FILE_TIME_IMPLEMENTATION: &str =
    "kernel32.get-system-time-as-file-time.v1";
pub const GET_SYSTEM_TIME_IMPLEMENTATION: &str = "kernel32.get-system-time.v1";
pub const RAISE_EXCEPTION_IMPLEMENTATION: &str = "kernel32.raise-exception.seh-dispatch.v1";
pub const RTL_UNWIND_IMPLEMENTATION: &str = "kernel32.rtl-unwind.current-seh-frame.v1";
pub const GET_CURRENT_PROCESS_ID_IMPLEMENTATION: &str = "kernel32.get-current-process-id.v1";
pub const GET_CURRENT_THREAD_ID_IMPLEMENTATION: &str = "kernel32.get-current-thread-id.v1";
pub const GET_CURRENT_THREAD_IMPLEMENTATION: &str = "kernel32.get-current-thread.v1";
pub const SET_THREAD_AFFINITY_MASK_IMPLEMENTATION: &str = "kernel32.set-thread-affinity-mask.v1";
pub const SET_THREAD_PRIORITY_IMPLEMENTATION: &str = "kernel32.set-thread-priority.v1";
pub const CREATE_THREAD_IMPLEMENTATION: &str = "kernel32.create-thread.v1";
pub const RESUME_THREAD_IMPLEMENTATION: &str = "kernel32.resume-thread.v1";
pub const GET_SYSTEM_INFO_IMPLEMENTATION: &str = "kernel32.get-system-info.v1";
pub const GLOBAL_MEMORY_STATUS_IMPLEMENTATION: &str =
    "kernel32.global-memory-status.x86-deterministic.v1";
pub const VIRTUAL_ALLOC_IMPLEMENTATION: &str = "kernel32.virtual-alloc.reserve-top-down.v1";
pub const GET_VERSION_EX_A_IMPLEMENTATION: &str = "kernel32.get-version-ex-a.v1";
pub const GET_LAST_ERROR_IMPLEMENTATION: &str = "kernel32.get-last-error.v1";
pub const SET_LAST_ERROR_IMPLEMENTATION: &str = "kernel32.set-last-error.v1";
pub const GET_ACP_IMPLEMENTATION: &str = "kernel32.get-acp.v1";
pub const GET_SYSTEM_DEFAULT_LANG_ID_IMPLEMENTATION: &str =
    "kernel32.get-system-default-lang-id.v1";
pub const GET_DRIVE_TYPE_A_IMPLEMENTATION: &str = "kernel32.get-drive-type-a.v1";
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
pub const GET_FILE_ATTRIBUTES_A_IMPLEMENTATION: &str =
    "kernel32.get-file-attributes-a.runtime-vfs.v1";
pub const FIND_FIRST_FILE_A_IMPLEMENTATION: &str =
    "kernel32.find-first-file-a.runtime-vfs-windows-932-lexical.v1";
pub const FIND_NEXT_FILE_A_IMPLEMENTATION: &str =
    "kernel32.find-next-file-a.runtime-vfs-windows-932-lexical.v1";
pub const FIND_CLOSE_IMPLEMENTATION: &str = "kernel32.find-close.runtime-vfs.v1";
pub const DELETE_FILE_A_IMPLEMENTATION: &str = "kernel32.delete-file-a.missing-runtime-vfs.v1";
pub const CREATE_DIRECTORY_A_IMPLEMENTATION: &str = "kernel32.create-directory-a.runtime-vfs.v1";
pub const CREATE_FILE_A_IMPLEMENTATION: &str = "kernel32.create-file-a.source.v1";
pub const GET_FILE_SIZE_IMPLEMENTATION: &str = "kernel32.get-file-size.source.v1";
pub const READ_FILE_IMPLEMENTATION: &str = "kernel32.read-file.source.v1";
pub const WRITE_FILE_IMPLEMENTATION: &str = "kernel32.write-file.profile.v1";
pub const FLUSH_FILE_BUFFERS_IMPLEMENTATION: &str = "kernel32.flush-file-buffers.profile.v1";
pub const SET_FILE_POINTER_IMPLEMENTATION: &str = "kernel32.set-file-pointer.source.v1";
pub const CLOSE_HANDLE_IMPLEMENTATION: &str = "kernel32.close-handle.v2";
pub const CREATE_MUTEX_A_IMPLEMENTATION: &str = "kernel32.create-mutex-a.v1";
pub const CREATE_EVENT_A_IMPLEMENTATION: &str = "kernel32.create-event-a.unnamed.v1";
pub const SET_EVENT_IMPLEMENTATION: &str = "kernel32.set-event.v1";
pub const RESET_EVENT_IMPLEMENTATION: &str = "kernel32.reset-event.v1";
pub const WAIT_FOR_SINGLE_OBJECT_IMPLEMENTATION: &str = "kernel32.wait-for-single-object.event.v1";
pub const WAIT_FOR_MULTIPLE_OBJECTS_IMPLEMENTATION: &str =
    "kernel32.wait-for-multiple-objects.dsound-events.v1";
pub const CREATE_WAITABLE_TIMER_A_IMPLEMENTATION: &str = "kernel32.create-waitable-timer-a.v1";
pub const SET_WAITABLE_TIMER_IMPLEMENTATION: &str =
    "kernel32.set-waitable-timer.relative-one-shot.v1";
pub const WAIT_FOR_SINGLE_OBJECT_EX_IMPLEMENTATION: &str =
    "kernel32.wait-for-single-object-ex.waitable-timer.v1";
pub const SLEEP_ZERO_IMPLEMENTATION: &str = "kernel32.sleep-zero.yield-current-thread.v1";
pub const RUNTIME_SLEEP_DELAY_SERVICE: &str = "runtime.sleep-delay.v1";
pub const GET_STRING_TYPE_W_IMPLEMENTATION: &str = "kernel32.get-string-type-w.v1";
pub const LC_MAP_STRING_W_IMPLEMENTATION: &str = "kernel32.lc-map-string-w.v1";
pub const QUERY_PERFORMANCE_FREQUENCY_IMPLEMENTATION: &str =
    "kernel32.query-performance-frequency.v1";
pub const QUERY_PERFORMANCE_COUNTER_IMPLEMENTATION: &str = "kernel32.query-performance-counter.v1";
pub const GET_STARTUP_INFO_W_IMPLEMENTATION: &str = "kernel32.get-startup-info-w.v1";
pub const HEAP_SET_INFORMATION_IMPLEMENTATION: &str = "kernel32.heap-set-information.v1";
pub const HEAP_CREATE_IMPLEMENTATION: &str = "kernel32.heap-create.v1";
pub const HEAP_ALLOC_IMPLEMENTATION: &str = "kernel32.heap-alloc.v2";
pub const HEAP_REALLOC_IMPLEMENTATION: &str = "kernel32.heap-realloc.move.v1";
pub const HEAP_FREE_IMPLEMENTATION: &str = "kernel32.heap-free.v1";
pub const HEAP_SIZE_IMPLEMENTATION: &str = "kernel32.heap-size.v1";
pub const LOAD_LIBRARY_A_IMPLEMENTATION: &str = "kernel32.load-library-a.runtime-system.v1";
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
pub const DELETE_CRITICAL_SECTION_IMPLEMENTATION: &str = "kernel32.delete-critical-section.v1";

const HEAP_ENABLE_TERMINATION_ON_CORRUPTION: u32 = 1;
const HEAP_ZERO_MEMORY: u32 = 0x0000_0008;
const MAX_MODULE_NAME_CODE_UNITS: usize = 64;
const MAX_LOAD_LIBRARY_MODULE_NAME_BYTES: usize = 260;
const MAX_EXPORT_NAME_BYTES: usize = 64;
const CP_ACP: u32 = 0;
const WINDOWS_932_CODE_PAGE: u32 = 932;
const ERROR_SUCCESS: u32 = 0;
const ERROR_FILE_NOT_FOUND: u32 = 2;
const ERROR_PATH_NOT_FOUND: u32 = 3;
const ERROR_ACCESS_DENIED: u32 = 5;
const ERROR_INVALID_HANDLE: u32 = 6;
const ERROR_NO_MORE_FILES: u32 = 18;
const ERROR_NOT_ENOUGH_MEMORY: u32 = 8;
const ERROR_SHARING_VIOLATION: u32 = 32;
const ERROR_DISK_FULL: u32 = 112;
const ERROR_INVALID_PARAMETER: u32 = 87;
const ERROR_INSUFFICIENT_BUFFER: u32 = 122;
const ERROR_INVALID_NAME: u32 = 123;
const ERROR_MOD_NOT_FOUND: u32 = 126;
const ERROR_ALREADY_EXISTS: u32 = 183;
const ERROR_NEGATIVE_SEEK: u32 = 131;
const INVALID_HANDLE_VALUE: u32 = u32::MAX;
const WAIT_OBJECT_0: u32 = 0;
const WAIT_TIMEOUT: u32 = 258;
const WAIT_FAILED: u32 = u32::MAX;
const EXCEPTION_NONCONTINUABLE: u32 = 1;
const EXCEPTION_UNWINDING: u32 = 2;
const EXCEPTION_CONTINUE_EXECUTION: u32 = 0;
const EXCEPTION_CONTINUE_SEARCH: u32 = 1;
const EXCEPTION_NESTED_EXCEPTION: u32 = 2;
const EXCEPTION_COLLIDED_UNWIND: u32 = 3;
const EXCEPTION_MAXIMUM_PARAMETERS: usize = 15;
const X86_EXCEPTION_RECORD_BYTES: usize = 80;
const X86_CONTEXT_BYTES: usize = 716;
const X86_CONTEXT_OFFSET: u32 = X86_EXCEPTION_RECORD_BYTES as u32;
const X86_CONTEXT_FULL_CONTROL_INTEGER_SEGMENTS: u32 = 0x0001_0007;
const X86_OS_VERSION_INFO_A_BASE_BYTES: u32 = 148;
const X86_OS_VERSION_INFO_EX_A_BYTES: u32 = 156;
const X86_WIN32_FIND_DATA_A_BYTES: usize = 320;
const X86_WIN32_FIND_DATA_A_FILE_NAME_OFFSET: usize = 44;
const X86_WIN32_FIND_DATA_A_FILE_NAME_CAPACITY: usize = 260;
pub const MEM_RESERVE: u32 = 0x0000_2000;
pub const MEM_TOP_DOWN: u32 = 0x0010_0000;
pub const PAGE_READWRITE: u32 = 0x0000_0004;

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
    dispatch_kernel32_api_with_prevalidated_policy(policy, contract, frame, session)
}

pub fn dispatch_kernel32_api_with_prevalidated_policy(
    policy: &Kernel32CompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> ApiHandlerResult {
    if contract.implementation_id == RAISE_EXCEPTION_IMPLEMENTATION {
        return handle_raise_exception(contract, frame, session);
    }
    if contract.implementation_id == RTL_UNWIND_IMPLEMENTATION {
        return handle_rtl_unwind(contract, frame, session);
    }
    if contract.implementation_id == READ_FILE_IMPLEMENTATION {
        return handle_read_file(policy, contract, frame, session);
    }
    if contract.implementation_id == FLUSH_FILE_BUFFERS_IMPLEMENTATION {
        return handle_flush_file_buffers(policy, contract, frame, session);
    }
    if contract.implementation_id == CREATE_DIRECTORY_A_IMPLEMENTATION {
        return handle_create_directory_a(policy, contract, frame, session);
    }
    if contract.implementation_id == CLOSE_HANDLE_IMPLEMENTATION {
        return handle_close_handle(policy, contract, frame, session);
    }
    if contract.implementation_id == WAIT_FOR_SINGLE_OBJECT_EX_IMPLEMENTATION {
        return handle_wait_for_single_object_ex(policy, contract, frame, session);
    }
    if contract.implementation_id == WAIT_FOR_MULTIPLE_OBJECTS_IMPLEMENTATION {
        return handle_wait_for_multiple_objects(contract, frame, session);
    }
    if contract.implementation_id == SLEEP_ZERO_IMPLEMENTATION {
        return handle_sleep(policy, contract, frame, session);
    }
    let result = match contract.implementation_id.as_str() {
        GET_SYSTEM_TIME_AS_FILE_TIME_IMPLEMENTATION => {
            handle_get_system_time_as_file_time(contract, frame, session)
        }
        GET_SYSTEM_TIME_IMPLEMENTATION => handle_get_system_time(contract, frame, session),
        GET_CURRENT_PROCESS_ID_IMPLEMENTATION => handle_get_current_process_id(session),
        GET_CURRENT_THREAD_ID_IMPLEMENTATION => handle_get_current_thread_id(session),
        GET_CURRENT_THREAD_IMPLEMENTATION => handle_get_current_thread(contract, session),
        SET_THREAD_AFFINITY_MASK_IMPLEMENTATION => {
            handle_set_thread_affinity_mask(policy, contract, frame, session)
        }
        SET_THREAD_PRIORITY_IMPLEMENTATION => {
            handle_set_thread_priority(policy, contract, frame, session)
        }
        CREATE_THREAD_IMPLEMENTATION => handle_create_thread(policy, contract, frame, session),
        RESUME_THREAD_IMPLEMENTATION => handle_resume_thread(contract, frame, session),
        GET_SYSTEM_INFO_IMPLEMENTATION => handle_get_system_info(policy, contract, frame),
        GLOBAL_MEMORY_STATUS_IMPLEMENTATION => handle_global_memory_status(policy, contract, frame),
        VIRTUAL_ALLOC_IMPLEMENTATION => handle_virtual_alloc(policy, contract, frame, session),
        GET_VERSION_EX_A_IMPLEMENTATION => {
            handle_get_version_ex_a(policy, contract, frame, session)
        }
        GET_LAST_ERROR_IMPLEMENTATION => handle_get_last_error(contract, session),
        SET_LAST_ERROR_IMPLEMENTATION => handle_set_last_error(contract, frame),
        GET_ACP_IMPLEMENTATION => handle_get_acp(policy),
        GET_SYSTEM_DEFAULT_LANG_ID_IMPLEMENTATION => handle_get_system_default_lang_id(policy),
        GET_DRIVE_TYPE_A_IMPLEMENTATION => {
            handle_get_drive_type_a(policy, contract, frame, session)
        }
        GET_CP_INFO_IMPLEMENTATION => handle_get_cp_info(policy, contract, frame),
        IS_PROCESSOR_FEATURE_PRESENT_IMPLEMENTATION => {
            handle_is_processor_feature_present(policy, contract, frame)
        }
        SET_UNHANDLED_EXCEPTION_FILTER_IMPLEMENTATION => {
            handle_set_unhandled_exception_filter(contract, frame, session)
        }
        GET_TICK_COUNT_IMPLEMENTATION => handle_get_tick_count(policy, contract, session),
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
        GET_FILE_ATTRIBUTES_A_IMPLEMENTATION => {
            handle_get_file_attributes_a(policy, contract, frame, session)
        }
        FIND_FIRST_FILE_A_IMPLEMENTATION => {
            handle_find_first_file_a(policy, contract, frame, session)
        }
        FIND_NEXT_FILE_A_IMPLEMENTATION => {
            handle_find_next_file_a(policy, contract, frame, session)
        }
        FIND_CLOSE_IMPLEMENTATION => handle_find_close(policy, contract, frame, session),
        DELETE_FILE_A_IMPLEMENTATION => handle_delete_file_a(policy, contract, frame, session),
        CREATE_FILE_A_IMPLEMENTATION => handle_create_file_a(policy, contract, frame, session),
        GET_FILE_SIZE_IMPLEMENTATION => handle_get_file_size(policy, contract, frame, session),
        WRITE_FILE_IMPLEMENTATION => handle_write_file(policy, contract, frame, session),
        SET_FILE_POINTER_IMPLEMENTATION => {
            handle_set_file_pointer(policy, contract, frame, session)
        }
        CREATE_MUTEX_A_IMPLEMENTATION => handle_create_mutex_a(policy, contract, frame, session),
        CREATE_EVENT_A_IMPLEMENTATION => handle_create_event_a(contract, frame, session),
        SET_EVENT_IMPLEMENTATION => handle_set_event(contract, frame, session, true),
        RESET_EVENT_IMPLEMENTATION => handle_set_event(contract, frame, session, false),
        WAIT_FOR_SINGLE_OBJECT_IMPLEMENTATION => {
            handle_wait_for_single_object(contract, frame, session)
        }
        CREATE_WAITABLE_TIMER_A_IMPLEMENTATION => {
            handle_create_waitable_timer_a(policy, contract, frame, session)
        }
        SET_WAITABLE_TIMER_IMPLEMENTATION => {
            handle_set_waitable_timer(policy, contract, frame, session)
        }
        GET_STRING_TYPE_W_IMPLEMENTATION => {
            handle_get_string_type_w(policy, contract, frame, session)
        }
        LC_MAP_STRING_W_IMPLEMENTATION => handle_lc_map_string_w(policy, contract, frame, session),
        QUERY_PERFORMANCE_FREQUENCY_IMPLEMENTATION => {
            handle_query_performance_frequency(contract, frame, session)
        }
        QUERY_PERFORMANCE_COUNTER_IMPLEMENTATION => {
            handle_query_performance_counter(policy, contract, frame, session)
        }
        GET_STARTUP_INFO_W_IMPLEMENTATION => handle_get_startup_info_w(contract, frame, session),
        HEAP_SET_INFORMATION_IMPLEMENTATION => {
            handle_heap_set_information(contract, frame, session)
        }
        HEAP_CREATE_IMPLEMENTATION => handle_heap_create(contract, frame, session),
        HEAP_ALLOC_IMPLEMENTATION => handle_heap_alloc(policy, contract, frame, session),
        HEAP_REALLOC_IMPLEMENTATION => handle_heap_realloc(policy, contract, frame, session),
        HEAP_FREE_IMPLEMENTATION => handle_heap_free(policy, contract, frame, session),
        HEAP_SIZE_IMPLEMENTATION => handle_heap_size(policy, contract, frame, session),
        LOAD_LIBRARY_A_IMPLEMENTATION => handle_load_library_a(policy, contract, frame, session),
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
        DELETE_CRITICAL_SECTION_IMPLEMENTATION => {
            handle_delete_critical_section(contract, frame, session)
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
        "GetSystemTime" => get_system_time_contract(endpoint)?,
        "RaiseException" => raise_exception_contract(endpoint)?,
        "RtlUnwind" => rtl_unwind_contract(endpoint)?,
        "GetCurrentProcessId" => get_current_process_id_contract(endpoint)?,
        "GetCurrentThreadId" => get_current_thread_id_contract(endpoint)?,
        "GetCurrentThread" => get_current_thread_contract(endpoint)?,
        "SetThreadAffinityMask" => set_thread_affinity_mask_contract(endpoint)?,
        "SetThreadPriority" => set_thread_priority_contract(endpoint)?,
        "CreateThread" => create_thread_contract(endpoint)?,
        "ResumeThread" => resume_thread_contract(endpoint)?,
        "GetSystemInfo" => get_system_info_contract(endpoint)?,
        "GlobalMemoryStatus" => global_memory_status_contract(endpoint)?,
        "VirtualAlloc" => virtual_alloc_contract(endpoint)?,
        "GetVersionExA" => get_version_ex_a_contract(endpoint)?,
        "GetLastError" => get_last_error_contract(endpoint)?,
        "SetLastError" => set_last_error_contract(endpoint)?,
        "GetACP" => get_acp_contract(endpoint)?,
        "GetSystemDefaultLangID" => get_system_default_lang_id_contract(endpoint)?,
        "GetDriveTypeA" => get_drive_type_a_contract(endpoint)?,
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
        "GetFileAttributesA" => get_file_attributes_a_contract(endpoint)?,
        "FindFirstFileA" => find_first_file_a_contract(endpoint)?,
        "FindNextFileA" => find_next_file_a_contract(endpoint)?,
        "FindClose" => find_close_contract(endpoint)?,
        "DeleteFileA" => delete_file_a_contract(endpoint)?,
        "CreateDirectoryA" => create_directory_a_contract(endpoint)?,
        "CreateFileA" => create_file_a_contract(endpoint)?,
        "GetFileSize" => get_file_size_contract(endpoint)?,
        "ReadFile" => read_file_contract(endpoint)?,
        "WriteFile" => write_file_contract(endpoint)?,
        "FlushFileBuffers" => flush_file_buffers_contract(endpoint)?,
        "SetFilePointer" => set_file_pointer_contract(endpoint)?,
        "CloseHandle" => close_handle_contract(endpoint)?,
        "CreateMutexA" => create_mutex_a_contract(endpoint)?,
        "CreateEventA" => create_event_a_contract(endpoint)?,
        "SetEvent" => set_event_contract(endpoint)?,
        "ResetEvent" => reset_event_contract(endpoint)?,
        "WaitForSingleObject" => wait_for_single_object_contract(endpoint)?,
        "WaitForMultipleObjects" => wait_for_multiple_objects_contract(endpoint)?,
        "CreateWaitableTimerA" => create_waitable_timer_a_contract(endpoint)?,
        "SetWaitableTimer" => set_waitable_timer_contract(endpoint)?,
        "WaitForSingleObjectEx" => wait_for_single_object_ex_contract(endpoint)?,
        "Sleep" => sleep_contract(endpoint)?,
        "GetStringTypeW" => get_string_type_w_contract(endpoint)?,
        "LCMapStringW" => lc_map_string_w_contract(endpoint)?,
        "QueryPerformanceFrequency" => query_performance_frequency_contract(endpoint)?,
        "QueryPerformanceCounter" => query_performance_counter_contract(endpoint)?,
        "GetStartupInfoW" => get_startup_info_w_contract(endpoint)?,
        "HeapSetInformation" => heap_set_information_contract(endpoint)?,
        "HeapCreate" => heap_create_contract(endpoint)?,
        "HeapAlloc" => heap_alloc_contract(endpoint)?,
        "HeapReAlloc" => heap_realloc_contract(endpoint)?,
        "HeapFree" => heap_free_contract(endpoint)?,
        "HeapSize" => heap_size_contract(endpoint)?,
        "LoadLibraryA" => load_library_a_contract(endpoint)?,
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
        "DeleteCriticalSection" => delete_critical_section_contract(endpoint)?,
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

fn handle_raise_exception(
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> ApiHandlerResult {
    match prepare_raise_exception_callback(contract, frame, session) {
        Ok(request) => ApiHandlerResult::GuestCallback(request),
        Err(fault) => ApiHandlerResult::Fault(fault),
    }
}

fn handle_rtl_unwind(
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> ApiHandlerResult {
    match rtl_unwind_current_seh_frame(contract, frame, session) {
        Ok(immediate) => ApiHandlerResult::Immediate(immediate),
        Err(fault) => ApiHandlerResult::Fault(fault),
    }
}

fn rtl_unwind_current_seh_frame(
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let target_frame = GuestAddress(required_argument(contract, frame, 0, "target frame")?);
    let target_ip = GuestAddress(required_argument(contract, frame, 1, "target instruction")?);
    let exception_record = GuestAddress(required_argument(contract, frame, 2, "exception record")?);
    let return_value = required_argument(contract, frame, 3, "return value")?;
    if target_frame.0 == 0 {
        return Err(ApiFault::unsupported(
            "win32-seh",
            "rtl-unwind-exit-unwind-unsupported",
            "RtlUnwind exit unwind is not implemented",
        ));
    }
    if target_ip != frame.return_address {
        return Err(ApiFault::unsupported(
            "win32-seh",
            "rtl-unwind-nonlocal-target-unsupported",
            "RtlUnwind currently supports the reached continuation at the API return address",
        ));
    }
    session
        .memory()
        .validate_access(target_ip, 1, MemoryAccess::Execute)
        .map_err(|error| ApiFault::from_memory(error, &contract.endpoint))?;
    if exception_record.0 == 0 {
        return Err(ApiFault::unsupported(
            "win32-seh",
            "rtl-unwind-generated-exception-record-unsupported",
            "RtlUnwind with a generated exception record is not implemented",
        ));
    }
    let teb_address = session
        .runtime()
        .current_thread_teb_address()
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    let current_head = GuestAddress(read_guest_u32(session, teb_address, &contract.endpoint)?);
    if current_head != target_frame {
        return Err(ApiFault::unsupported(
            "win32-seh",
            "rtl-unwind-intermediate-frames-unsupported",
            "RtlUnwind currently supports a target equal to the current SEH chain head",
        ));
    }
    let registration = read_x86_exception_registration(session, target_frame, &contract.endpoint)?;
    let exception_bytes = session
        .memory()
        .read(exception_record, X86_EXCEPTION_RECORD_BYTES)
        .map_err(|error| ApiFault::from_memory(error, &contract.endpoint))?;
    session
        .memory()
        .validate_access(
            exception_record,
            X86_EXCEPTION_RECORD_BYTES,
            MemoryAccess::Write,
        )
        .map_err(|error| ApiFault::from_memory(error, &contract.endpoint))?;
    let exception_flags = u32::from_le_bytes(
        exception_bytes[4..8]
            .try_into()
            .expect("exception flags word"),
    );
    if exception_flags & !EXCEPTION_NONCONTINUABLE != 0 {
        return Err(ApiFault::unsupported(
            "win32-seh",
            "rtl-unwind-exception-flags-unsupported",
            "RtlUnwind currently supports the reached noncontinuable dispatch record",
        ));
    }
    let exception_flags_address = exception_record
        .0
        .checked_add(4)
        .map(GuestAddress)
        .ok_or_else(|| {
            ApiFault::invalid_call(
                "win32-seh",
                "rtl-unwind-exception-flags-address-overflow",
                "RtlUnwind exception flags cross the x86 address limit",
            )
        })?;
    let writes = [
        ApiGuestWrite {
            address: teb_address,
            bytes: registration.next.0.to_le_bytes().to_vec(),
        },
        ApiGuestWrite {
            address: exception_flags_address,
            bytes: (exception_flags | EXCEPTION_UNWINDING)
                .to_le_bytes()
                .to_vec(),
        },
    ];
    let receipts = session
        .commit_indirect_guest_writes(&writes)
        .map_err(|error| ApiFault::from_memory(error, &contract.endpoint))?;
    let mut immediate = ApiImmediate::new(ApiReturnValue::Void).with_void_eax(return_value);
    for receipt in receipts {
        immediate = immediate.with_committed_guest_write(receipt);
    }
    Ok(immediate)
}

fn prepare_raise_exception_callback(
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &ExecutionSession<'_>,
) -> Result<GuestCallbackRequest, ApiFault> {
    let exception_code = required_argument(contract, frame, 0, "exception code")?;
    let exception_flags = required_argument(contract, frame, 1, "exception flags")?;
    let argument_count = required_argument(contract, frame, 2, "exception argument count")?;
    let arguments_address = required_argument(contract, frame, 3, "exception arguments")?;
    if exception_flags != EXCEPTION_NONCONTINUABLE {
        return Err(ApiFault::unsupported(
            "win32-seh",
            "raise-exception-flags-unsupported",
            "RaiseException currently supports the reached noncontinuable exception variant",
        ));
    }
    let argument_count = usize::try_from(argument_count).map_err(|_| {
        ApiFault::resource_limit(
            "win32-seh",
            "exception-argument-count-unrepresentable",
            "RaiseException argument count does not fit host accounting",
        )
    })?;
    if argument_count > EXCEPTION_MAXIMUM_PARAMETERS {
        return Err(ApiFault::invalid_call(
            "win32-seh",
            "exception-argument-count-invalid",
            "RaiseException accepts at most 15 exception arguments",
        ));
    }
    let argument_bytes = argument_count.checked_mul(4).ok_or_else(|| {
        ApiFault::resource_limit(
            "win32-seh",
            "exception-argument-bytes-overflow",
            "RaiseException argument bytes overflowed host accounting",
        )
    })?;
    let exception_arguments = if argument_bytes == 0 {
        Vec::new()
    } else {
        if arguments_address == 0 {
            return Err(ApiFault::invalid_call(
                "win32-seh",
                "exception-arguments-null",
                "RaiseException requires a readable arguments array when the count is nonzero",
            ));
        }
        session
            .memory()
            .read(GuestAddress(arguments_address), argument_bytes)
            .map_err(|error| ApiFault::from_memory(error, &contract.endpoint))?
            .chunks_exact(4)
            .map(|bytes| u32::from_le_bytes(bytes.try_into().expect("four-byte chunk")))
            .collect::<Vec<_>>()
    };

    let context = session.x86_context().ok_or_else(|| {
        ApiFault::runtime_defect(
            "win32-seh",
            "x86-context-missing",
            "RaiseException dispatch requires the authoritative x86 register snapshot",
        )
    })?;
    if context.esp != frame.stack_pointer.0 || context.eip != contract.endpoint.address.0 {
        return Err(ApiFault::invalid_call(
            "win32-seh",
            "x86-context-api-frame-mismatch",
            "RaiseException register snapshot does not identify the trapped API frame",
        ));
    }
    let callback_return_sentinel = session.guest_callback_return_sentinel().ok_or_else(|| {
        ApiFault::runtime_defect(
            "win32-seh",
            "callback-return-sentinel-missing",
            "RaiseException dispatch requires the process callback return sentinel",
        )
    })?;
    let teb_address = session
        .runtime()
        .current_thread_teb_address()
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    let exception_list = read_guest_u32(session, teb_address, &contract.endpoint)?;
    if exception_list == u32::MAX {
        return Err(unhandled_exception_fault(exception_code));
    }
    let registration =
        read_x86_exception_registration(session, GuestAddress(exception_list), &contract.endpoint)?;
    let scratch = x86_exception_dispatch_scratch(
        exception_code,
        exception_flags,
        &exception_arguments,
        frame,
        context,
    )?;
    let request = exception_dispatch_callback_request(
        registration,
        exception_flags,
        scratch,
        callback_return_sentinel,
    );
    session.preflight_guest_callback(&ApiGuestCallbackCall {
        request: Box::new(request.clone()),
        contract: contract.clone(),
        frame: frame.clone(),
    })?;
    Ok(request)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct X86ExceptionRegistration {
    frame: GuestAddress,
    next: GuestAddress,
    handler: GuestAddress,
}

fn read_x86_exception_registration(
    session: &ExecutionSession<'_>,
    frame: GuestAddress,
    endpoint: &ApiEndpointIdentity,
) -> Result<X86ExceptionRegistration, ApiFault> {
    if frame.0 == u32::MAX || !frame.0.is_multiple_of(4) {
        return Err(ApiFault::invalid_call(
            "win32-seh",
            "exception-registration-address-invalid",
            "SEH registration frame must be aligned and may not be the chain terminator",
        ));
    }
    let stack = session
        .runtime()
        .current_thread_state()
        .map_err(|error| ApiFault::from_runtime(error, endpoint))?
        .stack
        .as_ref()
        .ok_or_else(|| {
            ApiFault::runtime_defect(
                "win32-seh",
                "thread-stack-missing",
                "SEH registration validation requires a registered x86 thread stack",
            )
        })?;
    let frame_end = u64::from(frame.0).checked_add(8).ok_or_else(|| {
        ApiFault::invalid_call(
            "win32-seh",
            "exception-registration-range-overflow",
            "SEH registration frame crosses the x86 address limit",
        )
    })?;
    if u64::from(frame.0) < u64::from(stack.committed_range.start().0)
        || frame_end > stack.committed_range.end()
    {
        return Err(ApiFault::invalid_call(
            "win32-seh",
            "exception-registration-outside-stack",
            "SEH registration frame is outside the current thread committed stack",
        ));
    }
    let bytes = session
        .memory()
        .read(frame, 8)
        .map_err(|error| ApiFault::from_memory(error, endpoint))?;
    let next = GuestAddress(u32::from_le_bytes(
        bytes[0..4].try_into().expect("next word"),
    ));
    let handler = GuestAddress(u32::from_le_bytes(
        bytes[4..8].try_into().expect("handler word"),
    ));
    if next.0 != u32::MAX {
        let next_end = u64::from(next.0).checked_add(8).ok_or_else(|| {
            ApiFault::invalid_call(
                "win32-seh",
                "exception-registration-next-overflow",
                "next SEH registration crosses the x86 address limit",
            )
        })?;
        if !next.0.is_multiple_of(4)
            || next.0 <= frame.0
            || u64::from(next.0) < u64::from(stack.committed_range.start().0)
            || next_end > stack.committed_range.end()
        {
            return Err(ApiFault::invalid_call(
                "win32-seh",
                "exception-registration-next-invalid",
                "SEH registration chain must advance monotonically within the committed stack",
            ));
        }
    }
    session
        .memory()
        .validate_access(handler, 1, MemoryAccess::Execute)
        .map_err(|error| ApiFault::from_memory(error, endpoint))?;
    Ok(X86ExceptionRegistration {
        frame,
        next,
        handler,
    })
}

fn read_guest_u32(
    session: &ExecutionSession<'_>,
    address: GuestAddress,
    endpoint: &ApiEndpointIdentity,
) -> Result<u32, ApiFault> {
    let bytes = session
        .memory()
        .read(address, 4)
        .map_err(|error| ApiFault::from_memory(error, endpoint))?;
    Ok(u32::from_le_bytes(
        bytes.try_into().expect("four-byte read"),
    ))
}

fn x86_exception_dispatch_scratch(
    exception_code: u32,
    exception_flags: u32,
    exception_arguments: &[u32],
    frame: &ApiCallFrame,
    context: crate::api::X86CpuContext,
) -> Result<Vec<u8>, ApiFault> {
    let mut scratch = vec![0; X86_EXCEPTION_RECORD_BYTES + X86_CONTEXT_BYTES];
    write_scratch_u32(&mut scratch, 0, exception_code);
    write_scratch_u32(&mut scratch, 4, exception_flags);
    write_scratch_u32(&mut scratch, 12, frame.return_address.0);
    write_scratch_u32(&mut scratch, 16, exception_arguments.len() as u32);
    for (index, value) in exception_arguments.iter().copied().enumerate() {
        write_scratch_u32(&mut scratch, 20 + index * 4, value);
    }

    let context_offset = X86_CONTEXT_OFFSET as usize;
    write_scratch_u32(
        &mut scratch,
        context_offset,
        X86_CONTEXT_FULL_CONTROL_INTEGER_SEGMENTS,
    );
    write_scratch_u32(&mut scratch, context_offset + 140, context.gs);
    write_scratch_u32(&mut scratch, context_offset + 144, context.fs);
    write_scratch_u32(&mut scratch, context_offset + 148, context.es);
    write_scratch_u32(&mut scratch, context_offset + 152, context.ds);
    write_scratch_u32(&mut scratch, context_offset + 156, context.edi);
    write_scratch_u32(&mut scratch, context_offset + 160, context.esi);
    write_scratch_u32(&mut scratch, context_offset + 164, context.ebx);
    write_scratch_u32(&mut scratch, context_offset + 168, context.edx);
    write_scratch_u32(&mut scratch, context_offset + 172, context.ecx);
    write_scratch_u32(&mut scratch, context_offset + 176, context.eax);
    write_scratch_u32(&mut scratch, context_offset + 180, context.ebp);
    write_scratch_u32(&mut scratch, context_offset + 184, frame.return_address.0);
    write_scratch_u32(&mut scratch, context_offset + 188, context.cs);
    write_scratch_u32(&mut scratch, context_offset + 192, context.eflags);
    let continuation_esp = frame
        .stack_pointer
        .0
        .checked_add(4)
        .and_then(|value| value.checked_add(frame.argument_bytes))
        .ok_or_else(|| {
            ApiFault::invalid_call(
                "win32-seh",
                "exception-context-stack-overflow",
                "RaiseException continuation stack crosses the x86 address limit",
            )
        })?;
    write_scratch_u32(&mut scratch, context_offset + 196, continuation_esp);
    write_scratch_u32(&mut scratch, context_offset + 200, context.ss);
    Ok(scratch)
}

fn write_scratch_u32(scratch: &mut [u8], offset: usize, value: u32) {
    scratch[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn exception_dispatch_callback_request(
    registration: X86ExceptionRegistration,
    exception_flags: u32,
    scratch: Vec<u8>,
    return_sentinel: GuestAddress,
) -> GuestCallbackRequest {
    GuestCallbackRequest {
        target: registration.handler,
        arguments: vec![
            GuestCallbackArgument::ScratchPointer { offset: 0 },
            GuestCallbackArgument::U32(registration.frame.0),
            GuestCallbackArgument::ScratchPointer {
                offset: X86_CONTEXT_OFFSET,
            },
            GuestCallbackArgument::U32(0),
        ],
        scratch,
        scratch_pointer_relocations: Vec::new(),
        return_sentinel,
        completion_policy: GuestCallbackCompletionPolicy::ExceptionDispatch {
            next_frame: registration.next,
            exception_flags,
        },
    }
}

fn unhandled_exception_fault(exception_code: u32) -> ApiFault {
    ApiFault::unsupported(
        "win32-seh",
        "unhandled-exception",
        format!("guest exception {exception_code:#010x} reached the end of the SEH chain"),
    )
}

pub(crate) fn resolve_exception_dispatch_callback(
    call: &ApiGuestCallbackCall,
    result: u32,
    session: &ExecutionSession<'_>,
) -> ApiHandlerResult {
    let GuestCallbackCompletionPolicy::ExceptionDispatch {
        next_frame,
        exception_flags,
    } = call.request.completion_policy
    else {
        return ApiHandlerResult::Fault(ApiFault::runtime_defect(
            "win32-seh",
            "exception-callback-policy-mismatch",
            "SEH callback continuation has the wrong completion policy",
        ));
    };
    match result {
        EXCEPTION_CONTINUE_SEARCH => {
            let exception_code = u32::from_le_bytes(
                call.request.scratch[0..4]
                    .try_into()
                    .expect("validated exception record"),
            );
            if next_frame.0 == u32::MAX {
                return ApiHandlerResult::Fault(unhandled_exception_fault(exception_code));
            }
            match read_x86_exception_registration(session, next_frame, &call.frame.endpoint) {
                Ok(registration) => {
                    ApiHandlerResult::GuestCallback(exception_dispatch_callback_request(
                        registration,
                        exception_flags,
                        call.request.scratch.clone(),
                        call.request.return_sentinel,
                    ))
                }
                Err(fault) => ApiHandlerResult::Fault(fault),
            }
        }
        EXCEPTION_CONTINUE_EXECUTION if exception_flags & EXCEPTION_NONCONTINUABLE != 0 => {
            ApiHandlerResult::Fault(ApiFault::invalid_call(
                "win32-seh",
                "noncontinuable-exception-continued",
                "guest SEH handler attempted to continue a noncontinuable exception",
            ))
        }
        EXCEPTION_CONTINUE_EXECUTION => ApiHandlerResult::Fault(ApiFault::unsupported(
            "win32-seh",
            "continuable-exception-resume-unsupported",
            "restoring a handler-modified x86 CONTEXT is not implemented",
        )),
        EXCEPTION_NESTED_EXCEPTION => ApiHandlerResult::Fault(ApiFault::unsupported(
            "win32-seh",
            "nested-exception-disposition-unsupported",
            "nested SEH exception disposition is not implemented",
        )),
        EXCEPTION_COLLIDED_UNWIND => ApiHandlerResult::Fault(ApiFault::unsupported(
            "win32-seh",
            "collided-unwind-disposition-unsupported",
            "collided SEH unwind disposition is not implemented",
        )),
        _ => ApiHandlerResult::Fault(ApiFault::invalid_call(
            "win32-seh",
            "exception-disposition-invalid",
            format!("guest SEH handler returned invalid disposition {result}"),
        )),
    }
}

fn handle_get_system_time(
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let output = required_argument(contract, frame, 0, "SYSTEMTIME output")?;
    Ok(ApiImmediate::new(ApiReturnValue::Void).with_guest_write(
        GuestAddress(output),
        system_time_bytes(session.runtime().clock().wall_clock_file_time),
    ))
}

fn system_time_bytes(file_time: u64) -> Vec<u8> {
    const TICKS_PER_MILLISECOND: u64 = 10_000;
    const MILLISECONDS_PER_DAY: u64 = 86_400_000;
    const DAYS_FROM_1601_TO_1970: i64 = 134_774;

    let milliseconds = file_time / TICKS_PER_MILLISECOND;
    let days_since_1601 = milliseconds / MILLISECONDS_PER_DAY;
    let milliseconds_in_day = milliseconds % MILLISECONDS_PER_DAY;
    let days_since_1970 = days_since_1601 as i64 - DAYS_FROM_1601_TO_1970;
    let (year, month, day) = civil_date_from_days_since_1970(days_since_1970);
    let hour = milliseconds_in_day / 3_600_000;
    let minute = (milliseconds_in_day % 3_600_000) / 60_000;
    let second = (milliseconds_in_day % 60_000) / 1_000;
    let millisecond = milliseconds_in_day % 1_000;
    let day_of_week = (days_since_1601 + 1) % 7;

    [
        year,
        month,
        day_of_week,
        day,
        hour,
        minute,
        second,
        millisecond,
    ]
    .into_iter()
    .flat_map(|value| (value as u16).to_le_bytes())
    .collect()
}

fn civil_date_from_days_since_1970(days: i64) -> (u64, u64, u64) {
    let shifted_days = days + 719_468;
    let era = shifted_days.div_euclid(146_097);
    let day_of_era = shifted_days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    year += if month <= 2 { 1 } else { 0 };
    (year as u64, month as u64, day as u64)
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

fn handle_get_current_thread(
    contract: &ApiContract,
    session: &ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let resolution = session
        .runtime()
        .resolve_current_thread_pseudo_handle()
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    Ok(ApiImmediate::new(ApiReturnValue::U32(resolution.value)))
}

fn handle_virtual_alloc(
    policy: &Kernel32CompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let preferred_address = required_argument(contract, frame, 0, "preferred address")?;
    let byte_length = required_argument(contract, frame, 1, "reservation size")?;
    let allocation_type = required_argument(contract, frame, 2, "allocation type")?;
    let protection = required_argument(contract, frame, 3, "protection")?;
    let virtual_alloc = policy.virtual_alloc().ok_or_else(|| {
        ApiFault::unsupported(
            "win32-virtual-memory",
            "virtual-alloc-policy-disabled",
            format!(
                "compatibility policy {} does not enable VirtualAlloc",
                policy.id
            ),
        )
    })?;
    if (virtual_alloc.require_null_preferred_address && preferred_address != 0)
        || !virtual_alloc
            .allowed_reserve_byte_lengths
            .contains(&byte_length)
        || !virtual_alloc
            .allowed_allocation_types
            .contains(&allocation_type)
        || !virtual_alloc.allowed_protections.contains(&protection)
    {
        return Err(ApiFault::unsupported(
            "win32-virtual-memory",
            "virtual-alloc-variant-unsupported",
            format!(
                "compatibility policy {} does not enable VirtualAlloc({preferred_address:#010x}, {byte_length:#010x}, {allocation_type:#010x}, {protection:#010x})",
                policy.id
            ),
        ));
    }
    let system_info = policy.system_info().ok_or_else(|| {
        ApiFault::runtime_defect(
            "win32-virtual-memory",
            "virtual-alloc-system-info-missing",
            "validated VirtualAlloc policy has no x86 system-information profile",
        )
    })?;
    let reservation = session
        .reserve_virtual_memory(RuntimeVirtualMemoryReserveRequest {
            preferred_address: None,
            byte_length,
            minimum_application_address: GuestAddress(system_info.minimum_application_address),
            maximum_application_address: GuestAddress(system_info.maximum_application_address),
            allocation_granularity: system_info.allocation_granularity,
            top_down: allocation_type == (MEM_RESERVE | MEM_TOP_DOWN),
            protection: crate::memory::PageProtection {
                read: protection == PAGE_READWRITE,
                write: protection == PAGE_READWRITE,
                execute: false,
            },
        })
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    Ok(ApiImmediate::new(ApiReturnValue::OpaquePointer(
        reservation.range.base.0,
    )))
}

fn handle_set_thread_affinity_mask(
    policy: &Kernel32CompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let thread_handle = required_argument(contract, frame, 0, "thread handle")?;
    let requested_mask = required_argument(contract, frame, 1, "thread affinity mask")?;
    let process_mask = policy.thread_affinity_process_mask().ok_or_else(|| {
        ApiFault::unsupported(
            "win32-thread-affinity",
            "set-thread-affinity-mask-policy-disabled",
            format!(
                "compatibility policy {} does not enable thread affinity",
                policy.id
            ),
        )
    })?;
    if thread_handle == 0 {
        return Ok(ApiImmediate::new(ApiReturnValue::U32(0)).with_last_error(ERROR_INVALID_HANDLE));
    }
    if thread_handle != RUNTIME_CURRENT_THREAD_PSEUDO_HANDLE {
        return Err(ApiFault::unsupported(
            "win32-thread-affinity",
            "set-thread-affinity-mask-handle-unsupported",
            "SetThreadAffinityMask currently accepts only the current-thread pseudo-handle",
        ));
    }
    if requested_mask == 0 || requested_mask & !process_mask != 0 {
        return Ok(
            ApiImmediate::new(ApiReturnValue::U32(0)).with_last_error(ERROR_INVALID_PARAMETER)
        );
    }
    let preserved_last_error = session
        .last_error()
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    let previous_mask = session
        .runtime_mut()
        .set_current_thread_affinity_mask(process_mask, requested_mask)
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    Ok(ApiImmediate::new(ApiReturnValue::U32(previous_mask)).with_last_error(preserved_last_error))
}

fn handle_set_thread_priority(
    policy: &Kernel32CompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let thread_handle = required_argument(contract, frame, 0, "thread handle")?;
    let priority_value =
        i32::from_le_bytes(required_argument(contract, frame, 1, "thread priority")?.to_le_bytes());
    let Some(priority) = RuntimeThreadPriority::from_win32_value(priority_value) else {
        return Ok(ApiImmediate::new(ApiReturnValue::Bool32(false))
            .with_last_error(ERROR_INVALID_PARAMETER));
    };
    if !policy.allows_thread_priority(priority_value) {
        return Err(ApiFault::unsupported(
            "win32-thread-priority",
            "set-thread-priority-value-unsupported",
            format!(
                "compatibility policy {} does not enable thread priority {priority_value}",
                policy.id
            ),
        ));
    }
    if thread_handle == RUNTIME_CURRENT_THREAD_PSEUDO_HANDLE {
        return Err(ApiFault::unsupported(
            "win32-thread-priority",
            "set-thread-priority-pseudo-handle-unsupported",
            "SetThreadPriority currently requires a typed logical thread handle",
        ));
    }
    let preserved_last_error = session
        .last_error()
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    match session
        .runtime_mut()
        .set_thread_priority(RuntimeHandle(thread_handle), priority)
    {
        Ok(_) => {
            Ok(ApiImmediate::new(ApiReturnValue::Bool32(true))
                .with_last_error(preserved_last_error))
        }
        Err(error)
            if matches!(
                error.class,
                RuntimeStateErrorClass::InvalidInput
                    | RuntimeStateErrorClass::NotFound
                    | RuntimeStateErrorClass::Conflict
            ) =>
        {
            Ok(ApiImmediate::new(ApiReturnValue::Bool32(false))
                .with_last_error(ERROR_INVALID_HANDLE))
        }
        Err(error) => Err(ApiFault::from_runtime(error, &contract.endpoint)),
    }
}

fn handle_create_thread(
    policy: &Kernel32CompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    const CREATE_SUSPENDED: u32 = 0x0000_0004;

    let thread_policy = policy.create_thread().ok_or_else(|| {
        ApiFault::unsupported(
            "win32-thread-bootstrap",
            "create-thread-policy-disabled",
            format!(
                "compatibility policy {} does not enable guest thread creation",
                policy.id
            ),
        )
    })?;
    let security_attributes = required_argument(contract, frame, 0, "security attributes")?;
    let stack_size = required_argument(contract, frame, 1, "stack size")?;
    let start_address = required_argument(contract, frame, 2, "start address")?;
    let parameter = required_argument(contract, frame, 3, "thread parameter")?;
    let creation_flags = required_argument(contract, frame, 4, "creation flags")?;
    let thread_id_output = required_argument(contract, frame, 5, "thread ID output")?;
    let preserved_last_error = session
        .last_error()
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    let failed = |last_error| {
        ApiImmediate::new(ApiReturnValue::LogicalHandle {
            namespace: RUNTIME_THREAD_OBJECT_NAMESPACE.to_owned(),
            value: 0,
        })
        .with_last_error(last_error)
    };

    if security_attributes != 0
        || stack_size != 0
        || start_address == 0
        || !thread_policy
            .allowed_creation_flags
            .contains(&creation_flags)
    {
        return Ok(failed(ERROR_INVALID_PARAMETER));
    }
    if session.runtime().scheduler().thread_states().len() >= thread_policy.maximum_threads as usize
    {
        return Ok(failed(ERROR_NOT_ENOUGH_MEMORY));
    }
    frame
        .argument_bytes
        .checked_add(4)
        .and_then(|cleanup| frame.stack_pointer.0.checked_add(cleanup))
        .ok_or_else(|| {
            ApiFault::invalid_call(
                "win32-thread-bootstrap",
                "create-thread-return-stack-overflow",
                "CreateThread return cleanup crosses the x86 address limit",
            )
        })?;

    let result = match session.create_guest_thread(RuntimeCreateThreadRequest {
        stack_reserve_bytes: u64::from(thread_policy.default_stack_reserve_bytes),
        stack_commit_bytes: u64::from(thread_policy.default_stack_commit_bytes),
        entry_point: GuestAddress(start_address),
        parameter,
        thread_id_output: (thread_id_output != 0).then_some(GuestAddress(thread_id_output)),
        initial_suspend_count: u32::from(creation_flags & CREATE_SUSPENDED != 0),
    }) {
        Ok(result) => result,
        Err(error) if error.class == RuntimeStateErrorClass::ResourceLimit => {
            return Ok(failed(ERROR_NOT_ENOUGH_MEMORY));
        }
        Err(error)
            if matches!(
                error.class,
                RuntimeStateErrorClass::InvalidInput
                    | RuntimeStateErrorClass::NotFound
                    | RuntimeStateErrorClass::Conflict
            ) =>
        {
            return Ok(failed(ERROR_INVALID_PARAMETER));
        }
        Err(error) => return Err(ApiFault::from_runtime(error, &contract.endpoint)),
    };
    let mut immediate = ApiImmediate::new(ApiReturnValue::LogicalHandle {
        namespace: RUNTIME_THREAD_OBJECT_NAMESPACE.to_owned(),
        value: result.handle.0,
    })
    .with_last_error(preserved_last_error);
    if let (Some(address), Some(write)) = (result.thread_id_output, result.thread_id_write) {
        immediate = immediate.with_committed_guest_write(ApiGuestWriteReceipt {
            write: ApiGuestWrite {
                address,
                bytes: result.thread_id.0.to_le_bytes().to_vec(),
            },
            result: write,
        });
    }
    Ok(immediate)
}

fn handle_resume_thread(
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let handle = RuntimeHandle(required_argument(contract, frame, 0, "thread handle")?);
    let preserved_last_error = session
        .last_error()
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    if handle.kind() != Ok(RuntimeObjectKind::Thread)
        || session
            .runtime()
            .object_for_handle(handle, RuntimeObjectKind::Thread)
            .is_err()
    {
        return Ok(
            ApiImmediate::new(ApiReturnValue::U32(u32::MAX)).with_last_error(ERROR_INVALID_HANDLE)
        );
    }
    match session.runtime_mut().resume_thread(handle) {
        Ok(result) => Ok(
            ApiImmediate::new(ApiReturnValue::U32(result.previous_suspend_count))
                .with_last_error(preserved_last_error),
        ),
        Err(error) if error.class != RuntimeStateErrorClass::RuntimeDefect => {
            Ok(ApiImmediate::new(ApiReturnValue::U32(u32::MAX))
                .with_last_error(ERROR_INVALID_HANDLE))
        }
        Err(error) => Err(ApiFault::from_runtime(error, &contract.endpoint)),
    }
}

fn handle_get_system_info(
    policy: &Kernel32CompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
) -> Result<ApiImmediate, ApiFault> {
    let output = required_argument(contract, frame, 0, "SYSTEM_INFO output")?;
    let info = policy.system_info().ok_or_else(|| {
        ApiFault::unsupported(
            "win32-system-info",
            "get-system-info-policy-disabled",
            format!(
                "compatibility policy {} does not enable x86 system information",
                policy.id
            ),
        )
    })?;
    let active_processor_mask = policy.thread_affinity_process_mask().ok_or_else(|| {
        ApiFault::runtime_defect(
            "win32-system-info",
            "get-system-info-affinity-policy-missing",
            "validated x86 system information has no virtual process affinity mask",
        )
    })?;
    Ok(ApiImmediate::new(ApiReturnValue::Void).with_guest_write(
        GuestAddress(output),
        x86_system_info_bytes(&info, active_processor_mask),
    ))
}

fn x86_system_info_bytes(info: &Kernel32SystemInfoPolicy, active_processor_mask: u32) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(36);
    bytes.extend_from_slice(&info.processor_architecture.to_le_bytes());
    bytes.extend_from_slice(&0_u16.to_le_bytes());
    bytes.extend_from_slice(&info.page_size.to_le_bytes());
    bytes.extend_from_slice(&info.minimum_application_address.to_le_bytes());
    bytes.extend_from_slice(&info.maximum_application_address.to_le_bytes());
    bytes.extend_from_slice(&active_processor_mask.to_le_bytes());
    bytes.extend_from_slice(&active_processor_mask.count_ones().to_le_bytes());
    bytes.extend_from_slice(&info.processor_type.to_le_bytes());
    bytes.extend_from_slice(&info.allocation_granularity.to_le_bytes());
    bytes.extend_from_slice(&info.processor_level.to_le_bytes());
    bytes.extend_from_slice(&info.processor_revision.to_le_bytes());
    bytes
}

fn handle_global_memory_status(
    policy: &Kernel32CompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
) -> Result<ApiImmediate, ApiFault> {
    let output = required_argument(contract, frame, 0, "MEMORYSTATUS output")?;
    let status = policy.memory_status().ok_or_else(|| {
        ApiFault::unsupported(
            "win32-system-info",
            "global-memory-status-policy-disabled",
            format!(
                "compatibility policy {} does not enable x86 memory status",
                policy.id
            ),
        )
    })?;
    let system_info = policy.system_info().ok_or_else(|| {
        ApiFault::runtime_defect(
            "win32-system-info",
            "global-memory-status-system-info-missing",
            "validated x86 memory status has no virtual address profile",
        )
    })?;
    Ok(ApiImmediate::new(ApiReturnValue::Void).with_guest_write(
        GuestAddress(output),
        x86_memory_status_bytes(&status, &system_info),
    ))
}

fn x86_memory_status_bytes(
    status: &Kernel32MemoryStatusPolicy,
    system_info: &Kernel32SystemInfoPolicy,
) -> Vec<u8> {
    const X86_MEMORY_STATUS_BYTES: u32 = 32;

    let total_virtual_bytes = system_info
        .maximum_application_address
        .checked_sub(system_info.minimum_application_address)
        .and_then(|span| span.checked_add(1))
        .expect("validated x86 application-address span");
    let mut bytes = Vec::with_capacity(X86_MEMORY_STATUS_BYTES as usize);
    for value in [
        X86_MEMORY_STATUS_BYTES,
        status.memory_load_percent,
        status.total_physical_bytes,
        status.available_physical_bytes,
        status.total_page_file_bytes,
        status.available_page_file_bytes,
        total_virtual_bytes,
        status.available_virtual_bytes,
    ] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes
}

fn handle_get_version_ex_a(
    policy: &Kernel32CompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let output = GuestAddress(required_argument(
        contract,
        frame,
        0,
        "OSVERSIONINFOA output",
    )?);
    let version = policy.os_version_info_a().ok_or_else(|| {
        ApiFault::unsupported(
            "win32-os-version",
            "get-version-ex-a-policy-disabled",
            format!(
                "compatibility policy {} does not enable ANSI OS version information",
                policy.id
            ),
        )
    })?;
    let declared_size_bytes = session
        .memory()
        .read(output, size_of::<u32>())
        .map_err(|error| ApiFault::from_memory(error, &contract.endpoint))?;
    let declared_size = u32::from_le_bytes(
        declared_size_bytes
            .try_into()
            .expect("checked OSVERSIONINFOA size field has DWORD width"),
    );
    if !matches!(
        declared_size,
        X86_OS_VERSION_INFO_A_BASE_BYTES | X86_OS_VERSION_INFO_EX_A_BYTES
    ) {
        return Ok(ApiImmediate::new(ApiReturnValue::Bool32(false))
            .with_last_error(ERROR_INSUFFICIENT_BUFFER));
    }

    let preserved_last_error = session
        .last_error()
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    let mut bytes = session
        .memory()
        .read(output, declared_size as usize)
        .map_err(|error| ApiFault::from_memory(error, &contract.endpoint))?
        .to_vec();
    merge_os_version_info_a(&mut bytes, version);
    Ok(ApiImmediate::new(ApiReturnValue::Bool32(true))
        .with_guest_write(output, bytes)
        .with_last_error(preserved_last_error))
}

fn merge_os_version_info_a(bytes: &mut [u8], version: &Kernel32OsVersionInfoAPolicy) {
    bytes[4..8].copy_from_slice(&version.major_version.to_le_bytes());
    bytes[8..12].copy_from_slice(&version.minor_version.to_le_bytes());
    bytes[12..16].copy_from_slice(&version.build_number.to_le_bytes());
    bytes[16..20].copy_from_slice(&version.platform_id.to_le_bytes());
    let service_pack = version.service_pack.as_bytes();
    bytes[20..20 + service_pack.len()].copy_from_slice(service_pack);
    bytes[20 + service_pack.len()] = 0;
    if bytes.len() == X86_OS_VERSION_INFO_EX_A_BYTES as usize {
        bytes[148..150].copy_from_slice(&version.service_pack_major.to_le_bytes());
        bytes[150..152].copy_from_slice(&version.service_pack_minor.to_le_bytes());
        bytes[152..154].copy_from_slice(&version.suite_mask.to_le_bytes());
        bytes[154] = version.product_type;
        bytes[155] = 0;
    }
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

fn handle_get_system_default_lang_id(
    policy: &Kernel32CompatibilityPolicy,
) -> Result<ApiImmediate, ApiFault> {
    let lang_id = policy.system_default_lang_id().ok_or_else(|| {
        ApiFault::unsupported(
            "win32-locale",
            "get-system-default-lang-id-policy-disabled",
            format!(
                "compatibility policy {} does not enable a system-default LANGID",
                policy.id
            ),
        )
    })?;
    Ok(ApiImmediate::new(ApiReturnValue::U16(lang_id)))
}

fn handle_get_drive_type_a(
    policy: &Kernel32CompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    const DRIVE_NO_ROOT_DIR: u32 = 1;
    const DRIVE_FIXED: u32 = 3;

    let drive_policy = policy.get_drive_type_a().ok_or_else(|| {
        ApiFault::unsupported(
            "win32-process-drive",
            "get-drive-type-a-policy-disabled",
            format!(
                "compatibility policy {} does not enable GetDriveTypeA",
                policy.id
            ),
        )
    })?;
    let root_path_address = required_argument(contract, frame, 0, "ANSI root path")?;
    let preserved_last_error = session
        .last_error()
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    let root_path = if root_path_address == 0 {
        None
    } else {
        Some(read_nul_terminated_drive_root_path(
            contract,
            session,
            GuestAddress(root_path_address),
            drive_policy,
        )?)
    };
    let result = if drive_policy.process_image_drive_only
        && session
            .runtime()
            .is_process_image_drive_root_a(root_path.as_deref())
    {
        DRIVE_FIXED
    } else {
        DRIVE_NO_ROOT_DIR
    };
    Ok(ApiImmediate::new(ApiReturnValue::U32(result)).with_last_error(preserved_last_error))
}

fn read_nul_terminated_drive_root_path(
    contract: &ApiContract,
    session: &ExecutionSession<'_>,
    source: GuestAddress,
    policy: Kernel32GetDriveTypeAPolicy,
) -> Result<Vec<u8>, ApiFault> {
    let maximum = usize::try_from(policy.maximum_path_bytes).map_err(|_| {
        ApiFault::runtime_defect(
            "win32-process-drive",
            "get-drive-type-a-limit-not-representable",
            "validated ANSI drive-root path limit does not fit host accounting",
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
        "win32-process-drive",
        "get-drive-type-a-path-not-terminated",
        format!(
            "ANSI drive-root path exceeds compatibility policy limit {}",
            policy.maximum_path_bytes
        ),
    ))
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

fn handle_get_tick_count(
    policy: &Kernel32CompatibilityPolicy,
    contract: &ApiContract,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let tick_count = match policy.tick_count_increment_per_read() {
        Some(increment) => session
            .runtime_mut()
            .sample_tick_count(increment)
            .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?,
        None => session.runtime().clock().tick_count,
    };
    Ok(ApiImmediate::new(ApiReturnValue::U32(tick_count)))
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

fn handle_get_file_attributes_a(
    policy: &Kernel32CompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let file_policy = require_source_file_policy(policy)?;
    let attributes_policy = policy.get_file_attributes_a().ok_or_else(|| {
        ApiFault::unsupported(
            "win32-vfs",
            "get-file-attributes-a-policy-disabled",
            format!(
                "compatibility policy {} does not enable VFS path attributes",
                policy.id
            ),
        )
    })?;
    let path_address = required_argument(contract, frame, 0, "ANSI file path")?;
    let path = read_nul_terminated_source_path(
        contract,
        session,
        GuestAddress(path_address),
        file_policy.maximum_path_bytes,
    )?;
    let preserved_last_error = session
        .last_error()
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    let resolution = session
        .runtime()
        .resolve_file_system_path_a(&path)
        .map_err(|error| {
            ApiFault::unsupported(
                "win32-vfs",
                "get-file-attributes-a-path-unsupported",
                error.message,
            )
        })?;
    let (attributes, last_error) = match resolution {
        RuntimePathResolution::RegularFile => (
            attributes_policy.regular_file_attributes,
            preserved_last_error,
        ),
        RuntimePathResolution::Directory => {
            (attributes_policy.directory_attributes, preserved_last_error)
        }
        RuntimePathResolution::MissingFile => (u32::MAX, ERROR_FILE_NOT_FOUND),
        RuntimePathResolution::MissingPath => (u32::MAX, ERROR_PATH_NOT_FOUND),
    };
    Ok(ApiImmediate::new(ApiReturnValue::U32(attributes)).with_last_error(last_error))
}

fn handle_find_first_file_a(
    policy: &Kernel32CompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let file_policy = require_source_file_policy(policy)?;
    let find_policy = require_find_file_a_policy(policy)?;
    let attributes = policy.get_file_attributes_a().ok_or_else(|| {
        ApiFault::runtime_defect(
            "win32-vfs",
            "find-file-a-attributes-policy-missing",
            "validated FindFile policy lost its VFS attribute projection",
        )
    })?;
    let pattern_address = required_argument(contract, frame, 0, "ANSI search pattern")?;
    let output = GuestAddress(required_argument(contract, frame, 1, "find data output")?);
    let pattern = read_nul_terminated_source_path(
        contract,
        session,
        GuestAddress(pattern_address),
        file_policy.maximum_path_bytes,
    )?;
    let preserved_last_error = session
        .last_error()
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    let maximum_matches = usize::try_from(find_policy.maximum_matches).map_err(|_| {
        ApiFault::runtime_defect(
            "win32-vfs",
            "find-file-a-match-limit-not-representable",
            "validated FindFile match limit does not fit host accounting",
        )
    })?;
    let result = session.find_first_file_a(
        &pattern,
        output,
        maximum_matches,
        X86_WIN32_FIND_DATA_A_BYTES,
        |entry| x86_win32_find_data_a(entry, attributes),
    );
    match result {
        Ok(Some((result, bytes, write))) => Ok(file_search_handle_result(result.handle.0)
            .with_committed_guest_write(ApiGuestWriteReceipt {
                write: ApiGuestWrite {
                    address: output,
                    bytes,
                },
                result: write,
            })
            .with_last_error(preserved_last_error)),
        Ok(None) => {
            Ok(file_search_handle_result(INVALID_HANDLE_VALUE)
                .with_last_error(ERROR_FILE_NOT_FOUND))
        }
        Err(error)
            if matches!(
                error.code,
                "file-search-directory-missing" | "source-path-outside-mount"
            ) =>
        {
            Ok(file_search_handle_result(INVALID_HANDLE_VALUE)
                .with_last_error(ERROR_PATH_NOT_FOUND))
        }
        Err(error)
            if matches!(
                error.code,
                "file-search-pattern-shape-invalid"
                    | "file-search-pattern-component-invalid"
                    | "file-search-pattern-windows-932-invalid"
                    | "file-search-pattern-character-invalid"
                    | "source-file-path-not-absolute"
                    | "source-path-parent-escape"
                    | "source-path-character-invalid"
            ) =>
        {
            Err(ApiFault::unsupported(
                "win32-vfs",
                "find-first-file-a-pattern-unsupported",
                error.message,
            ))
        }
        Err(error) => Err(ApiFault::from_runtime(error, &contract.endpoint)),
    }
}

fn handle_delete_file_a(
    policy: &Kernel32CompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let file_policy = require_source_file_policy(policy)?;
    require_delete_file_a_policy(policy)?;
    let path_address = required_argument(contract, frame, 0, "ANSI file path")?;
    let path = read_nul_terminated_source_path(
        contract,
        session,
        GuestAddress(path_address),
        file_policy.maximum_path_bytes,
    )?;
    let resolution = session
        .runtime()
        .resolve_file_system_path_a(&path)
        .map_err(|error| {
            ApiFault::unsupported("win32-vfs", "delete-file-a-path-unsupported", error.message)
        })?;
    match resolution {
        RuntimePathResolution::MissingFile => {
            Ok(ApiImmediate::new(ApiReturnValue::Bool32(false))
                .with_last_error(ERROR_FILE_NOT_FOUND))
        }
        RuntimePathResolution::MissingPath => {
            Ok(ApiImmediate::new(ApiReturnValue::Bool32(false))
                .with_last_error(ERROR_PATH_NOT_FOUND))
        }
        RuntimePathResolution::RegularFile => Err(ApiFault::unsupported(
            "win32-vfs",
            "delete-file-a-existing-file-unsupported",
            "DeleteFileA is enabled only for a missing final VFS component",
        )),
        RuntimePathResolution::Directory => Err(ApiFault::unsupported(
            "win32-vfs",
            "delete-file-a-directory-unsupported",
            "DeleteFileA directory and existing-object mutation are outside the enabled variant",
        )),
    }
}

fn handle_find_next_file_a(
    policy: &Kernel32CompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    require_find_file_a_policy(policy)?;
    let attributes = policy.get_file_attributes_a().ok_or_else(|| {
        ApiFault::runtime_defect(
            "win32-vfs",
            "find-file-a-attributes-policy-missing",
            "validated FindFile policy lost its VFS attribute projection",
        )
    })?;
    let handle = RuntimeHandle(required_argument(contract, frame, 0, "file search handle")?);
    let output = GuestAddress(required_argument(contract, frame, 1, "find data output")?);
    let preserved_last_error = session
        .last_error()
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    match session.find_next_file_a(handle, output, X86_WIN32_FIND_DATA_A_BYTES, |entry| {
        x86_win32_find_data_a(entry, attributes)
    }) {
        Ok(Some((_entry, bytes, write))) => Ok(ApiImmediate::new(ApiReturnValue::Bool32(true))
            .with_committed_guest_write(ApiGuestWriteReceipt {
                write: ApiGuestWrite {
                    address: output,
                    bytes,
                },
                result: write,
            })
            .with_last_error(preserved_last_error)),
        Ok(None) => {
            Ok(ApiImmediate::new(ApiReturnValue::Bool32(false))
                .with_last_error(ERROR_NO_MORE_FILES))
        }
        Err(error)
            if matches!(
                error.code,
                "runtime-handle-type-unknown"
                    | "runtime-handle-type-mismatch"
                    | "runtime-handle-unknown"
                    | "runtime-handle-stale"
                    | "file-search-id-unknown"
            ) =>
        {
            Ok(ApiImmediate::new(ApiReturnValue::Bool32(false))
                .with_last_error(ERROR_INVALID_HANDLE))
        }
        Err(error) => Err(ApiFault::from_runtime(error, &contract.endpoint)),
    }
}

fn handle_find_close(
    policy: &Kernel32CompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    require_find_file_a_policy(policy)?;
    let handle = RuntimeHandle(required_argument(contract, frame, 0, "file search handle")?);
    let preserved_last_error = session
        .last_error()
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    match session.runtime_mut().find_close(handle) {
        Ok(_) => {
            Ok(ApiImmediate::new(ApiReturnValue::Bool32(true))
                .with_last_error(preserved_last_error))
        }
        Err(error)
            if matches!(
                error.code,
                "runtime-handle-type-unknown"
                    | "runtime-handle-type-mismatch"
                    | "runtime-handle-unknown"
                    | "runtime-handle-stale"
                    | "file-search-id-unknown"
            ) =>
        {
            Ok(ApiImmediate::new(ApiReturnValue::Bool32(false))
                .with_last_error(ERROR_INVALID_HANDLE))
        }
        Err(error) => Err(ApiFault::from_runtime(error, &contract.endpoint)),
    }
}

fn file_search_handle_result(value: u32) -> ApiImmediate {
    ApiImmediate::new(ApiReturnValue::LogicalHandle {
        namespace: RUNTIME_FILE_SEARCH_NAMESPACE.to_owned(),
        value,
    })
}

fn x86_win32_find_data_a(
    entry: &RuntimeFileSearchEntry,
    attributes: Kernel32GetFileAttributesAPolicy,
) -> Result<Vec<u8>, crate::runtime::RuntimeStateError> {
    if entry.file_name_a.is_empty()
        || entry.file_name_a.len() >= X86_WIN32_FIND_DATA_A_FILE_NAME_CAPACITY
        || entry.file_name_a.contains(&0)
    {
        return Err(crate::runtime::RuntimeStateError::invalid_input(
            "runtime-vfs",
            "file-search-name-not-representable",
            "file search result name does not fit WIN32_FIND_DATAA.cFileName",
        ));
    }
    let mut bytes = vec![0; X86_WIN32_FIND_DATA_A_BYTES];
    let projected_attributes = match entry.kind {
        RuntimeFileSearchEntryKind::RegularFile => attributes.regular_file_attributes,
        RuntimeFileSearchEntryKind::Directory => attributes.directory_attributes,
    };
    bytes[0..4].copy_from_slice(&projected_attributes.to_le_bytes());
    bytes[28..32].copy_from_slice(&((entry.byte_length >> 32) as u32).to_le_bytes());
    bytes[32..36].copy_from_slice(&(entry.byte_length as u32).to_le_bytes());
    let name_end = X86_WIN32_FIND_DATA_A_FILE_NAME_OFFSET + entry.file_name_a.len();
    bytes[X86_WIN32_FIND_DATA_A_FILE_NAME_OFFSET..name_end].copy_from_slice(&entry.file_name_a);
    Ok(bytes)
}

fn handle_create_directory_a(
    policy: &Kernel32CompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> ApiHandlerResult {
    let result = (|| {
        let file_policy = require_source_file_policy(policy)?;
        if !policy.create_directory_a_enabled() {
            return Err(ApiFault::unsupported(
                "win32-vfs",
                "create-directory-a-policy-disabled",
                format!(
                    "compatibility policy {} does not enable writable directory creation",
                    policy.id
                ),
            ));
        }
        let path_address = required_argument(contract, frame, 0, "ANSI directory path")?;
        let security_attributes = required_argument(contract, frame, 1, "security attributes")?;
        if path_address == 0 {
            return Ok(ApiHandlerResult::Immediate(
                ApiImmediate::new(ApiReturnValue::Bool32(false))
                    .with_last_error(ERROR_PATH_NOT_FOUND),
            ));
        }
        if security_attributes != 0 {
            return Err(ApiFault::unsupported(
                "win32-vfs",
                "create-directory-a-security-attributes-unsupported",
                "the writable-profile directory slice supports NULL security attributes only",
            ));
        }
        let path = read_nul_terminated_source_path(
            contract,
            session,
            GuestAddress(path_address),
            file_policy.maximum_path_bytes,
        )?;
        let request = match session.runtime_mut().begin_writable_directory_create(&path) {
            Ok(request) => request,
            Err(error)
                if matches!(
                    error.code,
                    "writable-directory-already-exists"
                        | "writable-directory-file-conflict"
                        | "writable-directory-create-pending"
                ) =>
            {
                return Ok(ApiHandlerResult::Immediate(
                    ApiImmediate::new(ApiReturnValue::Bool32(false))
                        .with_last_error(ERROR_ALREADY_EXISTS),
                ));
            }
            Err(error)
                if matches!(
                    error.code,
                    "writable-directory-parent-missing" | "source-path-outside-mount"
                ) =>
            {
                return Ok(ApiHandlerResult::Immediate(
                    ApiImmediate::new(ApiReturnValue::Bool32(false))
                        .with_last_error(ERROR_PATH_NOT_FOUND),
                ));
            }
            Err(error)
                if matches!(
                    error.code,
                    "writable-profile-missing" | "writable-directory-unregistered"
                ) =>
            {
                return Err(ApiFault::unsupported(
                    "win32-vfs",
                    "create-directory-a-path-unsupported",
                    error.message,
                ));
            }
            Err(error) => return Err(ApiFault::from_runtime(error, &contract.endpoint)),
        };
        Ok(ApiHandlerResult::Deferred(crate::api::HostRequest {
            request_id: request.request_id,
            service: RUNTIME_WRITABLE_DIRECTORY_CREATE_SERVICE.to_owned(),
            payload: request.encode_host_payload().to_vec(),
        }))
    })();
    match result {
        Ok(outcome) => outcome,
        Err(fault) => ApiHandlerResult::Fault(fault),
    }
}

fn handle_create_file_a(
    policy: &Kernel32CompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let file_policy = require_source_file_policy(policy)?;
    let path_address = required_argument(contract, frame, 0, "ANSI file path")?;
    let desired_access = required_argument(contract, frame, 1, "desired access")?;
    let share_mode = required_argument(contract, frame, 2, "share mode")?;
    let security_attributes = required_argument(contract, frame, 3, "security attributes")?;
    let creation_disposition = required_argument(contract, frame, 4, "creation disposition")?;
    let flags_and_attributes = required_argument(contract, frame, 5, "flags and attributes")?;
    let template_file = required_argument(contract, frame, 6, "template file")?;
    let supported_read_open =
        desired_access == 0x8000_0000 && share_mode == 1 && creation_disposition == 3;
    let supported_write_open =
        desired_access == 0x4000_0000 && share_mode == 0 && matches!(creation_disposition, 2 | 4);
    if path_address == 0 {
        return Ok(source_file_handle_result(INVALID_HANDLE_VALUE)
            .with_last_error(ERROR_INVALID_PARAMETER));
    }
    if security_attributes != 0
        || template_file != 0
        || !file_policy.allowed_desired_access.contains(&desired_access)
        || !file_policy.allowed_share_modes.contains(&share_mode)
        || !file_policy
            .allowed_creation_dispositions
            .contains(&creation_disposition)
        || !file_policy
            .allowed_flags_and_attributes
            .contains(&flags_and_attributes)
        || !(supported_read_open || supported_write_open)
    {
        return Err(ApiFault::unsupported(
            "win32-vfs",
            "create-file-a-variant-unsupported",
            format!(
                "compatibility policy {} enables only bounded source-read and writable-profile open variants",
                policy.id
            ),
        ));
    }
    let path = read_nul_terminated_source_path(
        contract,
        session,
        GuestAddress(path_address),
        file_policy.maximum_path_bytes,
    )?;
    let preserved_last_error = session
        .last_error()
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    match session.runtime_mut().open_file_a(
        &path,
        desired_access,
        share_mode,
        creation_disposition,
        flags_and_attributes,
    ) {
        Ok(Some(opened)) => Ok(source_file_handle_result(opened.handle.0).with_last_error(
            if matches!(creation_disposition, 2 | 4) {
                if opened.existed {
                    ERROR_ALREADY_EXISTS
                } else {
                    ERROR_SUCCESS
                }
            } else {
                preserved_last_error
            },
        )),
        Ok(None) => {
            Ok(source_file_handle_result(INVALID_HANDLE_VALUE)
                .with_last_error(ERROR_FILE_NOT_FOUND))
        }
        Err(error) if error.code == "source-file-sharing-violation" => {
            Ok(source_file_handle_result(INVALID_HANDLE_VALUE)
                .with_last_error(ERROR_SHARING_VIOLATION))
        }
        Err(error) if error.code == "immutable-source-write-denied" => Ok(
            source_file_handle_result(INVALID_HANDLE_VALUE).with_last_error(ERROR_ACCESS_DENIED),
        ),
        Err(error) => Err(ApiFault::unsupported(
            "win32-vfs",
            "create-file-a-path-unsupported",
            error.message,
        )),
    }
}

fn source_file_handle_result(value: u32) -> ApiImmediate {
    ApiImmediate::new(ApiReturnValue::LogicalHandle {
        namespace: RUNTIME_SOURCE_FILE_NAMESPACE.to_owned(),
        value,
    })
}

fn handle_get_file_size(
    policy: &Kernel32CompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    require_source_file_policy(policy)?;
    let handle = RuntimeHandle(required_argument(contract, frame, 0, "file handle")?);
    let size_high = required_argument(contract, frame, 1, "file size high output")?;
    if size_high != 0 {
        return Err(ApiFault::unsupported(
            "win32-vfs",
            "get-file-size-high-output-unsupported",
            "the immutable-source slice supports only files whose size fits the low DWORD",
        ));
    }
    let preserved_last_error = session
        .last_error()
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    let size = match session.runtime().source_file_byte_length(handle) {
        Ok(size) => size,
        Err(_) => {
            return Ok(ApiImmediate::new(ApiReturnValue::U32(u32::MAX))
                .with_last_error(ERROR_INVALID_HANDLE));
        }
    };
    let size = u32::try_from(size).map_err(|_| {
        ApiFault::runtime_defect(
            "win32-vfs",
            "source-file-size-not-representable",
            "validated source file size does not fit GetFileSize low DWORD",
        )
    })?;
    Ok(ApiImmediate::new(ApiReturnValue::U32(size)).with_last_error(preserved_last_error))
}

fn handle_read_file(
    policy: &Kernel32CompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> ApiHandlerResult {
    let result = (|| {
        let file_policy = require_source_file_policy(policy)?;
        let handle = RuntimeHandle(required_argument(contract, frame, 0, "file handle")?);
        let buffer = GuestAddress(required_argument(contract, frame, 1, "read buffer")?);
        let requested_bytes = required_argument(contract, frame, 2, "requested bytes")?;
        let bytes_read = GuestAddress(required_argument(contract, frame, 3, "bytes-read output")?);
        let overlapped = required_argument(contract, frame, 4, "overlapped state")?;
        let backing = match session.runtime().source_file_for_handle(handle) {
            Ok(file) => file.backing,
            Err(_) => {
                return Ok(ApiHandlerResult::Immediate(
                    ApiImmediate::new(ApiReturnValue::Bool32(false))
                        .with_last_error(ERROR_INVALID_HANDLE),
                ));
            }
        };
        if buffer.0 == 0
            || bytes_read.0 == 0
            || requested_bytes == 0
            || requested_bytes > file_policy.maximum_read_bytes
            || requested_bytes > RUNTIME_SOURCE_READ_MAX_BYTES
            || overlapped != 0
        {
            return Err(ApiFault::unsupported(
                "win32-vfs",
                "read-file-variant-unsupported",
                format!(
                    "compatibility policy {} enables one bounded synchronous source read shape",
                    policy.id
                ),
            ));
        }
        let requested_bytes_usize = usize::try_from(requested_bytes).map_err(|_| {
            ApiFault::runtime_defect(
                "win32-vfs",
                "source-read-length-not-representable",
                "validated source read length does not fit host accounting",
            )
        })?;
        session
            .memory()
            .validate_access(buffer, requested_bytes_usize, MemoryAccess::Write)
            .map_err(|error| ApiFault::from_memory(error, &contract.endpoint))?;
        session
            .memory()
            .validate_access(bytes_read, 4, MemoryAccess::Write)
            .map_err(|error| ApiFault::from_memory(error, &contract.endpoint))?;
        if matches!(backing, RuntimeFileBacking::WritableProfile { .. })
            && session
                .runtime()
                .writable_file_needs_commit(handle)
                .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?
        {
            let (result, bytes, buffer_write, count_write) = session
                .read_writable_file(handle, buffer, bytes_read, requested_bytes)
                .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
            let mut immediate = ApiImmediate::new(ApiReturnValue::Bool32(true))
                .with_committed_guest_write(ApiGuestWriteReceipt {
                    write: ApiGuestWrite {
                        address: bytes_read,
                        bytes: result.transferred_bytes.to_le_bytes().to_vec(),
                    },
                    result: count_write,
                })
                .with_last_error(ERROR_SUCCESS);
            if let Some(buffer_write) = buffer_write {
                immediate = immediate.with_committed_guest_write(ApiGuestWriteReceipt {
                    write: ApiGuestWrite {
                        address: buffer,
                        bytes,
                    },
                    result: buffer_write,
                });
            }
            return Ok(ApiHandlerResult::Immediate(immediate));
        }
        let request = session
            .runtime_mut()
            .begin_source_file_read(handle, buffer, bytes_read, requested_bytes)
            .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
        Ok(ApiHandlerResult::Deferred(crate::api::HostRequest {
            request_id: request.request_id,
            service: if request.source_id == 0 {
                RUNTIME_WRITABLE_READ_SERVICE.to_owned()
            } else {
                RUNTIME_SOURCE_READ_SERVICE.to_owned()
            },
            payload: request.encode_host_payload().to_vec(),
        }))
    })();
    match result {
        Ok(outcome) => outcome,
        Err(fault) => ApiHandlerResult::Fault(fault),
    }
}

fn handle_write_file(
    policy: &Kernel32CompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let file_policy = require_source_file_policy(policy)?;
    let handle = RuntimeHandle(required_argument(contract, frame, 0, "file handle")?);
    let buffer = GuestAddress(required_argument(contract, frame, 1, "write buffer")?);
    let requested_bytes = required_argument(contract, frame, 2, "requested bytes")?;
    let bytes_written = GuestAddress(required_argument(
        contract,
        frame,
        3,
        "bytes-written output",
    )?);
    let overlapped = required_argument(contract, frame, 4, "overlapped state")?;
    let backing = match session.runtime().source_file_for_handle(handle) {
        Ok(file) => file.backing,
        Err(_) => {
            return Ok(ApiImmediate::new(ApiReturnValue::Bool32(false))
                .with_last_error(ERROR_INVALID_HANDLE));
        }
    };
    if matches!(backing, RuntimeFileBacking::ImmutableSource { .. }) {
        return Ok(
            ApiImmediate::new(ApiReturnValue::Bool32(false)).with_last_error(ERROR_ACCESS_DENIED)
        );
    }
    if buffer.0 == 0
        || bytes_written.0 == 0
        || requested_bytes == 0
        || requested_bytes > file_policy.maximum_write_bytes
        || requested_bytes > RUNTIME_WRITABLE_FILE_MAX_IO_BYTES
        || overlapped != 0
    {
        return Err(ApiFault::unsupported(
            "win32-vfs",
            "write-file-variant-unsupported",
            format!(
                "compatibility policy {} enables one bounded synchronous writable-profile write shape",
                policy.id
            ),
        ));
    }
    let bytes = session
        .memory()
        .read(buffer, requested_bytes as usize)
        .map_err(|error| ApiFault::from_memory(error, &contract.endpoint))?
        .to_vec();
    match session.write_writable_file(handle, bytes_written, &bytes) {
        Ok((result, count_write)) => Ok(ApiImmediate::new(ApiReturnValue::Bool32(true))
            .with_committed_guest_write(ApiGuestWriteReceipt {
                write: ApiGuestWrite {
                    address: bytes_written,
                    bytes: result.transferred_bytes.to_le_bytes().to_vec(),
                },
                result: count_write,
            })
            .with_last_error(ERROR_SUCCESS)),
        Err(error) if error.code == "writable-profile-quota-exceeded" => {
            Ok(ApiImmediate::new(ApiReturnValue::Bool32(false)).with_last_error(ERROR_DISK_FULL))
        }
        Err(error) => Err(ApiFault::from_runtime(error, &contract.endpoint)),
    }
}

fn handle_flush_file_buffers(
    policy: &Kernel32CompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> ApiHandlerResult {
    let result = (|| {
        require_source_file_policy(policy)?;
        let handle = RuntimeHandle(required_argument(contract, frame, 0, "file handle")?);
        let backing = match session.runtime().source_file_for_handle(handle) {
            Ok(file) => file.backing,
            Err(_) => {
                return Ok(ApiHandlerResult::Immediate(
                    ApiImmediate::new(ApiReturnValue::Bool32(false))
                        .with_last_error(ERROR_INVALID_HANDLE),
                ));
            }
        };
        if matches!(backing, RuntimeFileBacking::ImmutableSource { .. }) {
            return Ok(ApiHandlerResult::Immediate(
                ApiImmediate::new(ApiReturnValue::Bool32(false))
                    .with_last_error(ERROR_ACCESS_DENIED),
            ));
        }
        if !session
            .runtime()
            .writable_file_needs_commit(handle)
            .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?
        {
            return Ok(ApiHandlerResult::Immediate(
                ApiImmediate::new(ApiReturnValue::Bool32(true)).with_last_error(ERROR_SUCCESS),
            ));
        }
        let request = session
            .runtime_mut()
            .begin_writable_file_commit(handle, false)
            .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
        Ok(ApiHandlerResult::Deferred(crate::api::HostRequest {
            request_id: request.request_id,
            service: RUNTIME_WRITABLE_COMMIT_SERVICE.to_owned(),
            payload: request.encode_host_payload().to_vec(),
        }))
    })();
    match result {
        Ok(outcome) => outcome,
        Err(fault) => ApiHandlerResult::Fault(fault),
    }
}

fn handle_set_file_pointer(
    policy: &Kernel32CompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    require_source_file_policy(policy)?;
    let handle = RuntimeHandle(required_argument(contract, frame, 0, "file handle")?);
    let distance = required_argument(contract, frame, 1, "distance low")? as i32 as i64;
    let distance_high = required_argument(contract, frame, 2, "distance high")?;
    let origin = required_argument(contract, frame, 3, "move method")?;
    if distance_high != 0 {
        return Err(ApiFault::unsupported(
            "win32-vfs",
            "set-file-pointer-high-distance-unsupported",
            "the first immutable-source seek slice supports a signed low DWORD only",
        ));
    }
    let preserved_last_error = session
        .last_error()
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    if session.runtime().source_file_for_handle(handle).is_err() {
        return Ok(
            ApiImmediate::new(ApiReturnValue::U32(u32::MAX)).with_last_error(ERROR_INVALID_HANDLE)
        );
    }
    match session
        .runtime_mut()
        .seek_source_file(handle, distance, origin)
    {
        Ok(cursor) => Ok(ApiImmediate::new(ApiReturnValue::U32(cursor as u32))
            .with_last_error(preserved_last_error)),
        Err(error) if error.code == "source-seek-range-invalid" => {
            Ok(ApiImmediate::new(ApiReturnValue::U32(u32::MAX))
                .with_last_error(ERROR_NEGATIVE_SEEK))
        }
        Err(error) if error.code == "source-seek-origin-invalid" => {
            Ok(ApiImmediate::new(ApiReturnValue::U32(u32::MAX))
                .with_last_error(ERROR_INVALID_PARAMETER))
        }
        Err(error) => Err(ApiFault::from_runtime(error, &contract.endpoint)),
    }
}

fn handle_close_handle(
    _policy: &Kernel32CompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> ApiHandlerResult {
    let result = (|| {
        let handle = RuntimeHandle(required_argument(contract, frame, 0, "object handle")?);
        let preserved_last_error = session
            .last_error()
            .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
        let Ok(kind) = handle.kind() else {
            return Ok(ApiHandlerResult::Immediate(
                ApiImmediate::new(ApiReturnValue::Bool32(false))
                    .with_last_error(ERROR_INVALID_HANDLE),
            ));
        };
        if !matches!(
            kind,
            RuntimeObjectKind::Process
                | RuntimeObjectKind::Thread
                | RuntimeObjectKind::Mutex
                | RuntimeObjectKind::Event
                | RuntimeObjectKind::WaitableTimer
                | RuntimeObjectKind::File
        ) || session.runtime().object_for_handle(handle, kind).is_err()
        {
            return Ok(ApiHandlerResult::Immediate(
                ApiImmediate::new(ApiReturnValue::Bool32(false))
                    .with_last_error(ERROR_INVALID_HANDLE),
            ));
        }
        if kind == RuntimeObjectKind::File
            && session
                .runtime()
                .writable_file_needs_commit(handle)
                .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?
        {
            let request = session
                .runtime_mut()
                .begin_writable_file_commit(handle, true)
                .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
            return Ok(ApiHandlerResult::Deferred(crate::api::HostRequest {
                request_id: request.request_id,
                service: RUNTIME_WRITABLE_COMMIT_SERVICE.to_owned(),
                payload: request.encode_host_payload().to_vec(),
            }));
        }
        session
            .runtime_mut()
            .close_handle(handle)
            .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
        Ok(ApiHandlerResult::Immediate(
            ApiImmediate::new(ApiReturnValue::Bool32(true)).with_last_error(preserved_last_error),
        ))
    })();
    match result {
        Ok(outcome) => outcome,
        Err(fault) => ApiHandlerResult::Fault(fault),
    }
}

pub fn complete_kernel32_source_read(
    dispatcher: &ApiDispatcher,
    call: ApiDeferredCall,
    response: RuntimeSourceReadHostResponse,
    session: &mut ExecutionSession<'_>,
) -> (
    DispatchOutcome,
    Option<crate::runtime::RuntimeSourceReadCompletion>,
) {
    if call.contract.implementation_id != READ_FILE_IMPLEMENTATION
        || call.request.request_id != response.request_id
        || !matches!(
            call.request.service.as_str(),
            RUNTIME_SOURCE_READ_SERVICE | RUNTIME_WRITABLE_READ_SERVICE
        )
    {
        return (
            DispatchOutcome::Fault(ApiFault::invalid_call(
                "win32-vfs",
                "source-read-suspended-call-mismatch",
                "source read host completion does not match its suspended API call",
            )),
            None,
        );
    }
    let plan = match session.runtime().plan_source_file_read_completion(response) {
        Ok(plan) => plan,
        Err(error) => {
            return (
                DispatchOutcome::Fault(ApiFault::from_runtime(error, &call.contract.endpoint)),
                None,
            );
        }
    };
    let response = plan.response();
    let mut immediate = ApiImmediate::new(ApiReturnValue::Bool32(plan.completion().succeeded))
        .with_guest_write(
            plan.bytes_read(),
            response.transferred_bytes.to_le_bytes().to_vec(),
        )
        .with_last_error(plan.completion().win32_error);
    if plan.completion().succeeded && !response.bytes.is_empty() {
        immediate = immediate.with_guest_write(plan.buffer(), response.bytes.clone());
    }
    let request_id = call.request.request_id;
    let outcome = dispatcher.complete_deferred(
        call,
        HostCompletion {
            request_id,
            result: immediate,
        },
        session,
    );
    if matches!(outcome, DispatchOutcome::Immediate(_)) {
        let completion = session
            .runtime_mut()
            .commit_source_file_read_completion(plan);
        (outcome, Some(completion))
    } else {
        (outcome, None)
    }
}

pub fn complete_kernel32_waitable_timer_pacing(
    dispatcher: &ApiDispatcher,
    call: ApiDeferredCall,
    acknowledgement: u32,
    session: &mut ExecutionSession<'_>,
) -> (DispatchOutcome, Option<RuntimeWaitForSingleObjectExResult>) {
    if call.contract.implementation_id != WAIT_FOR_SINGLE_OBJECT_EX_IMPLEMENTATION
        || call.request.service != RUNTIME_WAITABLE_TIMER_PACING_SERVICE
        || acknowledgement != 1
    {
        return (
            DispatchOutcome::Fault(ApiFault::invalid_call(
                "win32-wait",
                "waitable-timer-pacing-suspended-call-mismatch",
                "timer pacing host completion does not match its suspended API call",
            )),
            None,
        );
    }
    let plan = match session
        .runtime()
        .plan_waitable_timer_pacing_completion(call.request.request_id)
    {
        Ok(plan) => plan,
        Err(error) => {
            return (
                DispatchOutcome::Fault(ApiFault::from_runtime(error, &call.contract.endpoint)),
                None,
            );
        }
    };
    let result = plan.result();
    let registration = result
        .registration
        .expect("timer pacing completion plan has a wait registration");
    let pacing = registration
        .waitable_timer_pacing
        .expect("timer pacing completion plan has a pacing contract");
    if call.request.payload != pacing.encode_host_payload(registration.id) {
        return (
            DispatchOutcome::Fault(ApiFault::invalid_call(
                "win32-wait",
                "waitable-timer-pacing-payload-mismatch",
                "timer pacing host payload differs from the Rust wait authority",
            )),
            None,
        );
    }
    let preserved_last_error = match session.last_error() {
        Ok(last_error) => last_error,
        Err(error) => {
            return (
                DispatchOutcome::Fault(ApiFault::from_runtime(error, &call.contract.endpoint)),
                None,
            );
        }
    };
    let request_id = call.request.request_id;
    let outcome = dispatcher.complete_deferred(
        call,
        HostCompletion {
            request_id,
            result: wait_result_immediate(result, preserved_last_error),
        },
        session,
    );
    if matches!(outcome, DispatchOutcome::Immediate(_)) {
        let result = session
            .runtime_mut()
            .commit_waitable_timer_pacing_completion(plan);
        (outcome, Some(result))
    } else {
        (outcome, None)
    }
}

pub fn complete_kernel32_audio_notification_wait(
    dispatcher: &ApiDispatcher,
    call: ApiDeferredCall,
    acknowledgement: u32,
    session: &mut ExecutionSession<'_>,
) -> (DispatchOutcome, Option<RuntimeHostCompletion>) {
    if call.contract.implementation_id != WAIT_FOR_MULTIPLE_OBJECTS_IMPLEMENTATION
        || call.request.service != RUNTIME_AUDIO_NOTIFICATION_PACING_SERVICE
        || acknowledgement != 1
    {
        return (
            DispatchOutcome::Fault(ApiFault::invalid_call(
                "win32-wait",
                "audio-notification-pacing-suspended-call-mismatch",
                "audio notification pacing completion does not match its suspended API call",
            )),
            None,
        );
    }
    let handles = match read_wait_handles(&call.contract, &call.frame, session) {
        Ok(handles) => handles,
        Err(fault) => return (DispatchOutcome::Fault(fault), None),
    };
    let timeout = call.frame.argument_u32(3).unwrap_or(0);
    let plan = match session.runtime().plan_audio_notification_wait_completion(
        call.request.request_id,
        &handles,
        timeout,
        &call.request.payload,
    ) {
        Ok(plan) => plan,
        Err(error) => {
            return (
                DispatchOutcome::Fault(ApiFault::from_runtime(error, &call.contract.endpoint)),
                None,
            );
        }
    };
    if call.request.payload != plan.encode_host_payload() {
        return (
            DispatchOutcome::Fault(ApiFault::invalid_call(
                "win32-wait",
                "audio-notification-pacing-payload-mismatch",
                "audio notification host payload differs from Rust audio authority",
            )),
            None,
        );
    }
    let expected_result_index = plan.result_index();
    let preserved_last_error = match session.last_error() {
        Ok(last_error) => last_error,
        Err(error) => {
            return (
                DispatchOutcome::Fault(ApiFault::from_runtime(error, &call.contract.endpoint)),
                None,
            );
        }
    };
    let result = expected_result_index.map_or(WAIT_TIMEOUT, |index| WAIT_OBJECT_0 + index);
    let request_id = call.request.request_id;
    let outcome = dispatcher.complete_deferred(
        call,
        HostCompletion {
            request_id,
            result: ApiImmediate::new(ApiReturnValue::U32(result))
                .with_last_error(preserved_last_error),
        },
        session,
    );
    if matches!(outcome, DispatchOutcome::Immediate(_)) {
        let (completion, committed_result_index) = session
            .runtime_mut()
            .commit_audio_notification_wait_completion(plan);
        debug_assert_eq!(committed_result_index, expected_result_index);
        (outcome, Some(completion))
    } else {
        (outcome, None)
    }
}

pub fn complete_kernel32_writable_commit(
    dispatcher: &ApiDispatcher,
    call: ApiDeferredCall,
    response: RuntimeWritableCommitHostResponse,
    session: &mut ExecutionSession<'_>,
) -> (
    DispatchOutcome,
    Option<crate::runtime::RuntimeWritableCommitCompletion>,
) {
    if !matches!(
        call.contract.implementation_id.as_str(),
        CLOSE_HANDLE_IMPLEMENTATION | FLUSH_FILE_BUFFERS_IMPLEMENTATION
    ) || call.request.request_id != response.request_id
        || call.request.service != RUNTIME_WRITABLE_COMMIT_SERVICE
    {
        return (
            DispatchOutcome::Fault(ApiFault::invalid_call(
                "win32-vfs",
                "writable-commit-suspended-call-mismatch",
                "writable commit host completion does not match its suspended API call",
            )),
            None,
        );
    }
    let plan = match session
        .runtime()
        .plan_writable_file_commit_completion(response)
    {
        Ok(plan) => plan,
        Err(error) => {
            return (
                DispatchOutcome::Fault(ApiFault::from_runtime(error, &call.contract.endpoint)),
                None,
            );
        }
    };
    let completion_last_error = if plan.completion().succeeded
        && call.contract.implementation_id == CLOSE_HANDLE_IMPLEMENTATION
    {
        match session.last_error() {
            Ok(last_error) => last_error,
            Err(error) => {
                return (
                    DispatchOutcome::Fault(ApiFault::from_runtime(error, &call.contract.endpoint)),
                    None,
                );
            }
        }
    } else {
        plan.completion().win32_error
    };
    let immediate = ApiImmediate::new(ApiReturnValue::Bool32(plan.completion().succeeded))
        .with_last_error(completion_last_error);
    let request_id = call.request.request_id;
    let outcome = dispatcher.complete_deferred(
        call,
        HostCompletion {
            request_id,
            result: immediate,
        },
        session,
    );
    if matches!(outcome, DispatchOutcome::Immediate(_)) {
        let completion = session
            .runtime_mut()
            .commit_writable_file_commit_completion(plan);
        (outcome, Some(completion))
    } else {
        (outcome, None)
    }
}

pub fn complete_kernel32_writable_directory_create(
    dispatcher: &ApiDispatcher,
    call: ApiDeferredCall,
    response: RuntimeWritableDirectoryCreateHostResponse,
    session: &mut ExecutionSession<'_>,
) -> (
    DispatchOutcome,
    Option<crate::runtime::RuntimeWritableDirectoryCreateCompletion>,
) {
    if call.contract.implementation_id != CREATE_DIRECTORY_A_IMPLEMENTATION
        || call.request.request_id != response.request_id
        || call.request.service != RUNTIME_WRITABLE_DIRECTORY_CREATE_SERVICE
    {
        return (
            DispatchOutcome::Fault(ApiFault::invalid_call(
                "win32-vfs",
                "writable-directory-create-suspended-call-mismatch",
                "directory create host completion does not match its suspended API call",
            )),
            None,
        );
    }
    let request = match session
        .runtime()
        .writable_directory_create_request(response.request_id)
    {
        Ok(request) => request,
        Err(error) => {
            return (
                DispatchOutcome::Fault(ApiFault::from_runtime(error, &call.contract.endpoint)),
                None,
            );
        }
    };
    if call.request.payload != request.encode_host_payload() {
        return (
            DispatchOutcome::Fault(ApiFault::invalid_call(
                "win32-vfs",
                "writable-directory-create-payload-mismatch",
                "directory create host payload differs from the Rust VFS authority",
            )),
            None,
        );
    }
    let plan = match session
        .runtime()
        .plan_writable_directory_create_completion(response)
    {
        Ok(plan) => plan,
        Err(error) => {
            return (
                DispatchOutcome::Fault(ApiFault::from_runtime(error, &call.contract.endpoint)),
                None,
            );
        }
    };
    let completion_last_error = if plan.completion().succeeded {
        match session.last_error() {
            Ok(last_error) => last_error,
            Err(error) => {
                return (
                    DispatchOutcome::Fault(ApiFault::from_runtime(error, &call.contract.endpoint)),
                    None,
                );
            }
        }
    } else {
        plan.completion().win32_error
    };
    let immediate = ApiImmediate::new(ApiReturnValue::Bool32(plan.completion().succeeded))
        .with_last_error(completion_last_error);
    let request_id = call.request.request_id;
    let outcome = dispatcher.complete_deferred(
        call,
        HostCompletion {
            request_id,
            result: immediate,
        },
        session,
    );
    if matches!(outcome, DispatchOutcome::Immediate(_)) {
        let completion = session
            .runtime_mut()
            .commit_writable_directory_create_completion(plan);
        (outcome, Some(completion))
    } else {
        (outcome, None)
    }
}

fn require_source_file_policy(
    policy: &Kernel32CompatibilityPolicy,
) -> Result<&Kernel32SourceFilePolicy, ApiFault> {
    policy.source_files().ok_or_else(|| {
        ApiFault::unsupported(
            "win32-vfs",
            "source-file-policy-disabled",
            format!(
                "compatibility policy {} does not enable immutable source files",
                policy.id
            ),
        )
    })
}

fn require_find_file_a_policy(
    policy: &Kernel32CompatibilityPolicy,
) -> Result<Kernel32FindFileAPolicy, ApiFault> {
    policy.find_file_a().ok_or_else(|| {
        ApiFault::unsupported(
            "win32-vfs",
            "find-file-a-policy-disabled",
            format!(
                "compatibility policy {} does not enable deterministic VFS enumeration",
                policy.id
            ),
        )
    })
}

fn require_delete_file_a_policy(
    policy: &Kernel32CompatibilityPolicy,
) -> Result<Kernel32DeleteFileAPolicy, ApiFault> {
    policy.delete_file_a().ok_or_else(|| {
        ApiFault::unsupported(
            "win32-vfs",
            "delete-file-a-policy-disabled",
            format!(
                "compatibility policy {} does not enable bounded missing-file deletion probes",
                policy.id
            ),
        )
    })
}

fn read_nul_terminated_source_path(
    contract: &ApiContract,
    session: &ExecutionSession<'_>,
    source: GuestAddress,
    maximum_path_bytes: u32,
) -> Result<Vec<u8>, ApiFault> {
    let maximum = usize::try_from(maximum_path_bytes).map_err(|_| {
        ApiFault::runtime_defect(
            "win32-vfs",
            "source-file-path-limit-not-representable",
            "validated source file path limit does not fit host accounting",
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
        "win32-vfs",
        "source-file-path-not-terminated",
        format!("ANSI source file path did not terminate within {maximum_path_bytes} bytes"),
    ))
}

fn handle_create_mutex_a(
    policy: &Kernel32CompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let mutex_policy = policy.create_mutex_a().ok_or_else(|| {
        ApiFault::unsupported(
            "win32-mutex",
            "create-mutex-a-policy-disabled",
            format!(
                "compatibility policy {} does not enable CreateMutexA",
                policy.id
            ),
        )
    })?;
    let security_attributes = required_argument(contract, frame, 0, "security attributes")?;
    let initial_owner = required_argument(contract, frame, 1, "initial owner")? != 0;
    let name_address = required_argument(contract, frame, 2, "ANSI mutex name")?;
    if security_attributes != 0 {
        return Err(ApiFault::unsupported(
            "win32-mutex",
            "create-mutex-a-security-attributes-unsupported",
            "the selected CreateMutexA policy enables only null security attributes",
        ));
    }
    let name_a = if name_address == 0 {
        if !mutex_policy.allow_unnamed {
            return Err(ApiFault::unsupported(
                "win32-mutex",
                "create-mutex-a-unnamed-unsupported",
                "the selected CreateMutexA policy does not enable unnamed mutexes",
            ));
        }
        None
    } else {
        if !mutex_policy.allow_named {
            return Err(ApiFault::unsupported(
                "win32-mutex",
                "create-mutex-a-named-unsupported",
                "the selected CreateMutexA policy does not enable named mutexes",
            ));
        }
        let bytes_with_nul = read_nul_terminated_ansi_mutex_name(
            contract,
            session,
            GuestAddress(name_address),
            mutex_policy,
        )?;
        if bytes_with_nul == [0] {
            return Err(ApiFault::unsupported(
                "win32-mutex",
                "create-mutex-a-empty-name-unsupported",
                "the selected CreateMutexA policy does not enable an empty object-manager name",
            ));
        }
        let utf16_units_with_nul = decode_windows_932(&bytes_with_nul);
        Some(
            RuntimeMutexNameA::new(bytes_with_nul, utf16_units_with_nul)
                .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?,
        )
    };
    let result = session
        .runtime_mut()
        .create_mutex_a(name_a, initial_owner)
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    Ok(
        ApiImmediate::new(ApiReturnValue::U32(result.handle.0)).with_last_error(
            if result.already_exists {
                ERROR_ALREADY_EXISTS
            } else {
                ERROR_SUCCESS
            },
        ),
    )
}

fn handle_create_event_a(
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let security_attributes = required_argument(contract, frame, 0, "security attributes")?;
    let manual_reset = required_argument(contract, frame, 1, "manual reset")? != 0;
    let initial_state = required_argument(contract, frame, 2, "initial state")? != 0;
    let name = required_argument(contract, frame, 3, "ANSI event name")?;
    if security_attributes != 0 {
        return Err(ApiFault::unsupported(
            "win32-event",
            "create-event-a-security-attributes-unsupported",
            "bounded CreateEventA supports null security attributes",
        ));
    }
    if name != 0 {
        return Err(ApiFault::unsupported(
            "win32-event",
            "create-event-a-named-unsupported",
            "bounded CreateEventA supports the reached unnamed event variant",
        ));
    }
    let (handle, _) = session
        .runtime_mut()
        .create_event(manual_reset, initial_state)
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    Ok(ApiImmediate::new(ApiReturnValue::LogicalHandle {
        namespace: RUNTIME_EVENT_OBJECT_NAMESPACE.to_owned(),
        value: handle.0,
    })
    .with_last_error(ERROR_SUCCESS))
}

fn handle_set_event(
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
    set: bool,
) -> Result<ApiImmediate, ApiFault> {
    let handle = RuntimeHandle(required_argument(contract, frame, 0, "event handle")?);
    let result = if set {
        session.runtime_mut().set_event(handle)
    } else {
        session.runtime_mut().reset_event(handle)
    };
    match result {
        Ok(_) => Ok(ApiImmediate::new(ApiReturnValue::Bool32(true))),
        Err(error) if error.class != RuntimeStateErrorClass::RuntimeDefect => {
            Ok(ApiImmediate::new(ApiReturnValue::Bool32(false))
                .with_last_error(ERROR_INVALID_HANDLE))
        }
        Err(error) => Err(ApiFault::from_runtime(error, &contract.endpoint)),
    }
}

fn handle_wait_for_single_object(
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let handle = RuntimeHandle(required_argument(contract, frame, 0, "wait handle")?);
    let timeout = required_argument(contract, frame, 1, "timeout")?;
    let preserved_last_error = session
        .last_error()
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    if handle.kind() != Ok(RuntimeObjectKind::Event)
        || session.runtime().event_for_handle(handle).is_err()
    {
        return Ok(ApiImmediate::new(ApiReturnValue::U32(WAIT_FAILED))
            .with_last_error(ERROR_INVALID_HANDLE));
    }
    if timeout == 0 {
        let signaled = session
            .runtime_mut()
            .try_wait_event(handle)
            .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
        return Ok(ApiImmediate::new(ApiReturnValue::U32(if signaled {
            WAIT_OBJECT_0
        } else {
            WAIT_TIMEOUT
        }))
        .with_last_error(preserved_last_error));
    }
    if timeout != u32::MAX {
        return Err(ApiFault::unsupported(
            "win32-wait",
            "wait-for-single-object-finite-timeout-unsupported",
            "bounded event WaitForSingleObject supports zero or infinite timeout",
        ));
    }
    let wait = session
        .runtime_mut()
        .wait_current_thread_on(handle)
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    match wait {
        RuntimeWaitDisposition::AlreadySignaled { .. } => {
            Ok(ApiImmediate::new(ApiReturnValue::U32(WAIT_OBJECT_0))
                .with_last_error(preserved_last_error))
        }
        RuntimeWaitDisposition::Pending { .. } => {
            Ok(ApiImmediate::new(ApiReturnValue::U32(WAIT_OBJECT_0))
                .with_last_error(preserved_last_error)
                .with_execution_boundary(RuntimeApiBoundaryDisposition::BlockCurrentThread))
        }
        RuntimeWaitDisposition::MutexAcquired { .. } => Err(ApiFault::runtime_defect(
            "win32-wait",
            "event-wait-returned-mutex-acquisition",
            "event wait unexpectedly returned a mutex acquisition",
        )),
    }
}

fn handle_wait_for_multiple_objects(
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> ApiHandlerResult {
    let result = (|| {
        let wait_all = required_argument(contract, frame, 2, "wait-all")? != 0;
        let timeout = required_argument(contract, frame, 3, "timeout")?;
        if wait_all {
            return Err(ApiFault::unsupported(
                "win32-wait",
                "wait-for-multiple-objects-wait-all-unsupported",
                "reached DirectSound event wait requires wait-any semantics",
            ));
        }
        let handles = read_wait_handles(contract, frame, session)?;
        let preserved_last_error = session
            .last_error()
            .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
        for (index, &handle) in handles.iter().enumerate() {
            let event = match session.runtime().event_for_handle(handle) {
                Ok(event) => event,
                Err(error) if error.class != RuntimeStateErrorClass::RuntimeDefect => {
                    return Ok(ApiHandlerResult::Immediate(
                        ApiImmediate::new(ApiReturnValue::U32(WAIT_FAILED))
                            .with_last_error(ERROR_INVALID_HANDLE),
                    ));
                }
                Err(error) => return Err(ApiFault::from_runtime(error, &contract.endpoint)),
            };
            if matches!(event.wait_state, RuntimeWaitState::Signaled { .. }) {
                session
                    .runtime_mut()
                    .try_wait_event(handle)
                    .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
                return Ok(ApiHandlerResult::Immediate(
                    ApiImmediate::new(ApiReturnValue::U32(WAIT_OBJECT_0 + index as u32))
                        .with_last_error(preserved_last_error),
                ));
            }
        }
        if timeout == 0 {
            return Ok(ApiHandlerResult::Immediate(
                ApiImmediate::new(ApiReturnValue::U32(WAIT_TIMEOUT))
                    .with_last_error(preserved_last_error),
            ));
        }
        let begin = match session
            .runtime_mut()
            .begin_audio_notification_wait(&handles, timeout)
        {
            Ok(begin) => begin,
            Err(error) if error.code == "audio-notification-wait-buffer-unknown" => {
                return Err(ApiFault::unsupported(
                    "win32-wait",
                    "wait-for-multiple-objects-source-unsupported",
                    "finite wait handle array is not owned by a playing DirectSound notification buffer",
                ));
            }
            Err(error) => return Err(ApiFault::from_runtime(error, &contract.endpoint)),
        };
        Ok(ApiHandlerResult::Deferred(crate::api::HostRequest {
            request_id: begin.request.request_id,
            service: begin.request.service,
            payload: begin.payload,
        }))
    })();
    match result {
        Ok(outcome) => outcome,
        Err(fault) => ApiHandlerResult::Fault(fault),
    }
}

fn read_wait_handles(
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &ExecutionSession<'_>,
) -> Result<Vec<RuntimeHandle>, ApiFault> {
    let count =
        usize::try_from(required_argument(contract, frame, 0, "handle count")?).map_err(|_| {
            ApiFault::invalid_call(
                "win32-wait",
                "wait-handle-count-unrepresentable",
                "wait handle count does not fit host accounting",
            )
        })?;
    let pointer = required_argument(contract, frame, 1, "handle array")?;
    if count == 0 || count > RUNTIME_AUDIO_NOTIFICATION_LIMIT || pointer == 0 {
        return Err(ApiFault::invalid_call(
            "win32-wait",
            "wait-handle-array-invalid",
            "wait handle array must be bounded, nonempty, and nonnull",
        ));
    }
    let bytes = session
        .memory()
        .read(GuestAddress(pointer), count * 4)
        .map_err(|error| ApiFault::from_memory(error, &contract.endpoint))?;
    Ok((0..count)
        .map(|index| {
            let offset = index * 4;
            RuntimeHandle(u32::from_le_bytes(
                bytes[offset..offset + 4]
                    .try_into()
                    .expect("validated wait handle array byte range"),
            ))
        })
        .collect())
}

fn read_nul_terminated_ansi_mutex_name(
    contract: &ApiContract,
    session: &ExecutionSession<'_>,
    source: GuestAddress,
    policy: Kernel32CreateMutexAPolicy,
) -> Result<Vec<u8>, ApiFault> {
    let maximum = usize::try_from(policy.maximum_name_bytes).map_err(|_| {
        ApiFault::runtime_defect(
            "win32-mutex",
            "create-mutex-a-limit-not-representable",
            "validated ANSI mutex name limit does not fit host accounting",
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
        "win32-mutex",
        "create-mutex-a-name-not-terminated",
        format!(
            "ANSI mutex name did not terminate within the policy limit of {} bytes",
            policy.maximum_name_bytes
        ),
    ))
}

fn handle_create_waitable_timer_a(
    policy: &Kernel32CompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let timer_policy = policy.create_waitable_timer_a().ok_or_else(|| {
        ApiFault::unsupported(
            "win32-waitable-timer",
            "create-waitable-timer-a-policy-disabled",
            format!(
                "compatibility policy {} does not enable CreateWaitableTimerA",
                policy.id
            ),
        )
    })?;
    let security_attributes = required_argument(contract, frame, 0, "security attributes")?;
    let manual_reset = required_argument(contract, frame, 1, "manual reset")? != 0;
    let name_address = required_argument(contract, frame, 2, "ANSI timer name")?;
    if security_attributes != 0 {
        return Err(ApiFault::unsupported(
            "win32-waitable-timer",
            "create-waitable-timer-a-security-attributes-unsupported",
            "the selected CreateWaitableTimerA policy enables only null security attributes",
        ));
    }
    if name_address != 0 || !timer_policy.allow_unnamed {
        return Err(ApiFault::unsupported(
            "win32-waitable-timer",
            "create-waitable-timer-a-named-unsupported",
            "the selected CreateWaitableTimerA policy enables only unnamed timers",
        ));
    }
    if (manual_reset && !timer_policy.allow_manual_reset)
        || (!manual_reset && !timer_policy.allow_auto_reset)
    {
        return Err(ApiFault::unsupported(
            "win32-waitable-timer",
            "create-waitable-timer-a-reset-mode-unsupported",
            format!(
                "compatibility policy {} does not enable the requested timer reset mode",
                policy.id
            ),
        ));
    }
    let result = session
        .runtime_mut()
        .create_waitable_timer_a(manual_reset)
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    Ok(ApiImmediate::new(ApiReturnValue::LogicalHandle {
        namespace: RUNTIME_WAITABLE_TIMER_OBJECT_NAMESPACE.to_owned(),
        value: result.handle.0,
    })
    .with_last_error(ERROR_SUCCESS))
}

fn handle_set_waitable_timer(
    policy: &Kernel32CompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let timer_policy = policy.set_waitable_timer().ok_or_else(|| {
        ApiFault::unsupported(
            "win32-waitable-timer",
            "set-waitable-timer-policy-disabled",
            format!(
                "compatibility policy {} does not enable SetWaitableTimer",
                policy.id
            ),
        )
    })?;
    let handle = RuntimeHandle(required_argument(
        contract,
        frame,
        0,
        "waitable timer handle",
    )?);
    let due_time_address = GuestAddress(required_argument(contract, frame, 1, "due time")?);
    let period_milliseconds =
        i32::from_le_bytes(required_argument(contract, frame, 2, "period")?.to_le_bytes());
    let completion_routine = required_argument(contract, frame, 3, "completion routine")?;
    let completion_context = required_argument(contract, frame, 4, "completion context")?;
    let resume = required_argument(contract, frame, 5, "resume")? != 0;
    if period_milliseconds != 0 || !timer_policy.allow_one_shot {
        return Err(ApiFault::unsupported(
            "win32-waitable-timer",
            "set-waitable-timer-periodic-unsupported",
            "the selected SetWaitableTimer policy enables only one-shot timers",
        ));
    }
    if completion_routine != 0 || completion_context != 0 {
        return Err(ApiFault::unsupported(
            "win32-waitable-timer",
            "set-waitable-timer-completion-routine-unsupported",
            "the selected SetWaitableTimer policy enables no completion routine or context",
        ));
    }
    if resume {
        return Err(ApiFault::unsupported(
            "win32-waitable-timer",
            "set-waitable-timer-resume-unsupported",
            "the selected SetWaitableTimer policy enables no resume request",
        ));
    }
    if session.runtime().waitable_timer_for_handle(handle).is_err() {
        return Ok(
            ApiImmediate::new(ApiReturnValue::Bool32(false)).with_last_error(ERROR_INVALID_HANDLE)
        );
    }
    let bytes = session
        .memory()
        .read(due_time_address, size_of::<i64>())
        .map_err(|error| ApiFault::from_memory(error, &contract.endpoint))?;
    let due_time_100ns = i64::from_le_bytes(bytes.try_into().map_err(|_| {
        ApiFault::runtime_defect(
            "win32-waitable-timer",
            "set-waitable-timer-due-time-read-size-invalid",
            "checked waitable timer due-time input did not contain one LARGE_INTEGER",
        )
    })?);
    if due_time_100ns >= 0 {
        return Err(ApiFault::unsupported(
            "win32-waitable-timer",
            "set-waitable-timer-absolute-due-time-unsupported",
            "the selected SetWaitableTimer policy enables only negative relative due times",
        ));
    }
    let relative_due_time_100ns = due_time_100ns.unsigned_abs();
    if !(timer_policy.minimum_relative_due_time_100ns
        ..=timer_policy.maximum_relative_due_time_100ns)
        .contains(&relative_due_time_100ns)
    {
        return Err(ApiFault::unsupported(
            "win32-waitable-timer",
            "set-waitable-timer-relative-due-time-unsupported",
            format!(
                "compatibility policy {} enables relative due times from {} through {} 100ns units, not {}",
                policy.id,
                timer_policy.minimum_relative_due_time_100ns,
                timer_policy.maximum_relative_due_time_100ns,
                relative_due_time_100ns,
            ),
        ));
    }
    session
        .runtime_mut()
        .set_waitable_timer(handle, relative_due_time_100ns)
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    Ok(ApiImmediate::new(ApiReturnValue::Bool32(true)).with_last_error(ERROR_SUCCESS))
}

fn handle_wait_for_single_object_ex(
    policy: &Kernel32CompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> ApiHandlerResult {
    let result = (|| {
        let wait_policy = policy.wait_for_single_object_ex().ok_or_else(|| {
            ApiFault::unsupported(
                "win32-wait",
                "wait-for-single-object-ex-policy-disabled",
                format!(
                    "compatibility policy {} does not enable WaitForSingleObjectEx",
                    policy.id
                ),
            )
        })?;
        let handle = RuntimeHandle(required_argument(contract, frame, 0, "wait handle")?);
        let timeout_milliseconds = required_argument(contract, frame, 1, "timeout")?;
        let alertable = required_argument(contract, frame, 2, "alertable")? != 0;
        if timeout_milliseconds > wait_policy.maximum_timeout_milliseconds {
            return Err(ApiFault::unsupported(
                "win32-wait",
                "wait-for-single-object-ex-timeout-unsupported",
                format!(
                    "compatibility policy {} enables finite timeouts through {} milliseconds, not {}",
                    policy.id, wait_policy.maximum_timeout_milliseconds, timeout_milliseconds
                ),
            ));
        }
        if (alertable && !wait_policy.allow_alertable)
            || (!alertable && !wait_policy.allow_nonalertable)
        {
            return Err(ApiFault::unsupported(
                "win32-wait",
                "wait-for-single-object-ex-alertable-mode-unsupported",
                format!(
                    "compatibility policy {} does not enable the requested alertable mode",
                    policy.id
                ),
            ));
        }
        if !wait_policy.waitable_timer_only {
            return Err(ApiFault::runtime_defect(
                "win32-wait",
                "wait-for-single-object-ex-policy-object-band-invalid",
                "validated WaitForSingleObjectEx policy lost its timer-only object scope",
            ));
        }
        match session.runtime().waitable_timer_for_handle(handle) {
            Ok(_) => {}
            Err(error) if error.class != RuntimeStateErrorClass::RuntimeDefect => {
                return Ok(ApiHandlerResult::Immediate(
                    ApiImmediate::new(ApiReturnValue::U32(WAIT_FAILED))
                        .with_last_error(ERROR_INVALID_HANDLE),
                ));
            }
            Err(error) => return Err(ApiFault::from_runtime(error, &contract.endpoint)),
        }
        let preserved_last_error = session
            .last_error()
            .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
        let begin = session
            .runtime_mut()
            .begin_wait_for_single_object_ex(handle, timeout_milliseconds, alertable)
            .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
        match begin {
            RuntimeWaitForSingleObjectExBegin::Immediate { result } => Ok(
                ApiHandlerResult::Immediate(wait_result_immediate(result, preserved_last_error)),
            ),
            RuntimeWaitForSingleObjectExBegin::Deferred { registration } => {
                let pacing = registration.waitable_timer_pacing.ok_or_else(|| {
                    ApiFault::runtime_defect(
                        "win32-wait",
                        "waitable-timer-pacing-contract-missing",
                        "deferred WaitForSingleObjectEx registration has no pacing contract",
                    )
                })?;
                Ok(ApiHandlerResult::Deferred(crate::api::HostRequest {
                    request_id: pacing.request_id,
                    service: RUNTIME_WAITABLE_TIMER_PACING_SERVICE.to_owned(),
                    payload: pacing.encode_host_payload(registration.id).to_vec(),
                }))
            }
        }
    })();
    match result {
        Ok(outcome) => outcome,
        Err(fault) => ApiHandlerResult::Fault(fault),
    }
}

fn wait_result_immediate(
    result: RuntimeWaitForSingleObjectExResult,
    preserved_last_error: u32,
) -> ApiImmediate {
    let value = match result.outcome {
        RuntimeWaitForSingleObjectExOutcome::ObjectSignaled => WAIT_OBJECT_0,
        RuntimeWaitForSingleObjectExOutcome::TimedOut => WAIT_TIMEOUT,
    };
    ApiImmediate::new(ApiReturnValue::U32(value)).with_last_error(preserved_last_error)
}

fn handle_sleep(
    policy: &Kernel32CompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> ApiHandlerResult {
    let result = (|| {
        let milliseconds = required_argument(contract, frame, 0, "sleep interval")?;
        if !policy.allows_sleep_zero() {
            return Err(ApiFault::unsupported(
                "win32-thread-scheduling",
                "sleep-zero-policy-disabled",
                format!(
                    "compatibility policy {} does not enable Sleep(0)",
                    policy.id
                ),
            ));
        }
        if milliseconds == 0 {
            return Ok(ApiHandlerResult::Immediate(
                ApiImmediate::new(ApiReturnValue::Void)
                    .with_execution_boundary(RuntimeApiBoundaryDisposition::YieldCurrentThread),
            ));
        }
        if policy
            .maximum_sleep_milliseconds()
            .is_none_or(|maximum| milliseconds > maximum)
        {
            return Err(ApiFault::unsupported(
                "win32-thread-scheduling",
                "sleep-interval-unsupported",
                format!(
                    "compatibility policy {} does not enable Sleep({milliseconds})",
                    policy.id
                ),
            ));
        }
        let request = session
            .runtime_mut()
            .begin_host_request(RUNTIME_SLEEP_DELAY_SERVICE)
            .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
        Ok(ApiHandlerResult::Deferred(crate::api::HostRequest {
            request_id: request.request_id,
            service: request.service,
            payload: milliseconds.to_le_bytes().to_vec(),
        }))
    })();
    match result {
        Ok(outcome) => outcome,
        Err(fault) => ApiHandlerResult::Fault(fault),
    }
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
    policy: &Kernel32CompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let output = required_argument(contract, frame, 0, "performance counter output")?;
    let counter = match policy.performance_counter_increment_per_read() {
        Some(increment) => session
            .runtime_mut()
            .sample_performance_counter(increment)
            .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?,
        None => session.runtime().clock().performance_counter,
    };
    Ok(ApiImmediate::new(ApiReturnValue::Bool32(true))
        .with_guest_write(GuestAddress(output), counter.to_le_bytes().to_vec()))
}

fn handle_query_performance_frequency(
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let output = required_argument(contract, frame, 0, "performance frequency output")?;
    let frequency = session.runtime().clock().performance_frequency;
    Ok(ApiImmediate::new(ApiReturnValue::Bool32(true))
        .with_guest_write(GuestAddress(output), frequency.to_le_bytes().to_vec()))
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

fn handle_heap_realloc(
    policy: &Kernel32CompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let heap = required_argument(contract, frame, 0, "heap handle")?;
    let flags = required_argument(contract, frame, 1, "heap reallocation flags")?;
    let allocation = required_argument(contract, frame, 2, "heap allocation")?;
    let byte_length = required_argument(contract, frame, 3, "heap reallocation byte length")?;
    if !policy.allows_heap_allocation(flags, byte_length) {
        return Err(ApiFault::unsupported(
            "win32-heap",
            "heap-realloc-variant-unsupported",
            format!(
                "compatibility policy {} does not enable HeapReAlloc flags {flags:#010x} and size {byte_length:#x}",
                policy.id
            ),
        ));
    }
    let previous = session
        .runtime()
        .private_heap_allocation(heap, GuestAddress(allocation))
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    if previous.byte_length > RUNTIME_PRIVATE_HEAP_ARENA_STRIDE
        || byte_length > RUNTIME_PRIVATE_HEAP_ARENA_STRIDE
    {
        return Err(ApiFault::unsupported(
            "win32-heap",
            "heap-realloc-large-allocation-unsupported",
            "HeapReAlloc does not yet move lazy large-slot allocations",
        ));
    }
    let initialization = match flags {
        0 => RuntimeHeapInitialization::DeterministicUninitialized,
        HEAP_ZERO_MEMORY => RuntimeHeapInitialization::Zeroed,
        _ => {
            return Err(ApiFault::runtime_defect(
                "win32-heap",
                "validated-heap-realloc-flags-unimplemented",
                "validated HeapReAlloc policy selected flags without semantic initialization",
            ));
        }
    };
    let allocation = session
        .reallocate_private_heap_block(heap, GuestAddress(allocation), byte_length, initialization)
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

fn handle_load_library_a(
    policy: &Kernel32CompatibilityPolicy,
    contract: &ApiContract,
    frame: &ApiCallFrame,
    session: &mut ExecutionSession<'_>,
) -> Result<ApiImmediate, ApiFault> {
    let loader = policy.load_library_a().ok_or_else(|| {
        ApiFault::unsupported(
            "win32-module-loader",
            "load-library-a-policy-disabled",
            format!(
                "compatibility policy {} does not enable runtime-system module loading",
                policy.id
            ),
        )
    })?;
    let name_address = required_argument(contract, frame, 0, "module name")?;
    if name_address == 0 {
        return Ok(load_library_a_failure(ERROR_INVALID_PARAMETER));
    }
    let requested_name = read_load_library_a_name(
        contract,
        session,
        GuestAddress(name_address),
        loader.maximum_module_name_bytes as usize,
    )?;
    if requested_name.is_empty() {
        return Ok(load_library_a_failure(ERROR_INVALID_PARAMETER));
    }
    if !requested_name.to_ascii_lowercase().ends_with(".dll")
        || requested_name
            .bytes()
            .any(|byte| byte.is_ascii_control() || matches!(byte, b'/' | b'\\' | b':'))
    {
        return Err(ApiFault::unsupported(
            "win32-module-loader",
            "load-library-a-search-variant-unsupported",
            "LoadLibraryA currently accepts only an explicit ASCII DLL basename",
        ));
    }
    let Some(module_policy) = loader
        .runtime_system_modules
        .iter()
        .find(|module| module.canonical_name.eq_ignore_ascii_case(&requested_name))
    else {
        return Ok(load_library_a_failure(ERROR_MOD_NOT_FOUND));
    };
    for export in &module_policy.exports {
        if session
            .runtime()
            .dynamic_endpoint_by_name(&module_policy.canonical_name, export)
            .is_err()
        {
            return Err(ApiFault::runtime_defect(
                "win32-module-loader",
                "load-library-a-runtime-export-missing",
                format!(
                    "runtime-system module {} is missing its configured export {export}",
                    module_policy.canonical_name
                ),
            ));
        }
    }
    let preserved_last_error = session
        .last_error()
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    let loaded = session
        .runtime_mut()
        .load_current_thread_runtime_system_module(
            requested_name,
            module_policy.canonical_name.clone(),
        )
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    Ok(ApiImmediate::new(ApiReturnValue::LogicalHandle {
        namespace: LOGICAL_LOADED_MODULE_NAMESPACE.to_owned(),
        value: loaded.handle,
    })
    .with_last_error(preserved_last_error))
}

fn load_library_a_failure(last_error: u32) -> ApiImmediate {
    ApiImmediate::new(ApiReturnValue::LogicalHandle {
        namespace: LOGICAL_LOADED_MODULE_NAMESPACE.to_owned(),
        value: 0,
    })
    .with_last_error(last_error)
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
        session
            .runtime_mut()
            .wait_current_thread_for_critical_section(address)
            .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
        let section = session
            .runtime()
            .critical_section(address)
            .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
        return Ok(ApiImmediate::new(ApiReturnValue::Void)
            .with_guest_write(address, x86_critical_section_bytes(section))
            .with_execution_boundary(RuntimeApiBoundaryDisposition::BlockCurrentThread));
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

fn handle_delete_critical_section(
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
    session
        .runtime_mut()
        .delete_critical_section(address)
        .map_err(|error| ApiFault::from_runtime(error, &contract.endpoint))?;
    Ok(ApiImmediate::new(ApiReturnValue::Void)
        .with_guest_write(address, vec![0; X86_CRITICAL_SECTION_BYTE_LENGTH]))
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
    session
        .memory()
        .validate_access(
            address,
            X86_CRITICAL_SECTION_BYTE_LENGTH,
            MemoryAccess::Write,
        )
        .map_err(|error| ApiFault::from_memory(error, &contract.endpoint))?;
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

fn read_load_library_a_name(
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
                ApiFault::unsupported(
                    "win32-module-loader",
                    "load-library-a-encoding-unsupported",
                    "LoadLibraryA currently accepts only ASCII module names",
                )
            });
        }
        if !byte.is_ascii() {
            return Err(ApiFault::unsupported(
                "win32-module-loader",
                "load-library-a-encoding-unsupported",
                "LoadLibraryA currently accepts only ASCII module names",
            ));
        }
        bytes.push(byte);
    }
    Err(ApiFault::invalid_call(
        "win32-module-loader",
        "load-library-a-name-unterminated",
        "LoadLibraryA module name exceeds its bounded read",
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

pub fn get_system_time_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    kernel32_contract(
        endpoint,
        "GetSystemTime",
        vec![out_struct("systemTime", "SYSTEMTIME", 16)],
        ApiReturnSchema::Void,
        GET_SYSTEM_TIME_IMPLEMENTATION,
    )
}

pub fn raise_exception_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    let mut contract = kernel32_contract(
        endpoint,
        "RaiseException",
        vec![
            u32_argument("exceptionCode"),
            u32_argument("exceptionFlags"),
            u32_argument("numberOfArguments"),
            opaque_pointer("arguments", true, 4),
        ],
        ApiReturnSchema::Void,
        RAISE_EXCEPTION_IMPLEMENTATION,
    )?;
    contract.reentrancy_policy = ReentrancyPolicy::Allowed;
    contract.callback_policy = CallbackPolicy::Required;
    Ok(contract)
}

pub fn rtl_unwind_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    let mut contract = kernel32_contract(
        endpoint,
        "RtlUnwind",
        vec![
            opaque_pointer("targetFrame", true, 4),
            opaque_pointer("targetIp", true, 1),
            ApiArgumentSchema {
                name: "exceptionRecord".to_owned(),
                direction: ApiDirection::InOut,
                value: ApiValueSchema::Pointer {
                    constraint: ApiPointerConstraint {
                        nullable: true,
                        alignment: 4,
                        byte_length: X86_EXCEPTION_RECORD_BYTES as u32,
                        pointee: ApiPointeeSchema::Struct {
                            type_id: "EXCEPTION_RECORD".to_owned(),
                        },
                        write_requirement: ApiWriteRequirement::Optional,
                    },
                },
            },
            opaque_pointer("returnValue", true, 1),
        ],
        ApiReturnSchema::Void,
        RTL_UNWIND_IMPLEMENTATION,
    )?;
    contract.reentrancy_policy = ReentrancyPolicy::Allowed;
    contract.behavior = ApiBehavior::ImmediateWithCommittedIndirectGuestWrites;
    Ok(contract)
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

pub fn get_current_thread_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    kernel32_contract(
        endpoint,
        "GetCurrentThread",
        Vec::new(),
        ApiReturnSchema::U32,
        GET_CURRENT_THREAD_IMPLEMENTATION,
    )
}

pub fn set_thread_affinity_mask_contract(
    endpoint: ApiEndpointIdentity,
) -> Result<ApiContract, ApiFault> {
    let mut contract = kernel32_contract(
        endpoint,
        "SetThreadAffinityMask",
        vec![
            u32_argument("threadHandle"),
            u32_argument("threadAffinityMask"),
        ],
        ApiReturnSchema::U32,
        SET_THREAD_AFFINITY_MASK_IMPLEMENTATION,
    )?;
    contract.last_error_policy = LastErrorPolicy::FromHandler;
    Ok(contract)
}

pub fn set_thread_priority_contract(
    endpoint: ApiEndpointIdentity,
) -> Result<ApiContract, ApiFault> {
    let mut contract = kernel32_contract(
        endpoint,
        "SetThreadPriority",
        vec![u32_argument("threadHandle"), u32_argument("priority")],
        ApiReturnSchema::Bool32,
        SET_THREAD_PRIORITY_IMPLEMENTATION,
    )?;
    contract.last_error_policy = LastErrorPolicy::FromHandler;
    Ok(contract)
}

pub fn create_thread_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    let mut contract = kernel32_contract(
        endpoint,
        "CreateThread",
        vec![
            opaque_pointer("securityAttributes", true, 4),
            u32_argument("stackSize"),
            opaque_pointer("startAddress", false, 1),
            u32_argument("parameter"),
            u32_argument("creationFlags"),
            optional_out_struct("threadId", "DWORD", 4, 4),
        ],
        ApiReturnSchema::LogicalHandle {
            namespace: RUNTIME_THREAD_OBJECT_NAMESPACE.to_owned(),
            nullable: true,
        },
        CREATE_THREAD_IMPLEMENTATION,
    )?;
    contract.last_error_policy = LastErrorPolicy::FromHandler;
    Ok(contract)
}

pub fn resume_thread_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    let mut contract = kernel32_contract(
        endpoint,
        "ResumeThread",
        vec![u32_argument("threadHandle")],
        ApiReturnSchema::U32,
        RESUME_THREAD_IMPLEMENTATION,
    )?;
    contract.last_error_policy = LastErrorPolicy::FromHandler;
    Ok(contract)
}

pub fn get_system_info_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    kernel32_contract(
        endpoint,
        "GetSystemInfo",
        vec![out_struct("systemInfo", "SYSTEM_INFO", 36)],
        ApiReturnSchema::Void,
        GET_SYSTEM_INFO_IMPLEMENTATION,
    )
}

pub fn global_memory_status_contract(
    endpoint: ApiEndpointIdentity,
) -> Result<ApiContract, ApiFault> {
    kernel32_contract(
        endpoint,
        "GlobalMemoryStatus",
        vec![out_struct("memoryStatus", "MEMORYSTATUS", 32)],
        ApiReturnSchema::Void,
        GLOBAL_MEMORY_STATUS_IMPLEMENTATION,
    )
}

pub fn virtual_alloc_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    kernel32_contract(
        endpoint,
        "VirtualAlloc",
        vec![
            opaque_pointer("preferredAddress", true, 1),
            u32_argument("byteLength"),
            u32_argument("allocationType"),
            u32_argument("protection"),
        ],
        ApiReturnSchema::OpaquePointer { nullable: true },
        VIRTUAL_ALLOC_IMPLEMENTATION,
    )
}

pub fn get_version_ex_a_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    let mut contract = kernel32_contract(
        endpoint,
        "GetVersionExA",
        vec![dynamic_inout_struct(
            "versionInformation",
            "OSVERSIONINFOA/OSVERSIONINFOEXA.size-versioned",
            size_of::<u32>() as u32,
            X86_OS_VERSION_INFO_EX_A_BYTES,
        )],
        ApiReturnSchema::Bool32,
        GET_VERSION_EX_A_IMPLEMENTATION,
    )?;
    contract.last_error_policy = LastErrorPolicy::FromHandler;
    Ok(contract)
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

pub fn get_system_default_lang_id_contract(
    endpoint: ApiEndpointIdentity,
) -> Result<ApiContract, ApiFault> {
    kernel32_contract(
        endpoint,
        "GetSystemDefaultLangID",
        Vec::new(),
        ApiReturnSchema::U16,
        GET_SYSTEM_DEFAULT_LANG_ID_IMPLEMENTATION,
    )
}

pub fn get_drive_type_a_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    let mut contract = kernel32_contract(
        endpoint,
        "GetDriveTypeA",
        vec![dynamic_in_bytes("rootPath", true, 1, 1, 32_768)],
        ApiReturnSchema::U32,
        GET_DRIVE_TYPE_A_IMPLEMENTATION,
    )?;
    contract.last_error_policy = LastErrorPolicy::FromHandler;
    Ok(contract)
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

pub fn get_file_attributes_a_contract(
    endpoint: ApiEndpointIdentity,
) -> Result<ApiContract, ApiFault> {
    let mut contract = kernel32_contract(
        endpoint,
        "GetFileAttributesA",
        vec![dynamic_in_bytes("fileName", false, 1, 1, 32_768)],
        ApiReturnSchema::U32,
        GET_FILE_ATTRIBUTES_A_IMPLEMENTATION,
    )?;
    contract.last_error_policy = LastErrorPolicy::FromHandler;
    Ok(contract)
}

pub fn find_first_file_a_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    let mut contract = kernel32_contract(
        endpoint,
        "FindFirstFileA",
        vec![
            dynamic_in_bytes("fileName", false, 1, 1, 32_768),
            fallible_out_struct(
                "findFileData",
                "WIN32_FIND_DATAA",
                X86_WIN32_FIND_DATA_A_BYTES as u32,
            ),
        ],
        ApiReturnSchema::LogicalHandle {
            namespace: RUNTIME_FILE_SEARCH_NAMESPACE.to_owned(),
            nullable: false,
        },
        FIND_FIRST_FILE_A_IMPLEMENTATION,
    )?;
    contract.behavior = ApiBehavior::ImmediateWithCommittedIndirectGuestWrites;
    contract.last_error_policy = LastErrorPolicy::FromHandler;
    Ok(contract)
}

pub fn find_next_file_a_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    let mut contract = kernel32_contract(
        endpoint,
        "FindNextFileA",
        vec![
            logical_handle("fileSearch", RUNTIME_FILE_SEARCH_NAMESPACE, false),
            fallible_out_struct(
                "findFileData",
                "WIN32_FIND_DATAA",
                X86_WIN32_FIND_DATA_A_BYTES as u32,
            ),
        ],
        ApiReturnSchema::Bool32,
        FIND_NEXT_FILE_A_IMPLEMENTATION,
    )?;
    contract.behavior = ApiBehavior::ImmediateWithCommittedIndirectGuestWrites;
    contract.last_error_policy = LastErrorPolicy::FromHandler;
    Ok(contract)
}

pub fn find_close_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    let mut contract = kernel32_contract(
        endpoint,
        "FindClose",
        vec![logical_handle(
            "fileSearch",
            RUNTIME_FILE_SEARCH_NAMESPACE,
            false,
        )],
        ApiReturnSchema::Bool32,
        FIND_CLOSE_IMPLEMENTATION,
    )?;
    contract.last_error_policy = LastErrorPolicy::FromHandler;
    Ok(contract)
}

pub fn delete_file_a_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    let mut contract = kernel32_contract(
        endpoint,
        "DeleteFileA",
        vec![dynamic_in_bytes("fileName", false, 1, 1, 32_768)],
        ApiReturnSchema::Bool32,
        DELETE_FILE_A_IMPLEMENTATION,
    )?;
    contract.last_error_policy = LastErrorPolicy::FromHandler;
    Ok(contract)
}

pub fn create_directory_a_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    let mut contract = kernel32_contract(
        endpoint,
        "CreateDirectoryA",
        vec![
            dynamic_in_bytes("pathName", false, 1, 1, 32_768),
            opaque_pointer("securityAttributes", true, 4),
        ],
        ApiReturnSchema::Bool32,
        CREATE_DIRECTORY_A_IMPLEMENTATION,
    )?;
    contract.behavior = ApiBehavior::ImmediateOrDeferred;
    contract.last_error_policy = LastErrorPolicy::FromHandler;
    Ok(contract)
}

pub fn create_file_a_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    let mut contract = kernel32_contract(
        endpoint,
        "CreateFileA",
        vec![
            dynamic_in_bytes("fileName", false, 1, 1, 32_768),
            u32_argument("desiredAccess"),
            u32_argument("shareMode"),
            opaque_pointer("securityAttributes", true, 4),
            u32_argument("creationDisposition"),
            u32_argument("flagsAndAttributes"),
            logical_handle("templateFile", RUNTIME_SOURCE_FILE_NAMESPACE, true),
        ],
        ApiReturnSchema::LogicalHandle {
            namespace: RUNTIME_SOURCE_FILE_NAMESPACE.to_owned(),
            nullable: false,
        },
        CREATE_FILE_A_IMPLEMENTATION,
    )?;
    contract.last_error_policy = LastErrorPolicy::FromHandler;
    Ok(contract)
}

pub fn get_file_size_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    let mut contract = kernel32_contract(
        endpoint,
        "GetFileSize",
        vec![
            logical_handle("file", RUNTIME_SOURCE_FILE_NAMESPACE, false),
            optional_out_struct("fileSizeHigh", "DWORD", 4, 4),
        ],
        ApiReturnSchema::U32,
        GET_FILE_SIZE_IMPLEMENTATION,
    )?;
    contract.last_error_policy = LastErrorPolicy::FromHandler;
    Ok(contract)
}

pub fn read_file_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    let mut contract = kernel32_contract(
        endpoint,
        "ReadFile",
        vec![
            logical_handle("file", RUNTIME_SOURCE_FILE_NAMESPACE, false),
            dynamic_out_bytes("buffer", false, 1, 1, RUNTIME_SOURCE_READ_MAX_BYTES),
            u32_argument("bytesToRead"),
            out_struct("bytesRead", "DWORD", 4),
            opaque_pointer("overlapped", true, 4),
        ],
        ApiReturnSchema::Bool32,
        READ_FILE_IMPLEMENTATION,
    )?;
    contract.behavior = ApiBehavior::ImmediateOrDeferred;
    contract.last_error_policy = LastErrorPolicy::FromHandler;
    Ok(contract)
}

pub fn write_file_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    let mut contract = kernel32_contract(
        endpoint,
        "WriteFile",
        vec![
            logical_handle("file", RUNTIME_SOURCE_FILE_NAMESPACE, false),
            dynamic_in_bytes("buffer", false, 1, 1, RUNTIME_WRITABLE_FILE_MAX_IO_BYTES),
            u32_argument("bytesToWrite"),
            out_struct("bytesWritten", "DWORD", 4),
            opaque_pointer("overlapped", true, 4),
        ],
        ApiReturnSchema::Bool32,
        WRITE_FILE_IMPLEMENTATION,
    )?;
    contract.last_error_policy = LastErrorPolicy::FromHandler;
    Ok(contract)
}

pub fn flush_file_buffers_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    let mut contract = kernel32_contract(
        endpoint,
        "FlushFileBuffers",
        vec![logical_handle("file", RUNTIME_SOURCE_FILE_NAMESPACE, false)],
        ApiReturnSchema::Bool32,
        FLUSH_FILE_BUFFERS_IMPLEMENTATION,
    )?;
    contract.behavior = ApiBehavior::ImmediateOrDeferred;
    contract.last_error_policy = LastErrorPolicy::FromHandler;
    Ok(contract)
}

pub fn set_file_pointer_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    let mut contract = kernel32_contract(
        endpoint,
        "SetFilePointer",
        vec![
            logical_handle("file", RUNTIME_SOURCE_FILE_NAMESPACE, false),
            u32_argument("distanceLow"),
            opaque_pointer("distanceHigh", true, 4),
            u32_argument("moveMethod"),
        ],
        ApiReturnSchema::U32,
        SET_FILE_POINTER_IMPLEMENTATION,
    )?;
    contract.last_error_policy = LastErrorPolicy::FromHandler;
    Ok(contract)
}

pub fn close_handle_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    let mut contract = kernel32_contract(
        endpoint,
        "CloseHandle",
        vec![u32_argument("object")],
        ApiReturnSchema::Bool32,
        CLOSE_HANDLE_IMPLEMENTATION,
    )?;
    contract.behavior = ApiBehavior::ImmediateOrDeferred;
    contract.last_error_policy = LastErrorPolicy::FromHandler;
    Ok(contract)
}

pub fn create_mutex_a_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    let mut contract = kernel32_contract(
        endpoint,
        "CreateMutexA",
        vec![
            opaque_pointer("securityAttributes", true, 4),
            u32_argument("initialOwner"),
            dynamic_in_bytes("mutexName", true, 1, 1, 32_768),
        ],
        ApiReturnSchema::U32,
        CREATE_MUTEX_A_IMPLEMENTATION,
    )?;
    contract.last_error_policy = LastErrorPolicy::FromHandler;
    Ok(contract)
}

pub fn create_event_a_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    let mut contract = kernel32_contract(
        endpoint,
        "CreateEventA",
        vec![
            opaque_pointer("securityAttributes", true, 4),
            u32_argument("manualReset"),
            u32_argument("initialState"),
            opaque_pointer("eventName", true, 1),
        ],
        ApiReturnSchema::LogicalHandle {
            namespace: RUNTIME_EVENT_OBJECT_NAMESPACE.to_owned(),
            nullable: false,
        },
        CREATE_EVENT_A_IMPLEMENTATION,
    )?;
    contract.last_error_policy = LastErrorPolicy::FromHandler;
    Ok(contract)
}

pub fn set_event_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    let mut contract = kernel32_contract(
        endpoint,
        "SetEvent",
        vec![logical_handle(
            "event",
            RUNTIME_EVENT_OBJECT_NAMESPACE,
            false,
        )],
        ApiReturnSchema::Bool32,
        SET_EVENT_IMPLEMENTATION,
    )?;
    contract.last_error_policy = LastErrorPolicy::FromHandler;
    Ok(contract)
}

pub fn reset_event_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    let mut contract = kernel32_contract(
        endpoint,
        "ResetEvent",
        vec![logical_handle(
            "event",
            RUNTIME_EVENT_OBJECT_NAMESPACE,
            false,
        )],
        ApiReturnSchema::Bool32,
        RESET_EVENT_IMPLEMENTATION,
    )?;
    contract.last_error_policy = LastErrorPolicy::FromHandler;
    Ok(contract)
}

pub fn wait_for_single_object_contract(
    endpoint: ApiEndpointIdentity,
) -> Result<ApiContract, ApiFault> {
    let mut contract = kernel32_contract(
        endpoint,
        "WaitForSingleObject",
        vec![
            u32_argument("waitableObject"),
            u32_argument("timeoutMilliseconds"),
        ],
        ApiReturnSchema::U32,
        WAIT_FOR_SINGLE_OBJECT_IMPLEMENTATION,
    )?;
    contract.last_error_policy = LastErrorPolicy::FromHandler;
    Ok(contract)
}

pub fn wait_for_multiple_objects_contract(
    endpoint: ApiEndpointIdentity,
) -> Result<ApiContract, ApiFault> {
    let mut contract = kernel32_contract(
        endpoint,
        "WaitForMultipleObjects",
        vec![
            u32_argument("handleCount"),
            dynamic_in_bytes(
                "handles",
                false,
                4,
                4,
                (RUNTIME_AUDIO_NOTIFICATION_LIMIT * 4) as u32,
            ),
            u32_argument("waitAll"),
            u32_argument("timeoutMilliseconds"),
        ],
        ApiReturnSchema::U32,
        WAIT_FOR_MULTIPLE_OBJECTS_IMPLEMENTATION,
    )?;
    contract.behavior = ApiBehavior::ImmediateOrDeferred;
    contract.last_error_policy = LastErrorPolicy::FromHandler;
    Ok(contract)
}

pub fn create_waitable_timer_a_contract(
    endpoint: ApiEndpointIdentity,
) -> Result<ApiContract, ApiFault> {
    let mut contract = kernel32_contract(
        endpoint,
        "CreateWaitableTimerA",
        vec![
            opaque_pointer("securityAttributes", true, 4),
            u32_argument("manualReset"),
            opaque_pointer("timerName", true, 1),
        ],
        ApiReturnSchema::LogicalHandle {
            namespace: RUNTIME_WAITABLE_TIMER_OBJECT_NAMESPACE.to_owned(),
            nullable: false,
        },
        CREATE_WAITABLE_TIMER_A_IMPLEMENTATION,
    )?;
    contract.last_error_policy = LastErrorPolicy::FromHandler;
    Ok(contract)
}

pub fn set_waitable_timer_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    let mut contract = kernel32_contract(
        endpoint,
        "SetWaitableTimer",
        vec![
            logical_handle(
                "waitableTimer",
                RUNTIME_WAITABLE_TIMER_OBJECT_NAMESPACE,
                false,
            ),
            in_struct("dueTime", "LARGE_INTEGER", 8, 4),
            u32_argument("periodMilliseconds"),
            opaque_pointer("completionRoutine", true, 4),
            opaque_pointer("completionContext", true, 4),
            u32_argument("resume"),
        ],
        ApiReturnSchema::Bool32,
        SET_WAITABLE_TIMER_IMPLEMENTATION,
    )?;
    contract.last_error_policy = LastErrorPolicy::FromHandler;
    Ok(contract)
}

pub fn wait_for_single_object_ex_contract(
    endpoint: ApiEndpointIdentity,
) -> Result<ApiContract, ApiFault> {
    let mut contract = kernel32_contract(
        endpoint,
        "WaitForSingleObjectEx",
        vec![
            logical_handle(
                "waitableObject",
                RUNTIME_WAITABLE_TIMER_OBJECT_NAMESPACE,
                false,
            ),
            u32_argument("timeoutMilliseconds"),
            u32_argument("alertable"),
        ],
        ApiReturnSchema::U32,
        WAIT_FOR_SINGLE_OBJECT_EX_IMPLEMENTATION,
    )?;
    contract.behavior = ApiBehavior::ImmediateOrDeferred;
    contract.last_error_policy = LastErrorPolicy::FromHandler;
    Ok(contract)
}

pub fn sleep_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    let mut contract = kernel32_contract(
        endpoint,
        "Sleep",
        vec![u32_argument("milliseconds")],
        ApiReturnSchema::Void,
        SLEEP_ZERO_IMPLEMENTATION,
    )?;
    contract.behavior = ApiBehavior::ImmediateOrDeferred;
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

pub fn query_performance_frequency_contract(
    endpoint: ApiEndpointIdentity,
) -> Result<ApiContract, ApiFault> {
    kernel32_contract(
        endpoint,
        "QueryPerformanceFrequency",
        vec![out_struct("performanceFrequency", "LARGE_INTEGER", 8)],
        ApiReturnSchema::Bool32,
        QUERY_PERFORMANCE_FREQUENCY_IMPLEMENTATION,
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

pub fn heap_realloc_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    kernel32_contract(
        endpoint,
        "HeapReAlloc",
        vec![
            logical_handle("heap", LOGICAL_PRIVATE_HEAP_NAMESPACE, false),
            u32_argument("flags"),
            opaque_pointer("allocation", false, RUNTIME_PRIVATE_HEAP_ALIGNMENT),
            u32_argument("byteLength"),
        ],
        ApiReturnSchema::OpaquePointer { nullable: true },
        HEAP_REALLOC_IMPLEMENTATION,
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

pub fn load_library_a_contract(endpoint: ApiEndpointIdentity) -> Result<ApiContract, ApiFault> {
    let mut contract = kernel32_contract(
        endpoint,
        "LoadLibraryA",
        vec![nullable_input_string(
            "moduleName",
            ApiStringEncoding::Windows932,
            1,
            1,
            MAX_LOAD_LIBRARY_MODULE_NAME_BYTES as u32,
        )],
        ApiReturnSchema::LogicalHandle {
            namespace: LOGICAL_LOADED_MODULE_NAMESPACE.to_owned(),
            nullable: true,
        },
        LOAD_LIBRARY_A_IMPLEMENTATION,
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

pub fn delete_critical_section_contract(
    endpoint: ApiEndpointIdentity,
) -> Result<ApiContract, ApiFault> {
    kernel32_contract(
        endpoint,
        "DeleteCriticalSection",
        vec![inout_struct(
            "criticalSection",
            "RTL_CRITICAL_SECTION",
            X86_CRITICAL_SECTION_BYTE_LENGTH as u32,
        )],
        ApiReturnSchema::Void,
        DELETE_CRITICAL_SECTION_IMPLEMENTATION,
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

fn nullable_input_string(
    name: &str,
    encoding: ApiStringEncoding,
    alignment: u32,
    minimum_byte_length: u32,
    maximum_code_units: u32,
) -> ApiArgumentSchema {
    let mut argument = input_string(
        name,
        encoding,
        alignment,
        minimum_byte_length,
        maximum_code_units,
    );
    let ApiValueSchema::Pointer { constraint } = &mut argument.value else {
        unreachable!("input string helper did not create a pointer schema");
    };
    constraint.nullable = true;
    argument
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

fn fallible_out_struct(name: &str, type_id: &str, byte_length: u32) -> ApiArgumentSchema {
    let mut argument = out_struct(name, type_id, byte_length);
    let ApiValueSchema::Pointer { constraint } = &mut argument.value else {
        unreachable!("output struct helper did not create a pointer schema");
    };
    constraint.write_requirement = ApiWriteRequirement::Optional;
    argument
}

fn in_struct(name: &str, type_id: &str, byte_length: u32, alignment: u32) -> ApiArgumentSchema {
    ApiArgumentSchema {
        name: name.to_owned(),
        direction: ApiDirection::In,
        value: ApiValueSchema::Pointer {
            constraint: ApiPointerConstraint {
                nullable: false,
                alignment,
                byte_length,
                pointee: ApiPointeeSchema::Struct {
                    type_id: type_id.to_owned(),
                },
                write_requirement: ApiWriteRequirement::None,
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

fn dynamic_inout_struct(
    name: &str,
    type_id: &str,
    minimum_byte_length: u32,
    maximum_byte_length: u32,
) -> ApiArgumentSchema {
    ApiArgumentSchema {
        name: name.to_owned(),
        direction: ApiDirection::InOut,
        value: ApiValueSchema::DynamicPointer {
            constraint: ApiDynamicPointerConstraint {
                nullable: false,
                alignment: 4,
                minimum_byte_length,
                maximum_byte_length,
                pointee: ApiPointeeSchema::Struct {
                    type_id: type_id.to_owned(),
                },
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

#[cfg(test)]
mod tests {
    use super::system_time_bytes;

    #[test]
    fn file_time_is_projected_as_utc_system_time() {
        assert_eq!(
            system_time_bytes(134_282_016_000_000_000),
            [
                2026_u16.to_le_bytes(),
                7_u16.to_le_bytes(),
                6_u16.to_le_bytes(),
                11_u16.to_le_bytes(),
                0_u16.to_le_bytes(),
                0_u16.to_le_bytes(),
                0_u16.to_le_bytes(),
                0_u16.to_le_bytes(),
            ]
            .concat()
        );
    }

    #[test]
    fn file_time_projection_handles_epoch_and_leap_day_milliseconds() {
        assert_eq!(
            system_time_bytes(0),
            [1601_u16, 1, 1, 1, 0, 0, 0, 0]
                .into_iter()
                .flat_map(u16::to_le_bytes)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            system_time_bytes(133_537_247_981_230_000),
            [2024_u16, 2, 4, 29, 23, 59, 58, 123]
                .into_iter()
                .flat_map(u16::to_le_bytes)
                .collect::<Vec<_>>()
        );
    }
}
